use crate::{Promise, PromiseRejection};

impl<T, E> From<Option<T>> for Promise<T, E>
where
    E: PromiseRejection + Default,
{
    /// Converts `Some(value)` into a promise resolved with `value`, and
    /// `None` into a promise rejected with `E::default()`.
    fn from(option: Option<T>) -> Self {
        option.map_or_else(|| Self::reject(E::default()), Self::resolve)
    }
}

#[cfg(test)]
mod tests {
    use crate::{Promise, PromiseRejection, TaskFailure};

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    enum E {
        #[default]
        Missing,
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
    fn some_becomes_resolved() {
        let mut promise: Promise<i32, E> = Promise::from(Some(42));

        assert!(promise.is_resolved());
        assert_eq!(promise.peek(), Some(Ok(&42)));
        assert_eq!(promise.consume(), Some(Ok(42)));
    }

    #[test]
    fn none_becomes_rejected_with_default() {
        let mut promise: Promise<i32, E> = Promise::from(None);

        assert!(promise.is_rejected());
        assert!(!promise.is_resolved());
        assert!(!promise.is_pending());
        assert_eq!(promise.peek(), Some(Err(&E::Missing)));
        assert_eq!(promise.consume(), Some(Err(E::Missing)));
    }
}
