use crate::Promise;

impl<T, E> Promise<T, E>
where
    T: Send + Unpin + Sync + 'static,
    E: Send + Unpin + Sync + 'static,
{
    pub fn map<TO, CB>(self, callback: CB) -> Promise<TO, E>
    where
        TO: Unpin,
        CB: Send + FnOnce(T) -> TO + Send + Sync + 'static,
    {
        let future = async move {
            match self.await {
                Ok(value) => Ok(callback(value)),
                Err(err) => Err(err),
            }
        };

        Promise::Pending(Box::pin(future))
    }
}
