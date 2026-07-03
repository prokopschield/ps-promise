use std::future::Future;

use crate::{Promise, PromiseRejection};

impl<T, E> Promise<T, E>
where
    T: Send + 'static,
    E: PromiseRejection,
{
    /// Transforms the rejection with an asynchronous function.
    ///
    /// `f` runs once this promise rejects; a resolved value passes through
    /// untouched. The returned promise is scheduled via
    /// [`Promise::eager_or_lazy`].
    pub fn map_err<EO, F, Fut>(self, f: F) -> Promise<T, EO>
    where
        EO: PromiseRejection,
        F: FnOnce(E) -> Fut + Send + 'static,
        Fut: Future<Output = EO> + Send + 'static,
    {
        Promise::eager_or_lazy(async move {
            match self.await {
                Ok(value) => Ok(value),
                Err(err) => Err(f(err).await),
            }
        })
    }
}
