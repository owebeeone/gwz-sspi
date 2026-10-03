use super::bounds_tests::{binding, context, token};
use super::{adapters, generated::*, profile};
use crate::{TokenLimit, secret::Storage};

#[test]
fn envelope_body_cardinality_and_error_status_bits() {
    let mut value = token(b"x");
    value.finish = Some(FinishRef {});
    assert!(profile::validate(&value, &context()).is_err());
    value.finish = None;
    value.token = None;
    assert!(profile::validate(&value, &context()).is_err());
    value = token(b"x");
    value.kind = MessageKind::Finish;
    assert!(profile::validate(&value, &context()).is_err());
    for kind in [
        ErrorKind::InvalidRequest,
        ErrorKind::Protocol,
        ErrorKind::IdentityMismatch,
        ErrorKind::ProviderRejected,
        ErrorKind::Internal,
    ] {
        for native_status in [
            None,
            Some(0),
            Some(u64::from(u32::MAX)),
            Some(u64::from(u32::MAX) + 1),
        ] {
            let mut value = token(b"x");
            value.token = None;
            value.kind = MessageKind::Error;
            value.error = Some(ErrorRef {
                kind,
                phase: ErrorPhase::Challenge,
                native_status,
            });
            let expected = native_status.is_none()
                || (kind == ErrorKind::ProviderRejected
                    && native_status.unwrap() <= u64::from(u32::MAX));
            assert_eq!(profile::validate(&value, &context()).is_ok(), expected);
        }
    }
}
#[test]
fn hello_identity_fingerprints_sid_luid_and_numeric_bounds() {
    let build = [2; 32];
    let luid = [3; 8];
    for count in [0u8, 1, 14, 15, 16] {
        let mut sid = vec![0; 8 + 4 * usize::from(count)];
        sid[0] = 1;
        sid[1] = count;
        let mut value = token(b"x");
        value.token = None;
        value.kind = MessageKind::Hello;
        value.hello = Some(HelloRef {
            protocol_version: 1,
            schema_fingerprint: &CONTRACT,
            build_fingerprint: &build,
            primary_identity: PrimaryIdentityRef {
                sid: &sid,
                authentication_luid: &luid,
                session_id: 0,
            },
        });
        let mut context = profile::Context::unbound();
        context.build = Some(&build);
        context.primary = Some((&sid, &luid, 0));
        assert_eq!(
            profile::validate(&value, &context).is_ok(),
            count <= 15,
            "sid count={count}"
        );
        for version in [0, 2] {
            value.hello.as_mut().unwrap().protocol_version = version;
            assert!(profile::validate(&value, &context).is_err());
        }
        value.hello.as_mut().unwrap().protocol_version = 1;
        value.hello.as_mut().unwrap().schema_fingerprint = &build;
        assert!(profile::validate(&value, &context).is_err());
    }
    let sid = [1, 0, 0, 0, 0, 0, 0, 0];
    for sid_slice in [
        &sid[..7],
        &sid[..8],
        &[0, 0, 0, 0, 0, 0, 0, 0][..],
        &[1, 0, 0, 0, 0, 0, 0, 0, 0][..],
    ] {
        for luid_len in [7, 8, 9] {
            let luid = vec![0; luid_len];
            let mut value = token(b"x");
            value.token = None;
            value.kind = MessageKind::Hello;
            value.hello = Some(HelloRef {
                protocol_version: 1,
                schema_fingerprint: &CONTRACT,
                build_fingerprint: &build,
                primary_identity: PrimaryIdentityRef {
                    sid: sid_slice,
                    authentication_luid: &luid,
                    session_id: 0,
                },
            });
            let mut context = profile::Context::unbound();
            context.build = Some(&build);
            context.primary = Some((sid_slice, &luid, 0));
            assert_eq!(
                profile::validate(&value, &context).is_ok(),
                sid_slice == sid && luid_len == 8
            );
            value.hello.as_mut().unwrap().primary_identity.session_id = u64::from(u32::MAX) + 1;
            assert!(profile::validate(&value, &context).is_err());
        }
    }
}
#[test]
fn exact_caller_caps_cross_begin_adapter_without_defaults() {
    for cap in [1, 64, 65536] {
        let request = crate::AuthRequest {
            package: crate::Package::Ntlm,
            target: crate::SecretText::new("HTTP/fixture.invalid").unwrap(),
            identity: crate::Identity::CurrentLogon,
            channel_binding: crate::SecretBytes::new(&binding()),
            token_limit: TokenLimit::new(cap).unwrap(),
            digest: None,
        };
        let frame = adapters::encode_begin(&request).unwrap();
        let borrowed = profile::decode(&frame, &profile::Context::unbound()).unwrap();
        assert_eq!(borrowed.begin.unwrap().token_limit, u64::from(cap));
    }
    let context = context();
    let frame = profile::encode(&token(b"synthetic"), &context).unwrap();
    let published = adapters::publish_token(&frame, &context).unwrap();
    assert_eq!(published.payload.as_bytes(), b"synthetic");
    let malformed = Storage::copy(b"x");
    assert!(adapters::publish_token(&malformed, &context).is_err());
}

