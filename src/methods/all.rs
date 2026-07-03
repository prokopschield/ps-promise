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
            if promise.pending(cx) {
                is_pending = true;

                continue;
            }

            if promise.is_rejected() {
                match promise.consume() {
                    Some(Err(err)) => return Ready(Err(err)),
                    _ => unreachable!("The promise is rejected."),
                }
            }
        }

        if is_pending {
            return Pending;
        }

        // Phase 2: collect values
        let mut values = Vec::new();

        for promise in &mut this.promises {
            match promise.consume() {
                Some(Ok(val)) => values.push(val),
                Some(Err(err)) => return Ready(Err(err)),
                None => unreachable!("All promises are settled."),
            }
        }

        Ready(Ok(values))
    }
}

pub struct PromiseAll<T, E>
where
    T: Send + 'static,
    E: PromiseRejection,
{
    promises: Vec<Promise<T, E>>,
}

impl<I, T, E> From<I> for PromiseAll<T, E>
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
        all.ready(&mut cx());

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

        all.ready(&mut cx());

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

        all.ready(&mut cx());

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

        all.ready(&mut cx());
        assert_eq!(all.consume(), Some(Err(E::Code(1))));

        all.ready(&mut cx());
        assert_eq!(all.consume(), Some(Err(E::AlreadyConsumed)));
    }

    #[test]
    fn repoll_after_success_yields_already_consumed() {
        let mut all: Promise<Vec<i32>, E> = Promise::all([Promise::lazy(async { Ok(1) })]);

        all.ready(&mut cx());
        assert_eq!(all.consume(), Some(Ok(vec![1])));

        all.ready(&mut cx());
        assert_eq!(all.consume(), Some(Err(E::AlreadyConsumed)));
    }

    #[test]
    fn rejection_short_circuits_pending() {
        let mut all: Promise<Vec<i32>, E> = Promise::all([
            Promise::lazy(std::future::pending()),
            Promise::lazy(async { Err(E::Code(2)) }),
        ]);

        all.ready(&mut cx());

        assert_eq!(all.consume(), Some(Err(E::Code(2))));
    }

    #[test]
    fn all_rejected() {
        let mut all: Promise<Vec<i32>, E> = Promise::all([
            Promise::lazy(async { Err(E::Code(10)) }),
            Promise::lazy(async { Err(E::Code(20)) }),
        ]);

        all.ready(&mut cx());
        assert_eq!(all.consume(), Some(Err(E::Code(10))));

        all.ready(&mut cx());
        assert_eq!(all.consume(), Some(Err(E::AlreadyConsumed)));
    }
}
