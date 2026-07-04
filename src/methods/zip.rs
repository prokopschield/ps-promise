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
    /// Joins this [`Promise`] with another of a possibly different value
    /// type, resolving with both values as a tuple.
    ///
    /// Both promises are polled concurrently. Like [`Promise::all`], the
    /// promise resolves once both children have resolved; if either rejects,
    /// the zipped promise rejects immediately, without waiting for the other
    /// child. When both reject in the same poll, the left rejection takes
    /// precedence.
    ///
    /// Chain calls for higher arity: `a.zip(b).zip(c)` resolves with
    /// `((A, B), C)`.
    pub fn zip<U>(self, other: Promise<U, E>) -> Promise<(T, U), E>
    where
        U: Send + 'static,
    {
        Promise::lazy(PromiseZip {
            left: self,
            right: other,
        })
    }
}

struct PromiseZip<T, U, E>
where
    T: Send + 'static,
    U: Send + 'static,
    E: PromiseRejection,
{
    left: Promise<T, E>,
    right: Promise<U, E>,
}

impl<T, U, E> Future for PromiseZip<T, U, E>
where
    T: Send + 'static,
    U: Send + 'static,
    E: PromiseRejection,
{
    type Output = Result<(T, U), E>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = self.get_mut();

        let left_pending = this.left.pending(cx);
        let right_pending = this.right.pending(cx);

        if this.left.is_rejected() {
            match this.left.consume() {
                Some(Err(err)) => return Ready(Err(err)),
                _ => unreachable!("The promise is rejected."),
            }
        }

        if this.right.is_rejected() {
            match this.right.consume() {
                Some(Err(err)) => return Ready(Err(err)),
                _ => unreachable!("The promise is rejected."),
            }
        }

        if left_pending || right_pending {
            return Pending;
        }

        let left = match this.left.consume() {
            Some(Ok(value)) => value,
            Some(Err(err)) => return Ready(Err(err)),
            None => unreachable!("Both promises are settled."),
        };

        let right = match this.right.consume() {
            Some(Ok(value)) => value,
            Some(Err(err)) => return Ready(Err(err)),
            None => unreachable!("Both promises are settled."),
        };

        Ready(Ok((left, right)))
    }
}

#[cfg(test)]
mod tests {
    use std::task::{Context, Waker};

    use crate::{Promise, PromiseRejection, TaskFailure};

    #[derive(thiserror::Error, Debug, Clone, Copy, PartialEq, Eq)]
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
    fn resolves_with_heterogeneous_tuple() {
        let left: Promise<i32, E> = Promise::lazy(async { Ok(1) });
        let right: Promise<&str, E> = Promise::lazy(async { Ok("a") });

        let mut zipped = left.zip(right);

        zipped.settle(&mut cx());

        assert_eq!(zipped.consume(), Some(Ok((1, "a"))));
    }

    #[test]
    fn left_rejection_takes_precedence() {
        let left: Promise<i32, E> = Promise::lazy(async { Err(E::Code(1)) });
        let right: Promise<&str, E> = Promise::lazy(async { Err(E::Code(2)) });

        let mut zipped = left.zip(right);

        zipped.settle(&mut cx());

        assert_eq!(zipped.consume(), Some(Err(E::Code(1))));
    }

    #[test]
    fn right_rejection_rejects() {
        let left: Promise<i32, E> = Promise::lazy(async { Ok(1) });
        let right: Promise<&str, E> = Promise::lazy(async { Err(E::Code(2)) });

        let mut zipped = left.zip(right);

        zipped.settle(&mut cx());

        assert_eq!(zipped.consume(), Some(Err(E::Code(2))));
    }

    #[test]
    fn chains_for_higher_arity() {
        let a: Promise<i32, E> = Promise::lazy(async { Ok(1) });
        let b: Promise<&str, E> = Promise::lazy(async { Ok("b") });
        let c: Promise<bool, E> = Promise::lazy(async { Ok(true) });

        let mut zipped = a.zip(b).zip(c);

        zipped.settle(&mut cx());

        assert_eq!(zipped.consume(), Some(Ok(((1, "b"), true))));
    }

    #[test]
    fn rejection_short_circuits_pending() {
        let left: Promise<i32, E> = Promise::lazy(std::future::pending());
        let right: Promise<&str, E> = Promise::lazy(async { Err(E::Code(2)) });

        let mut zipped = left.zip(right);

        zipped.settle(&mut cx());

        assert_eq!(zipped.consume(), Some(Err(E::Code(2))));
    }

    #[test]
    fn repoll_after_success_yields_already_consumed() {
        let left: Promise<i32, E> = Promise::lazy(async { Ok(1) });
        let right: Promise<i32, E> = Promise::lazy(async { Ok(2) });

        let mut zipped = left.zip(right);

        zipped.settle(&mut cx());
        assert_eq!(zipped.consume(), Some(Ok((1, 2))));

        zipped.settle(&mut cx());
        assert_eq!(zipped.consume(), Some(Err(E::AlreadyConsumed)));
    }
}
