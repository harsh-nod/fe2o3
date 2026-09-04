use std::error::Error;

use super::*;

impl fmt::Display for WorkerV3VerificationProtocolErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroIdentity(field) => write!(formatter, "{field:?} identity is zero"),
            Self::ZeroEntryIdentity { ordinal, field } => {
                write!(formatter, "roster entry {ordinal} {field} identity is zero")
            }
            Self::InvalidEntryName {
                ordinal,
                field,
                source,
            } => write!(
                formatter,
                "roster entry {ordinal} {field:?} is not a canonical kernel name: {source}"
            ),
            Self::EntryCountOutOfRange { actual, maximum } => write!(
                formatter,
                "roster entry count {actual} is outside 1..={maximum}"
            ),
            Self::UnexpectedEntryOrdinal { expected, actual } => write!(
                formatter,
                "roster entry ordinal {actual} occurs at canonical position {expected}"
            ),
            Self::DuplicateEntryIdentity {
                field,
                first_ordinal,
                duplicate_ordinal,
            } => write!(
                formatter,
                "roster entry {duplicate_ordinal} duplicates {field:?} identity from entry {first_ordinal}"
            ),
            Self::UnexpectedPayloadKind { expected, actual } => {
                write!(
                    formatter,
                    "expected {expected:?} fd payload, got {actual:?}"
                )
            }
            Self::InvalidFdOrdinal { kind, actual } => {
                write!(
                    formatter,
                    "{kind:?} payload used invalid fd ordinal {actual}"
                )
            }
            Self::PayloadLengthOutOfRange {
                kind,
                actual,
                maximum,
            } => write!(
                formatter,
                "{kind:?} payload length {actual} is outside 1..={maximum} bytes"
            ),
            Self::ZeroPayloadDigest { kind } => {
                write!(formatter, "{kind:?} payload digest is zero")
            }
            Self::RequestLengthOutOfRange {
                actual,
                minimum,
                maximum,
            } => write!(
                formatter,
                "request length {actual} is outside {minimum}..={maximum} bytes"
            ),
            Self::InvalidResponseLength { actual } => write!(
                formatter,
                "response length must be {WORKER_V3_VERIFICATION_RESPONSE_BYTES_V1} bytes, got {actual}"
            ),
            Self::BadRequestMagic => {
                formatter.write_str("Worker V3 verification request magic mismatch")
            }
            Self::BadResponseMagic => {
                formatter.write_str("Worker V3 verification response magic mismatch")
            }
            Self::UnsupportedVersion { actual } => {
                write!(formatter, "unsupported version {actual}")
            }
            Self::UnsupportedFlags { actual } => {
                write!(formatter, "unsupported flags {actual:#06x}")
            }
            Self::UnsupportedPayloadFlags { actual } => {
                write!(formatter, "unsupported fd payload flags {actual:#06x}")
            }
            Self::InvalidTotalLength { declared, actual } => write!(
                formatter,
                "declared frame length {declared} differs from received length {actual}"
            ),
            Self::InvalidPayloadCount { actual } => write!(
                formatter,
                "request must bind exactly {WORKER_V3_VERIFICATION_FD_PAYLOADS_V1} fd payloads, got {actual}"
            ),
            Self::UnknownPayloadKind { actual } => {
                write!(formatter, "unknown fd payload kind {actual}")
            }
            Self::UnknownResponseDisposition { actual } => {
                write!(formatter, "unknown response disposition {actual}")
            }
            Self::NoncanonicalReservedBytes => formatter.write_str("reserved bytes are nonzero"),
            Self::InvalidEntrySectionLength {
                entry_count,
                actual,
                minimum,
                maximum,
            } => write!(
                formatter,
                "entry count {entry_count} requires {minimum}..={maximum} request bytes, got {actual}"
            ),
            Self::LengthOverflow => formatter.write_str("frame length overflow"),
            Self::AllocationFailed { requested } => {
                write!(formatter, "failed to allocate bounded capacity {requested}")
            }
            Self::Truncated => formatter.write_str("truncated frame"),
            Self::TrailingBytes => formatter.write_str("frame has trailing bytes"),
            Self::RequestIdentityMismatch => formatter.write_str("request identity mismatch"),
            Self::ResponseIdentityMismatch => formatter.write_str("response identity mismatch"),
        }
    }
}

impl Error for WorkerV3VerificationProtocolErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidEntryName { source, .. } => Some(source),
            _ => None,
        }
    }
}
