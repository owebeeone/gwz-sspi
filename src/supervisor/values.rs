use super::control::Signal;
use crate::{Error, ErrorKind};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

/// Trusted absolute installed worker path and matching artifact-set digest.
pub struct WorkerExecutable {
    pub(crate) path: PathBuf,
    pub(crate) build: [u8; 32],
}
impl WorkerExecutable {
    /// Construct from the host's trusted installed path, never a PATH lookup.
    /// The digest identifies the matching installed library/worker artifact set.
    pub fn new(path: PathBuf, build_fingerprint: [u8; 32]) -> Result<Self, Error> {
        if !path.is_absolute() || path.as_os_str().is_empty() {
            return Err(Error::new(ErrorKind::InvalidRequest));
        }
        Ok(Self {
            path,
            build: build_fingerprint,
        })
    }
    /// Borrow the configured absolute path.
    pub fn path(&self) -> &Path {
        &self.path
    }
}
/// Bounded native conversation capacity; no CLI setting is introduced.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// Capacity 1–64; default eight. Quarantined workers retain their slots.
    pub max_workers: u8,
}
impl Default for Options {
    fn default() -> Self {
        Self { max_workers: 8 }
    }
}
impl Options {
    pub(super) fn validate(self) -> Result<Self, Error> {
        if !(1..=64).contains(&self.max_workers) {
            return Err(Error::new(ErrorKind::InvalidRequest));
        }
        Ok(self)
    }
}
/// Immutable absolute monotonic parent deadline, with no default or extension.
#[derive(Clone, Copy, Debug)]
pub struct Deadline(Instant);
impl Deadline {
    /// Use an existing host operation timestamp in the parent clock domain.
    pub const fn new(instant: Instant) -> Self {
        Self(instant)
    }
    /// Return the exact immutable timestamp.
    pub const fn instant(self) -> Instant {
        self.0
    }
}
/// Owned cancellation signal; clones observe and signal the same cancellation.
#[derive(Clone, Default)]
pub struct Cancellation(pub(super) Arc<Signal>);
impl Cancellation {
    /// Create an unsignalled signal. No callback into the host is stored.
    pub fn new() -> Self {
        Self::default()
    }
    /// Revoke result eligibility and notify retained supervision.
    pub fn cancel(&self) {
        self.0.cancel();
    }
    /// Observe whether cancellation was signalled.
    pub fn is_cancelled(&self) -> bool {
        self.0.cancelled()
    }
}
/// Opaque checked monotonic identity scoped to one supervisor context.
#[derive(Clone)]
pub struct RecordId {
    pub(super) context: Arc<()>,
    pub(super) sequence: u64,
}
impl PartialEq for RecordId {
    fn eq(&self, other: &Self) -> bool {
        self.sequence == other.sequence && Arc::ptr_eq(&self.context, &other.context)
    }
}
impl Eq for RecordId {}
impl std::fmt::Debug for RecordId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RecordId")
            .field("sequence", &self.sequence)
            .finish_non_exhaustive()
    }
}
/// Evidence of retained or confirmed local cleanup; eviction is never confirmation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CleanupStatus {
    /// Supervision still owns resources; no replacement slot is admitted.
    Pending,
    /// Held exit, empty Job and all launch/I/O ownership completed.
    Confirmed,
    /// Foreign, never issued or FIFO-evicted record identity.
    Unknown,
}
/// Fixed failure plus owned cleanup status; no secret/provider diagnostic text.
#[derive(Debug)]
pub struct Failure {
    pub(super) error: Error,
    pub(super) id: Option<RecordId>,
    pub(super) cleanup: CleanupStatus,
}
impl Failure {
    /// Fixed error classification.
    pub fn kind(&self) -> ErrorKind {
        self.error.kind()
    }
    /// Numeric SECURITY_STATUS, only when the provider supplied one.
    pub fn native_status(&self) -> Option<u32> {
        self.error.native_status()
    }
    /// Registered record, or None when refusal occurred before registration.
    pub fn record_id(&self) -> Option<&RecordId> {
        self.id.as_ref()
    }
    /// Cleanup evidence at the instant the failure was returned.
    pub fn cleanup_status(&self) -> CleanupStatus {
        self.cleanup
    }
}
/// Immediate cancellation result; Pending is not confirmation of termination.
#[derive(Debug)]
pub struct CancellationReceipt {
    /// Context-owned record identity for subsequent cleanup observation.
    pub record_id: RecordId,
    /// Current cleanup evidence.
    pub cleanup: CleanupStatus,
}
/// Explicit-deadline shutdown outcome; retained supervision continues afterwards.
#[derive(Debug)]
pub struct ShutdownReport {
    /// Number of records confirmed during this supervisor lifetime.
    pub confirmed: usize,
    /// Charged outstanding records at the shutdown observation point.
    pub outstanding: Vec<RecordId>,
}
