use std::{
    future::Future,
    task::Poll::{Pending, Ready},
};

use crate::{Promise, PromiseRejection};

impl<T, E> Promise<T, E>
where
    T: Send + Unpin + 'static,
    E: PromiseRejection,
{
    pub fn all<I>(promises: I) -> Promise<Vec<T>, E>
    where
        I: IntoIterator<Item = Self>,
    {
        Promise::new(PromiseAll::from(promises))
    }
}

impl<T, E> Future for PromiseAll<T, E>
where
    T: Send + Unpin + 'static,
    E: PromiseRejection,
{
    type Output = Result<Vec<T>, E>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = self.get_mut();

        // Phase 1: drive all promises to completion.
        let mut is_pending = false;
        let mut rejection_idx = None;

        for (idx, promise) in this.promises.iter_mut().enumerate() {
            if promise.pending(cx) {
                is_pending = true;
            }

            if rejection_idx.is_none() && promise.is_rejected() {
                rejection_idx = Some(idx);
            }
        }

        if is_pending {
            return Pending;
        }

        if let Some(err) = rejection_idx
            .and_then(|idx| this.promises.get_mut(idx))
            .and_then(Promise::consume)
            .and_then(Result::err)
        {
            return Ready(Err(err));
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
    T: Send + Unpin + 'static,
    E: PromiseRejection,
{
    promises: Vec<Promise<T, E>>,
}

impl<I, T, E> From<I> for PromiseAll<T, E>
where
    T: Send + Unpin + 'static,
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

    use crate::{Promise, PromiseRejection};

    #[derive(thiserror::Error, Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
    enum E {
        #[error("Promise already consumed.")]
        AlreadyConsumed,
        #[error("Code: {0}")]
        Code(i32),
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
    fn empty() {
        let mut all: Promise<Vec<()>, E> = Promise::all([]);
        all.ready(&mut cx());

        match all {
            Promise::Resolved(v) => assert!(v.is_empty()),
            other => panic!("expected Resolved(vec![]), got {other:?}"),
        }
    }

    #[test]
    fn all_resolve() {
        let mut all = Promise::all([
            Promise::new(async { Ok::<_, E>(1) }),
            Promise::new(async { Ok(2) }),
            Promise::new(async { Ok(3) }),
        ]);

        all.ready(&mut cx());

        match all {
            Promise::Resolved(v) => assert_eq!(v, vec![1, 2, 3]),
            other => panic!("expected Resolved([1,2,3]), got {other:?}"),
        }
    }

    #[test]
    fn single_rejection() {
        let mut all: Promise<Vec<i32>, E> = Promise::all([
            Promise::new(async { Ok(1) }),
            Promise::new(async { Err(E::Code(2)) }),
            Promise::new(async { Ok(3) }),
        ]);

        all.ready(&mut cx());

        match all {
            Promise::Rejected(E::Code(2)) => {}
            other => panic!("expected Rejected(Code(2)), got {other:?}"),
        }
    }

    #[test]
    fn returns_first_error() {
        let mut all: Promise<Vec<i32>, E> = Promise::all([
            Promise::new(async { Err(E::Code(1)) }),
            Promise::new(async { Ok(99) }),
            Promise::new(async { Err(E::Code(2)) }),
            Promise::new(async { Err(E::Code(3)) }),
        ]);

        all.ready(&mut cx());
        assert_eq!(all.consume(), Some(Err(E::Code(1))));

        all.ready(&mut cx());
        assert_eq!(all.consume(), Some(Err(E::AlreadyConsumed)));
    }

    #[test]
    fn repoll_after_success_yields_already_consumed() {
        let mut all: Promise<Vec<i32>, E> = Promise::all([Promise::new(async { Ok(1) })]);

        all.ready(&mut cx());
        assert_eq!(all.consume(), Some(Ok(vec![1])));

        all.ready(&mut cx());
        assert_eq!(all.consume(), Some(Err(E::AlreadyConsumed)));
    }

    #[test]
    fn all_rejected() {
        let mut all: Promise<Vec<i32>, E> = Promise::all([
            Promise::new(async { Err(E::Code(10)) }),
            Promise::new(async { Err(E::Code(20)) }),
        ]);

        all.ready(&mut cx());
        assert_eq!(all.consume(), Some(Err(E::Code(10))));

        all.ready(&mut cx());
        assert_eq!(all.consume(), Some(Err(E::AlreadyConsumed)));
    }
}
