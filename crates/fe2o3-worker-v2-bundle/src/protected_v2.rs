use std::fmt;

use fe2o3_artifact_transaction::{
    BackendPublicationReceiptV2, DurablePublishedClaimCodecErrorV2, DurablePublishedHsacoClaimV2,
    MAX_DURABLE_FINALIZED_ARTIFACT_BYTES, MAX_DURABLE_PUBLISHED_HSACO_CLAIM_BYTES_V2,
    WorkerV2PublicationIntentIdentityV2,
};
use fe2o3_artifacts::{
    AbiKind, Access, AddressSpace, AliasClass, ArgumentOwnership, ArtifactContainerV1, BlockSize,
    BundleDecodeError, BundleIndexV1, Capability, ContainerDecodeError, DigestAlgorithm,
    DigestBytes, DirectLinkBundleEvidenceV1, DirectLinkDecodeError, Endianness,
    MAX_BUNDLE_INDEX_BYTES, MAX_CONTAINER_BYTES, MAX_DIRECT_LINK_EVIDENCE_BYTES, MAX_KERNELS,
    MAX_PROOF_RECORD_BYTES, Mutability, PayloadDigest, PointerWidth, ProofDecodeError,
    ProofRecordV1, ScalarType,
};
use fe2o3_build_authority::{CompilerClosureErrorV2, CompilerClosureV2};
use fe2o3_hsaco_finalize::InspectedProtectedRawWorkerV2HsacoIdentityV1;
use fe2o3_kernel_descriptor::{
    CodeObjectVersion, DecodeError as DescriptorDecodeError, DeviceTargetV1,
    MAX_DESCRIPTOR_TABLE_BYTES, decode_device_descriptor_table_v1,
};
use sha2::{Digest, Sha256};

use crate::model::{PublicationClaimViewV1, WorkerV2EnvelopeComponentsV1};
use crate::{
    DescriptorLineageV1, EnvelopeValidationError, ExactRawHsacoV1,
    MAX_WORKER_V2_PROOF_EVIDENCE_BYTES, MAX_WORKER_V2_RAW_HSACO_BYTES,
};

pub const WORKER_V2_FINAL_ARTIFACT_EVIDENCE_MAGIC_V2: [u8; 8] = *b"FE2W2F2\0";
pub const WORKER_V2_FINAL_ARTIFACT_EVIDENCE_VERSION_V2: u16 = 2;
pub const WORKER_V2_LOAD_ENVELOPE_MAGIC_V2: [u8; 8] = *b"FE2W2B2\0";
pub const WORKER_V2_LOAD_ENVELOPE_VERSION_V2: u16 = 2;

const FINAL_ARTIFACT_HEADER_BYTES_V2: usize = 24;
const COMPILER_CLOSURE_BYTES_V2: usize = (6 * 32) + 2 + 32;
const FINAL_ARTIFACT_FIXED_BODY_BYTES_V2: usize =
    FINAL_ARTIFACT_HEADER_BYTES_V2 + COMPILER_CLOSURE_BYTES_V2 + 32 + 32 + 8 + (6 * 32);
const LOAD_ENVELOPE_HEADER_BYTES_V2: usize = 77;
const CHECKSUM_BYTES_V2: usize = 32;
const MAX_TARGET_TEXT_BYTES_V2: usize = 256;

pub const MAX_WORKER_V2_FINAL_ARTIFACT_EVIDENCE_BYTES_V2: usize = FINAL_ARTIFACT_FIXED_BODY_BYTES_V2
    + MAX_TARGET_TEXT_BYTES_V2
    + MAX_DURABLE_PUBLISHED_HSACO_CLAIM_BYTES_V2
    + CHECKSUM_BYTES_V2;
pub const MAX_WORKER_V2_LOAD_ENVELOPE_BYTES_V2: usize = MAX_CONTAINER_BYTES
    + MAX_BUNDLE_INDEX_BYTES
    + MAX_DIRECT_LINK_EVIDENCE_BYTES
    + MAX_DESCRIPTOR_TABLE_BYTES
    + MAX_WORKER_V2_RAW_HSACO_BYTES
    + MAX_WORKER_V2_PROOF_EVIDENCE_BYTES
    + MAX_WORKER_V2_FINAL_ARTIFACT_EVIDENCE_BYTES_V2
    + 4096;

const FINAL_ARTIFACT_CHECKSUM_DOMAIN_V2: &[u8] =
    b"FE2O3/PROTECTED-WORKER-V2/FINAL-ARTIFACT-EVIDENCE-CHECKSUM/V2\0";
const FINAL_ARTIFACT_IDENTITY_DOMAIN_V2: &[u8] =
    b"FE2O3/PROTECTED-WORKER-V2/FINAL-ARTIFACT-EVIDENCE-IDENTITY/V2\0";
const LOAD_ENVELOPE_CHECKSUM_DOMAIN_V2: &[u8] =
    b"FE2O3/PROTECTED-WORKER-V2/LOAD-ENVELOPE-CHECKSUM/V2\0";
const LOAD_ENVELOPE_IDENTITY_DOMAIN_V2: &[u8] =
    b"FE2O3/PROTECTED-WORKER-V2/LOAD-ENVELOPE-IDENTITY/V2\0";
const TARGET_IDENTITY_DOMAIN_V2: &[u8] = b"FE2O3/PROTECTED-WORKER-V2/TARGET-IDENTITY/V2\0";
const ABI_IDENTITY_DOMAIN_V2: &[u8] = b"FE2O3/PROTECTED-WORKER-V2/ABI-IDENTITY/V2\0";
const DESCRIPTOR_IDENTITY_DOMAIN_V2: &[u8] = b"FE2O3/PROTECTED-WORKER-V2/DESCRIPTOR-IDENTITY/V2\0";
const SYMBOL_IDENTITY_DOMAIN_V2: &[u8] = b"FE2O3/PROTECTED-WORKER-V2/SYMBOL-IDENTITY/V2\0";
const RESOURCE_IDENTITY_DOMAIN_V2: &[u8] = b"FE2O3/PROTECTED-WORKER-V2/RESOURCE-IDENTITY/V2\0";
const PROOF_IDENTITY_DOMAIN_V2: &[u8] = b"FE2O3/PROTECTED-WORKER-V2/PROOF-EVIDENCE-IDENTITY/V2\0";

macro_rules! identity_type {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name([u8; 32]);

        impl $name {
            pub const fn as_bytes(self) -> [u8; 32] {
                self.0
            }
        }
    };
}

identity_type!(
    WorkerV2FinalArtifactEvidenceIdentityV2,
    "SHA-256 identity of one canonical protected final-artifact evidence record."
);
identity_type!(
    WorkerV2LoadEnvelopeIdentityV2,
    "SHA-256 identity of one canonical protected Worker V2 load envelope."
);
identity_type!(
    WorkerV2TargetIdentityV2,
    "Domain-separated identity of the exact manifest target."
);
identity_type!(
    WorkerV2AbiIdentityV2,
    "Domain-separated identity of every exact manifest kernel ABI."
);
identity_type!(
    WorkerV2DescriptorIdentityV2,
    "Domain-separated identity of the exact canonical descriptor table."
);
identity_type!(
    WorkerV2SymbolIdentityV2,
    "Domain-separated identity of every exact manifest kernel name and symbol."
);
identity_type!(
    WorkerV2ResourceIdentityV2,
    "Domain-separated identity of every exact manifest kernel resource contract."
);

