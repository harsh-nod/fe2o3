use std::{fmt, str};

use fe2o3_artifact_transaction::TargetIdentityV1;
use fe2o3_artifacts::{
    DigestAlgorithm, DigestBytes, DirectLinkContainerIdentityV1,
    DirectLinkFinalizedPayloadIdentityV1, DirectLinkLinkedOutputIdentityV1,
    DirectLinkRequestIdentityV1, DirectLinkResponseIdentityV1, IdentityText,
    MAX_IDENTITY_TEXT_BYTES, MeasuredToolIdentity, PayloadDigest,
};
use sha2::{Digest as _, Sha256};

pub const COMPILER_TRANSACTION_EVIDENCE_MAGIC_V1: [u8; 8] = *b"FE2CTX1\0";
pub const COMPILER_TRANSACTION_EVIDENCE_VERSION_V1: u16 = 1;
pub const MAX_COMPILER_TRANSACTION_EVIDENCE_BYTES_V1: usize = 2 * 1024 * 1024;
pub const MAX_COMPILER_TRANSACTION_DEPENDENCIES_V1: usize = 4096;
pub const MAX_COMPILER_TRANSACTION_FEATURES_V1: usize = 1024;

const HEADER_BYTES: usize = 16;
const FIELD_HEADER_BYTES: usize = 5;
const DIGEST_BYTES: usize = 33;
const SHA256_TAG: u8 = 1;
const MAX_DEPENDENCIES_FIELD_BYTES: usize =
    2 + MAX_COMPILER_TRANSACTION_DEPENDENCIES_V1 * (2 + MAX_IDENTITY_TEXT_BYTES + DIGEST_BYTES);
const MAX_FEATURES_FIELD_BYTES: usize =
    2 + MAX_COMPILER_TRANSACTION_FEATURES_V1 * (2 + MAX_IDENTITY_TEXT_BYTES);
const MAX_TOOL_FIELD_BYTES: usize = 4 + 2 * MAX_IDENTITY_TEXT_BYTES + 2 * DIGEST_BYTES;

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
const ENVELOPE_TAG: u8 = 16;
const CAPSULE_IDENTITY_TAG: u8 = 17;
const LAST_FIELD_TAG: u8 = CAPSULE_IDENTITY_TAG;

const CAPSULE_IDENTITY_DOMAIN: &[u8] = b"FE2O3/COMPILER-TRANSACTION-EVIDENCE-CAPSULE/V1\0";
const SOURCE_CLOSURE_IDENTITY_DOMAIN: &[u8] = b"FE2O3/COMPILER-SOURCE-CLOSURE/V1\0";

macro_rules! digest_identity {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(PayloadDigest);

        impl $name {
            pub const fn from_sha256(bytes: [u8; 32]) -> Self {
                Self(PayloadDigest::new(
                    DigestAlgorithm::Sha256,
                    DigestBytes::from_bytes(bytes),
                ))
            }

            pub const fn digest(self) -> PayloadDigest {
                self.0
            }
        }
    };
}

digest_identity!(
    /// SHA-256 identity of the root source input selected for device compilation.
    SourceRootIdentityV1
);
digest_identity!(
    /// SHA-256 identity of the exact final rustc invocation.
    RustcInvocationIdentityV1
);
digest_identity!(
    /// SHA-256 identity of the exact backend invocation and configuration.
    BackendInvocationIdentityV1
);
digest_identity!(
    /// SHA-256 identity of the semantic witness emitted by the frontend.
    SemanticWitnessIdentityV1
);
digest_identity!(
    /// SHA-256 identity of the canonical Kernel IR module.
    KernelIrIdentityV1
);
digest_identity!(
    /// SHA-256 identity of the exact canonical Worker V2 load envelope.
    WorkerV2EnvelopeIdentityV1
);

/// Domain-separated identity of one complete source/dependency/feature closure.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceClosureIdentityV1([u8; 32]);

impl SourceClosureIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Domain-separated identity of one complete compiler-transaction capsule.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerTransactionEvidenceIdentityV1([u8; 32]);