#[test]
fn unresolved_direct_or_missing_package_context_refuses() {
    let mut value = token(b"x");
    let token = value.token.as_mut().unwrap();
    token.status = TokenStatus::ContinueNeeded;
    token.observation = MechanismObservationRef {
        kind: ObservationKind::Unresolved,
        mechanism: None,
        authoritative: false,
    };
    for package in [
        None,
        Some(Package::Negotiate),
        Some(Package::Ntlm),
        Some(Package::Digest),
    ] {
        let mut context = context();
        context.package = package;
        assert_eq!(
            profile::validate(&value, &context).is_ok(),
            package == Some(Package::Negotiate)
        );
    }
}

#[test]
fn refused_caller_request_retains_owned_inputs_until_drop_and_wipes_them() {
    use crate::secret::audit::{Events, Probe};
    use std::sync::{Arc, Mutex};
    let events: Events = Arc::new(Mutex::new(Vec::new()));
    let probe = Probe(Some(events.clone()));
    let mut target = crate::SecretText::new("HTTP/fixture.invalid").unwrap();
    target.0.probe = probe.clone();
    let mut binding = crate::SecretBytes::new(b"invalid-binding");
    binding.0.probe = probe.clone();
    let mut user = crate::SecretText::new("synthetic").unwrap();
    user.0.probe = probe.clone();
    let mut domain = crate::SecretText::new("").unwrap();
    domain.0.probe = probe.clone();
    let mut password = crate::SecretText::new("synthetic-only").unwrap();
    password.0.probe = probe;
    let request = crate::AuthRequest {
        package: crate::Package::Ntlm,
        target,
        identity: crate::Identity::Explicit {
            user,
            domain,
            password,
        },
        channel_binding: binding,
        token_limit: TokenLimit::new(64).unwrap(),
        digest: None,
    };
    let error = adapters::encode_begin(&request).err().unwrap();
    assert_eq!(error.kind(), crate::ErrorKind::InvalidRequest);
    assert!(events.lock().unwrap().is_empty());
    drop(request);
    let events = events.lock().unwrap();
    assert_eq!(events.len(), 5);
    assert!(events.iter().all(|(_, zero)| *zero));
}

#[test]
fn hello_exact_max_sid_session_and_fingerprint_length_edges() {
    let build = [2; 32];
    let luid = [0; 8];
    let mut sid = vec![0; 68];
    sid[0] = 1;
    sid[1] = 15;
    for sid_len in [67, 68, 69] {
        let mut sid = sid.clone();
        sid.resize(sid_len, 0);
        for session in [
            u64::from(u32::MAX) - 1,
            u64::from(u32::MAX),
            u64::from(u32::MAX) + 1,
        ] {
            let mut value = token(b"x");
            value.token = None;
            value.kind = MessageKind::Hello;
            value.hello = Some(HelloRef {
                protocol_version: 1,
                schema_fingerprint: &CONTRACT,
                build_fingerprint: &build,
                primary_identity: PrimaryIdentityRef {
                    sid: &sid,
                    authentication_luid: &luid,
                    session_id: session,
                },
            });
            let mut context = profile::Context::unbound();
            context.build = Some(&build);
            context.primary = Some((&sid, &luid, session as u32));
            assert_eq!(
                profile::validate(&value, &context).is_ok(),
                sid_len == 68 && session <= u64::from(u32::MAX)
            );
        }
    }
    for len in [31, 32, 33] {
        let mut fingerprint = CONTRACT.to_vec();
        fingerprint.resize(len, 0);
        let mut artifact = build.to_vec();
        artifact.resize(len, 0);
        let mut value = token(b"x");
        value.token = None;
        value.kind = MessageKind::Hello;
        value.hello = Some(HelloRef {
            protocol_version: 1,
            schema_fingerprint: &fingerprint,
            build_fingerprint: &artifact,
            primary_identity: PrimaryIdentityRef {
                sid: &sid,
                authentication_luid: &luid,
                session_id: 0,
            },
        });
        let mut context = profile::Context::unbound();
        context.build = Some(&build);
        context.primary = Some((&sid, &luid, 0));
        assert_eq!(profile::validate(&value, &context).is_ok(), len == 32);
    }
}
#[test]
fn token_and_challenge_actual_codec_cap_extremes() {
    for cap in [1usize, 64, 65536] {
        for len in [0, cap - 1, cap, cap + 1] {
            let payload = vec![1; len];
            let mut context = context();
            context.cap = Some(TokenLimit::new(cap as u32).unwrap());
            context.provider_max = Some(100000);
            let value = token(&payload);
            assert_eq!(
                profile::encode(&value, &context).is_ok(),
                len <= cap,
                "token cap={cap} len={len}"
            );
            let mut value = token(&payload);
            value.token = None;
            value.kind = MessageKind::Challenge;
            value.challenge = Some(ChallengeRef {
                round: 2,
                payload: &payload,
            });
            assert_eq!(
                profile::encode(&value, &context).is_ok(),
                len > 0 && len <= cap,
                "challenge cap={cap} len={len}"
            );
        }
    }
}
