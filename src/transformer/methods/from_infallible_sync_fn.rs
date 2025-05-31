use std::sync::Arc;

use crate::Transformer;

impl<I, O, E> Transformer<I, O, E>
where
    I: Send + Sync + 'static,
{
    pub fn from_infallible_sync_fn<T>(transform_fn: T) -> Self
    where
        T: Fn(I) -> O + Send + Sync + 'static,
    {
        let transform_fn = Arc::new(transform_fn);

        Self {
            transform: Arc::new(move |input| {
                let transform_fn = transform_fn.clone();
                Box::pin(async move { Ok(transform_fn(input)) })
            }),
        }
    }
}
