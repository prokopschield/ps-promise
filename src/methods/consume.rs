use std::mem::replace;

use crate::{Promise, PromiseRejection};

impl<T, E> Promise<T, E>
where
    T: Unpin,
    E: PromiseRejection,
{
    /// If settled, consumes and returns the result.
    /// Returns `None` if still pending.
    pub fn consume(&mut self) -> Option<Result<T, E>> {
        match replace(self, Self::Consumed) {
            Self::Resolved(val) => Some(Ok(val)),
            Self::Rejected(err) => Some(Err(err)),
            Self::Consumed => Some(Err(E::already_consumed())),
            other @ Self::Pending(_) => {
                *self = other;
                None
            }
        }
    }
}
