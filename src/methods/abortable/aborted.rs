/// Rejection payload produced when an [`AbortHandle`](crate::AbortHandle) from
/// [`Promise::abortable`](crate::Promise::abortable) fires.
#[derive(thiserror::Error, Debug, Clone, Copy, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[error("Promise was aborted.")]
pub struct Aborted;
