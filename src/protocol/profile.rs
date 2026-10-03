use super::{
    cbor::{Reader, Size, Writer},
    generated::*,
};
use crate::secret::Storage;
use crate::{Error as Failure, ErrorKind as FailureKind, TokenLimit};

fn bad() -> Failure {
    Failure::new(FailureKind::Protocol)
}
fn require(condition: bool) -> Result<(), Failure> {
    if !condition {
        return Err(bad());
    }
    Ok(())
}
/// Pure admission context supplied by the future parent/worker boundary.
/// No OS lookup, registration, native credential work or lifecycle success.
pub(super) struct Context<'a> {
    pub(super) build: Option<&'a [u8; 32]>,
    pub(super) primary: Option<(&'a [u8], &'a [u8], u32)>,
    pub(super) package: Option<Package>,
    pub(super) cap: Option<TokenLimit>,
    pub(super) provider_max: Option<u32>,
    pub(super) expected_round: Option<u8>,
}
impl Context<'_> {
    pub(super) fn unbound() -> Self {
        Self {
            build: None,
            primary: None,
            package: None,
            cap: None,
            provider_max: None,
            expected_round: None,
        }
    }
    fn limit(&self) -> Result<usize, Failure> {
        let cap = self.cap.ok_or_else(bad)?.raw_bytes();
        let provider = self.provider_max.ok_or_else(bad)?;
        require(provider > 0)?;
        Ok(cap.min(provider) as usize)
    }
}
fn text(value: &str, min: usize, max: usize) -> Result<(), Failure> {
    require((min..=max).contains(&value.len()) && !value.as_bytes().contains(&0))
}
fn identity(value: &IdentityRef<'_>) -> Result<(), Failure> {
    match value.mode {
        IdentityMode::CurrentLogon => {
            require(value.user.is_none() && value.domain.is_none() && value.password.is_none())
        }
        IdentityMode::Explicit => {
            text(value.user.ok_or_else(bad)?, 1, 8192)?;
            text(value.domain.ok_or_else(bad)?, 0, 8192)?;
            text(value.password.ok_or_else(bad)?, 0, 8192)
        }
    }
}
fn canonical_target(value: &str) -> bool {
    let Some(host) = value.strip_prefix("HTTP/") else {
        return false;
    };
    if host.is_empty() {
        return false;
    }
    // Core canonicalizes the verified origin host. IPv6 brackets are removed
    // before forming the native target; the literal must not carry a port/zone.
    if host.contains(':') {
        return host.parse::<std::net::Ipv6Addr>().is_ok();
    }
    // Do not duplicate core DNS/IDNA policy (including trailing-dot handling).
    host.bytes()
        .all(|b| !b.is_ascii_control() && !b.is_ascii_whitespace() && !b"/\\[]@?#".contains(&b))
}
fn method(value: &str) -> bool {
    value
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
}
fn uri(value: &str) -> bool {
    let bytes = value.as_bytes();
    let mut n = 0;
    while n < bytes.len() {
        let b = bytes[n];
        if !(0x21..=0x7e).contains(&b) {
            return false;
        }
        if b == b'%' {
            if n + 2 >= bytes.len()
                || !bytes[n + 1].is_ascii_hexdigit()
                || !bytes[n + 2].is_ascii_hexdigit()
            {
                return false;
            }
            n += 2;
        }
        n += 1;
    }
    true
}
pub(super) fn begin(value: &BeginRef<'_>, provider_max: Option<u32>) -> Result<(), Failure> {
    require((1..=65536).contains(&value.token_limit))?;
    text(value.target, 1, 1024)?;
    identity(&value.identity)?;
    require(
        [53, 69, 85].contains(&value.channel_binding.len())
            && value.channel_binding.starts_with(b"tls-server-end-point:"),
    )?;
    let limit = value
        .token_limit
        .min(u64::from(provider_max.unwrap_or(u32::MAX)));
    if let Some(max) = provider_max {
        require(max > 0)?;
    }
    match value.package {
        Package::Digest => {
            require(value.identity.mode == IdentityMode::Explicit)?;
            let digest = value.digest.as_ref().ok_or_else(bad)?;
            require(
                !digest.initial_challenge.is_empty()
                    && digest.initial_challenge.len() as u64 <= limit,
            )?;
            text(digest.method, 1, 64)?;
            require(method(digest.method))?;
            text(digest.uri, 1, 8192)?;
            require(uri(digest.uri))
        }
        Package::Negotiate | Package::Ntlm => {
            require(value.digest.is_none() && canonical_target(value.target))
        }
    }
}
pub(super) fn validate(value: &EnvelopeRef<'_>, context: &Context<'_>) -> Result<(), Failure> {
    require(value.version == 1)?;
    let bodies = [
        value.hello.is_some(),
        value.begin.is_some(),
        value.challenge.is_some(),
        value.token.is_some(),
        value.finish.is_some(),
        value.finished.is_some(),
        value.error.is_some(),
    ];
    let matching = match value.kind {
        MessageKind::Hello => value.hello.is_some(),
        MessageKind::Begin => value.begin.is_some(),
        MessageKind::Challenge => value.challenge.is_some(),
        MessageKind::Token => value.token.is_some(),
        MessageKind::Finish => value.finish.is_some(),
        MessageKind::Finished => value.finished.is_some(),
        MessageKind::Error => value.error.is_some(),
    };
    require(bodies.into_iter().filter(|active| *active).count() == 1 && matching)?;
    if let Some(hello) = &value.hello {
        require(hello.protocol_version == 1 && hello.schema_fingerprint == CONTRACT)?;
        require(hello.build_fingerprint == context.build.ok_or_else(bad)?)?;
        let primary = &hello.primary_identity;
        let sid = primary.sid;
        require(
            sid.len() >= 8
                && sid[0] == 1
                && sid[1] <= 15
                && sid.len() == 8 + 4 * usize::from(sid[1]),
        )?;
        require(
            primary.authentication_luid.len() == 8 && primary.session_id <= u64::from(u32::MAX),
        )?;
        let (sid, luid, session) = context.primary.ok_or_else(bad)?;
        require(
            primary.sid == sid
                && primary.authentication_luid == luid
                && primary.session_id == u64::from(session),
        )?;
    }
    if let Some(value) = &value.begin {
        begin(value, context.provider_max)?;
        if let Some(cap) = context.cap {
            require(value.token_limit == u64::from(cap.raw_bytes()))?;
        }
        if let Some(package) = context.package {
            require(value.package == package)?;
        }
    }
    if let Some(value) = &value.challenge {
        require(
            (2..=8).contains(&value.round)
                && Some(value.round) == context.expected_round.map(u64::from),
        )?;
        require(!value.payload.is_empty() && value.payload.len() <= context.limit()?)?;
    }
    if let Some(token) = &value.token {
        require(
            (1..=8).contains(&token.round)
                && Some(token.round) == context.expected_round.map(u64::from),
        )?;
        require(
            token.attributes <= u64::from(u32::MAX) && token.payload.len() <= context.limit()?,
        )?;
        require(token.status != TokenStatus::ContinueNeeded || token.round < 8)?;
        let observation = &token.observation;
        match observation.kind {
            ObservationKind::Unresolved => require(
                observation.mechanism.is_none()
                    && !observation.authoritative
                    && context.package == Some(Package::Negotiate),
            )?,
            ObservationKind::Selected => {
                let selected = observation.mechanism.ok_or_else(bad)?;
                let allowed = match context.package.ok_or_else(bad)? {
                    Package::Negotiate => {
                        selected == Mechanism::Kerberos || selected == Mechanism::Ntlm
                    }
                    Package::Ntlm => selected == Mechanism::Ntlm,
                    Package::Digest => selected == Mechanism::Digest,
                };
                require(allowed)?;
            }
        }
        if token.status == TokenStatus::Complete {
            require(observation.kind == ObservationKind::Selected && observation.authoritative)?;
        }
    }
    if let Some(error) = &value.error {
        require(
            error
                .native_status
                .is_none_or(|status| status <= u64::from(u32::MAX)),
        )?;
        if error.kind != ErrorKind::ProviderRejected {
            require(error.native_status.is_none())?;
        }
    }
    Ok(())
}
pub(super) fn encode(value: &EnvelopeRef<'_>, context: &Context<'_>) -> Result<Storage, Failure> {
    validate(value, context)?;
    let mut size = Size(0);
    value.emit(&mut size)?;
    let mut storage = Storage::zeroed(size.0);
    let mut writer = Writer::new(storage.as_mut());
    value.emit(&mut writer)?;
    writer.end()?;
    Ok(storage)
}
pub(super) fn decode<'a>(
    frame: &'a Storage,
    context: &Context<'_>,
) -> Result<EnvelopeRef<'a>, Failure> {
    require((1..=MAX_BODY).contains(&frame.as_slice().len()))?;
    let mut reader = Reader::new(frame.as_slice());
    let value = EnvelopeRef::read(&mut reader, 1)?;
    reader.end()?;
    validate(&value, context)?;
    Ok(value)
}
pub(super) fn decode_owned(
    frame: &Storage,
    context: &Context<'_>,
) -> Result<EnvelopeOwned, Failure> {
    Ok(decode(frame, context)?.own())
}
