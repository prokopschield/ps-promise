use crate::{Promise, PromiseRejection};

impl<T, E> Promise<T, E>
where
    T: Send + Unpin + Sync + 'static,
    E: Send + Unpin + Sync + 'static,
{
    pub fn catch<TO, EO, CB>(self, callback: CB) -> Promise<TO, EO>
    where
        TO: From<T> + Unpin,
        EO: Unpin,
        CB: Send + FnOnce(E) -> Result<TO, EO> + Sync + 'static,
    {
        Promise::Pending(Box::pin(async move {
            match self.await {
                Ok(value) => Ok(value.into()),
                Err(err) => match err {
                    PromiseRejection::Err(err) => match callback(err) {
                        Ok(value) => Ok(value),
                        Err(err) => Err(PromiseRejection::<EO>::Err(err)),
                    },
                    PromiseRejection::PromisedConsumedAlready => {
                        Err(PromiseRejection::PromisedConsumedAlready)
                    }
                },
            }
        }))
    }
}
