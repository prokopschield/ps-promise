use std::{
    future::Future,
    task::Poll::{Pending, Ready},
};

use crate::{gate::GatedPromise, Promise, PromiseRejection};

impl<T, E> Promise<T, E>
where
    T: Send + 'static,
    E: PromiseRejection,
{
    /// Resolves with the first value it observes, or rejects with every
    /// rejection once all promises reject.
    ///
    /// Mirrors ECMAScript's `Promise.any`, with `Vec<E>` in place of
    /// `AggregateError`: rejections appear in input order, and an empty
    /// input rejects immediately with an empty `Vec`. The returned
    /// [`Promise`] is lazy.
    pub fn any<I>(promises: I) -> Promise<T, Vec<E>>
    where
        I: IntoIterator<Item = Self>,
    {
        Promise::lazy(PromiseAny::from(promises))
    }
}

impl<T, E> Future for PromiseAny<T, E>
where
    T: Send + 'static,
    E: PromiseRejection,
{
    type Output = Result<T, Vec<E>>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = self.get_mut();

        let mut is_pending = false;

        for promise in &mut this.promises {
            promise.poll(cx);

            if promise.inner.is_pending() {
                is_pending = true;

                continue;
            }

            if promise.inner.is_resolved() {
                match promise.inner.consume() {
                    Some(Ok(val)) => return Ready(Ok(val)),
                    _ => unreachable!("The promise is resolved."),
                }
            }
        }

        if is_pending {
            return Pending;
        }

        let mut errors = Vec::new();

        for promise in &mut this.promises {
            match promise.inner.consume() {
                Some(Err(err)) => errors.push(err),
                Some(Ok(_)) => unreachable!("A resolved promise short-circuits above."),
                None => unreachable!("We checked no Promise is pending."),
            }
        }

        Ready(Err(errors))
    }
}

struct PromiseAny<T, E>
where
    T: Send + 'static,
    E: PromiseRejection,
{
    promises: Vec<GatedPromise<T, E>>,
}

impl<I, T, E> From<I> for PromiseAny<T, E>
where
    T: Send + 'static,
    E: PromiseRejection,
    I: IntoIterator<Item = Promise<T, E>>,
{
    fn from(value: I) -> Self {
        Self {
            promises: value.into_iter().map(GatedPromise::new).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        future::Future,
        pin::Pin,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
        task::{Context, Poll, Waker},
    };

    use crate::{Promise, PromiseRejection, TaskFailure};

    #[derive(thiserror::Error, Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
    enum E {
        #[error("Promise already consumed.")]
        AlreadyConsumed,
        #[error("Task failed")]
        Failure,
        #[error("Code: {0}")]
        Code(i32),
    }

    impl PromiseRejection for E {
        fn already_consumed() -> Self {
            Self::AlreadyConsumed
        }

        fn task_failed(_: TaskFailure) -> Self {
            Self::Failure
        }
    }

    fn cx() -> Context<'static> {
        Context::from_waker(Waker::noop())
    }

    #[test]
    fn empty() {
        let mut all: Promise<(), Vec<E>> = Promise::any([]);

        all.poll_settled(&mut cx());

        match all.consume() {
            Some(Err(v)) => assert!(v.is_empty(), "Result vector is not empty!"),
            other => panic!("Invalid state for empty input: {other:?}"),
        }
    }

    #[test]
    fn resolving() {
        let mut all = Promise::any([
            Promise::lazy(async { Err(E::Code(1)) }),
            Promise::lazy(async { Ok(2) }),
            Promise::lazy(async { Err(E::Code(3)) }),
        ]);

        all.poll_settled(&mut cx());

        match all.consume() {
            Some(Ok(v)) => assert_eq!(v, 2),
            other => panic!("Expected Resolved(2), got {other:?}"),
        }
    }

    #[test]
    fn resolution_short_circuits_pending() {
        let mut any: Promise<i32, Vec<E>> = Promise::any([
            Promise::lazy(std::future::pending()),
            Promise::lazy(async { Ok(2) }),
        ]);

        any.poll_settled(&mut cx());

        assert_eq!(any.consume(), Some(Ok(2)));
    }

    /// Inner future that counts its polls and never settles.
    struct CountPolls {
        polls: Arc<AtomicUsize>,
    }

    impl Future for CountPolls {
        type Output = Result<i32, E>;

        fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
            self.polls.fetch_add(1, Ordering::SeqCst);

            Poll::Pending
        }
    }

    #[test]
    fn children_without_a_wakeup_request_are_not_repolled() {
        let polls = Arc::new(AtomicUsize::new(0));

        let never: Promise<i32, E> = Promise::lazy(CountPolls {
            polls: polls.clone(),
        });
        let (pending, _resolve, reject) = Promise::<i32, E>::with_resolvers();

        let mut any = Promise::any([never, pending]);

        assert!(!any.poll_settled(&mut cx()));
        assert!(!any.poll_settled(&mut cx()));
        assert!(!any.poll_settled(&mut cx()));

        assert_eq!(polls.load(Ordering::SeqCst), 1);

        reject.reject(E::Code(1));

        assert!(!any.poll_settled(&mut cx()));

        assert_eq!(polls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn rejecting() {
        let mut all: Promise<(), Vec<E>> = Promise::any([
            Promise::lazy(async { Err(E::Code(1)) }),
            Promise::lazy(async { Err(E::Code(2)) }),
            Promise::lazy(async { Err(E::Code(3)) }),
        ]);

        all.poll_settled(&mut cx());

        match all.consume() {
            Some(Err(v)) => assert_eq!(v, [1, 2, 3].map(E::Code)),
            other => panic!("Expected Rejected([1,2,3]), got {other:?}"),
        }
    }
}
