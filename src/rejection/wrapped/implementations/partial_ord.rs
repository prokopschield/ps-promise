use std::cmp::Ordering;

use crate::WrappedPromiseRejection;

impl<E> PartialOrd for WrappedPromiseRejection<E>
where
    E: PartialOrd + Send + Unpin + 'static,
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (Self::AlreadyConsumed, Self::AlreadyConsumed) => Some(Ordering::Equal),
            (Self::Rejected(a), Self::Rejected(b)) => a.partial_cmp(b),
            _ => None,
        }
    }
}
