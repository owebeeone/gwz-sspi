//! Owned caller values for contained Windows SSPI authentication.
//!
//! Owned secret values, strict private codec and parent supervision are implemented.
//! Native provider authentication and Windows runtime qualification remain deferred.
//! The optional worker executable refuses every invocation.
#![doc = include_str!("../docs/CallerValues.md")]
#![doc = include_str!("../docs/Supervision.md")]
#![deny(missing_docs)]
#[allow(dead_code)]
mod protocol;
mod secret;
mod values;
pub use secret::{SecretBytes, SecretText};
pub use values::{
    AuthRequest, DigestInput, Error, ErrorKind, Identity, Mechanism, MechanismObservation, Package,
    TokenLimit, TokenStatus, TokenStep,
};

mod supervisor;

pub use supervisor::{
    Cancellation, CancellationReceipt, CleanupStatus, Conversation, Deadline, Failure, Options,
    RecordId, ShutdownReport, Supervisor, WorkerExecutable,
};
