use std::future::Future;

use crate::{Promise, PromiseRejection};

impl<T, E> Promise<T, E>
where
    T: Send + 'static,
    E: PromiseRejection,
{
    /// Chains an asynchronous recovery onto the rejection.
    ///
    /// Mirrors ECMAScript's `catch`: `recover` runs once this promise
    /// rejects, and its outcome settles the returned promise; a resolved
    /// value bypasses `recover` and converts into `TO` via `From`. The
    /// returned promise is scheduled via [`Promise::eager_or_lazy`].
    pub fn catch<TO, EO, F, Fut>(self, recover: F) -> Promise<TO, EO>
    where
        TO: From<T> + Send + 'static,
        EO: PromiseRejection,
        F: FnOnce(E) -> Fut + Send + 'static,
        Fut: Future<Output = Result<TO, EO>> + Send + 'static,
    {
        Promise::eager_or_lazy(async move {
            match self.await {
                Ok(value) => Ok(value.into()),
                Err(err) => recover(err).await,
            }
        })
    }
}
