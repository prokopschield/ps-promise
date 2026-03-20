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
    pub fn any<I>(promises: I) -> Promise<T, Vec<E>>
    where
        I: IntoIterator<Item = Self>,
    {
        Promise::new(PromiseAny::from(promises))
    }
}

impl<T, E> Future for PromiseAny<T, E>
where
    T: Send + Unpin + 'static,
    E: PromiseRejection,
{
    type Output = Result<T, Vec<E>>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = self.get_mut();

        let mut is_pending = false;

        this.promises.iter_mut().for_each(|promise| {
            if promise.pending(cx) {
                is_pending = true;
            }
        });

        if is_pending {
            return Pending;
        }

        let mut errors = Vec::new();

        for promise in &mut this.promises {
            match promise.consume() {
                Some(Ok(val)) => return Ready(Ok(val)),
                Some(Err(err)) => errors.push(err),
                None => unreachable!("We checked no Promise is pending."),
            }
        }

        Ready(Err(errors))
    }
}

pub struct PromiseAny<T, E>
where
    T: Send + Unpin + 'static,
    E: PromiseRejection,
{
    promises: Vec<Promise<T, E>>,
}

impl<I, T, E> From<I> for PromiseAny<T, E>
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
        let mut all: Promise<(), Vec<E>> = Promise::any([]);

        all.ready(&mut cx());

        if let Promise::Rejected(v) = all {
            assert!(v.is_empty(), "Result vector is not empty!");
        } else {
            panic!("Invalid state for empty input: {all:?}");
        }
    }

    #[test]
    fn resolving() {
        let mut all = Promise::any([
            Promise::new(async { Err(E::Code(1)) }),
            Promise::new(async { Ok(2) }),
            Promise::new(async { Err(E::Code(3)) }),
        ]);

        all.ready(&mut cx());

        if let Promise::Resolved(v) = all {
            assert_eq!(v, 2);
        } else {
            panic!("Expected Resolved(2), got {all:?}");
        }
    }

    #[test]
    fn rejecting() {
        let mut all: Promise<(), Vec<E>> = Promise::any([
            Promise::new(async { Err(E::Code(1)) }),
            Promise::new(async { Err(E::Code(2)) }),
            Promise::new(async { Err(E::Code(3)) }),
        ]);

        all.ready(&mut cx());

        if let Promise::Rejected(v) = all {
            assert_eq!(v, [1, 2, 3].map(E::Code));
        } else {
            panic!("Expected Rejected([1,2,3]), got {all:?}");
        }
    }
}
