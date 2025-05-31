use std::{future::Future, sync::Arc};

use crate::Transformer;

impl<I, O, E> Transformer<I, O, E>
where
    I: Send + 'static,
{
    pub fn from_async_fn<F, T>(transform_fn: T) -> Self
    where
        F: Future<Output = Result<O, E>> + Send + 'static,
        T: Fn(I) -> F + Send + Sync + 'static,
    {
        let transform_fn = Arc::new(transform_fn);

        Self {
            transform: Arc::new(move |input| {
                let transform_fn = transform_fn.clone();
                Box::pin(async move { transform_fn(input).await })
            }),
        }
    }
}
