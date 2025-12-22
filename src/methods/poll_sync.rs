use std::{
    ptr::null,
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

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
    pub fn poll_sync(self) -> Self {
        let Self::Pending(mut future) = self else {
            return self;
        };

        let waker = waker();
        let mut cx = Context::from_waker(&waker);

        match future.as_mut().poll(&mut cx) {
            Poll::Ready(Ok(value)) => Self::Resolved(value),
            Poll::Ready(Err(err)) => Self::Rejected(err),
            Poll::Pending => Self::Pending(future),
        }
    }
}

fn waker() -> Waker {
    unsafe { Waker::from_raw(raw_waker()) }
}

fn raw_waker() -> RawWaker {
    const unsafe fn wake(_: *const ()) {}
    const unsafe fn wake_by_ref(_: *const ()) {}
    const unsafe fn drop(_: *const ()) {}

    unsafe fn clone(_: *const ()) -> RawWaker {
        raw_waker()
    }

    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);

    RawWaker::new(null(), &VTABLE)
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
        let promise = Promise::new(async { Ok::<_, ()>(42) });

        match promise.poll_sync() {
            Promise::Resolved(v) => assert_eq!(v, 42),
            _ => panic!("expected Resolved"),
        }
    }

    #[test]
    fn eager_rejects_immediately_ready_future() {
        let promise: Promise<(), ()> = Promise::new(async { Err(()) });

        match promise.poll_sync() {
            Promise::Rejected(_) => {}
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

        let promise = Promise::new(Never);

        match promise.poll_sync() {
            Promise::Pending(_) => {}
            _ => panic!("expected Pending"),
        }
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
                    Poll::Pending
                } else {
                    Poll::Pending
                }
            }
        }

        let promise = Promise::new(CloneWaker { stored: None });

        match promise.poll_sync() {
            Promise::Pending(_) => {}
            _ => panic!("expected Pending"),
        }
    }
}
