use crate::{PromiseRejection, TaskFailure};

impl PromiseRejection for () {
    fn already_consumed() -> Self {}

    fn task_failed(_: TaskFailure) -> Self {}
}
