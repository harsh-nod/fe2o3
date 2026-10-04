use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ValidationError {
    Empty { field: &'static str },
    TooMany { field: &'static str, max: usize },
    TextTooLong { field: &'static str, max: usize },
    InvalidText { field: &'static str },
    ZeroIdentity { field: &'static str },
    InvalidSourceLocation,
    Duplicate { field: &'static str },
    NonDenseBlockId { expected: u32, actual: u32 },
    InvalidEntryBlock { block: u32 },
    InvalidSuccessor { block: u32, successor: u32 },
    MissingKernel,
    EncodedUnitTooLarge { max: usize },
    Overflow { field: &'static str },
}

impl fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty { field } => write!(formatter, "{field} must not be empty"),
            Self::TooMany { field, max } => write!(formatter, "{field} exceeds the limit of {max}"),
            Self::TextTooLong { field, max } => {
                write!(formatter, "{field} exceeds the limit of {max} bytes")
            }
            Self::InvalidText { field } => write!(formatter, "{field} contains invalid text"),
            Self::ZeroIdentity { field } => write!(formatter, "{field} must not be all zero"),
            Self::InvalidSourceLocation => {
                write!(formatter, "source line and column must be one-based")
            }
            Self::Duplicate { field } => write!(formatter, "{field} contains a duplicate"),
            Self::NonDenseBlockId { expected, actual } => write!(
                formatter,
                "CFG block IDs must be dense: expected {expected}, found {actual}"
            ),
            Self::InvalidEntryBlock { block } => {
                write!(formatter, "entry block {block} does not exist")
            }
            Self::InvalidSuccessor { block, successor } => write!(
                formatter,
                "CFG block {block} references missing successor {successor}"
            ),
            Self::MissingKernel => write!(formatter, "frontend unit contains no kernel"),
            Self::EncodedUnitTooLarge { max } => {
                write!(formatter, "encoded frontend unit exceeds {max} bytes")
            }
            Self::Overflow { field } => write!(formatter, "{field} overflows its wire field"),
        }
    }
}

impl std::error::Error for ValidationError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodeError {
    TooLarge {
        max: usize,
    },
    Truncated,
    InvalidMagic,
    UnknownVersion(u16),
    UnsupportedFlags(u16),
    InvalidLength {
        declared: u32,
    },
    TrailingBytes,
    NonzeroReserved {
        field: &'static str,
    },
    CountOutOfRange {
        field: &'static str,
        count: u64,
        max: usize,
    },
    InvalidUtf8 {
        field: &'static str,
    },
    UnknownTag {
        kind: &'static str,
        tag: u16,
    },
    NonCanonical,
    Validation(ValidationError),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { max } => write!(formatter, "frontend unit exceeds {max} bytes"),
            Self::Truncated => write!(formatter, "frontend unit is truncated"),
            Self::InvalidMagic => write!(formatter, "frontend unit magic is invalid"),
            Self::UnknownVersion(version) => {
                write!(formatter, "unsupported frontend unit version {version}")
            }
            Self::UnsupportedFlags(flags) => {
                write!(formatter, "unsupported frontend unit flags {flags:#x}")
            }
            Self::InvalidLength { declared } => {
                write!(
                    formatter,
                    "frontend unit declares invalid length {declared}"
                )
            }
            Self::TrailingBytes => write!(formatter, "frontend unit contains trailing bytes"),
            Self::NonzeroReserved { field } => {
                write!(formatter, "{field} reserved field is nonzero")
            }
            Self::CountOutOfRange { field, count, max } => {
                write!(formatter, "{field} count {count} exceeds {max}")
            }
            Self::InvalidUtf8 { field } => write!(formatter, "{field} is not valid UTF-8"),
            Self::UnknownTag { kind, tag } => write!(formatter, "unknown {kind} tag {tag}"),
            Self::NonCanonical => write!(formatter, "frontend unit is not canonically encoded"),
            Self::Validation(error) => error.fmt(formatter),
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
