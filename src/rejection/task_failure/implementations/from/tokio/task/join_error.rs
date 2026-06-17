use std::sync::Arc;

use tokio::task::JoinError;

use crate::TaskFailure;

impl From<JoinError> for TaskFailure {
    fn from(err: JoinError) -> Self {
        err.try_into_panic()
            .map_or_else(|err| Self::Error(Arc::new(err)), Self::from)
    }
}
