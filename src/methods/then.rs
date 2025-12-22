use std::future::Future;

use crate::{Promise, PromiseRejection};

impl<T, E> Promise<T, E>
where
    T: Send + Unpin + Sync + 'static,
    E: PromiseRejection,
{
    pub fn then<TO, EO, F, Fut>(self, f: F) -> Promise<TO, EO>
    where
        TO: Unpin + 'static,
        EO: PromiseRejection + From<E> + 'static,
        F: FnOnce(T) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<TO, EO>> + Send + Sync + 'static,
    {
        Promise::Pending(Box::pin(async move {
            match self.await {
                Ok(value) => f(value).await,
                Err(err) => Err(err.into()),
            }
        }))
    }
}
