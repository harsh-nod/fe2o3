use std::{fmt, str};

use fe2o3_artifact_transaction::TargetIdentityV1;
use fe2o3_artifacts::{
    DigestAlgorithm, DigestBytes, DirectLinkContainerIdentityV1,
    DirectLinkFinalizedPayloadIdentityV1, DirectLinkLinkedOutputIdentityV1,
    DirectLinkRequestIdentityV1, DirectLinkResponseIdentityV1, IdentityText,
    MAX_IDENTITY_TEXT_BYTES, MeasuredToolIdentity, PayloadDigest,
};
use fe2o3_rustc_invocation::InvocationDigestV2;
use sha2::{Digest as _, Sha256};

pub const COMPILER_TRANSACTION_EVIDENCE_MAGIC_V2: [u8; 8] = *b"FE2CTX2\0";
pub const COMPILER_TRANSACTION_EVIDENCE_VERSION_V2: u16 = 2;
pub const MAX_COMPILER_TRANSACTION_EVIDENCE_BYTES_V2: usize = 2 * 1024 * 1024;
pub const MAX_COMPILER_TRANSACTION_DEPENDENCIES_V2: usize = 4096;
pub const MAX_COMPILER_TRANSACTION_FEATURES_V2: usize = 1024;
pub const CALLER_MEASURED_IDENTITY_ALGORITHM_V2: DigestAlgorithm = DigestAlgorithm::Sha256;

const HEADER_BYTES: usize = 16;
const FIELD_HEADER_BYTES: usize = 5;
const DIGEST_BYTES: usize = 33;
const SHA256_TAG: u8 = 1;
const MAX_DEPENDENCIES_FIELD_BYTES: usize =
    2 + MAX_COMPILER_TRANSACTION_DEPENDENCIES_V2 * (2 + MAX_IDENTITY_TEXT_BYTES + DIGEST_BYTES);
const MAX_FEATURES_FIELD_BYTES: usize =
    2 + MAX_COMPILER_TRANSACTION_FEATURES_V2 * (2 + MAX_IDENTITY_TEXT_BYTES);
const MAX_TOOL_FIELD_BYTES: usize = 4 + 2 * MAX_IDENTITY_TEXT_BYTES + 2 * DIGEST_BYTES;
const MIN_DEPENDENCY_ENTRY_BYTES: usize = 2 + 1 + DIGEST_BYTES;
const MIN_FEATURE_ENTRY_BYTES: usize = 2 + 1;

const SOURCE_ROOT_TAG: u8 = 1;
const DEPENDENCIES_TAG: u8 = 2;
const FEATURES_TAG: u8 = 3;
const RUSTC_TOOL_TAG: u8 = 4;
const RUSTC_INVOCATION_TAG: u8 = 5;
const BACKEND_TOOL_TAG: u8 = 6;
const BACKEND_INVOCATION_TAG: u8 = 7;
const SEMANTIC_WITNESS_TAG: u8 = 8;
const KERNEL_IR_TAG: u8 = 9;
const WORKER_REQUEST_TAG: u8 = 10;
const WORKER_RESPONSE_TAG: u8 = 11;
const TARGET_TAG: u8 = 12;
const RAW_HSACO_TAG: u8 = 13;
const FINALIZED_HSACO_TAG: u8 = 14;
const ARTIFACT_TAG: u8 = 15;
const CAPSULE_IDENTITY_TAG: u8 = 16;
const LAST_FIELD_TAG: u8 = CAPSULE_IDENTITY_TAG;

const CAPSULE_IDENTITY_DOMAIN: &[u8] = b"FE2O3/COMPILER-TRANSACTION-EVIDENCE-CAPSULE/V2\0";
const SOURCE_CLOSURE_IDENTITY_DOMAIN: &[u8] = b"FE2O3/COMPILER-SOURCE-CLOSURE/V2\0";

