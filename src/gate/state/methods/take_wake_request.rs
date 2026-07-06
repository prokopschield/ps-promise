use crate::sync::atomic::Ordering;

use super::super::GateState;

impl GateState {
    /// Returns whether the inner promise requested a wakeup since the last
    /// call, clearing the request.
    pub(in crate::gate) fn take_wake_request(&self) -> bool {
        self.woke.swap(false, Ordering::AcqRel)
    }
}
