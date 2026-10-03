use super::{adapters, generated::*, profile};
use crate::TokenLimit;
fn envelope<'a>(begin: BeginRef<'a>) -> EnvelopeRef<'a> {
    EnvelopeRef {
        version: 1,
        kind: MessageKind::Begin,
        hello: None,
        begin: Some(begin),
        challenge: None,
        token: None,
        finish: None,
        finished: None,
        error: None,
    }
}
fn begin<'a>(binding: &'a [u8]) -> BeginRef<'a> {
    BeginRef {
        package: Package::Digest,
        target: "fixture.invalid",
        identity: IdentityRef {
            mode: IdentityMode::Explicit,
            user: Some("synthetic"),
            domain: Some(""),
            password: Some(""),
        },
        channel_binding: binding,
        token_limit: 64,
        digest: Some(DigestInputRef {
            initial_challenge: b"synthetic",
            method: "GET",
            uri: "/fixture%20path",
        }),
    }
}
pub(super) fn binding() -> Vec<u8> {
    [b"tls-server-end-point:".as_slice(), &[0; 32]].concat()
}
pub(super) fn context() -> profile::Context<'static> {
    let mut context = profile::Context::unbound();
    context.cap = Some(TokenLimit::new(64).unwrap());
    context.provider_max = Some(64);
    context.package = Some(Package::Negotiate);
    context.expected_round = Some(2);
    context
}
pub(super) fn token<'a>(payload: &'a [u8]) -> EnvelopeRef<'a> {
    let mut value = envelope(begin(b""));
    value.begin = None;
    value.kind = MessageKind::Token;
    value.token = Some(TokenRef {
        round: 2,
        status: TokenStatus::Complete,
        attributes: 0,
        observation: MechanismObservationRef {
            kind: ObservationKind::Selected,
            mechanism: Some(Mechanism::Kerberos),
            authoritative: true,
        },
        payload,
    });
    value
}
#[test]
fn caller_provider_wire_cap_intersections_and_error_classes() {
    for cap in [1, 64, 65536] {
        let cap = TokenLimit::new(cap).unwrap();
        for provider in [1, 32, 65536, 100000] {
            let limit = cap.raw_bytes().min(provider) as usize;
            for (source, kind) in [
                (
                    adapters::PayloadSource::Caller,
                    crate::ErrorKind::InvalidRequest,
                ),
                (
                    adapters::PayloadSource::Provider,
                    crate::ErrorKind::ProviderRejected,
                ),
                (adapters::PayloadSource::Wire, crate::ErrorKind::Protocol),
            ] {
                assert!(adapters::admit_payload(limit - 1, cap, provider, source, false).is_ok());
                let source = match kind {
                    crate::ErrorKind::InvalidRequest => adapters::PayloadSource::Caller,
                    crate::ErrorKind::ProviderRejected => adapters::PayloadSource::Provider,
                    _ => adapters::PayloadSource::Wire,
                };
                assert!(adapters::admit_payload(limit, cap, provider, source, false).is_ok());
                let source = match kind {
                    crate::ErrorKind::InvalidRequest => adapters::PayloadSource::Caller,
                    crate::ErrorKind::ProviderRejected => adapters::PayloadSource::Provider,
                    _ => adapters::PayloadSource::Wire,
                };
                assert_eq!(
                    adapters::admit_payload(limit + 1, cap, provider, source, false)
                        .unwrap_err()
                        .kind(),
                    kind
                );
            }
        }
    }
    assert!(
        adapters::admit_payload(
            0,
            TokenLimit::new(1).unwrap(),
            1,
            adapters::PayloadSource::Caller,
            true
        )
        .is_err()
    );
    assert!(
        adapters::admit_payload(
            0,
            TokenLimit::new(1).unwrap(),
            0,
            adapters::PayloadSource::Caller,
            false
        )
        .is_err()
    );
}
#[test]
fn begin_caps_bindings_identity_target_and_digest_boundaries() {
    let binding = binding();
    for cap in [0, 1, 65535, 65536, 65537, u64::MAX] {
        let mut value = begin(&binding);
        value.token_limit = cap;
        value.digest.as_mut().unwrap().initial_challenge = b"x";
        assert_eq!(
            profile::begin(&value, None).is_ok(),
            (1..=65536).contains(&cap),
            "cap={cap}"
        );
    }
    for len in [0, 1, 1023, 1024, 1025] {
        let text = "x".repeat(len);
        let mut value = begin(&binding);
        value.target = &text;
        assert_eq!(
            profile::begin(&value, None).is_ok(),
            (1..=1024).contains(&len),
            "target={len}"
        );
    }
    for field in ["user", "domain", "password"] {
        for len in [0, 1, 8191, 8192, 8193] {
            let text = "x".repeat(len);
            let mut value = begin(&binding);
            match field {
                "user" => value.identity.user = Some(&text),
                "domain" => value.identity.domain = Some(&text),
                _ => value.identity.password = Some(&text),
            }
            assert_eq!(
                profile::begin(&value, None).is_ok(),
                len <= 8192 && (field != "user" || len > 0),
                "{field}={len}"
            );
        }
    }
    for field in ["user", "domain", "password"] {
        let mut value = begin(&binding);
        match field {
            "user" => value.identity.user = None,
            "domain" => value.identity.domain = None,
            _ => value.identity.password = None,
        }
        assert!(profile::begin(&value, None).is_err(), "missing={field}");
    }
    for len in [52, 53, 54, 68, 69, 70, 84, 85, 86] {
        let mut bytes = b"tls-server-end-point:".to_vec();
        bytes.resize(len, 0);
        assert_eq!(
            profile::begin(&begin(&bytes), None).is_ok(),
            [53, 69, 85].contains(&len),
            "binding={len}"
        );
    }
    let mut wrong = binding.clone();
    wrong[0] = b'T';
    assert!(profile::begin(&begin(&wrong), None).is_err());
    for len in [0, 1, 63, 64, 65] {
        let method = "A".repeat(len);
        let mut value = begin(&binding);
        value.digest.as_mut().unwrap().method = &method;
        assert_eq!(
            profile::begin(&value, None).is_ok(),
            (1..=64).contains(&len),
            "method={len}"
        );
    }
    for method in ["G ET", "G\0ET", "GÉT", "GET/", "GET:", "\n"] {
        let mut value = begin(&binding);
        value.digest.as_mut().unwrap().method = method;
        assert!(profile::begin(&value, None).is_err());
    }
    for len in [0, 1, 8191, 8192, 8193] {
        let uri = "x".repeat(len);
        let mut value = begin(&binding);
        value.digest.as_mut().unwrap().uri = &uri;
        assert_eq!(
            profile::begin(&value, None).is_ok(),
            (1..=8192).contains(&len),
            "uri={len}"
        );
    }
    for uri in ["/with space", "/\n", "/é", "/%", "/%a", "/%xz", "/\0"] {
        let mut value = begin(&binding);
        value.digest.as_mut().unwrap().uri = uri;
        assert!(profile::begin(&value, None).is_err());
    }
    for cap in [1, 64, 65536] {
        for len in [0, cap - 1, cap, cap + 1] {
            let challenge = vec![1; len];
            let mut value = begin(&binding);
            value.token_limit = cap as u64;
            value.digest.as_mut().unwrap().initial_challenge = &challenge;
            assert_eq!(
                profile::begin(&value, None).is_ok(),
                len > 0 && len <= cap,
                "digest cap={cap} len={len}"
            );
        }
    }
    let value = begin(&binding);
    assert!(profile::begin(&value, Some(0)).is_err());
    assert!(profile::begin(&value, Some(8)).is_err());
    assert!(profile::begin(&value, Some(9)).is_ok());
}
#[test]
fn package_identity_digest_relations_and_canonical_target() {
    let binding = binding();
    for package in [Package::Negotiate, Package::Ntlm, Package::Digest] {
        for explicit in [false, true] {
            for digest in [false, true] {
                let mut value = begin(&binding);
                value.package = package;
                if package != Package::Digest {
                    value.target = "HTTP/fixture.invalid";
                }
                if !explicit {
                    value.identity = IdentityRef {
                        mode: IdentityMode::CurrentLogon,
                        user: None,
                        domain: None,
                        password: None,
                    };
                }
                if !digest {
                    value.digest = None;
                }
                let expected = if package == Package::Digest {
                    explicit && digest
                } else {
                    !digest
                };
                assert_eq!(profile::begin(&value, None).is_ok(), expected);
            }
        }
    }
    for target in [
        "HTTP/fixture.invalid",
        "HTTP/127.0.0.1",
        "HTTP/fixture.invalid.",
        "HTTP/2001:db8::1",
    ] {
        let mut value = begin(&binding);
        value.package = Package::Ntlm;
        value.digest = None;
        value.target = target;
        assert!(profile::begin(&value, None).is_ok());
    }
    for target in [
        "",
        "http/fixture.invalid",
        "HTTP/x:443",
        "HTTP/x/path",
        "HTTP/x y",
        "HTTP/[2001:db8::1]",
        "HTTP/[fe80::1%zone]",
    ] {
        let mut value = begin(&binding);
        value.package = Package::Ntlm;
        value.digest = None;
        value.target = target;
        assert!(profile::begin(&value, None).is_err(), "target={target}");
    }
    let mut value = begin(&binding);
    value.package = Package::Ntlm;
    value.digest = None;
    value.target = "HTTP/x";
    value.identity.mode = IdentityMode::CurrentLogon;
    assert!(profile::begin(&value, None).is_err());
}
#[test]
fn token_and_challenge_round_cap_attribute_and_mechanism_matrix() {
    for package in [Package::Negotiate, Package::Ntlm, Package::Digest] {
        for mechanism in [
            None,
            Some(Mechanism::Kerberos),
            Some(Mechanism::Ntlm),
            Some(Mechanism::Digest),
        ] {
            for authoritative in [false, true] {
                for status in [TokenStatus::ContinueNeeded, TokenStatus::Complete] {
                    let mut value = token(b"x");
                    let token = value.token.as_mut().unwrap();
                    token.status = status;
                    token.observation = MechanismObservationRef {
                        kind: if mechanism.is_some() {
                            ObservationKind::Selected
                        } else {
                            ObservationKind::Unresolved
                        },
                        mechanism,
                        authoritative,
                    };
                    let mut context = context();
                    context.package = Some(package);
                    let allowed = match (package, mechanism) {
                        (Package::Negotiate, Some(Mechanism::Kerberos | Mechanism::Ntlm))
                        | (Package::Ntlm, Some(Mechanism::Ntlm))
                        | (Package::Digest, Some(Mechanism::Digest)) => true,
                        (_, None) => !authoritative && package == Package::Negotiate,
                        _ => false,
                    };
                    let expected = allowed
                        && (status != TokenStatus::Complete
                            || (mechanism.is_some() && authoritative));
                    assert_eq!(profile::validate(&value, &context).is_ok(), expected);
                }
            }
        }
    }
    for round in [0, 1, 2, 7, 8, 9, u64::MAX] {
        for status in [TokenStatus::ContinueNeeded, TokenStatus::Complete] {
            let mut value = token(b"");
            value.token.as_mut().unwrap().round = round;
            value.token.as_mut().unwrap().status = status;
            let mut context = context();
            context.expected_round = u8::try_from(round).ok();
            assert_eq!(
                profile::validate(&value, &context).is_ok(),
                (1..=8).contains(&round) && !(round == 8 && status == TokenStatus::ContinueNeeded)
            );
        }
    }
    for attributes in [
        0,
        23,
        24,
        255,
        256,
        65535,
        65536,
        u64::from(u32::MAX) - 1,
        u64::from(u32::MAX),
        u64::from(u32::MAX) + 1,
        u64::MAX,
    ] {
        let mut value = token(b"");
        value.token.as_mut().unwrap().attributes = attributes;
        assert_eq!(
            profile::encode(&value, &context()).is_ok(),
            attributes <= u64::from(u32::MAX)
        );
    }
    for len in [0, 1, 63, 64, 65] {
        let payload = vec![1; len];
        let value = token(&payload);
        assert_eq!(profile::validate(&value, &context()).is_ok(), len <= 64);
        let mut challenge = token(&payload);
        challenge.token = None;
        challenge.kind = MessageKind::Challenge;
        challenge.challenge = Some(ChallengeRef {
            round: 2,
            payload: &payload,
        });
        assert_eq!(
            profile::validate(&challenge, &context()).is_ok(),
            len > 0 && len <= 64
        );
        for round in [0, 1, 2, 7, 8, 9] {
            challenge.challenge.as_mut().unwrap().round = round;
            let mut context = context();
            context.expected_round = Some(round as u8);
            assert_eq!(
                profile::validate(&challenge, &context).is_ok(),
                (2..=8).contains(&round) && len > 0 && len <= 64
            );
        }
    }
    let value = token(b"x");
    let mut wrong = context();
    wrong.expected_round = Some(3);
    assert!(profile::validate(&value, &wrong).is_err());
    wrong = context();
    wrong.provider_max = Some(0);
    assert!(profile::validate(&value, &wrong).is_err());
    wrong = context();
    wrong.provider_max = Some(1);
    assert!(profile::validate(&token(b"xx"), &wrong).is_err());
}

#[test]
fn maximum_valid_digest_fields_fit_the_whole_frame_without_growth() {
    let mut binding = b"tls-server-end-point:".to_vec();
    binding.resize(85, 0);
    let text = "x".repeat(8192);
    let target = "x".repeat(1024);
    let method = "A".repeat(64);
    let challenge = vec![1; 65536];
    let value = envelope(BeginRef {
        package: Package::Digest,
        target: &target,
        identity: IdentityRef {
            mode: IdentityMode::Explicit,
            user: Some(&text),
            domain: Some(&text),
            password: Some(&text),
        },
        channel_binding: &binding,
        token_limit: 65536,
        digest: Some(DigestInputRef {
            initial_challenge: &challenge,
            method: &method,
            uri: &text,
        }),
    });
    let frame = profile::encode(&value, &profile::Context::unbound()).unwrap();
    assert!((99000..=MAX_BODY).contains(&frame.as_slice().len()));
    let decoded = profile::decode_owned(&frame, &profile::Context::unbound()).unwrap();
    assert_eq!(
        decoded
            .begin
            .unwrap()
            .digest
            .unwrap()
            .initial_challenge
            .as_bytes()
            .len(),
        65536
    );
}
