mod implementations;
mod methods;

use std::{sync::Arc, task::Waker};

use crate::PromiseRejection;

use super::state::SharedState;

/// A cloneable, multi-consumer handle to a [`crate::Promise`], created by [`crate::Promise::shared`].
pub struct SharedPromise<T, E>
where
    E: PromiseRejection,
{
    pub(super) state: Arc<SharedState<T, E>>,
    pub(super) waker: Waker,
    pub(super) waiter_id: usize,
}
