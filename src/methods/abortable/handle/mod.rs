mod methods;

use async_channel::Sender;

/// Aborts the [`Promise`](crate::Promise) created by
/// [`Promise::abortable`](crate::Promise::abortable).
///
/// Cloning the handle yields another trigger for the same promise; any clone
/// may abort, and the first settlement wins.
#[derive(Clone, Debug)]
pub struct AbortHandle {
    pub(super) sender: Sender<()>,
}
