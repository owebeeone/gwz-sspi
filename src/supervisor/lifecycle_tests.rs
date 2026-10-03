#[test]
fn fifo_capacity_quarantine_drop_and_pre_registration_controls() {
    for capacity in [1, 8, 64] {
        let context = context(capacity);
        let mut records = Vec::new();
        for _ in 0..capacity {
            records.push(register(&context));
        }
        let first = waiter(&context, Instant::now() + Duration::from_secs(3600));
        let next = waiter(&context, first.deadline);
        assert!(
            context
                .register(&first, &request(), Instant::now())
                .unwrap()
                .is_none()
        );
        context.fault(&records[0], Fault::new(ErrorKind::Cancelled));
        assert_eq!(
            context.status(&context.id(records[0].sequence)),
            CleanupStatus::Pending
        );
        assert!(
            context
                .register(&first, &request(), Instant::now())
                .unwrap()
                .is_none()
        );
        complete(&context, &records[0]);
        assert!(
            context
                .register(&next, &request(), Instant::now())
                .unwrap()
                .is_none()
        );
        drop(first);
        assert!(
            context
                .register(&next, &request(), Instant::now())
                .unwrap()
                .is_some()
        );
        context.close();
        assert_eq!(
            context
                .register(&next, &request(), Instant::now())
                .err()
                .unwrap()
                .kind,
            ErrorKind::CapacityUnavailable
        );
    }
}
#[test]
fn checked_ids_fifo_tombstones_and_cross_context_are_bounded() {
    let context = context(1);
    let other = super::context(1);
    let mut ids = Vec::new();
    for _ in 0..257 {
        let record = register(&context);
        ids.push(context.id(record.sequence));
        complete(&context, &record);
    }
    assert_eq!(context.status(&ids[0]), CleanupStatus::Unknown);
    assert_eq!(context.status(&ids[1]), CleanupStatus::Confirmed);
    assert_eq!(other.status(&ids[1]), CleanupStatus::Unknown);
    context.state.lock().unwrap().next = u64::MAX;
    let waiter = waiter(&context, Instant::now() + Duration::from_secs(3600));
    assert_eq!(
        context
            .register(&waiter, &request(), Instant::now())
            .err()
            .unwrap()
            .kind,
        ErrorKind::CapacityUnavailable
    );
    assert!(context.snapshot().outstanding.is_empty());
}
#[test]
fn completed_refused_start_releases_waiter_inputs_origin_and_wake() {
    let context = context(1);
    let supervisor = Supervisor {
        context: context.clone(),
    };
    let mut invalid = request();
    invalid.target = SecretText::new("not-http").unwrap();
    let events = Arc::new(std::sync::Mutex::new(Vec::new()));
    invalid.channel_binding.0.probe = crate::secret::audit::Probe(Some(events.clone()));
    let mut future = Box::pin(supervisor.start(
        invalid,
        Deadline::new(Instant::now() + Duration::from_secs(3600)),
        Cancellation::new(),
    ));
    let result = poll(&mut future);
    assert!(
        matches!(result,Poll::Ready(Err(failure)) if failure.record_id().is_none() && failure.cleanup_status()==CleanupStatus::Confirmed)
    );
    assert!(context.state.lock().unwrap().waiters.is_empty());
    assert_eq!(*events.lock().unwrap(), vec![(53, true)]);
    let next = waiter(&context, Instant::now() + Duration::from_secs(3600));
    assert!(
        context
            .register(&next, &request(), Instant::now())
            .unwrap()
            .is_some()
    );
    // Keep the completed failed future alive through the following admission.
    drop(future);
}
#[test]
fn poll_only_queues_charged_launch_and_keeps_original_capture_when_moved() {
    let checks = Arc::new(AtomicUsize::new(0));
    let captures = Arc::new(AtomicUsize::new(0));
    let base = context(1);
    let platform = Arc::new(FakePlatform {
        primary: Primary {
            sid: SecretBytes::new(&[1, 0, 0, 0, 0, 0, 0, 0]),
            luid: SecretBytes::new(&[0; 8]),
            session: 0,
        },
        captures: captures.clone(),
        checks: checks.clone(),
    });
    let context = Context::new(
        WorkerExecutable::new(base.executable.path.clone(), base.executable.build).unwrap(),
        1,
        platform,
    );
    let supervisor = Supervisor {
        context: context.clone(),
    };
    let mut future = Box::pin(supervisor.start(
        request(),
        Deadline::new(Instant::now() + Duration::from_secs(3600)),
        Cancellation::new(),
    ));
    assert_eq!(captures.load(Ordering::SeqCst), 1);
    assert!(poll(&mut future).is_pending());
    assert_eq!(checks.load(Ordering::SeqCst), 0);
    assert_eq!(context.snapshot().outstanding.len(), 1);
    let (record, origin) = context.dispatch.lock().unwrap().pop_front().unwrap();
    origin.verify().unwrap();
    assert_eq!(checks.load(Ordering::SeqCst), 1);
    context.update(&record, Instant::now(), |kernel| {
        kernel.launch_returned(true).unwrap();
        kernel.hello().unwrap();
    });
    assert!(matches!(poll(&mut future), Poll::Ready(Ok(_))));
}
#[test]
fn drop_after_registration_and_dropped_shutdown_retain_real_charge() {
    let context = context(1);
    let supervisor = Supervisor {
        context: context.clone(),
    };
    let mut future = Box::pin(supervisor.start(
        request(),
        Deadline::new(Instant::now() + Duration::from_secs(3600)),
        Cancellation::new(),
    ));
    assert!(poll(&mut future).is_pending());
    drop(future);
    let (record, origin) = context.dispatch.lock().unwrap().pop_front().unwrap();
    drop(origin);
    assert_eq!(
        context.status(&context.id(record.sequence)),
        CleanupStatus::Pending
    );
    context.update(&record, Instant::now(), |kernel| {
        assert!(!kernel.launch_returned(true).unwrap())
    });
    let shutdown = supervisor.shutdown(Deadline::new(Instant::now()));
    drop(shutdown);
    assert!(context.state.lock().unwrap().closed);
    assert_eq!(context.snapshot().outstanding.len(), 1);
    complete(&context, &record);
    assert_eq!(
        context.status(&context.id(record.sequence)),
        CleanupStatus::Confirmed
    );
}
#[test]
fn ready_token_is_refused_after_cancel_or_deadline_even_when_not_polled_earlier() {
    for expired in [false, true] {
        let context = context(1);
        let record = register(&context);
        context.update(&record, Instant::now(), |kernel| {
            kernel.stage = Stage::TokenReady {
                round: 1,
                status: TokenStatus::Complete,
                write_done: true,
            };
            if expired {
                kernel.deadline = Instant::now();
            }
        });
        record.payload.lock().unwrap().token = Some(token());
        if !expired {
            record.cancel.cancel();
        }
        let mut conversation = Conversation {
            context: context.clone(),
            record: record.clone(),
            armed: true,
        };
        let mut step = super::super::futures::Step {
            conversation: &mut conversation,
            challenge: None,
            started: true,
            done: false,
        };
        assert!(
            matches!(poll(&mut step),Poll::Ready(Err(failure)) if failure.kind()==if expired {ErrorKind::Timeout}else{ErrorKind::Cancelled})
        );
        assert!(record.payload.lock().unwrap().token.is_some());
    }
}
#[test]
fn normal_finish_ready_then_cancelled_before_poll_is_refused_after_reap() {
    let context = context(1);
    let record = register(&context);
    context.update(&record, Instant::now(), |kernel| {
        kernel.stage = Stage::Closing {
            ack: true,
            write_done: true,
        };
        kernel.proof = Proof {
            process_exited: true,
            job_empty: true,
            launch_finished: true,
            reader_finished: true,
            writer_finished: true,
        };
    });
    drop(context.reap(record.sequence).unwrap());
    record.cancel.cancel();
    let conversation = Conversation {
        context: context.clone(),
        record,
        armed: true,
    };
    let mut finish = super::super::futures::Finish {
        conversation,
        started: true,
        done: false,
    };
    assert!(
        matches!(poll(&mut finish),Poll::Ready(Err(failure)) if failure.kind()==ErrorKind::Cancelled && failure.cleanup_status()==CleanupStatus::Confirmed)
    );
}
#[test]
fn stale_pre_lock_time_cannot_admit_or_publish_past_deadline() {
    let context = context(1);
    let stale = Instant::now() - Duration::from_secs(2);
    let waiter = waiter(&context, Instant::now() - Duration::from_secs(1));
    assert_eq!(
        context
            .register(&waiter, &request(), stale)
            .err()
            .unwrap()
            .kind,
        ErrorKind::Timeout
    );
    let record = register(&context);
    context
        .state
        .lock()
        .unwrap()
        .records
        .get_mut(&record.sequence)
        .unwrap()
        .kernel
        .deadline = Instant::now() - Duration::from_secs(1);
    assert_eq!(
        context.update(&record, stale, |kernel| kernel.terminal.unwrap().kind),
        Some(ErrorKind::Timeout)
    );
}
#[test]
fn finish_ready_before_deadline_is_not_returned_when_polled_after_expiry() {
    let context = context(1);
    let record = register(&context);
    context.update(&record, Instant::now(), |kernel| {
        kernel.stage = Stage::Closing {
            ack: true,
            write_done: true,
        };
        kernel.proof = Proof {
            process_exited: true,
            job_empty: true,
            launch_finished: true,
            reader_finished: true,
            writer_finished: true,
        };
    });
    drop(context.reap(record.sequence).unwrap());
    context.clock.set(record.deadline + Duration::from_secs(1));
    let conversation = Conversation {
        context: context.clone(),
        record,
        armed: true,
    };
    let mut finish = super::super::futures::Finish {
        conversation,
        started: true,
        done: false,
    };
    assert!(
        matches!(poll(&mut finish),Poll::Ready(Err(failure)) if failure.kind()==ErrorKind::Timeout && failure.cleanup_status()==CleanupStatus::Confirmed)
    );
}
