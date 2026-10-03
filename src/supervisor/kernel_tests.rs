use super::kernel::*;
use crate::{ErrorKind, TokenStatus};
use std::time::{Duration, Instant};

#[test]
fn terminal_first_wins_and_late_output_cannot_publish() {
    let now = Instant::now();
    let mut kernel = Kernel::new(now + Duration::from_secs(1));
    kernel.launch_returned(true).unwrap();
    kernel.hello().unwrap();
    kernel.command(Command::Begin).unwrap();
    kernel.fail(Fault::new(ErrorKind::Cancelled));
    assert!(!kernel.fail(Fault::new(ErrorKind::Timeout)));
    assert!(kernel.token(1, TokenStatus::Complete).is_err());
    assert!(!kernel.publishable());
}
#[test]
fn cleanup_needs_every_held_proof_never_only_kill_finished_or_eof() {
    let mut kernel = Kernel::new(Instant::now());
    kernel.fail(Fault::new(ErrorKind::Cancelled));
    for mask in 0..32 {
        kernel.proof = Proof {
            process_exited: mask & 1 != 0,
            job_empty: mask & 2 != 0,
            launch_finished: mask & 4 != 0,
            reader_finished: mask & 8 != 0,
            writer_finished: mask & 16 != 0,
        };
        assert_eq!(kernel.reapable(), mask == 31, "proof mask={mask}");
    }
}
#[test]
fn all_finish_points_and_write_reply_orders_are_strict() {
    for rounds in 0..=8 {
        for reply_first in [false, true] {
            let mut kernel = Kernel::new(Instant::now() + Duration::from_secs(1));
            kernel.launch_returned(true).unwrap();
            kernel.hello().unwrap();
            for round in 1..=rounds {
                kernel
                    .command(if round == 1 {
                        Command::Begin
                    } else {
                        Command::Challenge(round)
                    })
                    .unwrap();
                assert!(kernel.command(Command::Finish).is_err());
                assert!(kernel.token(round + 1, TokenStatus::Complete).is_err());
                let status = if round == rounds {
                    TokenStatus::Complete
                } else {
                    TokenStatus::Continue
                };
                if reply_first {
                    kernel.token(round, status).unwrap();
                    assert!(!kernel.publishable());
                    kernel.write_done().unwrap();
                } else {
                    kernel.write_done().unwrap();
                    kernel.token(round, status).unwrap();
                }
                assert!(kernel.publishable());
                kernel.published().unwrap();
                assert!(!kernel.publishable());
            }
            kernel.command(Command::Finish).unwrap();
            if reply_first {
                kernel.finished().unwrap();
                assert!(!kernel.normal_ack());
                kernel.write_done().unwrap();
            } else {
                kernel.write_done().unwrap();
                assert!(!kernel.normal_ack());
                kernel.finished().unwrap();
            }
            assert!(kernel.normal_ack());
            assert!(kernel.finished().is_err());
        }
    }
    let mut kernel = Kernel::new(Instant::now() + Duration::from_secs(1));
    kernel.stage = Stage::Waiting {
        round: 8,
        write_done: true,
    };
    assert!(kernel.token(8, TokenStatus::Continue).is_err());
    kernel.token(8, TokenStatus::Complete).unwrap();
    kernel.published().unwrap();
    assert!(kernel.command(Command::Challenge(9)).is_err());
}
#[test]
fn every_cancel_write_reply_schedule_preserves_terminal_arbitration() {
    let actions = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];
    for schedule in actions {
        let mut kernel = Kernel::new(Instant::now() + Duration::from_secs(1));
        kernel.stage = Stage::Waiting {
            round: 1,
            write_done: false,
        };
        for action in schedule {
            match action {
                0 => {
                    kernel.fail(Fault::new(ErrorKind::Cancelled));
                }
                1 => {
                    let _result = kernel.write_done();
                }
                2 => {
                    let _result = kernel.token(1, TokenStatus::Complete);
                }
                _ => {
                    unreachable!();
                }
            }
        }
        assert_eq!(
            kernel.terminal.unwrap().kind,
            ErrorKind::Cancelled,
            "schedule={schedule:?}"
        );
        assert!(!kernel.publishable());
        assert!(kernel.published().is_err());
        assert!(!kernel.launch_returned(true).unwrap());
    }
}
#[test]
fn seeded_adversarial_trace_keeps_first_terminal_and_never_reaps_missing_proofs() {
    for seed in [1u64, 0x82a513c8, 0xdeadc0de, 0xffffffffffffffff] {
        let base = Instant::now();
        let mut state = seed;
        let mut trace = Vec::new();
        for _case in 0..128 {
            let mut kernel = Kernel::new(base + Duration::from_secs(1));
            let mut first = None;
            for _ in 0..128 {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                let action = (state % 12) as u8;
                trace.push(action);
                match action {
                    0 => {
                        kernel.control(base, true);
                    }
                    1 => {
                        kernel.control(base + Duration::from_secs(1), false);
                    }
                    2 => {
                        let _result = kernel.launch_returned(state & 16 != 0);
                    }
                    3 => {
                        let _result = kernel.hello();
                    }
                    4 => {
                        let _result = kernel.command(Command::Begin);
                    }
                    5 => {
                        let _result = kernel.command(Command::Challenge((state % 10) as u8));
                    }
                    6 => {
                        let _result = kernel.token(
                            (state % 10) as u8,
                            if state & 16 == 0 {
                                TokenStatus::Continue
                            } else {
                                TokenStatus::Complete
                            },
                        );
                    }
                    7 => {
                        let _result = kernel.write_done();
                    }
                    8 => {
                        let _result = kernel.published();
                    }
                    9 => {
                        let _result = kernel.command(Command::Finish);
                    }
                    10 => {
                        let _result = kernel.finished();
                    }
                    _ => {
                        kernel.proof = Proof {
                            process_exited: state & 16 != 0,
                            job_empty: state & 32 != 0,
                            launch_finished: state & 64 != 0,
                            reader_finished: state & 128 != 0,
                            writer_finished: state & 256 != 0,
                        };
                    }
                }
                if first.is_none() {
                    first = kernel.terminal;
                }
                assert_eq!(
                    kernel.terminal, first,
                    "seed={seed:x},input/actions={trace:?}"
                );
                assert!(
                    !kernel.reapable() || kernel.proof.complete(),
                    "seed={seed:x},input/actions={trace:?}"
                );
                if first.is_some() {
                    assert!(
                        !kernel.publishable(),
                        "seed={seed:x},input/actions={trace:?}"
                    );
                }
            }
            trace.clear();
        }
    }
}
