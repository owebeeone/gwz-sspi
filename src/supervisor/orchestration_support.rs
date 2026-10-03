use super::super::{
    dispatch, io, monitor,
    owners::Owner,
    ports::{Child, ReadPort, WritePort},
};
use crate::secret::Storage;
use std::sync::{Mutex, Weak, atomic::AtomicBool};
#[derive(Default)]
struct Effects {
    trace: Mutex<Vec<&'static str>>,
    context: Mutex<Weak<Context>>,
    sequence: AtomicUsize,
    resume_fail: AtomicBool,
    late_cancel: AtomicBool,
    launch_fail: AtomicBool,
    exited: AtomicBool,
    empty: AtomicBool,
    writes: AtomicUsize,
    origin_drops: AtomicUsize,
    child_drops: AtomicUsize,
}
impl Effects {
    fn event(&self, label: &'static str) {
        if let Some(context) = self.context.lock().unwrap().upgrade() {
            assert!(
                context.state.try_lock().is_ok(),
                "effect {label} under state lock"
            );
        }
        self.trace.lock().unwrap().push(label);
    }
}
struct Captured(Arc<Effects>);
impl Origin for Captured {
    fn verify(&self) -> Result<(), Error> {
        self.0.event("origin-verify");
        Ok(())
    }
}
impl Drop for Captured {
    fn drop(&mut self) {
        self.0.event("origin-drop");
        self.0.origin_drops.fetch_add(1, Ordering::SeqCst);
    }
}
struct Host {
    primary: Primary,
    effects: Arc<Effects>,
    contained: bool,
}
impl Platform for Host {
    fn primary(&self) -> &Primary {
        &self.primary
    }
    fn capture_origin(&self) -> Result<Box<dyn Origin>, Error> {
        self.effects.event("origin-capture");
        Ok(Box::new(Captured(self.effects.clone())))
    }
    fn launch(&self, _: &WorkerExecutable, _: Box<dyn Origin>) -> Result<Launched, Error> {
        self.effects.event("launch-return");
        if self.effects.launch_fail.load(Ordering::SeqCst) {
            return Err(Error::new(ErrorKind::WorkerUnavailable));
        }
        if self.effects.late_cancel.load(Ordering::SeqCst) {
            let context = self.effects.context.lock().unwrap().upgrade().unwrap();
            let record = {
                context
                    .state
                    .lock()
                    .unwrap()
                    .records
                    .get(&(self.effects.sequence.load(Ordering::SeqCst) as u64))
                    .unwrap()
                    .record
                    .clone()
            };
            context.fault(&record, Fault::new(ErrorKind::Cancelled));
        }
        Ok(Launched {
            child: Arc::new(HeldChild(self.effects.clone())),
            reader: Box::new(Input::new(
                self.effects.clone(),
                vec![
                    fixture("hello", None),
                    fixture("token_ntlm", Some(1)),
                    fixture("finished", None),
                ],
                1,
                0,
            )),
            writer: Box::new(Output(self.effects.clone(), 0)),
            contained: self.contained,
        })
    }
}
struct HeldChild(Arc<Effects>);
impl Child for HeldChild {
    fn resume(&self) -> Result<(), Error> {
        self.0.event("resume");
        if self.0.resume_fail.load(Ordering::SeqCst) {
            Err(Error::new(ErrorKind::ContainmentFailed))
        } else {
            Ok(())
        }
    }
    fn terminate(&self) -> bool {
        self.0.event("termination-request-failed");
        false
    }
    fn observe(&self) -> (bool, bool) {
        self.0.event("held-observation");
        (
            self.0.exited.load(Ordering::SeqCst),
            self.0.empty.load(Ordering::SeqCst),
        )
    }
    fn cancel_io(&self, _: &std::thread::JoinHandle<()>) {
        panic!("fake owners must not have native handles");
    }
}
impl Drop for HeldChild {
    fn drop(&mut self) {
        self.0.event("held-child-disposed");
        self.0.child_drops.fetch_add(1, Ordering::SeqCst);
    }
}
struct Completion {
    effects: Arc<Effects>,
    finished: AtomicBool,
    joined: AtomicBool,
    success: bool,
    label: &'static str,
}
struct HeldOwner(Arc<Completion>);
impl Owner for HeldOwner {
    fn is_finished(&self) -> bool {
        self.0.finished.load(Ordering::SeqCst)
    }
    fn join(self: Box<Self>) -> bool {
        assert!(self.0.finished.load(Ordering::SeqCst));
        self.0.effects.event(self.0.label);
        self.0.joined.store(true, Ordering::SeqCst);
        self.0.success
    }
    fn cancel(&self, _: &dyn Child) {
        self.0.effects.event("advisory-owner-cancel");
    }
}
impl Drop for HeldOwner {
    fn drop(&mut self) {
        self.0.effects.event("owner-disposed");
    }
}
fn owner(
    effects: &Arc<Effects>,
    label: &'static str,
    success: bool,
) -> (Box<dyn Owner>, Arc<Completion>) {
    let completion = Arc::new(Completion {
        effects: effects.clone(),
        finished: AtomicBool::new(false),
        joined: AtomicBool::new(false),
        success,
        label,
    });
    (Box::new(HeldOwner(completion.clone())), completion)
}
struct Input {
    effects: Arc<Effects>,
    frames: std::collections::VecDeque<Storage>,
    offset: usize,
    chunk: usize,
    fault: u8,
}
impl Input {
    fn new(effects: Arc<Effects>, frames: Vec<Storage>, chunk: usize, fault: u8) -> Self {
        Self {
            effects,
            frames: frames
                .into_iter()
                .map(|body| {
                    let mut frame = Storage::zeroed(4 + body.as_slice().len());
                    frame.as_mut()[..4]
                        .copy_from_slice(&(body.as_slice().len() as u32).to_le_bytes());
                    frame.as_mut()[4..].copy_from_slice(body.as_slice());
                    frame
                })
                .collect(),
            offset: 0,
            chunk,
            fault,
        }
    }
}
impl ReadPort for Input {
    fn read(&mut self, destination: &mut [u8]) -> Result<usize, Error> {
        self.effects.event("read");
        if self.fault == 1 {
            return Err(Error::new(ErrorKind::Protocol));
        }
        if self.fault == 2 {
            panic!("synthetic reader owner panic");
        }
        let Some(frame) = self.frames.front() else {
            return Ok(0);
        };
        let count = destination
            .len()
            .min(self.chunk)
            .min(frame.as_slice().len() - self.offset);
        destination[..count].copy_from_slice(&frame.as_slice()[self.offset..self.offset + count]);
        self.offset += count;
        if self.offset == frame.as_slice().len() {
            drop(self.frames.pop_front());
            self.offset = 0;
        }
        Ok(count)
    }
}
impl Drop for Input {
    fn drop(&mut self) {
        self.effects.event("read-port-disposed");
    }
}
struct Output(Arc<Effects>, u8);
impl WritePort for Output {
    fn write(&mut self, source: &[u8]) -> Result<usize, Error> {
        self.0.event("write");
        if self.1 == 1 {
            return Err(Error::new(ErrorKind::Protocol));
        }
        if self.1 == 2 {
            panic!("synthetic writer owner panic");
        }
        let count = source.len().min(3);
        self.0.writes.fetch_add(count, Ordering::SeqCst);
        Ok(count)
    }
}
impl Drop for Output {
    fn drop(&mut self) {
        self.0.event("write-port-disposed");
    }
}
fn host(contained: bool) -> (Arc<Context>, Arc<Effects>) {
    let effects = Arc::new(Effects::default());
    let base = context(1);
    let executable =
        WorkerExecutable::new(base.executable.path.clone(), base.executable.build).unwrap();
    let context = Context::new(
        executable,
        1,
        Arc::new(Host {
            primary: Primary {
                sid: SecretBytes::new(&[1, 0, 0, 0, 0, 0, 0, 0]),
                luid: SecretBytes::new(&[0; 8]),
                session: 0,
            },
            effects: effects.clone(),
            contained,
        }),
    );
    *effects.context.lock().unwrap() = Arc::downgrade(&context);
    (context, effects)
}
fn ticket(context: &Arc<Context>, effects: &Arc<Effects>) -> (Arc<Record>, Box<dyn Origin>) {
    let record = register(context);
    effects
        .sequence
        .store(record.sequence as usize, Ordering::SeqCst);
    let origin = context.platform.capture_origin().unwrap();
    (record, origin)
}
fn finish_launch(context: &Context, record: &Record, effects: &Arc<Effects>) {
    let (held, completion) = owner(effects, "launch-joined", true);
    *record.launch.lock().unwrap() = Some(held);
    assert!(context.reap(record.sequence).is_none());
    completion.finished.store(true, Ordering::SeqCst);
    dispatch::join_launch(context, record);
    assert!(completion.joined.load(Ordering::SeqCst));
    drop(context.reap(record.sequence).unwrap());
}

fn fixture(name: &str, round: Option<u8>) -> Storage {
    crate::protocol::supervision::fixtures::frame(name, round)
}
