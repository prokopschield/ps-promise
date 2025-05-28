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
        let future = async move {
            match self.await {
                Ok(value) => Ok(value.into()),
                Err(err) => match err {
                    PromiseRejection::Err(err) => match callback(err) {
                        Ok(value) => Ok(value),
                        Err(err) => Err(PromiseRejection::<EO>::Err(err)),
                    },
                    PromiseRejection::PromiseConsumedAlready => {
                        Err(PromiseRejection::PromiseConsumedAlready)
                    }
                },
            }
        };

        Promise::Pending(Box::pin(future))
    }
}
