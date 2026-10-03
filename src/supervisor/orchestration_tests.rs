#[test]
fn captured_origin_disposes_synchronously_on_each_unregistered_refusal_and_drop() {
    for case in 0..4 {
        let (context, effects) = host(true);
        let supervisor = Supervisor {
            context: context.clone(),
        };
        let cancellation = Cancellation::new();
        let deadline = context.clock.now() + Duration::from_secs(3600);
        let mut start =
            Box::pin(supervisor.start(request(), Deadline::new(deadline), cancellation.clone()));
        assert_eq!(effects.origin_drops.load(Ordering::SeqCst), 0);
        match case {
            0 => {
                cancellation.cancel();
            }
            1 => {
                context.clock.set(deadline + Duration::from_secs(1));
            }
            2 => {
                context.close();
            }
            _ => {
                drop(start);
                assert_eq!(effects.origin_drops.load(Ordering::SeqCst), 1);
                continue;
            }
        }
        assert!(
            matches!(poll(&mut start),Poll::Ready(Err(failure)) if failure.record_id().is_none() && failure.cleanup_status()==CleanupStatus::Confirmed)
        );
        assert_eq!(
            effects.origin_drops.load(Ordering::SeqCst),
            1,
            "case={case}"
        );
        assert!(context.snapshot().outstanding.is_empty());
        assert_eq!(effects.writes.load(Ordering::SeqCst), 0);
        // Completed future is deliberately retained through these observations.
    }
}
#[test]
fn launch_return_or_refusal_preserves_owned_child_until_actual_observations_and_join() {
    for case in 0..4 {
        let (context, effects) = host(case != 1);
        effects.late_cancel.store(case == 0, Ordering::SeqCst);
        effects.resume_fail.store(case == 2, Ordering::SeqCst);
        effects.launch_fail.store(case == 3, Ordering::SeqCst);
        let (record, origin) = ticket(&context, &effects);
        assert!(monitor::launch(&context, &record, origin).is_none());
        assert_eq!(effects.writes.load(Ordering::SeqCst), 0);
        let trace = effects.trace.lock().unwrap().clone();
        assert_eq!(
            trace.iter().filter(|event| **event == "resume").count(),
            usize::from(case == 2)
        );
        if case != 3 {
            assert_eq!(monitor::reap_once(&context, &record), Some(false));
            assert!(context.reap(record.sequence).is_none());
            assert_eq!(effects.child_drops.load(Ordering::SeqCst), 0);
            effects.exited.store(true, Ordering::SeqCst);
            assert_eq!(monitor::reap_once(&context, &record), Some(false));
            effects.empty.store(true, Ordering::SeqCst);
            assert_eq!(monitor::reap_once(&context, &record), Some(true));
            assert_eq!(effects.child_drops.load(Ordering::SeqCst), 1);
        }
        assert_eq!(
            context.status(&context.id(record.sequence)),
            CleanupStatus::Pending
        );
        finish_launch(&context, &record, &effects);
        assert_eq!(
            context.status(&context.id(record.sequence)),
            CleanupStatus::Confirmed
        );
    }
}
#[test]
fn unfinished_io_and_launch_owners_gate_disposal_and_capacity_despite_failed_termination() {
    let (context, effects) = host(true);
    let (record, origin) = ticket(&context, &effects);
    let pipes = monitor::launch(&context, &record, origin).unwrap();
    drop(pipes);
    let (reader, r) = owner(&effects, "reader-joined", true);
    let (writer, w) = owner(&effects, "writer-joined", true);
    {
        let mut tasks = record.tasks.lock().unwrap();
        tasks.reader = Some(reader);
        tasks.writer = Some(writer);
    }
    context.fault(&record, Fault::new(ErrorKind::Cancelled));
    let waiting = waiter(&context, record.deadline);
    for stage in 0..4 {
        if stage == 1 {
            effects.exited.store(true, Ordering::SeqCst);
            effects.empty.store(true, Ordering::SeqCst);
        }
        if stage == 2 {
            r.finished.store(true, Ordering::SeqCst);
        }
        if stage == 3 {
            w.finished.store(true, Ordering::SeqCst);
        }
        assert_eq!(
            monitor::reap_once(&context, &record),
            Some(stage == 3),
            "stage={stage}"
        );
        assert!(
            context
                .register(&waiting, &request(), context.clock.now())
                .unwrap()
                .is_none()
        );
        assert!(context.reap(record.sequence).is_none());
        assert_eq!(
            effects.child_drops.load(Ordering::SeqCst),
            usize::from(stage == 3)
        );
    }
    assert!(r.joined.load(Ordering::SeqCst));
    assert!(w.joined.load(Ordering::SeqCst));
    finish_launch(&context, &record, &effects);
    assert!(
        context
            .register(&waiting, &request(), context.clock.now())
            .unwrap()
            .is_some()
    );
    let trace = effects.trace.lock().unwrap().clone();
    assert!(trace.contains(&"termination-request-failed"));
    assert!(trace.contains(&"advisory-owner-cancel"));
    assert!(
        trace
            .iter()
            .position(|event| *event == "writer-joined")
            .unwrap()
            < trace
                .iter()
                .position(|event| *event == "held-child-disposed")
                .unwrap()
    );
    assert!(
        trace
            .iter()
            .position(|event| *event == "held-child-disposed")
            .unwrap()
            < trace
                .iter()
                .position(|event| *event == "launch-joined")
                .unwrap()
    );
}
#[test]
fn production_reader_writer_loops_handle_errors_panics_and_dispose_frames() {
    for writer in [false, true] {
        for fault in [1, 2] {
            let (context, effects) = host(true);
            let (record, origin) = ticket(&context, &effects);
            drop(monitor::launch(&context, &record, origin).unwrap());
            let (held, completion) = owner(
                &effects,
                if writer {
                    "writer-joined"
                } else {
                    "reader-joined"
                },
                true,
            );
            if writer {
                record.tasks.lock().unwrap().writer = Some(held);
            } else {
                record.tasks.lock().unwrap().reader = Some(held);
            }
            let events = Arc::new(Mutex::new(Vec::new()));
            if writer {
                context.update(&record, context.clock.now(), |kernel| {
                    kernel.hello().unwrap();
                    kernel
                        .command(super::super::kernel::Command::Begin)
                        .unwrap();
                });
                let (sender, receiver) = std::sync::mpsc::sync_channel(1);
                let mut frame = crate::protocol::supervision::begin(&request()).unwrap();
                let length = frame.as_slice().len();
                frame.probe = crate::secret::audit::Probe(Some(events.clone()));
                sender
                    .send(frame)
                    .unwrap_or_else(|_| panic!("test channel"));
                drop(sender);
                io::writer(
                    context.clone(),
                    record.clone(),
                    Box::new(Output(effects.clone(), fault)),
                    receiver,
                );
                let observed = events.lock().unwrap().clone();
                assert_eq!(observed, vec![(length, true)]);
            } else {
                io::reader(
                    context.clone(),
                    record.clone(),
                    Box::new(Input::new(effects.clone(), Vec::new(), 1, fault)),
                );
            }
            assert_eq!(
                context.update(&record, context.clock.now(), |kernel| kernel
                    .terminal
                    .unwrap()
                    .kind),
                Some(ErrorKind::Protocol)
            );
            assert_eq!(monitor::reap_once(&context, &record), Some(false));
            completion.finished.store(true, Ordering::SeqCst);
            effects.exited.store(true, Ordering::SeqCst);
            effects.empty.store(true, Ordering::SeqCst);
            assert_eq!(monitor::reap_once(&context, &record), Some(true));
            assert!(completion.joined.load(Ordering::SeqCst));
            finish_launch(&context, &record, &effects);
        }
    }
}
#[test]
fn completed_owner_join_failures_do_not_manufacture_native_cleanup_or_override_terminal() {
    for launch_failure in [false, true] {
        let (context, effects) = host(true);
        let (record, origin) = ticket(&context, &effects);
        drop(monitor::launch(&context, &record, origin).unwrap());
        let (held, completion) = owner(&effects, "panicked-owner-joined", false);
        completion.finished.store(true, Ordering::SeqCst);
        if launch_failure {
            *record.launch.lock().unwrap() = Some(held);
            dispatch::join_launch(&context, &record);
            assert_eq!(
                context.update(&record, context.clock.now(), |kernel| kernel
                    .terminal
                    .unwrap()
                    .kind),
                Some(ErrorKind::ContainmentFailed)
            );
        } else {
            record.tasks.lock().unwrap().reader = Some(held);
            assert_eq!(monitor::reap_once(&context, &record), Some(false));
            assert_eq!(
                context.update(&record, context.clock.now(), |kernel| kernel
                    .terminal
                    .unwrap()
                    .kind),
                Some(ErrorKind::Protocol)
            );
        }
        assert!(completion.joined.load(Ordering::SeqCst));
        assert!(context.reap(record.sequence).is_none());
        assert_eq!(effects.child_drops.load(Ordering::SeqCst), 0);
        effects.exited.store(true, Ordering::SeqCst);
        effects.empty.store(true, Ordering::SeqCst);
        assert_eq!(monitor::reap_once(&context, &record), Some(true));
        // A monitor still in recovery owns its launch until that owner finishes.
        finish_launch(&context, &record, &effects);
    }
}
#[test]
fn production_fake_port_lifecycle_moves_payloads_and_confirms_after_actual_owner_disposal() {
    let (context, effects) = host(true);
    let supervisor = Supervisor {
        context: context.clone(),
    };
    let mut start = Box::pin(supervisor.start(
        request(),
        Deadline::new(context.clock.now() + Duration::from_secs(3600)),
        Cancellation::new(),
    ));
    assert!(poll(&mut start).is_pending());
    let (record, origin) = dispatch::next_launch(&context).unwrap();
    effects
        .sequence
        .store(record.sequence as usize, Ordering::SeqCst);
    let (mut reader, mut writer) = monitor::launch(&context, &record, origin).unwrap();
    let commands = monitor::command_channel(&record);
    assert!(!io::read_once(&context, &record, &mut *reader).unwrap());
    let mut conversation = match poll(&mut start) {
        Poll::Ready(Ok(conversation)) => conversation,
        _ => {
            panic!("verified Hello did not produce conversation");
        }
    };
    let mut step = Box::pin(conversation.step(None));
    assert!(poll(&mut step).is_pending());
    assert!(
        io::write_one(
            &context,
            &record,
            &mut *writer,
            commands.try_recv().unwrap()
        )
        .unwrap()
    );
    assert!(!io::read_once(&context, &record, &mut *reader).unwrap());
    assert!(
        matches!(poll(&mut step),Poll::Ready(Ok(token)) if token.status==TokenStatus::Complete)
    );
    drop(step);
    let mut finish = Box::pin(conversation.finish());
    assert!(poll(&mut finish).is_pending());
    let sender = record.payload.lock().unwrap().sender.take();
    drop(sender);
    io::writer(context.clone(), record.clone(), writer, commands);
    io::reader(context.clone(), record.clone(), reader);
    assert!(poll(&mut finish).is_pending());
    effects.exited.store(true, Ordering::SeqCst);
    effects.empty.store(true, Ordering::SeqCst);
    assert_eq!(monitor::reap_once(&context, &record), Some(true));
    assert!(poll(&mut finish).is_pending());
    assert_eq!(effects.child_drops.load(Ordering::SeqCst), 1);
    finish_launch(&context, &record, &effects);
    assert!(matches!(poll(&mut finish), Poll::Ready(Ok(()))));
    assert_eq!(
        context.status(&context.id(record.sequence)),
        CleanupStatus::Confirmed
    );
    let trace = effects.trace.lock().unwrap().clone();
    assert!(trace.contains(&"read-port-disposed"));
    assert!(trace.contains(&"write-port-disposed"));
}
#[test]
fn actual_port_token_readiness_loses_to_reaping_in_each_cancel_expiry_payload_order() {
    for expired in [false, true] {
        for taken_first in [false, true] {
            let (context, effects) = host(true);
            let supervisor = Supervisor {
                context: context.clone(),
            };
            let mut start = Box::pin(supervisor.start(
                request(),
                Deadline::new(context.clock.now() + Duration::from_secs(3600)),
                Cancellation::new(),
            ));
            assert!(poll(&mut start).is_pending());
            let (record, origin) = dispatch::next_launch(&context).unwrap();
            let (mut reader, mut writer) = monitor::launch(&context, &record, origin).unwrap();
            let commands = monitor::command_channel(&record);
            assert!(!io::read_once(&context, &record, &mut *reader).unwrap());
            let mut conversation = match poll(&mut start) {
                Poll::Ready(Ok(value)) => value,
                _ => {
                    panic!("Hello");
                }
            };
            let mut step = Box::pin(conversation.step(None));
            assert!(poll(&mut step).is_pending());
            assert!(
                io::write_one(
                    &context,
                    &record,
                    &mut *writer,
                    commands.try_recv().unwrap()
                )
                .unwrap()
            );
            assert!(!io::read_once(&context, &record, &mut *reader).unwrap());
            assert!(
                context
                    .update(&record, context.clock.now(), |kernel| kernel.publishable())
                    .unwrap()
            );
            let mut held = None;
            if taken_first {
                held = record.payload.lock().unwrap().token.take();
            }
            if expired {
                context.clock.set(record.deadline + Duration::from_secs(1));
            } else {
                record.cancel.cancel();
            }
            context.update(&record, context.clock.now(), |_| {});
            drop(reader);
            drop(writer);
            effects.exited.store(true, Ordering::SeqCst);
            effects.empty.store(true, Ordering::SeqCst);
            assert_eq!(monitor::reap_once(&context, &record), Some(true));
            finish_launch(&context, &record, &effects);
            if !taken_first {
                held = record.payload.lock().unwrap().token.take();
            }
            let failure = super::super::futures::publish(&context, &record, held)
                .err()
                .unwrap();
            assert_eq!(
                failure.kind(),
                if expired {
                    ErrorKind::Timeout
                } else {
                    ErrorKind::Cancelled
                }
            );
            assert_eq!(failure.cleanup_status(), CleanupStatus::Confirmed);
            assert_eq!(effects.child_drops.load(Ordering::SeqCst), 1);
            drop(step);
        }
    }
}
