use std::{
    future::Future,
    pin::Pin,
    task::{
        Context,
        Poll::{self, Pending, Ready},
    },
};

use crate::{PromiseRejection, TaskFailure};

use super::super::AbortablePromise;

impl<T, E> Future for AbortablePromise<T, E>
where
    E: PromiseRejection,
{
    type Output = Result<T, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let aborted = || Ready(Err(E::task_failed(TaskFailure::Aborted)));
        let this = self.get_mut();

        this.receiver.poll(cx);

        if this.receiver.is_resolved() {
            return aborted();
        }

        this.inner.poll(cx);
        this.receiver.poll(cx);

        if this.receiver.is_resolved() {
            aborted()
        } else if let Some(result) = this.inner.consume() {
            Ready(result)
        } else {
            Pending
        }
    }
}
