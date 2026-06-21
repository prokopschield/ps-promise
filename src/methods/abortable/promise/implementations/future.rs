use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{
        Context,
        Poll::{self, Pending, Ready},
    },
};

use crate::{Aborted, Promise, PromiseRejection, TaskFailure};

use super::super::AbortablePromise;

impl<T, E> Future for AbortablePromise<T, E>
where
    T: Unpin,
    E: PromiseRejection,
{
    type Output = Result<T, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let aborted = || Ready(Err(E::task_failed(TaskFailure::Error(Arc::new(Aborted)))));
        let this = self.get_mut();

        this.receiver.poll(cx);

        if matches!(this.receiver, Promise::Resolved(())) {
            return aborted();
        }

        this.inner.poll(cx);
        this.receiver.poll(cx);

        if matches!(this.receiver, Promise::Resolved(())) {
            aborted()
        } else if let Some(result) = this.inner.consume() {
            Ready(result)
        } else {
            Pending
        }
    }
}
