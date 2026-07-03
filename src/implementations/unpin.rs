use crate::{Promise, PromiseRejection};

/// [`Promise`] is unconditionally [`Unpin`]: the pending future is heap-pinned
/// behind [`Pin<Box<_>>`](std::pin::Pin), and no variant's payload is ever
/// pinned, so moving a [`Promise`] never moves pinned data.
impl<T, E> Unpin for Promise<T, E> where E: PromiseRejection {}

#[cfg(test)]
mod tests {
    use std::marker::PhantomPinned;

    use crate::{Promise, PromiseRejection, TaskFailure};

    #[test]
    fn accepts_a_not_unpin_value_type() {
        let mut promise: Promise<PhantomPinned, ()> = Promise::resolve(PhantomPinned);

        assert!(promise.consume().is_some_and(|result| result.is_ok()));
    }

    struct NotUnpinRejection(PhantomPinned);

    impl PromiseRejection for NotUnpinRejection {
        fn already_consumed() -> Self {
            Self(PhantomPinned)
        }

        fn task_failed(_: TaskFailure) -> Self {
            Self(PhantomPinned)
        }
    }

    #[test]
    fn accepts_a_not_unpin_rejection_type() {
        let mut promise: Promise<i32, NotUnpinRejection> =
            Promise::reject(NotUnpinRejection(PhantomPinned));

        assert!(promise.consume().is_some_and(|result| result.is_err()));
    }
}
