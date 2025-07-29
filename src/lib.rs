mod implementations;
mod methods;
mod transformer;

use thiserror::Error;
pub use transformer::{BoxedFuture, Transform, Transformer};

use std::{future::Future, pin::Pin};

pub type BoxedPromiseFuture<T, E> = Pin<Box<dyn Future<Output = Result<T, E>> + Send + Sync>>;

#[must_use = "Promises don't do anything unless you await them!"]
pub enum Promise<T, E>
where
    T: Unpin,
    E: PromiseRejection,
{
    Pending(BoxedPromiseFuture<T, E>),
    Resolved(T),
    Rejected(E),
    Consumed,
}

pub trait PromiseRejection
where
    Self: Send + Sync + Unpin + 'static,
{
    /// This method should return the error variant representing this [`Promise`] being consumed more than once.
    fn already_consumed() -> Self;
}

#[derive(Clone, Debug, Error, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum WrappedPromiseRejection<E>
where
    E: Send + Sync + Unpin + 'static,
{
    #[error("This Promise was consumed already.")]
    AlreadyConsumed,
    #[error(transparent)]
    Rejected(#[from] E),
}

impl<E> PromiseRejection for WrappedPromiseRejection<E>
where
    E: Send + Sync + Unpin + 'static,
{
    fn already_consumed() -> Self {
        Self::AlreadyConsumed
    }
}
