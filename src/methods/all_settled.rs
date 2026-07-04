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
    /// Waits for every promise to settle and collects the outcomes in order.
    ///
    /// Unlike [`Promise::all`], a rejected child does not reject the result;
    /// each outcome is reported as its own `Result<T, E>`. Mirrors
    /// ECMAScript's `Promise.allSettled`.
    ///
    /// The returned [`Promise`] is lazy; child promises progress only while
    /// it is polled, though eager children keep running on their own.
    pub fn all_settled<I>(promises: I) -> Promise<Vec<Result<T, E>>, E>
    where
        I: IntoIterator<Item = Self>,
    {
        Promise::lazy(PromiseAllSettled::from(promises))
    }
}

impl<T, E> Future for PromiseAllSettled<T, E>
where
    T: Send + 'static,
    E: PromiseRejection,
{
    type Output = Result<Vec<Result<T, E>>, E>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = self.get_mut();

        // Phase 1: drive all promises to completion.
        let mut is_pending = false;

        for promise in &mut this.promises {
            promise.poll(cx);

            if promise.inner.is_pending() {
                is_pending = true;
            }
        }

        if is_pending {
            return Pending;
        }

        // Phase 2: collect outcomes.
        let mut outcomes = Vec::with_capacity(this.promises.len());

        for promise in &mut this.promises {
            match promise.inner.consume() {
                Some(outcome) => outcomes.push(outcome),
                None => unreachable!("All promises are settled."),
            }
        }

        Ready(Ok(outcomes))
    }
}

pub struct PromiseAllSettled<T, E>
where
    T: Send + 'static,
    E: PromiseRejection,
{
    promises: Vec<GatedPromise<T, E>>,
}

impl<I, T, E> From<I> for PromiseAllSettled<T, E>
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
        let mut settled: Promise<Vec<Result<(), E>>, E> = Promise::all_settled([]);

        settled.poll_settled(&mut cx());

        match settled.consume() {
            Some(Ok(v)) => assert!(v.is_empty()),
            other => panic!("expected Resolved(vec![]), got {other:?}"),
        }
    }

    #[test]
    fn collects_mixed_outcomes_in_order() {
        let mut settled: Promise<Vec<Result<i32, E>>, E> = Promise::all_settled([
            Promise::lazy(async { Ok(1) }),
            Promise::lazy(async { Err(E::Code(2)) }),
            Promise::lazy(async { Ok(3) }),
        ]);

        settled.poll_settled(&mut cx());

        match settled.consume() {
            Some(Ok(v)) => assert_eq!(v, vec![Ok(1), Err(E::Code(2)), Ok(3)]),
            other => panic!("expected Resolved([Ok(1), Err(Code(2)), Ok(3)]), got {other:?}"),
        }
    }

    #[test]
    fn never_rejects_on_child_rejection() {
        let mut settled: Promise<Vec<Result<i32, E>>, E> = Promise::all_settled([
            Promise::lazy(async { Err(E::Code(1)) }),
            Promise::lazy(async { Err(E::Code(2)) }),
        ]);

        settled.poll_settled(&mut cx());

        match settled.consume() {
            Some(Ok(v)) => assert_eq!(v, vec![Err(E::Code(1)), Err(E::Code(2))]),
            other => panic!("expected Resolved([Err(Code(1)), Err(Code(2))]), got {other:?}"),
        }
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

        let mut settled = Promise::all_settled([never, pending]);

        assert!(!settled.poll_settled(&mut cx()));
        assert!(!settled.poll_settled(&mut cx()));
        assert!(!settled.poll_settled(&mut cx()));

        assert_eq!(polls.load(Ordering::SeqCst), 1);

        resolve.resolve(1);

        assert!(!settled.poll_settled(&mut cx()));

        assert_eq!(polls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn repoll_after_success_yields_already_consumed() {
        let mut settled: Promise<Vec<Result<i32, E>>, E> =
            Promise::all_settled([Promise::lazy(async { Ok(1) })]);

        settled.poll_settled(&mut cx());
        assert_eq!(settled.consume(), Some(Ok(vec![Ok(1)])));

        settled.poll_settled(&mut cx());
        assert_eq!(settled.consume(), Some(Err(E::AlreadyConsumed)));
    }
}
