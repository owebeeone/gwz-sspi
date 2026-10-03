//! One serial worker conversation; fake and native ports use the same bridge.
mod bootstrap;
// Portable private bridges exist on non-Windows for deterministic unit tests.
#[allow(dead_code)]
mod framing;
#[allow(dead_code)]
mod native;
mod platform;
#[allow(dead_code)]
mod session;
#[allow(dead_code)]
mod storage;
pub use bootstrap::{WorkerBootstrap, worker_entry};
#[cfg(test)]
mod tests {
    include!("tests.rs");
}
