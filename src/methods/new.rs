use std::future::Future;

use crate::Promise;

impl<T, E> Promise<T, E>
where
    T: Unpin,
    E: Unpin,
{
    pub fn new<F>(future: F) -> Self
    where
        F: Future<Output = Result<T, E>> + Send + Sync + 'static,
    {
        Self::Pending(Box::pin(async move {
            match future.await {
                Ok(value) => Ok(value),
                Err(err) => Err(crate::PromiseRejection::Err(err)),
            }
        }))
    }
}
