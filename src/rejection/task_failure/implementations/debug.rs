use std::fmt::{self, Debug, Formatter};

use super::super::{panic_message, TaskFailure};

impl Debug for TaskFailure {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Error(err) => f.debug_tuple("Error").field(err).finish(),
            Self::Panic(payload) => f
                .debug_tuple("Panic")
                .field(&panic_message(&**payload))
                .finish(),
        }
    }
}
