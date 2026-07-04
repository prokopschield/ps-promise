use crate::{Promise, PromiseRejection};

impl<T, E> From<Result<T, E>> for Promise<T, E>
where
    E: PromiseRejection,
{
    /// Converts `Ok(value)` into a promise resolved with `value`, and
    /// `Err(err)` into a promise rejected with `err`.
    fn from(result: Result<T, E>) -> Self {
        result.map_or_else(Self::reject, Self::resolve)
    }
}

#[cfg(test)]
mod tests {
    use crate::{Promise, PromiseRejection, TaskFailure};

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum E {
        Fail,
        AlreadyConsumed,
        TaskFailed,
    }

    impl PromiseRejection for E {
        fn already_consumed() -> Self {
            Self::AlreadyConsumed
        }

        fn task_failed(_: TaskFailure) -> Self {
            Self::TaskFailed
        }
    }

    #[test]
    fn ok_becomes_resolved() {
        let mut promise: Promise<i32, E> = Promise::from(Ok(42));

        assert!(promise.is_resolved());
        assert_eq!(promise.peek(), Some(Ok(&42)));
        assert_eq!(promise.consume(), Some(Ok(42)));
    }

    #[test]
    fn err_becomes_rejected() {
        let mut promise: Promise<i32, E> = Promise::from(Err(E::Fail));

        assert!(promise.is_rejected());
        assert!(!promise.is_resolved());
        assert!(!promise.is_pending());
        assert_eq!(promise.peek(), Some(Err(&E::Fail)));
        assert_eq!(promise.consume(), Some(Err(E::Fail)));
    }
}
