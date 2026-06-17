mod implementations;

use std::error::Error;
use std::sync::Arc;

use thiserror::Error;

#[derive(Error)]
pub enum TaskFailure {
    #[error(transparent)]
    Error(Arc<dyn Error + Send + Sync + 'static>),

    #[error("task panicked: {0}")]
    Panic(Arc<str>),
}
