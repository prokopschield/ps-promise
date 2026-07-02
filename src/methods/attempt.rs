use std::panic::{catch_unwind, AssertUnwindSafe};

use crate::{Promise, PromiseRejection, TaskFailure};

impl<T, E> Promise<T, E>
where
    T: Unpin,
    E: PromiseRejection,
{
    /// Calls a closure and captures its outcome in a settled [`Promise`].
    ///
    /// Mirrors ECMAScript's `Promise.try` (named after Bluebird's `attempt`
    /// alias, as `try` is a reserved keyword): the closure is invoked
    /// synchronously during this call, and the returned [`Promise`] is
    /// already settled. `Ok` resolves the promise, `Err` rejects it, and a
    /// panic is caught and rejects it with [`TaskFailure::Panic`] mapped
    /// through [`PromiseRejection::task_failed`].
    pub fn attempt<F>(f: F) -> Self
    where
        F: FnOnce() -> Result<T, E>,
    {
        match catch_unwind(AssertUnwindSafe(f)) {
            Ok(Ok(value)) => Self::Resolved(value),
            Ok(Err(err)) => Self::Rejected(err),
            Err(panic) => Self::Rejected(E::task_failed(TaskFailure::from(panic))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    enum E {
        AlreadyConsumed,
        Fail,
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
    fn ok_produces_resolved() {
        let promise: Promise<u32, E> = Promise::attempt(|| Ok(42));

        assert!(promise.is_resolved());

        match promise {
            Promise::Resolved(value) => assert_eq!(value, 42),
            other => panic!("expected Resolved(42), got {other:?}"),
        }
    }

    #[test]
    fn err_produces_rejected() {
        let promise: Promise<u32, E> = Promise::attempt(|| Err(E::Fail));

        assert!(promise.is_rejected());

        match promise {
            Promise::Rejected(E::Fail) => {}
            other => panic!("expected Rejected(Fail), got {other:?}"),
        }
    }

    #[test]
    fn panic_produces_task_failed_rejection() {
        let promise: Promise<u32, E> = Promise::attempt(|| panic!("boom"));

        match promise {
            Promise::Rejected(E::TaskFailed(failure @ TaskFailure::Panic(_))) => {
                assert_eq!(failure.to_string(), "task panicked: boom");
            }
            other => panic!("expected Rejected(TaskFailed(Panic(_))), got {other:?}"),
        }
    }

    #[test]
    fn closure_runs_synchronously() {
        let mut ran = false;

        let promise: Promise<(), E> = Promise::attempt(|| {
            ran = true;

            Ok(())
        });

        assert!(ran);
        assert!(promise.is_resolved());
    }
}
