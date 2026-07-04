use std::sync::Arc;

use crate::Promise;

use super::super::{state::GateState, GatedPromise};

impl<T, E> GatedPromise<T, E> {
    /// Wraps `promise`. The first poll always reaches the wrapped promise.
    pub fn new(promise: Promise<T, E>) -> Self {
        Self {
            inner: promise,
            state: Arc::new(GateState::new()),
        }
    }
}
