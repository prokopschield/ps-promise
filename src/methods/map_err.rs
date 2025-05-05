use crate::{Promise, PromiseRejection};

impl<T, E> Promise<T, E>
where
    T: Send + Unpin + Sync + 'static,
    E: Send + Unpin + Sync + 'static,
{
    pub fn map_err<EO, CB>(self, callback: CB) -> Promise<T, EO>
    where
        EO: Unpin,
        CB: Send + FnOnce(E) -> EO + Send + Sync + 'static,
    {
        Promise::Pending(Box::pin(async move {
            match self.await {
                Ok(value) => Ok(value),
                Err(err) => match err {
                    PromiseRejection::Err(err) => Err(PromiseRejection::Err(callback(err))),
                    PromiseRejection::PromisedConsumedAlready => {
                        Err(PromiseRejection::PromisedConsumedAlready)
                    }
                },
            }
        }))
    }
}