/// Exact SHA-256 identity and length of the final bytes carried by the artifact container.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerV2FinalBytesIdentityV2 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl WorkerV2FinalBytesIdentityV2 {
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    pub fn matches(self, bytes: &[u8]) -> bool {
        u64::try_from(bytes.len()) == Ok(self.byte_len)
            && self.sha256 == Sha256::digest(bytes).as_slice()
    }

    fn from_exact_bytes(bytes: &[u8]) -> Result<Self, WorkerV2FinalArtifactValidationErrorV2> {
        if bytes.is_empty() || bytes.len() > MAX_DURABLE_FINALIZED_ARTIFACT_BYTES {
            return Err(WorkerV2FinalArtifactValidationErrorV2::FinalBytesSize {
                actual: bytes.len(),
                max: MAX_DURABLE_FINALIZED_ARTIFACT_BYTES,
            });
        }
        Ok(Self {
            sha256: Sha256::digest(bytes).into(),
            byte_len: bytes.len() as u64,
        })
    }
}

/// Which independently typed evidence identity closes the protected artifact record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerV2ProofOrInspectionKindV2 {
    ProofRecords,
    ProtectedInspection,
}

/// Exact proof closure or protected raw-HSACO inspection identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerV2ProofOrInspectionIdentityV2 {
    kind: WorkerV2ProofOrInspectionKindV2,
    identity: [u8; 32],
}

impl WorkerV2ProofOrInspectionIdentityV2 {
    pub fn from_proof_records(proof_records: &[ProofRecordV1]) -> Self {
        Self {
            kind: WorkerV2ProofOrInspectionKindV2::ProofRecords,
            identity: derive_proof_identity_v2(proof_records),
        }
    }

    pub const fn from_protected_inspection(
        identity: InspectedProtectedRawWorkerV2HsacoIdentityV1,
    ) -> Self {
        Self {
            kind: WorkerV2ProofOrInspectionKindV2::ProtectedInspection,
            identity: *identity.as_bytes(),
        }
    }

    pub const fn kind(self) -> WorkerV2ProofOrInspectionKindV2 {
        self.kind
    }

    pub const fn as_bytes(self) -> [u8; 32] {
        self.identity
    }

    const fn from_wire(kind: WorkerV2ProofOrInspectionKindV2, identity: [u8; 32]) -> Self {
        Self { kind, identity }
    }
}

/// Canonical inert evidence for one exact closure-protected final artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerV2FinalArtifactEvidenceV2 {
    compiler_closure: CompilerClosureV2,
    publication_intent: WorkerV2PublicationIntentIdentityV2,
    backend_receipt: BackendPublicationReceiptV2,
    published_claim: DurablePublishedHsacoClaimV2,
    final_bytes: WorkerV2FinalBytesIdentityV2,
    target: DeviceTargetV1,
    code_object_version: CodeObjectVersion,
    target_identity: WorkerV2TargetIdentityV2,
    abi_identity: WorkerV2AbiIdentityV2,
    descriptor_identity: WorkerV2DescriptorIdentityV2,
    symbol_identity: WorkerV2SymbolIdentityV2,
    resource_identity: WorkerV2ResourceIdentityV2,
    proof_or_inspection_identity: WorkerV2ProofOrInspectionIdentityV2,
}

