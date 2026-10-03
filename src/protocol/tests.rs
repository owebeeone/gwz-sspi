use super::{cbor, generated::*, profile};

fn finish<'a>() -> EnvelopeRef<'a> {
    EnvelopeRef {
        version: 1,
        kind: MessageKind::Finish,
        hello: None,
        begin: None,
        challenge: None,
        token: None,
        finish: Some(FinishRef {}),
        finished: None,
        error: None,
    }
}
#[test]
fn actual_rust_matches_reference_finish() {
    let frame = profile::encode(&finish(), &profile::Context::unbound()).unwrap();
    assert_eq!(
        frame.as_slice(),
        &[
            0xa9, 1, 1, 2, 5, 10, 0xf6, 11, 0xf6, 12, 0xf6, 13, 0xf6, 14, 0xa0, 15, 0xf6, 16, 0xf6
        ]
    );
    let decoded = profile::decode(&frame, &profile::Context::unbound()).unwrap();
    assert!(decoded.finish.is_some());
}
#[test]
fn strict_profile_rejects_noncanonical_and_trailing() {
    let valid = [
        0xa9, 1, 1, 2, 5, 10, 0xf6, 11, 0xf6, 12, 0xf6, 13, 0xf6, 14, 0xa0, 15, 0xf6, 16, 0xf6,
    ];
    for cut in 0..valid.len() {
        let input = crate::secret::Storage::copy(&valid[..cut]);
        assert!(
            profile::decode(&input, &profile::Context::unbound()).is_err(),
            "cut={cut}"
        );
    }
    let mut extra = valid.to_vec();
    extra.push(0);
    assert!(
        profile::decode(
            &crate::secret::Storage::copy(&extra),
            &profile::Context::unbound()
        )
        .is_err()
    );
    let input = [
        0xb8, 9, 1, 1, 2, 5, 10, 0xf6, 11, 0xf6, 12, 0xf6, 13, 0xf6, 14, 0xa0, 15, 0xf6, 16, 0xf6,
    ];
    assert!(
        profile::decode(
            &crate::secret::Storage::copy(&input),
            &profile::Context::unbound()
        )
        .is_err()
    );
    let _ = cbor::Reader::new(&valid);
}

pub(super) fn context(name: &str) -> profile::Context<'static> {
    let mut context = profile::Context::unbound();
    context.cap = Some(crate::TokenLimit::new(64).unwrap());
    context.provider_max = Some(64);
    context.expected_round = Some(2);
    context.package = Some(if name == "token_digest" {
        Package::Digest
    } else if name == "token_ntlm" {
        Package::Ntlm
    } else {
        Package::Negotiate
    });
    if name == "hello" {
        context.build = Some(b"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
        context.primary = Some((&[1, 0, 0, 0, 0, 0, 0, 0], &[0; 8], 0));
    }
    if name.starts_with("begin_") {
        context.package = None;
    }
    context
}
#[test]
fn all_independent_reference_vectors_match_rust_owned_and_borrowed_walks() {
    for (name, bytes) in super::test_vectors::VECTORS {
        let held = crate::secret::Storage::copy(bytes);
        let context = context(name);
        let borrowed =
            profile::decode(&held, &context).unwrap_or_else(|_| panic!("fixture={name}"));
        let encoded = profile::encode(&borrowed, &context).unwrap();
        assert_eq!(encoded.as_slice(), *bytes, "fixture={name}");
        let owned = profile::decode_owned(&held, &context).unwrap();
        drop(held);
        let encoded = profile::encode(&owned.borrow(), &context).unwrap();
        assert_eq!(encoded.as_slice(), *bytes, "owned fixture={name}");
    }
}
#[test]
fn every_reference_truncation_and_scalar_mutation_is_closed() {
    for (name, bytes) in super::test_vectors::VECTORS {
        let context = context(name);
        for cut in 0..bytes.len() {
            assert!(
                profile::decode(&crate::secret::Storage::copy(&bytes[..cut]), &context).is_err(),
                "fixture={name} cut={cut}"
            );
        }
        for (offset, replacement) in [
            (0, 0xa8),
            (0, 0xaa),
            (1, 0),
            (1, 2),
            (2, 0xf5),
            (2, 0x20),
            (2, 2),
            (3, 1),
            (4, 0),
            (4, 99),
            (5, 99),
        ] {
            let mut changed = bytes.to_vec();
            changed[offset] = replacement;
            assert!(
                profile::decode(&crate::secret::Storage::copy(&changed), &context).is_err(),
                "fixture={name} offset={offset} replacement={replacement}"
            );
        }
    }
}
#[test]
fn noncanonical_scalar_encodings_wrong_types_and_indefinite_maps_refuse() {
    let value = super::test_vectors::VECTORS
        .iter()
        .find(|(name, _)| *name == "finish")
        .unwrap()
        .1;
    for (offset, width, replacement) in [
        (0, 1, vec![0xbf]),
        (1, 1, vec![0x18, 1]),
        (2, 1, vec![0x18, 1]),
        (2, 1, vec![0x19, 0, 1]),
        (2, 1, vec![0xfb, 0x3f, 0xf0, 0, 0, 0, 0, 0, 0]),
        (2, 1, vec![0x60]),
        (2, 1, vec![0x40]),
        (4, 1, vec![0x18, 5]),
        (14, 1, vec![0xa1, 1, 0]),
    ] {
        let mut changed = value.to_vec();
        changed.splice(offset..offset + width, replacement);
        assert!(
            profile::decode(
                &crate::secret::Storage::copy(&changed),
                &profile::Context::unbound()
            )
            .is_err(),
            "offset={offset}"
        );
    }
}
#[test]
fn depth_unknown_duplicate_and_missing_nested_fields_refuse() {
    // Work from independent reference Begin; overwrite a known nested field key,
    // scalar type, map count, and add nested arrays. No generic production tree.
    let bytes = super::test_vectors::VECTORS
        .iter()
        .find(|(name, _)| *name == "begin_current")
        .unwrap()
        .1;
    let nested = bytes.iter().position(|b| *b == 0xa4).unwrap();
    for (offset, replacement) in [
        (nested, 0xa3),
        (nested, 0xa5),
        (nested + 1, 99),
        (nested + 3, 1),
        (nested + 2, 0x89),
    ] {
        let mut changed = bytes.to_vec();
        changed[offset] = replacement;
        assert!(
            profile::decode(
                &crate::secret::Storage::copy(&changed),
                &context("begin_current")
            )
            .is_err(),
            "offset={offset}"
        );
    }
}

