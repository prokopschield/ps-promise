mod helpers;
mod implementations;

use std::any::Any;
use std::error::Error;

use helpers::panic_message;
use thiserror::Error;

#[derive(Error)]
pub enum TaskFailure {
    #[error(transparent)]
    Error(Box<dyn Error + Send + 'static>),

    #[error("task panicked: {}", panic_message(&**_0))]
    Panic(Box<dyn Any + Send + 'static>),
}
