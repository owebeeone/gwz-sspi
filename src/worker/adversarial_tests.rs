#[test]
fn actual_frames_and_every_owned_input_wipe_before_normal_or_error_return() {
    use crate::protocol::supervision;
    for fault in [
        None,
        Some("acquire"),
        Some("initialize"),
        Some("complete"),
        Some("observation"),
        Some("free_output"),
        Some("cleanup"),
    ] {
        let req = request(Package::Ntlm, true, 64);
        let mut provider = Provider::new();
        provider.status = 0x90313;
        provider.fail = fault;
        let mut input = sequence(&[
            supervision::begin(&req).unwrap(),
            supervision::finish().unwrap(),
        ]);
        let mut output = Output::new();
        let events = Arc::new(Mutex::new(Vec::new()));
        let probe = Probe(Some(events.clone()));
        let result = session::run_audited(
            &mut input,
            &mut output,
            &mut provider,
            &primary(),
            &[0x42; 32],
            &probe,
        );
        assert_eq!(result.is_ok(), fault.is_none());
        let events = events.lock().unwrap();
        assert!(events.iter().all(|(_, zero)| *zero));
        for len in [
            "HTTP/example.test".len(),
            53,
            "u😀".len(),
            "域".len(),
            "p😀".len(),
        ] {
            assert!(
                events.contains(&(len, true)),
                "fault={fault:?} missing length={len}"
            );
        }
        assert!(events.iter().filter(|(len, _)| *len > 100).count() >= 2);
        assert!(
            provider
                .events
                .lock()
                .unwrap()
                .iter()
                .all(|(_, zero)| *zero)
        );
    }
}
#[test]
fn illegal_phase_and_rounds_refuse_before_another_native_call() {
    use crate::protocol::supervision;
    let req = request(Package::Ntlm, false, 64);
    for complete in [false, true] {
        for wrong in [
            supervision::begin(&req).unwrap(),
            supervision::challenge(3, &SecretBytes::new(b"challenge"), req.token_limit).unwrap(),
            crate::protocol::supervision::fixtures::frame("token_ntlm", Some(2)),
        ] {
            let mut provider = Provider::new();
            if complete {
                provider.status = 0;
            }
            let mut input = sequence(&[supervision::begin(&req).unwrap(), wrong]);
            let mut output = Output::new();
            assert_eq!(
                session::run(
                    &mut input,
                    &mut output,
                    &mut provider,
                    &primary(),
                    &[0x42; 32]
                )
                .unwrap_err()
                .kind(),
                ErrorKind::Protocol
            );
            assert_eq!(provider.log.lock().unwrap().rounds, 1);
            assert_eq!(
                provider
                    .log
                    .lock()
                    .unwrap()
                    .calls
                    .iter()
                    .filter(|c| **c == "cleanup")
                    .count(),
                1
            );
        }
    }
}
#[test]
fn negotiate_observation_is_admitted_before_owned_copy_and_publication() {
    use crate::protocol::supervision;
    for package in [Package::Negotiate, Package::Ntlm] {
        for status in [0, 0x90312] {
            for observation in [
                MechanismObservation::Unresolved,
                MechanismObservation::Selected {
                    mechanism: Mechanism::Ntlm,
                    authoritative: false,
                },
                MechanismObservation::Selected {
                    mechanism: Mechanism::Kerberos,
                    authoritative: true,
                },
                MechanismObservation::Selected {
                    mechanism: Mechanism::Digest,
                    authoritative: true,
                },
            ] {
                let req = request(package, false, 64);
                let mut provider = Provider::new();
                provider.status = status;
                provider.observation = observation;
                let mut input = sequence(&[
                    supervision::begin(&req).unwrap(),
                    supervision::finish().unwrap(),
                ]);
                let mut output = Output::new();
                let okay = match observation {
                    MechanismObservation::Unresolved => {
                        package == Package::Negotiate && status != 0
                    }
                    MechanismObservation::Selected {
                        mechanism,
                        authoritative,
                    } => {
                        (status != 0 || authoritative)
                            && match package {
                                Package::Negotiate => mechanism != Mechanism::Digest,
                                Package::Ntlm => mechanism == Mechanism::Ntlm,
                                _ => false,
                            }
                    }
                };
                assert_eq!(
                    session::run(
                        &mut input,
                        &mut output,
                        &mut provider,
                        &primary(),
                        &[0x42; 32]
                    )
                    .is_ok(),
                    okay,
                    "package={package:?} status={status:x} observation={observation:?}"
                );
                assert_eq!(*provider.events.lock().unwrap(), [(9, true)]);
            }
        }
    }
}
#[test]
fn every_partial_input_and_output_failure_retains_normal_disposal() {
    use crate::protocol::supervision;
    let req = request(Package::Ntlm, true, 64);
    let begin = supervision::begin(&req).unwrap();
    let finish = supervision::finish().unwrap();
    let whole = sequence(&[begin, finish]);
    for offset in 0..=whole.bytes.as_slice().len() {
        let mut input = Input {
            bytes: Storage::copy(&whole.bytes.as_slice()[..offset]),
            at: 0,
            chunk: 3,
        };
        let mut output = Output::new();
        let mut provider = Provider::new();
        let events = Arc::new(Mutex::new(Vec::new()));
        let _ = session::run_audited(
            &mut input,
            &mut output,
            &mut provider,
            &primary(),
            &[0x42; 32],
            &Probe(Some(events.clone())),
        );
        let trace = provider.log.lock().unwrap();
        assert_eq!(
            trace.calls.iter().filter(|c| **c == "cleanup").count(),
            usize::from(trace.calls.contains(&"acquire")),
            "input offset={offset}"
        );
        assert!(
            events.lock().unwrap().iter().all(|(_, zero)| *zero),
            "input offset={offset}"
        );
    }
    let mut provider = Provider::new();
    let mut output = Output::new();
    let mut input = sequence(&[
        supervision::begin(&req).unwrap(),
        supervision::finish().unwrap(),
    ]);
    session::run(
        &mut input,
        &mut output,
        &mut provider,
        &primary(),
        &[0x42; 32],
    )
    .unwrap();
    for offset in 0..output.at {
        let mut provider = Provider::new();
        let mut input = sequence(&[
            supervision::begin(&req).unwrap(),
            supervision::finish().unwrap(),
        ]);
        let mut output = Output::new();
        output.fail_at = Some(offset);
        let events = Arc::new(Mutex::new(Vec::new()));
        assert!(
            session::run_audited(
                &mut input,
                &mut output,
                &mut provider,
                &primary(),
                &[0x42; 32],
                &Probe(Some(events.clone()))
            )
            .is_err()
        );
        let trace = provider.log.lock().unwrap();
        assert_eq!(
            trace.calls.iter().filter(|c| **c == "cleanup").count(),
            usize::from(trace.calls.contains(&"acquire")),
            "output offset={offset}"
        );
        assert!(
            events.lock().unwrap().iter().all(|(_, zero)| *zero),
            "output offset={offset}"
        );
    }
}
#[test]
fn seeded_worker_fault_chunk_and_phase_schedules_use_the_production_bridge() {
    use crate::protocol::supervision;
    for seed in [1u64, 0x649f109d, 0xda26cb46, u64::MAX] {
        let mut state = seed;
        for case in 0..64 {
            let mut next = || {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state
            };
            let chunk = (next() % 31 + 1) as usize;
            let rounds = (next() % 7 + 1) as u8;
            let fault = match next() % 7 {
                0 => Some("initialize"),
                1 => Some("observation"),
                2 => Some("free_output"),
                3 => Some("cleanup"),
                _ => None,
            };
            let req = request(Package::Ntlm, next() & 1 != 0, 64);
            let mut frames = vec![supervision::begin(&req).unwrap()];
            for round in 2..=rounds {
                frames.push(
                    supervision::challenge(round, &SecretBytes::new(b"challenge"), req.token_limit)
                        .unwrap(),
                );
            }
            frames.push(supervision::finish().unwrap());
            let mut input = sequence(&frames);
            input.chunk = chunk;
            let mut output = Output::new();
            output.chunk = (next() % 29 + 1) as usize;
            let mut provider = Provider::new();
            provider.fail = fault;
            let result = session::run(
                &mut input,
                &mut output,
                &mut provider,
                &primary(),
                &[0x42; 32],
            );
            assert_eq!(
                result.is_ok(),
                fault.is_none(),
                "seed={seed:x} case={case} rounds={rounds} input_chunk={chunk} output_chunk={} fault={fault:?} trace={:?}",
                output.chunk,
                provider.log.lock().unwrap().calls
            );
            assert!(
                provider
                    .events
                    .lock()
                    .unwrap()
                    .iter()
                    .all(|(_, zero)| *zero)
            );
        }
    }
}
#[test]
fn worker_framing_rejects_preallocation_bounds_and_partial_port_errors() {
    struct BadRead;
    impl ReadPort for BadRead {
        fn read(&mut self, bytes: &mut [u8]) -> Result<usize, Error> {
            Ok(bytes.len() + 1)
        }
    }
    struct BadWrite;
    impl WritePort for BadWrite {
        fn write(&mut self, bytes: &[u8]) -> Result<usize, Error> {
            Ok(bytes.len() + 1)
        }
    }
    let mut bad_read = BadRead;
    let mut sink = Output::new();
    let mut provider = Provider::new();
    assert_eq!(
        session::run(
            &mut bad_read,
            &mut sink,
            &mut provider,
            &primary(),
            &[0x42; 32]
        )
        .unwrap_err()
        .kind(),
        ErrorKind::Protocol
    );
    assert!(provider.log.lock().unwrap().calls.is_empty());
    let mut input = sequence(&[crate::protocol::supervision::finish().unwrap()]);
    assert_eq!(
        session::run(
            &mut input,
            &mut BadWrite,
            &mut Never,
            &primary(),
            &[0x42; 32]
        )
        .unwrap_err()
        .kind(),
        ErrorKind::Protocol
    );
    for len in [0u32, 100001, u32::MAX] {
        let mut input = Input {
            bytes: Storage::copy(&len.to_le_bytes()),
            at: 0,
            chunk: 1,
        };
        let mut sink = Output::new();
        let mut provider = Provider::new();
        assert_eq!(
            session::run(
                &mut input,
                &mut sink,
                &mut provider,
                &primary(),
                &[0x42; 32]
            )
            .unwrap_err()
            .kind(),
            ErrorKind::Protocol
        );
        assert!(provider.log.lock().unwrap().calls.is_empty());
    }
    struct Failing {
        inner: Input,
        at: usize,
    }
    impl ReadPort for Failing {
        fn read(&mut self, bytes: &mut [u8]) -> Result<usize, Error> {
            if self.inner.at == self.at {
                return Err(Error::new(ErrorKind::Protocol));
            }
            self.inner.read(bytes)
        }
    }
    let req = request(Package::Ntlm, true, 64);
    let begin = crate::protocol::supervision::begin(&req).unwrap();
    let total = begin.as_slice().len() + 4;
    for offset in 0..total {
        let mut input = Failing {
            inner: sequence(&[crate::protocol::supervision::begin(&req).unwrap()]),
            at: offset,
        };
        let mut provider = Provider::new();
        let mut sink = Output::new();
        let events = Arc::new(Mutex::new(Vec::new()));
        assert_eq!(
            session::run_audited(
                &mut input,
                &mut sink,
                &mut provider,
                &primary(),
                &[0x42; 32],
                &Probe(Some(events.clone()))
            )
            .unwrap_err()
            .kind(),
            ErrorKind::Protocol
        );
        assert!(provider.log.lock().unwrap().calls.is_empty());
        assert!(events.lock().unwrap().iter().all(|(_, zero)| *zero));
    }
}
#[test]
fn challenge_uses_provider_and_caller_cap_intersection_before_initialize() {
    use crate::protocol::supervision;
    for max in [8, 9, 10] {
        for cap in [8, 9, 10] {
            let req = request(Package::Ntlm, false, cap);
            let mut provider = Provider::new();
            provider.max = max;
            provider.size = 8;
            let mut input = sequence(&[
                supervision::begin(&req).unwrap(),
                supervision::challenge(
                    2,
                    &SecretBytes::new(b"challenge"),
                    TokenLimit::new(10).unwrap(),
                )
                .unwrap(),
                supervision::finish().unwrap(),
            ]);
            let mut output = Output::new();
            let result = session::run(
                &mut input,
                &mut output,
                &mut provider,
                &primary(),
                &[0x42; 32],
            );
            let admitted = max.min(cap) >= 9;
            assert_eq!(result.is_ok(), admitted, "provider={max} cap={cap}");
            assert_eq!(
                provider.log.lock().unwrap().rounds,
                if admitted { 2 } else { 1 }
            );
            if let Err(error) = result {
                assert_eq!(error.kind(), ErrorKind::Protocol);
            }
        }
    }
}
