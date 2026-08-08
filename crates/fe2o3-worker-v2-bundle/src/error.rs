use std::fmt;

use fe2o3_artifact_transaction::{LinkPublicationCodecError, LinkPublicationPhaseV1};
use fe2o3_artifacts::{
    BundleDecodeError, ContainerDecodeError, DirectLinkDecodeError, DirectLinkEvidenceError,
    ProofDecodeError,
};
use fe2o3_kernel_descriptor::DecodeError as DescriptorDecodeError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicationClaimFieldV1 {
    Scope,
    Request,
    Worker,
    Response,
    LinkedOutput,
    Finalization,
    FinalizedOutput,
    Publication,
    ReceiptFinalizedOutput,
    ReceiptPublication,
}

#[derive(Debug)]
#[non_exhaustive]
pub enum EnvelopeValidationError {
    EmptyRawHsaco,
    RawHsacoTooLarge { max: usize },
    RawHsacoDigestMismatch,
    UnsupportedDigestAlgorithm { field: &'static str },
    BundleDoesNotMatchContainer,
    DirectLinkBindingCount { actual: usize },
    DirectLink(DirectLinkEvidenceError),
    MissingFinalizedPayload,
    FinalizedPayloadIsNotNative,
    FinalizedPayloadNotUsedByEveryKernel,
    DescriptorTargetMismatch,
    UnfinalizedDescriptorLineage,
    DescriptorKernelCountMismatch,
    DescriptorKernelMismatch { field: &'static str },
    ProofCountMismatch,
    DuplicateProofKernel,
    ProofKernelSetMismatch,
    ProofEvidenceTooLarge { max: usize },
    PublicationRecordNotPublished { actual: LinkPublicationPhaseV1 },
    PublicationClaimMismatch(PublicationClaimFieldV1),
    PublicationBridge,
}

impl fmt::Display for EnvelopeValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyRawHsaco => formatter.write_str("raw Worker V2 HSACO must not be empty"),
            Self::RawHsacoTooLarge { max } => {
                write!(formatter, "raw Worker V2 HSACO exceeds {max} bytes")
            }
            Self::RawHsacoDigestMismatch => {
                formatter.write_str("raw Worker V2 HSACO digest does not match its bytes")
            }
            Self::UnsupportedDigestAlgorithm { field } => {
                write!(formatter, "{field} must use SHA-256")
            }
            Self::BundleDoesNotMatchContainer => formatter
                .write_str("bundle index is not the complete canonical closure of the container"),
            Self::DirectLinkBindingCount { actual } => write!(
                formatter,
                "Worker V2 envelope requires exactly one direct-link binding, found {actual}"
            ),
            Self::DirectLink(error) => error.fmt(formatter),
            Self::MissingFinalizedPayload => formatter
                .write_str("direct-link finalized payload is missing from the artifact container"),
            Self::FinalizedPayloadIsNotNative => {
                formatter.write_str("Worker V2 finalized payload is not a native executable")
            }
            Self::FinalizedPayloadNotUsedByEveryKernel => formatter
                .write_str("every Worker V2 manifest kernel must reference the finalized payload"),
            Self::DescriptorTargetMismatch => formatter
                .write_str("descriptor target does not match the container manifest target"),
            Self::UnfinalizedDescriptorLineage => formatter
                .write_str("descriptor lineage carries an unfinalized zero code-object digest"),
            Self::DescriptorKernelCountMismatch => formatter.write_str(
                "descriptor kernel set does not have the same size as the manifest kernel set",
            ),
            Self::DescriptorKernelMismatch { field } => {
                write!(formatter, "descriptor and manifest kernel {field} differ")
            }
            Self::ProofCountMismatch => formatter
                .write_str("proof record count does not match the complete manifest kernel set"),
            Self::DuplicateProofKernel => {
                formatter.write_str("duplicate proof record for one kernel")
            }
            Self::ProofKernelSetMismatch => {
                formatter.write_str("proof record kernel set does not match the manifest")
            }
            Self::ProofEvidenceTooLarge { max } => {
                write!(formatter, "proof evidence exceeds {max} bytes")
            }
            Self::PublicationRecordNotPublished { actual } => write!(
                formatter,
                "durable publication record is at {actual:?}, not Published"
            ),
            Self::PublicationClaimMismatch(field) => {
                write!(
                    formatter,
                    "durable publication claim {field:?} does not match"
                )
            }
            Self::PublicationBridge => formatter.write_str(
                "durable publication claim does not match the manifest-derived direct-link bridge",
            ),
        }
    }
}

