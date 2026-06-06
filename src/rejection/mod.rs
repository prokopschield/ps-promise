mod task_failure;
mod unit;
mod vec;
mod wrapped;

pub use task_failure::TaskFailure;
pub use wrapped::WrappedPromiseRejection;

pub trait PromiseRejection
where
    Self: Send + Unpin + 'static,
{
    /// Returns the error variant representing this [`Promise`](crate::Promise) being consumed more than once.
    fn already_consumed() -> Self;

    /// Returns the error variant representing the underlying task failing, e.g. by panicking or being cancelled by the runtime.
    fn task_failed() -> Self;
}
