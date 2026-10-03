use super::{
    framing::{ReadFrame, WriteFrame},
    generated::*,
    profile,
};
use crate::secret::{
    Storage,
    audit::{Events, Probe},
};
use std::sync::{Arc, Mutex};
fn probe() -> (Probe, Events) {
    let events = Arc::new(Mutex::new(Vec::new()));
    (Probe(Some(events.clone())), events)
}
fn observed(events: &Events, count: usize) {
    let events = events.lock().unwrap();
    assert_eq!(events.len(), count);
    assert!(
        events.iter().all(|(_, zero)| *zero),
        "wipe observations={events:?}"
    );
}
fn body(bytes: &[u8], probe: Probe) -> Storage {
    let mut storage = Storage::copy(bytes);
    storage.probe = probe;
    storage
}
fn input(bytes: &[u8]) -> Vec<u8> {
    [(bytes.len() as u32).to_le_bytes().as_slice(), bytes].concat()
}
#[test]
fn reader_every_partial_eof_abort_and_drop_wipes_before_release() {
    let bytes = b"synthetic-only-frame";
    let input = input(bytes);
    for cut in 0..=input.len() {
        for operation in ["eof", "abort", "drop", "take"] {
            let (probe, events) = probe();
            let mut read = ReadFrame::new();
            read.probe = probe;
            assert_eq!(read.push(&input[..cut]).unwrap(), cut);
            match operation {
                "eof" => {
                    let error = read.eof();
                    assert_eq!(error.kind(), crate::ErrorKind::Protocol);
                    assert!(read.push(b"x").is_err());
                }
                "abort" => {
                    read.abort();
                    assert!(read.take().is_err());
                }
                "take" => {
                    if cut == input.len() {
                        let frame = read.take().unwrap();
                        assert_eq!(frame.as_slice(), bytes);
                        drop(frame);
                    } else {
                        assert!(read.take().is_err());
                    }
                }
                _ => {}
            }
            drop(read);
            observed(&events, usize::from(cut >= 4));
            if cut >= 4 {
                assert_eq!(events.lock().unwrap()[0].0, bytes.len());
            }
        }
    }
}
#[test]
fn writer_every_partial_abort_drop_finish_failure_and_bad_accounting_wipes() {
    let bytes = b"synthetic-only-frame";
    for cut in 0..=bytes.len() + 4 {
        for operation in ["abort", "drop", "finish", "zero", "overrun"] {
            let (probe, events) = probe();
            let mut write = WriteFrame::new(body(bytes, probe)).unwrap();
            let mut written = 0;
            while written < cut {
                let count = (cut - written).min(write.remaining().unwrap().len());
                write.advance(count).unwrap();
                written += count;
            }
            match operation {
                "abort" => {
                    write.abort();
                    assert!(write.remaining().is_err());
                }
                "finish" => {
                    assert_eq!(write.finish().is_ok(), cut == bytes.len() + 4);
                    observed(&events, 1);
                    continue;
                }
                "zero" => {
                    assert!(write.advance(0).is_err());
                    assert!(write.remaining().is_err());
                }
                "overrun" => {
                    let count = write.remaining().unwrap().len() + 1;
                    assert!(write.advance(count).is_err());
                }
                _ => {}
            }
            drop(write);
            observed(&events, 1);
            assert_eq!(events.lock().unwrap()[0].0, bytes.len());
        }
    }
}
#[test]
fn length_header_bounds_before_body_allocation_and_one_frame_ownership() {
    for len in [0u32, 1, 99999, 100000, 100001, u32::MAX] {
        let (probe, events) = probe();
        let mut read = ReadFrame::new();
        read.probe = probe;
        assert_eq!(
            read.push(&len.to_le_bytes()).is_ok(),
            (1..=100000).contains(&len)
        );
        drop(read);
        observed(&events, usize::from((1..=100000).contains(&len)));
    }
    for len in [0usize, 1, 99999, 100000, 100001] {
        let (probe, events) = probe();
        let bytes = vec![1; len];
        let write = WriteFrame::new(body(&bytes, probe));
        assert_eq!(write.is_ok(), (1..=100000).contains(&len));
        drop(write);
        observed(&events, 1);
    }
    let mut read = ReadFrame::new();
    let bytes = input(b"x");
    let mut plus = bytes.clone();
    plus.extend_from_slice(&bytes);
    assert_eq!(read.push(&plus).unwrap(), bytes.len());
    assert!(read.complete());
    let frame = read.take().unwrap();
    assert_eq!(frame.as_slice(), b"x");
    assert!(read.push(&bytes).is_err());
}
#[test]
fn seeded_random_chunks_reassemble_reads_and_writes_without_secret_growth() {
    let seed = 0x78c0_1063_5e3a_2941u64;
    let mut state = seed;
    for case in 0..128 {
        for (name, bytes) in super::test_vectors::VECTORS {
            let mut trace = Vec::new();
            let (probe, events) = probe();
            let mut write = WriteFrame::new(body(bytes, probe)).unwrap();
            let mut output = Vec::new();
            while !write.complete() {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                let remaining = write.remaining().unwrap();
                let count = (state as usize % 17 + 1).min(remaining.len());
                trace.push(count);
                output.extend_from_slice(&remaining[..count]);
                write.advance(count).unwrap();
            }
            assert_eq!(
                output,
                input(bytes),
                "seed={seed:x} case={case} fixture={name} write_trace={trace:?}"
            );
            write.finish().unwrap();
            observed(&events, 1);
            let (probe, events) = self::probe();
            let mut read = ReadFrame::new();
            read.probe = probe;
            let mut offset = 0;
            trace.clear();
            while offset < output.len() {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                let count = (state as usize % 23 + 1).min(output.len() - offset);
                trace.push(count);
                let consumed = read.push(&output[offset..offset + count]).unwrap();
                assert_eq!(
                    consumed, count,
                    "seed={seed:x} case={case} fixture={name} offset={offset} read_trace={trace:?}"
                );
                offset += consumed;
            }
            let frame = read.take().unwrap();
            assert_eq!(
                frame.as_slice(),
                *bytes,
                "seed={seed:x} case={case} fixture={name} read_trace={trace:?}"
            );
            drop(frame);
            observed(&events, 1);
        }
    }
}
fn mark(storage: &mut Storage, probe: &Probe, count: &mut usize) {
    storage.probe = probe.clone();
    *count += 1;
}
fn mark_owned(value: &mut EnvelopeOwned, probe: &Probe) -> usize {
    let mut count = 0;
    if let Some(value) = &mut value.hello {
        mark(&mut value.schema_fingerprint.0, probe, &mut count);
        mark(&mut value.build_fingerprint.0, probe, &mut count);
        mark(&mut value.primary_identity.sid.0, probe, &mut count);
        mark(
            &mut value.primary_identity.authentication_luid.0,
            probe,
            &mut count,
        );
    }
    if let Some(value) = &mut value.begin {
        mark(&mut value.target.0, probe, &mut count);
        mark(&mut value.channel_binding.0, probe, &mut count);
        for text in [
            &mut value.identity.user,
            &mut value.identity.domain,
            &mut value.identity.password,
        ]
        .into_iter()
        .flatten()
        {
            mark(&mut text.0, probe, &mut count);
        }
        if let Some(value) = &mut value.digest {
            mark(&mut value.initial_challenge.0, probe, &mut count);
            mark(&mut value.method.0, probe, &mut count);
            mark(&mut value.uri.0, probe, &mut count);
        }
    }
    if let Some(value) = &mut value.challenge {
        mark(&mut value.payload.0, probe, &mut count);
    }
    if let Some(value) = &mut value.token {
        mark(&mut value.payload.0, probe, &mut count);
    }
    count
}
#[test]
fn all_owned_secret_fields_and_frames_wipe_before_deallocation() {
    for (name, bytes) in super::test_vectors::VECTORS {
        let (probe, events) = probe();
        let mut frame = body(bytes, probe.clone());
        let context = super::tests::context(name);
        let mut owned = profile::decode_owned(&frame, &context).unwrap();
        let count = mark_owned(&mut owned, &probe);
        assert!(events.lock().unwrap().is_empty());
        drop(owned);
        observed(&events, count);
        // The held inbound frame remains initialized and live until its owner drops.
        assert_eq!(frame.as_slice(), *bytes);
        frame.probe = probe;
        drop(frame);
        observed(&events, count + 1);
    }
}
#[test]
fn refused_decode_context_keeps_only_held_frame() {
    for (name, bytes) in super::test_vectors::VECTORS {
        let (probe, events) = probe();
        let frame = body(bytes, probe);
        let mut context = super::tests::context(name);
        if name.starts_with("token") || *name == "challenge" {
            context.expected_round = Some(3);
        } else if *name == "hello" {
            context.build = Some(&[0; 32]);
        } else {
            continue;
        }
        assert!(profile::decode_owned(&frame, &context).is_err());
        assert!(events.lock().unwrap().is_empty());
        drop(frame);
        observed(&events, 1);
    }
}
#[test]
fn owned_secret_types_remain_send_sync() {
    fn require<T: Send + Sync>() {}
    require::<crate::SecretBytes>();
    require::<crate::SecretText>();
    require::<crate::AuthRequest>();
    require::<crate::TokenStep>();
    require::<Storage>();
    require::<EnvelopeOwned>();
    let (probe, events) = probe();
    let mut secret = crate::SecretBytes::new(b"synthetic-only");
    secret.0.probe = probe.clone();
    let mut text = crate::SecretText::new("synthetic-only").unwrap();
    text.0.probe = probe;
    drop(secret);
    drop(text);
    observed(&events, 2);
}

#[test]
fn every_short_encode_destination_wipes_partial_payload_before_release() {
    use super::cbor::Writer;
    for (name, bytes) in super::test_vectors::VECTORS {
        let frame = Storage::copy(bytes);
        let context = super::tests::context(name);
        let value = profile::decode(&frame, &context).unwrap();
        for len in 0..bytes.len() {
            let (probe, events) = probe();
            let mut destination = Storage::zeroed(len);
            destination.probe = probe;
            let mut writer = Writer::new(destination.as_mut());
            assert!(
                value.emit(&mut writer).is_err(),
                "fixture={name} destination={len}"
            );
            drop(destination);
            observed(&events, 1);
            assert_eq!(events.lock().unwrap()[0].0, len);
        }
    }
}