macro_rules! caller_measured_digest_identity {
    ($(#[$meta:meta])* $name:ident, $field:literal) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(PayloadDigest);

        impl $name {
            pub fn try_from_sha256(
                bytes: [u8; 32],
            ) -> Result<Self, CompilerTransactionValidationErrorV2> {
                require_nonzero($field, &bytes)?;
                Ok(Self(PayloadDigest::new(
                    CALLER_MEASURED_IDENTITY_ALGORITHM_V2,
                    DigestBytes::from_bytes(bytes),
                )))
            }

            pub const fn digest(self) -> PayloadDigest {
                self.0
            }
        }
    };
}

caller_measured_digest_identity!(
    /// Caller-measured SHA-256 identity of the root source input.
    ///
    /// Construction checks representation only; it does not validate the measurement's origin.
    CallerMeasuredSourceRootIdentityV2,
    "caller-measured source root"
);
caller_measured_digest_identity!(
    /// Caller-measured SHA-256 identity of the backend invocation and configuration.
    ///
    /// Construction checks representation only; it does not validate a backend domain.
    CallerMeasuredBackendInvocationIdentityV2,
    "caller-measured backend invocation"
);
caller_measured_digest_identity!(
    /// Caller-measured SHA-256 identity of the semantic witness bytes.
    ///
    /// Construction checks representation only; it does not validate witness semantics.
    CallerMeasuredSemanticWitnessIdentityV2,
    "caller-measured semantic witness"
);
caller_measured_digest_identity!(
    /// Caller-measured SHA-256 identity of the claimed canonical Kernel IR bytes.
    ///
    /// Construction checks representation only; it does not parse or validate Kernel IR.
    CallerMeasuredKernelIrIdentityV2,
    "caller-measured Kernel IR"
);

/// Domain-separated identity of one complete source/dependency/feature closure.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceClosureIdentityV2([u8; 32]);

impl SourceClosureIdentityV2 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Domain-separated identity of one complete compiler-transaction capsule.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerTransactionEvidenceIdentityV2([u8; 32]);

impl CompilerTransactionEvidenceIdentityV2 {
    pub fn from_bytes(bytes: [u8; 32]) -> Result<Self, CompilerTransactionValidationErrorV2> {
        require_nonzero("compiler transaction capsule", &bytes)?;
        Ok(Self(bytes))
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// One named source dependency and its caller-measured SHA-256 content identity.
///
/// Construction checks representation only; it does not validate the measurement's origin.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallerMeasuredSourceDependencyV2 {
    name: IdentityText,
    identity: PayloadDigest,
}

impl CallerMeasuredSourceDependencyV2 {
    pub fn try_from_sha256(
        name: IdentityText,
        bytes: [u8; 32],
    ) -> Result<Self, CompilerTransactionValidationErrorV2> {
        require_nonzero("caller-measured source dependency", &bytes)?;
        Ok(Self {
            name,
            identity: PayloadDigest::new(
                CALLER_MEASURED_IDENTITY_ALGORITHM_V2,
                DigestBytes::from_bytes(bytes),
            ),
        })
    }

    pub const fn name(&self) -> &IdentityText {
        &self.name
    }

    pub const fn identity(&self) -> PayloadDigest {
        self.identity
    }
}

/// Canonical complete source closure selected for one compiler transaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerSourceClosureV2 {
    root: CallerMeasuredSourceRootIdentityV2,
    dependencies: Vec<CallerMeasuredSourceDependencyV2>,
    features: Vec<IdentityText>,
}

impl CompilerSourceClosureV2 {
    pub fn new(
        root: CallerMeasuredSourceRootIdentityV2,
        mut dependencies: Vec<CallerMeasuredSourceDependencyV2>,
        mut features: Vec<IdentityText>,
    ) -> Result<Self, CompilerTransactionValidationErrorV2> {
        require_sha256_nonzero("caller-measured source root", root.digest())?;
        if dependencies.len() > MAX_COMPILER_TRANSACTION_DEPENDENCIES_V2 {
            return Err(CompilerTransactionValidationErrorV2::TooManyDependencies {
                max: MAX_COMPILER_TRANSACTION_DEPENDENCIES_V2,
            });
        }
        if features.len() > MAX_COMPILER_TRANSACTION_FEATURES_V2 {
            return Err(CompilerTransactionValidationErrorV2::TooManyFeatures {
                max: MAX_COMPILER_TRANSACTION_FEATURES_V2,
            });
        }
        dependencies.sort_unstable_by(|left, right| left.name.as_str().cmp(right.name.as_str()));
        if dependencies
            .windows(2)
            .any(|pair| pair[0].name == pair[1].name)
        {
            return Err(CompilerTransactionValidationErrorV2::DuplicateDependency);
        }
        features.sort_unstable();
        if features.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(CompilerTransactionValidationErrorV2::DuplicateFeature);
        }
        Ok(Self {
            root,
            dependencies,
            features,
        })
    }

    pub const fn root(&self) -> CallerMeasuredSourceRootIdentityV2 {
        self.root
    }

    pub fn dependencies(&self) -> &[CallerMeasuredSourceDependencyV2] {
        &self.dependencies
    }

    pub fn features(&self) -> &[IdentityText] {
        &self.features
    }

    pub fn identity(&self) -> SourceClosureIdentityV2 {
        let mut hasher = Sha256::new();
        hasher.update(SOURCE_CLOSURE_IDENTITY_DOMAIN);
        hash_digest(&mut hasher, self.root.digest());
        hasher.update((self.dependencies.len() as u32).to_le_bytes());
        for dependency in &self.dependencies {
            hash_text(&mut hasher, dependency.name.as_str());
            hash_digest(&mut hasher, dependency.identity);
        }
        hasher.update((self.features.len() as u32).to_le_bytes());
        for feature in &self.features {
            hash_text(&mut hasher, feature.as_str());
        }
        SourceClosureIdentityV2(hasher.finalize().into())
    }
}

/// All identities supplied to one inert compiler-transaction evidence capsule.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerTransactionEvidencePartsV2 {
    pub source_closure: CompilerSourceClosureV2,
    pub rustc_tool: MeasuredToolIdentity,
    pub rustc_invocation: InvocationDigestV2,
    pub backend_tool: MeasuredToolIdentity,
    pub backend_invocation: CallerMeasuredBackendInvocationIdentityV2,
    pub semantic_witness: CallerMeasuredSemanticWitnessIdentityV2,
    pub kernel_ir: CallerMeasuredKernelIrIdentityV2,
    pub worker_request: DirectLinkRequestIdentityV1,
    pub worker_response: DirectLinkResponseIdentityV1,
    pub target: TargetIdentityV1,
    pub raw_hsaco: DirectLinkLinkedOutputIdentityV1,
    pub finalized_hsaco: DirectLinkFinalizedPayloadIdentityV1,
    pub artifact: DirectLinkContainerIdentityV1,
}

/// Bounded canonical evidence joining the complete compiler transaction.
///
/// Every measurement is caller-supplied. Construction and decoding establish only
/// canonical structure, digest-domain separation, and byte-level binding. This value
/// authenticates no producer and grants no compiler, publication, load, or launch authority.
/// A later external publication receipt may bind this capsule identity and a separately
/// constructed Worker V2 load-envelope identity without introducing a hash cycle here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerTransactionEvidenceCapsuleV2 {
    parts: CompilerTransactionEvidencePartsV2,
    identity: CompilerTransactionEvidenceIdentityV2,
    encoded_len: usize,
}

impl CompilerTransactionEvidenceCapsuleV2 {
    pub fn new(
        parts: CompilerTransactionEvidencePartsV2,
    ) -> Result<Self, CompilerTransactionValidationErrorV2> {
        Self::new_with_max_encoded_bytes(parts, MAX_COMPILER_TRANSACTION_EVIDENCE_BYTES_V2)
    }

    fn new_with_max_encoded_bytes(
        parts: CompilerTransactionEvidencePartsV2,
        max_encoded_bytes: usize,
    ) -> Result<Self, CompilerTransactionValidationErrorV2> {
        validate_parts(&parts)?;
        let encoded_len = checked_encoded_total_len(&parts).ok_or(
            CompilerTransactionValidationErrorV2::EvidenceTooLarge {
                max: max_encoded_bytes,
            },
        )?;
        if encoded_len > max_encoded_bytes {
            return Err(CompilerTransactionValidationErrorV2::EvidenceTooLarge {
                max: max_encoded_bytes,
            });
        }
        let prefix = encode_prefix(&parts, encoded_len);
        let identity = calculate_capsule_identity(&prefix)?;
        Ok(Self {
            parts,
            identity,
            encoded_len,
        })
    }

