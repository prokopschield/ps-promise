use crate::PromiseRejection;

impl<E> PromiseRejection for Vec<E>
where
    E: PromiseRejection,
{
    fn already_consumed() -> Self {
        Self::default()
    }

    fn task_failed() -> Self {
        Self::default()
    }
}
