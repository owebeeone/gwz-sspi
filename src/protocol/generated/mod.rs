// Generated from protocol/sspi.ir.json by rust_projection.py; do not edit.
use super::cbor::{Reader, Sink};
use crate::{Error as CodecError, ErrorKind as CodecErrorKind};
use crate::{SecretBytes, SecretText};
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum MessageKind {
    Hello = 1,
    Begin = 2,
    Challenge = 3,
    Token = 4,
    Finish = 5,
    Finished = 6,
    Error = 7,
}
impl MessageKind {
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        match r.uint()? {
            2 => Ok(Self::Begin),
            3 => Ok(Self::Challenge),
            7 => Ok(Self::Error),
            5 => Ok(Self::Finish),
            6 => Ok(Self::Finished),
            1 => Ok(Self::Hello),
            4 => Ok(Self::Token),
            _ => Err(CodecError::new(CodecErrorKind::Protocol)),
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Package {
    Negotiate = 1,
    Ntlm = 2,
    Digest = 3,
}
impl Package {
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        match r.uint()? {
            3 => Ok(Self::Digest),
            1 => Ok(Self::Negotiate),
            2 => Ok(Self::Ntlm),
            _ => Err(CodecError::new(CodecErrorKind::Protocol)),
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum IdentityMode {
    CurrentLogon = 1,
    Explicit = 2,
}
impl IdentityMode {
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        match r.uint()? {
            1 => Ok(Self::CurrentLogon),
            2 => Ok(Self::Explicit),
            _ => Err(CodecError::new(CodecErrorKind::Protocol)),
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum TokenStatus {
    ContinueNeeded = 1,
    Complete = 2,
}
impl TokenStatus {
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        match r.uint()? {
            2 => Ok(Self::Complete),
            1 => Ok(Self::ContinueNeeded),
            _ => Err(CodecError::new(CodecErrorKind::Protocol)),
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ObservationKind {
    Unresolved = 1,
    Selected = 2,
}
impl ObservationKind {
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        match r.uint()? {
            2 => Ok(Self::Selected),
            1 => Ok(Self::Unresolved),
            _ => Err(CodecError::new(CodecErrorKind::Protocol)),
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Mechanism {
    Kerberos = 1,
    Ntlm = 2,
    Digest = 3,
}
impl Mechanism {
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        match r.uint()? {
            3 => Ok(Self::Digest),
            1 => Ok(Self::Kerberos),
            2 => Ok(Self::Ntlm),
            _ => Err(CodecError::new(CodecErrorKind::Protocol)),
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ErrorKind {
    InvalidRequest = 1,
    Protocol = 2,
    IdentityMismatch = 3,
    ProviderRejected = 4,
    Internal = 5,
}
impl ErrorKind {
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        match r.uint()? {
            3 => Ok(Self::IdentityMismatch),
            5 => Ok(Self::Internal),
            1 => Ok(Self::InvalidRequest),
            2 => Ok(Self::Protocol),
            4 => Ok(Self::ProviderRejected),
            _ => Err(CodecError::new(CodecErrorKind::Protocol)),
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ErrorPhase {
    Bootstrap = 1,
    Begin = 2,
    Challenge = 3,
    Finish = 4,
}
impl ErrorPhase {
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        match r.uint()? {
            2 => Ok(Self::Begin),
            1 => Ok(Self::Bootstrap),
            3 => Ok(Self::Challenge),
            4 => Ok(Self::Finish),
            _ => Err(CodecError::new(CodecErrorKind::Protocol)),
        }
    }
}
mod primary_identity;
pub(super) use primary_identity::{PrimaryIdentityOwned, PrimaryIdentityRef};
mod hello;
pub(super) use hello::{HelloOwned, HelloRef};
mod identity;
pub(super) use identity::{IdentityOwned, IdentityRef};
mod digest_input;
pub(super) use digest_input::{DigestInputOwned, DigestInputRef};
mod begin;
pub(super) use begin::{BeginOwned, BeginRef};
mod challenge;
pub(super) use challenge::{ChallengeOwned, ChallengeRef};
mod mechanism_observation;
pub(super) use mechanism_observation::{MechanismObservationOwned, MechanismObservationRef};
mod token;
pub(super) use token::{TokenOwned, TokenRef};
mod finish;
pub(super) use finish::{FinishOwned, FinishRef};
mod finished;
pub(super) use finished::{FinishedOwned, FinishedRef};
mod error;
pub(super) use error::{ErrorOwned, ErrorRef};
mod envelope;
pub(super) use envelope::{EnvelopeOwned, EnvelopeRef};
pub(super) const MAX_BODY: usize = 100000;
pub(super) const MAX_DEPTH: usize = 8;
pub(super) const CONTRACT: [u8; 32] = [
    31, 219, 231, 97, 93, 23, 115, 212, 16, 5, 121, 225, 226, 84, 226, 236, 231, 190, 246, 204,
    105, 187, 102, 136, 217, 11, 252, 34, 242, 211, 91, 109,
];
