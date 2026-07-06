use crate::sync::{atomic::AtomicBool, Mutex};

use super::super::GateState;

impl GateState {
    /// Creates the gate with the wakeup request initially set, so the first
    /// poll of the wrapper reaches the inner promise.
    #[cfg(not(loom))]
    pub(in crate::gate) const fn new() -> Self {
        Self {
            waker: Mutex::new(None),
            woke: AtomicBool::new(true),
        }
    }

    /// Creates the gate with the wakeup request initially set, so the first
    /// poll of the wrapper reaches the inner promise.
    ///
    /// Not `const` under loom: `loom::sync::Mutex::new` is not a `const fn`.
    #[cfg(loom)]
    pub(in crate::gate) fn new() -> Self {
        Self {
            waker: Mutex::new(None),
            woke: AtomicBool::new(true),
        }
    }
}
