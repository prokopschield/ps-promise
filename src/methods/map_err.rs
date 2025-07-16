use crate::{Promise, PromiseRejection, Transformer};

impl<T, E> Promise<T, E>
where
    T: Send + Unpin + Sync + 'static,
    E: PromiseRejection,
{
    pub fn map_err<EO>(self, transformer: Transformer<E, EO, EO>) -> Promise<T, EO>
    where
        EO: PromiseRejection + 'static,
    {
        let future = async move {
            match self.await {
                Ok(value) => Ok(value),
                Err(err) => match (transformer.transform)(err).await {
                    Ok(err) | Err(err) => Err(err),
                },
            }
        };

        Promise::Pending(Box::pin(future))
    }
}
