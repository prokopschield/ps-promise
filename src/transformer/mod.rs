mod methods;

use std::{convert::Infallible, future::Future, pin::Pin, sync::Arc};

pub type BoxedFuture<O, E> = Pin<Box<dyn Future<Output = Result<O, E>> + Send + 'static>>;
pub type Transform<I, O, E> = Arc<dyn Fn(I) -> BoxedFuture<O, E>>;

#[derive(Clone)]
pub struct Transformer<I = Infallible, O = Infallible, E = Infallible> {
    pub transform: Transform<I, O, E>,
}
