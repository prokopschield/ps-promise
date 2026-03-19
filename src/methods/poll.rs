use std::task::{Context, Poll};

use crate::{Promise, PromiseRejection};

impl<T, E> Promise<T, E>
where
    T: Unpin,
    E: PromiseRejection,
{
    /// Attempts to advance this [`Promise`] using the provided execution [`Context`].
    ///
    /// This performs exactly one poll of the underlying future.
    pub fn poll(self, cx: &mut Context<'_>) -> Self {
        let Self::Pending(mut future) = self else {
            return self;
        };

        match future.as_mut().poll(cx) {
            Poll::Ready(Ok(value)) => Self::Resolved(value),
            Poll::Ready(Err(err)) => Self::Rejected(err),
            Poll::Pending => Self::Pending(future),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        future::Future,
        pin::Pin,
        sync::{
            atomic::{AtomicBool, AtomicUsize, Ordering},
            Arc,
        },
        task::{Context, Poll, Waker},
    };

    use crate::{Promise, PromiseRejection};

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum E {
        AlreadyConsumed,
        Fail,
    }

    impl PromiseRejection for E {
        fn already_consumed() -> Self {
            Self::AlreadyConsumed
        }
    }

    fn cx() -> Context<'static> {
        Context::from_waker(Waker::noop())
    }

    #[test]
    fn resolves_ready_future() {
        let promise: Promise<i32, E> = Promise::new(async { Ok(42) });
        match promise.poll(&mut cx()) {
            Promise::Resolved(v) => assert_eq!(v, 42),
            other => panic!("expected Resolved(42), got {other:?}"),
        }
    }

    #[test]
    fn rejects_ready_future() {
        let promise: Promise<(), E> = Promise::new(async { Err(E::Fail) });
        match promise.poll(&mut cx()) {
            Promise::Rejected(E::Fail) => {}
            other => panic!("expected Rejected(Fail), got {other:?}"),
        }
    }

    #[test]
    fn stays_pending() {
        struct Never;

        impl Future for Never {
            type Output = Result<(), E>;
            fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
                Poll::Pending
            }
        }

        let promise: Promise<(), E> = Promise::new(Never);
        match promise.poll(&mut cx()) {
            Promise::Pending(_) => {}
            other => panic!("expected Pending, got {other:?}"),
        }
    }

    #[test]
    fn resolved_is_identity() {
        let promise: Promise<i32, E> = Promise::resolve(99);
        match promise.poll(&mut cx()) {
            Promise::Resolved(v) => assert_eq!(v, 99),
            other => panic!("expected Resolved(99), got {other:?}"),
        }
    }

    #[test]
    fn rejected_is_identity() {
        let promise: Promise<(), E> = Promise::reject(E::Fail);
        match promise.poll(&mut cx()) {
            Promise::Rejected(E::Fail) => {}
            other => panic!("expected Rejected(Fail), got {other:?}"),
        }
    }

    #[test]
    fn consumed_is_identity() {
        let promise: Promise<(), E> = Promise::Consumed;
        match promise.poll(&mut cx()) {
            Promise::Consumed => {}
            other => panic!("expected Consumed, got {other:?}"),
        }
    }

    #[test]
    fn pending_then_resolves() {
        struct Delayed {
            ready: Arc<AtomicBool>,
        }

        impl Future for Delayed {
            type Output = Result<i32, E>;
            fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
                if self.ready.load(Ordering::Relaxed) {
                    Poll::Ready(Ok(7))
                } else {
                    Poll::Pending
                }
            }
        }

        let ready = Arc::new(AtomicBool::new(false));
        let ready2 = ready.clone();

        let promise: Promise<i32, E> = Promise::new(Delayed { ready: ready2 });

        // First poll: still pending
        let promise = match promise.poll(&mut cx()) {
            Promise::Pending(f) => Promise::Pending(f),
            other => panic!("expected Pending, got {other:?}"),
        };

        ready.store(true, Ordering::Relaxed);

        // Second poll: now resolves
        match promise.poll(&mut cx()) {
            Promise::Resolved(v) => assert_eq!(v, 7),
            other => panic!("expected Resolved(7), got {other:?}"),
        }
    }

    #[test]
    fn pending_then_rejects() {
        struct Delayed {
            ready: Arc<AtomicBool>,
        }

        impl Future for Delayed {
            type Output = Result<(), E>;
            fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
                if self.ready.load(Ordering::Relaxed) {
                    Poll::Ready(Err(E::Fail))
                } else {
                    Poll::Pending
                }
            }
        }

        let ready = Arc::new(AtomicBool::new(false));
        let ready2 = ready.clone();

        let promise: Promise<(), E> = Promise::new(Delayed { ready: ready2 });

        let promise = match promise.poll(&mut cx()) {
            Promise::Pending(f) => Promise::Pending(f),
            other => panic!("expected Pending, got {other:?}"),
        };

        ready.store(true, Ordering::Relaxed);

        match promise.poll(&mut cx()) {
            Promise::Rejected(E::Fail) => {}
            other => panic!("expected Rejected(Fail), got {other:?}"),
        }
    }

    #[test]
    fn waker_is_forwarded() {
        struct StoreWaker {
            woken: Arc<AtomicBool>,
        }

        impl Future for StoreWaker {
            type Output = Result<(), E>;
            fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                let waker = cx.waker().clone();
                let flag = self.woken.clone();
                waker.wake();
                flag.store(true, Ordering::Relaxed);
                Poll::Pending
            }
        }

        let woken = Arc::new(AtomicBool::new(false));
        let woken2 = woken.clone();

        let promise: Promise<(), E> = Promise::new(StoreWaker { woken: woken2 });
        drop(promise.poll(&mut cx()));

        assert!(
            woken.load(Ordering::Relaxed),
            "waker should have been invoked"
        );
    }

    #[test]
    fn polls_exactly_once() {
        struct Counter {
            count: Arc<AtomicUsize>,
        }

        impl Future for Counter {
            type Output = Result<(), E>;
            fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
                self.count.fetch_add(1, Ordering::Relaxed);
                Poll::Pending
            }
        }

        let count = Arc::new(AtomicUsize::new(0));
        let count2 = count.clone();

        let promise: Promise<(), E> = Promise::new(Counter { count: count2 });
        drop(promise.poll(&mut cx()));

        assert_eq!(count.load(Ordering::Relaxed), 1);
    }
}
