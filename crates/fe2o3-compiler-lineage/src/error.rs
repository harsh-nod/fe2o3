use std::fmt;

/// Failure to construct canonical V3 inert semantic-lineage content.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum LineageErrorV3 {
    /// A required canonical preimage was empty.
    EmptyPreimage {
        /// Stable field name.
        field: &'static str,
    },
    /// A canonical preimage exceeded its field bound.
    PreimageTooLarge {
        /// Stable field name.
        field: &'static str,
        /// Maximum accepted bytes.
        max: usize,
    },
    /// A derived identity hit the reserved all-zero value.
    ZeroIdentity {
        /// Stable identity name.
        field: &'static str,
    },
    /// The canonical rustc invocation could not be encoded.
    Invocation(fe2o3_rustc_invocation::ValidationError),
    /// The declared target differs from the canonical rustc invocation target.
    TargetMismatch,
    /// An encoded length could not be represented or summed.
    LengthOverflow,
    /// The complete capsule exceeded its aggregate bound.
    CapsuleTooLarge {
        /// Maximum accepted bytes.
        max: usize,
    },
}

impl fmt::Display for LineageErrorV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPreimage { field } => {
                write!(formatter, "{field} preimage must not be empty")
            }
            Self::PreimageTooLarge { field, max } => {
                write!(formatter, "{field} preimage exceeds {max} bytes")
            }
            Self::ZeroIdentity { field } => write!(formatter, "{field} identity must be nonzero"),
            Self::Invocation(error) => write!(formatter, "invalid rustc invocation: {error}"),
            Self::TargetMismatch => {
                formatter.write_str("device target differs from the rustc invocation target")
            }
            Self::LengthOverflow => formatter.write_str("lineage length overflows its encoding"),
            Self::CapsuleTooLarge { max } => {
                write!(formatter, "semantic capsule exceeds {max} bytes")
            }
        }
    }
}

impl std::error::Error for LineageErrorV3 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Invocation(error) => Some(error),
            _ => None,
        }
    }
}

/// Failure to strictly decode canonical V3 inert semantic-lineage content.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum LineageDecodeErrorV3 {
    /// The complete input exceeded the aggregate bound.
    TooLarge {
        /// Maximum accepted bytes.
        max: usize,
    },
    /// The input ended before a declared field was complete.
    Truncated,
    /// The V3 capsule magic was absent.
    InvalidMagic,
    /// The declared version is unsupported.
    UnsupportedVersion(u16),
    /// Header flags were nonzero.
    UnsupportedFlags(u16),
    /// A reserved header field was nonzero.
    NonzeroReserved,
    /// The declared total length was structurally impossible.
    InvalidLength(u64),
    /// The input contained bytes beyond its declared complete capsule.
    TrailingBytes,
    /// The embedded rustc invocation was invalid or noncanonical.
    Invocation(fe2o3_rustc_invocation::DecodeError),
    /// The embedded rustc invocation digest did not match its exact bytes.
    InvocationDigestMismatch,
    /// The target spelling was not UTF-8.
    InvalidTargetText,
    /// The target spelling was unknown or invalid.
    InvalidTarget,
    /// Parsed data had a noncanonical wire representation.
    NonCanonical,
    /// The target differs from the embedded rustc invocation target.
    TargetMismatch,
    /// A required canonical preimage was empty.
    EmptyPreimage {
        /// Stable field name.
        field: &'static str,
    },
    /// A declared field length exceeded its bound.
    PreimageTooLarge {
        /// Stable field name.
        field: &'static str,
        /// Maximum accepted bytes.
        max: usize,
    },
    /// An encoded identity used the reserved all-zero value.
    ZeroIdentity {
        /// Stable identity name.
        field: &'static str,
    },
    /// A receipt identity did not match its exact preimage.
    ReceiptIdentityMismatch {
        /// Stable receipt name.
        field: &'static str,
    },
    /// The terminal capsule identity did not match the complete preceding bytes.
    CapsuleIdentityMismatch,
}

impl fmt::Display for LineageDecodeErrorV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { max } => write!(formatter, "semantic capsule exceeds {max} bytes"),
            Self::Truncated => formatter.write_str("semantic capsule is truncated"),
            Self::InvalidMagic => formatter.write_str("semantic capsule magic is invalid"),
            Self::UnsupportedVersion(version) => {
                write!(formatter, "unsupported semantic capsule version {version}")
            }
            Self::UnsupportedFlags(flags) => {
                write!(formatter, "unsupported semantic capsule flags {flags:#x}")
            }
            Self::NonzeroReserved => {
                formatter.write_str("semantic capsule reserved field is nonzero")
            }
            Self::InvalidLength(length) => {
                write!(
                    formatter,
                    "semantic capsule declares invalid length {length}"
                )
            }
            Self::TrailingBytes => formatter.write_str("semantic capsule contains trailing bytes"),
            Self::Invocation(error) => write!(formatter, "invalid rustc invocation: {error}"),
            Self::InvocationDigestMismatch => {
                formatter.write_str("rustc invocation digest does not match its exact bytes")
            }
            Self::InvalidTargetText => formatter.write_str("device target is not UTF-8"),
            Self::InvalidTarget => formatter.write_str("device target is invalid"),
            Self::NonCanonical => formatter.write_str("semantic capsule is not canonical"),
            Self::TargetMismatch => {
                formatter.write_str("device target differs from the rustc invocation target")
            }
            Self::EmptyPreimage { field } => {
                write!(formatter, "{field} preimage must not be empty")
            }
            Self::PreimageTooLarge { field, max } => {
                write!(formatter, "{field} preimage exceeds {max} bytes")
            }
            Self::ZeroIdentity { field } => write!(formatter, "{field} identity must be nonzero"),
            Self::ReceiptIdentityMismatch { field } => {
                write!(
                    formatter,
                    "{field} identity does not match its exact preimage"
                )
            }
            Self::CapsuleIdentityMismatch => {
                formatter.write_str("capsule identity does not match its exact preimage")
            }
        }
    }
}

impl std::error::Error for LineageDecodeErrorV3 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Invocation(error) => Some(error),
            _ => None,
        }
    }
}
