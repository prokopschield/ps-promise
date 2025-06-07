use crate::{Promise, PromiseRejection, Transformer};

impl<T, E> Promise<T, E>
where
    T: Send + Unpin + Sync + 'static,
    E: Send + Unpin + Sync + 'static,
{
    pub fn map_err<EO>(self, transformer: Transformer<E, EO, EO>) -> Promise<T, EO>
    where
        EO: Unpin + 'static,
    {
        let future = async move {
            match self.await {
                Ok(value) => Ok(value),
                Err(err) => match err {
                    PromiseRejection::Err(err) => match (transformer.transform)(err).await {
                        Ok(err) | Err(err) => Err(PromiseRejection::Err(err)),
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
