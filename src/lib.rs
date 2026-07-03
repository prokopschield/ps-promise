mod features;
mod implementations;
mod methods;
mod rejection;

use std::{future::Future, pin::Pin};

pub use methods::*;
pub use rejection::*;

pub type BoxedPromiseFuture<T, E> = Pin<Box<dyn Future<Output = Result<T, E>> + Send>>;

#[must_use = "Promises don't do anything unless you await them!"]
pub struct Promise<T, E> {
    pub(crate) state: State<T, E>,
}

pub(crate) enum State<T, E> {
    Pending(BoxedPromiseFuture<T, E>),
    Resolved(T),
    Rejected(E),
    Consumed,
}
