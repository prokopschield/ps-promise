use tokio::task::JoinError;

use crate::TaskFailure;

impl From<JoinError> for TaskFailure {
    fn from(err: JoinError) -> Self {
        err.try_into_panic()
            .map_or_else(|err| Self::Error(Box::new(err)), Self::from)
    }
}