    pub fn source_closure(&self) -> &CompilerSourceClosureV2 {
        &self.parts.source_closure
    }

    pub const fn rustc_tool(&self) -> &MeasuredToolIdentity {
        &self.parts.rustc_tool
    }

    pub const fn rustc_invocation(&self) -> InvocationDigestV2 {
        self.parts.rustc_invocation
    }

    pub const fn backend_tool(&self) -> &MeasuredToolIdentity {
        &self.parts.backend_tool
    }

    pub const fn backend_invocation(&self) -> CallerMeasuredBackendInvocationIdentityV2 {
        self.parts.backend_invocation
    }

    pub const fn semantic_witness(&self) -> CallerMeasuredSemanticWitnessIdentityV2 {
        self.parts.semantic_witness
    }

    pub const fn kernel_ir(&self) -> CallerMeasuredKernelIrIdentityV2 {
        self.parts.kernel_ir
    }

    pub const fn worker_request(&self) -> DirectLinkRequestIdentityV1 {
        self.parts.worker_request
    }

    pub const fn worker_response(&self) -> DirectLinkResponseIdentityV1 {
        self.parts.worker_response
    }

    pub const fn target(&self) -> TargetIdentityV1 {
        self.parts.target
    }

    pub const fn raw_hsaco(&self) -> DirectLinkLinkedOutputIdentityV1 {
        self.parts.raw_hsaco
    }

    pub const fn finalized_hsaco(&self) -> DirectLinkFinalizedPayloadIdentityV1 {
        self.parts.finalized_hsaco
    }

    pub const fn artifact(&self) -> DirectLinkContainerIdentityV1 {
        self.parts.artifact
    }

    pub const fn identity(&self) -> CompilerTransactionEvidenceIdentityV2 {
        self.identity
    }

    pub const fn authenticates_producer(&self) -> bool {
        false
    }

    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = self.encode_prefix();
        write_field(&mut bytes, CAPSULE_IDENTITY_TAG, self.identity.as_bytes());
        debug_assert_eq!(bytes.len(), self.encoded_len);
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CompilerTransactionDecodeErrorV2> {
        decode_capsule(bytes, None)
    }

    /// Decodes a capsule only when it has the externally expected transaction identity.
    ///
    /// This detects a stale or substituted, otherwise well-formed capsule. The expected
    /// identity must itself come from an authenticated/currentness boundary.
    pub fn from_bytes_for_identity(
        bytes: &[u8],
        expected: CompilerTransactionEvidenceIdentityV2,
    ) -> Result<Self, CompilerTransactionDecodeErrorV2> {
        decode_capsule(bytes, Some(expected))
    }

    fn encode_prefix(&self) -> Vec<u8> {
        encode_prefix(&self.parts, self.encoded_len)
    }
}

fn encode_prefix(parts: &CompilerTransactionEvidencePartsV2, encoded_len: usize) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(encoded_len);
    bytes.extend_from_slice(&COMPILER_TRANSACTION_EVIDENCE_MAGIC_V2);
    bytes.extend_from_slice(&COMPILER_TRANSACTION_EVIDENCE_VERSION_V2.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(&(encoded_len as u32).to_le_bytes());
    write_field(
        &mut bytes,
        SOURCE_ROOT_TAG,
        &encode_digest(parts.source_closure.root.digest()),
    );
    write_dependencies_field(&mut bytes, &parts.source_closure);
    write_features_field(&mut bytes, &parts.source_closure);
    write_tool_field(&mut bytes, RUSTC_TOOL_TAG, &parts.rustc_tool);
    write_field(
        &mut bytes,
        RUSTC_INVOCATION_TAG,
        parts.rustc_invocation.as_bytes(),
    );
    write_tool_field(&mut bytes, BACKEND_TOOL_TAG, &parts.backend_tool);
    write_field(
        &mut bytes,
        BACKEND_INVOCATION_TAG,
        &encode_digest(parts.backend_invocation.digest()),
    );
    write_field(
        &mut bytes,
        SEMANTIC_WITNESS_TAG,
        &encode_digest(parts.semantic_witness.digest()),
    );
    write_field(
        &mut bytes,
        KERNEL_IR_TAG,
        &encode_digest(parts.kernel_ir.digest()),
    );
    write_field(
        &mut bytes,
        WORKER_REQUEST_TAG,
        &encode_digest(parts.worker_request.digest()),
    );
    write_field(
        &mut bytes,
        WORKER_RESPONSE_TAG,
        &encode_digest(parts.worker_response.digest()),
    );
    write_field(&mut bytes, TARGET_TAG, parts.target.as_bytes());
    write_field(
        &mut bytes,
        RAW_HSACO_TAG,
        &encode_digest(parts.raw_hsaco.digest()),
    );
    write_field(
        &mut bytes,
        FINALIZED_HSACO_TAG,
        &encode_digest(parts.finalized_hsaco.digest()),
    );
    write_field(
        &mut bytes,
        ARTIFACT_TAG,
        &encode_digest(parts.artifact.digest()),
    );
    bytes
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CompilerTransactionValidationErrorV2 {
    TooManyDependencies { max: usize },
    TooManyFeatures { max: usize },
    EvidenceTooLarge { max: usize },
    DuplicateDependency,
    DuplicateFeature,
    ReservedZeroIdentity { field: &'static str },
    UnsupportedDigestAlgorithm { field: &'static str },
}

impl fmt::Display for CompilerTransactionValidationErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyDependencies { max } => {
                write!(
                    formatter,
                    "compiler source closure exceeds {max} dependencies"
                )
            }
            Self::TooManyFeatures { max } => {
                write!(formatter, "compiler source closure exceeds {max} features")
            }
            Self::EvidenceTooLarge { max } => {
                write!(
                    formatter,
                    "compiler transaction capsule exceeds {max} bytes"
                )
            }
            Self::DuplicateDependency => {
                formatter.write_str("compiler source closure contains a duplicate dependency")
            }
            Self::DuplicateFeature => {
                formatter.write_str("compiler source closure contains a duplicate feature")
            }
            Self::ReservedZeroIdentity { field } => {
                write!(formatter, "{field} uses the reserved all-zero identity")
            }
            Self::UnsupportedDigestAlgorithm { field } => {
                write!(formatter, "{field} must use SHA-256")
            }
        }
    }
}

