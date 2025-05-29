use crate::Promise;

impl<T, E> Promise<T, E>
where
    T: Unpin,
    E: Unpin,
{
    pub fn resolve(value: T) -> Self {
        Self::Resolved(value)
    }
}
