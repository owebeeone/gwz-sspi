mod api;
mod clock;
mod context;
mod control;
mod dispatch;
mod driver;
mod futures;
mod io;
pub(crate) mod kernel;
mod monitor;
mod platform;
pub(crate) mod ports;
mod values;
pub use api::{Conversation, Supervisor};
pub use values::{
    Cancellation, CancellationReceipt, CleanupStatus, Deadline, Failure, Options, RecordId,
    ShutdownReport, WorkerExecutable,
};
#[cfg(test)]
mod kernel_tests {
    include!("kernel_tests.rs");
}

#[cfg(test)]
mod tests {
    include!("test_support.rs");
    mod lifecycle {
        use super::*;
        include!("lifecycle_tests.rs");
    }
    mod control {
        use super::*;
        include!("control_tests.rs");
    }
    mod io {
        use super::*;
        include!("io_tests.rs");
    }
}
