use crate::Promise;

impl<T, E> Promise<T, E>
where
    T: Unpin,
    E: Unpin,
{
    pub const fn reject(err: E) -> Self {
        Self::Rejected(crate::PromiseRejection::Err(err))
    }
}
