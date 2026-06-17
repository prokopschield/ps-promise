use std::fmt::{self, Debug, Formatter};

use super::super::TaskFailure;

impl Debug for TaskFailure {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Error(err) => f.debug_tuple("Error").field(err).finish(),
            Self::Panic(message) => f.debug_tuple("Panic").field(message).finish(),
        }
    }
}
