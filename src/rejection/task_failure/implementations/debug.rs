use std::fmt::{self, Debug, Formatter};

use super::super::TaskFailure;

impl Debug for TaskFailure {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Aborted => f.write_str("Aborted"),
            Self::Error(err) => f.debug_tuple("Error").field(err).finish(),
            Self::Panic(message) => f.debug_tuple("Panic").field(message).finish(),
            Self::Timeout => f.write_str("Timeout"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TaskFailure;

    #[test]
    fn unit_variants_render_their_names() {
        assert_eq!(format!("{:?}", TaskFailure::Aborted), "Aborted");
        assert_eq!(format!("{:?}", TaskFailure::Timeout), "Timeout");
    }
}
