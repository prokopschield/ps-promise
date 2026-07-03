use std::future::Future;

use crate::{Promise, WrappedPromiseRejection};

impl<T, E> Promise<T, WrappedPromiseRejection<E>>
where
    T: Send + 'static,
    E: Send + 'static,
{
    pub fn wrap(future: impl Future<Output = Result<T, E>> + Send + 'static) -> Self {
        Self::eager_or_lazy(async move { Ok(future.await?) })
    }
}
