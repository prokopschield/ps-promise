use std::{
    future::Future,
    task::Poll::{Pending, Ready},
};

use crate::{Promise, PromiseRejection};

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
    /// [`Promise`] is lazy, and each poll re-polls every still-pending
    /// input, so driving n independently-woken promises costs O(n²) child
    /// polls in total.
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
            if promise.pending(cx) {
                is_pending = true;

                continue;
            }

            if promise.is_resolved() {
                match promise.consume() {
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
            match promise.consume() {
                Some(Err(err)) => errors.push(err),
                Some(Ok(_)) => unreachable!("A resolved promise short-circuits above."),
                None => unreachable!("We checked no Promise is pending."),
            }
        }

        Ready(Err(errors))
    }
}

pub struct PromiseAny<T, E>
where
    T: Send + 'static,
    E: PromiseRejection,
{
    promises: Vec<Promise<T, E>>,
}

impl<I, T, E> From<I> for PromiseAny<T, E>
where
    T: Send + 'static,
    E: PromiseRejection,
    I: IntoIterator<Item = Promise<T, E>>,
{
    fn from(value: I) -> Self {
        Self {
            promises: value.into_iter().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::task::{Context, Waker};

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

        all.settle(&mut cx());

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

        all.settle(&mut cx());

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

        any.settle(&mut cx());

        assert_eq!(any.consume(), Some(Ok(2)));
    }

    #[test]
    fn rejecting() {
        let mut all: Promise<(), Vec<E>> = Promise::any([
            Promise::lazy(async { Err(E::Code(1)) }),
            Promise::lazy(async { Err(E::Code(2)) }),
            Promise::lazy(async { Err(E::Code(3)) }),
        ]);

        all.settle(&mut cx());

        match all.consume() {
            Some(Err(v)) => assert_eq!(v, [1, 2, 3].map(E::Code)),
            other => panic!("Expected Rejected([1,2,3]), got {other:?}"),
        }
    }
}
