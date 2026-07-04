mod methods;
mod state;

use std::sync::Arc;

use crate::Promise;

use self::state::GateState;

/// An internal wrapper that drops spurious re-polls of an inner [`Promise`].
///
/// A combinator that owns several child promises re-polls every still-pending
/// child whenever it is itself polled, so each child is polled once per event
/// on any of its siblings, which is quadratic in the number of children. A
/// `GatedPromise` forwards a poll to the inner promise only when the inner
/// promise requested a wakeup, and drops every other poll as spurious.
///
/// The wrapper passes its own waker to the inner promise and keeps the most
/// recently received outer waker; a wakeup request from the inner promise
/// notifies that waker. The wrapper is single-consumer: it keeps exactly one
/// outer waker, so only the task that polled it last is notified.
#[derive(Debug)]
pub struct GatedPromise<T, E> {
    /// The wrapped promise. Its settled state is inspected and consumed
    /// directly, bypassing the gate.
    pub inner: Promise<T, E>,

    /// State shared with the waker passed to the inner promise.
    state: Arc<GateState>,
}
