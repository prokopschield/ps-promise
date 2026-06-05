mod features;
mod implementations;
mod methods;

use thiserror::Error;

use std::{future::Future, pin::Pin};

pub type BoxedPromiseFuture<T, E> = Pin<Box<dyn Future<Output = Result<T, E>> + Send>>;

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
    Self: Send + Unpin + 'static,
{
    /// Returns the error variant representing this [`Promise`] being consumed more than once.
    fn already_consumed() -> Self;

    /// Returns the error variant representing the underlying task failing, e.g. by panicking or being cancelled by the runtime.
    fn task_failed() -> Self;
}

#[derive(Clone, Debug, Error, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum WrappedPromiseRejection<E>
where
    E: Send + Unpin + 'static,
{
    #[error("This Promise was consumed already.")]
    AlreadyConsumed,
    #[error("The underlying task failed.")]
    TaskFailed,
    #[error(transparent)]
    Rejected(#[from] E),
}

impl<E> PromiseRejection for WrappedPromiseRejection<E>
where
    E: Send + Unpin + 'static,
{
    fn already_consumed() -> Self {
        Self::AlreadyConsumed
    }

    fn task_failed() -> Self {
        Self::TaskFailed
    }
}

impl<E> PromiseRejection for Vec<E>
where
    E: PromiseRejection,
{
    fn already_consumed() -> Self {
        Self::default()
    }

    fn task_failed() -> Self {
        Self::default()
    }
}

impl PromiseRejection for () {
    fn already_consumed() -> Self {}

    fn task_failed() -> Self {}
}
