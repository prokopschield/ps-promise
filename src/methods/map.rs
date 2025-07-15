use crate::{Promise, PromiseRejection, Transformer};

impl<T, E> Promise<T, E>
where
    T: Send + Unpin + Sync + 'static,
    E: PromiseRejection,
{
    pub fn map<TO>(self, transformer: Transformer<T, TO, E>) -> Promise<TO, E>
    where
        TO: Unpin + 'static,
    {
        let future = async move {
            match self.await {
                Ok(value) => match (transformer.transform)(value).await {
                    Ok(value) => Ok(value),
                    Err(err) => Err(err),
                },
                Err(err) => Err(err),
            }
        };

        Promise::Pending(Box::pin(future))
    }
}
