use crate::{BoxedPromiseFuture, Promise};

impl<T, E> From<BoxedPromiseFuture<T, E>> for Promise<T, E> {
    fn from(future: BoxedPromiseFuture<T, E>) -> Self {
        Self::Pending(future)
    }
}
