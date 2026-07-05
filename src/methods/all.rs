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
    /// Resolves with every value once all promises resolve, or rejects with
    /// the first rejection it observes.
    ///
    /// Mirrors ECMAScript's `Promise.all`: values appear in input order, and
    /// an empty input resolves immediately with an empty `Vec`. The returned
    /// [`Promise`] is lazy.
    pub fn all<I>(promises: I) -> Promise<Vec<T>, E>
    where
        I: IntoIterator<Item = Self>,
    {
        Promise::lazy(PromiseAll::from(promises))
    }
}

impl<T, E> Future for PromiseAll<T, E>
where
    T: Send + 'static,
    E: PromiseRejection,
{
    type Output = Result<Vec<T>, E>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = self.get_mut();

        // Phase 1: drive the promises, short-circuiting on the first rejection.
        let mut is_pending = false;

        for promise in &mut this.promises {
            promise.poll(cx);

            if promise.inner.is_pending() {
                is_pending = true;

                continue;
            }

            if !promise.inner.is_resolved() {
                let Some(Err(err)) = promise.inner.consume() else {
                    unreachable!("The promise is neither pending nor resolved.")
                };

                return Ready(Err(err));
            }
        }

        if is_pending {
            return Pending;
        }

        // Phase 2: collect values
        let mut values = Vec::new();

        for promise in &mut this.promises {
            match promise.inner.consume() {
                Some(Ok(val)) => values.push(val),
                Some(Err(err)) => return Ready(Err(err)),
                None => unreachable!("All promises are settled."),
            }
        }

        Ready(Ok(values))
    }
}

struct PromiseAll<T, E>
where
    T: Send + 'static,
    E: PromiseRejection,
{
    promises: Vec<GatedPromise<T, E>>,
}

impl<I, T, E> From<I> for PromiseAll<T, E>
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
        #[error("The underlying task failed.")]
        TaskFailed,
        #[error("Code: {0}")]
        Code(i32),
    }

    impl PromiseRejection for E {
        fn already_consumed() -> Self {
            Self::AlreadyConsumed
        }

        fn task_failed(_: TaskFailure) -> Self {
            Self::TaskFailed
        }
    }

    fn cx() -> Context<'static> {
        Context::from_waker(Waker::noop())
    }

    #[test]
    fn empty() {
        let mut all: Promise<Vec<()>, E> = Promise::all([]);
        all.poll_settled(&mut cx());

        match all.consume() {
            Some(Ok(v)) => assert!(v.is_empty()),
            other => panic!("expected Resolved(vec![]), got {other:?}"),
        }
    }

    #[test]
    fn all_resolve() {
        let mut all = Promise::all([
            Promise::lazy(async { Ok::<_, E>(1) }),
            Promise::lazy(async { Ok(2) }),
            Promise::lazy(async { Ok(3) }),
        ]);

        all.poll_settled(&mut cx());

        match all.consume() {
            Some(Ok(v)) => assert_eq!(v, vec![1, 2, 3]),
            other => panic!("expected Resolved([1,2,3]), got {other:?}"),
        }
    }

    #[test]
    fn single_rejection() {
        let mut all: Promise<Vec<i32>, E> = Promise::all([
            Promise::lazy(async { Ok(1) }),
            Promise::lazy(async { Err(E::Code(2)) }),
            Promise::lazy(async { Ok(3) }),
        ]);

        all.poll_settled(&mut cx());

        match all.consume() {
            Some(Err(E::Code(2))) => {}
            other => panic!("expected Rejected(Code(2)), got {other:?}"),
        }
    }

    #[test]
    fn returns_first_error() {
        let mut all: Promise<Vec<i32>, E> = Promise::all([
            Promise::lazy(async { Err(E::Code(1)) }),
            Promise::lazy(async { Ok(99) }),
            Promise::lazy(async { Err(E::Code(2)) }),
            Promise::lazy(async { Err(E::Code(3)) }),
        ]);

        all.poll_settled(&mut cx());
        assert_eq!(all.consume(), Some(Err(E::Code(1))));

        all.poll_settled(&mut cx());
        assert_eq!(all.consume(), Some(Err(E::AlreadyConsumed)));
    }

    #[test]
    fn repoll_after_success_yields_already_consumed() {
        let mut all: Promise<Vec<i32>, E> = Promise::all([Promise::lazy(async { Ok(1) })]);

        all.poll_settled(&mut cx());
        assert_eq!(all.consume(), Some(Ok(vec![1])));

        all.poll_settled(&mut cx());
        assert_eq!(all.consume(), Some(Err(E::AlreadyConsumed)));
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
        let (pending, resolve, _reject) = Promise::<i32, E>::with_resolvers();

        let mut all = Promise::all([never, pending]);

        assert!(!all.poll_settled(&mut cx()));
        assert!(!all.poll_settled(&mut cx()));
        assert!(!all.poll_settled(&mut cx()));

        assert_eq!(polls.load(Ordering::SeqCst), 1);

        resolve.resolve(1);

        assert!(!all.poll_settled(&mut cx()));

        assert_eq!(polls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn rejection_short_circuits_pending() {
        let mut all: Promise<Vec<i32>, E> = Promise::all([
            Promise::lazy(std::future::pending()),
            Promise::lazy(async { Err(E::Code(2)) }),
        ]);

        all.poll_settled(&mut cx());

        assert_eq!(all.consume(), Some(Err(E::Code(2))));
    }

    /// A child that was consumed before being handed to `all` rejects the
    /// result on the first poll, even while a sibling is still pending.
    #[test]
    fn consumed_input_short_circuits_pending() {
        let mut consumed: Promise<i32, E> = Promise::resolve(1);

        assert_eq!(consumed.consume(), Some(Ok(1)));

        let mut all: Promise<Vec<i32>, E> =
            Promise::all([Promise::lazy(std::future::pending()), consumed]);

        assert!(all.poll_settled(&mut cx()));
        assert_eq!(all.consume(), Some(Err(E::AlreadyConsumed)));
    }

    /// A child that failed rejects the result on the first poll, even while
    /// a sibling is still pending.
    #[test]
    fn failed_input_short_circuits_pending() {
        let mut failed: Promise<i32, E> = Promise::lazy(async { panic!("boom") });

        assert!(failed.poll_settled(&mut cx()));

        let mut all: Promise<Vec<i32>, E> =
            Promise::all([Promise::lazy(std::future::pending()), failed]);

        assert!(all.poll_settled(&mut cx()));
        assert_eq!(all.consume(), Some(Err(E::TaskFailed)));
    }

    #[test]
    fn all_rejected() {
        let mut all: Promise<Vec<i32>, E> = Promise::all([
            Promise::lazy(async { Err(E::Code(10)) }),
            Promise::lazy(async { Err(E::Code(20)) }),
        ]);

        all.poll_settled(&mut cx());
        assert_eq!(all.consume(), Some(Err(E::Code(10))));

        all.poll_settled(&mut cx());
        assert_eq!(all.consume(), Some(Err(E::AlreadyConsumed)));
    }
}