impl CompilerTransactionEvidenceIdentityV1 {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// One named source dependency and the SHA-256 identity of its complete selected input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerSourceDependencyV1 {
    name: IdentityText,
    identity: PayloadDigest,
}

impl CompilerSourceDependencyV1 {
    pub fn new(
        name: IdentityText,
        identity: PayloadDigest,
    ) -> Result<Self, CompilerTransactionValidationErrorV1> {
        require_sha256("source dependency", identity)?;
        Ok(Self { name, identity })
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
pub struct CompilerSourceClosureV1 {
    root: SourceRootIdentityV1,
    dependencies: Vec<CompilerSourceDependencyV1>,
    features: Vec<IdentityText>,
}

impl CompilerSourceClosureV1 {
    pub fn new(
        root: SourceRootIdentityV1,
        mut dependencies: Vec<CompilerSourceDependencyV1>,
        mut features: Vec<IdentityText>,
    ) -> Result<Self, CompilerTransactionValidationErrorV1> {
        require_sha256("source root", root.digest())?;
        if dependencies.len() > MAX_COMPILER_TRANSACTION_DEPENDENCIES_V1 {
            return Err(CompilerTransactionValidationErrorV1::TooManyDependencies {
                max: MAX_COMPILER_TRANSACTION_DEPENDENCIES_V1,
            });
        }
        if features.len() > MAX_COMPILER_TRANSACTION_FEATURES_V1 {
            return Err(CompilerTransactionValidationErrorV1::TooManyFeatures {
                max: MAX_COMPILER_TRANSACTION_FEATURES_V1,
            });
        }
        dependencies.sort_unstable_by(|left, right| left.name.as_str().cmp(right.name.as_str()));
        if dependencies
            .windows(2)
            .any(|pair| pair[0].name == pair[1].name)
        {
            return Err(CompilerTransactionValidationErrorV1::DuplicateDependency);
        }
        features.sort_unstable();
        if features.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(CompilerTransactionValidationErrorV1::DuplicateFeature);
        }
        Ok(Self {
            root,
            dependencies,
            features,
        })
    }

    pub const fn root(&self) -> SourceRootIdentityV1 {
        self.root
    }

    pub fn dependencies(&self) -> &[CompilerSourceDependencyV1] {
        &self.dependencies
    }

    pub fn features(&self) -> &[IdentityText] {
        &self.features
    }

    pub fn identity(&self) -> SourceClosureIdentityV1 {
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
        SourceClosureIdentityV1(hasher.finalize().into())
    }
}

/// All identities supplied to one inert compiler-transaction evidence capsule.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerTransactionEvidencePartsV1 {
    pub source_closure: CompilerSourceClosureV1,
    pub rustc_tool: MeasuredToolIdentity,
    pub rustc_invocation: RustcInvocationIdentityV1,
    pub backend_tool: MeasuredToolIdentity,
    pub backend_invocation: BackendInvocationIdentityV1,
    pub semantic_witness: SemanticWitnessIdentityV1,
    pub kernel_ir: KernelIrIdentityV1,
    pub worker_request: DirectLinkRequestIdentityV1,
    pub worker_response: DirectLinkResponseIdentityV1,
    pub target: TargetIdentityV1,
    pub raw_hsaco: DirectLinkLinkedOutputIdentityV1,
    pub finalized_hsaco: DirectLinkFinalizedPayloadIdentityV1,
    pub artifact: DirectLinkContainerIdentityV1,
    pub envelope: WorkerV2EnvelopeIdentityV1,
}

/// Bounded canonical evidence joining the complete compiler transaction.
///
/// Every measurement is caller-supplied. Construction and decoding establish only
/// canonical structure, digest-domain separation, and byte-level binding. This value
/// authenticates no producer and grants no compiler, publication, load, or launch authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerTransactionEvidenceCapsuleV1 {
    parts: CompilerTransactionEvidencePartsV1,
    identity: CompilerTransactionEvidenceIdentityV1,
    encoded_len: usize,
}

