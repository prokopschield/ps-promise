use std::future::Future;

use crate::{Promise, PromiseRejection};

impl<T, E> Promise<T, E>
where
    T: Send + 'static,
    E: PromiseRejection,
{
    /// Chains an asynchronous transformation onto the resolved value.
    ///
    /// Mirrors ECMAScript's `then`: `f` runs once this promise resolves,
    /// and its outcome settles the returned promise; a rejection bypasses
    /// `f` and converts into `EO` via `From`. Since a [`Promise`] is itself
    /// a [`Future`], returning one from `f` chains it, and a nested
    /// `Promise<Promise<T, E>, E>` flattens via `.then(|p| p)`. The
    /// returned promise is scheduled via [`Promise::eager_or_lazy`].
    pub fn then<TO, EO, F, Fut>(self, f: F) -> Promise<TO, EO>
    where
        TO: Send + 'static,
        EO: PromiseRejection + From<E>,
        F: FnOnce(T) -> Fut + Send + 'static,
        Fut: Future<Output = Result<TO, EO>> + Send + 'static,
    {
        Promise::eager_or_lazy(async move {
            match self.await {
                Ok(value) => f(value).await,
                Err(err) => Err(err.into()),
            }
        })
    }
}
