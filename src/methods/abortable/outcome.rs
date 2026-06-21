/// Best-effort success hint from [`AbortHandle::abort`](crate::AbortHandle::abort).
///
/// An abort request appears to be live, so a still-pending promise is expected
/// to reject with [`Aborted`](crate::Aborted) on its next poll. Like all of
/// [`AbortHandle::abort`](crate::AbortHandle::abort)'s output, this is advisory
/// rather than authoritative under concurrent settlement.
#[derive(Debug, Clone, Copy, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct PromiseAborted;

/// Best-effort error hint from [`AbortHandle::abort`](crate::AbortHandle::abort).
///
/// The promise appears to be no longer abortable, having settled on its own or
/// through an earlier abort, or been dropped. Like all of
/// [`AbortHandle::abort`](crate::AbortHandle::abort)'s output, this is advisory
/// rather than authoritative under concurrent settlement.
#[derive(Debug, Clone, Copy, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct PromiseSettled;
