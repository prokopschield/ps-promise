mod implementations;
mod methods;

use std::{convert::Infallible, future::Future, pin::Pin};

pub type BoxedPromiseFuture<T, E> = Pin<Box<dyn Future<Output = Result<T, E>> + Send>>;

#[must_use = "Promises don't do anything unless you await them!"]
pub enum Promise<T = Infallible, E = Infallible>
where
    T: Unpin,
    E: Unpin,
{
    Pending(BoxedPromiseFuture<T, E>),
    Resolved(T),
    Rejected(PromiseRejection<E>),
    Consumed,
}

#[derive(thiserror::Error, Debug)]
pub enum PromiseRejection<E> {
    #[error(transparent)]
    Err(#[from] E),
    #[error("This happens when a Promise is consumed more than once.")]
    PromiseConsumedAlready,
}

pub type Result<T, E> = std::result::Result<T, PromiseRejection<E>>;
