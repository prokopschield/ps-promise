use std::error::Error;

use crate::TaskFailure;

impl From<Box<dyn Error + Send + 'static>> for TaskFailure {
    fn from(err: Box<dyn Error + Send + 'static>) -> Self {
        Self::Error(err)
    }
}
