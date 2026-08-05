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
    EmptyCollection { field: &'static str },
    MissingCodeObject,
    MissingCapability(&'static str),
    PointerWidthMismatch,
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
            Self::EmptyCollection { field } => write!(f, "{field} must not be empty"),
            Self::MissingCodeObject => write!(f, "kernel references an unknown code object"),
            Self::MissingCapability(capability) => {
                write!(
                    f,
                    "target does not provide required capability {capability}"
                )
            }
            Self::PointerWidthMismatch => {
                write!(f, "ABI pointer width does not match the target")
            }
            Self::Overflow(field) => write!(f, "{field} overflows its representation"),
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
    UnknownTag {
        kind: &'static str,
        tag: u8,
    },
    UnknownCapability(u16),
    CountOutOfRange {
        field: &'static str,
        count: u64,
        min: usize,
        max: usize,
    },
    NonCanonicalOrder {
        field: &'static str,
    },
    TrailingBytes,
    Validation(ValidationError),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { max } => write!(f, "manifest exceeds {max} bytes"),
            Self::Truncated => write!(f, "manifest is truncated"),
            Self::InvalidMagic => write!(f, "manifest magic is invalid"),
            Self::UnknownVersion(version) => write!(f, "unsupported manifest version {version}"),
            Self::UnsupportedFlags(flags) => write!(f, "unsupported manifest flags {flags:#x}"),
            Self::UnknownTag { kind, tag } => write!(f, "unknown {kind} tag {tag}"),
            Self::UnknownCapability(tag) => write!(f, "unknown capability tag {tag}"),
            Self::CountOutOfRange {
                field,
                count,
                min,
                max,
            } => write!(f, "{field} count {count} is outside {min}..={max}"),
            Self::NonCanonicalOrder { field } => {
                write!(f, "{field} entries are not in canonical order")
            }
            Self::TrailingBytes => write!(f, "manifest contains trailing bytes"),
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