impl std::error::Error for CompilerTransactionValidationErrorV2 {}

#[derive(Debug)]
#[non_exhaustive]
pub enum CompilerTransactionDecodeErrorV2 {
    TooLarge {
        max: usize,
    },
    Truncated,
    InvalidMagic,
    UnknownVersion(u16),
    UnsupportedFlags(u16),
    InvalidTotalLength,
    TrailingBytes,
    UnknownField(u8),
    DuplicateField(u8),
    UnexpectedField {
        expected: u8,
        actual: u8,
    },
    LengthOutOfRange {
        field: u8,
        value: u64,
        max: usize,
    },
    CountOutOfRange {
        field: &'static str,
        value: u64,
        max: usize,
    },
    CollectionEncodingTooShort {
        field: &'static str,
        count: usize,
        minimum: usize,
        remaining: usize,
    },
    AllocationFailed {
        field: &'static str,
        count: usize,
    },
    StringTooLong {
        field: &'static str,
        value: usize,
        max: usize,
    },
    InvalidUtf8 {
        field: &'static str,
    },
    InvalidText {
        field: &'static str,
    },
    UnknownDigestAlgorithm(u8),
    InvalidRustcInvocationIdentity,
    NonCanonicalDependencyOrder,
    NonCanonicalFeatureOrder,
    FieldTrailingBytes {
        field: u8,
    },
    CapsuleIdentityMismatch,
    UnexpectedCapsuleIdentity,
    Validation(CompilerTransactionValidationErrorV2),
    NonCanonical,
}

impl fmt::Display for CompilerTransactionDecodeErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { max } => write!(formatter, "compiler transaction capsule exceeds {max} bytes"),
            Self::Truncated => formatter.write_str("compiler transaction capsule is truncated"),
            Self::InvalidMagic => formatter.write_str("compiler transaction capsule magic is invalid"),
            Self::UnknownVersion(version) => write!(formatter, "unsupported compiler transaction capsule version {version}"),
            Self::UnsupportedFlags(flags) => write!(formatter, "unsupported compiler transaction capsule flags {flags:#x}"),
            Self::InvalidTotalLength => formatter.write_str("compiler transaction capsule total length is invalid"),
            Self::TrailingBytes => formatter.write_str("compiler transaction capsule has trailing bytes"),
            Self::UnknownField(tag) => write!(formatter, "unknown compiler transaction field tag {tag}"),
            Self::DuplicateField(tag) => write!(formatter, "duplicate compiler transaction field tag {tag}"),
            Self::UnexpectedField { expected, actual } => write!(formatter, "expected compiler transaction field {expected}, found {actual}"),
            Self::LengthOutOfRange { field, value, max } => write!(formatter, "compiler transaction field {field} length {value} exceeds {max}"),
            Self::CountOutOfRange { field, value, max } => write!(formatter, "compiler transaction {field} count {value} exceeds {max}"),
            Self::CollectionEncodingTooShort { field, count, minimum, remaining } => write!(formatter, "compiler transaction {field} count {count} requires at least {minimum} bytes, found {remaining}"),
            Self::AllocationFailed { field, count } => write!(formatter, "could not reserve compiler transaction {field} collection for {count} entries"),
            Self::StringTooLong { field, value, max } => write!(formatter, "compiler transaction {field} length {value} exceeds {max}"),
            Self::InvalidUtf8 { field } => write!(formatter, "compiler transaction {field} is not UTF-8"),
            Self::InvalidText { field } => write!(formatter, "compiler transaction {field} is not canonical identity text"),
            Self::UnknownDigestAlgorithm(tag) => write!(formatter, "unknown compiler transaction digest algorithm {tag}"),
            Self::InvalidRustcInvocationIdentity => formatter.write_str("compiler transaction rustc invocation identity is invalid"),
            Self::NonCanonicalDependencyOrder => formatter.write_str("compiler transaction dependencies are not strictly ordered"),
            Self::NonCanonicalFeatureOrder => formatter.write_str("compiler transaction features are not strictly ordered"),
            Self::FieldTrailingBytes { field } => write!(formatter, "compiler transaction field {field} has trailing bytes"),
            Self::CapsuleIdentityMismatch => formatter.write_str("compiler transaction capsule identity does not match its fields"),
            Self::UnexpectedCapsuleIdentity => formatter.write_str("compiler transaction capsule is stale or substituted relative to the expected identity"),
            Self::Validation(error) => error.fmt(formatter),
            Self::NonCanonical => formatter.write_str("compiler transaction capsule is not canonical"),
        }
    }
}

impl std::error::Error for CompilerTransactionDecodeErrorV2 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Validation(error) => Some(error),
            _ => None,
        }
    }
}

fn validate_parts(
    parts: &CompilerTransactionEvidencePartsV2,
) -> Result<(), CompilerTransactionValidationErrorV2> {
    for (field, digest) in [
        (
            "caller-measured source root",
            parts.source_closure.root.digest(),
        ),
        (
            "caller-measured rustc executable",
            parts.rustc_tool.executable_digest(),
        ),
        (
            "caller-measured rustc configuration",
            parts.rustc_tool.configuration_digest(),
        ),
        (
            "caller-measured backend executable",
            parts.backend_tool.executable_digest(),
        ),
        (
            "caller-measured backend configuration",
            parts.backend_tool.configuration_digest(),
        ),
        (
            "caller-measured backend invocation",
            parts.backend_invocation.digest(),
        ),
        (
            "caller-measured semantic witness",
            parts.semantic_witness.digest(),
        ),
        ("caller-measured Kernel IR", parts.kernel_ir.digest()),
        ("Worker V2 request", parts.worker_request.digest()),
        ("Worker V2 response", parts.worker_response.digest()),
        ("raw HSACO", parts.raw_hsaco.digest()),
        ("finalized HSACO", parts.finalized_hsaco.digest()),
        ("artifact", parts.artifact.digest()),
    ] {
        require_sha256_nonzero(field, digest)?;
    }
    for dependency in &parts.source_closure.dependencies {
        require_sha256_nonzero("caller-measured source dependency", dependency.identity)?;
    }
    require_nonzero("target", parts.target.as_bytes())?;
    Ok(())
}

