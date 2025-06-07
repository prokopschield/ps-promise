mod implementations;
mod methods;

use std::{convert::Infallible, future::Future, pin::Pin, sync::Arc};

pub type BoxedFuture<O, E> = Pin<Box<dyn Future<Output = Result<O, E>> + Send + Sync + 'static>>;
pub type Transform<I, O, E> = Arc<dyn Fn(I) -> BoxedFuture<O, E> + Send + Sync>;

pub struct Transformer<I = Infallible, O = Infallible, E = Infallible> {
    pub transform: Transform<I, O, E>,
}
