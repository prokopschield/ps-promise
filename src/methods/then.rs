use std::future::Future;

use crate::{Promise, PromiseRejection};

impl<T, E> Promise<T, E>
where
    T: Send + Unpin + 'static,
    E: PromiseRejection,
{
    pub fn then<TO, EO, F, Fut>(self, f: F) -> Promise<TO, EO>
    where
        TO: Send + Unpin + 'static,
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
