use crate::{Promise, PromiseRejection};

impl<T, E> Default for Promise<T, E>
where
    T: Default + Unpin,
    E: PromiseRejection,
{
    fn default() -> Self {
        Self::Resolved(T::default())
    }
}
