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
    pub fn poll(&mut self, cx: &mut Context<'_>) {
        let Self::Pending(future) = self else {
            return;
        };

        match future.as_mut().poll(cx) {
            Poll::Ready(Ok(value)) => *self = Self::Resolved(value),
            Poll::Ready(Err(err)) => *self = Self::Rejected(err),
            Poll::Pending => {}
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
        let mut promise: Promise<i32, E> = Promise::new(async { Ok(42) });
        promise.poll(&mut cx());
        match promise {
            Promise::Resolved(v) => assert_eq!(v, 42),
            other => panic!("expected Resolved(42), got {other:?}"),
        }
    }

    #[test]
    fn rejects_ready_future() {
        let mut promise: Promise<(), E> = Promise::new(async { Err(E::Fail) });
        promise.poll(&mut cx());
        match promise {
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

        let mut promise: Promise<(), E> = Promise::new(Never);
        promise.poll(&mut cx());
        assert!(promise.is_pending());
    }

    #[test]
    fn resolved_is_identity() {
        let mut promise: Promise<i32, E> = Promise::resolve(99);
        promise.poll(&mut cx());
        match promise {
            Promise::Resolved(v) => assert_eq!(v, 99),
            other => panic!("expected Resolved(99), got {other:?}"),
        }
    }

    #[test]
    fn rejected_is_identity() {
        let mut promise: Promise<(), E> = Promise::reject(E::Fail);
        promise.poll(&mut cx());
        match promise {
            Promise::Rejected(E::Fail) => {}
            other => panic!("expected Rejected(Fail), got {other:?}"),
        }
    }

    #[test]
    fn consumed_is_identity() {
        let mut promise: Promise<(), E> = Promise::Consumed;
        promise.poll(&mut cx());
        assert!(promise.is_consumed());
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

        let mut promise: Promise<i32, E> = Promise::new(Delayed {
            ready: ready.clone(),
        });

        promise.poll(&mut cx());
        assert!(promise.is_pending());

        ready.store(true, Ordering::Relaxed);

        promise.poll(&mut cx());
        match promise {
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

        let mut promise: Promise<(), E> = Promise::new(Delayed {
            ready: ready.clone(),
        });

        promise.poll(&mut cx());
        assert!(promise.is_pending());

        ready.store(true, Ordering::Relaxed);

        promise.poll(&mut cx());
        match promise {
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

        let mut promise: Promise<(), E> = Promise::new(StoreWaker {
            woken: woken.clone(),
        });
        promise.poll(&mut cx());

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

        let mut promise: Promise<(), E> = Promise::new(Counter {
            count: count.clone(),
        });
        promise.poll(&mut cx());

        assert_eq!(count.load(Ordering::Relaxed), 1);
    }
}
