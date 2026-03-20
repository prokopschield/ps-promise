use std::future::Future;

use crate::{Promise, PromiseRejection};

impl<T, E> Promise<T, E>
where
    T: Send + Unpin + 'static,
    E: PromiseRejection,
{
    pub fn map_err<EO, F, Fut>(self, f: F) -> Promise<T, EO>
    where
        EO: PromiseRejection + 'static,
        F: FnOnce(E) -> Fut + Send + 'static,
        Fut: Future<Output = EO> + Send + 'static,
    {
        Promise::new(async move {
            match self.await {
                Ok(value) => Ok(value),
                Err(err) => Err(f(err).await),
            }
        })
    }
}
