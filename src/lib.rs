mod features;
mod implementations;
mod methods;
mod rejection;

use std::{future::Future, pin::Pin};

pub use methods::{Reject, Resolve, ResolversDropped};
pub use rejection::{PromiseRejection, TaskFailure, WrappedPromiseRejection};

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
