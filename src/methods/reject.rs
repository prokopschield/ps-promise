use crate::Promise;

impl<T, E> Promise<T, E> {
    pub const fn reject(err: E) -> Self {
        Self::Rejected(err)
    }
}
