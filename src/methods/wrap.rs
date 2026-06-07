use std::future::Future;

use crate::{Promise, WrappedPromiseRejection};

impl<T, E> Promise<T, WrappedPromiseRejection<E>>
where
    T: Send + Unpin + 'static,
    E: Send + Unpin + 'static,
{
    pub fn wrap(future: impl Future<Output = Result<T, E>> + Send + 'static) -> Self {
        Self::eager_or_lazy(async move { Ok(future.await?) })
    }
}