fn require_sha256_nonzero(
    field: &'static str,
    digest: PayloadDigest,
) -> Result<(), CompilerTransactionValidationErrorV2> {
    if digest.algorithm() != DigestAlgorithm::Sha256 {
        return Err(CompilerTransactionValidationErrorV2::UnsupportedDigestAlgorithm { field });
    }
    require_nonzero(field, digest.bytes().as_bytes())
}

fn require_nonzero(
    field: &'static str,
    bytes: &[u8; 32],
) -> Result<(), CompilerTransactionValidationErrorV2> {
    if bytes == &[0; 32] {
        Err(CompilerTransactionValidationErrorV2::ReservedZeroIdentity { field })
    } else {
        Ok(())
    }
}

fn checked_encoded_total_len(parts: &CompilerTransactionEvidencePartsV2) -> Option<usize> {
    HEADER_BYTES
        .checked_add(FIELD_HEADER_BYTES.checked_mul(usize::from(LAST_FIELD_TAG))?)?
        .checked_add(DIGEST_BYTES.checked_mul(9)?)?
        .checked_add(32_usize.checked_mul(3)?)?
        .checked_add(checked_dependencies_encoded_len(&parts.source_closure)?)?
        .checked_add(checked_features_encoded_len(&parts.source_closure)?)?
        .checked_add(checked_tool_encoded_len(&parts.rustc_tool)?)?
        .checked_add(checked_tool_encoded_len(&parts.backend_tool)?)
}

fn checked_dependencies_encoded_len(source: &CompilerSourceClosureV2) -> Option<usize> {
    source
        .dependencies
        .iter()
        .try_fold(2_usize, |total, dependency| {
            total
                .checked_add(2)?
                .checked_add(dependency.name.as_str().len())?
                .checked_add(DIGEST_BYTES)
        })
}

fn checked_features_encoded_len(source: &CompilerSourceClosureV2) -> Option<usize> {
    source.features.iter().try_fold(2_usize, |total, feature| {
        total.checked_add(2)?.checked_add(feature.as_str().len())
    })
}

fn checked_tool_encoded_len(tool: &MeasuredToolIdentity) -> Option<usize> {
    4_usize
        .checked_add(tool.name().as_str().len())?
        .checked_add(tool.version().as_str().len())?
        .checked_add(DIGEST_BYTES.checked_mul(2)?)
}

fn write_dependencies_field(bytes: &mut Vec<u8>, source: &CompilerSourceClosureV2) {
    let payload_len = checked_dependencies_encoded_len(source)
        .expect("validated dependency field length must remain representable");
    write_field_header(bytes, DEPENDENCIES_TAG, payload_len);
    bytes.extend_from_slice(&(source.dependencies.len() as u16).to_le_bytes());
    for dependency in &source.dependencies {
        write_text(bytes, dependency.name.as_str());
        bytes.extend_from_slice(&encode_digest(dependency.identity));
    }
}

fn write_features_field(bytes: &mut Vec<u8>, source: &CompilerSourceClosureV2) {
    let payload_len = checked_features_encoded_len(source)
        .expect("validated feature field length must remain representable");
    write_field_header(bytes, FEATURES_TAG, payload_len);
    bytes.extend_from_slice(&(source.features.len() as u16).to_le_bytes());
    for feature in &source.features {
        write_text(bytes, feature.as_str());
    }
}

fn write_tool_field(bytes: &mut Vec<u8>, tag: u8, tool: &MeasuredToolIdentity) {
    let payload_len = checked_tool_encoded_len(tool)
        .expect("validated tool field length must remain representable");
    write_field_header(bytes, tag, payload_len);
    write_text(bytes, tool.name().as_str());
    write_text(bytes, tool.version().as_str());
    bytes.extend_from_slice(&encode_digest(tool.executable_digest()));
    bytes.extend_from_slice(&encode_digest(tool.configuration_digest()));
}

fn encode_digest(digest: PayloadDigest) -> [u8; DIGEST_BYTES] {
    debug_assert_eq!(digest.algorithm(), DigestAlgorithm::Sha256);
    let mut bytes = [0; DIGEST_BYTES];
    bytes[0] = SHA256_TAG;
    bytes[1..].copy_from_slice(digest.bytes().as_bytes());
    bytes
}

fn write_text(bytes: &mut Vec<u8>, value: &str) {
    debug_assert!(value.len() <= MAX_IDENTITY_TEXT_BYTES);
    bytes.extend_from_slice(&(value.len() as u16).to_le_bytes());
    bytes.extend_from_slice(value.as_bytes());
}

fn write_field(bytes: &mut Vec<u8>, tag: u8, payload: &[u8]) {
    write_field_header(bytes, tag, payload.len());
    bytes.extend_from_slice(payload);
}

fn write_field_header(bytes: &mut Vec<u8>, tag: u8, payload_len: usize) {
    bytes.push(tag);
    bytes.extend_from_slice(&(payload_len as u32).to_le_bytes());
}

fn calculate_capsule_identity(
    prefix: &[u8],
) -> Result<CompilerTransactionEvidenceIdentityV2, CompilerTransactionValidationErrorV2> {
    let mut hasher = Sha256::new();
    hasher.update(CAPSULE_IDENTITY_DOMAIN);
    hasher.update(prefix);
    CompilerTransactionEvidenceIdentityV2::from_bytes(hasher.finalize().into())
}

fn hash_text(hasher: &mut Sha256, value: &str) {
    hasher.update((value.len() as u32).to_le_bytes());
    hasher.update(value.as_bytes());
}

fn hash_digest(hasher: &mut Sha256, digest: PayloadDigest) {
    hasher.update([SHA256_TAG]);
    hasher.update(digest.bytes().as_bytes());
}

