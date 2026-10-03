// Test-only ownership used by native fixtures, also exercised without OS effects.
use std::sync::{Arc, Mutex};
pub(crate) trait Cleanup {
    fn cleanup(&mut self) -> bool;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Report {
    pub(crate) helper_confirmed: bool,
    pub(crate) scratch_removed: bool,
}
pub(crate) type Receipt = Arc<Mutex<Option<Report>>>;
pub(crate) struct Guard<P: Cleanup, S: Cleanup> {
    pub(crate) owner: P,
    scratch: S,
    receipt: Receipt,
}
impl<P: Cleanup, S: Cleanup> Guard<P, S> {
    pub(crate) fn new(owner: P, scratch: S, receipt: Receipt) -> Self {
        Self {
            owner,
            scratch,
            receipt,
        }
    }
    pub(crate) fn settle(&mut self) -> Report {
        let saved = *self.receipt.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(report) = saved {
            return report;
        }
        let helper_confirmed =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.owner.cleanup()))
                .unwrap_or(false);
        let scratch_removed =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.scratch.cleanup()))
                .unwrap_or(false);
        let report = Report {
            helper_confirmed,
            scratch_removed,
        };
        *self.receipt.lock().unwrap_or_else(|p| p.into_inner()) = Some(report);
        report
    }
}
impl<P: Cleanup, S: Cleanup> Drop for Guard<P, S> {
    fn drop(&mut self) {
        self.settle();
    }
}
mod tests {
    use super::*;
    struct Port {
        events: Arc<Mutex<Vec<&'static str>>>,
        name: &'static str,
        result: bool,
    }
    impl Cleanup for Port {
        fn cleanup(&mut self) -> bool {
            self.events.lock().unwrap().push(self.name);
            self.result
        }
    }
    #[test]
    fn guard_cleans_immediately_owned_helper_and_scratch_on_every_unwind_boundary() {
        for boundary in ["after_spawn", "after_pid", "observer_failure"] {
            let events = Arc::new(Mutex::new(Vec::new()));
            let receipt = Arc::new(Mutex::new(None));
            let outcome = std::panic::catch_unwind({
                let events = events.clone();
                let receipt = receipt.clone();
                move || {
                    let _guard = Guard::new(
                        Port {
                            events: events.clone(),
                            name: "helper_waited",
                            result: true,
                        },
                        Port {
                            events,
                            name: "scratch_removed",
                            result: true,
                        },
                        receipt,
                    );
                    panic!("forced {boundary}");
                }
            });
            assert!(outcome.is_err());
            assert_eq!(
                *events.lock().unwrap(),
                ["helper_waited", "scratch_removed"]
            );
            assert_eq!(
                *receipt.lock().unwrap(),
                Some(Report {
                    helper_confirmed: true,
                    scratch_removed: true
                })
            );
        }
    }
    #[test]
    fn failed_cleanup_is_reported_and_does_not_skip_scratch() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let receipt = Arc::new(Mutex::new(None));
        let mut guard = Guard::new(
            Port {
                events: events.clone(),
                name: "helper_not_confirmed",
                result: false,
            },
            Port {
                events: events.clone(),
                name: "scratch_removed",
                result: true,
            },
            receipt.clone(),
        );
        assert_eq!(
            guard.settle(),
            Report {
                helper_confirmed: false,
                scratch_removed: true
            }
        );
        drop(guard);
        assert_eq!(
            *events.lock().unwrap(),
            ["helper_not_confirmed", "scratch_removed"]
        );
    }
}