impl CompilerTransactionEvidenceCapsuleV1 {
    pub fn new(
        parts: CompilerTransactionEvidencePartsV1,
    ) -> Result<Self, CompilerTransactionValidationErrorV1> {
        Self::new_with_max_encoded_bytes(parts, MAX_COMPILER_TRANSACTION_EVIDENCE_BYTES_V1)
    }

    fn new_with_max_encoded_bytes(
        parts: CompilerTransactionEvidencePartsV1,
        max_encoded_bytes: usize,
    ) -> Result<Self, CompilerTransactionValidationErrorV1> {
        validate_parts(&parts)?;
        let encoded_len = checked_encoded_total_len(&parts).ok_or(
            CompilerTransactionValidationErrorV1::EvidenceTooLarge {
                max: max_encoded_bytes,
            },
        )?;
        if encoded_len > max_encoded_bytes {
            return Err(CompilerTransactionValidationErrorV1::EvidenceTooLarge {
                max: max_encoded_bytes,
            });
        }
        let mut capsule = Self {
            parts,
            identity: CompilerTransactionEvidenceIdentityV1::from_bytes([0; 32]),
            encoded_len,
        };
        capsule.identity = calculate_capsule_identity(&capsule.encode_prefix());
        Ok(capsule)
    }

    pub fn source_closure(&self) -> &CompilerSourceClosureV1 {
        &self.parts.source_closure
    }

    pub const fn rustc_tool(&self) -> &MeasuredToolIdentity {
        &self.parts.rustc_tool
    }

    pub const fn rustc_invocation(&self) -> RustcInvocationIdentityV1 {
        self.parts.rustc_invocation
    }

    pub const fn backend_tool(&self) -> &MeasuredToolIdentity {
        &self.parts.backend_tool
    }

    pub const fn backend_invocation(&self) -> BackendInvocationIdentityV1 {
        self.parts.backend_invocation
    }

    pub const fn semantic_witness(&self) -> SemanticWitnessIdentityV1 {
        self.parts.semantic_witness
    }

