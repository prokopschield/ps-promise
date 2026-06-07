use std::future::Future;

use crate::{Promise, PromiseRejection};

impl<T, E> Promise<T, E>
where
    T: Send + Unpin + 'static,
    E: PromiseRejection,
{
    pub fn map<TO, F, Fut>(self, f: F) -> Promise<TO, E>
    where
        TO: Send + Unpin + 'static,
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
