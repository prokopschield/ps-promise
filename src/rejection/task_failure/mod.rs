mod implementations;

use std::error::Error;
use std::sync::Arc;

use thiserror::Error;

#[derive(Error)]
pub enum TaskFailure {
    #[error(transparent)]
    Error(Box<dyn Error + Send + 'static>),

    #[error("task panicked: {0}")]
    Panic(Arc<str>),
}
