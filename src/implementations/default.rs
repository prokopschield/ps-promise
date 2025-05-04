use crate::Promise;

impl<T, E> Default for Promise<T, E>
where
    T: Default + Unpin,
    E: Unpin,
{
    fn default() -> Self {
        Self::Resolved(T::default())
    }
}
