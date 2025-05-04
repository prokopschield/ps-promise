use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use crate::Promise;

impl<T, E> Promise<T, E>
where
    T: Send + Unpin + 'static,
    E: Send + Unpin + 'static,
{
    pub fn ready(&mut self, cx: &mut Context<'_>) -> bool {
        let this = Pin::new(&mut *self);

        match Future::poll(this, cx) {
            Poll::Pending => false,
            Poll::Ready(Ok(value)) => {
                *self = Self::Resolved(value);
                true
            }
            Poll::Ready(Err(err)) => {
                *self = Self::Rejected(err);
                true
            }
        }
    }
}
