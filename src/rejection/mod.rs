mod task_failure;
mod unit;
mod vec;
mod wrapped;

pub use task_failure::TaskFailure;
pub use wrapped::WrappedPromiseRejection;

/// A rejection type usable as the error of a [`Promise`](crate::Promise).
///
/// A promise must synthesize rejection values on its own in two cases: when
/// a consumed promise is consumed again
/// ([`already_consumed`](Self::already_consumed)), and when the underlying
/// task fails without producing a rejection, e.g. by panicking
/// ([`task_failed`](Self::task_failed)). This trait provides those
/// constructors, which is why every rejection type must implement it.
///
/// Escape hatches exist for error types that do not model these cases:
/// `()` discards all information, `Vec<E>` lifts rejections element-wise
/// (and is the rejection type of [`Promise::any`](crate::Promise::any)),
/// [`WrappedPromiseRejection`] wraps an arbitrary error type, and the
/// `anyhow` feature implements the trait for `anyhow::Error`.
pub trait PromiseRejection
where
    Self: Send + 'static,
{
    /// Returns the rejection value representing an already-consumed promise being consumed again.
    fn already_consumed() -> Self;

    /// Returns the rejection value representing the underlying task failing, e.g. by panicking or being cancelled by the runtime.
    fn task_failed(failure: TaskFailure) -> Self;
}
