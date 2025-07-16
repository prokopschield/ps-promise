use std::{
    future::Future,
    task::{Context, Poll},
};

use crate::{Promise, PromiseRejection};

impl<T, E> Promise<T, E>
where
    T: Send + Unpin + 'static,
    E: PromiseRejection + 'static,
{
    /// Polls the promise's inner future if pending.
    /// Returns `true` if the promise is now resolved or rejected.
    pub fn ready(&mut self, cx: &mut Context<'_>) -> bool {
        if let Self::Pending(fut) = self {
            // Just use fut.as_mut(), which is Pin<&mut ...>
            match Future::poll(fut.as_mut(), cx) {
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
        } else {
            // Already resolved, rejected, or consumed
            true
        }
    }
}
