use std::task::{Context, Waker};

use crate::{Promise, PromiseRejection};

impl<T, E> Promise<T, E>
where
    T: Unpin,
    E: PromiseRejection,
{
    /// Attempts to advance this [`Promise`] immediately on the current thread.
    ///
    /// This performs exactly one poll of the underlying future using a no-op waker.
    ///
    /// Wakes triggered during this synchronous poll are ignored.
    /// Execution will effectively resume only when the [`Promise`] is polled again by a real executor.
    pub fn poll_sync(&mut self) {
        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);

        self.poll(&mut cx);
    }
}

#[cfg(test)]
mod tests {
    use std::{
        future::Future,
        pin::Pin,
        task::{Context, Poll},
    };

    use crate::Promise;

    #[test]
    fn eager_resolves_immediately_ready_future() {
        let mut promise = Promise::new(async { Ok::<_, ()>(42) });
        promise.poll_sync();

        match promise {
            Promise::Resolved(v) => assert_eq!(v, 42),
            _ => panic!("expected Resolved"),
        }
    }

    #[test]
    fn eager_rejects_immediately_ready_future() {
        let mut promise: Promise<(), ()> = Promise::new(async { Err(()) });
        promise.poll_sync();

        match promise {
            Promise::Rejected(()) => {}
            _ => panic!("expected Rejected"),
        }
    }

    #[test]
    fn eager_stops_on_pending_without_wake() {
        struct Never;

        impl Future for Never {
            type Output = Result<(), ()>;

            fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
                Poll::Pending
            }
        }

        let mut promise = Promise::new(Never);
        promise.poll_sync();

        assert!(promise.is_pending());
    }

    #[test]
    fn eager_handles_waker_cloning() {
        struct CloneWaker {
            stored: Option<std::task::Waker>,
        }

        impl Future for CloneWaker {
            type Output = Result<(), ()>;

            fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                if self.stored.is_none() {
                    self.stored = Some(cx.waker().clone());
                }

                Poll::Pending
            }
        }

        let mut promise = Promise::new(CloneWaker { stored: None });
        promise.poll_sync();

        assert!(promise.is_pending());
    }
}
