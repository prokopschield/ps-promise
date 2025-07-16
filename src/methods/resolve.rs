use crate::{Promise, PromiseRejection};

impl<T, E> Promise<T, E>
where
    T: Unpin,
    E: PromiseRejection,
{
    pub const fn resolve(value: T) -> Self {
        Self::Resolved(value)
    }
}
