use crate::{Promise, State, TaskFailure};

impl<T, E> From<TaskFailure> for Promise<T, E> {
    /// Converts a [`TaskFailure`] into a failed promise.
    ///
    /// The failure is stored as is and mapped through
    /// [`PromiseRejection::task_failed`](crate::PromiseRejection::task_failed)
    /// on every consumption.
    fn from(failure: TaskFailure) -> Self {
        Self {
            state: State::Failed(failure),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{Promise, PromiseRejection, TaskFailure};

    #[derive(Debug)]
    enum E {
        AlreadyConsumed,
        TaskFailed(TaskFailure),
    }

    impl PromiseRejection for E {
        fn already_consumed() -> Self {
            Self::AlreadyConsumed
        }

        fn task_failed(failure: TaskFailure) -> Self {
            Self::TaskFailed(failure)
        }
    }

    #[test]
    fn failure_becomes_failed() {
        let promise: Promise<i32, E> = Promise::from(TaskFailure::Panic("boom".into()));

        assert!(promise.is_failed());
        assert!(promise.is_rejected());
        assert!(!promise.is_pending());
        assert!(!promise.is_resolved());
    }

    #[test]
    fn consumption_maps_through_task_failed() {
        let mut promise: Promise<i32, E> = Promise::from(TaskFailure::Panic("boom".into()));

        match promise.consume() {
            Some(Err(E::TaskFailed(failure @ TaskFailure::Panic(_)))) => {
                assert_eq!(failure.to_string(), "task panicked: boom");
            }
            other => panic!("expected Err(TaskFailed(Panic(_))), got {other:?}"),
        }
    }

    #[test]
    fn failure_is_repeat_consumable() {
        let mut promise: Promise<i32, E> = Promise::from(TaskFailure::Panic("boom".into()));

        assert!(matches!(
            promise.consume(),
            Some(Err(E::TaskFailed(TaskFailure::Panic(_))))
        ));
        assert!(matches!(
            promise.consume(),
            Some(Err(E::TaskFailed(TaskFailure::Panic(_))))
        ));
    }
}
