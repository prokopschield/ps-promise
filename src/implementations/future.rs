use std::{future::Future, mem::replace};

use crate::{Promise, PromiseRejection, Result};

impl<T, E> Future for Promise<T, E>
where
    T: Unpin,
    E: Unpin,
{
    type Output = Result<T, E>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = self.get_mut();

        match replace(this, Self::Consumed) {
            Self::Pending(mut future) => match future.as_mut().poll(cx) {
                std::task::Poll::Pending => {
                    *this = Self::Pending(future);
                    std::task::Poll::Pending
                }
                std::task::Poll::Ready(value) => std::task::Poll::Ready(value),
            },
            Self::Resolved(value) => std::task::Poll::Ready(Ok(value)),
            Self::Rejected(err) => std::task::Poll::Ready(Err(err)),
            Self::Consumed => std::task::Poll::Ready(Err(PromiseRejection::PromiseConsumedAlready)),
        }
    }
}