    pub const fn kernel_ir(&self) -> KernelIrIdentityV1 {
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

    pub const fn envelope(&self) -> WorkerV2EnvelopeIdentityV1 {
        self.parts.envelope
    }

    pub const fn identity(&self) -> CompilerTransactionEvidenceIdentityV1 {
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

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CompilerTransactionDecodeErrorV1> {
        decode_capsule(bytes, None)
    }

    /// Decodes a capsule only when it has the externally expected transaction identity.
    ///
    /// This detects a stale or substituted, otherwise well-formed capsule. The expected
    /// identity must itself come from an authenticated/currentness boundary.
    pub fn from_bytes_for_identity(
        bytes: &[u8],
        expected: CompilerTransactionEvidenceIdentityV1,
    ) -> Result<Self, CompilerTransactionDecodeErrorV1> {
        decode_capsule(bytes, Some(expected))
    }

    fn encode_prefix(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.encoded_len);
        bytes.extend_from_slice(&COMPILER_TRANSACTION_EVIDENCE_MAGIC_V1);
        bytes.extend_from_slice(&COMPILER_TRANSACTION_EVIDENCE_VERSION_V1.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&(self.encoded_len as u32).to_le_bytes());
        write_field(
            &mut bytes,
            SOURCE_ROOT_TAG,
            &encode_digest(self.parts.source_closure.root.digest()),
        );
        write_dependencies_field(&mut bytes, &self.parts.source_closure);
        write_features_field(&mut bytes, &self.parts.source_closure);
        write_tool_field(&mut bytes, RUSTC_TOOL_TAG, &self.parts.rustc_tool);
        write_field(
            &mut bytes,
            RUSTC_INVOCATION_TAG,
            &encode_digest(self.parts.rustc_invocation.digest()),
        );
        write_tool_field(&mut bytes, BACKEND_TOOL_TAG, &self.parts.backend_tool);
        write_field(
            &mut bytes,
            BACKEND_INVOCATION_TAG,
            &encode_digest(self.parts.backend_invocation.digest()),
        );
        write_field(
            &mut bytes,
            SEMANTIC_WITNESS_TAG,
            &encode_digest(self.parts.semantic_witness.digest()),
        );
        write_field(
            &mut bytes,
            KERNEL_IR_TAG,
            &encode_digest(self.parts.kernel_ir.digest()),
        );
        write_field(
            &mut bytes,
            WORKER_REQUEST_TAG,
            &encode_digest(self.parts.worker_request.digest()),
        );
        write_field(
            &mut bytes,
            WORKER_RESPONSE_TAG,
            &encode_digest(self.parts.worker_response.digest()),
        );
        write_field(&mut bytes, TARGET_TAG, self.parts.target.as_bytes());
        write_field(
            &mut bytes,
            RAW_HSACO_TAG,
            &encode_digest(self.parts.raw_hsaco.digest()),
        );
        write_field(
            &mut bytes,
            FINALIZED_HSACO_TAG,
            &encode_digest(self.parts.finalized_hsaco.digest()),
        );
        write_field(
            &mut bytes,
            ARTIFACT_TAG,
            &encode_digest(self.parts.artifact.digest()),
        );
        write_field(
            &mut bytes,
            ENVELOPE_TAG,
            &encode_digest(self.parts.envelope.digest()),
        );
        bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CompilerTransactionValidationErrorV1 {
    TooManyDependencies { max: usize },
    TooManyFeatures { max: usize },
    EvidenceTooLarge { max: usize },
    DuplicateDependency,
    DuplicateFeature,
    UnsupportedDigestAlgorithm { field: &'static str },
}

impl fmt::Display for CompilerTransactionValidationErrorV1 {
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
            Self::UnsupportedDigestAlgorithm { field } => {
                write!(formatter, "{field} must use SHA-256")
            }
        }
    }
}

impl std::error::Error for CompilerTransactionValidationErrorV1 {}

#[derive(Debug)]
#[non_exhaustive]
pub enum CompilerTransactionDecodeErrorV1 {
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
    NonCanonicalDependencyOrder,
    NonCanonicalFeatureOrder,
    FieldTrailingBytes {
        field: u8,
    },
    CapsuleIdentityMismatch,
    UnexpectedCapsuleIdentity,
    Validation(CompilerTransactionValidationErrorV1),
    NonCanonical,
}

impl fmt::Display for CompilerTransactionDecodeErrorV1 {
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
            Self::StringTooLong { field, value, max } => write!(formatter, "compiler transaction {field} length {value} exceeds {max}"),
            Self::InvalidUtf8 { field } => write!(formatter, "compiler transaction {field} is not UTF-8"),
            Self::InvalidText { field } => write!(formatter, "compiler transaction {field} is not canonical identity text"),
            Self::UnknownDigestAlgorithm(tag) => write!(formatter, "unknown compiler transaction digest algorithm {tag}"),
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

impl std::error::Error for CompilerTransactionDecodeErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Validation(error) => Some(error),
            _ => None,
        }
    }
}

fn validate_parts(
    parts: &CompilerTransactionEvidencePartsV1,
) -> Result<(), CompilerTransactionValidationErrorV1> {
    for (field, digest) in [
        ("source root", parts.source_closure.root.digest()),
        ("rustc executable", parts.rustc_tool.executable_digest()),
        (
            "rustc configuration",
            parts.rustc_tool.configuration_digest(),
        ),
        ("rustc invocation", parts.rustc_invocation.digest()),
        ("backend executable", parts.backend_tool.executable_digest()),
        (
            "backend configuration",
            parts.backend_tool.configuration_digest(),
        ),
        ("backend invocation", parts.backend_invocation.digest()),
        ("semantic witness", parts.semantic_witness.digest()),
        ("Kernel IR", parts.kernel_ir.digest()),
        ("Worker V2 request", parts.worker_request.digest()),
        ("Worker V2 response", parts.worker_response.digest()),
        ("raw HSACO", parts.raw_hsaco.digest()),
        ("finalized HSACO", parts.finalized_hsaco.digest()),
        ("artifact", parts.artifact.digest()),
        ("Worker V2 envelope", parts.envelope.digest()),
    ] {
        require_sha256(field, digest)?;
    }
    for dependency in &parts.source_closure.dependencies {
        require_sha256("source dependency", dependency.identity)?;
    }
    Ok(())
}

