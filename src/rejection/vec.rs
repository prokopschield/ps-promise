use crate::PromiseRejection;

impl<E> PromiseRejection for Vec<E>
where
    E: PromiseRejection,
{
    fn already_consumed() -> Self {
        vec![E::already_consumed()]
    }

    fn task_failed() -> Self {
        vec![E::task_failed()]
    }
}
