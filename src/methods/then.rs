use crate::{Promise, PromiseRejection, Transformer};

impl<T, E> Promise<T, E>
where
    T: Send + Unpin + Sync + 'static,
    E: PromiseRejection,
{
    pub fn then<TO, EO>(self, transformer: Transformer<T, TO, EO>) -> Promise<TO, EO>
    where
        TO: Unpin + 'static,
        EO: PromiseRejection + From<E> + 'static,
    {
        Promise::Pending(Box::pin(async move {
            match self.await {
                Ok(value) => (transformer.transform)(value).await,
                Err(err) => Err(err.into()),
            }
        }))
    }
}
