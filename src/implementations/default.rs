use crate::{Promise, PromiseRejection};

impl<T, E> Default for Promise<T, E>
where
    T: Default,
    E: PromiseRejection,
{
    fn default() -> Self {
        Self::resolve(T::default())
    }
}
