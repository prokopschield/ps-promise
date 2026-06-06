use std::any::Any;

use crate::TaskFailure;

impl From<Box<dyn Any + Send + 'static>> for TaskFailure {
    fn from(payload: Box<dyn Any + Send + 'static>) -> Self {
        Self::Panic(payload)
    }
}