fn require_sha256(
    field: &'static str,
    digest: PayloadDigest,
) -> Result<(), CompilerTransactionValidationErrorV1> {
    if digest.algorithm() == DigestAlgorithm::Sha256 {
        Ok(())
    } else {
        Err(CompilerTransactionValidationErrorV1::UnsupportedDigestAlgorithm { field })
    }
}

fn checked_encoded_total_len(parts: &CompilerTransactionEvidencePartsV1) -> Option<usize> {
    HEADER_BYTES
        .checked_add(FIELD_HEADER_BYTES.checked_mul(usize::from(LAST_FIELD_TAG))?)?
        .checked_add(DIGEST_BYTES.checked_mul(11)?)?
        .checked_add(32_usize.checked_mul(2)?)?
        .checked_add(checked_dependencies_encoded_len(&parts.source_closure)?)?
        .checked_add(checked_features_encoded_len(&parts.source_closure)?)?
        .checked_add(checked_tool_encoded_len(&parts.rustc_tool)?)?
        .checked_add(checked_tool_encoded_len(&parts.backend_tool)?)
}

fn checked_dependencies_encoded_len(source: &CompilerSourceClosureV1) -> Option<usize> {
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

fn checked_features_encoded_len(source: &CompilerSourceClosureV1) -> Option<usize> {
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

fn write_dependencies_field(bytes: &mut Vec<u8>, source: &CompilerSourceClosureV1) {
    let payload_len = checked_dependencies_encoded_len(source)
        .expect("validated dependency field length must remain representable");
    write_field_header(bytes, DEPENDENCIES_TAG, payload_len);
    bytes.extend_from_slice(&(source.dependencies.len() as u16).to_le_bytes());
    for dependency in &source.dependencies {
        write_text(bytes, dependency.name.as_str());
        bytes.extend_from_slice(&encode_digest(dependency.identity));
    }
}

fn write_features_field(bytes: &mut Vec<u8>, source: &CompilerSourceClosureV1) {
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

fn calculate_capsule_identity(prefix: &[u8]) -> CompilerTransactionEvidenceIdentityV1 {
    let mut hasher = Sha256::new();
    hasher.update(CAPSULE_IDENTITY_DOMAIN);
    hasher.update(prefix);
    CompilerTransactionEvidenceIdentityV1(hasher.finalize().into())
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
    expected: Option<CompilerTransactionEvidenceIdentityV1>,
) -> Result<CompilerTransactionEvidenceCapsuleV1, CompilerTransactionDecodeErrorV1> {
    if bytes.len() > MAX_COMPILER_TRANSACTION_EVIDENCE_BYTES_V1 {
        return Err(CompilerTransactionDecodeErrorV1::TooLarge {
            max: MAX_COMPILER_TRANSACTION_EVIDENCE_BYTES_V1,
        });
    }
    let mut reader = Reader::new(bytes);
    if reader.array::<8>()? != COMPILER_TRANSACTION_EVIDENCE_MAGIC_V1 {
        return Err(CompilerTransactionDecodeErrorV1::InvalidMagic);
    }
    let version = reader.u16()?;
    if version != COMPILER_TRANSACTION_EVIDENCE_VERSION_V1 {
        return Err(CompilerTransactionDecodeErrorV1::UnknownVersion(version));
    }
    let flags = reader.u16()?;
    if flags != 0 {
        return Err(CompilerTransactionDecodeErrorV1::UnsupportedFlags(flags));
    }
    let total_len = reader.u32()? as usize;
    if total_len > bytes.len() {
        return Err(CompilerTransactionDecodeErrorV1::Truncated);
    }
    if total_len < bytes.len() {
        return Err(CompilerTransactionDecodeErrorV1::TrailingBytes);
    }
    if total_len < HEADER_BYTES {
        return Err(CompilerTransactionDecodeErrorV1::InvalidTotalLength);
    }

    let root = SourceRootIdentityV1(decode_digest_field(&mut reader, SOURCE_ROOT_TAG)?);
    let dependencies =
        decode_dependencies(reader.field(DEPENDENCIES_TAG, MAX_DEPENDENCIES_FIELD_BYTES)?)?;
    let features = decode_features(reader.field(FEATURES_TAG, MAX_FEATURES_FIELD_BYTES)?)?;
    let rustc_tool = decode_tool(
        reader.field(RUSTC_TOOL_TAG, MAX_TOOL_FIELD_BYTES)?,
        RUSTC_TOOL_TAG,
    )?;
    let rustc_invocation =
        RustcInvocationIdentityV1(decode_digest_field(&mut reader, RUSTC_INVOCATION_TAG)?);
    let backend_tool = decode_tool(
        reader.field(BACKEND_TOOL_TAG, MAX_TOOL_FIELD_BYTES)?,
        BACKEND_TOOL_TAG,
    )?;
    let backend_invocation =
        BackendInvocationIdentityV1(decode_digest_field(&mut reader, BACKEND_INVOCATION_TAG)?);
    let semantic_witness =
        SemanticWitnessIdentityV1(decode_digest_field(&mut reader, SEMANTIC_WITNESS_TAG)?);
    let kernel_ir = KernelIrIdentityV1(decode_digest_field(&mut reader, KERNEL_IR_TAG)?);
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
    let envelope = WorkerV2EnvelopeIdentityV1(decode_digest_field(&mut reader, ENVELOPE_TAG)?);

    let identity_prefix_len = reader.position();
    let encoded_identity =
        CompilerTransactionEvidenceIdentityV1::from_bytes(decode_fixed_field::<32>(
            &mut reader,
            CAPSULE_IDENTITY_TAG,
        )?);
    if !reader.is_empty() {
        return Err(CompilerTransactionDecodeErrorV1::TrailingBytes);
    }
    let calculated_identity = calculate_capsule_identity(&bytes[..identity_prefix_len]);
    if encoded_identity != calculated_identity {
        return Err(CompilerTransactionDecodeErrorV1::CapsuleIdentityMismatch);
    }
    if expected.is_some_and(|value| value != calculated_identity) {
        return Err(CompilerTransactionDecodeErrorV1::UnexpectedCapsuleIdentity);
    }

    let source_closure = CompilerSourceClosureV1::new(root, dependencies, features)
        .map_err(CompilerTransactionDecodeErrorV1::Validation)?;
    let capsule = CompilerTransactionEvidenceCapsuleV1::new(CompilerTransactionEvidencePartsV1 {
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
        envelope,
    })
    .map_err(CompilerTransactionDecodeErrorV1::Validation)?;
    if capsule.identity != encoded_identity || capsule.to_bytes() != bytes {
        return Err(CompilerTransactionDecodeErrorV1::NonCanonical);
    }
    Ok(capsule)
}

fn decode_dependencies(
    bytes: &[u8],
) -> Result<Vec<CompilerSourceDependencyV1>, CompilerTransactionDecodeErrorV1> {
    let mut reader = Reader::new(bytes);
    let count = usize::from(reader.u16()?);
    if count > MAX_COMPILER_TRANSACTION_DEPENDENCIES_V1 {
        return Err(CompilerTransactionDecodeErrorV1::CountOutOfRange {
            field: "dependency",
            value: count as u64,
            max: MAX_COMPILER_TRANSACTION_DEPENDENCIES_V1,
        });
    }
    let mut dependencies = Vec::with_capacity(count);
    for _ in 0..count {
        let name = reader.identity_text("dependency name")?;
        let identity = reader.digest()?;
        if dependencies
            .last()
            .is_some_and(|previous: &CompilerSourceDependencyV1| {
                previous.name.as_str() >= name.as_str()
            })
        {
            return Err(CompilerTransactionDecodeErrorV1::NonCanonicalDependencyOrder);
        }
        dependencies.push(
            CompilerSourceDependencyV1::new(name, identity)
                .map_err(CompilerTransactionDecodeErrorV1::Validation)?,
        );
    }
    if !reader.is_empty() {
        return Err(CompilerTransactionDecodeErrorV1::FieldTrailingBytes {
            field: DEPENDENCIES_TAG,
        });
    }
    Ok(dependencies)
}

fn decode_features(bytes: &[u8]) -> Result<Vec<IdentityText>, CompilerTransactionDecodeErrorV1> {
    let mut reader = Reader::new(bytes);
    let count = usize::from(reader.u16()?);
    if count > MAX_COMPILER_TRANSACTION_FEATURES_V1 {
        return Err(CompilerTransactionDecodeErrorV1::CountOutOfRange {
            field: "feature",
            value: count as u64,
            max: MAX_COMPILER_TRANSACTION_FEATURES_V1,
        });
    }
    let mut features = Vec::with_capacity(count);
    for _ in 0..count {
        let feature = reader.identity_text("feature")?;
        if features
            .last()
            .is_some_and(|previous: &IdentityText| previous.as_str() >= feature.as_str())
        {
            return Err(CompilerTransactionDecodeErrorV1::NonCanonicalFeatureOrder);
        }
        features.push(feature);
    }
    if !reader.is_empty() {
        return Err(CompilerTransactionDecodeErrorV1::FieldTrailingBytes {
            field: FEATURES_TAG,
        });
    }
    Ok(features)
}

fn decode_tool(
    bytes: &[u8],
    field: u8,
) -> Result<MeasuredToolIdentity, CompilerTransactionDecodeErrorV1> {
    let mut reader = Reader::new(bytes);
    let name = reader.identity_text("tool name")?;
    let version = reader.identity_text("tool version")?;
    let executable = reader.digest()?;
    let configuration = reader.digest()?;
    if !reader.is_empty() {
        return Err(CompilerTransactionDecodeErrorV1::FieldTrailingBytes { field });
    }
    Ok(MeasuredToolIdentity::new(
        name,
        version,
        executable,
        configuration,
    ))
}

fn decode_digest_field(
    reader: &mut Reader<'_>,
    tag: u8,
) -> Result<PayloadDigest, CompilerTransactionDecodeErrorV1> {
    let bytes = reader.field(tag, DIGEST_BYTES)?;
    if bytes.len() != DIGEST_BYTES {
        return Err(CompilerTransactionDecodeErrorV1::LengthOutOfRange {
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
) -> Result<[u8; N], CompilerTransactionDecodeErrorV1> {
    let bytes = reader.field(tag, N)?;
    if bytes.len() != N {
        return Err(CompilerTransactionDecodeErrorV1::LengthOutOfRange {
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

    const fn position(&self) -> usize {
        self.original_len - self.remaining.len()
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], CompilerTransactionDecodeErrorV1> {
        if self.remaining.len() < count {
            return Err(CompilerTransactionDecodeErrorV1::Truncated);
        }
        let (value, remaining) = self.remaining.split_at(count);
        self.remaining = remaining;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], CompilerTransactionDecodeErrorV1> {
        let mut value = [0; N];
        value.copy_from_slice(self.take(N)?);
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8, CompilerTransactionDecodeErrorV1> {
        Ok(self.array::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, CompilerTransactionDecodeErrorV1> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, CompilerTransactionDecodeErrorV1> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn field(
        &mut self,
        expected: u8,
        max: usize,
    ) -> Result<&'a [u8], CompilerTransactionDecodeErrorV1> {
        let actual = self.u8()?;
        if actual != expected {
            return if actual > LAST_FIELD_TAG || actual == 0 {
                Err(CompilerTransactionDecodeErrorV1::UnknownField(actual))
            } else if actual < expected {
                Err(CompilerTransactionDecodeErrorV1::DuplicateField(actual))
            } else {
                Err(CompilerTransactionDecodeErrorV1::UnexpectedField { expected, actual })
            };
        }
        let length = u64::from(self.u32()?);
        if length > max as u64 {
            return Err(CompilerTransactionDecodeErrorV1::LengthOutOfRange {
                field: actual,
                value: length,
                max,
            });
        }
        self.take(length as usize)
    }

    fn digest(&mut self) -> Result<PayloadDigest, CompilerTransactionDecodeErrorV1> {
        let algorithm = self.u8()?;
        if algorithm != SHA256_TAG {
            return Err(CompilerTransactionDecodeErrorV1::UnknownDigestAlgorithm(
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
    ) -> Result<IdentityText, CompilerTransactionDecodeErrorV1> {
        let length = usize::from(self.u16()?);
        if length > MAX_IDENTITY_TEXT_BYTES {
            return Err(CompilerTransactionDecodeErrorV1::StringTooLong {
                field,
                value: length,
                max: MAX_IDENTITY_TEXT_BYTES,
            });
        }
        let value = str::from_utf8(self.take(length)?)
            .map_err(|_| CompilerTransactionDecodeErrorV1::InvalidUtf8 { field })?;
        IdentityText::new(value)
            .map_err(|_| CompilerTransactionDecodeErrorV1::InvalidText { field })
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

    fn maximum_source_set_parts() -> CompilerTransactionEvidencePartsV1 {
        let dependencies = (0..MAX_COMPILER_TRANSACTION_DEPENDENCIES_V1)
            .map(|index| {
                CompilerSourceDependencyV1::new(
                    maximum_text(&format!("dependency-{index:04}:")),
                    digest(index as u8),
                )
                .unwrap()
            })
            .collect();
        let features = (0..MAX_COMPILER_TRANSACTION_FEATURES_V1)
            .map(|index| maximum_text(&format!("feature-{index:04}:")))
            .collect();
        let source_closure = CompilerSourceClosureV1::new(
            SourceRootIdentityV1::from_sha256([0x10; 32]),
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
        CompilerTransactionEvidencePartsV1 {
            source_closure,
            rustc_tool: measured_tool("rustc", 0x20),
            rustc_invocation: RustcInvocationIdentityV1::from_sha256([0x22; 32]),
            backend_tool: measured_tool("backend", 0x30),
            backend_invocation: BackendInvocationIdentityV1::from_sha256([0x32; 32]),
            semantic_witness: SemanticWitnessIdentityV1::from_sha256([0x40; 32]),
            kernel_ir: KernelIrIdentityV1::from_sha256([0x41; 32]),
            worker_request: DirectLinkRequestIdentityV1::new(digest(0x50)),
            worker_response: DirectLinkResponseIdentityV1::new(digest(0x51)),
            target: TargetIdentityV1::from_bytes([0x52; 32]),
            raw_hsaco: DirectLinkLinkedOutputIdentityV1::new(digest(0x60)),
            finalized_hsaco: DirectLinkFinalizedPayloadIdentityV1::new(digest(0x61)),
            artifact: DirectLinkContainerIdentityV1::new(digest(0x62)),
            envelope: WorkerV2EnvelopeIdentityV1::from_sha256([0x63; 32]),
        }
    }

    #[test]
    fn constructor_rejects_legal_source_set_over_aggregate_limit_before_encoding() {
        let parts = maximum_source_set_parts();
        let encoded_len = checked_encoded_total_len(&parts).unwrap();
        let test_limit = encoded_len - 1;

        assert!(matches!(
            CompilerTransactionEvidenceCapsuleV1::new_with_max_encoded_bytes(parts, test_limit),
            Err(CompilerTransactionValidationErrorV1::EvidenceTooLarge { max })
                if max == test_limit
        ));
    }

    #[test]
    fn maximum_current_public_source_set_remains_within_two_mibibytes() {
        let parts = maximum_source_set_parts();
        let encoded_len = checked_encoded_total_len(&parts).unwrap();
        assert_eq!(encoded_len, 1_457_824);
        assert!(encoded_len < MAX_COMPILER_TRANSACTION_EVIDENCE_BYTES_V1);

        let capsule = CompilerTransactionEvidenceCapsuleV1::new(parts).unwrap();
        assert_eq!(capsule.to_bytes().len(), encoded_len);
    }
}
