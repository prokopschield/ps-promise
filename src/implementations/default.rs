use crate::Promise;

impl<T, E> Default for Promise<T, E>
where
    T: Default,
{
    fn default() -> Self {
        Self::Resolved(T::default())
    }
}
