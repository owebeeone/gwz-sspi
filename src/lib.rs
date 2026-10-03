//! Owned caller values for contained Windows SSPI authentication.
//!
//! Secret buffers and the private strict codec are implemented. Supervisor,
//! Conversation, native authentication and process containment remain unimplemented.
//! The optional worker executable refuses every invocation.
#![doc = include_str!("../docs/CallerValues.md")]
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