#[test]
fn malformed_utf8_nul_boolean_and_huge_lengths_refuse() {
    let bytes = super::test_vectors::VECTORS
        .iter()
        .find(|(name, _)| *name == "begin_explicit")
        .unwrap()
        .1;
    let text = bytes
        .windows(9)
        .position(|window| window == b"synthetic")
        .unwrap();
    for replacement in [0, 0xff] {
        let mut changed = bytes.to_vec();
        changed[text] = replacement;
        assert!(
            profile::decode(
                &crate::secret::Storage::copy(&changed),
                &context("begin_explicit")
            )
            .is_err()
        );
    }
    let bytes = super::test_vectors::VECTORS
        .iter()
        .find(|(name, _)| *name == "token_provisional")
        .unwrap()
        .1;
    let boolean = bytes.iter().position(|b| *b == 0xf4).unwrap();
    for replacement in [0, 1, 0xf6, 0xf7] {
        let mut changed = bytes.to_vec();
        changed[boolean] = replacement;
        assert!(
            profile::decode(
                &crate::secret::Storage::copy(&changed),
                &context("token_provisional")
            )
            .is_err()
        );
    }
    let mut reader = cbor::Reader::new(&[0xa0]);
    assert!(reader.map(0, MAX_DEPTH + 1).is_err());
    for bytes in [
        &[0x5b, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff][..],
        &[0x5a, 0xff, 0xff, 0xff, 0xff][..],
        &[0x58, 0][..],
        &[0x59, 0, 0][..],
    ] {
        assert!(cbor::Reader::new(bytes).bytes().is_err());
    }
}
#[test]
fn nested_map_counts_keys_and_optional_null_are_closed_at_every_generated_level() {
    // Every map header in the independent fixtures represents one generated
    // record. Payload fixtures contain no map-head bytes; mutate each in place.
    for (name, bytes) in super::test_vectors::VECTORS {
        for offset in bytes.iter().enumerate().filter_map(|(offset, b)| {
            if (0xa0..=0xa9).contains(b) {
                Some(offset)
            } else {
                None
            }
        }) {
            for replacement in [0xbf, 0xa0, 0xa1, 0xaf] {
                if bytes[offset] == replacement {
                    continue;
                }
                let mut changed = bytes.to_vec();
                changed[offset] = replacement;
                assert!(
                    profile::decode(&crate::secret::Storage::copy(&changed), &context(name))
                        .is_err(),
                    "fixture={name} map={offset} replacement={replacement}"
                );
            }
            if bytes[offset] > 0xa0 {
                for replacement in [0, 99, 0x20] {
                    let mut changed = bytes.to_vec();
                    changed[offset + 1] = replacement;
                    assert!(
                        profile::decode(&crate::secret::Storage::copy(&changed), &context(name))
                            .is_err(),
                        "fixture={name} key={offset}"
                    );
                }
            }
        }
    }
}
#[test]
fn full_encode_size_is_counted_before_fixed_buffer_and_public_error_projection_is_checked() {
    let mut size = cbor::Size(MAX_BODY);
    assert!(cbor::Sink::raw(&mut size, b"x").is_err());
    for name in ["error", "error_provider"] {
        let bytes = super::test_vectors::VECTORS
            .iter()
            .find(|(candidate, _)| *candidate == name)
            .unwrap()
            .1;
        let error =
            super::adapters::worker_error(&crate::secret::Storage::copy(bytes), &context(name))
                .unwrap();
        if name == "error_provider" {
            assert_eq!(error.kind(), crate::ErrorKind::ProviderRejected);
            assert_eq!(error.native_status(), Some(u32::MAX));
        } else {
            assert_eq!(error.kind(), crate::ErrorKind::Protocol);
            assert_eq!(error.native_status(), None);
        }
    }
    let frame = profile::encode(&finish(), &profile::Context::unbound()).unwrap();
    assert!(super::adapters::worker_error(&frame, &profile::Context::unbound()).is_err());
}