fn decode_capsule(
    bytes: &[u8],
    expected: Option<CompilerTransactionEvidenceIdentityV2>,
) -> Result<CompilerTransactionEvidenceCapsuleV2, CompilerTransactionDecodeErrorV2> {
    if bytes.len() > MAX_COMPILER_TRANSACTION_EVIDENCE_BYTES_V2 {
        return Err(CompilerTransactionDecodeErrorV2::TooLarge {
            max: MAX_COMPILER_TRANSACTION_EVIDENCE_BYTES_V2,
        });
    }
    let mut reader = Reader::new(bytes);
    if reader.array::<8>()? != COMPILER_TRANSACTION_EVIDENCE_MAGIC_V2 {
        return Err(CompilerTransactionDecodeErrorV2::InvalidMagic);
    }
    let version = reader.u16()?;
    if version != COMPILER_TRANSACTION_EVIDENCE_VERSION_V2 {
        return Err(CompilerTransactionDecodeErrorV2::UnknownVersion(version));
    }
    let flags = reader.u16()?;
    if flags != 0 {
        return Err(CompilerTransactionDecodeErrorV2::UnsupportedFlags(flags));
    }
    let total_len = reader.u32()? as usize;
    if total_len > bytes.len() {
        return Err(CompilerTransactionDecodeErrorV2::Truncated);
    }
    if total_len < bytes.len() {
        return Err(CompilerTransactionDecodeErrorV2::TrailingBytes);
    }
    if total_len < HEADER_BYTES {
        return Err(CompilerTransactionDecodeErrorV2::InvalidTotalLength);
    }

    let root_digest = decode_digest_field(&mut reader, SOURCE_ROOT_TAG)?;
    let root = CallerMeasuredSourceRootIdentityV2::try_from_sha256(*root_digest.bytes().as_bytes())
        .map_err(CompilerTransactionDecodeErrorV2::Validation)?;
    let dependencies =
        decode_dependencies(reader.field(DEPENDENCIES_TAG, MAX_DEPENDENCIES_FIELD_BYTES)?)?;
    let features = decode_features(reader.field(FEATURES_TAG, MAX_FEATURES_FIELD_BYTES)?)?;
    let rustc_tool = decode_tool(
        reader.field(RUSTC_TOOL_TAG, MAX_TOOL_FIELD_BYTES)?,
        RUSTC_TOOL_TAG,
    )?;
    let rustc_invocation = InvocationDigestV2::from_bytes(decode_fixed_field::<32>(
        &mut reader,
        RUSTC_INVOCATION_TAG,
    )?)
    .map_err(|_| CompilerTransactionDecodeErrorV2::InvalidRustcInvocationIdentity)?;
    let backend_tool = decode_tool(
        reader.field(BACKEND_TOOL_TAG, MAX_TOOL_FIELD_BYTES)?,
        BACKEND_TOOL_TAG,
    )?;
    let backend_invocation_digest = decode_digest_field(&mut reader, BACKEND_INVOCATION_TAG)?;
    let backend_invocation = CallerMeasuredBackendInvocationIdentityV2::try_from_sha256(
        *backend_invocation_digest.bytes().as_bytes(),
    )
    .map_err(CompilerTransactionDecodeErrorV2::Validation)?;
    let semantic_witness_digest = decode_digest_field(&mut reader, SEMANTIC_WITNESS_TAG)?;
    let semantic_witness = CallerMeasuredSemanticWitnessIdentityV2::try_from_sha256(
        *semantic_witness_digest.bytes().as_bytes(),
    )
    .map_err(CompilerTransactionDecodeErrorV2::Validation)?;
    let kernel_ir_digest = decode_digest_field(&mut reader, KERNEL_IR_TAG)?;
    let kernel_ir =
        CallerMeasuredKernelIrIdentityV2::try_from_sha256(*kernel_ir_digest.bytes().as_bytes())
            .map_err(CompilerTransactionDecodeErrorV2::Validation)?;
    let worker_request =
        DirectLinkRequestIdentityV1::new(decode_digest_field(&mut reader, WORKER_REQUEST_TAG)?);
    let worker_response =
        DirectLinkResponseIdentityV1::new(decode_digest_field(&mut reader, WORKER_RESPONSE_TAG)?);
    let target = TargetIdentityV1::from_bytes(decode_fixed_field::<32>(&mut reader, TARGET_TAG)?);
    let raw_hsaco =
        DirectLinkLinkedOutputIdentityV1::new(decode_digest_field(&mut reader, RAW_HSACO_TAG)?);
    let finalized_hsaco = DirectLinkFinalizedPayloadIdentityV1::new(decode_digest_field(
        &mut reader,
        FINALIZED_HSACO_TAG,
    )?);
    let artifact =
        DirectLinkContainerIdentityV1::new(decode_digest_field(&mut reader, ARTIFACT_TAG)?);

    let identity_prefix_len = reader.position();
    let encoded_identity =
        CompilerTransactionEvidenceIdentityV2::from_bytes(decode_fixed_field::<32>(
            &mut reader,
            CAPSULE_IDENTITY_TAG,
        )?)
        .map_err(CompilerTransactionDecodeErrorV2::Validation)?;
    if !reader.is_empty() {
        return Err(CompilerTransactionDecodeErrorV2::TrailingBytes);
    }
    let calculated_identity = calculate_capsule_identity(&bytes[..identity_prefix_len])
        .map_err(CompilerTransactionDecodeErrorV2::Validation)?;
    if encoded_identity != calculated_identity {
        return Err(CompilerTransactionDecodeErrorV2::CapsuleIdentityMismatch);
    }
    if expected.is_some_and(|value| value != calculated_identity) {
        return Err(CompilerTransactionDecodeErrorV2::UnexpectedCapsuleIdentity);
    }

    let source_closure = CompilerSourceClosureV2::new(root, dependencies, features)
        .map_err(CompilerTransactionDecodeErrorV2::Validation)?;
    let capsule = CompilerTransactionEvidenceCapsuleV2::new(CompilerTransactionEvidencePartsV2 {
        source_closure,
        rustc_tool,
        rustc_invocation,
        backend_tool,
        backend_invocation,
        semantic_witness,
        kernel_ir,
        worker_request,
        worker_response,
        target,
        raw_hsaco,
        finalized_hsaco,
        artifact,
    })
    .map_err(CompilerTransactionDecodeErrorV2::Validation)?;
    if capsule.identity != encoded_identity || capsule.to_bytes() != bytes {
        return Err(CompilerTransactionDecodeErrorV2::NonCanonical);
    }
    Ok(capsule)
}

