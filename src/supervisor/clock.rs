pub(super) use selected::Clock;
// Private clock implementation, selected at an enclosing module boundary.
#[cfg(not(test))]
mod selected {
    use std::time::Instant;
    #[derive(Default)]
    pub(in crate::supervisor) struct Clock {}
    impl Clock {
        pub(in crate::supervisor) fn now(&self) -> Instant {
            Instant::now()
        }
    }
}
#[cfg(test)]
mod selected {
    use std::{sync::Mutex, time::Instant};
    pub(in crate::supervisor) struct Clock(Mutex<Instant>);
    impl Default for Clock {
        fn default() -> Self {
            Self(Mutex::new(Instant::now()))
        }
    }
    impl Clock {
        pub(in crate::supervisor) fn now(&self) -> Instant {
            *self.0.lock().unwrap()
        }
        pub(in crate::supervisor) fn set(&self, time: Instant) {
            *self.0.lock().unwrap() = time;
        }
    }
}
