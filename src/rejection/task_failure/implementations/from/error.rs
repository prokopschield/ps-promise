use std::{error::Error, sync::Arc};

use crate::TaskFailure;

impl From<Box<dyn Error + Send + Sync + 'static>> for TaskFailure {
    fn from(err: Box<dyn Error + Send + Sync + 'static>) -> Self {
        Self::Error(Arc::from(err))
    }
}