fn decode_dependencies(
    bytes: &[u8],
) -> Result<Vec<CallerMeasuredSourceDependencyV2>, CompilerTransactionDecodeErrorV2> {
    let mut reader = Reader::new(bytes);
    let count = usize::from(reader.u16()?);
    if count > MAX_COMPILER_TRANSACTION_DEPENDENCIES_V2 {
        return Err(CompilerTransactionDecodeErrorV2::CountOutOfRange {
            field: "dependency",
            value: count as u64,
            max: MAX_COMPILER_TRANSACTION_DEPENDENCIES_V2,
        });
    }
    require_minimum_collection_encoding(
        "dependency",
        count,
        MIN_DEPENDENCY_ENTRY_BYTES,
        reader.remaining_len(),
    )?;
    let mut dependencies = Vec::new();
    dependencies.try_reserve_exact(count).map_err(|_| {
        CompilerTransactionDecodeErrorV2::AllocationFailed {
            field: "dependency",
            count,
        }
    })?;
    for _ in 0..count {
        let name = reader.identity_text("dependency name")?;
        let identity = reader.digest()?;
        if dependencies
            .last()
            .is_some_and(|previous: &CallerMeasuredSourceDependencyV2| {
                previous.name.as_str() >= name.as_str()
            })
        {
            return Err(CompilerTransactionDecodeErrorV2::NonCanonicalDependencyOrder);
        }
        dependencies.push(
            CallerMeasuredSourceDependencyV2::try_from_sha256(name, *identity.bytes().as_bytes())
                .map_err(CompilerTransactionDecodeErrorV2::Validation)?,
        );
    }
    if !reader.is_empty() {
        return Err(CompilerTransactionDecodeErrorV2::FieldTrailingBytes {
            field: DEPENDENCIES_TAG,
        });
    }
    Ok(dependencies)
}

fn decode_features(bytes: &[u8]) -> Result<Vec<IdentityText>, CompilerTransactionDecodeErrorV2> {
    let mut reader = Reader::new(bytes);
    let count = usize::from(reader.u16()?);
    if count > MAX_COMPILER_TRANSACTION_FEATURES_V2 {
        return Err(CompilerTransactionDecodeErrorV2::CountOutOfRange {
            field: "feature",
            value: count as u64,
            max: MAX_COMPILER_TRANSACTION_FEATURES_V2,
        });
    }
    require_minimum_collection_encoding(
        "feature",
        count,
        MIN_FEATURE_ENTRY_BYTES,
        reader.remaining_len(),
    )?;
    let mut features = Vec::new();
    features.try_reserve_exact(count).map_err(|_| {
        CompilerTransactionDecodeErrorV2::AllocationFailed {
            field: "feature",
            count,
        }
    })?;
    for _ in 0..count {
        let feature = reader.identity_text("feature")?;
        if features
            .last()
            .is_some_and(|previous: &IdentityText| previous.as_str() >= feature.as_str())
        {
            return Err(CompilerTransactionDecodeErrorV2::NonCanonicalFeatureOrder);
        }
        features.push(feature);
    }
    if !reader.is_empty() {
        return Err(CompilerTransactionDecodeErrorV2::FieldTrailingBytes {
            field: FEATURES_TAG,
        });
    }
    Ok(features)
}

fn decode_tool(
    bytes: &[u8],
    field: u8,
) -> Result<MeasuredToolIdentity, CompilerTransactionDecodeErrorV2> {
    let mut reader = Reader::new(bytes);
    let name = reader.identity_text("tool name")?;
    let version = reader.identity_text("tool version")?;
    let executable = reader.digest()?;
    let configuration = reader.digest()?;
    if !reader.is_empty() {
        return Err(CompilerTransactionDecodeErrorV2::FieldTrailingBytes { field });
    }
    Ok(MeasuredToolIdentity::new(
        name,
        version,
        executable,
        configuration,
    ))
}

fn require_minimum_collection_encoding(
    field: &'static str,
    count: usize,
    minimum_entry_bytes: usize,
    remaining: usize,
) -> Result<(), CompilerTransactionDecodeErrorV2> {
    let minimum = count.checked_mul(minimum_entry_bytes).ok_or(
        CompilerTransactionDecodeErrorV2::CollectionEncodingTooShort {
            field,
            count,
            minimum: usize::MAX,
            remaining,
        },
    )?;
    if minimum > remaining {
        Err(
            CompilerTransactionDecodeErrorV2::CollectionEncodingTooShort {
                field,
                count,
                minimum,
                remaining,
            },
        )
    } else {
        Ok(())
    }
}

fn decode_digest_field(
    reader: &mut Reader<'_>,
    tag: u8,
) -> Result<PayloadDigest, CompilerTransactionDecodeErrorV2> {
    let bytes = reader.field(tag, DIGEST_BYTES)?;
    if bytes.len() != DIGEST_BYTES {
        return Err(CompilerTransactionDecodeErrorV2::LengthOutOfRange {
            field: tag,
            value: bytes.len() as u64,
            max: DIGEST_BYTES,
        });
    }
    let mut nested = Reader::new(bytes);
    nested.digest()
}

fn decode_fixed_field<const N: usize>(
    reader: &mut Reader<'_>,
    tag: u8,
) -> Result<[u8; N], CompilerTransactionDecodeErrorV2> {
    let bytes = reader.field(tag, N)?;
    if bytes.len() != N {
        return Err(CompilerTransactionDecodeErrorV2::LengthOutOfRange {
            field: tag,
            value: bytes.len() as u64,
            max: N,
        });
    }
    let mut value = [0; N];
    value.copy_from_slice(bytes);
    Ok(value)
}

