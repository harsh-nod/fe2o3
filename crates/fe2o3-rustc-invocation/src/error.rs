use std::fmt;

/// Why a typed V1 invocation input was rejected.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ValidationError {
    /// A required value was empty.
    Empty {
        /// The rejected field.
        field: &'static str,
    },
    /// A value exceeded its byte limit.
    TooLong {
        /// The rejected field.
        field: &'static str,
        /// The maximum accepted byte length.
        max: usize,
    },
    /// A collection exceeded its entry limit.
    TooMany {
        /// The rejected collection.
        field: &'static str,
        /// The maximum accepted entry count.
        max: usize,
    },
    /// Text contained a forbidden NUL or another field-specific character.
    InvalidText {
        /// The rejected field.
        field: &'static str,
    },
    /// A path was not in the required canonical lexical form.
    InvalidPath {
        /// The rejected path field.
        field: &'static str,
    },
    /// A set-like collection was not strictly increasing.
    NonCanonicalOrder {
        /// The rejected collection.
        field: &'static str,
    },
    /// A set-like collection contained a duplicate key or value.
    Duplicate {
        /// The rejected collection.
        field: &'static str,
    },
    /// The AMD target ID was unknown, unsupported, or noncanonical.
    InvalidAmdTarget,
    /// The complete encoded descriptor exceeded its V1 bound.
    EncodedDescriptorTooLarge {
        /// The maximum accepted descriptor size.
        max: usize,
    },
    /// A length or count could not be represented by the wire format.
    Overflow {
        /// The field whose representation overflowed.
        field: &'static str,
    },
}

impl fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty { field } => write!(formatter, "{field} must not be empty"),
            Self::TooLong { field, max } => write!(formatter, "{field} exceeds {max} bytes"),
            Self::TooMany { field, max } => write!(formatter, "{field} exceeds {max} entries"),
            Self::InvalidText { field } => write!(formatter, "{field} contains invalid text"),
            Self::InvalidPath { field } => write!(formatter, "{field} is not a canonical path"),
            Self::NonCanonicalOrder { field } => {
                write!(formatter, "{field} entries are not in canonical order")
            }
            Self::Duplicate { field } => write!(formatter, "duplicate {field}"),
            Self::InvalidAmdTarget => {
                formatter.write_str("AMD target ID is unknown, unsupported, or noncanonical")
            }
            Self::EncodedDescriptorTooLarge { max } => {
                write!(
                    formatter,
                    "encoded invocation descriptor exceeds {max} bytes"
                )
            }
            Self::Overflow { field } => write!(formatter, "{field} overflows its representation"),
        }
    }
}

impl std::error::Error for ValidationError {}

/// Why canonical V1 descriptor bytes could not be decoded.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum DecodeError {
    /// The input exceeded the complete descriptor bound.
    TooLarge {
        /// The maximum accepted descriptor size.
        max: usize,
    },
    /// The input ended before a declared field was complete.
    Truncated,
    /// The fixed descriptor magic was not present.
    InvalidMagic,
    /// The descriptor version is not supported.
    UnknownVersion(u16),
    /// Header flags were nonzero.
    UnsupportedFlags(u16),
    /// A reserved field was nonzero.
    NonzeroReserved {
        /// The reserved field.
        field: &'static str,
    },
    /// The declared total length was smaller than the supplied byte slice or
    /// parsed records left bytes unconsumed.
    TrailingBytes,
    /// The declared total length was structurally impossible.
    InvalidLength {
        /// The declared byte length.
        declared: u32,
    },
    /// A length-prefixed string was not UTF-8.
    InvalidUtf8 {
        /// The rejected field.
        field: &'static str,
    },
    /// An enum or option used an unknown or reserved tag.
    UnknownTag {
        /// The kind of tagged value.
        kind: &'static str,
        /// The rejected numeric tag.
        tag: u16,
    },
    /// A declared collection count exceeded its V1 bound.
    CountOutOfRange {
        /// The rejected collection.
        field: &'static str,
        /// The declared count.
        count: u64,
        /// The maximum accepted count.
        max: usize,
    },
    /// Parsed data did not re-encode byte-for-byte.
    NonCanonical,
    /// A typed field or cross-field invariant was invalid.
    Validation(ValidationError),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { max } => write!(formatter, "descriptor exceeds {max} bytes"),
            Self::Truncated => formatter.write_str("descriptor is truncated"),
            Self::InvalidMagic => formatter.write_str("descriptor magic is invalid"),
            Self::UnknownVersion(version) => {
                write!(formatter, "unsupported descriptor version {version}")
            }
            Self::UnsupportedFlags(flags) => {
                write!(formatter, "unsupported descriptor flags {flags:#x}")
            }
            Self::NonzeroReserved { field } => {
                write!(formatter, "{field} reserved field is nonzero")
            }
            Self::TrailingBytes => formatter.write_str("descriptor contains trailing bytes"),
            Self::InvalidLength { declared } => {
                write!(formatter, "descriptor declares invalid length {declared}")
            }
            Self::InvalidUtf8 { field } => write!(formatter, "{field} is not UTF-8"),
            Self::UnknownTag { kind, tag } => write!(formatter, "unknown {kind} tag {tag}"),
            Self::CountOutOfRange { field, count, max } => {
                write!(formatter, "{field} count {count} exceeds {max}")
            }
            Self::NonCanonical => formatter.write_str("descriptor is not canonically encoded"),
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

/// Why an invocation digest could not be produced or accepted.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum DigestError {
    /// Encoding the typed descriptor failed.
    Encoding(ValidationError),
    /// The all-zero value is reserved by the artifact transaction protocol.
    ReservedAllZero,
    /// A hexadecimal digest did not contain exactly 64 ASCII characters.
    InvalidHexLength,
    /// A hexadecimal digest contained a non-lowercase-hex character.
    InvalidHexCharacter {
        /// The byte offset of the rejected character.
        index: usize,
    },
}

impl fmt::Display for DigestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Encoding(error) => error.fmt(formatter),
            Self::ReservedAllZero => {
                formatter.write_str("the all-zero invocation digest is reserved")
            }
            Self::InvalidHexLength => {
                formatter.write_str("invocation digest must contain exactly 64 hex characters")
            }
            Self::InvalidHexCharacter { index } => write!(
                formatter,
                "invocation digest contains invalid hex at byte {index}"
            ),
        }
    }
}

impl std::error::Error for DigestError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Encoding(error) => Some(error),
            Self::ReservedAllZero | Self::InvalidHexLength | Self::InvalidHexCharacter { .. } => {
                None
            }
        }
    }
}

impl From<ValidationError> for DigestError {
    fn from(value: ValidationError) -> Self {
        Self::Encoding(value)
    }
}
