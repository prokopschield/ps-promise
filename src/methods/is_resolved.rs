use crate::{Promise, PromiseRejection};

impl<T, E> Promise<T, E>
where
    T: Unpin,
    E: PromiseRejection,
{
    pub const fn is_resolved(&self) -> bool {
        matches!(self, Self::Resolved(_))
    }
}
