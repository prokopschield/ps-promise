use std::{future::Future, pin::Pin, sync::Arc};

use crate::Transformer;

impl<I, O, E> Transformer<I, O, E>
where
    I: Send + Sync + 'static,
{
    pub fn from_infallible_async_box_fn<T>(transform_fn: T) -> Self
    where
        T: Fn(I) -> Pin<Box<dyn Future<Output = O> + Send + Sync + 'static>>
            + Send
            + Sync
            + 'static,
    {
        let transform_fn = Arc::new(transform_fn);

        Self {
            transform: Arc::new(move |input| {
                let transform_fn = transform_fn.clone();
                Box::pin(async move { Ok(transform_fn(input).await) })
            }),
        }
    }
}
