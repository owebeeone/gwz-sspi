//! Worker projections use only generated records and the closed profile.
use super::{adapters, cbor::Reader, generated as wire, profile};
use crate::secret::Storage;
use crate::supervisor::ports::Primary;
use crate::{
    AuthRequest, DigestInput, Error, ErrorKind, Identity, Mechanism, MechanismObservation, Package,
    SecretBytes, SecretText, TokenLimit, TokenStatus, TokenStep,
};
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Phase {
    Begin,
    Challenge(u8),
    Finish,
}
pub(crate) enum Command {
    Begin(AuthRequest, u32),
    Challenge(SecretBytes),
    Finish,
}
fn package(p: Package) -> wire::Package {
    match p {
        Package::Negotiate => wire::Package::Negotiate,
        Package::Ntlm => wire::Package::Ntlm,
        Package::Digest => wire::Package::Digest,
    }
}
fn caller(p: wire::Package) -> Package {
    match p {
        wire::Package::Negotiate => Package::Negotiate,
        wire::Package::Ntlm => Package::Ntlm,
        wire::Package::Digest => Package::Digest,
    }
}
fn bad() -> Error {
    Error::new(ErrorKind::Protocol)
}
fn envelope(kind: wire::MessageKind) -> wire::EnvelopeRef<'static> {
    wire::EnvelopeRef {
        version: 1,
        kind,
        hello: None,
        begin: None,
        challenge: None,
        token: None,
        finish: None,
        finished: None,
        error: None,
    }
}
pub(crate) fn command(
    frame: &Storage,
    phase: Phase,
    request: Option<(&AuthRequest, u32)>,
    mut maximum: impl FnMut(Package) -> Result<u32, Error>,
) -> Result<Command, Error> {
    let mut r = Reader::new(frame.as_slice());
    let value = wire::EnvelopeRef::read(&mut r, 1)?;
    r.end()?;
    let mut context = profile::Context::unbound();
    if let Some((request, max)) = request {
        context.package = Some(package(request.package));
        context.cap = Some(request.token_limit);
        context.provider_max = Some(max);
    }
    if let Phase::Challenge(round) = phase {
        context.expected_round = Some(round);
    }
    // Kind/phase checked before metadata or constructing any owned secrets.
    if value.kind == wire::MessageKind::Finish {
        profile::validate(&value, &context)?;
        return Ok(Command::Finish);
    }
    match (phase, value.kind) {
        (Phase::Begin, wire::MessageKind::Begin) => {
            profile::validate(&value, &context)?;
            let begin = value.begin.as_ref().ok_or_else(bad)?;
            let max = maximum(caller(begin.package))?;
            if max == 0 {
                return Err(Error::provider(None));
            }
            profile::begin(begin, Some(max)).map_err(|_| Error::new(ErrorKind::InvalidRequest))?;
            let identity = match begin.identity.mode {
                wire::IdentityMode::CurrentLogon => Identity::CurrentLogon,
                wire::IdentityMode::Explicit => Identity::Explicit {
                    user: SecretText::admitted(begin.identity.user.unwrap()),
                    domain: SecretText::admitted(begin.identity.domain.unwrap()),
                    password: SecretText::admitted(begin.identity.password.unwrap()),
                },
            };
            Ok(Command::Begin(
                AuthRequest {
                    package: caller(begin.package),
                    target: SecretText::admitted(begin.target),
                    identity,
                    channel_binding: SecretBytes::new(begin.channel_binding),
                    token_limit: TokenLimit::new(begin.token_limit as u32)?,
                    digest: begin.digest.as_ref().map(|d| DigestInput {
                        initial_challenge: SecretBytes::new(d.initial_challenge),
                        method: SecretText::admitted(d.method),
                        uri: SecretText::admitted(d.uri),
                    }),
                },
                max,
            ))
        }
        (Phase::Challenge(_), wire::MessageKind::Challenge) => {
            profile::validate(&value, &context)?;
            Ok(Command::Challenge(SecretBytes::new(
                value.challenge.as_ref().unwrap().payload,
            )))
        }
        _ => Err(bad()),
    }
}
pub(crate) fn hello(primary: &Primary, build: &[u8; 32]) -> Result<Storage, Error> {
    let mut value = envelope(wire::MessageKind::Hello);
    value.hello = Some(wire::HelloRef {
        protocol_version: 1,
        schema_fingerprint: &wire::CONTRACT,
        build_fingerprint: build,
        primary_identity: wire::PrimaryIdentityRef {
            sid: primary.sid.as_bytes(),
            authentication_luid: primary.luid.as_bytes(),
            session_id: u64::from(primary.session),
        },
    });
    let mut context = profile::Context::unbound();
    context.build = Some(build);
    context.primary = Some((
        primary.sid.as_bytes(),
        primary.luid.as_bytes(),
        primary.session,
    ));
    profile::encode(&value, &context)
}
pub(crate) fn finished() -> Result<Storage, Error> {
    let mut value = envelope(wire::MessageKind::Finished);
    value.finished = Some(wire::FinishedRef {});
    profile::encode(&value, &profile::Context::unbound())
}
pub(crate) fn error(phase: Phase, error: &Error) -> Result<Storage, Error> {
    let mut value = envelope(wire::MessageKind::Error);
    value.error = Some(wire::ErrorRef {
        kind: match error.kind() {
            ErrorKind::InvalidRequest => wire::ErrorKind::InvalidRequest,
            ErrorKind::IdentityMismatch => wire::ErrorKind::IdentityMismatch,
            ErrorKind::ProviderRejected => wire::ErrorKind::ProviderRejected,
            _ => wire::ErrorKind::Protocol,
        },
        phase: match phase {
            Phase::Begin => wire::ErrorPhase::Begin,
            Phase::Challenge(_) => wire::ErrorPhase::Challenge,
            Phase::Finish => wire::ErrorPhase::Finish,
        },
        native_status: error.native_status().map(u64::from),
    });
    profile::encode(&value, &profile::Context::unbound())
}
pub(crate) fn token(
    round: u8,
    step: &TokenStep,
    request: &AuthRequest,
    max: u32,
) -> Result<Storage, Error> {
    adapters::admit_payload(
        step.payload.as_bytes().len(),
        request.token_limit,
        max,
        adapters::PayloadSource::Provider,
        false,
    )?;
    let (kind, mechanism, authoritative) = match step.observation {
        MechanismObservation::Unresolved => (wire::ObservationKind::Unresolved, None, false),
        MechanismObservation::Selected {
            mechanism,
            authoritative,
        } => (
            wire::ObservationKind::Selected,
            Some(match mechanism {
                Mechanism::Kerberos => wire::Mechanism::Kerberos,
                Mechanism::Ntlm => wire::Mechanism::Ntlm,
                Mechanism::Digest => wire::Mechanism::Digest,
            }),
            authoritative,
        ),
    };
    let mut value = envelope(wire::MessageKind::Token);
    value.token = Some(wire::TokenRef {
        round: u64::from(round),
        status: match step.status {
            TokenStatus::Continue => wire::TokenStatus::ContinueNeeded,
            TokenStatus::Complete => wire::TokenStatus::Complete,
        },
        attributes: u64::from(step.attributes),
        observation: wire::MechanismObservationRef {
            kind,
            mechanism,
            authoritative,
        },
        payload: step.payload.as_bytes(),
    });
    let mut context = profile::Context::unbound();
    context.package = Some(package(request.package));
    context.cap = Some(request.token_limit);
    context.provider_max = Some(max);
    context.expected_round = Some(round);
    profile::encode(&value, &context).map_err(|_| Error::provider(None))
}

#[cfg(test)]
pub(crate) mod inspection {
    use super::*;
    pub(crate) fn is_finish_reply(frame: &Storage) -> bool {
        let mut r = Reader::new(frame.as_slice());
        let v = wire::EnvelopeRef::read(&mut r, 1).unwrap();
        v.kind == wire::MessageKind::Finished
            || v.error
                .as_ref()
                .is_some_and(|e| e.phase == wire::ErrorPhase::Finish)
    }
}