impl WorkerV2FinalArtifactEvidenceV2 {
    #[allow(clippy::too_many_arguments)]
    pub fn from_proof_records(
        container: &ArtifactContainerV1,
        descriptor_lineage: &DescriptorLineageV1,
        proof_records: &[ProofRecordV1],
        compiler_closure: CompilerClosureV2,
        publication_intent: WorkerV2PublicationIntentIdentityV2,
        backend_receipt: BackendPublicationReceiptV2,
        published_claim: DurablePublishedHsacoClaimV2,
    ) -> Result<Self, WorkerV2FinalArtifactValidationErrorV2> {
        Self::new(
            container,
            descriptor_lineage,
            compiler_closure,
            publication_intent,
            backend_receipt,
            published_claim,
            WorkerV2ProofOrInspectionIdentityV2::from_proof_records(proof_records),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_protected_inspection(
        container: &ArtifactContainerV1,
        descriptor_lineage: &DescriptorLineageV1,
        protected_inspection: InspectedProtectedRawWorkerV2HsacoIdentityV1,
        compiler_closure: CompilerClosureV2,
        publication_intent: WorkerV2PublicationIntentIdentityV2,
        backend_receipt: BackendPublicationReceiptV2,
        published_claim: DurablePublishedHsacoClaimV2,
    ) -> Result<Self, WorkerV2FinalArtifactValidationErrorV2> {
        Self::new(
            container,
            descriptor_lineage,
            compiler_closure,
            publication_intent,
            backend_receipt,
            published_claim,
            WorkerV2ProofOrInspectionIdentityV2::from_protected_inspection(protected_inspection),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new(
        container: &ArtifactContainerV1,
        descriptor_lineage: &DescriptorLineageV1,
        compiler_closure: CompilerClosureV2,
        publication_intent: WorkerV2PublicationIntentIdentityV2,
        backend_receipt: BackendPublicationReceiptV2,
        published_claim: DurablePublishedHsacoClaimV2,
        proof_or_inspection_identity: WorkerV2ProofOrInspectionIdentityV2,
    ) -> Result<Self, WorkerV2FinalArtifactValidationErrorV2> {
        let final_output = backend_receipt.finalized_output_identity();
        let final_payload = container
            .payloads()
            .iter()
            .find(|payload| {
                payload.digest().algorithm() == DigestAlgorithm::Sha256
                    && payload.digest().bytes().as_bytes() == &final_output
            })
            .ok_or(WorkerV2FinalArtifactValidationErrorV2::FinalBytesIdentityMismatch)?;
        let table = descriptor_lineage.table();
        let value = Self {
            compiler_closure,
            publication_intent,
            backend_receipt,
            published_claim,
            final_bytes: WorkerV2FinalBytesIdentityV2::from_exact_bytes(final_payload.bytes())?,
            target: table.device_target(),
            code_object_version: table.code_object_version(),
            target_identity: WorkerV2TargetIdentityV2(derive_target_identity_v2(container)),
            abi_identity: WorkerV2AbiIdentityV2(derive_abi_identity_v2(container)),
            descriptor_identity: WorkerV2DescriptorIdentityV2(hash_domain_bytes(
                DESCRIPTOR_IDENTITY_DOMAIN_V2,
                &descriptor_lineage.canonical_bytes(),
            )),
            symbol_identity: WorkerV2SymbolIdentityV2(derive_symbol_identity_v2(container)),
            resource_identity: WorkerV2ResourceIdentityV2(derive_resource_identity_v2(container)),
            proof_or_inspection_identity,
        };
        value.validate_self()?;
        Ok(value)
    }

    pub const fn compiler_closure(&self) -> CompilerClosureV2 {
        self.compiler_closure
    }

    pub const fn publication_intent_identity(&self) -> WorkerV2PublicationIntentIdentityV2 {
        self.publication_intent
    }

    pub const fn backend_receipt(&self) -> BackendPublicationReceiptV2 {
        self.backend_receipt
    }

    pub const fn published_claim(&self) -> &DurablePublishedHsacoClaimV2 {
        &self.published_claim
    }

    pub const fn final_bytes_identity(&self) -> WorkerV2FinalBytesIdentityV2 {
        self.final_bytes
    }

    pub const fn target(&self) -> DeviceTargetV1 {
        self.target
    }

    pub const fn code_object_version(&self) -> CodeObjectVersion {
        self.code_object_version
    }

    pub const fn target_identity(&self) -> WorkerV2TargetIdentityV2 {
        self.target_identity
    }

    pub const fn abi_identity(&self) -> WorkerV2AbiIdentityV2 {
        self.abi_identity
    }

    pub const fn descriptor_identity(&self) -> WorkerV2DescriptorIdentityV2 {
        self.descriptor_identity
    }

    pub const fn symbol_identity(&self) -> WorkerV2SymbolIdentityV2 {
        self.symbol_identity
    }

    pub const fn resource_identity(&self) -> WorkerV2ResourceIdentityV2 {
        self.resource_identity
    }

    pub const fn proof_or_inspection_identity(&self) -> WorkerV2ProofOrInspectionIdentityV2 {
        self.proof_or_inspection_identity
    }

    pub fn identity(&self) -> WorkerV2FinalArtifactEvidenceIdentityV2 {
        WorkerV2FinalArtifactEvidenceIdentityV2(hash_domain_bytes(
            FINAL_ARTIFACT_IDENTITY_DOMAIN_V2,
            &self.to_bytes(),
        ))
    }

    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }

    pub const fn grants_proof_authority(&self) -> bool {
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

    fn validate_self(&self) -> Result<(), WorkerV2FinalArtifactValidationErrorV2> {
        if self.publication_intent.as_bytes() == [0; 32] {
            return Err(WorkerV2FinalArtifactValidationErrorV2::ZeroPublicationIntent);
        }
        if self.backend_receipt != self.published_claim.receipt() {
            return Err(WorkerV2FinalArtifactValidationErrorV2::BackendReceiptMismatch);
        }
        if self.compiler_closure != self.backend_receipt.compiler_closure()
            || self.compiler_closure != self.published_claim.compiler_closure()
        {
            return Err(WorkerV2FinalArtifactValidationErrorV2::CompilerClosureMismatch);
        }
        if self.final_bytes.byte_len == 0
            || self.final_bytes.byte_len > MAX_DURABLE_FINALIZED_ARTIFACT_BYTES as u64
        {
            return Err(WorkerV2FinalArtifactValidationErrorV2::FinalBytesSize {
                actual: usize::try_from(self.final_bytes.byte_len).unwrap_or(usize::MAX),
                max: MAX_DURABLE_FINALIZED_ARTIFACT_BYTES,
            });
        }
        if self.backend_receipt.finalized_output_identity() != self.final_bytes.sha256 {
            return Err(WorkerV2FinalArtifactValidationErrorV2::FinalBytesIdentityMismatch);
        }
        if self.proof_or_inspection_identity.identity == [0; 32] {
            return Err(WorkerV2FinalArtifactValidationErrorV2::ProofOrInspectionIdentityMismatch);
        }
        self.published_claim
            .encode_canonical()
            .map_err(WorkerV2FinalArtifactValidationErrorV2::PublishedClaim)?;
        Ok(())
    }

    fn validate_against(
        &self,
        components: &WorkerV2EnvelopeComponentsV1,
    ) -> Result<(), WorkerV2FinalArtifactValidationErrorV2> {
        self.validate_self()?;
        if !self.final_bytes.matches(components.finalized_payload()) {
            return Err(WorkerV2FinalArtifactValidationErrorV2::FinalBytesIdentityMismatch);
        }
        let table = components.descriptor_lineage().table();
        if self.target != table.device_target() {
            return Err(WorkerV2FinalArtifactValidationErrorV2::TargetMismatch);
        }
        if self.code_object_version != table.code_object_version() {
            return Err(WorkerV2FinalArtifactValidationErrorV2::CodeObjectVersionMismatch);
        }
        let container = components.container();
        let expected = [
            (
                self.target_identity.0 == derive_target_identity_v2(container),
                WorkerV2FinalArtifactFieldV2::Target,
            ),
            (
                self.abi_identity.0 == derive_abi_identity_v2(container),
                WorkerV2FinalArtifactFieldV2::Abi,
            ),
            (
                self.descriptor_identity.0
                    == hash_domain_bytes(
                        DESCRIPTOR_IDENTITY_DOMAIN_V2,
                        &components.descriptor_lineage().canonical_bytes(),
                    ),
                WorkerV2FinalArtifactFieldV2::Descriptor,
            ),
            (
                self.symbol_identity.0 == derive_symbol_identity_v2(container),
                WorkerV2FinalArtifactFieldV2::Symbols,
            ),
            (
                self.resource_identity.0 == derive_resource_identity_v2(container),
                WorkerV2FinalArtifactFieldV2::Resources,
            ),
        ];
        for (matches, field) in expected {
            if !matches {
                return Err(WorkerV2FinalArtifactValidationErrorV2::FacetIdentityMismatch(field));
            }
        }
        if self.proof_or_inspection_identity.kind == WorkerV2ProofOrInspectionKindV2::ProofRecords
            && self.proof_or_inspection_identity.identity
                != derive_proof_identity_v2(components.proof_records())
        {
            return Err(WorkerV2FinalArtifactValidationErrorV2::ProofOrInspectionIdentityMismatch);
        }
        Ok(())
    }
}

/// Protected V2 envelope retaining the existing required capsule inputs and exact final evidence.
#[derive(Debug, Eq, PartialEq)]
pub struct WorkerV2LoadEnvelopeV2 {
    components: WorkerV2EnvelopeComponentsV1,
    final_artifact_evidence: WorkerV2FinalArtifactEvidenceV2,
}

impl WorkerV2LoadEnvelopeV2 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        container: ArtifactContainerV1,
        bundle_index: BundleIndexV1,
        direct_link_evidence: DirectLinkBundleEvidenceV1,
        descriptor_lineage: DescriptorLineageV1,
        proof_records: Vec<ProofRecordV1>,
        raw_hsaco: ExactRawHsacoV1,
        final_artifact_evidence: WorkerV2FinalArtifactEvidenceV2,
    ) -> Result<Self, WorkerV2LoadEnvelopeValidationErrorV2> {
        let claim = final_artifact_evidence.published_claim();
        let components = WorkerV2EnvelopeComponentsV1::new(
            container,
            bundle_index,
            direct_link_evidence,
            descriptor_lineage,
            proof_records,
            raw_hsaco,
            PublicationClaimViewV1::new(claim.plan(), claim.upstream_evidence()),
        )?;
        final_artifact_evidence.validate_against(&components)?;
        Ok(Self {
            components,
            final_artifact_evidence,
        })
    }

    pub const fn container(&self) -> &ArtifactContainerV1 {
        self.components.container()
    }

    pub const fn bundle_index(&self) -> &BundleIndexV1 {
        self.components.bundle_index()
    }

    pub const fn direct_link_evidence(&self) -> &DirectLinkBundleEvidenceV1 {
        self.components.direct_link_evidence()
    }

    pub const fn descriptor_lineage(&self) -> &DescriptorLineageV1 {
        self.components.descriptor_lineage()
    }

    pub fn proof_records(&self) -> &[ProofRecordV1] {
        self.components.proof_records()
    }

    pub const fn raw_hsaco(&self) -> &ExactRawHsacoV1 {
        self.components.raw_hsaco()
    }

    pub const fn final_artifact_evidence(&self) -> &WorkerV2FinalArtifactEvidenceV2 {
        &self.final_artifact_evidence
    }

    pub fn finalized_payload(&self) -> &[u8] {
        self.components.finalized_payload()
    }

    pub fn finalized_payload_identity(&self) -> PayloadDigest {
        self.components.finalized_payload_identity()
    }

    pub fn identity(&self) -> WorkerV2LoadEnvelopeIdentityV2 {
        WorkerV2LoadEnvelopeIdentityV2(hash_domain_bytes(
            LOAD_ENVELOPE_IDENTITY_DOMAIN_V2,
            &self.to_bytes(),
        ))
    }

    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }

    pub const fn grants_proof_authority(&self) -> bool {
        false
    }

    pub const fn grants_currentness_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerV2FinalArtifactFieldV2 {
    Target,
    Abi,
    Descriptor,
    Symbols,
    Resources,
}

#[derive(Debug)]
#[non_exhaustive]
pub enum WorkerV2FinalArtifactValidationErrorV2 {
    ZeroPublicationIntent,
    BackendReceiptMismatch,
    CompilerClosureMismatch,
    FinalBytesSize { actual: usize, max: usize },
    FinalBytesIdentityMismatch,
    TargetMismatch,
    CodeObjectVersionMismatch,
    FacetIdentityMismatch(WorkerV2FinalArtifactFieldV2),
    ProofOrInspectionIdentityMismatch,
    PublishedClaim(DurablePublishedClaimCodecErrorV2),
}

impl fmt::Display for WorkerV2FinalArtifactValidationErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroPublicationIntent => {
                formatter.write_str("protected publication-intent identity must be nonzero")
            }
            Self::BackendReceiptMismatch => formatter
                .write_str("protected backend receipt does not equal the durable V2 claim receipt"),
            Self::CompilerClosureMismatch => formatter.write_str(
                "protected compiler closure differs across final evidence, receipt, and claim",
            ),
            Self::FinalBytesSize { actual, max } => {
                write!(
                    formatter,
                    "final artifact size {actual} is outside 1..={max} bytes"
                )
            }
            Self::FinalBytesIdentityMismatch => {
                formatter.write_str("final artifact bytes identity does not match")
            }
            Self::TargetMismatch => {
                formatter.write_str("final artifact target does not match the descriptor")
            }
            Self::CodeObjectVersionMismatch => formatter
                .write_str("final artifact code-object version does not match the descriptor"),
            Self::FacetIdentityMismatch(field) => {
                write!(
                    formatter,
                    "final artifact {field:?} identity does not match"
                )
            }
            Self::ProofOrInspectionIdentityMismatch => {
                formatter.write_str("final artifact proof-or-inspection identity does not match")
            }
            Self::PublishedClaim(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for WorkerV2FinalArtifactValidationErrorV2 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::PublishedClaim(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum WorkerV2LoadEnvelopeValidationErrorV2 {
    Components(EnvelopeValidationError),
    FinalArtifact(WorkerV2FinalArtifactValidationErrorV2),
}

impl fmt::Display for WorkerV2LoadEnvelopeValidationErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Components(error) => error.fmt(formatter),
            Self::FinalArtifact(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for WorkerV2LoadEnvelopeValidationErrorV2 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Components(error) => Some(error),
            Self::FinalArtifact(error) => Some(error),
        }
    }
}

impl From<EnvelopeValidationError> for WorkerV2LoadEnvelopeValidationErrorV2 {
    fn from(value: EnvelopeValidationError) -> Self {
        Self::Components(value)
    }
}

impl From<WorkerV2FinalArtifactValidationErrorV2> for WorkerV2LoadEnvelopeValidationErrorV2 {
    fn from(value: WorkerV2FinalArtifactValidationErrorV2) -> Self {
        Self::FinalArtifact(value)
    }
}

fn hash_domain_bytes(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    digest.finalize().into()
}

fn push_len_bytes(bytes: &mut Vec<u8>, value: &[u8]) {
    bytes.extend_from_slice(&(value.len() as u64).to_le_bytes());
    bytes.extend_from_slice(value);
}

fn derive_target_identity_v2(container: &ArtifactContainerV1) -> [u8; 32] {
    let target = container.manifest().target();
    let mut bytes = Vec::new();
    push_len_bytes(&mut bytes, target.triple().as_str().as_bytes());
    push_len_bytes(&mut bytes, target.architecture().as_str().as_bytes());
    bytes.push(pointer_width_tag(target.pointer_width()));
    bytes.push(endianness_tag(target.endianness()));
    bytes.extend_from_slice(&(target.capabilities().len() as u16).to_le_bytes());
    for capability in target.capabilities() {
        bytes.extend_from_slice(&capability_tag(*capability).to_le_bytes());
    }
    hash_domain_bytes(TARGET_IDENTITY_DOMAIN_V2, &bytes)
}

fn derive_abi_identity_v2(container: &ArtifactContainerV1) -> [u8; 32] {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&(container.manifest().kernels().len() as u16).to_le_bytes());
    for kernel in container.manifest().kernels() {
        bytes.extend_from_slice(kernel.kernel_id().as_bytes());
        let abi = kernel.abi();
        bytes.extend_from_slice(&abi.size().to_le_bytes());
        bytes.extend_from_slice(&abi.alignment().to_le_bytes());
        bytes.push(pointer_width_tag(abi.pointer_width()));
        bytes.extend_from_slice(&(abi.fields().len() as u16).to_le_bytes());
        for field in abi.fields() {
            push_len_bytes(&mut bytes, field.name().as_str().as_bytes());
            bytes.extend_from_slice(&field.offset().to_le_bytes());
            bytes.extend_from_slice(&field.size().to_le_bytes());
            bytes.extend_from_slice(&field.alignment().to_le_bytes());
            match field.kind() {
                AbiKind::Scalar(value) => {
                    bytes.push(0);
                    bytes.push(scalar_tag(value));
                }
                AbiKind::Pointer {
                    pointee_size,
                    pointee_alignment,
                } => {
                    bytes.push(1);
                    bytes.extend_from_slice(&pointee_size.to_le_bytes());
                    bytes.extend_from_slice(&pointee_alignment.to_le_bytes());
                }
                AbiKind::Slice {
                    element_size,
                    element_alignment,
                } => {
                    bytes.push(2);
                    bytes.extend_from_slice(&element_size.to_le_bytes());
                    bytes.extend_from_slice(&element_alignment.to_le_bytes());
                }
            }
            bytes.push(mutability_tag(field.mutability()));
            bytes.push(access_tag(field.access()));
            bytes.push(address_space_tag(field.address_space()));
            bytes.extend_from_slice(field.type_identity().rust_type().bytes().as_bytes());
            bytes.extend_from_slice(field.type_identity().layout().bytes().as_bytes());
            bytes.push(ownership_tag(field.ownership()));
            bytes.push(alias_class_tag(field.alias_class()));
        }
    }
    hash_domain_bytes(ABI_IDENTITY_DOMAIN_V2, &bytes)
}

fn derive_symbol_identity_v2(container: &ArtifactContainerV1) -> [u8; 32] {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&(container.manifest().kernels().len() as u16).to_le_bytes());
    for kernel in container.manifest().kernels() {
        bytes.extend_from_slice(kernel.kernel_id().as_bytes());
        push_len_bytes(&mut bytes, kernel.name().as_str().as_bytes());
        push_len_bytes(&mut bytes, kernel.symbol().as_str().as_bytes());
    }
    hash_domain_bytes(SYMBOL_IDENTITY_DOMAIN_V2, &bytes)
}

