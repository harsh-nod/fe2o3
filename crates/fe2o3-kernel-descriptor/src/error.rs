use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ValidationError {
    Empty { field: &'static str },
    TooLong { field: &'static str, max: usize },
    InvalidText { field: &'static str },
    TooMany { field: &'static str, max: usize },
    Duplicate { field: &'static str },
    NonCanonicalOrder { field: &'static str },
    InvalidValue { field: &'static str },
    InvalidArgument(&'static str),
    InvalidPhysicalAbi(&'static str),
    DanglingReference { field: &'static str },
    UnreachableRecord { field: &'static str },
    IdentityMismatch { field: &'static str },
    Overflow { field: &'static str },
    EncodedTableTooLarge { max: usize },
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty { field } => write!(f, "{field} must not be empty"),
            Self::TooLong { field, max } => write!(f, "{field} exceeds {max} bytes"),
            Self::InvalidText { field } => write!(f, "{field} contains invalid text"),
            Self::TooMany { field, max } => write!(f, "{field} exceeds {max} entries"),
            Self::Duplicate { field } => write!(f, "duplicate {field}"),
            Self::NonCanonicalOrder { field } => {
                write!(f, "{field} entries are not in canonical order")
            }
            Self::InvalidValue { field } => write!(f, "{field} is invalid"),
            Self::InvalidArgument(reason) => write!(f, "invalid logical argument: {reason}"),
            Self::InvalidPhysicalAbi(reason) => write!(f, "invalid physical ABI: {reason}"),
            Self::DanglingReference { field } => write!(f, "dangling {field} reference"),
            Self::UnreachableRecord { field } => write!(f, "unreachable {field} record"),
            Self::IdentityMismatch { field } => write!(f, "{field} identity does not match"),
            Self::Overflow { field } => write!(f, "{field} overflows its representation"),
            Self::EncodedTableTooLarge { max } => {
                write!(f, "encoded descriptor table exceeds {max} bytes")
            }
        }
    }
}

impl std::error::Error for ValidationError {}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum DecodeError {
    TooLarge {
        max: usize,
    },
    Truncated,
    InvalidMagic,
    UnknownVersion(u16),
    UnsupportedFlags(u16),
    TrailingBytes,
    InvalidText {
        field: &'static str,
    },
    UnknownTag {
        kind: &'static str,
        tag: u16,
    },
    NonzeroReserved {
        field: &'static str,
    },
    CountOutOfRange {
        field: &'static str,
        count: u64,
        max: usize,
    },
    NonCanonical,
    Validation(ValidationError),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { max } => write!(f, "descriptor table exceeds {max} bytes"),
            Self::Truncated => write!(f, "descriptor table is truncated"),
            Self::InvalidMagic => write!(f, "descriptor table magic is invalid"),
            Self::UnknownVersion(version) => {
                write!(f, "unsupported descriptor table version {version}")
            }
            Self::UnsupportedFlags(flags) => {
                write!(f, "unsupported descriptor table flags {flags:#x}")
            }
            Self::TrailingBytes => write!(f, "descriptor table contains trailing bytes"),
            Self::InvalidText { field } => write!(f, "{field} contains invalid text"),
            Self::UnknownTag { kind, tag } => write!(f, "unknown {kind} tag {tag}"),
            Self::NonzeroReserved { field } => write!(f, "{field} reserved field is nonzero"),
            Self::CountOutOfRange { field, count, max } => {
                write!(f, "{field} count {count} exceeds {max}")
            }
            Self::NonCanonical => write!(f, "descriptor table is not canonically encoded"),
            Self::Validation(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for DecodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Validation(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ValidationError> for DecodeError {
    fn from(value: ValidationError) -> Self {
        Self::Validation(value)
    }
}
