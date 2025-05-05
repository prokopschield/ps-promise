use crate::{Promise, PromiseRejection};

impl<T, E> Promise<T, E>
where
    T: Send + Unpin + Sync + 'static,
    E: Send + Unpin + Sync + 'static,
{
    pub fn then<TO, EO, CB>(self, callback: CB) -> Promise<TO, EO>
    where
        TO: Unpin,
        EO: Unpin + From<E>,
        CB: Send + FnOnce(T) -> Result<TO, EO> + Send + Sync + 'static,
    {
        Promise::Pending(Box::pin(async move {
            match self.await {
                Ok(value) => match callback(value) {
                    Ok(value) => Ok(value),
                    Err(err) => Err(PromiseRejection::<EO>::Err(err)),
                },
                Err(err) => match err {
                    PromiseRejection::Err(err) => Err(PromiseRejection::<EO>::Err(err.into())),
                    PromiseRejection::PromisedConsumedAlready => {
                        Err(PromiseRejection::PromisedConsumedAlready)
                    }
                },
            }
        }))
    }
}
