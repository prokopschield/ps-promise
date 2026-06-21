use crate::{Promise, PromiseRejection};

use super::super::AbortablePromise;

impl<T, E> From<AbortablePromise<T, E>> for Promise<T, E>
where
    T: Unpin,
    E: PromiseRejection,
    Self: Send + 'static,
{
    fn from(promise: AbortablePromise<T, E>) -> Self {
        Self::lazy(promise)
    }
}
