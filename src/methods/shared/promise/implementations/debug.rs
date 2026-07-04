use std::fmt::{self, Debug, Formatter};

use crate::PromiseRejection;

use super::super::SharedPromise;

impl<T, E> Debug for SharedPromise<T, E>
where
    E: PromiseRejection,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("SharedPromise")
            .field("waiter_id", &self.waiter_id)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use crate::{Promise, PromiseRejection, TaskFailure};

    #[derive(Debug, Clone, PartialEq)]
    enum E {
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
    fn debug_names_the_type_and_waiter() {
        let shared = Promise::<i32, E>::resolve(7).shared();

        let rendered = format!("{shared:?}");

        assert!(rendered.starts_with("SharedPromise { waiter_id: "));
    }
}
