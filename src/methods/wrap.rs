use std::future::Future;

use crate::{Promise, WrappedPromiseRejection};

impl<T, E> Promise<T, WrappedPromiseRejection<E>>
where
    T: Send + 'static,
    E: Send + 'static,
{
    /// Wraps a future whose error type does not implement
    /// [`PromiseRejection`](crate::PromiseRejection).
    ///
    /// The error is lifted into [`WrappedPromiseRejection::Rejected`], so
    /// any `E: Send + 'static` can serve as a rejection. The returned
    /// promise is scheduled via [`Promise::eager_or_lazy`].
    pub fn wrap(future: impl Future<Output = Result<T, E>> + Send + 'static) -> Self {
        Self::eager_or_lazy(async move { Ok(future.await?) })
    }
}
