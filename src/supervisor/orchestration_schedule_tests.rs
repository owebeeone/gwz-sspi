#[test]
fn seeded_actual_reaper_schedules_retain_capacity_until_disposal_then_launch_join() {
    for seed in [1u64, 0x4a2ff930, 0xc1a58b01, 0xffffffffffffffff] {
        let mut random = seed;
        for case in 0..32 {
            let (context, effects) = host(true);
            let (record, origin) = ticket(&context, &effects);
            drop(monitor::launch(&context, &record, origin).unwrap());
            let (reader, r) = owner(&effects, "reader-joined", true);
            let (writer, w) = owner(&effects, "writer-joined", true);
            {
                let mut tasks = record.tasks.lock().unwrap();
                tasks.reader = Some(reader);
                tasks.writer = Some(writer);
            }
            record.cancel.cancel();
            context.update(&record, context.clock.now(), |_| {});
            let mut actions = Vec::new();
            let mut retired = false;
            for _ in 0..64 {
                random ^= random << 13;
                random ^= random >> 7;
                random ^= random << 17;
                let action = random % 4;
                actions.push(action);
                match action {
                    0 => {
                        effects.exited.store(true, Ordering::SeqCst);
                    }
                    1 => {
                        effects.empty.store(true, Ordering::SeqCst);
                    }
                    2 => {
                        r.finished.store(true, Ordering::SeqCst);
                    }
                    _ => {
                        w.finished.store(true, Ordering::SeqCst);
                    }
                }
                if !retired {
                    retired = monitor::reap_once(&context, &record) == Some(true);
                }
                assert!(
                    context.reap(record.sequence).is_none(),
                    "seed={seed:x},case={case},actions={actions:?}"
                );
                assert_eq!(
                    context.status(&context.id(record.sequence)),
                    CleanupStatus::Pending,
                    "seed={seed:x},case={case},actions={actions:?}"
                );
                assert_eq!(
                    effects.child_drops.load(Ordering::SeqCst),
                    usize::from(retired),
                    "seed={seed:x},case={case},actions={actions:?}"
                );
                if retired {
                    break;
                }
            }
            assert!(
                retired,
                "seed={seed:x},case={case},actions={actions:?},effects={:?}",
                effects.trace.lock().unwrap()
            );
            assert!(r.joined.load(Ordering::SeqCst) && w.joined.load(Ordering::SeqCst));
            finish_launch(&context, &record, &effects);
            assert_eq!(
                context.status(&context.id(record.sequence)),
                CleanupStatus::Confirmed
            );
        }
    }
}
