//! Deterministic clock for tests.

use crate::time::Clock;
use jiff::Timestamp;
use std::sync::Mutex;

pub struct TestClock {
    now: Mutex<Timestamp>,
}

impl TestClock {
    #[must_use]
    pub fn at(initial: Timestamp) -> Self {
        Self {
            now: Mutex::new(initial),
        }
    }

    pub fn advance(&self, duration: std::time::Duration) {
        let mut now = self.now.lock().expect("TestClock mutex poisoned");
        *now = now.checked_add(duration).expect("TestClock overflow");
    }

    pub fn set(&self, t: Timestamp) {
        *self.now.lock().expect("TestClock mutex poisoned") = t;
    }
}

impl Clock for TestClock {
    fn now(&self) -> Timestamp {
        *self.now.lock().expect("TestClock mutex poisoned")
    }
}
