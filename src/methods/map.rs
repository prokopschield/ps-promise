use std::future::Future;

use crate::{Promise, PromiseRejection};

impl<T, E> Promise<T, E>
where
    T: Send + Unpin + Sync + 'static,
    E: PromiseRejection,
{
    pub fn map<TO, F, Fut>(self, f: F) -> Promise<TO, E>
    where
        TO: Unpin + 'static,
        F: FnOnce(T) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<TO, E>> + Send + Sync + 'static,
    {
        Promise::Pending(Box::pin(async move {
            match self.await {
                Ok(value) => f(value).await,
                Err(err) => Err(err),
            }
        }))
    }
}
