use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ValidationError {
    Empty { field: &'static str },
    TooLong { field: &'static str, max: usize },
    InvalidText { field: &'static str },
    Duplicate { field: &'static str },
    InvalidRank(u8),
    InvalidDimension { field: &'static str },
    InvalidAlignment { field: &'static str, value: u32 },
    InvalidLayout(&'static str),
    InvalidAccess(&'static str),
    TooMany { field: &'static str, max: usize },
    Overflow(&'static str),
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty { field } => write!(f, "{field} must not be empty"),
            Self::TooLong { field, max } => write!(f, "{field} exceeds {max} bytes"),
            Self::InvalidText { field } => write!(f, "{field} contains invalid text"),
            Self::Duplicate { field } => write!(f, "duplicate {field}"),
            Self::InvalidRank(rank) => write!(f, "launch rank {rank} is not in 1..=3"),
            Self::InvalidDimension { field } => write!(f, "invalid {field} dimensions"),
            Self::InvalidAlignment { field, value } => {
                write!(f, "{field} alignment {value} is invalid")
            }
            Self::InvalidLayout(reason) => write!(f, "invalid ABI layout: {reason}"),
            Self::InvalidAccess(reason) => write!(f, "invalid ABI access: {reason}"),
            Self::TooMany { field, max } => write!(f, "{field} exceeds {max} entries"),
            Self::Overflow(field) => write!(f, "{field} overflows its representation"),
        }
    }
}

impl std::error::Error for ValidationError {}
