use std::future::Future;

use crate::{Promise, PromiseRejection};

impl<T, E> Promise<T, E>
where
    T: Send + Unpin + Sync + 'static,
    E: PromiseRejection,
{
    pub fn catch<TO, EO, F, Fut>(self, recover: F) -> Promise<TO, EO>
    where
        TO: From<T> + Unpin + 'static,
        EO: PromiseRejection + 'static,
        F: FnOnce(E) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<TO, EO>> + Send + Sync + 'static,
    {
        let future = async move {
            match self.await {
                Ok(value) => Ok(value.into()),
                Err(err) => recover(err).await,
            }
        };

        Promise::Pending(Box::pin(future))
    }
}