fn derive_resource_identity_v2(container: &ArtifactContainerV1) -> [u8; 32] {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&(container.manifest().kernels().len() as u16).to_le_bytes());
    for kernel in container.manifest().kernels() {
        bytes.extend_from_slice(kernel.kernel_id().as_bytes());
        bytes.extend_from_slice(&(kernel.required_capabilities().len() as u16).to_le_bytes());
        for capability in kernel.required_capabilities() {
            bytes.extend_from_slice(&capability_tag(*capability).to_le_bytes());
        }
        let launch = kernel.launch();
        bytes.push(launch.rank());
        match launch.block_size() {
            BlockSize::Any => bytes.push(0),
            BlockSize::Exact(value) => {
                bytes.push(1);
                push_dimensions(&mut bytes, value);
            }
            BlockSize::AtMost(value) => {
                bytes.push(2);
                push_dimensions(&mut bytes, value);
            }
        }
        push_dimensions(&mut bytes, launch.max_grid());
        bytes.extend_from_slice(&launch.static_shared_memory_bytes().to_le_bytes());
        bytes.extend_from_slice(&launch.max_dynamic_shared_memory_bytes().to_le_bytes());
    }
    hash_domain_bytes(RESOURCE_IDENTITY_DOMAIN_V2, &bytes)
}

