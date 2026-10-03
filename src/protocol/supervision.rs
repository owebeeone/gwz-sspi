//! Private phase bridge: typed borrowed admission precedes secret copies.
use super::{adapters, cbor::Reader, generated as wire, profile};
use crate::secret::Storage;
use crate::supervisor::{kernel::Expected, ports::Primary};
use crate::{AuthRequest, Error, ErrorKind, Package, SecretBytes, TokenLimit, TokenStep};

pub(crate) enum Reply {
    Hello,
    Token { round: u8, step: TokenStep },
    Finished,
    Failure(Error),
}
fn package(value: Package) -> wire::Package {
    match value {
        Package::Negotiate => wire::Package::Negotiate,
        Package::Ntlm => wire::Package::Ntlm,
        Package::Digest => wire::Package::Digest,
    }
}
pub(crate) fn validate_request(request: &AuthRequest) -> Result<(), Error> {
    adapters::validate_request(request)
}
pub(crate) fn begin(request: &AuthRequest) -> Result<Storage, Error> {
    adapters::encode_begin(request)
}
pub(crate) fn finish() -> Result<Storage, Error> {
    let value = wire::EnvelopeRef {
        version: 1,
        kind: wire::MessageKind::Finish,
        hello: None,
        begin: None,
        challenge: None,
        token: None,
        finish: Some(wire::FinishRef {}),
        finished: None,
        error: None,
    };
    profile::encode(&value, &profile::Context::unbound())
}
pub(crate) fn challenge(
    round: u8,
    payload: &SecretBytes,
    cap: TokenLimit,
) -> Result<Storage, Error> {
    adapters::admit_payload(
        payload.as_bytes().len(),
        cap,
        65536,
        adapters::PayloadSource::Caller,
        true,
    )?;
    let value = wire::EnvelopeRef {
        version: 1,
        kind: wire::MessageKind::Challenge,
        hello: None,
        begin: None,
        challenge: Some(wire::ChallengeRef {
            round: u64::from(round),
            payload: payload.as_bytes(),
        }),
        token: None,
        finish: None,
        finished: None,
        error: None,
    };
    let mut context = profile::Context::unbound();
    context.cap = Some(cap);
    context.provider_max = Some(65536);
    context.expected_round = Some(round);
    profile::encode(&value, &context).map_err(|_| Error::new(ErrorKind::InvalidRequest))
}
pub(crate) fn reply(
    frame: &Storage,
    expected: Expected,
    primary: &Primary,
    build: &[u8; 32],
    package_value: Package,
    cap: TokenLimit,
) -> Result<Reply, Error> {
    if !(1..=wire::MAX_BODY).contains(&frame.as_slice().len()) {
        return Err(Error::new(ErrorKind::Protocol));
    }
    // Parse structure first using generated walks, never a second field schema.
    let mut reader = Reader::new(frame.as_slice());
    let value = wire::EnvelopeRef::read(&mut reader, 1)?;
    reader.end()?;
    let mut context = profile::Context::unbound();
    context.build = Some(build);
    context.primary = Some((
        primary.sid.as_bytes(),
        primary.luid.as_bytes(),
        primary.session,
    ));
    context.package = Some(package(package_value));
    context.cap = Some(cap);
    context.provider_max = Some(65536);
    context.expected_round = match expected {
        Expected::Begin => Some(1),
        Expected::Challenge(round) => Some(round),
        _ => None,
    };
    if let Some(error) = &value.error {
        let phase = match expected {
            Expected::Bootstrap => wire::ErrorPhase::Bootstrap,
            Expected::Begin => wire::ErrorPhase::Begin,
            Expected::Challenge(_) => wire::ErrorPhase::Challenge,
            Expected::Finish => wire::ErrorPhase::Finish,
        };
        if error.phase != phase {
            return Err(Error::new(ErrorKind::Protocol));
        }
    } else {
        let matches = match expected {
            Expected::Bootstrap => value.kind == wire::MessageKind::Hello,
            Expected::Begin | Expected::Challenge(_) => value.kind == wire::MessageKind::Token,
            Expected::Finish => value.kind == wire::MessageKind::Finished,
        };
        if !matches {
            return Err(Error::new(ErrorKind::Protocol));
        }
    }
    // Distinguish matching-artifact/actual-primary failures without accepting
    // malformed cardinality or scalar bounds from the unadmitted projection.
    if expected == Expected::Bootstrap && value.kind == wire::MessageKind::Hello {
        let hello = value
            .hello
            .as_ref()
            .ok_or_else(|| Error::new(ErrorKind::Protocol))?;
        if value.version != 1
            || hello.protocol_version != 1
            || hello.schema_fingerprint != wire::CONTRACT
            || hello.build_fingerprint != build
        {
            return Err(Error::new(ErrorKind::WorkerMismatch));
        }
        if hello.primary_identity.sid != primary.sid.as_bytes()
            || hello.primary_identity.authentication_luid != primary.luid.as_bytes()
            || hello.primary_identity.session_id != u64::from(primary.session)
        {
            return Err(Error::new(ErrorKind::IdentityMismatch));
        }
    }
    profile::validate(&value, &context)?;
    match value.kind {
        wire::MessageKind::Hello => Ok(Reply::Hello),
        wire::MessageKind::Token => Ok(Reply::Token {
            round: value.token.as_ref().unwrap().round as u8,
            step: adapters::publish_token(frame, &context)?,
        }),
        wire::MessageKind::Finished => Ok(Reply::Finished),
        wire::MessageKind::Error => Ok(Reply::Failure(adapters::worker_error(frame, &context)?)),
        _ => Err(Error::new(ErrorKind::Protocol)),
    }
}
