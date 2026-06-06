use crate::PromiseRejection;

impl PromiseRejection for () {
    fn already_consumed() -> Self {}

    fn task_failed() -> Self {}
}
