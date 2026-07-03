use std::future::Future;

use crate::{Promise, PromiseRejection};

impl<T, E> Promise<T, E>
where
    T: Send + 'static,
    E: PromiseRejection,
{
    /// Transforms the resolved value with an asynchronous function.
    ///
    /// `f` runs once this promise resolves; a rejection passes through
    /// untouched. The returned promise is scheduled via
    /// [`Promise::eager_or_lazy`].
    pub fn map<TO, F, Fut>(self, f: F) -> Promise<TO, E>
    where
        TO: Send + 'static,
        F: FnOnce(T) -> Fut + Send + 'static,
        Fut: Future<Output = TO> + Send + 'static,
    {
        Promise::eager_or_lazy(async move {
            match self.await {
                Ok(value) => Ok(f(value).await),
                Err(err) => Err(err),
            }
        })
    }
}
