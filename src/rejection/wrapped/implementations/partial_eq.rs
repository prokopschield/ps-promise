use crate::WrappedPromiseRejection;

impl<E> PartialEq for WrappedPromiseRejection<E>
where
    E: PartialEq + Send + Unpin + 'static,
{
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::AlreadyConsumed, Self::AlreadyConsumed) => true,
            (Self::Rejected(a), Self::Rejected(b)) => a == b,
            _ => false,
        }
    }
}
