use std::sync::Arc;

use crate::{BoxedFuture, Transformer};

impl<I, O, E> Transformer<I, O, E> {
    pub fn from_async_box_fn<T>(transform_fn: T) -> Self
    where
        T: Fn(I) -> BoxedFuture<O, E> + 'static,
    {
        Self {
            transform: Arc::new(transform_fn),
        }
    }
}
