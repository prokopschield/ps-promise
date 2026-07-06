use crate::sync::{atomic::AtomicBool, Mutex};

use super::super::GateState;

impl GateState {
    /// Creates the gate with the wakeup request initially set, so the first
    /// poll of the wrapper reaches the inner promise.
    pub(in crate::gate) const fn new() -> Self {
        Self {
            waker: Mutex::new(None),
            woke: AtomicBool::new(true),
        }
    }
}
