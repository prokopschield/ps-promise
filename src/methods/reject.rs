use crate::{Promise, PromiseRejection};

impl<T, E> Promise<T, E>
where
    E: PromiseRejection,
{
    pub const fn reject(err: E) -> Self {
        Self::Rejected(err)
    }
}