impl std::error::Error for EnvelopeValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::DirectLink(error) => Some(error),
            _ => None,
        }
    }
}

impl From<DirectLinkEvidenceError> for EnvelopeValidationError {
    fn from(value: DirectLinkEvidenceError) -> Self {
        Self::DirectLink(value)
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum EnvelopeDecodeError {
    TooLarge {
        max: usize,
    },
    Truncated,
    InvalidMagic,
    UnknownVersion(u16),
    UnsupportedFlags(u16),
    NonZeroReserved(u16),
    LengthOutOfRange {
        field: &'static str,
        value: u64,
        max: usize,
    },
    CountOutOfRange {
        field: &'static str,
        value: u64,
        max: usize,
    },
    LengthOverflow,
    TrailingBytes,
    UnknownDigestAlgorithm(u8),
    Container(ContainerDecodeError),
    Bundle(BundleDecodeError),
    DirectLink(DirectLinkDecodeError),
    Descriptor(DescriptorDecodeError),
    Proof(ProofDecodeError),
    Publication(LinkPublicationCodecError),
    Validation(EnvelopeValidationError),
    NonCanonical,
}

impl fmt::Display for EnvelopeDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { max } => write!(formatter, "Worker V2 envelope exceeds {max} bytes"),
            Self::Truncated => formatter.write_str("Worker V2 envelope is truncated"),
            Self::InvalidMagic => formatter.write_str("Worker V2 envelope magic is invalid"),
            Self::UnknownVersion(version) => {
                write!(
                    formatter,
                    "unsupported Worker V2 envelope version {version}"
                )
            }
            Self::UnsupportedFlags(flags) => {
                write!(formatter, "unsupported Worker V2 envelope flags {flags:#x}")
            }
            Self::NonZeroReserved(value) => {
                write!(
                    formatter,
                    "Worker V2 envelope reserved field is nonzero: {value:#x}"
                )
            }
            Self::LengthOutOfRange { field, value, max } => {
                write!(formatter, "{field} length {value} exceeds {max}")
            }
            Self::CountOutOfRange { field, value, max } => {
                write!(formatter, "{field} count {value} exceeds {max}")
            }
            Self::LengthOverflow => formatter.write_str("Worker V2 envelope length overflows"),
            Self::TrailingBytes => formatter.write_str("Worker V2 envelope has trailing bytes"),
            Self::UnknownDigestAlgorithm(tag) => {
                write!(
                    formatter,
                    "unknown Worker V2 envelope digest algorithm {tag}"
                )
            }
            Self::Container(error) => error.fmt(formatter),
            Self::Bundle(error) => error.fmt(formatter),
            Self::DirectLink(error) => error.fmt(formatter),
            Self::Descriptor(error) => error.fmt(formatter),
            Self::Proof(error) => error.fmt(formatter),
            Self::Publication(error) => error.fmt(formatter),
            Self::Validation(error) => error.fmt(formatter),
            Self::NonCanonical => formatter.write_str("Worker V2 envelope is not canonical"),
        }
    }
}

impl std::error::Error for EnvelopeDecodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Container(error) => Some(error),
            Self::Bundle(error) => Some(error),
            Self::DirectLink(error) => Some(error),
            Self::Descriptor(error) => Some(error),
            Self::Proof(error) => Some(error),
            Self::Publication(error) => Some(error),
            Self::Validation(error) => Some(error),
            _ => None,
        }
    }
}
