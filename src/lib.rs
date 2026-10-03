//! Owned caller values for contained Windows SSPI authentication.
//!
//! Owned secret values, strict private codec and parent supervision are implemented.
//! Shared child bootstrap and serial Windows Negotiate/NTLM processing are implemented.
//! Native review, host integration and provider qualification remain gated.
#![doc = include_str!("../docs/CallerValues.md")]
#![doc = include_str!("../docs/Supervision.md")]
#![doc = include_str!("../docs/WorkerEntry.md")]
#![doc = include_str!("../docs/HostPackaging.md")]
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

mod worker;
pub use worker::{WorkerBootstrap, worker_entry};

pub mod packaging;
