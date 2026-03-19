use crate::{Promise, PromiseRejection};

impl<T, E> Promise<T, E>
where
    T: Unpin,
    E: PromiseRejection,
{
    pub const fn is_pending(&self) -> bool {
        matches!(self, Self::Pending(_))
    }
}
