#[test]
fn successful_serial_bridge_holds_native_owner_until_finish_and_wipes_output() {
    use crate::protocol::supervision::{self, Reply};
    for explicit in [false, true] {
        let mut provider = Provider::new();
        let req = request(Package::Ntlm, explicit, 64);
        let mut input = sequence(&[
            supervision::begin(&req).unwrap(),
            supervision::challenge(2, &SecretBytes::new(b"challenge"), req.token_limit).unwrap(),
            supervision::finish().unwrap(),
        ]);
        let mut output = Output::new();
        session::run(
            &mut input,
            &mut output,
            &mut provider,
            &primary(),
            &[0x42; 32],
        )
        .unwrap();
        let reply = replies(&output, Package::Ntlm, 64);
        assert!(matches!(
            reply.as_slice(),
            [
                Reply::Hello,
                Reply::Token { round: 1, .. },
                Reply::Token { round: 2, .. },
                Reply::Finished
            ]
        ));
        let calls = &provider.log.lock().unwrap().calls;
        assert_eq!(
            calls,
            &[
                "maximum",
                "acquire",
                "initialize",
                "observation",
                "free_output",
                "initialize",
                "observation",
                "free_output",
                "cleanup",
                "session_drop"
            ]
        );
        assert_eq!(*provider.events.lock().unwrap(), [(9, true), (9, true)]);
    }
}
#[test]
fn native_status_completion_and_free_failures_are_terminal_before_publication() {
    use crate::protocol::supervision::{self, Reply};
    for status in [0, 0x90312, 0x90313, 0x90314, 0x80090308, 0x90315] {
        for fault in [
            None,
            Some("complete"),
            Some("observation"),
            Some("free_output"),
            Some("initialize"),
            Some("cleanup"),
        ] {
            let req = request(Package::Ntlm, true, 64);
            let mut provider = Provider::new();
            provider.status = status;
            provider.fail = fault;
            let mut input = sequence(&[
                supervision::begin(&req).unwrap(),
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
            let reply = replies(&output, Package::Ntlm, 64);
            let needs_complete = status == 0x90313 || status == 0x90314;
            let supported = [0, 0x90312, 0x90313, 0x90314].contains(&status);
            let succeeds =
                supported && (fault.is_none() || fault == Some("complete") && !needs_complete);
            assert_eq!(
                result.is_ok(),
                succeeds,
                "status={status:x} fault={fault:?}"
            );
            let tokens = reply
                .iter()
                .filter(|r| matches!(r, Reply::Token { .. }))
                .count();
            assert_eq!(
                tokens,
                usize::from(succeeds || supported && fault == Some("cleanup"))
            );
            if let Err(error) = result {
                assert_eq!(error.kind(), ErrorKind::ProviderRejected);
            }
            let calls = provider.log.lock().unwrap();
            assert_eq!(calls.calls.iter().filter(|c| **c == "cleanup").count(), 1);
            assert_eq!(
                calls.calls.iter().filter(|c| **c == "complete").count(),
                usize::from(needs_complete && fault != Some("initialize"))
            );
            assert!(
                provider
                    .events
                    .lock()
                    .unwrap()
                    .iter()
                    .all(|(_, wiped)| *wiped)
            );
        }
    }
}
#[test]
fn provider_cap_and_digest_gap_refuse_before_credentials() {
    use crate::protocol::supervision::{self, Reply};
    let mut req = request(Package::Digest, true, 64);
    req.digest = Some(crate::DigestInput {
        initial_challenge: SecretBytes::new(b"challenge"),
        method: SecretText::new("POST").unwrap(),
        uri: SecretText::new("/resource%20a").unwrap(),
    });
    for (max, kind) in [
        (0, ErrorKind::ProviderRejected),
        (8, ErrorKind::InvalidRequest),
        (9, ErrorKind::ProviderRejected),
        (65536, ErrorKind::ProviderRejected),
    ] {
        let mut provider = Provider::new();
        provider.max = max;
        let mut input = sequence(&[supervision::begin(&req).unwrap()]);
        let mut output = Output::new();
        let error = session::run(
            &mut input,
            &mut output,
            &mut provider,
            &primary(),
            &[0x42; 32],
        )
        .unwrap_err();
        assert_eq!(error.kind(), kind);
        assert_eq!(provider.log.lock().unwrap().calls, ["maximum"]);
        assert!(matches!(
            replies(&output, Package::Digest, 64).as_slice(),
            [Reply::Hello, Reply::Failure(_)]
        ));
    }
}
#[test]
fn output_bound_and_round_eight_refuse_and_release_native_buffers() {
    use crate::protocol::supervision::{self, Reply};
    for size in [0, 63, 64, 65, 65537] {
        let req = request(Package::Ntlm, false, 64);
        let mut provider = Provider::new();
        provider.size = size;
        provider.status = 0;
        let mut input = sequence(&[
            supervision::begin(&req).unwrap(),
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
        assert_eq!(result.is_ok(), size <= 64);
        assert_eq!(*provider.events.lock().unwrap(), [(size, true)]);
    }
    let req = request(Package::Ntlm, false, 64);
    let mut provider = Provider::new();
    let mut frames = vec![supervision::begin(&req).unwrap()];
    for round in 2..=8 {
        frames.push(
            supervision::challenge(round, &SecretBytes::new(b"challenge"), req.token_limit)
                .unwrap(),
        );
    }
    let mut input = sequence(&frames);
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
        ErrorKind::ProviderRejected
    );
    assert_eq!(provider.log.lock().unwrap().rounds, 8);
    assert_eq!(
        replies(&output, Package::Ntlm, 64)
            .iter()
            .filter(|r| matches!(r, Reply::Token { .. }))
            .count(),
        7
    );
    assert_eq!(provider.events.lock().unwrap().len(), 8);
}
#[test]
fn eighth_round_complete_is_legal_and_finishes_the_serial_context() {
    use crate::protocol::supervision::{self, Reply};
    let req = request(Package::Ntlm, false, 64);
    let mut provider = Provider::new();
    provider.complete_round = Some(8);
    let mut frames = vec![supervision::begin(&req).unwrap()];
    for round in 2..=8 {
        frames.push(
            supervision::challenge(round, &SecretBytes::new(b"challenge"), req.token_limit)
                .unwrap(),
        );
    }
    frames.push(supervision::finish().unwrap());
    let mut input = sequence(&frames);
    let mut output = Output::new();
    session::run(
        &mut input,
        &mut output,
        &mut provider,
        &primary(),
        &[0x42; 32],
    )
    .unwrap();
    let reply = replies(&output, Package::Ntlm, 64);
    assert!(
        matches!(&reply[8],Reply::Token{round:8,step} if step.status==crate::TokenStatus::Complete)
    );
    assert!(matches!(reply.last(), Some(Reply::Finished)));
    assert_eq!(provider.log.lock().unwrap().rounds, 8);
    assert_eq!(provider.events.lock().unwrap().len(), 8);
}
