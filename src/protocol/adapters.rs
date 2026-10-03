//! Explicit caller/private projections, with admission before secret copies.
use super::{
    generated as wire,
    profile::{self, Context},
};
use crate::secret::Storage;
use crate::{
    AuthRequest, Error, ErrorKind, Identity, Mechanism, MechanismObservation, Package, TokenStatus,
    TokenStep,
};

fn package(value: Package) -> wire::Package {
    match value {
        Package::Negotiate => wire::Package::Negotiate,
        Package::Ntlm => wire::Package::Ntlm,
        Package::Digest => wire::Package::Digest,
    }
}
fn begin_ref(request: &AuthRequest) -> wire::BeginRef<'_> {
    let identity = match &request.identity {
        Identity::CurrentLogon => wire::IdentityRef {
            mode: wire::IdentityMode::CurrentLogon,
            user: None,
            domain: None,
            password: None,
        },
        Identity::Explicit {
            user,
            domain,
            password,
        } => wire::IdentityRef {
            mode: wire::IdentityMode::Explicit,
            user: Some(user.as_str()),
            domain: Some(domain.as_str()),
            password: Some(password.as_str()),
        },
    };
    wire::BeginRef {
        package: package(request.package),
        target: request.target.as_str(),
        identity,
        channel_binding: request.channel_binding.as_bytes(),
        token_limit: u64::from(request.token_limit.raw_bytes()),
        digest: request.digest.as_ref().map(|digest| wire::DigestInputRef {
            initial_challenge: digest.initial_challenge.as_bytes(),
            method: digest.method.as_str(),
            uri: digest.uri.as_str(),
        }),
    }
}
pub(super) fn encode_begin(request: &AuthRequest) -> Result<Storage, Error> {
    let envelope = wire::EnvelopeRef {
        version: 1,
        kind: wire::MessageKind::Begin,
        hello: None,
        begin: Some(begin_ref(request)),
        challenge: None,
        token: None,
        finish: None,
        finished: None,
        error: None,
    };
    profile::encode(&envelope, &Context::unbound())
        .map_err(|_| Error::new(ErrorKind::InvalidRequest))
}
pub(super) fn publish_token(frame: &Storage, context: &Context<'_>) -> Result<TokenStep, Error> {
    let envelope = profile::decode(frame, context)?;
    let token = envelope
        .token
        .ok_or_else(|| Error::new(ErrorKind::Protocol))?;
    let observation = match token.observation.mechanism {
        None => MechanismObservation::Unresolved,
        Some(value) => MechanismObservation::Selected {
            mechanism: match value {
                wire::Mechanism::Kerberos => Mechanism::Kerberos,
                wire::Mechanism::Ntlm => Mechanism::Ntlm,
                wire::Mechanism::Digest => Mechanism::Digest,
            },
            authoritative: token.observation.authoritative,
        },
    };
    Ok(TokenStep {
        status: match token.status {
            wire::TokenStatus::ContinueNeeded => TokenStatus::Continue,
            wire::TokenStatus::Complete => TokenStatus::Complete,
        },
        attributes: token.attributes as u32,
        observation,
        payload: crate::SecretBytes::new(token.payload),
    })
}
pub(super) fn worker_error(frame: &Storage, context: &Context<'_>) -> Result<Error, Error> {
    let envelope = profile::decode(frame, context)?;
    let value = envelope
        .error
        .ok_or_else(|| Error::new(ErrorKind::Protocol))?;
    Ok(match value.kind {
        wire::ErrorKind::InvalidRequest => Error::new(ErrorKind::InvalidRequest),
        wire::ErrorKind::Protocol => Error::new(ErrorKind::Protocol),
        wire::ErrorKind::IdentityMismatch => Error::new(ErrorKind::IdentityMismatch),
        wire::ErrorKind::ProviderRejected => {
            Error::provider(value.native_status.map(|status| status as u32))
        }
        wire::ErrorKind::Internal => Error::new(ErrorKind::Protocol),
    })
}

pub(super) enum PayloadSource {
    Caller,
    Provider,
    Wire,
}
/// Pure cap intersection; native package metadata must supply a nonzero maximum.
pub(super) fn admit_payload(
    length: usize,
    cap: crate::TokenLimit,
    provider_max: u32,
    source: PayloadSource,
    nonempty: bool,
) -> Result<(), Error> {
    let kind = match source {
        PayloadSource::Caller => ErrorKind::InvalidRequest,
        PayloadSource::Provider => ErrorKind::ProviderRejected,
        PayloadSource::Wire => ErrorKind::Protocol,
    };
    if provider_max == 0
        || (nonempty && length == 0)
        || length > cap.raw_bytes().min(provider_max) as usize
    {
        return Err(Error::new(kind));
    }
    Ok(())
}
