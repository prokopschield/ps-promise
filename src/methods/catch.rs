use crate::{Promise, PromiseRejection, Transformer};

impl<T, E> Promise<T, E>
where
    T: Send + Unpin + Sync + 'static,
    E: PromiseRejection,
{
    pub fn catch<TO, EO>(self, transformer: Transformer<E, TO, EO>) -> Promise<TO, EO>
    where
        TO: From<T> + Unpin + 'static,
        EO: PromiseRejection + 'static,
    {
        let future = async move {
            match self.await {
                Ok(value) => Ok(value.into()),
                Err(err) => (transformer.transform)(err).await,
            }
        };

        Promise::Pending(Box::pin(future))
    }
}