fn derive_proof_identity_v2(proof_records: &[ProofRecordV1]) -> [u8; 32] {
    let mut proof_records = proof_records.to_vec();
    proof_records.sort_unstable_by_key(|record| record.target().artifact().kernel_id());
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&(proof_records.len() as u16).to_le_bytes());
    for proof in &proof_records {
        push_len_bytes(&mut bytes, &proof.to_bytes());
    }
    hash_domain_bytes(PROOF_IDENTITY_DOMAIN_V2, &bytes)
}

fn push_dimensions(bytes: &mut Vec<u8>, value: fe2o3_artifacts::Dimensions) {
    bytes.extend_from_slice(&value.x().to_le_bytes());
    bytes.extend_from_slice(&value.y().to_le_bytes());
    bytes.extend_from_slice(&value.z().to_le_bytes());
}

const fn pointer_width_tag(value: PointerWidth) -> u8 {
    match value {
        PointerWidth::Bits32 => 0,
        PointerWidth::Bits64 => 1,
    }
}

const fn endianness_tag(value: Endianness) -> u8 {
    match value {
        Endianness::Little => 0,
        Endianness::Big => 1,
    }
}

const fn capability_tag(value: Capability) -> u16 {
    match value {
        Capability::Subgroup => 0,
        Capability::Ballot => 1,
        Capability::Shuffle => 2,
        Capability::WorkgroupMemory => 3,
        Capability::MatrixMultiply => 4,
        Capability::AsyncCopy => 5,
        Capability::Atomics => 6,
        Capability::AmdWave => 7,
        Capability::AmdMfma => 8,
        Capability::AmdWmma => 9,
        Capability::AmdDsPermute => 10,
    }
}

const fn scalar_tag(value: ScalarType) -> u8 {
    match value {
        ScalarType::I8 => 0,
        ScalarType::U8 => 1,
        ScalarType::I16 => 2,
        ScalarType::U16 => 3,
        ScalarType::I32 => 4,
        ScalarType::U32 => 5,
        ScalarType::I64 => 6,
        ScalarType::U64 => 7,
        ScalarType::F16 => 8,
        ScalarType::F32 => 9,
        ScalarType::F64 => 10,
    }
}

const fn mutability_tag(value: Mutability) -> u8 {
    match value {
        Mutability::Immutable => 0,
        Mutability::Mutable => 1,
    }
}

const fn access_tag(value: Access) -> u8 {
    match value {
        Access::ByValue => 0,
        Access::ReadOnly => 1,
        Access::WriteOnly => 2,
        Access::ReadWrite => 3,
    }
}

const fn address_space_tag(value: AddressSpace) -> u8 {
    match value {
        AddressSpace::Value => 0,
        AddressSpace::Global => 1,
        AddressSpace::Constant => 2,
        AddressSpace::Workgroup => 3,
        AddressSpace::Private => 4,
        AddressSpace::Generic => 5,
    }
}

const fn ownership_tag(value: ArgumentOwnership) -> u8 {
    match value {
        ArgumentOwnership::ByValue => 0,
        ArgumentOwnership::SharedBorrow => 1,
        ArgumentOwnership::UniqueBorrow => 2,
        ArgumentOwnership::RawPointer => 3,
    }
}

const fn alias_class_tag(value: AliasClass) -> u8 {
    match value {
        AliasClass::Value => 0,
        AliasClass::SharedReadOnly => 1,
        AliasClass::Exclusive => 2,
        AliasClass::SharedAtomic => 3,
        AliasClass::Unrestricted => 4,
    }
}

fn code_object_version_tag(value: CodeObjectVersion) -> u8 {
    match value {
        CodeObjectVersion::V4 => 0,
        CodeObjectVersion::V5 => 1,
        CodeObjectVersion::V6 => 2,
    }
}

fn decode_code_object_version(tag: u8) -> Option<CodeObjectVersion> {
    match tag {
        0 => Some(CodeObjectVersion::V4),
        1 => Some(CodeObjectVersion::V5),
        2 => Some(CodeObjectVersion::V6),
        _ => None,
    }
}

fn encode_compiler_closure(bytes: &mut Vec<u8>, closure: CompilerClosureV2) {
    for pin in [
        closure.cargo_executable_sha256(),
        closure.cargo_binding_trampoline_sha256(),
        closure.cargo_fe2o3_binding_wrapper_sha256(),
        closure.rustc_executable_sha256(),
        closure.rustc_runtime_tree_sha256(),
        closure.codegen_backend_sha256(),
    ] {
        bytes.extend_from_slice(&pin);
    }
    bytes.extend_from_slice(
        &closure
            .cargo_binding_transition_protocol_version()
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&closure.identity_sha256());
}

