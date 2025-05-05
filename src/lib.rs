mod implementations;
mod methods;

use std::{convert::Infallible, future::Future, pin::Pin};

#[must_use = "Promises don't do anything unless you await them!"]
pub enum Promise<T = Infallible, E = Infallible>
where
    T: Unpin,
    E: Unpin,
{
    Pending(Pin<Box<dyn Future<Output = Result<T, E>> + Send + Sync>>),
    Resolved(T),
    Rejected(PromiseRejection<E>),
    Consumed,
}

#[derive(thiserror::Error, Debug)]
pub enum PromiseRejection<E> {
    #[error(transparent)]
    Err(#[from] E),
    #[error("This happens then a Promise is consumed more than once.")]
    PromisedConsumedAlready,
}

pub type Result<T, E> = std::result::Result<T, PromiseRejection<E>>;
