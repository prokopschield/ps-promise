use crate::{BoxedPromiseFuture, Promise, PromiseRejection};

impl<T, E> From<BoxedPromiseFuture<T, E>> for Promise<T, E>
where
    E: PromiseRejection,
{
    fn from(future: BoxedPromiseFuture<T, E>) -> Self {
        Self::Pending(future)
    }
}
