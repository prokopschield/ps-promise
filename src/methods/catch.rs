use crate::{Promise, PromiseRejection, Transformer};

impl<T, E> Promise<T, E>
where
    T: Send + Unpin + Sync + 'static,
    E: Send + Unpin + Sync + 'static,
{
    pub fn catch<TO, EO>(self, transformer: Transformer<E, TO, EO>) -> Promise<TO, EO>
    where
        TO: From<T> + Unpin + 'static,
        EO: Unpin + 'static,
    {
        let future = async move {
            match self.await {
                Ok(value) => Ok(value.into()),
                Err(err) => match err {
                    PromiseRejection::Err(err) => match (transformer.transform)(err).await {
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
