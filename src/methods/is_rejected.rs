use crate::{Promise, PromiseRejection};

impl<T, E> Promise<T, E>
where
    E: PromiseRejection,
{
    pub const fn is_rejected(&self) -> bool {
        matches!(self, Self::Rejected(_))
    }
}
