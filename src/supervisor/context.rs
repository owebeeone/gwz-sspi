//! Context-owned records, admission and bounded cleanup tombstones.
use super::{
    control::{WakeHub, WakeSlot},
    kernel::{Fault, Kernel, Stage},
    ports::{Child, Origin, Platform},
    values::*,
};
use crate::secret::Storage;
use crate::{AuthRequest, ErrorKind, TokenStep};
use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak, mpsc::SyncSender};
use std::thread::JoinHandle;
use std::time::Instant;

pub(super) struct Payload {
    pub(super) request: Option<AuthRequest>,
    pub(super) token: Option<TokenStep>,
    pub(super) sender: Option<SyncSender<Storage>>,
}
#[derive(Clone, Copy)]
pub(super) struct Completion {
    pub(super) fault: Option<Fault>,
    pub(super) normal: bool,
}
pub(super) struct Tasks {
    pub(super) child: Option<Arc<dyn Child>>,
    pub(super) reader: Option<JoinHandle<()>>,
    pub(super) writer: Option<JoinHandle<()>>,
}
pub(super) struct Record {
    pub(super) sequence: u64,
    pub(super) deadline: Instant,
    pub(super) cancel: Cancellation,
    pub(super) stop: AtomicBool,
    pub(super) wake: Arc<WakeSlot>,
    pub(super) payload: Mutex<Payload>,
    pub(super) launch: Mutex<Option<JoinHandle<()>>>,
    pub(super) completion: OnceLock<Completion>,
    pub(super) tasks: Mutex<Tasks>,
    pub(super) package: crate::Package,
    pub(super) cap: crate::TokenLimit,
}
impl Record {
    pub(super) fn discard_payload(&self) {
        let discarded = {
            let mut payload = self.payload.lock().unwrap_or_else(|p| p.into_inner());
            (
                payload.request.take(),
                payload.token.take(),
                payload.sender.take(),
            )
        };
        drop(discarded);
    }
}
pub(super) struct Entry {
    pub(super) kernel: Kernel,
    pub(super) record: Arc<Record>,
}
pub(super) struct Waiter {
    pub(super) wake: Arc<WakeSlot>,
    pub(super) deadline: Instant,
    pub(super) cancel: Cancellation,
}
pub(super) struct WaitTicket {
    pub(super) owner: Weak<Waiter>,
    pub(super) deadline: Instant,
    pub(super) cancel: Cancellation,
}
pub(super) struct State {
    pub(super) closed: bool,
    pub(super) next: u64,
    pub(super) records: BTreeMap<u64, Entry>,
    pub(super) waiters: VecDeque<WaitTicket>,
    pub(super) tombstones: VecDeque<u64>,
    pub(super) confirmed: usize,
}
pub(super) type LaunchTicket = (Arc<Record>, Box<dyn Origin>);
pub(super) struct Context {
    pub(super) state: Mutex<State>,
    pub(super) clock: super::clock::Clock,
    pub(super) dispatch: Mutex<VecDeque<LaunchTicket>>,
    pub(super) hub: Arc<WakeHub>,
    pub(super) identity: Arc<()>,
    pub(super) capacity: usize,
    pub(super) platform: Arc<dyn Platform>,
    pub(super) executable: WorkerExecutable,
}
impl Context {
    pub(super) fn new(
        executable: WorkerExecutable,
        capacity: usize,
        platform: Arc<dyn Platform>,
    ) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(State {
                closed: false,
                next: 1,
                records: BTreeMap::new(),
                waiters: VecDeque::new(),
                tombstones: VecDeque::new(),
                confirmed: 0,
            }),
            clock: super::clock::Clock::default(),
            dispatch: Mutex::new(VecDeque::new()),
            hub: Arc::new(WakeHub::default()),
            identity: Arc::new(()),
            capacity,
            platform,
            executable,
        })
    }
    pub(super) fn id(&self, sequence: u64) -> RecordId {
        RecordId {
            context: self.identity.clone(),
            sequence,
        }
    }
    pub(super) fn status(&self, id: &RecordId) -> CleanupStatus {
        if !Arc::ptr_eq(&self.identity, &id.context) {
            return CleanupStatus::Unknown;
        }
        let state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        if state.records.contains_key(&id.sequence) {
            CleanupStatus::Pending
        } else if state.tombstones.contains(&id.sequence) {
            CleanupStatus::Confirmed
        } else {
            CleanupStatus::Unknown
        }
    }
    pub(super) fn enqueue(&self, waiter: &Arc<Waiter>) {
        self.hub.register(&waiter.wake);
        waiter.cancel.0.subscribe(&self.hub);
        self.state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .waiters
            .push_back(WaitTicket {
                owner: Arc::downgrade(waiter),
                deadline: waiter.deadline,
                cancel: waiter.cancel.clone(),
            });
        self.hub.notify();
    }
    pub(super) fn remove_waiter(&self, waiter: &Arc<Waiter>) {
        self.state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .waiters
            .retain(|entry| !entry.owner.ptr_eq(&Arc::downgrade(waiter)));
        self.hub.notify();
    }
    pub(super) fn register(
        &self,
        waiter: &Arc<Waiter>,
        request: &AuthRequest,
        now: Instant,
    ) -> Result<Option<Arc<Record>>, Fault> {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        let now = now.max(self.clock.now());
        if waiter.cancel.is_cancelled() {
            return Err(Fault::new(ErrorKind::Cancelled));
        }
        if now >= waiter.deadline {
            return Err(Fault::new(ErrorKind::Timeout));
        }
        if state.closed {
            return Err(Fault::new(ErrorKind::CapacityUnavailable));
        }
        state.waiters.retain(|entry| {
            entry.owner.strong_count() > 0 && now < entry.deadline && !entry.cancel.is_cancelled()
        });
        let first = state
            .waiters
            .front()
            .is_some_and(|first| first.owner.ptr_eq(&Arc::downgrade(waiter)));
        if !first || state.records.len() >= self.capacity {
            return Ok(None);
        }
        let sequence = state.next;
        state.next = state
            .next
            .checked_add(1)
            .ok_or_else(|| Fault::new(ErrorKind::CapacityUnavailable))?;
        let record = Arc::new(Record {
            sequence,
            deadline: waiter.deadline,
            cancel: waiter.cancel.clone(),
            stop: AtomicBool::new(false),
            wake: waiter.wake.clone(),
            payload: Mutex::new(Payload {
                request: None,
                token: None,
                sender: None,
            }),
            launch: Mutex::new(None),
            completion: OnceLock::new(),
            tasks: Mutex::new(Tasks {
                child: None,
                reader: None,
                writer: None,
            }),
            package: request.package,
            cap: request.token_limit,
        });
        state.records.insert(
            sequence,
            Entry {
                kernel: Kernel::new(waiter.deadline),
                record: record.clone(),
            },
        );
        state.waiters.pop_front();
        Ok(Some(record))
    }
    /// Closure operates only on the pure kernel; owned data never enters it.
    pub(super) fn update<T>(
        &self,
        record: &Record,
        now: Instant,
        transition: impl FnOnce(&mut Kernel) -> T,
    ) -> Option<T> {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        let now = now.max(self.clock.now());
        let entry = state.records.get_mut(&record.sequence)?;
        entry.kernel.control(now, record.cancel.is_cancelled());
        let result = transition(&mut entry.kernel);
        if entry.kernel.terminal.is_some() {
            record.stop.store(true, Ordering::Release);
        }
        Some(result)
    }
    pub(super) fn fault(&self, record: &Record, fault: Fault) {
        self.update(record, Instant::now(), |kernel| {
            kernel.fail(fault);
        });
        self.hub.notify();
    }
    pub(super) fn close(&self) {
        {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            state.closed = true;
            for entry in state.records.values_mut() {
                entry.kernel.fail(Fault::new(ErrorKind::Cancelled));
                entry.record.stop.store(true, Ordering::Release);
            }
        }
        self.hub.notify();
    }
    pub(super) fn failure(&self, record: Option<&Record>, fault: Fault) -> Failure {
        let id = record.map(|record| self.id(record.sequence));
        let cleanup = if record.is_some_and(|record| record.completion.get().is_some()) {
            CleanupStatus::Confirmed
        } else {
            id.as_ref()
                .map_or(CleanupStatus::Confirmed, |id| self.status(id))
        };
        Failure {
            error: fault.error(),
            id,
            cleanup,
        }
    }
    pub(super) fn snapshot(&self) -> ShutdownReport {
        let state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        ShutdownReport {
            confirmed: state.confirmed,
            outstanding: state
                .records
                .keys()
                .map(|sequence| self.id(*sequence))
                .collect(),
        }
    }
    pub(super) fn spawn(
        self: &Arc<Self>,
        record: Arc<Record>,
        origin: Box<dyn Origin>,
    ) -> Result<(), Fault> {
        self.dispatch
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push_back((record, origin));
        self.hub.notify();
        Ok(())
    }
    pub(super) fn spawn_launch(
        self: &Arc<Self>,
        record: Arc<Record>,
        origin: Box<dyn Origin>,
    ) -> Result<(), Fault> {
        let context = self.clone();
        let owned = record.clone();
        let handle = std::thread::Builder::new()
            .name("gwz-sspi-launch".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    super::monitor::run(context.clone(), owned.clone(), origin)
                }));
                if result.is_err() {
                    context.fault(&owned, Fault::new(ErrorKind::ContainmentFailed));
                    context.update(&owned, Instant::now(), |kernel| kernel.quarantined = true);
                    let _recovery = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        super::monitor::reap(context.clone(), owned.clone())
                    }));
                }
            })
            .map_err(|_| Fault::new(ErrorKind::ContainmentFailed));
        match handle {
            Ok(handle) => {
                *record.launch.lock().unwrap_or_else(|p| p.into_inner()) = Some(handle);
                self.hub.notify();
                Ok(())
            }
            Err(fault) => {
                record.discard_payload();
                self.update(&record, Instant::now(), |kernel| {
                    kernel.fail(fault);
                    kernel.proof = super::kernel::Proof {
                        process_exited: true,
                        job_empty: true,
                        launch_finished: true,
                        reader_finished: true,
                        writer_finished: true,
                    };
                });
                self.hub.notify();
                Err(fault)
            }
        }
    }
    pub(super) fn reap(&self, sequence: u64) -> Option<Entry> {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        if !state.records.get(&sequence)?.kernel.reapable() {
            return None;
        }
        let entry = state.records.get(&sequence)?;
        let _set = entry.record.completion.set(Completion {
            fault: entry.kernel.terminal,
            normal: entry.kernel.normal_ack(),
        });
        let mut removed = state.records.remove(&sequence)?;
        removed.kernel.stage = Stage::Reaped;
        state.tombstones.push_back(sequence);
        if state.tombstones.len() > 256 {
            state.tombstones.pop_front();
        }
        state.confirmed = state.confirmed.saturating_add(1);
        Some(removed)
    }
}
