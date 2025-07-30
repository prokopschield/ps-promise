use std::future::Future;

use crate::{Promise, WrappedPromiseRejection};

impl<T, E> Promise<T, WrappedPromiseRejection<E>>
where
    T: Unpin,
    E: Send + Sync + Unpin + 'static,
{
    pub fn wrap(future: impl Future<Output = Result<T, E>> + Send + Sync + 'static) -> Self {
        Self::new(async move { Ok(future.await?) })
    }
}
