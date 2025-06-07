use crate::{Promise, PromiseRejection, Transformer};

impl<T, E> Promise<T, E>
where
    T: Send + Unpin + Sync + 'static,
    E: Send + Unpin + Sync + 'static,
{
    pub fn then<TO, EO>(self, transformer: Transformer<T, TO, EO>) -> Promise<TO, EO>
    where
        TO: Unpin + 'static,
        EO: Unpin + From<E> + 'static,
    {
        Promise::Pending(Box::pin(async move {
            match self.await {
                Ok(value) => match (transformer.transform)(value).await {
                    Ok(value) => Ok(value),
                    Err(err) => Err(PromiseRejection::<EO>::Err(err)),
                },
                Err(err) => match err {
                    PromiseRejection::Err(err) => Err(PromiseRejection::<EO>::Err(err.into())),
                    PromiseRejection::PromiseConsumedAlready => {
                        Err(PromiseRejection::PromiseConsumedAlready)
                    }
                },
            }
        }))
    }
}
