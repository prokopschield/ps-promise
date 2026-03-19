use std::{
    future::Future,
    task::Poll::{Pending, Ready},
};

use crate::{Promise, PromiseRejection};

impl<T, E> Future for Promise<T, E>
where
    T: Unpin,
    E: PromiseRejection,
{
    type Output = Result<T, E>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = self.get_mut();

        this.poll(cx);

        this.consume().map_or_else(|| Pending, Ready)
    }
}
