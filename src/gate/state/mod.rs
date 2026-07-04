mod implementations;
mod methods;

use std::{
    sync::{atomic::AtomicBool, Mutex},
    task::Waker,
};

/// State shared between a [`GatedPromise`](super::GatedPromise) and the waker
/// it passes to the inner promise: the most recently received outer waker and
/// a flag recording whether the inner promise requested a wakeup.
#[derive(Debug)]
pub(super) struct GateState {
    /// The most recently received outer waker. Replaced on every poll of the
    /// wrapper, and taken when the inner promise requests a wakeup.
    waker: Mutex<Option<Waker>>,

    /// Whether the inner promise requested a wakeup since it was last polled.
    woke: AtomicBool,
}
