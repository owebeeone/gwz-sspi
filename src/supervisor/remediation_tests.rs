#[test]
fn publication_loses_record_but_preserves_cancel_or_timeout_with_both_payload_orders() {
    for kind in [ErrorKind::Cancelled, ErrorKind::Timeout] {
        for taken_first in [false, true] {
            let context = context(1);
            let record = register(&context);
            record.payload.lock().unwrap().token = Some(token());
            context.update(&record, context.clock.now(), |kernel| {
                kernel.stage = Stage::TokenReady {
                    round: 1,
                    status: TokenStatus::Complete,
                    write_done: true,
                }
            });
            assert!(
                context
                    .update(&record, context.clock.now(), |kernel| kernel.publishable())
                    .unwrap()
            );
            let mut held = None;
            if taken_first {
                held = record.payload.lock().unwrap().token.take();
            }
            if kind == ErrorKind::Cancelled {
                record.cancel.cancel();
            } else {
                context.clock.set(record.deadline + Duration::from_secs(1));
            }
            context.update(&record, context.clock.now(), |_| {});
            record.discard_payload();
            context.update(&record, context.clock.now(), |kernel| {
                kernel.proof = Proof {
                    process_exited: true,
                    job_empty: true,
                    launch_finished: true,
                    reader_finished: true,
                    writer_finished: true,
                }
            });
            drop(context.reap(record.sequence).unwrap());
            if !taken_first {
                held = record.payload.lock().unwrap().token.take();
            }
            let failure = super::super::futures::publish(&context, &record, held)
                .err()
                .unwrap();
            assert_eq!(
                failure.kind(),
                kind,
                "winning={kind:?},taken_first={taken_first}"
            );
            assert_eq!(failure.cleanup_status(), CleanupStatus::Confirmed);
        }
    }
}
#[test]
fn every_completed_failed_step_wipes_its_challenge_while_future_is_retained() {
    for case in 0..4 {
        let context = context(1);
        let record = register(&context);
        context.update(&record, context.clock.now(), |kernel| {
            kernel.stage = if case == 1 {
                Stage::Complete
            } else {
                Stage::Between(1)
            }
        });
        if case == 2 {
            record.cancel.cancel();
        }
        if case == 3 {
            context.clock.set(record.deadline + Duration::from_secs(1));
        }
        let length = if case == 0 { 65 } else { 1 };
        let source = zeroize::Zeroizing::new([b'x'; 65]);
        let mut challenge = SecretBytes::new(&source[..length]);
        drop(source);
        let events = Arc::new(std::sync::Mutex::new(Vec::new()));
        challenge.0.probe = crate::secret::audit::Probe(Some(events.clone()));
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        record.payload.lock().unwrap().sender = Some(sender);
        let mut conversation = Conversation {
            context: context.clone(),
            record,
            armed: true,
        };
        let mut future = super::super::futures::Step {
            conversation: &mut conversation,
            challenge: Some(challenge),
            started: false,
            done: false,
        };
        let expected = match case {
            0 => ErrorKind::InvalidRequest,
            1 => ErrorKind::Protocol,
            2 => ErrorKind::Cancelled,
            _ => ErrorKind::Timeout,
        };
        assert!(matches!(poll(&mut future), Poll::Ready(Err(failure)) if failure.kind()==expected));
        let observed = events.lock().unwrap().clone();
        assert_eq!(
            observed,
            vec![(length, true)],
            "case={case}: retained completed future"
        );
        assert!(
            receiver.try_recv().is_err(),
            "case={case}: no command frame"
        );
        assert!(future.challenge.is_none());
        drop(future);
    }
}
