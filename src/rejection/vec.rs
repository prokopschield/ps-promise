use crate::{PromiseRejection, TaskFailure};

impl<E> PromiseRejection for Vec<E>
where
    E: PromiseRejection,
{
    fn already_consumed() -> Self {
        vec![E::already_consumed()]
    }

    fn task_failed(failure: TaskFailure) -> Self {
        vec![E::task_failed(failure)]
    }
}
