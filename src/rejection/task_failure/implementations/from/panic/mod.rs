mod helpers;

use std::any::Any;

use helpers::panic_message;

use crate::TaskFailure;

impl From<Box<dyn Any + Send + 'static>> for TaskFailure {
    fn from(payload: Box<dyn Any + Send + 'static>) -> Self {
        Self::Panic(panic_message(&*payload).into())
    }
}