struct Reader<'a> {
    original_len: usize,
    remaining: &'a [u8],
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self {
            original_len: bytes.len(),
            remaining: bytes,
        }
    }

    const fn is_empty(&self) -> bool {
        self.remaining.is_empty()
    }

    const fn remaining_len(&self) -> usize {
        self.remaining.len()
    }

    const fn position(&self) -> usize {
        self.original_len - self.remaining.len()
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], CompilerTransactionDecodeErrorV2> {
        if self.remaining.len() < count {
            return Err(CompilerTransactionDecodeErrorV2::Truncated);
        }
        let (value, remaining) = self.remaining.split_at(count);
        self.remaining = remaining;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], CompilerTransactionDecodeErrorV2> {
        let mut value = [0; N];
        value.copy_from_slice(self.take(N)?);
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8, CompilerTransactionDecodeErrorV2> {
        Ok(self.array::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, CompilerTransactionDecodeErrorV2> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, CompilerTransactionDecodeErrorV2> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn field(
        &mut self,
        expected: u8,
        max: usize,
    ) -> Result<&'a [u8], CompilerTransactionDecodeErrorV2> {
        let actual = self.u8()?;
        if actual != expected {
            return if actual > LAST_FIELD_TAG || actual == 0 {
                Err(CompilerTransactionDecodeErrorV2::UnknownField(actual))
            } else if actual < expected {
                Err(CompilerTransactionDecodeErrorV2::DuplicateField(actual))
            } else {
                Err(CompilerTransactionDecodeErrorV2::UnexpectedField { expected, actual })
            };
        }
        let length = u64::from(self.u32()?);
        if length > max as u64 {
            return Err(CompilerTransactionDecodeErrorV2::LengthOutOfRange {
                field: actual,
                value: length,
                max,
            });
        }
        self.take(length as usize)
    }

    fn digest(&mut self) -> Result<PayloadDigest, CompilerTransactionDecodeErrorV2> {
        let algorithm = self.u8()?;
        if algorithm != SHA256_TAG {
            return Err(CompilerTransactionDecodeErrorV2::UnknownDigestAlgorithm(
                algorithm,
            ));
        }
        Ok(PayloadDigest::new(
            DigestAlgorithm::Sha256,
            DigestBytes::from_bytes(self.array()?),
        ))
    }

    fn identity_text(
        &mut self,
        field: &'static str,
    ) -> Result<IdentityText, CompilerTransactionDecodeErrorV2> {
        let length = usize::from(self.u16()?);
        if length > MAX_IDENTITY_TEXT_BYTES {
            return Err(CompilerTransactionDecodeErrorV2::StringTooLong {
                field,
                value: length,
                max: MAX_IDENTITY_TEXT_BYTES,
            });
        }
        let value = str::from_utf8(self.take(length)?)
            .map_err(|_| CompilerTransactionDecodeErrorV2::InvalidUtf8 { field })?;
        IdentityText::new(value)
            .map_err(|_| CompilerTransactionDecodeErrorV2::InvalidText { field })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(seed: u8) -> PayloadDigest {
        PayloadDigest::new(DigestAlgorithm::Sha256, DigestBytes::from_bytes([seed; 32]))
    }

    fn maximum_text(prefix: &str) -> IdentityText {
        assert!(prefix.len() <= MAX_IDENTITY_TEXT_BYTES);
        IdentityText::new(format!(
            "{prefix}{}",
            "x".repeat(MAX_IDENTITY_TEXT_BYTES - prefix.len())
        ))
        .unwrap()
    }

    fn maximum_source_set_parts() -> CompilerTransactionEvidencePartsV2 {
        let dependencies = (0..MAX_COMPILER_TRANSACTION_DEPENDENCIES_V2)
            .map(|index| {
                CallerMeasuredSourceDependencyV2::try_from_sha256(
                    maximum_text(&format!("dependency-{index:04}:")),
                    [(index % 255 + 1) as u8; 32],
                )
                .unwrap()
            })
            .collect();
        let features = (0..MAX_COMPILER_TRANSACTION_FEATURES_V2)
            .map(|index| maximum_text(&format!("feature-{index:04}:")))
            .collect();
        let source_closure = CompilerSourceClosureV2::new(
            CallerMeasuredSourceRootIdentityV2::try_from_sha256([0x10; 32]).unwrap(),
            dependencies,
            features,
        )
        .unwrap();
        let measured_tool = |prefix: &str, seed: u8| {
            MeasuredToolIdentity::new(
                maximum_text(&format!("{prefix}-name:")),
                maximum_text(&format!("{prefix}-version:")),
                digest(seed),
                digest(seed.wrapping_add(1)),
            )
        };
        CompilerTransactionEvidencePartsV2 {
            source_closure,
            rustc_tool: measured_tool("rustc", 0x20),
            rustc_invocation: InvocationDigestV2::from_bytes([0x22; 32]).unwrap(),
            backend_tool: measured_tool("backend", 0x30),
            backend_invocation: CallerMeasuredBackendInvocationIdentityV2::try_from_sha256(
                [0x32; 32],
            )
            .unwrap(),
            semantic_witness: CallerMeasuredSemanticWitnessIdentityV2::try_from_sha256([0x40; 32])
                .unwrap(),
            kernel_ir: CallerMeasuredKernelIrIdentityV2::try_from_sha256([0x41; 32]).unwrap(),
            worker_request: DirectLinkRequestIdentityV1::new(digest(0x50)),
            worker_response: DirectLinkResponseIdentityV1::new(digest(0x51)),
            target: TargetIdentityV1::from_bytes([0x52; 32]),
            raw_hsaco: DirectLinkLinkedOutputIdentityV1::new(digest(0x60)),
            finalized_hsaco: DirectLinkFinalizedPayloadIdentityV1::new(digest(0x61)),
            artifact: DirectLinkContainerIdentityV1::new(digest(0x62)),
        }
    }

    #[test]
    fn constructor_rejects_legal_source_set_over_aggregate_limit_before_encoding() {
        let parts = maximum_source_set_parts();
        let encoded_len = checked_encoded_total_len(&parts).unwrap();
        let test_limit = encoded_len - 1;

        assert!(matches!(
            CompilerTransactionEvidenceCapsuleV2::new_with_max_encoded_bytes(parts, test_limit),
            Err(CompilerTransactionValidationErrorV2::EvidenceTooLarge { max })
                if max == test_limit
        ));
    }

    #[test]
    fn maximum_current_public_source_set_remains_within_two_mibibytes() {
        let parts = maximum_source_set_parts();
        let encoded_len = checked_encoded_total_len(&parts).unwrap();
        assert_eq!(encoded_len, 1_457_785);
        assert!(encoded_len < MAX_COMPILER_TRANSACTION_EVIDENCE_BYTES_V2);

        let capsule = CompilerTransactionEvidenceCapsuleV2::new(parts).unwrap();
        assert_eq!(capsule.to_bytes().len(), encoded_len);
    }
}