impl WorkerV2FinalArtifactEvidenceV2 {
    pub fn to_bytes(&self) -> Vec<u8> {
        let target = self.target.to_string();
        let claim = self
            .published_claim
            .encode_canonical()
            .expect("validated protected claim must encode canonically");
        let total_len =
            FINAL_ARTIFACT_FIXED_BODY_BYTES_V2 + target.len() + claim.len() + CHECKSUM_BYTES_V2;
        let mut bytes = Vec::with_capacity(total_len);
        bytes.extend_from_slice(&WORKER_V2_FINAL_ARTIFACT_EVIDENCE_MAGIC_V2);
        bytes.extend_from_slice(&WORKER_V2_FINAL_ARTIFACT_EVIDENCE_VERSION_V2.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&(total_len as u32).to_le_bytes());
        bytes.extend_from_slice(&(target.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&(claim.len() as u16).to_le_bytes());
        bytes.push(match self.proof_or_inspection_identity.kind {
            WorkerV2ProofOrInspectionKindV2::ProofRecords => 0,
            WorkerV2ProofOrInspectionKindV2::ProtectedInspection => 1,
        });
        bytes.push(code_object_version_tag(self.code_object_version));
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        debug_assert_eq!(bytes.len(), FINAL_ARTIFACT_HEADER_BYTES_V2);
        encode_compiler_closure(&mut bytes, self.compiler_closure);
        bytes.extend_from_slice(&self.publication_intent.as_bytes());
        bytes.extend_from_slice(&self.final_bytes.sha256);
        bytes.extend_from_slice(&self.final_bytes.byte_len.to_le_bytes());
        for identity in [
            self.target_identity.0,
            self.abi_identity.0,
            self.descriptor_identity.0,
            self.symbol_identity.0,
            self.resource_identity.0,
            self.proof_or_inspection_identity.identity,
        ] {
            bytes.extend_from_slice(&identity);
        }
        bytes.extend_from_slice(target.as_bytes());
        bytes.extend_from_slice(&claim);
        let checksum = hash_domain_bytes(FINAL_ARTIFACT_CHECKSUM_DOMAIN_V2, &bytes);
        bytes.extend_from_slice(&checksum);
        debug_assert_eq!(bytes.len(), total_len);
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, WorkerV2FinalArtifactDecodeErrorV2> {
        if bytes.len() > MAX_WORKER_V2_FINAL_ARTIFACT_EVIDENCE_BYTES_V2 {
            return Err(WorkerV2FinalArtifactDecodeErrorV2::TooLarge {
                max: MAX_WORKER_V2_FINAL_ARTIFACT_EVIDENCE_BYTES_V2,
            });
        }
        let mut reader = ReaderV2::new(bytes);
        if reader.array::<8>()? != WORKER_V2_FINAL_ARTIFACT_EVIDENCE_MAGIC_V2 {
            return Err(WorkerV2FinalArtifactDecodeErrorV2::InvalidMagic);
        }
        let version = reader.u16()?;
        if version != WORKER_V2_FINAL_ARTIFACT_EVIDENCE_VERSION_V2 {
            return Err(WorkerV2FinalArtifactDecodeErrorV2::UnknownVersion(version));
        }
        let flags = reader.u16()?;
        if flags != 0 {
            return Err(WorkerV2FinalArtifactDecodeErrorV2::UnsupportedFlags(flags));
        }
        let total_len = reader.length_u32(
            "final artifact evidence",
            MAX_WORKER_V2_FINAL_ARTIFACT_EVIDENCE_BYTES_V2,
        )?;
        check_total_len(total_len, bytes.len())?;
        if bytes.len() < CHECKSUM_BYTES_V2 {
            return Err(WorkerV2FinalArtifactDecodeErrorV2::Truncated);
        }
        let (body, checksum) = bytes.split_at(bytes.len() - CHECKSUM_BYTES_V2);
        if hash_domain_bytes(FINAL_ARTIFACT_CHECKSUM_DOMAIN_V2, body) != checksum {
            return Err(WorkerV2FinalArtifactDecodeErrorV2::ChecksumMismatch);
        }
        let target_len = reader.length_u16("target", MAX_TARGET_TEXT_BYTES_V2)?;
        let claim_len = reader.length_u16(
            "published claim",
            MAX_DURABLE_PUBLISHED_HSACO_CLAIM_BYTES_V2,
        )?;
        let proof_kind = match reader.u8()? {
            0 => WorkerV2ProofOrInspectionKindV2::ProofRecords,
            1 => WorkerV2ProofOrInspectionKindV2::ProtectedInspection,
            tag => return Err(WorkerV2FinalArtifactDecodeErrorV2::UnknownEvidenceKind(tag)),
        };
        let cov_tag = reader.u8()?;
        let code_object_version = decode_code_object_version(cov_tag)
            .ok_or(WorkerV2FinalArtifactDecodeErrorV2::UnknownCodeObjectVersion(cov_tag))?;
        if reader.u16()? != 0 {
            return Err(WorkerV2FinalArtifactDecodeErrorV2::NonZeroReserved);
        }
        let compiler_closure = CompilerClosureV2::from_pins_and_identity(
            reader.array()?,
            reader.array()?,
            reader.array()?,
            reader.array()?,
            reader.array()?,
            reader.array()?,
            reader.u16()?,
            reader.array()?,
        )
        .map_err(WorkerV2FinalArtifactDecodeErrorV2::CompilerClosure)?;
        let publication_intent = WorkerV2PublicationIntentIdentityV2::from_bytes(reader.array()?);
        let final_bytes = WorkerV2FinalBytesIdentityV2 {
            sha256: reader.array()?,
            byte_len: reader.u64()?,
        };
        let target_identity = WorkerV2TargetIdentityV2(reader.array()?);
        let abi_identity = WorkerV2AbiIdentityV2(reader.array()?);
        let descriptor_identity = WorkerV2DescriptorIdentityV2(reader.array()?);
        let symbol_identity = WorkerV2SymbolIdentityV2(reader.array()?);
        let resource_identity = WorkerV2ResourceIdentityV2(reader.array()?);
        let proof_or_inspection_identity =
            WorkerV2ProofOrInspectionIdentityV2::from_wire(proof_kind, reader.array()?);
        let target_bytes = reader.take(target_len)?;
        let target_text = std::str::from_utf8(target_bytes)
            .map_err(|_| WorkerV2FinalArtifactDecodeErrorV2::InvalidTarget)?;
        let target = DeviceTargetV1::parse(target_text)
            .map_err(|_| WorkerV2FinalArtifactDecodeErrorV2::InvalidTarget)?;
        let published_claim =
            DurablePublishedHsacoClaimV2::decode_canonical(reader.take(claim_len)?)
                .map_err(WorkerV2FinalArtifactDecodeErrorV2::PublishedClaim)?;
        if reader.remaining_len() != CHECKSUM_BYTES_V2 {
            return Err(WorkerV2FinalArtifactDecodeErrorV2::TrailingBytes);
        }
        let value = Self {
            compiler_closure,
            publication_intent,
            backend_receipt: published_claim.receipt(),
            published_claim,
            final_bytes,
            target,
            code_object_version,
            target_identity,
            abi_identity,
            descriptor_identity,
            symbol_identity,
            resource_identity,
            proof_or_inspection_identity,
        };
        value
            .validate_self()
            .map_err(WorkerV2FinalArtifactDecodeErrorV2::Validation)?;
        if value.to_bytes() != bytes {
            return Err(WorkerV2FinalArtifactDecodeErrorV2::NonCanonical);
        }
        Ok(value)
    }
}

impl WorkerV2LoadEnvelopeV2 {
    pub fn to_bytes(&self) -> Vec<u8> {
        let container = self.container().to_bytes();
        let bundle = self.bundle_index().to_bytes();
        let direct_link = self.direct_link_evidence().to_bytes();
        let descriptor = self.descriptor_lineage().canonical_bytes();
        let final_artifact = self.final_artifact_evidence.to_bytes();
        let proofs = self
            .proof_records()
            .iter()
            .map(ProofRecordV1::to_bytes)
            .collect::<Vec<_>>();
        let total_len = LOAD_ENVELOPE_HEADER_BYTES_V2
            + container.len()
            + bundle.len()
            + direct_link.len()
            + descriptor.len()
            + final_artifact.len()
            + proofs.iter().map(|proof| 4 + proof.len()).sum::<usize>()
            + self.raw_hsaco().bytes().len()
            + CHECKSUM_BYTES_V2;
        let mut bytes = Vec::with_capacity(total_len);
        bytes.extend_from_slice(&WORKER_V2_LOAD_ENVELOPE_MAGIC_V2);
        bytes.extend_from_slice(&WORKER_V2_LOAD_ENVELOPE_VERSION_V2.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&(total_len as u32).to_le_bytes());
        for length in [
            container.len(),
            bundle.len(),
            direct_link.len(),
            descriptor.len(),
            self.raw_hsaco().bytes().len(),
        ] {
            bytes.extend_from_slice(&(length as u32).to_le_bytes());
        }
        bytes.extend_from_slice(&(proofs.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&(final_artifact.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.push(0);
        bytes.extend_from_slice(self.raw_hsaco().identity().bytes().as_bytes());
        debug_assert_eq!(bytes.len(), LOAD_ENVELOPE_HEADER_BYTES_V2);
        bytes.extend_from_slice(&container);
        bytes.extend_from_slice(&bundle);
        bytes.extend_from_slice(&direct_link);
        bytes.extend_from_slice(&descriptor);
        bytes.extend_from_slice(&final_artifact);
        for proof in proofs {
            bytes.extend_from_slice(&(proof.len() as u32).to_le_bytes());
            bytes.extend_from_slice(&proof);
        }
        bytes.extend_from_slice(self.raw_hsaco().bytes());
        let checksum = hash_domain_bytes(LOAD_ENVELOPE_CHECKSUM_DOMAIN_V2, &bytes);
        bytes.extend_from_slice(&checksum);
        debug_assert_eq!(bytes.len(), total_len);
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, WorkerV2LoadEnvelopeDecodeErrorV2> {
        if bytes.len() > MAX_WORKER_V2_LOAD_ENVELOPE_BYTES_V2 {
            return Err(WorkerV2LoadEnvelopeDecodeErrorV2::TooLarge {
                max: MAX_WORKER_V2_LOAD_ENVELOPE_BYTES_V2,
            });
        }
        let mut reader = ReaderV2::new(bytes);
        if reader.array::<8>()? != WORKER_V2_LOAD_ENVELOPE_MAGIC_V2 {
            return Err(WorkerV2LoadEnvelopeDecodeErrorV2::InvalidMagic);
        }
        let version = reader.u16()?;
        if version != WORKER_V2_LOAD_ENVELOPE_VERSION_V2 {
            return Err(WorkerV2LoadEnvelopeDecodeErrorV2::UnknownVersion(version));
        }
        let flags = reader.u16()?;
        if flags != 0 {
            return Err(WorkerV2LoadEnvelopeDecodeErrorV2::UnsupportedFlags(flags));
        }
        let total_len = reader.length_u32("envelope", MAX_WORKER_V2_LOAD_ENVELOPE_BYTES_V2)?;
        match total_len.cmp(&bytes.len()) {
            std::cmp::Ordering::Greater => {
                return Err(WorkerV2LoadEnvelopeDecodeErrorV2::Truncated);
            }
            std::cmp::Ordering::Less => {
                return Err(WorkerV2LoadEnvelopeDecodeErrorV2::TrailingBytes);
            }
            std::cmp::Ordering::Equal => {}
        }
        if bytes.len() < CHECKSUM_BYTES_V2 {
            return Err(WorkerV2LoadEnvelopeDecodeErrorV2::Truncated);
        }
        let (body, checksum) = bytes.split_at(bytes.len() - CHECKSUM_BYTES_V2);
        if hash_domain_bytes(LOAD_ENVELOPE_CHECKSUM_DOMAIN_V2, body) != checksum {
            return Err(WorkerV2LoadEnvelopeDecodeErrorV2::ChecksumMismatch);
        }
        let container_len = reader.length_u32("artifact container", MAX_CONTAINER_BYTES)?;
        let bundle_len = reader.length_u32("bundle index", MAX_BUNDLE_INDEX_BYTES)?;
        let direct_link_len =
            reader.length_u32("direct-link evidence", MAX_DIRECT_LINK_EVIDENCE_BYTES)?;
        let descriptor_len = reader.length_u32("descriptor lineage", MAX_DESCRIPTOR_TABLE_BYTES)?;
        let raw_len = reader.length_u32("raw HSACO", MAX_WORKER_V2_RAW_HSACO_BYTES)?;
        let proof_count = usize::from(reader.u16()?);
        if proof_count > MAX_KERNELS {
            return Err(WorkerV2LoadEnvelopeDecodeErrorV2::CountOutOfRange {
                field: "proof records",
                value: proof_count as u64,
                max: MAX_KERNELS,
            });
        }
        if reader.u16()? != 0 {
            return Err(WorkerV2LoadEnvelopeDecodeErrorV2::NonZeroReserved);
        }
        let final_artifact_len = reader.length_u16(
            "final artifact evidence",
            MAX_WORKER_V2_FINAL_ARTIFACT_EVIDENCE_BYTES_V2,
        )?;
        if reader.u16()? != 0 {
            return Err(WorkerV2LoadEnvelopeDecodeErrorV2::NonZeroReserved);
        }
        if reader.u8()? != 0 {
            return Err(WorkerV2LoadEnvelopeDecodeErrorV2::UnknownDigestAlgorithm);
        }
        let raw_identity = PayloadDigest::new(
            DigestAlgorithm::Sha256,
            DigestBytes::from_bytes(reader.array()?),
        );
        let minimum = LOAD_ENVELOPE_HEADER_BYTES_V2
            .checked_add(container_len)
            .and_then(|value| value.checked_add(bundle_len))
            .and_then(|value| value.checked_add(direct_link_len))
            .and_then(|value| value.checked_add(descriptor_len))
            .and_then(|value| value.checked_add(final_artifact_len))
            .and_then(|value| value.checked_add(raw_len))
            .and_then(|value| value.checked_add(proof_count.checked_mul(4)?))
            .and_then(|value| value.checked_add(CHECKSUM_BYTES_V2))
            .ok_or(WorkerV2LoadEnvelopeDecodeErrorV2::LengthOverflow)?;
        if minimum > total_len {
            return Err(WorkerV2LoadEnvelopeDecodeErrorV2::Truncated);
        }
        let container = ArtifactContainerV1::from_bytes(reader.take(container_len)?)
            .map_err(WorkerV2LoadEnvelopeDecodeErrorV2::Container)?;
        let bundle_index = BundleIndexV1::from_bytes(reader.take(bundle_len)?)
            .map_err(WorkerV2LoadEnvelopeDecodeErrorV2::Bundle)?;
        let direct_link_evidence =
            DirectLinkBundleEvidenceV1::from_bytes(reader.take(direct_link_len)?)
                .map_err(WorkerV2LoadEnvelopeDecodeErrorV2::DirectLink)?;
        let descriptor_lineage = DescriptorLineageV1::new(
            decode_device_descriptor_table_v1(reader.take(descriptor_len)?)
                .map_err(WorkerV2LoadEnvelopeDecodeErrorV2::Descriptor)?,
        );
        let final_artifact_evidence =
            WorkerV2FinalArtifactEvidenceV2::from_bytes(reader.take(final_artifact_len)?)
                .map_err(WorkerV2LoadEnvelopeDecodeErrorV2::FinalArtifact)?;
        let mut proof_records = Vec::with_capacity(proof_count);
        let mut proof_bytes = 0usize;
        for _ in 0..proof_count {
            let proof_len = reader.length_u32("proof record", MAX_PROOF_RECORD_BYTES)?;
            proof_bytes = proof_bytes
                .checked_add(proof_len)
                .ok_or(WorkerV2LoadEnvelopeDecodeErrorV2::LengthOverflow)?;
            if proof_bytes > MAX_WORKER_V2_PROOF_EVIDENCE_BYTES {
                return Err(WorkerV2LoadEnvelopeDecodeErrorV2::LengthOutOfRange {
                    field: "proof evidence",
                    value: proof_bytes as u64,
                    max: MAX_WORKER_V2_PROOF_EVIDENCE_BYTES,
                });
            }
            proof_records.push(
                ProofRecordV1::from_bytes(reader.take(proof_len)?)
                    .map_err(WorkerV2LoadEnvelopeDecodeErrorV2::Proof)?,
            );
        }
        let raw_hsaco = ExactRawHsacoV1::new(raw_identity, reader.take(raw_len)?.to_vec())
            .map_err(|error| {
                WorkerV2LoadEnvelopeDecodeErrorV2::Validation(
                    WorkerV2LoadEnvelopeValidationErrorV2::Components(error),
                )
            })?;
        if reader.remaining_len() != CHECKSUM_BYTES_V2 {
            return Err(WorkerV2LoadEnvelopeDecodeErrorV2::TrailingBytes);
        }
        let value = Self::new(
            container,
            bundle_index,
            direct_link_evidence,
            descriptor_lineage,
            proof_records,
            raw_hsaco,
            final_artifact_evidence,
        )
        .map_err(WorkerV2LoadEnvelopeDecodeErrorV2::Validation)?;
        if value.to_bytes() != bytes {
            return Err(WorkerV2LoadEnvelopeDecodeErrorV2::NonCanonical);
        }
        Ok(value)
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum WorkerV2FinalArtifactDecodeErrorV2 {
    TooLarge {
        max: usize,
    },
    Truncated,
    TrailingBytes,
    InvalidMagic,
    UnknownVersion(u16),
    UnsupportedFlags(u16),
    NonZeroReserved,
    ChecksumMismatch,
    LengthOutOfRange {
        field: &'static str,
        value: u64,
        max: usize,
    },
    UnknownEvidenceKind(u8),
    UnknownCodeObjectVersion(u8),
    InvalidTarget,
    CompilerClosure(CompilerClosureErrorV2),
    PublishedClaim(DurablePublishedClaimCodecErrorV2),
    Validation(WorkerV2FinalArtifactValidationErrorV2),
    NonCanonical,
}

impl fmt::Display for WorkerV2FinalArtifactDecodeErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { max } => {
                write!(formatter, "final artifact evidence exceeds {max} bytes")
            }
            Self::Truncated => formatter.write_str("final artifact evidence is truncated"),
            Self::TrailingBytes => {
                formatter.write_str("final artifact evidence has trailing bytes")
            }
            Self::InvalidMagic => formatter.write_str("final artifact evidence magic is invalid"),
            Self::UnknownVersion(version) => write!(
                formatter,
                "unsupported final artifact evidence version {version}"
            ),
            Self::UnsupportedFlags(flags) => write!(
                formatter,
                "unsupported final artifact evidence flags {flags:#x}"
            ),
            Self::NonZeroReserved => {
                formatter.write_str("final artifact evidence reserved field is nonzero")
            }
            Self::ChecksumMismatch => {
                formatter.write_str("final artifact evidence checksum does not match")
            }
            Self::LengthOutOfRange { field, value, max } => {
                write!(formatter, "{field} length {value} exceeds {max}")
            }
            Self::UnknownEvidenceKind(tag) => {
                write!(formatter, "unknown proof-or-inspection evidence kind {tag}")
            }
            Self::UnknownCodeObjectVersion(tag) => {
                write!(formatter, "unknown code-object version tag {tag}")
            }
            Self::InvalidTarget => {
                formatter.write_str("final artifact target is invalid or noncanonical")
            }
            Self::CompilerClosure(error) => error.fmt(formatter),
            Self::PublishedClaim(error) => error.fmt(formatter),
            Self::Validation(error) => error.fmt(formatter),
            Self::NonCanonical => formatter.write_str("final artifact evidence is not canonical"),
        }
    }
}

impl std::error::Error for WorkerV2FinalArtifactDecodeErrorV2 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CompilerClosure(error) => Some(error),
            Self::PublishedClaim(error) => Some(error),
            Self::Validation(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum WorkerV2LoadEnvelopeDecodeErrorV2 {
    TooLarge {
        max: usize,
    },
    Truncated,
    TrailingBytes,
    InvalidMagic,
    UnknownVersion(u16),
    UnsupportedFlags(u16),
    NonZeroReserved,
    ChecksumMismatch,
    UnknownDigestAlgorithm,
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
    Container(ContainerDecodeError),
    Bundle(BundleDecodeError),
    DirectLink(DirectLinkDecodeError),
    Descriptor(DescriptorDecodeError),
    Proof(ProofDecodeError),
    FinalArtifact(WorkerV2FinalArtifactDecodeErrorV2),
    Validation(WorkerV2LoadEnvelopeValidationErrorV2),
    NonCanonical,
}

impl fmt::Display for WorkerV2LoadEnvelopeDecodeErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { max } => write!(
                formatter,
                "protected Worker V2 envelope exceeds {max} bytes"
            ),
            Self::Truncated => formatter.write_str("protected Worker V2 envelope is truncated"),
            Self::TrailingBytes => {
                formatter.write_str("protected Worker V2 envelope has trailing bytes")
            }
            Self::InvalidMagic => {
                formatter.write_str("protected Worker V2 envelope magic is invalid")
            }
            Self::UnknownVersion(version) => write!(
                formatter,
                "unsupported protected Worker V2 envelope version {version}"
            ),
            Self::UnsupportedFlags(flags) => write!(
                formatter,
                "unsupported protected Worker V2 envelope flags {flags:#x}"
            ),
            Self::NonZeroReserved => {
                formatter.write_str("protected Worker V2 envelope reserved field is nonzero")
            }
            Self::ChecksumMismatch => {
                formatter.write_str("protected Worker V2 envelope checksum does not match")
            }
            Self::UnknownDigestAlgorithm => {
                formatter.write_str("unknown protected Worker V2 envelope digest algorithm")
            }
            Self::LengthOutOfRange { field, value, max } => {
                write!(formatter, "{field} length {value} exceeds {max}")
            }
            Self::CountOutOfRange { field, value, max } => {
                write!(formatter, "{field} count {value} exceeds {max}")
            }
            Self::LengthOverflow => {
                formatter.write_str("protected Worker V2 envelope length overflows")
            }
            Self::Container(error) => error.fmt(formatter),
            Self::Bundle(error) => error.fmt(formatter),
            Self::DirectLink(error) => error.fmt(formatter),
            Self::Descriptor(error) => error.fmt(formatter),
            Self::Proof(error) => error.fmt(formatter),
            Self::FinalArtifact(error) => error.fmt(formatter),
            Self::Validation(error) => error.fmt(formatter),
            Self::NonCanonical => {
                formatter.write_str("protected Worker V2 envelope is not canonical")
            }
        }
    }
}

impl std::error::Error for WorkerV2LoadEnvelopeDecodeErrorV2 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Container(error) => Some(error),
            Self::Bundle(error) => Some(error),
            Self::DirectLink(error) => Some(error),
            Self::Descriptor(error) => Some(error),
            Self::Proof(error) => Some(error),
            Self::FinalArtifact(error) => Some(error),
            Self::Validation(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Debug)]
enum WireDecodeErrorV2 {
    Truncated,
    LengthOutOfRange {
        field: &'static str,
        value: u64,
        max: usize,
    },
}

impl From<WireDecodeErrorV2> for WorkerV2FinalArtifactDecodeErrorV2 {
    fn from(value: WireDecodeErrorV2) -> Self {
        match value {
            WireDecodeErrorV2::Truncated => Self::Truncated,
            WireDecodeErrorV2::LengthOutOfRange { field, value, max } => {
                Self::LengthOutOfRange { field, value, max }
            }
        }
    }
}

impl From<WireDecodeErrorV2> for WorkerV2LoadEnvelopeDecodeErrorV2 {
    fn from(value: WireDecodeErrorV2) -> Self {
        match value {
            WireDecodeErrorV2::Truncated => Self::Truncated,
            WireDecodeErrorV2::LengthOutOfRange { field, value, max } => {
                Self::LengthOutOfRange { field, value, max }
            }
        }
    }
}

struct ReaderV2<'a> {
    remaining: &'a [u8],
}

impl<'a> ReaderV2<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { remaining: bytes }
    }

    const fn remaining_len(&self) -> usize {
        self.remaining.len()
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], WireDecodeErrorV2> {
        if self.remaining.len() < count {
            return Err(WireDecodeErrorV2::Truncated);
        }
        let (value, remaining) = self.remaining.split_at(count);
        self.remaining = remaining;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], WireDecodeErrorV2> {
        let mut value = [0; N];
        value.copy_from_slice(self.take(N)?);
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8, WireDecodeErrorV2> {
        Ok(self.array::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, WireDecodeErrorV2> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, WireDecodeErrorV2> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, WireDecodeErrorV2> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    fn length_u16(&mut self, field: &'static str, max: usize) -> Result<usize, WireDecodeErrorV2> {
        let value = u64::from(self.u16()?);
        Self::checked_length(value, field, max)
    }

    fn length_u32(&mut self, field: &'static str, max: usize) -> Result<usize, WireDecodeErrorV2> {
        let value = u64::from(self.u32()?);
        Self::checked_length(value, field, max)
    }

    fn checked_length(
        value: u64,
        field: &'static str,
        max: usize,
    ) -> Result<usize, WireDecodeErrorV2> {
        if value > max as u64 {
            Err(WireDecodeErrorV2::LengthOutOfRange { field, value, max })
        } else {
            Ok(value as usize)
        }
    }
}

fn check_total_len(
    total_len: usize,
    actual_len: usize,
) -> Result<(), WorkerV2FinalArtifactDecodeErrorV2> {
    match total_len.cmp(&actual_len) {
        std::cmp::Ordering::Greater => Err(WorkerV2FinalArtifactDecodeErrorV2::Truncated),
        std::cmp::Ordering::Less => Err(WorkerV2FinalArtifactDecodeErrorV2::TrailingBytes),
        std::cmp::Ordering::Equal => Ok(()),
    }
}
