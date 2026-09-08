use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use fe2o3_amd_target::{
    ProductionAmdCapabilityOwnerV1, ProductionAmdTargetCapabilityModelErrorV1,
    ProductionAmdTargetProfileV1,
};
use fe2o3_kernel_ir::{
    AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE, AccessMode, AddressSpace, AsyncCopyCompletionV1,
    AtomicKind, CollectiveCapabilityOperationV1, ExecutionCapabilityOperationV1,
    ExecutionCapabilityRequirementV1, ExecutionCollectiveKindV1, ExecutionMemoryAddressSpaceV1,
    ExecutionMemoryOrderingV1, ExecutionMemoryScopeV1, ExecutionMemorySemanticsV1, KernelId,
    KernelIrDecodeError, MatrixElement, MatrixOperationKind, MemoryOrdering, Module,
    NumericalModeV1, OperationKind, ResourceCapabilityRequirementV1, ScalarType,
    SynchronizationScope, TargetCapability, Type, VerifiedCanonicalKernelIrErrorV12,
    VerifiedCanonicalKernelIrErrorV13, VerifiedCanonicalKernelIrIdentityV12,
    VerifiedCanonicalKernelIrIdentityV13, VerifiedCanonicalKernelIrV12,
    VerifiedCanonicalKernelIrV13, WaveF32ReductionKindV1, WaveOperationKind, WaveWidth,
    WorkgroupMemoryExtent, decode_module_v12, decode_module_v13,
};
use fe2o3_target_spec::{
    MAX_TARGET_CAPABILITY_CLOSURE_REQUIREMENTS_V1, TargetAbiConstraintV1, TargetAddressSpaceV1,
    TargetAsyncCopyRequirementV1, TargetAsyncWaitRequirementV1, TargetAtomicOperationV1,
    TargetAtomicRequirementV1, TargetBarrierParticipationV1, TargetBarrierRequirementV1,
    TargetCapabilityDecisionOutcomeV1, TargetCapabilityDecisionV1, TargetCapabilityModelIdentityV1,
    TargetCapabilityQueryErrorV1, TargetCapabilityRequirementV1, TargetCollectiveOperationV1,
    TargetCollectiveParticipationV1, TargetCollectiveRequirementV1, TargetEndiannessV1,
    TargetExecutionScopeV1, TargetFenceRequirementV1, TargetLaunchEvidenceKindV1,
    TargetMatrixLayoutV1, TargetMatrixLayoutsV1, TargetMatrixOperationV1,
    TargetMatrixRequirementV1, TargetMatrixShapeV1, TargetMemoryAccessV1, TargetMemoryOrderingV1,
    TargetMemoryScopeV1, TargetNumericalModeV1, TargetNumericalRequirementV1,
    TargetObjectConstraintV1, TargetObjectFormatV1, TargetResourceRequirementV1,
    TargetScalarEncodingV1, TargetScalarKindV1, TargetScalarTypeV1, query_target_capability_v1,
};
use sha2::{Digest, Sha256};

use crate::{
    ProductionTargetBindingErrorV1, ProductionTargetBoundKernelIrV1, bind_production_target_v1,
};

const LAUNCH_EVIDENCE_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/PRODUCTION-TARGET-LAUNCH-EVIDENCE/V1\0";
const CAPABILITY_CLOSURE_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/PRODUCTION-TARGET-CAPABILITY-CLOSURE/V1\0";
const CAPABILITY_CLOSURE_MAGIC_V1: &[u8; 8] = b"F2TCAP01";
const CAPABILITY_CLOSURE_WIRE_VERSION_V1: u16 = 1;
/// A closure is deliberately small enough to validate before allocating from input lengths.
pub const MAX_PRODUCTION_TARGET_CAPABILITY_CLOSURE_BYTES_V1: usize = 1 << 20;
const MAX_PRODUCTION_TARGET_CAPABILITY_DEPENDENCIES_V1: usize = 32;

/// Canonical KIR wire version naming one capability-query subject.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionCanonicalGraphVersionV1 {
    V12,
    V13,
}

impl ProductionCanonicalGraphVersionV1 {
    const fn tag(self) -> u8 {
        match self {
            Self::V12 => 12,
            Self::V13 => 13,
        }
    }
}

/// Versioned identity and epoch consumed by the module-based query core.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProductionCanonicalGraphSubjectV1 {
    version: ProductionCanonicalGraphVersionV1,
    digest: [u8; 32],
    canonical_length: u64,
    epoch: u64,
}

impl ProductionCanonicalGraphSubjectV1 {
    fn from_v12(identity: VerifiedCanonicalKernelIrIdentityV12, epoch: u64) -> Self {
        Self {
            version: ProductionCanonicalGraphVersionV1::V12,
            digest: *identity.digest(),
            canonical_length: identity.canonical_length(),
            epoch,
        }
    }

    fn from_v13(identity: VerifiedCanonicalKernelIrIdentityV13, epoch: u64) -> Self {
        Self {
            version: ProductionCanonicalGraphVersionV1::V13,
            digest: *identity.digest(),
            canonical_length: identity.canonical_length(),
            epoch,
        }
    }

    pub const fn version(self) -> ProductionCanonicalGraphVersionV1 {
        self.version
    }

    pub const fn digest(self) -> [u8; 32] {
        self.digest
    }

    pub const fn canonical_length(self) -> u64 {
        self.canonical_length
    }

    pub const fn epoch(self) -> u64 {
        self.epoch
    }
}

/// Stable diagnostics emitted by the production V12 target-capability boundary.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionTargetCapabilityDiagnosticCodeV1 {
    InvalidCanonicalKir,
    InvalidTargetModel,
    InvalidRequirement,
    OmittedAxis,
    Unsupported,
    Incomplete,
    Unreviewed,
    MissingLaunchEvidence,
    LaunchEvidenceMismatch,
    NeutralTargetSelection,
    UnsupportedExtension,
    InvalidLaunchShape,
    EpochOverflow,
    TargetBinding,
}

impl ProductionTargetCapabilityDiagnosticCodeV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidCanonicalKir => "FE2O3-TARGET-CAP-001",
            Self::InvalidTargetModel => "FE2O3-TARGET-CAP-002",
            Self::InvalidRequirement => "FE2O3-TARGET-CAP-003",
            Self::OmittedAxis => "FE2O3-TARGET-CAP-004",
            Self::Unsupported => "FE2O3-TARGET-CAP-005",
            Self::Incomplete => "FE2O3-TARGET-CAP-006",
            Self::Unreviewed => "FE2O3-TARGET-CAP-007",
            Self::MissingLaunchEvidence => "FE2O3-TARGET-CAP-008",
            Self::LaunchEvidenceMismatch => "FE2O3-TARGET-CAP-009",
            Self::NeutralTargetSelection => "FE2O3-TARGET-CAP-010",
            Self::UnsupportedExtension => "FE2O3-TARGET-CAP-011",
            Self::InvalidLaunchShape => "FE2O3-TARGET-CAP-012",
            Self::EpochOverflow => "FE2O3-TARGET-CAP-013",
            Self::TargetBinding => "FE2O3-TARGET-CAP-014",
        }
    }
}

impl fmt::Display for ProductionTargetCapabilityDiagnosticCodeV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// An axis that a coarse KIR or target-query schema cannot represent exactly.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum OmittedCapabilityAxisV1 {
    ScalarEncoding,
    ScalarKind,
    SubgroupWidth,
    AtomicOperation,
    AtomicOrdering,
    AtomicFailureOrdering,
    BarrierExecutionScope,
    BarrierMemoryScope,
    BarrierOrdering,
    BarrierAddressSpaces,
    BarrierParticipation,
    CollectiveExecutionScope,
    CollectiveOperation,
    CollectiveParticipation,
    CollectiveNumericalMode,
    MatrixLayouts,
    MatrixNumericalMode,
    AsyncCompletionExecutionScope,
    AsyncCompletionMemoryScope,
    AsyncCompletionOrdering,
    NumericalValueType,
    DynamicSharedMemoryBytes,
    ExtensionSemantics,
}

impl fmt::Display for OmittedCapabilityAxisV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ScalarEncoding => "scalar encoding",
            Self::ScalarKind => "scalar signedness",
            Self::SubgroupWidth => "subgroup width",
            Self::AtomicOperation => "atomic operation",
            Self::AtomicOrdering => "atomic ordering",
            Self::AtomicFailureOrdering => "atomic failure ordering",
            Self::BarrierExecutionScope => "barrier execution scope",
            Self::BarrierMemoryScope => "barrier memory scope",
            Self::BarrierOrdering => "barrier ordering",
            Self::BarrierAddressSpaces => "barrier address-space set",
            Self::BarrierParticipation => "barrier participation",
            Self::CollectiveExecutionScope => "collective execution scope",
            Self::CollectiveOperation => "collective operation",
            Self::CollectiveParticipation => "collective participation",
            Self::CollectiveNumericalMode => "collective numerical mode",
            Self::MatrixLayouts => "matrix operand/result layouts",
            Self::MatrixNumericalMode => "matrix numerical mode",
            Self::AsyncCompletionExecutionScope => "async completion execution scope",
            Self::AsyncCompletionMemoryScope => "async completion memory scope",
            Self::AsyncCompletionOrdering => "async completion ordering",
            Self::NumericalValueType => "numerical value type",
            Self::DynamicSharedMemoryBytes => "dynamic shared-memory byte bound",
            Self::ExtensionSemantics => "extension semantics",
        })
    }
}

/// Exact launch-dependent value associated with one immutable neutral graph.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionTargetLaunchEvidenceValueV1 {
    DynamicSharedMemoryBytes(u64),
    WorkgroupDimensions {
        x: u32,
        y: u32,
        z: u32,
    },
    CooperativeGridAdmission,
    /// Whether global memory used by system-scope atomics is host/device
    /// coherent and mapped for the complete launch lifetime.
    SystemAtomicMemoryEligibility(bool),
}

impl ProductionTargetLaunchEvidenceValueV1 {
    pub const fn kind(self) -> TargetLaunchEvidenceKindV1 {
        match self {
            Self::DynamicSharedMemoryBytes(_) => {
                TargetLaunchEvidenceKindV1::DynamicSharedMemoryBytes
            }
            Self::WorkgroupDimensions { .. } => TargetLaunchEvidenceKindV1::WorkgroupDimensions,
            Self::CooperativeGridAdmission => TargetLaunchEvidenceKindV1::CooperativeGridAdmission,
            Self::SystemAtomicMemoryEligibility(_) => {
                TargetLaunchEvidenceKindV1::SystemAtomicMemoryEligibility
            }
        }
    }
}

trait ExactLaunchEvidenceV1 {
    fn identity(&self) -> [u8; 32];
    fn subject(&self) -> ProductionCanonicalGraphSubjectV1;
    fn values(&self) -> &[ProductionTargetLaunchEvidenceValueV1];

    fn value(
        &self,
        kind: TargetLaunchEvidenceKindV1,
    ) -> Option<ProductionTargetLaunchEvidenceValueV1> {
        self.values()
            .binary_search_by_key(&kind, |value| value.kind())
            .ok()
            .map(|index| self.values()[index])
    }
}

fn exact_launch_values(
    values: impl IntoIterator<Item = ProductionTargetLaunchEvidenceValueV1>,
) -> Result<Box<[ProductionTargetLaunchEvidenceValueV1]>, ProductionTargetCapabilityErrorV1> {
    let mut by_kind = BTreeMap::new();
    for value in values {
        if let ProductionTargetLaunchEvidenceValueV1::WorkgroupDimensions { x, y, z } = value
            && (x == 0 || y == 0 || z == 0)
        {
            return Err(ProductionTargetCapabilityErrorV1::InvalidLaunchEvidenceValue { value });
        }
        if by_kind.insert(value.kind(), value).is_some() {
            return Err(ProductionTargetCapabilityErrorV1::DuplicateLaunchEvidence {
                kind: value.kind(),
            });
        }
    }
    Ok(by_kind.into_values().collect::<Vec<_>>().into_boxed_slice())
}

/// Authority-free launch facts bound to one exact canonical KIR identity and epoch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionTargetLaunchEvidenceV1 {
    identity: [u8; 32],
    subject: ProductionCanonicalGraphSubjectV1,
    neutral_kernel_ir: VerifiedCanonicalKernelIrIdentityV12,
    neutral_epoch: u64,
    values: Box<[ProductionTargetLaunchEvidenceValueV1]>,
}

impl ProductionTargetLaunchEvidenceV1 {
    pub fn for_static_launches(
        neutral: &VerifiedCanonicalKernelIrV12,
        neutral_epoch: u64,
    ) -> Result<Self, ProductionTargetCapabilityErrorV1> {
        Self::from_exact_values(neutral, neutral_epoch, [])
    }

    pub fn with_dynamic_shared_memory_bytes(
        neutral: &VerifiedCanonicalKernelIrV12,
        neutral_epoch: u64,
        exact_bytes: u64,
    ) -> Result<Self, ProductionTargetCapabilityErrorV1> {
        Self::from_exact_values(
            neutral,
            neutral_epoch,
            [ProductionTargetLaunchEvidenceValueV1::DynamicSharedMemoryBytes(exact_bytes)],
        )
    }

    pub fn with_system_atomic_memory_eligibility(
        neutral: &VerifiedCanonicalKernelIrV12,
        neutral_epoch: u64,
        eligible: bool,
    ) -> Result<Self, ProductionTargetCapabilityErrorV1> {
        Self::from_exact_values(
            neutral,
            neutral_epoch,
            [ProductionTargetLaunchEvidenceValueV1::SystemAtomicMemoryEligibility(eligible)],
        )
    }

    pub fn from_exact_values(
        neutral: &VerifiedCanonicalKernelIrV12,
        neutral_epoch: u64,
        values: impl IntoIterator<Item = ProductionTargetLaunchEvidenceValueV1>,
    ) -> Result<Self, ProductionTargetCapabilityErrorV1> {
        neutral
            .revalidate()
            .map_err(ProductionTargetCapabilityErrorV1::InvalidCanonicalKir)?;
        let values = exact_launch_values(values)?;
        let neutral_kernel_ir = *neutral.identity();
        let subject = ProductionCanonicalGraphSubjectV1::from_v12(neutral_kernel_ir, neutral_epoch);
        let identity = launch_evidence_identity(subject, &values);
        Ok(Self {
            identity,
            subject,
            neutral_kernel_ir,
            neutral_epoch,
            values,
        })
    }

    pub const fn identity(&self) -> [u8; 32] {
        self.identity
    }

    pub const fn neutral_kernel_ir(&self) -> VerifiedCanonicalKernelIrIdentityV12 {
        self.neutral_kernel_ir
    }

    pub const fn neutral_epoch(&self) -> u64 {
        self.neutral_epoch
    }

    pub const fn subject(&self) -> ProductionCanonicalGraphSubjectV1 {
        self.subject
    }

    pub fn values(&self) -> &[ProductionTargetLaunchEvidenceValueV1] {
        &self.values
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

impl ExactLaunchEvidenceV1 for ProductionTargetLaunchEvidenceV1 {
    fn identity(&self) -> [u8; 32] {
        self.identity
    }

    fn subject(&self) -> ProductionCanonicalGraphSubjectV1 {
        self.subject
    }

    fn values(&self) -> &[ProductionTargetLaunchEvidenceValueV1] {
        &self.values
    }
}

/// Authority-free launch facts for one exact verified canonical KIR V13 graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionTargetLaunchEvidenceV13 {
    identity: [u8; 32],
    subject: ProductionCanonicalGraphSubjectV1,
    values: Box<[ProductionTargetLaunchEvidenceValueV1]>,
}

impl ProductionTargetLaunchEvidenceV13 {
    pub fn for_static_launches(
        neutral: &VerifiedCanonicalKernelIrV13,
        neutral_epoch: u64,
    ) -> Result<Self, ProductionTargetCapabilityErrorV1> {
        Self::from_exact_values(neutral, neutral_epoch, [])
    }

    pub fn from_exact_values(
        neutral: &VerifiedCanonicalKernelIrV13,
        neutral_epoch: u64,
        values: impl IntoIterator<Item = ProductionTargetLaunchEvidenceValueV1>,
    ) -> Result<Self, ProductionTargetCapabilityErrorV1> {
        neutral
            .revalidate()
            .map_err(ProductionTargetCapabilityErrorV1::InvalidCanonicalKirV13)?;
        let values = exact_launch_values(values)?;
        let subject =
            ProductionCanonicalGraphSubjectV1::from_v13(*neutral.identity(), neutral_epoch);
        Ok(Self {
            identity: launch_evidence_identity(subject, &values),
            subject,
            values,
        })
    }

    pub const fn identity(&self) -> [u8; 32] {
        self.identity
    }

    pub const fn subject(&self) -> ProductionCanonicalGraphSubjectV1 {
        self.subject
    }

    pub fn values(&self) -> &[ProductionTargetLaunchEvidenceValueV1] {
        &self.values
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

impl ExactLaunchEvidenceV1 for ProductionTargetLaunchEvidenceV13 {
    fn identity(&self) -> [u8; 32] {
        self.identity
    }

    fn subject(&self) -> ProductionCanonicalGraphSubjectV1 {
        self.subject
    }

    fn values(&self) -> &[ProductionTargetLaunchEvidenceValueV1] {
        &self.values
    }
}

/// One exact target query, its direct prerequisites, and the lowering owner that admitted it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionTargetLegalizationRecordV1 {
    root: bool,
    decision: TargetCapabilityDecisionV1,
    dependencies: Box<[TargetCapabilityRequirementV1]>,
    owner: ProductionAmdCapabilityOwnerV1,
}

impl ProductionTargetLegalizationRecordV1 {
    /// Whether this requirement came directly from the canonical graph or target binding policy.
    pub const fn is_root(&self) -> bool {
        self.root
    }

    /// Immutable target query and answer for this record.
    pub const fn decision(&self) -> TargetCapabilityDecisionV1 {
        self.decision
    }

    /// Direct semantic prerequisites in canonical ascending order.
    pub fn dependencies(&self) -> &[TargetCapabilityRequirementV1] {
        &self.dependencies
    }

    /// Existing implementation component responsible for the admitted requirement.
    pub const fn owner(&self) -> ProductionAmdCapabilityOwnerV1 {
        self.owner
    }
}

/// Facts that only final object inspection can establish.
///
/// These are carried in pre-artifact closure bytes so a consumer cannot confuse target
/// support with final ABI/resource evidence. They are obligations, never satisfied receipts.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionArtifactOnlyRequirementV1 {
    ExactKernelArgumentLayout,
    ExactRegistersPerInvocation,
    ExactPrivateSegmentBytes,
    ExactScratchBytes,
    ExactLoadableObject,
}

/// Complete successful query closure for one immutable neutral KIR epoch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionTargetCapabilityClosureV1 {
    identity: [u8; 32],
    canonical_bytes: Box<[u8]>,
    neutral_kernel_ir: VerifiedCanonicalKernelIrIdentityV12,
    neutral_epoch: u64,
    target_model: TargetCapabilityModelIdentityV1,
    launch_evidence: ProductionTargetLaunchEvidenceV1,
    decisions: Box<[TargetCapabilityDecisionV1]>,
    owners: Box<[ProductionAmdCapabilityOwnerV1]>,
    records: Box<[ProductionTargetLegalizationRecordV1]>,
    artifact_only_requirements: Box<[ProductionArtifactOnlyRequirementV1]>,
}

impl ProductionTargetCapabilityClosureV1 {
    /// Strictly decodes an owned V1 closure and replays every target query.
    pub fn decode_canonical(
        bytes: &[u8],
        launch_evidence: &ProductionTargetLaunchEvidenceV1,
    ) -> Result<Self, ProductionTargetCapabilityCanonicalErrorV1> {
        let decoded = decode_capability_closure(bytes)?;
        if decoded.subject.version() != ProductionCanonicalGraphVersionV1::V12 {
            return Err(ProductionTargetCapabilityCanonicalErrorV1::GraphVersionMismatch);
        }
        if decoded.subject != launch_evidence.subject()
            || decoded.launch_evidence_identity != launch_evidence.identity()
        {
            return Err(ProductionTargetCapabilityCanonicalErrorV1::LaunchEvidenceMismatch);
        }
        Ok(Self {
            identity: capability_closure_identity(bytes),
            canonical_bytes: bytes.to_vec().into_boxed_slice(),
            neutral_kernel_ir: launch_evidence.neutral_kernel_ir(),
            neutral_epoch: decoded.subject.epoch(),
            target_model: decoded.target_model,
            launch_evidence: launch_evidence.clone(),
            decisions: decoded.decisions,
            owners: decoded.owners,
            records: decoded.records,
            artifact_only_requirements: decoded.artifact_only_requirements,
        })
    }

    pub const fn identity(&self) -> [u8; 32] {
        self.identity
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn neutral_kernel_ir(&self) -> VerifiedCanonicalKernelIrIdentityV12 {
        self.neutral_kernel_ir
    }

    pub const fn neutral_epoch(&self) -> u64 {
        self.neutral_epoch
    }

    pub const fn target_model(&self) -> TargetCapabilityModelIdentityV1 {
        self.target_model
    }

    pub const fn launch_evidence(&self) -> &ProductionTargetLaunchEvidenceV1 {
        &self.launch_evidence
    }

    pub fn decisions(&self) -> &[TargetCapabilityDecisionV1] {
        &self.decisions
    }

    /// Existing implementation owner aligned one-for-one with `decisions`.
    pub fn capability_owners(&self) -> &[ProductionAmdCapabilityOwnerV1] {
        &self.owners
    }

    pub fn legalization_records(&self) -> &[ProductionTargetLegalizationRecordV1] {
        &self.records
    }

    pub fn artifact_only_requirements(&self) -> &[ProductionArtifactOnlyRequirementV1] {
        &self.artifact_only_requirements
    }

    pub const fn proves_semantic_refinement(&self) -> bool {
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
}

/// Successful V13 query closure produced by the same module core as V12.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionTargetCapabilityClosureV13 {
    identity: [u8; 32],
    canonical_bytes: Box<[u8]>,
    subject: ProductionCanonicalGraphSubjectV1,
    target_model: TargetCapabilityModelIdentityV1,
    launch_evidence: ProductionTargetLaunchEvidenceV13,
    decisions: Box<[TargetCapabilityDecisionV1]>,
    owners: Box<[ProductionAmdCapabilityOwnerV1]>,
    records: Box<[ProductionTargetLegalizationRecordV1]>,
    artifact_only_requirements: Box<[ProductionArtifactOnlyRequirementV1]>,
}

impl ProductionTargetCapabilityClosureV13 {
    /// Strictly decodes an owned V1 closure and replays every target query.
    pub fn decode_canonical(
        bytes: &[u8],
        launch_evidence: &ProductionTargetLaunchEvidenceV13,
    ) -> Result<Self, ProductionTargetCapabilityCanonicalErrorV1> {
        let decoded = decode_capability_closure(bytes)?;
        if decoded.subject.version() != ProductionCanonicalGraphVersionV1::V13 {
            return Err(ProductionTargetCapabilityCanonicalErrorV1::GraphVersionMismatch);
        }
        if decoded.subject != launch_evidence.subject()
            || decoded.launch_evidence_identity != launch_evidence.identity()
        {
            return Err(ProductionTargetCapabilityCanonicalErrorV1::LaunchEvidenceMismatch);
        }
        Ok(Self {
            identity: capability_closure_identity(bytes),
            canonical_bytes: bytes.to_vec().into_boxed_slice(),
            subject: decoded.subject,
            target_model: decoded.target_model,
            launch_evidence: launch_evidence.clone(),
            decisions: decoded.decisions,
            owners: decoded.owners,
            records: decoded.records,
            artifact_only_requirements: decoded.artifact_only_requirements,
        })
    }

    pub const fn identity(&self) -> [u8; 32] {
        self.identity
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn subject(&self) -> ProductionCanonicalGraphSubjectV1 {
        self.subject
    }

    pub const fn target_model(&self) -> TargetCapabilityModelIdentityV1 {
        self.target_model
    }

    pub const fn launch_evidence(&self) -> &ProductionTargetLaunchEvidenceV13 {
        &self.launch_evidence
    }

    pub fn decisions(&self) -> &[TargetCapabilityDecisionV1] {
        &self.decisions
    }

    pub fn capability_owners(&self) -> &[ProductionAmdCapabilityOwnerV1] {
        &self.owners
    }

    pub fn legalization_records(&self) -> &[ProductionTargetLegalizationRecordV1] {
        &self.records
    }

    pub fn artifact_only_requirements(&self) -> &[ProductionArtifactOnlyRequirementV1] {
        &self.artifact_only_requirements
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
}

struct ProductionTargetCapabilityCoreClosureV1 {
    identity: [u8; 32],
    canonical_bytes: Box<[u8]>,
    target_model: TargetCapabilityModelIdentityV1,
    decisions: Box<[TargetCapabilityDecisionV1]>,
    owners: Box<[ProductionAmdCapabilityOwnerV1]>,
    records: Box<[ProductionTargetLegalizationRecordV1]>,
    artifact_only_requirements: Box<[ProductionArtifactOnlyRequirementV1]>,
}

/// V12 target-bound custody available only after capability closure succeeds.
#[derive(Debug)]
pub struct ProductionCapabilityLegalizedTargetBoundKernelIrV1 {
    target_bound: ProductionTargetBoundKernelIrV1,
    canonical_target_bound: VerifiedCanonicalKernelIrV12,
    input_epoch: u64,
    output_epoch: u64,
    capability_closure: ProductionTargetCapabilityClosureV1,
}

impl ProductionCapabilityLegalizedTargetBoundKernelIrV1 {
    pub fn module(&self) -> &Module {
        self.target_bound.module()
    }

    pub fn kernel_ids(&self) -> &[KernelId] {
        self.target_bound.kernel_ids()
    }

    pub const fn profile(&self) -> ProductionAmdTargetProfileV1 {
        self.target_bound.profile()
    }

    pub const fn input_epoch(&self) -> u64 {
        self.input_epoch
    }

    pub const fn output_epoch(&self) -> u64 {
        self.output_epoch
    }

    pub const fn capability_closure(&self) -> &ProductionTargetCapabilityClosureV1 {
        &self.capability_closure
    }

    pub const fn target_bound_identity(&self) -> &VerifiedCanonicalKernelIrIdentityV12 {
        self.canonical_target_bound.identity()
    }

    pub fn into_parts(
        self,
    ) -> (
        ProductionTargetBoundKernelIrV1,
        VerifiedCanonicalKernelIrV12,
        ProductionTargetCapabilityClosureV1,
    ) {
        (
            self.target_bound,
            self.canonical_target_bound,
            self.capability_closure,
        )
    }
}

/// Fail-closed errors for the bounded binary capability-closure replay boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionTargetCapabilityCanonicalErrorV1 {
    Oversized { length: usize },
    Truncated,
    InvalidMagic,
    UnsupportedWireVersion(u16),
    InvalidTag { field: &'static str, tag: u8 },
    InvalidBoolean(u8),
    InvalidRequirement,
    InvalidTargetModel,
    NonCanonical,
    GraphVersionMismatch,
    LaunchEvidenceMismatch,
}

impl fmt::Display for ProductionTargetCapabilityCanonicalErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Oversized { length } => write!(
                formatter,
                "capability closure has {length} bytes; maximum is {MAX_PRODUCTION_TARGET_CAPABILITY_CLOSURE_BYTES_V1}"
            ),
            Self::Truncated => formatter.write_str("truncated capability closure"),
            Self::InvalidMagic => formatter.write_str("invalid capability-closure magic"),
            Self::UnsupportedWireVersion(version) => {
                write!(
                    formatter,
                    "unsupported capability-closure wire version {version}"
                )
            }
            Self::InvalidTag { field, tag } => {
                write!(formatter, "invalid {field} tag {tag}")
            }
            Self::InvalidBoolean(value) => write!(formatter, "invalid canonical boolean {value}"),
            Self::InvalidRequirement => formatter.write_str("invalid capability requirement"),
            Self::InvalidTargetModel => formatter.write_str("unknown target model identity"),
            Self::NonCanonical => formatter.write_str("noncanonical capability closure"),
            Self::GraphVersionMismatch => {
                formatter.write_str("canonical graph version does not match closure type")
            }
            Self::LaunchEvidenceMismatch => formatter.write_str(
                "capability closure does not match the exact launch-evidence subject and identity",
            ),
        }
    }
}

impl Error for ProductionTargetCapabilityCanonicalErrorV1 {}

/// Closed failure modes for exact production capability legalization.
#[derive(Debug)]
pub enum ProductionTargetCapabilityErrorV1 {
    InvalidCanonicalKir(VerifiedCanonicalKernelIrErrorV12),
    InvalidCanonicalKirV13(VerifiedCanonicalKernelIrErrorV13),
    CanonicalKirDecode(KernelIrDecodeError),
    InvalidTargetModel(ProductionAmdTargetCapabilityModelErrorV1),
    Query {
        requirement: TargetCapabilityRequirementV1,
        source: TargetCapabilityQueryErrorV1,
    },
    OmittedAxis {
        capability: TargetCapability,
        axis: OmittedCapabilityAxisV1,
    },
    NeutralSelectedAmdProfile {
        name: String,
    },
    UnsupportedExtension {
        namespace: String,
        name: String,
    },
    Rejected {
        requirement: TargetCapabilityRequirementV1,
        outcome: TargetCapabilityDecisionOutcomeV1,
    },
    MissingCapabilityOwner {
        requirement: TargetCapabilityRequirementV1,
    },
    MissingOperationLoweringOwner {
        operation: &'static str,
    },
    LaunchEvidenceSubjectMismatch,
    DuplicateLaunchEvidence {
        kind: TargetLaunchEvidenceKindV1,
    },
    InvalidLaunchEvidenceValue {
        value: ProductionTargetLaunchEvidenceValueV1,
    },
    MissingLaunchEvidence {
        requirement: TargetCapabilityRequirementV1,
        kind: TargetLaunchEvidenceKindV1,
    },
    LaunchEvidenceValueMismatch {
        requirement: TargetCapabilityRequirementV1,
        observed: ProductionTargetLaunchEvidenceValueV1,
    },
    UnusedLaunchEvidence {
        kind: TargetLaunchEvidenceKindV1,
    },
    EmptyKernelClosure,
    MissingWorkgroupSize {
        kernel: KernelId,
    },
    WorkgroupSizeOverflow {
        kernel: KernelId,
    },
    UnknownStaticResourceFootprint {
        operation: &'static str,
    },
    StaticResourceAggregateOverflow {
        kind: StaticResourceKindV1,
    },
    CapabilityClosureTooLarge {
        records: usize,
    },
    EpochOverflow,
    TargetBinding(ProductionTargetBindingErrorV1),
}

impl ProductionTargetCapabilityErrorV1 {
    pub const fn diagnostic_code(&self) -> ProductionTargetCapabilityDiagnosticCodeV1 {
        match self {
            Self::InvalidCanonicalKir(_)
            | Self::InvalidCanonicalKirV13(_)
            | Self::CanonicalKirDecode(_) => {
                ProductionTargetCapabilityDiagnosticCodeV1::InvalidCanonicalKir
            }
            Self::InvalidTargetModel(_) => {
                ProductionTargetCapabilityDiagnosticCodeV1::InvalidTargetModel
            }
            Self::Query { .. } => ProductionTargetCapabilityDiagnosticCodeV1::InvalidRequirement,
            Self::MissingCapabilityOwner { .. } | Self::MissingOperationLoweringOwner { .. } => {
                ProductionTargetCapabilityDiagnosticCodeV1::Incomplete
            }
            Self::OmittedAxis { .. } => ProductionTargetCapabilityDiagnosticCodeV1::OmittedAxis,
            Self::Rejected {
                outcome: TargetCapabilityDecisionOutcomeV1::Unsupported,
                ..
            } => ProductionTargetCapabilityDiagnosticCodeV1::Unsupported,
            Self::Rejected {
                outcome: TargetCapabilityDecisionOutcomeV1::Incomplete,
                ..
            } => ProductionTargetCapabilityDiagnosticCodeV1::Incomplete,
            Self::Rejected {
                outcome: TargetCapabilityDecisionOutcomeV1::Unreviewed,
                ..
            } => ProductionTargetCapabilityDiagnosticCodeV1::Unreviewed,
            Self::Rejected { .. } | Self::MissingLaunchEvidence { .. } => {
                ProductionTargetCapabilityDiagnosticCodeV1::MissingLaunchEvidence
            }
            Self::LaunchEvidenceSubjectMismatch
            | Self::DuplicateLaunchEvidence { .. }
            | Self::InvalidLaunchEvidenceValue { .. }
            | Self::LaunchEvidenceValueMismatch { .. }
            | Self::UnusedLaunchEvidence { .. } => {
                ProductionTargetCapabilityDiagnosticCodeV1::LaunchEvidenceMismatch
            }
            Self::NeutralSelectedAmdProfile { .. } => {
                ProductionTargetCapabilityDiagnosticCodeV1::NeutralTargetSelection
            }
            Self::UnsupportedExtension { .. } => {
                ProductionTargetCapabilityDiagnosticCodeV1::UnsupportedExtension
            }
            Self::EmptyKernelClosure
            | Self::MissingWorkgroupSize { .. }
            | Self::WorkgroupSizeOverflow { .. }
            | Self::UnknownStaticResourceFootprint { .. }
            | Self::StaticResourceAggregateOverflow { .. }
            | Self::CapabilityClosureTooLarge { .. } => {
                ProductionTargetCapabilityDiagnosticCodeV1::InvalidLaunchShape
            }
            Self::EpochOverflow => ProductionTargetCapabilityDiagnosticCodeV1::EpochOverflow,
            Self::TargetBinding(_) => ProductionTargetCapabilityDiagnosticCodeV1::TargetBinding,
        }
    }
}

impl fmt::Display for ProductionTargetCapabilityErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: ", self.diagnostic_code())?;
        match self {
            Self::InvalidCanonicalKir(error) => write!(formatter, "{error}"),
            Self::InvalidCanonicalKirV13(error) => write!(formatter, "{error}"),
            Self::CanonicalKirDecode(error) => {
                write!(
                    formatter,
                    "verified canonical Kernel IR V12 could not be decoded: {error}"
                )
            }
            Self::InvalidTargetModel(error) => write!(formatter, "{error}"),
            Self::Query {
                requirement,
                source,
            } => write!(formatter, "cannot query [{requirement}]: {source}"),
            Self::OmittedAxis { capability, axis } => write!(
                formatter,
                "Kernel IR capability {capability:?} omits the exact {axis} required for target legalization"
            ),
            Self::NeutralSelectedAmdProfile { name } => write!(
                formatter,
                "target-neutral Kernel IR selected AMD profile {name}; only the AMD target binder may select a processor profile"
            ),
            Self::UnsupportedExtension { namespace, name } => write!(
                formatter,
                "target capability extension {namespace}:{name} has no exact neutral query adapter"
            ),
            Self::Rejected {
                requirement,
                outcome,
            } => write!(
                formatter,
                "target decision for [{requirement}] is {outcome}, not an admitted final result"
            ),
            Self::MissingCapabilityOwner { requirement } => write!(
                formatter,
                "target tuple [{requirement}] has a final answer but no existing production implementation owner"
            ),
            Self::MissingOperationLoweringOwner { operation } => write!(
                formatter,
                "{operation} has no production AMD lowering owner"
            ),
            Self::LaunchEvidenceSubjectMismatch => formatter.write_str(
                "launch evidence does not name the exact neutral Kernel IR identity and epoch",
            ),
            Self::DuplicateLaunchEvidence { kind } => {
                write!(formatter, "launch evidence contains duplicate {kind}")
            }
            Self::InvalidLaunchEvidenceValue { value } => {
                write!(
                    formatter,
                    "launch evidence contains invalid value {value:?}"
                )
            }
            Self::MissingLaunchEvidence { requirement, kind } => write!(
                formatter,
                "target decision for [{requirement}] requires exact launch evidence {kind}"
            ),
            Self::LaunchEvidenceValueMismatch {
                requirement,
                observed,
            } => write!(
                formatter,
                "launch evidence {observed:?} does not exactly match [{requirement}]"
            ),
            Self::UnusedLaunchEvidence { kind } => {
                write!(
                    formatter,
                    "launch evidence {kind} was not required by the closure"
                )
            }
            Self::EmptyKernelClosure => {
                formatter.write_str("target legalization requires at least one kernel")
            }
            Self::MissingWorkgroupSize { kernel } => write!(
                formatter,
                "kernel {kernel} has no exact workgroup dimensions for target legalization"
            ),
            Self::WorkgroupSizeOverflow { kernel } => write!(
                formatter,
                "kernel {kernel} workgroup dimensions overflow the production resource model"
            ),
            Self::UnknownStaticResourceFootprint { operation } => write!(
                formatter,
                "{operation} has an unknown or overflowing static resource footprint"
            ),
            Self::StaticResourceAggregateOverflow { kind } => write!(
                formatter,
                "the complete module's {kind:?} resource footprint overflows u64"
            ),
            Self::CapabilityClosureTooLarge { records } => write!(
                formatter,
                "target-capability closure has {records} records; maximum is {MAX_TARGET_CAPABILITY_CLOSURE_REQUIREMENTS_V1}"
            ),
            Self::EpochOverflow => {
                formatter.write_str("target binding cannot advance the canonical graph epoch")
            }
            Self::TargetBinding(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for ProductionTargetCapabilityErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidCanonicalKir(error) => Some(error),
            Self::InvalidCanonicalKirV13(error) => Some(error),
            Self::CanonicalKirDecode(error) => Some(error),
            Self::InvalidTargetModel(error) => Some(error),
            Self::Query { source, .. } => Some(source),
            Self::TargetBinding(error) => Some(error),
            _ => None,
        }
    }
}

/// Checks every effective KIR capability against one exact AMD target model.
///
/// The result is inert evidence about one graph epoch. It cannot publish, load,
/// or launch an artifact, and nonfinal target answers are compilation errors.
pub fn legalize_production_target_capabilities_v1(
    neutral: &VerifiedCanonicalKernelIrV12,
    neutral_epoch: u64,
    launch_evidence: &ProductionTargetLaunchEvidenceV1,
    profile: ProductionAmdTargetProfileV1,
) -> Result<ProductionTargetCapabilityClosureV1, ProductionTargetCapabilityErrorV1> {
    neutral
        .revalidate()
        .map_err(ProductionTargetCapabilityErrorV1::InvalidCanonicalKir)?;
    let module = decode_module_v12(neutral.canonical_bytes())
        .map_err(ProductionTargetCapabilityErrorV1::CanonicalKirDecode)?;
    let subject = ProductionCanonicalGraphSubjectV1::from_v12(*neutral.identity(), neutral_epoch);
    let core = legalize_production_target_capability_module_v1(
        &module,
        subject,
        launch_evidence,
        profile,
    )?;
    Ok(ProductionTargetCapabilityClosureV1 {
        identity: core.identity,
        canonical_bytes: core.canonical_bytes,
        neutral_kernel_ir: *neutral.identity(),
        neutral_epoch,
        target_model: core.target_model,
        launch_evidence: launch_evidence.clone(),
        decisions: core.decisions,
        owners: core.owners,
        records: core.records,
        artifact_only_requirements: core.artifact_only_requirements,
    })
}

/// Checked V13 adapter into the version-independent module query core.
pub fn legalize_production_target_capabilities_v13(
    neutral: &VerifiedCanonicalKernelIrV13,
    neutral_epoch: u64,
    launch_evidence: &ProductionTargetLaunchEvidenceV13,
    profile: ProductionAmdTargetProfileV1,
) -> Result<ProductionTargetCapabilityClosureV13, ProductionTargetCapabilityErrorV1> {
    neutral
        .revalidate()
        .map_err(ProductionTargetCapabilityErrorV1::InvalidCanonicalKirV13)?;
    let module = decode_module_v13(neutral.canonical_bytes())
        .map_err(ProductionTargetCapabilityErrorV1::CanonicalKirDecode)?;
    let subject = ProductionCanonicalGraphSubjectV1::from_v13(*neutral.identity(), neutral_epoch);
    let core = legalize_production_target_capability_module_v1(
        &module,
        subject,
        launch_evidence,
        profile,
    )?;
    Ok(ProductionTargetCapabilityClosureV13 {
        identity: core.identity,
        canonical_bytes: core.canonical_bytes,
        subject,
        target_model: core.target_model,
        launch_evidence: launch_evidence.clone(),
        decisions: core.decisions,
        owners: core.owners,
        records: core.records,
        artifact_only_requirements: core.artifact_only_requirements,
    })
}

/// Single target-neutral `Module` query core shared by canonical adapters.
fn legalize_production_target_capability_module_v1(
    module: &Module,
    subject: ProductionCanonicalGraphSubjectV1,
    launch_evidence: &impl ExactLaunchEvidenceV1,
    profile: ProductionAmdTargetProfileV1,
) -> Result<ProductionTargetCapabilityCoreClosureV1, ProductionTargetCapabilityErrorV1> {
    if launch_evidence.subject() != subject
        || launch_evidence.identity()
            != launch_evidence_identity(launch_evidence.subject(), launch_evidence.values())
    {
        return Err(ProductionTargetCapabilityErrorV1::LaunchEvidenceSubjectMismatch);
    }

    let model = profile
        .capability_model()
        .map_err(ProductionTargetCapabilityErrorV1::InvalidTargetModel)?;
    let target_model = fe2o3_target_spec::TargetCapabilityQueryV1::model_identity(&model);
    let mut roots = production_target_requirements_for_module_v1(module)?;
    roots.extend(exact_pre_artifact_binding_requirements());
    let graph = exact_requirement_graph(&roots)?;
    let mut decisions = Vec::with_capacity(graph.len());
    let mut owners = Vec::with_capacity(graph.len());
    let mut records = Vec::with_capacity(graph.len());
    let mut consumed_evidence = BTreeSet::new();
    for (requirement, dependencies) in graph {
        let decision = query_target_capability_v1(&model, requirement).map_err(|source| {
            ProductionTargetCapabilityErrorV1::Query {
                requirement,
                source,
            }
        })?;
        match decision.outcome() {
            TargetCapabilityDecisionOutcomeV1::Supported => {}
            TargetCapabilityDecisionOutcomeV1::DynamicLaunchEvidenceRequired(kind) => {
                let observed = launch_evidence.value(kind).ok_or(
                    ProductionTargetCapabilityErrorV1::MissingLaunchEvidence { requirement, kind },
                )?;
                if !launch_evidence_matches(requirement, observed) {
                    return Err(
                        ProductionTargetCapabilityErrorV1::LaunchEvidenceValueMismatch {
                            requirement,
                            observed,
                        },
                    );
                }
                consumed_evidence.insert(kind);
            }
            outcome => {
                return Err(ProductionTargetCapabilityErrorV1::Rejected {
                    requirement,
                    outcome,
                });
            }
        }
        let owner = model
            .capability_owner(requirement)
            .ok_or(ProductionTargetCapabilityErrorV1::MissingCapabilityOwner { requirement })?;
        decisions.push(decision);
        owners.push(owner);
        records.push(ProductionTargetLegalizationRecordV1 {
            root: roots.contains(&requirement),
            decision,
            dependencies: dependencies
                .into_iter()
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            owner,
        });
    }
    for value in launch_evidence.values() {
        if !consumed_evidence.contains(&value.kind()) {
            return Err(ProductionTargetCapabilityErrorV1::UnusedLaunchEvidence {
                kind: value.kind(),
            });
        }
    }
    let decisions = decisions.into_boxed_slice();
    let owners = owners.into_boxed_slice();
    let records = records.into_boxed_slice();
    let artifact_only_requirements = exact_artifact_only_requirements();
    let canonical_bytes = encode_capability_closure(
        subject,
        target_model,
        launch_evidence.identity(),
        &records,
        &artifact_only_requirements,
    )?
    .into_boxed_slice();
    let identity = capability_closure_identity(&canonical_bytes);
    Ok(ProductionTargetCapabilityCoreClosureV1 {
        identity,
        canonical_bytes,
        target_model,
        decisions,
        owners,
        records,
        artifact_only_requirements,
    })
}

/// Runs capability closure before the existing structural AMD target transform.
pub fn bind_capability_legalized_production_target_v1(
    neutral: VerifiedCanonicalKernelIrV12,
    neutral_epoch: u64,
    launch_evidence: &ProductionTargetLaunchEvidenceV1,
    profile: ProductionAmdTargetProfileV1,
) -> Result<ProductionCapabilityLegalizedTargetBoundKernelIrV1, ProductionTargetCapabilityErrorV1> {
    let capability_closure = legalize_production_target_capabilities_v1(
        &neutral,
        neutral_epoch,
        launch_evidence,
        profile,
    )?;
    let module = decode_module_v12(neutral.canonical_bytes())
        .map_err(ProductionTargetCapabilityErrorV1::CanonicalKirDecode)?;
    let output_epoch = neutral_epoch
        .checked_add(1)
        .ok_or(ProductionTargetCapabilityErrorV1::EpochOverflow)?;
    let target_bound = bind_production_target_v1(&module, profile)
        .map_err(ProductionTargetCapabilityErrorV1::TargetBinding)?;
    let canonical_target_bound =
        VerifiedCanonicalKernelIrV12::from_module(target_bound.module().clone())
            .map_err(ProductionTargetCapabilityErrorV1::InvalidCanonicalKir)?;
    Ok(ProductionCapabilityLegalizedTargetBoundKernelIrV1 {
        target_bound,
        canonical_target_bound,
        input_epoch: neutral_epoch,
        output_epoch,
        capability_closure,
    })
}

/// Extracts graph requirements without selecting a target or adding target ABI facts.
pub fn production_target_requirements_for_module_v1(
    module: &Module,
) -> Result<BTreeSet<TargetCapabilityRequirementV1>, ProductionTargetCapabilityErrorV1> {
    if module.kernels.is_empty() {
        return Err(ProductionTargetCapabilityErrorV1::EmptyKernelClosure);
    }
    let mut requirements = BTreeSet::new();
    let mut derived_v13_declarations = BTreeSet::new();
    let mut static_workgroup_bytes = 0_u64;
    let mut private_bytes_per_invocation = 0_u64;

    let mut maximum_workgroup_invocations = 0_u32;
    for kernel in &module.kernels {
        let size = kernel.workgroup_size.ok_or_else(|| {
            ProductionTargetCapabilityErrorV1::MissingWorkgroupSize {
                kernel: kernel.id.clone(),
            }
        })?;
        let invocations = size
            .x
            .checked_mul(size.y)
            .and_then(|value| value.checked_mul(size.z))
            .ok_or_else(
                || ProductionTargetCapabilityErrorV1::WorkgroupSizeOverflow {
                    kernel: kernel.id.clone(),
                },
            )?;
        maximum_workgroup_invocations = maximum_workgroup_invocations.max(invocations);
        requirements.insert(TargetCapabilityRequirementV1::Resource(
            TargetResourceRequirementV1::WorkgroupDimensions {
                x: size.x,
                y: size.y,
                z: size.z,
            },
        ));
    }
    requirements.insert(TargetCapabilityRequirementV1::Resource(
        TargetResourceRequirementV1::WorkgroupInvocationsAtMost(maximum_workgroup_invocations),
    ));

    for function in &module.functions {
        for ty in function
            .signature
            .parameters
            .iter()
            .chain(&function.signature.results)
        {
            requirements_for_type(ty, &mut requirements)?;
        }
        if let Some(body) = &function.body {
            let mut value_types = BTreeMap::new();
            for (value, ty) in body.parameters.iter().zip(&function.signature.parameters) {
                value_types.insert(*value, ty.clone());
            }
            for block in &body.blocks {
                for value in &block.parameters {
                    requirements_for_type(&value.ty, &mut requirements)?;
                    value_types.insert(value.id, value.ty.clone());
                }
                for operation in &block.operations {
                    for value in &operation.results {
                        requirements_for_type(&value.ty, &mut requirements)?;
                        value_types.insert(value.id, value.ty.clone());
                    }
                    requirements_for_operation(
                        operation.kind.clone(),
                        &value_types,
                        &mut requirements,
                    )?;
                    accumulate_static_resource_footprint(
                        &operation.kind,
                        &mut static_workgroup_bytes,
                        &mut private_bytes_per_invocation,
                    )?;
                    if let OperationKind::ExecutionCapability(contract) = &operation.kind {
                        derived_v13_declarations.extend(contract.operation.required_capabilities());
                    }
                }
            }
        }
    }

    // Keep per-operation requirements for exact lowering authorization and add the complete
    // module footprint for resource admission. A larger aggregate is not a substitute for an
    // operation's exact requirement because closure membership is intentionally non-lossy.
    if static_workgroup_bytes != 0 {
        requirements.insert(TargetCapabilityRequirementV1::Resource(
            TargetResourceRequirementV1::StaticSharedMemoryBytesAtMost(static_workgroup_bytes),
        ));
    }
    if private_bytes_per_invocation != 0 {
        requirements.insert(TargetCapabilityRequirementV1::Resource(
            TargetResourceRequirementV1::PrivateMemoryBytesPerInvocationAtMost(
                private_bytes_per_invocation,
            ),
        ));
    }

    let mut capabilities = module.required_capabilities.clone();
    capabilities.extend(
        module
            .functions
            .iter()
            .flat_map(|function| function.required_capabilities.iter().cloned()),
    );
    capabilities.extend(
        module
            .kernels
            .iter()
            .flat_map(|kernel| kernel.required_capabilities.iter().cloned()),
    );
    for capability in capabilities {
        if derived_v13_declarations.contains(&capability) {
            continue;
        }
        requirements.extend(requirements_for_capability(&capability)?);
    }
    Ok(requirements)
}

fn accumulate_static_resource_footprint(
    kind: &OperationKind,
    static_workgroup_bytes: &mut u64,
    private_bytes_per_invocation: &mut u64,
) -> Result<(), ProductionTargetCapabilityErrorV1> {
    let allocation = match kind {
        OperationKind::ExecutionCapability(contract) => match &contract.operation {
            ExecutionCapabilityOperationV1::LdsAllocate {
                layout, elements, ..
            }
            | ExecutionCapabilityOperationV1::WorkgroupMemoryAllocate {
                layout, elements, ..
            } => Some((
                StaticResourceKindV1::Workgroup,
                layout.checked_footprint(*elements).ok_or(
                    ProductionTargetCapabilityErrorV1::UnknownStaticResourceFootprint {
                        operation: execution_operation_name(&contract.operation),
                    },
                )?,
            )),
            ExecutionCapabilityOperationV1::PrivateMemoryAllocate {
                layout, elements, ..
            } => Some((
                StaticResourceKindV1::Private,
                layout.checked_footprint(*elements).ok_or(
                    ProductionTargetCapabilityErrorV1::UnknownStaticResourceFootprint {
                        operation: execution_operation_name(&contract.operation),
                    },
                )?,
            )),
            _ => None,
        },
        OperationKind::WorkgroupMemory(memory) => match memory.extent {
            WorkgroupMemoryExtent::Static(elements) => Some((
                StaticResourceKindV1::Workgroup,
                type_storage_bytes(&memory.element)
                    .and_then(|bytes| bytes.checked_mul(u64::from(elements)))
                    .ok_or(
                        ProductionTargetCapabilityErrorV1::UnknownStaticResourceFootprint {
                            operation: "workgroup-memory",
                        },
                    )?,
            )),
            WorkgroupMemoryExtent::Dynamic | WorkgroupMemoryExtent::DynamicAtLeast(_) => None,
        },
        OperationKind::Alloca {
            element,
            count,
            address_space,
            ..
        } if matches!(
            address_space,
            AddressSpace::Workgroup | AddressSpace::Private
        ) =>
        {
            if count.is_some() {
                return Err(
                    ProductionTargetCapabilityErrorV1::UnknownStaticResourceFootprint {
                        operation: "dynamic-alloca",
                    },
                );
            }
            Some((
                if *address_space == AddressSpace::Workgroup {
                    StaticResourceKindV1::Workgroup
                } else {
                    StaticResourceKindV1::Private
                },
                type_storage_bytes(element).ok_or(
                    ProductionTargetCapabilityErrorV1::UnknownStaticResourceFootprint {
                        operation: "alloca",
                    },
                )?,
            ))
        }
        _ => None,
    };
    let Some((kind, bytes)) = allocation else {
        return Ok(());
    };
    let total = match kind {
        StaticResourceKindV1::Workgroup => static_workgroup_bytes,
        StaticResourceKindV1::Private => private_bytes_per_invocation,
    };
    *total = total
        .checked_add(bytes)
        .ok_or(ProductionTargetCapabilityErrorV1::StaticResourceAggregateOverflow { kind })?;
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StaticResourceKindV1 {
    Workgroup,
    Private,
}

fn exact_pre_artifact_binding_requirements() -> BTreeSet<TargetCapabilityRequirementV1> {
    BTreeSet::from([
        TargetCapabilityRequirementV1::Abi(TargetAbiConstraintV1::PointerWidth(64)),
        TargetCapabilityRequirementV1::Abi(TargetAbiConstraintV1::Endianness(
            TargetEndiannessV1::Little,
        )),
    ])
}

fn exact_artifact_only_requirements() -> Box<[ProductionArtifactOnlyRequirementV1]> {
    Box::new([
        ProductionArtifactOnlyRequirementV1::ExactKernelArgumentLayout,
        ProductionArtifactOnlyRequirementV1::ExactRegistersPerInvocation,
        ProductionArtifactOnlyRequirementV1::ExactPrivateSegmentBytes,
        ProductionArtifactOnlyRequirementV1::ExactScratchBytes,
        ProductionArtifactOnlyRequirementV1::ExactLoadableObject,
    ])
}

fn exact_requirement_graph(
    roots: &BTreeSet<TargetCapabilityRequirementV1>,
) -> Result<
    BTreeMap<TargetCapabilityRequirementV1, BTreeSet<TargetCapabilityRequirementV1>>,
    ProductionTargetCapabilityErrorV1,
> {
    let mut graph = BTreeMap::new();
    let mut pending = roots.clone();
    while let Some(requirement) = pending.pop_first() {
        if graph.contains_key(&requirement) {
            continue;
        }
        let dependencies = exact_requirement_dependencies(requirement);
        if dependencies.len() > MAX_PRODUCTION_TARGET_CAPABILITY_DEPENDENCIES_V1 {
            return Err(
                ProductionTargetCapabilityErrorV1::CapabilityClosureTooLarge {
                    records: graph.len() + 1,
                },
            );
        }
        pending.extend(dependencies.iter().copied());
        graph.insert(requirement, dependencies);
        if graph.len() + pending.len() > MAX_TARGET_CAPABILITY_CLOSURE_REQUIREMENTS_V1 {
            return Err(
                ProductionTargetCapabilityErrorV1::CapabilityClosureTooLarge {
                    records: graph.len() + pending.len(),
                },
            );
        }
    }
    Ok(graph)
}

fn exact_requirement_dependencies(
    requirement: TargetCapabilityRequirementV1,
) -> BTreeSet<TargetCapabilityRequirementV1> {
    let mut dependencies = BTreeSet::new();
    match requirement {
        TargetCapabilityRequirementV1::Atomic(atomic) => {
            dependencies.insert(TargetCapabilityRequirementV1::AddressSpace(
                atomic.address_space(),
                TargetMemoryAccessV1::ReadWrite,
            ));
        }
        TargetCapabilityRequirementV1::Barrier(barrier) => {
            dependencies.insert(TargetCapabilityRequirementV1::AddressSpace(
                barrier.address_space(),
                TargetMemoryAccessV1::ReadWrite,
            ));
        }
        TargetCapabilityRequirementV1::Fence(fence) => {
            dependencies.insert(TargetCapabilityRequirementV1::AddressSpace(
                fence.address_space(),
                TargetMemoryAccessV1::ReadWrite,
            ));
        }
        TargetCapabilityRequirementV1::Collective(collective) => {
            match collective.execution_scope() {
                TargetExecutionScopeV1::Subgroup => {
                    dependencies.insert(TargetCapabilityRequirementV1::SubgroupSize(
                        u16::try_from(collective.participants()).unwrap_or(u16::MAX),
                    ));
                }
                TargetExecutionScopeV1::Workgroup => {
                    dependencies.insert(TargetCapabilityRequirementV1::AddressSpace(
                        TargetAddressSpaceV1::Workgroup,
                        TargetMemoryAccessV1::ReadWrite,
                    ));
                }
                TargetExecutionScopeV1::Grid => {}
            }
        }
        TargetCapabilityRequirementV1::Matrix(matrix) => {
            dependencies.insert(TargetCapabilityRequirementV1::SubgroupSize(
                matrix.subgroup_size(),
            ));
        }
        TargetCapabilityRequirementV1::AsyncCopy(copy) => {
            dependencies.insert(TargetCapabilityRequirementV1::AddressSpace(
                copy.source(),
                TargetMemoryAccessV1::Read,
            ));
            dependencies.insert(TargetCapabilityRequirementV1::AddressSpace(
                copy.destination(),
                TargetMemoryAccessV1::Write,
            ));
        }
        TargetCapabilityRequirementV1::Numerical(numerical) => {
            dependencies.insert(TargetCapabilityRequirementV1::ScalarType(
                numerical.value_type(),
            ));
        }
        TargetCapabilityRequirementV1::ScalarType(_)
        | TargetCapabilityRequirementV1::SubgroupSize(_)
        | TargetCapabilityRequirementV1::AddressSpace(_, _)
        | TargetCapabilityRequirementV1::AsyncWait(_)
        | TargetCapabilityRequirementV1::Resource(_)
        | TargetCapabilityRequirementV1::Abi(_)
        | TargetCapabilityRequirementV1::Object(_) => {}
    }
    dependencies
}

fn requirements_for_type(
    ty: &Type,
    requirements: &mut BTreeSet<TargetCapabilityRequirementV1>,
) -> Result<(), ProductionTargetCapabilityErrorV1> {
    match ty {
        Type::Scalar(ScalarType::Index) | Type::Unit => {}
        Type::Scalar(scalar) => {
            requirements.insert(TargetCapabilityRequirementV1::ScalarType(target_scalar(
                *scalar,
            )?));
        }
        Type::Pointer(pointer) => {
            requirements.insert(TargetCapabilityRequirementV1::AddressSpace(
                target_address_space(pointer.address_space),
                target_access(pointer.access),
            ));
            requirements_for_type(&pointer.pointee, requirements)?;
        }
        Type::Slice(slice) => {
            requirements.insert(TargetCapabilityRequirementV1::AddressSpace(
                target_address_space(slice.address_space),
                target_access(slice.access),
            ));
            requirements_for_type(&slice.element, requirements)?;
        }
        Type::KernelContext(_) | Type::GlobalCapability(_) | Type::ExecutionCapability(_) => {}
    }
    Ok(())
}

fn requirements_for_operation(
    kind: OperationKind,
    value_types: &BTreeMap<fe2o3_kernel_ir::ValueId, Type>,
    requirements: &mut BTreeSet<TargetCapabilityRequirementV1>,
) -> Result<(), ProductionTargetCapabilityErrorV1> {
    let address = |space, access| {
        TargetCapabilityRequirementV1::AddressSpace(
            target_address_space(space),
            target_access(access),
        )
    };
    match kind {
        OperationKind::Alloca {
            element,
            address_space,
            ..
        } => {
            requirements_for_type(&element, requirements)?;
            requirements.insert(address(address_space, AccessMode::ReadWrite));
        }
        OperationKind::Load { access, .. } | OperationKind::GuardedLoad { access, .. } => {
            requirements.insert(TargetCapabilityRequirementV1::AddressSpace(
                target_address_space(access.address_space),
                TargetMemoryAccessV1::Read,
            ));
        }
        OperationKind::Store { access, .. } | OperationKind::GuardedStore { access, .. } => {
            requirements.insert(TargetCapabilityRequirementV1::AddressSpace(
                target_address_space(access.address_space),
                TargetMemoryAccessV1::Write,
            ));
        }
        OperationKind::Barrier(barrier) => {
            let execution_scope = target_execution_scope(barrier.execution_scope).ok_or(
                ProductionTargetCapabilityErrorV1::OmittedAxis {
                    capability: TargetCapability::WorkgroupBarrier,
                    axis: OmittedCapabilityAxisV1::BarrierExecutionScope,
                },
            )?;
            for space in barrier.semantics.address_spaces {
                requirements.insert(TargetCapabilityRequirementV1::Barrier(
                    TargetBarrierRequirementV1::new(
                        execution_scope,
                        target_memory_scope(barrier.memory_scope),
                        target_address_space(space),
                        target_ordering(barrier.semantics.ordering),
                        TargetBarrierParticipationV1::DynamicMask,
                    ),
                ));
            }
        }
        OperationKind::WorkgroupBarrier(barrier) => {
            for space in barrier.semantics.address_spaces {
                requirements.insert(TargetCapabilityRequirementV1::Barrier(
                    TargetBarrierRequirementV1::new(
                        TargetExecutionScopeV1::Workgroup,
                        target_memory_scope(barrier.memory_scope),
                        target_address_space(space),
                        target_ordering(barrier.semantics.ordering),
                        TargetBarrierParticipationV1::Uniform,
                    ),
                ));
            }
        }
        OperationKind::Fence(fence) => {
            for space in fence.semantics.address_spaces {
                requirements.insert(TargetCapabilityRequirementV1::Fence(
                    TargetFenceRequirementV1::new(
                        target_memory_scope(fence.memory_scope),
                        target_address_space(space),
                        target_ordering(fence.semantics.ordering),
                    ),
                ));
            }
        }
        OperationKind::Atomic(atomic) => {
            let value_type = value_types
                .get(&atomic.pointer)
                .and_then(|ty| match ty {
                    Type::Pointer(pointer) => pointer.pointee.as_scalar(),
                    _ => None,
                })
                .ok_or(ProductionTargetCapabilityErrorV1::OmittedAxis {
                    capability: TargetCapability::Atomic {
                        width_bits: 0,
                        address_space: atomic.access.address_space,
                        max_scope: atomic.scope,
                    },
                    axis: OmittedCapabilityAxisV1::ScalarKind,
                })?;
            let operation = target_atomic_operation(atomic.kind).ok_or_else(|| {
                ProductionTargetCapabilityErrorV1::OmittedAxis {
                    capability: TargetCapability::Atomic {
                        width_bits: value_type.bit_width().unwrap_or(0),
                        address_space: atomic.access.address_space,
                        max_scope: atomic.scope,
                    },
                    axis: OmittedCapabilityAxisV1::AtomicOperation,
                }
            })?;
            let scalar = target_scalar(value_type)?;
            let requirement = if operation == TargetAtomicOperationV1::CompareExchange {
                TargetAtomicRequirementV1::compare_exchange(
                    scalar,
                    target_ordering(atomic.ordering),
                    target_ordering(atomic.failure_ordering.ok_or_else(|| {
                        ProductionTargetCapabilityErrorV1::OmittedAxis {
                            capability: TargetCapability::Atomic {
                                width_bits: value_type.bit_width().unwrap_or(0),
                                address_space: atomic.access.address_space,
                                max_scope: atomic.scope,
                            },
                            axis: OmittedCapabilityAxisV1::AtomicFailureOrdering,
                        }
                    })?),
                    target_memory_scope(atomic.scope),
                    target_address_space(atomic.access.address_space),
                )
            } else {
                TargetAtomicRequirementV1::new(
                    scalar,
                    operation,
                    target_ordering(atomic.ordering),
                    target_memory_scope(atomic.scope),
                    target_address_space(atomic.access.address_space),
                )
            };
            requirements.insert(TargetCapabilityRequirementV1::Atomic(requirement));
        }
        OperationKind::WorkgroupMemory(memory) => match memory.extent {
            WorkgroupMemoryExtent::Static(elements) => {
                let bytes = type_storage_bytes(&memory.element)
                    .and_then(|size| u64::from(elements).checked_mul(size))
                    .ok_or(ProductionTargetCapabilityErrorV1::OmittedAxis {
                        capability: TargetCapability::WorkgroupMemory,
                        axis: OmittedCapabilityAxisV1::DynamicSharedMemoryBytes,
                    })?;
                requirements.insert(TargetCapabilityRequirementV1::Resource(
                    TargetResourceRequirementV1::StaticSharedMemoryBytesAtMost(bytes),
                ));
            }
            WorkgroupMemoryExtent::Dynamic | WorkgroupMemoryExtent::DynamicAtLeast(_) => {
                return omitted(
                    &TargetCapability::DynamicWorkgroupMemory,
                    OmittedCapabilityAxisV1::DynamicSharedMemoryBytes,
                );
            }
        },
        OperationKind::Wave(wave) => {
            requirements.insert(TargetCapabilityRequirementV1::SubgroupSize(
                u16::try_from(wave.width.lanes()).unwrap_or(u16::MAX),
            ));
            let collective = match wave.kind {
                WaveOperationKind::LaneId => None,
                WaveOperationKind::Ballot { .. } => Some((
                    TargetCollectiveOperationV1::Ballot,
                    TargetScalarTypeV1::new(TargetScalarKindV1::Boolean, 1),
                    wave.active_lanes,
                    None,
                )),
                WaveOperationKind::Any { .. } => Some((
                    TargetCollectiveOperationV1::Any,
                    TargetScalarTypeV1::new(TargetScalarKindV1::Boolean, 1),
                    wave.active_lanes,
                    None,
                )),
                WaveOperationKind::All { .. } => Some((
                    TargetCollectiveOperationV1::All,
                    TargetScalarTypeV1::new(TargetScalarKindV1::Boolean, 1),
                    wave.active_lanes,
                    None,
                )),
                WaveOperationKind::ShuffleIndex {
                    value, tile_width, ..
                } => Some((
                    TargetCollectiveOperationV1::Broadcast,
                    value_types
                        .get(&value)
                        .and_then(Type::as_scalar)
                        .map(target_scalar)
                        .transpose()?
                        .ok_or(ProductionTargetCapabilityErrorV1::OmittedAxis {
                            capability: TargetCapability::Subgroups,
                            axis: OmittedCapabilityAxisV1::ScalarKind,
                        })?,
                    tile_width,
                    None,
                )),
                WaveOperationKind::ReduceF32 {
                    tile_width, kind, ..
                } => Some((
                    match kind {
                        WaveF32ReductionKindV1::Sum => TargetCollectiveOperationV1::ReduceAdd,
                        WaveF32ReductionKindV1::Maximum => TargetCollectiveOperationV1::ReduceMax,
                    },
                    TargetScalarTypeV1::new(TargetScalarKindV1::Float, 32),
                    tile_width,
                    Some(TargetNumericalModeV1::IeeeStrict),
                )),
                WaveOperationKind::BroadcastF32 { tile_width, .. } => Some((
                    TargetCollectiveOperationV1::Broadcast,
                    TargetScalarTypeV1::new(TargetScalarKindV1::Float, 32),
                    tile_width,
                    Some(TargetNumericalModeV1::IeeeStrict),
                )),
            };
            if let Some((operation, value_type, participants, numerical_mode)) = collective {
                requirements.insert(TargetCapabilityRequirementV1::Collective(
                    TargetCollectiveRequirementV1::new(
                        TargetExecutionScopeV1::Subgroup,
                        operation,
                        value_type,
                        participants,
                        TargetCollectiveParticipationV1::Full,
                        numerical_mode,
                    ),
                ));
            }
        }
        OperationKind::Matrix(matrix) => {
            if let Some(requirement) = matrix_requirement(&matrix.kind, matrix.tensor_layout) {
                requirements.insert(TargetCapabilityRequirementV1::Matrix(requirement));
            }
        }
        OperationKind::ExecutionCapability(contract) => {
            requirements.extend(target_requirements_for_execution_operation_v1(
                &contract.operation,
            )?);
        }
        _ => {}
    }
    Ok(())
}

pub(crate) fn target_requirements_for_execution_operation_v1(
    operation: &ExecutionCapabilityOperationV1,
) -> Result<BTreeSet<TargetCapabilityRequirementV1>, ProductionTargetCapabilityErrorV1> {
    use ExecutionCapabilityOperationV1 as Op;

    let mut requirements = BTreeSet::new();
    let static_workgroup_memory = |requirements: &mut BTreeSet<_>, bytes: u64| {
        requirements.insert(TargetCapabilityRequirementV1::Resource(
            TargetResourceRequirementV1::StaticSharedMemoryBytesAtMost(bytes),
        ));
    };

    match operation {
        Op::WorkgroupDerive { .. } | Op::WorkgroupMemoryIndex { .. } => {}
        Op::SubgroupDerive { width, .. } | Op::MatrixAccess { width, .. } => {
            requirements.insert(TargetCapabilityRequirementV1::SubgroupSize(
                u16::try_from(*width).unwrap_or(u16::MAX),
            ));
        }
        Op::LdsAllocate {
            layout, elements, ..
        }
        | Op::LdsInitializeByInvocation {
            layout, elements, ..
        }
        | Op::LdsReadPublished {
            layout, elements, ..
        } => {
            insert_execution_address_requirement(
                &mut requirements,
                ExecutionMemoryAddressSpaceV1::Workgroup,
                TargetMemoryAccessV1::ReadWrite,
            );
            static_workgroup_memory(
                &mut requirements,
                exact_execution_footprint(operation, *layout, *elements)?,
            );
        }
        Op::LdsPublish {
            layout, elements, ..
        } => {
            insert_execution_address_requirement(
                &mut requirements,
                ExecutionMemoryAddressSpaceV1::Workgroup,
                TargetMemoryAccessV1::ReadWrite,
            );
            static_workgroup_memory(
                &mut requirements,
                exact_execution_footprint(operation, *layout, *elements)?,
            );
            insert_execution_barrier_requirements(
                &mut requirements,
                ExecutionMemorySemanticsV1 {
                    scope: ExecutionMemoryScopeV1::Workgroup,
                    ordering: ExecutionMemoryOrderingV1::AcquireRelease,
                    spaces: fe2o3_kernel_ir::ExecutionMemorySpacesV1::Workgroup,
                },
            )?;
        }
        Op::WorkgroupBarrier { semantics, .. } => {
            insert_execution_barrier_requirements(&mut requirements, *semantics)?;
        }
        Op::SubgroupBarrier {
            semantics, width, ..
        } => {
            requirements.insert(TargetCapabilityRequirementV1::SubgroupSize(
                u16::try_from(*width).unwrap_or(u16::MAX),
            ));
            insert_execution_barrier_requirements(&mut requirements, *semantics)?;
        }
        Op::WorkgroupFence { semantics, .. } => {
            for space in semantics.spaces.address_spaces() {
                requirements.insert(TargetCapabilityRequirementV1::Fence(
                    TargetFenceRequirementV1::new(
                        target_memory_scope(semantics.scope.synchronization_scope()),
                        target_address_space(space),
                        target_ordering(semantics.ordering.memory_ordering()),
                    ),
                ));
            }
        }
        Op::SubgroupFence {
            semantics, width, ..
        } => {
            requirements.insert(TargetCapabilityRequirementV1::SubgroupSize(
                u16::try_from(*width).unwrap_or(u16::MAX),
            ));
            for space in semantics.spaces.address_spaces() {
                requirements.insert(TargetCapabilityRequirementV1::Fence(
                    TargetFenceRequirementV1::new(
                        target_memory_scope(semantics.scope.synchronization_scope()),
                        target_address_space(space),
                        target_ordering(semantics.ordering.memory_ordering()),
                    ),
                ));
            }
        }
        Op::Atomic {
            kind,
            value_type,
            address_space,
            scope,
            success,
            failure,
            ..
        } => {
            insert_execution_address_requirement(
                &mut requirements,
                *address_space,
                TargetMemoryAccessV1::ReadWrite,
            );
            if let Some(operation) = kind.atomic_kind().and_then(target_atomic_operation) {
                let value_type = target_scalar(*value_type)?;
                let success =
                    success.map_or(MemoryOrdering::Relaxed, |order| order.memory_ordering());
                let requirement = if operation == TargetAtomicOperationV1::CompareExchange {
                    TargetAtomicRequirementV1::compare_exchange(
                        value_type,
                        target_ordering(success),
                        target_ordering(
                            failure
                                .expect("verified V13 compare-exchange failure order")
                                .memory_ordering(),
                        ),
                        target_memory_scope(scope.synchronization_scope()),
                        target_address_space(address_space.address_space()),
                    )
                } else {
                    TargetAtomicRequirementV1::new(
                        value_type,
                        operation,
                        target_ordering(success),
                        target_memory_scope(scope.synchronization_scope()),
                        target_address_space(address_space.address_space()),
                    )
                };
                requirements.insert(TargetCapabilityRequirementV1::Atomic(requirement));
            }
        }
        Op::WorkgroupCollective {
            kind,
            value_type,
            layout,
            elements,
            ..
        } => {
            insert_execution_address_requirement(
                &mut requirements,
                ExecutionMemoryAddressSpaceV1::Workgroup,
                TargetMemoryAccessV1::ReadWrite,
            );
            static_workgroup_memory(
                &mut requirements,
                exact_execution_footprint(operation, *layout, *elements)?,
            );
            insert_execution_collective_requirement(
                &mut requirements,
                TargetExecutionScopeV1::Workgroup,
                *kind,
                *value_type,
                u32::try_from(*elements).unwrap_or(u32::MAX),
            )?;
        }
        Op::SubgroupCollective {
            kind,
            value_type,
            width,
            ..
        } => {
            requirements.insert(TargetCapabilityRequirementV1::SubgroupSize(
                u16::try_from(*width).unwrap_or(u16::MAX),
            ));
            insert_execution_collective_requirement(
                &mut requirements,
                TargetExecutionScopeV1::Subgroup,
                *kind,
                *value_type,
                *width,
            )?;
        }
        Op::AsyncCopy {
            layout, elements, ..
        } => {
            insert_execution_address_requirement(
                &mut requirements,
                ExecutionMemoryAddressSpaceV1::Global,
                TargetMemoryAccessV1::Read,
            );
            insert_execution_address_requirement(
                &mut requirements,
                ExecutionMemoryAddressSpaceV1::Workgroup,
                TargetMemoryAccessV1::Write,
            );
            let bytes = u32::try_from(exact_execution_footprint(operation, *layout, *elements)?)
                .map_err(
                    |_| ProductionTargetCapabilityErrorV1::UnknownStaticResourceFootprint {
                        operation: execution_operation_name(operation),
                    },
                )?;
            requirements.insert(TargetCapabilityRequirementV1::AsyncCopy(
                TargetAsyncCopyRequirementV1::new(
                    TargetAddressSpaceV1::Global,
                    TargetAddressSpaceV1::Workgroup,
                    bytes,
                    layout.byte_alignment,
                ),
            ));
        }
        Op::AsyncWait { .. } => {
            requirements.insert(TargetCapabilityRequirementV1::AsyncWait(
                TargetAsyncWaitRequirementV1::new(
                    TargetExecutionScopeV1::Workgroup,
                    TargetMemoryScopeV1::Workgroup,
                    TargetMemoryOrderingV1::AcquireRelease,
                    1,
                ),
            ));
        }
        Op::RawMemoryBind { space, access, .. } => {
            insert_execution_address_requirement(
                &mut requirements,
                *space,
                target_access(access.access_mode()),
            );
        }
        Op::PrivateMemoryAllocate {
            layout, elements, ..
        } => {
            insert_execution_address_requirement(
                &mut requirements,
                ExecutionMemoryAddressSpaceV1::Private,
                TargetMemoryAccessV1::ReadWrite,
            );
            requirements.insert(TargetCapabilityRequirementV1::Resource(
                TargetResourceRequirementV1::PrivateMemoryBytesPerInvocationAtMost(
                    exact_execution_footprint(operation, *layout, *elements)?,
                ),
            ));
        }
        Op::WorkgroupMemoryAllocate {
            layout, elements, ..
        } => {
            insert_execution_address_requirement(
                &mut requirements,
                ExecutionMemoryAddressSpaceV1::Workgroup,
                TargetMemoryAccessV1::ReadWrite,
            );
            static_workgroup_memory(
                &mut requirements,
                exact_execution_footprint(operation, *layout, *elements)?,
            );
        }
        Op::WorkgroupMemoryPublish { .. } => {
            insert_execution_address_requirement(
                &mut requirements,
                ExecutionMemoryAddressSpaceV1::Workgroup,
                TargetMemoryAccessV1::ReadWrite,
            );
            insert_execution_barrier_requirements(
                &mut requirements,
                ExecutionMemorySemanticsV1 {
                    scope: ExecutionMemoryScopeV1::Workgroup,
                    ordering: ExecutionMemoryOrderingV1::AcquireRelease,
                    spaces: fe2o3_kernel_ir::ExecutionMemorySpacesV1::Workgroup,
                },
            )?;
        }
        Op::MemoryLoad { space, .. } => insert_execution_address_requirement(
            &mut requirements,
            *space,
            TargetMemoryAccessV1::Read,
        ),
        Op::MemoryStore { space, .. } => insert_execution_address_requirement(
            &mut requirements,
            *space,
            TargetMemoryAccessV1::Write,
        ),
    }
    Ok(requirements)
}

fn exact_execution_footprint(
    operation: &ExecutionCapabilityOperationV1,
    layout: fe2o3_kernel_ir::ExecutionElementLayoutV1,
    elements: u64,
) -> Result<u64, ProductionTargetCapabilityErrorV1> {
    layout.checked_footprint(elements).ok_or(
        ProductionTargetCapabilityErrorV1::UnknownStaticResourceFootprint {
            operation: execution_operation_name(operation),
        },
    )
}

fn execution_operation_name(operation: &ExecutionCapabilityOperationV1) -> &'static str {
    use ExecutionCapabilityOperationV1 as Op;
    match operation {
        Op::WorkgroupDerive { .. } => "workgroup-derive",
        Op::SubgroupDerive { .. } => "subgroup-derive",
        Op::LdsAllocate { .. } => "lds-allocate",
        Op::LdsInitializeByInvocation { .. } => "lds-initialize-by-invocation",
        Op::LdsPublish { .. } => "lds-publish",
        Op::LdsReadPublished { .. } => "lds-read-published",
        Op::WorkgroupBarrier { .. } => "workgroup-barrier",
        Op::SubgroupBarrier { .. } => "subgroup-barrier",
        Op::WorkgroupFence { .. } => "workgroup-fence",
        Op::SubgroupFence { .. } => "subgroup-fence",
        Op::Atomic { .. } => "atomic",
        Op::WorkgroupCollective { .. } => "workgroup-collective",
        Op::SubgroupCollective { .. } => "subgroup-collective",
        Op::MatrixAccess { .. } => "matrix-access",
        Op::AsyncCopy { .. } => "async-copy",
        Op::AsyncWait { .. } => "async-wait",
        Op::RawMemoryBind { .. } => "raw-memory-bind",
        Op::PrivateMemoryAllocate { .. } => "private-memory-allocate",
        Op::WorkgroupMemoryIndex { .. } => "workgroup-memory-index",
        Op::WorkgroupMemoryAllocate { .. } => "workgroup-memory-allocate",
        Op::WorkgroupMemoryPublish { .. } => "workgroup-memory-publish",
        Op::MemoryLoad { .. } => "memory-load",
        Op::MemoryStore { .. } => "memory-store",
    }
}

fn insert_execution_address_requirement(
    requirements: &mut BTreeSet<TargetCapabilityRequirementV1>,
    space: ExecutionMemoryAddressSpaceV1,
    access: TargetMemoryAccessV1,
) {
    requirements.insert(TargetCapabilityRequirementV1::AddressSpace(
        target_address_space(space.address_space()),
        access,
    ));
}

fn insert_execution_barrier_requirements(
    requirements: &mut BTreeSet<TargetCapabilityRequirementV1>,
    semantics: ExecutionMemorySemanticsV1,
) -> Result<(), ProductionTargetCapabilityErrorV1> {
    let execution_scope = target_execution_scope(semantics.scope.synchronization_scope()).ok_or(
        ProductionTargetCapabilityErrorV1::OmittedAxis {
            capability: TargetCapability::WorkgroupBarrier,
            axis: OmittedCapabilityAxisV1::BarrierExecutionScope,
        },
    )?;
    for space in semantics.spaces.address_spaces() {
        requirements.insert(TargetCapabilityRequirementV1::Barrier(
            TargetBarrierRequirementV1::new(
                execution_scope,
                target_memory_scope(semantics.scope.synchronization_scope()),
                target_address_space(space),
                target_ordering(semantics.ordering.memory_ordering()),
                TargetBarrierParticipationV1::Uniform,
            ),
        ));
    }
    Ok(())
}

fn insert_execution_collective_requirement(
    requirements: &mut BTreeSet<TargetCapabilityRequirementV1>,
    execution_scope: TargetExecutionScopeV1,
    kind: ExecutionCollectiveKindV1,
    value_type: ScalarType,
    participants: u32,
) -> Result<(), ProductionTargetCapabilityErrorV1> {
    let operation = match kind {
        ExecutionCollectiveKindV1::ReduceSum => TargetCollectiveOperationV1::ReduceAdd,
        ExecutionCollectiveKindV1::InclusiveScanSum => {
            TargetCollectiveOperationV1::InclusiveScanAdd
        }
        ExecutionCollectiveKindV1::ExclusiveScanSum => {
            TargetCollectiveOperationV1::ExclusiveScanAdd
        }
    };
    let value_type = target_scalar(value_type)?;
    requirements.insert(TargetCapabilityRequirementV1::Collective(
        TargetCollectiveRequirementV1::new(
            execution_scope,
            operation,
            value_type,
            participants,
            TargetCollectiveParticipationV1::Full,
            (value_type.kind() == TargetScalarKindV1::Float)
                .then_some(TargetNumericalModeV1::IeeeStrict),
        ),
    ));
    Ok(())
}

fn type_storage_bytes(ty: &Type) -> Option<u64> {
    match ty {
        Type::Scalar(ScalarType::Bool | ScalarType::I8 | ScalarType::U8) => Some(1),
        Type::Scalar(ScalarType::I16 | ScalarType::U16 | ScalarType::F16 | ScalarType::Bf16) => {
            Some(2)
        }
        Type::Scalar(ScalarType::I32 | ScalarType::U32 | ScalarType::F32) => Some(4),
        Type::Scalar(ScalarType::I64 | ScalarType::U64 | ScalarType::F64 | ScalarType::Index)
        | Type::Pointer(_) => Some(8),
        Type::Scalar(ScalarType::I128 | ScalarType::U128) | Type::Slice(_) => Some(16),
        Type::Unit
        | Type::KernelContext(_)
        | Type::GlobalCapability(_)
        | Type::ExecutionCapability(_) => None,
    }
}

fn matrix_element_type(element: MatrixElement) -> TargetScalarTypeV1 {
    match element {
        MatrixElement::Bf16 => TargetScalarTypeV1::bfloat16(),
        MatrixElement::F32 => TargetScalarTypeV1::new(TargetScalarKindV1::Float, 32),
        MatrixElement::Fp4E2M1 => TargetScalarTypeV1::with_encoding(
            TargetScalarKindV1::Float,
            4,
            TargetScalarEncodingV1::Float4E2M1Ocp,
        ),
        MatrixElement::Fp8E4M3 => TargetScalarTypeV1::with_encoding(
            TargetScalarKindV1::Float,
            8,
            TargetScalarEncodingV1::Float8E4M3Ocp,
        ),
    }
}

fn matrix_requirement(
    kind: &MatrixOperationKind,
    layout: Option<fe2o3_kernel_ir::TensorLayoutContractV1>,
) -> Option<TargetMatrixRequirementV1> {
    let layout = layout?;
    let layouts = TargetMatrixLayoutsV1::new(
        TargetMatrixLayoutV1::CooperativeFragment,
        TargetMatrixLayoutV1::CooperativeFragment,
        TargetMatrixLayoutV1::CooperativeFragment,
    );
    let (operation, profile, numerical_mode) = match kind {
        MatrixOperationKind::MultiplyAccumulate { profile, .. } => (
            TargetMatrixOperationV1::MatrixMultiplyAccumulate,
            *profile,
            TargetNumericalModeV1::AllowContraction,
        ),
        MatrixOperationKind::ScaledMultiplyAccumulate { profile, .. } => (
            TargetMatrixOperationV1::ScaledMatrixMultiplyAccumulate,
            *profile,
            TargetNumericalModeV1::AllowApproximation,
        ),
        MatrixOperationKind::LdsLoad { .. } | MatrixOperationKind::LdsStore { .. } => return None,
    };
    Some(TargetMatrixRequirementV1::with_complete_contract(
        operation,
        TargetMatrixShapeV1::new(profile.m, profile.n, profile.k),
        matrix_element_type(layout.a.element),
        matrix_element_type(layout.b.element),
        matrix_element_type(profile.accumulator),
        layouts,
        numerical_mode,
        u16::try_from(profile.wave_width.lanes()).unwrap_or(u16::MAX),
        layout.subgroup_width,
    ))
}

fn requirements_for_capability(
    capability: &TargetCapability,
) -> Result<Vec<TargetCapabilityRequirementV1>, ProductionTargetCapabilityErrorV1> {
    let one = |requirement| Ok(vec![requirement]);
    match capability {
        TargetCapability::Float16 => one(TargetCapabilityRequirementV1::ScalarType(target_scalar(
            ScalarType::F16,
        )?)),
        TargetCapability::BFloat16 => one(TargetCapabilityRequirementV1::ScalarType(
            TargetScalarTypeV1::bfloat16(),
        )),
        TargetCapability::Float64 => one(TargetCapabilityRequirementV1::ScalarType(target_scalar(
            ScalarType::F64,
        )?)),
        TargetCapability::Int64 => omitted(capability, OmittedCapabilityAxisV1::ScalarKind),
        TargetCapability::Subgroups => omitted(capability, OmittedCapabilityAxisV1::SubgroupWidth),
        TargetCapability::SubgroupSize(size) => {
            let size = u16::try_from(*size).map_err(|_| {
                ProductionTargetCapabilityErrorV1::OmittedAxis {
                    capability: capability.clone(),
                    axis: OmittedCapabilityAxisV1::SubgroupWidth,
                }
            })?;
            one(TargetCapabilityRequirementV1::SubgroupSize(size))
        }
        TargetCapability::WaveWidth(width) => {
            one(TargetCapabilityRequirementV1::SubgroupSize(match width {
                WaveWidth::Wave32 => 32,
                WaveWidth::Wave64 => 64,
            }))
        }
        TargetCapability::WorkgroupMemory => one(TargetCapabilityRequirementV1::AddressSpace(
            TargetAddressSpaceV1::Workgroup,
            TargetMemoryAccessV1::ReadWrite,
        )),
        TargetCapability::WorkgroupBarrier => {
            omitted(capability, OmittedCapabilityAxisV1::BarrierExecutionScope)
        }
        TargetCapability::Atomic { .. } => {
            omitted(capability, OmittedCapabilityAxisV1::AtomicOperation)
        }
        TargetCapability::DynamicWorkgroupMemory => omitted(
            capability,
            OmittedCapabilityAxisV1::DynamicSharedMemoryBytes,
        ),
        TargetCapability::Extension { namespace, name }
            if namespace == AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE =>
        {
            Err(ProductionTargetCapabilityErrorV1::NeutralSelectedAmdProfile { name: name.clone() })
        }
        TargetCapability::Extension { namespace, name } => {
            Err(ProductionTargetCapabilityErrorV1::UnsupportedExtension {
                namespace: namespace.clone(),
                name: name.clone(),
            })
        }
        TargetCapability::Execution(requirement) => execution_requirements(capability, requirement),
    }
}

fn execution_requirements(
    capability: &TargetCapability,
    requirement: &ExecutionCapabilityRequirementV1,
) -> Result<Vec<TargetCapabilityRequirementV1>, ProductionTargetCapabilityErrorV1> {
    let one = |requirement| Ok(vec![requirement]);
    match requirement {
        ExecutionCapabilityRequirementV1::AddressSpace {
            address_space,
            access,
        } => one(TargetCapabilityRequirementV1::AddressSpace(
            target_address_space(*address_space),
            target_access(*access),
        )),
        ExecutionCapabilityRequirementV1::Atomic {
            value_type,
            operation,
            ordering,
            failure_ordering,
            scope,
            address_space,
        } => {
            let operation = target_atomic_operation(*operation).ok_or_else(|| {
                ProductionTargetCapabilityErrorV1::OmittedAxis {
                    capability: capability.clone(),
                    axis: OmittedCapabilityAxisV1::AtomicOperation,
                }
            })?;
            let value_type = target_scalar(*value_type)?;
            let requirement = if operation == TargetAtomicOperationV1::CompareExchange {
                TargetAtomicRequirementV1::compare_exchange(
                    value_type,
                    target_ordering(*ordering),
                    target_ordering(failure_ordering.ok_or_else(|| {
                        ProductionTargetCapabilityErrorV1::OmittedAxis {
                            capability: capability.clone(),
                            axis: OmittedCapabilityAxisV1::AtomicFailureOrdering,
                        }
                    })?),
                    target_memory_scope(*scope),
                    target_address_space(*address_space),
                )
            } else {
                if failure_ordering.is_some() {
                    return omitted(capability, OmittedCapabilityAxisV1::AtomicFailureOrdering);
                }
                TargetAtomicRequirementV1::new(
                    value_type,
                    operation,
                    target_ordering(*ordering),
                    target_memory_scope(*scope),
                    target_address_space(*address_space),
                )
            };
            one(TargetCapabilityRequirementV1::Atomic(requirement))
        }
        ExecutionCapabilityRequirementV1::Barrier {
            execution_scope,
            memory_scope,
            ordering,
            address_spaces,
        } => {
            let _complete_v12_axes = (execution_scope, memory_scope, ordering, address_spaces);
            omitted(capability, OmittedCapabilityAxisV1::BarrierParticipation)
        }
        ExecutionCapabilityRequirementV1::Collective {
            execution_scope,
            operation,
            value_type,
            participants,
        } => {
            let operation = target_collective_operation(*operation).ok_or_else(|| {
                ProductionTargetCapabilityErrorV1::OmittedAxis {
                    capability: capability.clone(),
                    axis: OmittedCapabilityAxisV1::CollectiveOperation,
                }
            })?;
            let _complete_v12_axes = (
                target_execution_scope(*execution_scope),
                operation,
                target_scalar(*value_type)?,
                participants,
            );
            omitted(capability, OmittedCapabilityAxisV1::CollectiveParticipation)
        }
        ExecutionCapabilityRequirementV1::Matrix { .. } => {
            omitted(capability, OmittedCapabilityAxisV1::MatrixLayouts)
        }
        ExecutionCapabilityRequirementV1::AsyncCopy { completion, .. } => match completion {
            AsyncCopyCompletionV1::ExplicitWaitGroups { .. } => omitted(
                capability,
                OmittedCapabilityAxisV1::AsyncCompletionMemoryScope,
            ),
            AsyncCopyCompletionV1::WorkgroupBarrier => {
                omitted(capability, OmittedCapabilityAxisV1::AsyncCompletionOrdering)
            }
        },
        ExecutionCapabilityRequirementV1::Numerical { value_type, mode } => one(
            TargetCapabilityRequirementV1::Numerical(TargetNumericalRequirementV1::new(
                target_scalar(*value_type)?,
                target_numerical_mode(*mode),
            )),
        ),
        ExecutionCapabilityRequirementV1::Resource(resource) => one(
            TargetCapabilityRequirementV1::Resource(target_resource(*resource)),
        ),
    }
}

fn omitted<T>(
    capability: &TargetCapability,
    axis: OmittedCapabilityAxisV1,
) -> Result<T, ProductionTargetCapabilityErrorV1> {
    Err(ProductionTargetCapabilityErrorV1::OmittedAxis {
        capability: capability.clone(),
        axis,
    })
}

fn target_scalar(
    scalar: ScalarType,
) -> Result<TargetScalarTypeV1, ProductionTargetCapabilityErrorV1> {
    let kind = match scalar {
        ScalarType::Bool => TargetScalarKindV1::Boolean,
        scalar if scalar.is_signed_integer() => TargetScalarKindV1::SignedInteger,
        scalar if scalar.is_integer() => TargetScalarKindV1::UnsignedInteger,
        scalar if scalar.is_float() => TargetScalarKindV1::Float,
        _ => {
            return Err(ProductionTargetCapabilityErrorV1::OmittedAxis {
                capability: TargetCapability::Execution(
                    ExecutionCapabilityRequirementV1::Numerical {
                        value_type: scalar,
                        mode: NumericalModeV1::StrictIeee,
                    },
                ),
                axis: OmittedCapabilityAxisV1::ScalarEncoding,
            });
        }
    };
    let bit_width = scalar
        .bit_width()
        .ok_or(ProductionTargetCapabilityErrorV1::OmittedAxis {
            capability: TargetCapability::Execution(ExecutionCapabilityRequirementV1::Numerical {
                value_type: scalar,
                mode: NumericalModeV1::StrictIeee,
            }),
            axis: OmittedCapabilityAxisV1::ScalarEncoding,
        })?;
    Ok(if scalar == ScalarType::Bf16 {
        TargetScalarTypeV1::bfloat16()
    } else {
        TargetScalarTypeV1::new(kind, bit_width)
    })
}

const fn target_address_space(address_space: AddressSpace) -> TargetAddressSpaceV1 {
    match address_space {
        AddressSpace::Private => TargetAddressSpaceV1::Private,
        AddressSpace::Workgroup => TargetAddressSpaceV1::Workgroup,
        AddressSpace::Global => TargetAddressSpaceV1::Global,
        AddressSpace::Constant => TargetAddressSpaceV1::Constant,
        AddressSpace::Generic => TargetAddressSpaceV1::Generic,
    }
}

const fn target_access(access: AccessMode) -> TargetMemoryAccessV1 {
    match access {
        AccessMode::ReadOnly => TargetMemoryAccessV1::Read,
        AccessMode::WriteOnly => TargetMemoryAccessV1::Write,
        AccessMode::ReadWrite => TargetMemoryAccessV1::ReadWrite,
    }
}

const fn target_ordering(ordering: MemoryOrdering) -> TargetMemoryOrderingV1 {
    match ordering {
        MemoryOrdering::Relaxed => TargetMemoryOrderingV1::Relaxed,
        MemoryOrdering::Acquire => TargetMemoryOrderingV1::Acquire,
        MemoryOrdering::Release => TargetMemoryOrderingV1::Release,
        MemoryOrdering::AcquireRelease => TargetMemoryOrderingV1::AcquireRelease,
        MemoryOrdering::SequentiallyConsistent => TargetMemoryOrderingV1::SequentiallyConsistent,
    }
}

const fn target_memory_scope(scope: SynchronizationScope) -> TargetMemoryScopeV1 {
    match scope {
        SynchronizationScope::Invocation => TargetMemoryScopeV1::Invocation,
        SynchronizationScope::Subgroup => TargetMemoryScopeV1::Subgroup,
        SynchronizationScope::Workgroup => TargetMemoryScopeV1::Workgroup,
        SynchronizationScope::Device => TargetMemoryScopeV1::Device,
        SynchronizationScope::System => TargetMemoryScopeV1::System,
    }
}

const fn target_execution_scope(scope: SynchronizationScope) -> Option<TargetExecutionScopeV1> {
    match scope {
        SynchronizationScope::Subgroup => Some(TargetExecutionScopeV1::Subgroup),
        SynchronizationScope::Workgroup => Some(TargetExecutionScopeV1::Workgroup),
        SynchronizationScope::Device => Some(TargetExecutionScopeV1::Grid),
        SynchronizationScope::Invocation | SynchronizationScope::System => None,
    }
}

const fn target_atomic_operation(operation: AtomicKind) -> Option<TargetAtomicOperationV1> {
    match operation {
        AtomicKind::Load => Some(TargetAtomicOperationV1::Load),
        AtomicKind::Store => Some(TargetAtomicOperationV1::Store),
        AtomicKind::Exchange => Some(TargetAtomicOperationV1::Exchange),
        AtomicKind::CompareExchange => Some(TargetAtomicOperationV1::CompareExchange),
        AtomicKind::Add => Some(TargetAtomicOperationV1::Add),
        AtomicKind::Subtract => Some(TargetAtomicOperationV1::Sub),
        AtomicKind::Min => Some(TargetAtomicOperationV1::Min),
        AtomicKind::Max => Some(TargetAtomicOperationV1::Max),
        AtomicKind::BitAnd => Some(TargetAtomicOperationV1::And),
        AtomicKind::BitOr => Some(TargetAtomicOperationV1::Or),
        AtomicKind::BitXor => Some(TargetAtomicOperationV1::Xor),
    }
}

const fn target_collective_operation(
    operation: CollectiveCapabilityOperationV1,
) -> Option<TargetCollectiveOperationV1> {
    match operation {
        CollectiveCapabilityOperationV1::Broadcast => Some(TargetCollectiveOperationV1::Broadcast),
        CollectiveCapabilityOperationV1::ReduceAdd => Some(TargetCollectiveOperationV1::ReduceAdd),
        CollectiveCapabilityOperationV1::ReduceMin => Some(TargetCollectiveOperationV1::ReduceMin),
        CollectiveCapabilityOperationV1::ReduceMax => Some(TargetCollectiveOperationV1::ReduceMax),
        CollectiveCapabilityOperationV1::InclusiveScanAdd => {
            Some(TargetCollectiveOperationV1::InclusiveScanAdd)
        }
        CollectiveCapabilityOperationV1::ExclusiveScanAdd => {
            Some(TargetCollectiveOperationV1::ExclusiveScanAdd)
        }
        CollectiveCapabilityOperationV1::Any => Some(TargetCollectiveOperationV1::Any),
        CollectiveCapabilityOperationV1::All => Some(TargetCollectiveOperationV1::All),
    }
}

const fn target_numerical_mode(mode: NumericalModeV1) -> TargetNumericalModeV1 {
    match mode {
        NumericalModeV1::StrictIeee => TargetNumericalModeV1::IeeeStrict,
        NumericalModeV1::AllowContraction => TargetNumericalModeV1::AllowContraction,
        NumericalModeV1::AllowApproximation => TargetNumericalModeV1::AllowApproximation,
    }
}

const fn target_resource(resource: ResourceCapabilityRequirementV1) -> TargetResourceRequirementV1 {
    match resource {
        ResourceCapabilityRequirementV1::WorkgroupInvocationsAtMost(value) => {
            TargetResourceRequirementV1::WorkgroupInvocationsAtMost(value)
        }
        ResourceCapabilityRequirementV1::StaticWorkgroupMemoryBytesAtMost(value) => {
            TargetResourceRequirementV1::StaticSharedMemoryBytesAtMost(value)
        }
        ResourceCapabilityRequirementV1::DynamicWorkgroupMemoryBytesAtMost(value) => {
            TargetResourceRequirementV1::DynamicSharedMemoryBytesAtMost(value)
        }
        ResourceCapabilityRequirementV1::PrivateMemoryBytesPerInvocationAtMost(value) => {
            TargetResourceRequirementV1::PrivateMemoryBytesPerInvocationAtMost(value)
        }
    }
}

fn launch_evidence_matches(
    requirement: TargetCapabilityRequirementV1,
    evidence: ProductionTargetLaunchEvidenceValueV1,
) -> bool {
    match (requirement, evidence) {
        (
            TargetCapabilityRequirementV1::Resource(
                TargetResourceRequirementV1::DynamicSharedMemoryBytesAtMost(required),
            ),
            ProductionTargetLaunchEvidenceValueV1::DynamicSharedMemoryBytes(observed),
        ) => required == observed,
        (
            TargetCapabilityRequirementV1::Atomic(atomic),
            ProductionTargetLaunchEvidenceValueV1::SystemAtomicMemoryEligibility(true),
        ) => {
            atomic.scope() == TargetMemoryScopeV1::System
                && atomic.address_space() == TargetAddressSpaceV1::Global
        }
        _ => false,
    }
}

fn launch_evidence_identity(
    subject: ProductionCanonicalGraphSubjectV1,
    values: &[ProductionTargetLaunchEvidenceValueV1],
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update((LAUNCH_EVIDENCE_IDENTITY_DOMAIN_V1.len() as u32).to_le_bytes());
    digest.update(LAUNCH_EVIDENCE_IDENTITY_DOMAIN_V1);
    digest.update([subject.version().tag()]);
    digest.update(subject.digest());
    digest.update(subject.canonical_length().to_le_bytes());
    digest.update(subject.epoch().to_le_bytes());
    digest.update((values.len() as u64).to_le_bytes());
    for value in values {
        match value {
            ProductionTargetLaunchEvidenceValueV1::DynamicSharedMemoryBytes(bytes) => {
                digest.update([1]);
                digest.update(bytes.to_le_bytes());
            }
            ProductionTargetLaunchEvidenceValueV1::WorkgroupDimensions { x, y, z } => {
                digest.update([2]);
                digest.update(x.to_le_bytes());
                digest.update(y.to_le_bytes());
                digest.update(z.to_le_bytes());
            }
            ProductionTargetLaunchEvidenceValueV1::CooperativeGridAdmission => {
                digest.update([3]);
            }
            ProductionTargetLaunchEvidenceValueV1::SystemAtomicMemoryEligibility(eligible) => {
                digest.update([4]);
                digest.update([u8::from(*eligible)]);
            }
        }
    }
    digest.finalize().into()
}

fn encode_capability_closure(
    subject: ProductionCanonicalGraphSubjectV1,
    target_model: TargetCapabilityModelIdentityV1,
    launch_evidence_identity: [u8; 32],
    records: &[ProductionTargetLegalizationRecordV1],
    artifact_only_requirements: &[ProductionArtifactOnlyRequirementV1],
) -> Result<Vec<u8>, ProductionTargetCapabilityErrorV1> {
    let mut output = Vec::with_capacity(128 + records.len() * 64);
    output.extend_from_slice(CAPABILITY_CLOSURE_MAGIC_V1);
    put_u16(&mut output, CAPABILITY_CLOSURE_WIRE_VERSION_V1);
    put_u8(&mut output, subject.version().tag());
    output.extend_from_slice(&subject.digest());
    put_u64(&mut output, subject.canonical_length());
    put_u64(&mut output, subject.epoch());
    for word in target_model.profile_fingerprint().words() {
        put_u64(&mut output, word);
    }
    for word in target_model.revision_fingerprint().words() {
        put_u64(&mut output, word);
    }
    output.extend_from_slice(&launch_evidence_identity);
    put_u16(
        &mut output,
        u16::try_from(records.len()).map_err(|_| {
            ProductionTargetCapabilityErrorV1::CapabilityClosureTooLarge {
                records: records.len(),
            }
        })?,
    );
    let mut previous = None;
    for record in records {
        let requirement = record.decision.requirement();
        if previous.is_some_and(|previous| previous >= requirement)
            || record.dependencies.len() > MAX_PRODUCTION_TARGET_CAPABILITY_DEPENDENCIES_V1
            || !record.dependencies.windows(2).all(|pair| pair[0] < pair[1])
        {
            return Err(
                ProductionTargetCapabilityErrorV1::CapabilityClosureTooLarge {
                    records: records.len(),
                },
            );
        }
        previous = Some(requirement);
        put_bool(&mut output, record.root);
        put_requirement(&mut output, requirement);
        put_u8(&mut output, outcome_tag(record.decision.outcome()));
        if let TargetCapabilityDecisionOutcomeV1::DynamicLaunchEvidenceRequired(kind) =
            record.decision.outcome()
        {
            put_u8(&mut output, launch_evidence_kind_tag(kind));
        }
        put_u8(&mut output, capability_owner_tag(record.owner));
        put_u8(
            &mut output,
            u8::try_from(record.dependencies.len()).expect("dependency bound checked"),
        );
        for dependency in &record.dependencies {
            put_requirement(&mut output, *dependency);
        }
    }
    put_u8(
        &mut output,
        u8::try_from(artifact_only_requirements.len())
            .expect("fixed artifact requirement roster fits u8"),
    );
    for requirement in artifact_only_requirements {
        put_u8(&mut output, artifact_requirement_tag(*requirement));
    }
    if output.len() > MAX_PRODUCTION_TARGET_CAPABILITY_CLOSURE_BYTES_V1 {
        return Err(
            ProductionTargetCapabilityErrorV1::CapabilityClosureTooLarge {
                records: records.len(),
            },
        );
    }
    Ok(output)
}

fn capability_closure_identity(canonical_bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update((CAPABILITY_CLOSURE_IDENTITY_DOMAIN_V1.len() as u32).to_le_bytes());
    digest.update(CAPABILITY_CLOSURE_IDENTITY_DOMAIN_V1);
    digest.update((canonical_bytes.len() as u64).to_le_bytes());
    digest.update(canonical_bytes);
    digest.finalize().into()
}

fn put_requirement(output: &mut Vec<u8>, requirement: TargetCapabilityRequirementV1) {
    match requirement {
        TargetCapabilityRequirementV1::ScalarType(value) => {
            put_u8(output, 1);
            put_scalar(output, value);
        }
        TargetCapabilityRequirementV1::SubgroupSize(value) => {
            put_u8(output, 2);
            put_u16(output, value);
        }
        TargetCapabilityRequirementV1::AddressSpace(space, access) => {
            put_u8(output, 3);
            put_u8(output, address_space_tag(space));
            put_u8(output, memory_access_tag(access));
        }
        TargetCapabilityRequirementV1::Atomic(value) => {
            put_u8(output, 4);
            put_scalar(output, value.value_type());
            put_u8(output, atomic_operation_tag(value.operation()));
            put_u8(output, memory_ordering_tag(value.ordering()));
            match value.failure_ordering() {
                Some(ordering) => {
                    put_bool(output, true);
                    put_u8(output, memory_ordering_tag(ordering));
                }
                None => put_bool(output, false),
            }
            put_u8(output, memory_scope_tag(value.scope()));
            put_u8(output, address_space_tag(value.address_space()));
        }
        TargetCapabilityRequirementV1::Barrier(value) => {
            put_u8(output, 5);
            put_u8(output, execution_scope_tag(value.execution_scope()));
            put_u8(output, memory_scope_tag(value.memory_scope()));
            put_u8(output, address_space_tag(value.address_space()));
            put_u8(output, memory_ordering_tag(value.ordering()));
            put_u8(output, barrier_participation_tag(value.participation()));
        }
        TargetCapabilityRequirementV1::Fence(value) => {
            put_u8(output, 6);
            put_u8(output, memory_scope_tag(value.memory_scope()));
            put_u8(output, address_space_tag(value.address_space()));
            put_u8(output, memory_ordering_tag(value.ordering()));
        }
        TargetCapabilityRequirementV1::Collective(value) => {
            put_u8(output, 7);
            put_u8(output, execution_scope_tag(value.execution_scope()));
            put_u8(output, collective_operation_tag(value.operation()));
            put_scalar(output, value.value_type());
            put_u32(output, value.participants());
            match value.participation() {
                TargetCollectiveParticipationV1::Full => put_u8(output, 1),
                TargetCollectiveParticipationV1::UniformPrefix(count) => {
                    put_u8(output, 2);
                    put_u32(output, count);
                }
                TargetCollectiveParticipationV1::DynamicMask => put_u8(output, 3),
            }
            put_optional_numerical_mode(output, value.numerical_mode());
        }
        TargetCapabilityRequirementV1::Matrix(value) => {
            put_u8(output, 8);
            put_u8(output, matrix_operation_tag(value.operation()));
            put_u16(output, value.m());
            put_u16(output, value.n());
            put_u16(output, value.k());
            put_scalar(output, value.lhs_type());
            put_scalar(output, value.rhs_type());
            put_scalar(output, value.accumulator_type());
            put_u8(output, matrix_layout_tag(value.lhs_layout()));
            put_u8(output, matrix_layout_tag(value.rhs_layout()));
            put_u8(output, matrix_layout_tag(value.output_layout()));
            put_u8(output, numerical_mode_tag(value.numerical_mode()));
            put_u16(output, value.subgroup_size());
            put_u16(output, value.active_participants());
        }
        TargetCapabilityRequirementV1::AsyncCopy(value) => {
            put_u8(output, 9);
            put_u8(output, address_space_tag(value.source()));
            put_u8(output, address_space_tag(value.destination()));
            put_u32(output, value.bytes());
            put_u16(output, value.alignment());
        }
        TargetCapabilityRequirementV1::AsyncWait(value) => {
            put_u8(output, 10);
            put_u8(output, execution_scope_tag(value.execution_scope()));
            put_u8(output, memory_scope_tag(value.memory_scope()));
            put_u8(output, memory_ordering_tag(value.ordering()));
            put_u16(output, value.max_pending_groups());
        }
        TargetCapabilityRequirementV1::Numerical(value) => {
            put_u8(output, 11);
            put_scalar(output, value.value_type());
            put_u8(output, numerical_mode_tag(value.mode()));
        }
        TargetCapabilityRequirementV1::Resource(value) => {
            put_u8(output, 12);
            match value {
                TargetResourceRequirementV1::WorkgroupInvocationsAtMost(value) => {
                    put_u8(output, 1);
                    put_u32(output, value);
                }
                TargetResourceRequirementV1::StaticSharedMemoryBytesAtMost(value) => {
                    put_u8(output, 2);
                    put_u64(output, value);
                }
                TargetResourceRequirementV1::DynamicSharedMemoryBytesAtMost(value) => {
                    put_u8(output, 3);
                    put_u64(output, value);
                }
                TargetResourceRequirementV1::PrivateMemoryBytesPerInvocationAtMost(value) => {
                    put_u8(output, 4);
                    put_u64(output, value);
                }
                TargetResourceRequirementV1::WorkgroupDimensions { x, y, z } => {
                    put_u8(output, 5);
                    put_u32(output, x);
                    put_u32(output, y);
                    put_u32(output, z);
                }
                TargetResourceRequirementV1::SubgroupsPerWorkgroupAtMost(value) => {
                    put_u8(output, 6);
                    put_u32(output, value);
                }
                TargetResourceRequirementV1::RegistersPerInvocationAtMost(value) => {
                    put_u8(output, 7);
                    put_u32(output, value);
                }
            }
        }
        TargetCapabilityRequirementV1::Abi(value) => {
            put_u8(output, 13);
            match value {
                TargetAbiConstraintV1::PointerWidth(value) => {
                    put_u8(output, 1);
                    put_u16(output, value);
                }
                TargetAbiConstraintV1::Endianness(value) => {
                    put_u8(output, 2);
                    put_u8(output, endianness_tag(value));
                }
                TargetAbiConstraintV1::KernelArgumentAlignmentAtMost(value) => {
                    put_u8(output, 3);
                    put_u16(output, value);
                }
                TargetAbiConstraintV1::KernelArgumentSegmentBytesAtMost(value) => {
                    put_u8(output, 4);
                    put_u32(output, value);
                }
            }
        }
        TargetCapabilityRequirementV1::Object(value) => {
            put_u8(output, 14);
            match value {
                TargetObjectConstraintV1::Format(value) => {
                    put_u8(output, 1);
                    put_u8(output, object_format_tag(value));
                }
                TargetObjectConstraintV1::Relocatable(value) => {
                    put_u8(output, 2);
                    put_bool(output, value);
                }
            }
        }
    }
}

fn put_scalar(output: &mut Vec<u8>, scalar: TargetScalarTypeV1) {
    put_u8(output, scalar_kind_tag(scalar.kind()));
    put_u16(output, scalar.bit_width());
    put_u8(output, scalar_encoding_tag(scalar.encoding()));
}

fn put_optional_numerical_mode(output: &mut Vec<u8>, mode: Option<TargetNumericalModeV1>) {
    match mode {
        Some(mode) => {
            put_bool(output, true);
            put_u8(output, numerical_mode_tag(mode));
        }
        None => put_bool(output, false),
    }
}

fn put_u8(output: &mut Vec<u8>, value: u8) {
    output.push(value);
}

fn put_bool(output: &mut Vec<u8>, value: bool) {
    put_u8(output, u8::from(value));
}

fn put_u16(output: &mut Vec<u8>, value: u16) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn put_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn put_u64(output: &mut Vec<u8>, value: u64) {
    output.extend_from_slice(&value.to_le_bytes());
}

struct DecodedCapabilityClosureV1 {
    subject: ProductionCanonicalGraphSubjectV1,
    target_model: TargetCapabilityModelIdentityV1,
    launch_evidence_identity: [u8; 32],
    decisions: Box<[TargetCapabilityDecisionV1]>,
    owners: Box<[ProductionAmdCapabilityOwnerV1]>,
    records: Box<[ProductionTargetLegalizationRecordV1]>,
    artifact_only_requirements: Box<[ProductionArtifactOnlyRequirementV1]>,
}

fn decode_capability_closure(
    bytes: &[u8],
) -> Result<DecodedCapabilityClosureV1, ProductionTargetCapabilityCanonicalErrorV1> {
    if bytes.len() > MAX_PRODUCTION_TARGET_CAPABILITY_CLOSURE_BYTES_V1 {
        return Err(ProductionTargetCapabilityCanonicalErrorV1::Oversized {
            length: bytes.len(),
        });
    }
    let mut reader = CapabilityClosureReaderV1 { bytes, offset: 0 };
    if reader.array::<8>()? != *CAPABILITY_CLOSURE_MAGIC_V1 {
        return Err(ProductionTargetCapabilityCanonicalErrorV1::InvalidMagic);
    }
    let wire_version = reader.u16()?;
    if wire_version != CAPABILITY_CLOSURE_WIRE_VERSION_V1 {
        return Err(
            ProductionTargetCapabilityCanonicalErrorV1::UnsupportedWireVersion(wire_version),
        );
    }
    let version = match reader.u8()? {
        12 => ProductionCanonicalGraphVersionV1::V12,
        13 => ProductionCanonicalGraphVersionV1::V13,
        tag => return Err(invalid_tag("canonical graph version", tag)),
    };
    let subject = ProductionCanonicalGraphSubjectV1 {
        version,
        digest: reader.array()?,
        canonical_length: reader.u64()?,
        epoch: reader.u64()?,
    };
    let profile_fingerprint = [reader.u64()?, reader.u64()?, reader.u64()?, reader.u64()?];
    let revision_fingerprint = [reader.u64()?, reader.u64()?, reader.u64()?, reader.u64()?];
    let model = [
        ProductionAmdTargetProfileV1::Gfx942,
        ProductionAmdTargetProfileV1::Gfx950,
    ]
    .into_iter()
    .filter_map(|profile| profile.capability_model().ok())
    .find(|model| {
        let identity = fe2o3_target_spec::TargetCapabilityQueryV1::model_identity(model);
        identity.profile_fingerprint().words() == profile_fingerprint
            && identity.revision_fingerprint().words() == revision_fingerprint
    })
    .ok_or(ProductionTargetCapabilityCanonicalErrorV1::InvalidTargetModel)?;
    let target_model = fe2o3_target_spec::TargetCapabilityQueryV1::model_identity(&model);
    let launch_evidence_identity = reader.array()?;
    let record_count = usize::from(reader.u16()?);
    if record_count == 0 || record_count > MAX_TARGET_CAPABILITY_CLOSURE_REQUIREMENTS_V1 {
        return Err(ProductionTargetCapabilityCanonicalErrorV1::NonCanonical);
    }
    let mut decisions = Vec::with_capacity(record_count);
    let mut owners = Vec::with_capacity(record_count);
    let mut records = Vec::with_capacity(record_count);
    let mut previous = None;
    for _ in 0..record_count {
        let root = reader.bool()?;
        let requirement = reader.requirement()?;
        if previous.is_some_and(|previous| previous >= requirement) {
            return Err(ProductionTargetCapabilityCanonicalErrorV1::NonCanonical);
        }
        previous = Some(requirement);
        let encoded_outcome = reader.outcome()?;
        let encoded_owner = reader.owner()?;
        let dependency_count = usize::from(reader.u8()?);
        if dependency_count > MAX_PRODUCTION_TARGET_CAPABILITY_DEPENDENCIES_V1 {
            return Err(ProductionTargetCapabilityCanonicalErrorV1::NonCanonical);
        }
        let mut dependencies = Vec::with_capacity(dependency_count);
        for _ in 0..dependency_count {
            let dependency = reader.requirement()?;
            if dependencies
                .last()
                .is_some_and(|previous| *previous >= dependency)
            {
                return Err(ProductionTargetCapabilityCanonicalErrorV1::NonCanonical);
            }
            dependencies.push(dependency);
        }
        let decision = query_target_capability_v1(&model, requirement)
            .map_err(|_| ProductionTargetCapabilityCanonicalErrorV1::InvalidRequirement)?;
        if decision.outcome() != encoded_outcome {
            return Err(ProductionTargetCapabilityCanonicalErrorV1::NonCanonical);
        }
        let owner = model
            .capability_owner(requirement)
            .ok_or(ProductionTargetCapabilityCanonicalErrorV1::NonCanonical)?;
        if owner != encoded_owner {
            return Err(ProductionTargetCapabilityCanonicalErrorV1::NonCanonical);
        }
        decisions.push(decision);
        owners.push(owner);
        records.push(ProductionTargetLegalizationRecordV1 {
            root,
            decision,
            dependencies: dependencies.into_boxed_slice(),
            owner,
        });
    }
    let artifact_count = usize::from(reader.u8()?);
    let mut artifact_only_requirements = Vec::with_capacity(artifact_count);
    for _ in 0..artifact_count {
        artifact_only_requirements.push(reader.artifact_requirement()?);
    }
    if !reader.is_finished() {
        return Err(ProductionTargetCapabilityCanonicalErrorV1::NonCanonical);
    }
    let expected_artifact_requirements = exact_artifact_only_requirements();
    if artifact_only_requirements.as_slice() != expected_artifact_requirements.as_ref() {
        return Err(ProductionTargetCapabilityCanonicalErrorV1::NonCanonical);
    }

    let requirements = records
        .iter()
        .map(|record| record.decision.requirement())
        .collect::<BTreeSet<_>>();
    let roots = records
        .iter()
        .filter(|record| record.root)
        .map(|record| record.decision.requirement())
        .collect::<BTreeSet<_>>();
    if roots.is_empty() {
        return Err(ProductionTargetCapabilityCanonicalErrorV1::NonCanonical);
    }
    for record in &records {
        if record.dependencies.as_ref()
            != exact_requirement_dependencies(record.decision.requirement())
                .into_iter()
                .collect::<Vec<_>>()
                .as_slice()
            || record
                .dependencies
                .iter()
                .any(|dependency| !requirements.contains(dependency))
        {
            return Err(ProductionTargetCapabilityCanonicalErrorV1::NonCanonical);
        }
    }
    let mut reachable = roots.clone();
    loop {
        let before = reachable.len();
        for record in &records {
            if reachable.contains(&record.decision.requirement()) {
                reachable.extend(record.dependencies.iter().copied());
            }
        }
        if reachable.len() == before {
            break;
        }
    }
    if reachable != requirements {
        return Err(ProductionTargetCapabilityCanonicalErrorV1::NonCanonical);
    }

    let records = records.into_boxed_slice();
    let artifact_only_requirements = artifact_only_requirements.into_boxed_slice();
    let reencoded = encode_capability_closure(
        subject,
        target_model,
        launch_evidence_identity,
        &records,
        &artifact_only_requirements,
    )
    .map_err(|_| ProductionTargetCapabilityCanonicalErrorV1::NonCanonical)?;
    if reencoded != bytes {
        return Err(ProductionTargetCapabilityCanonicalErrorV1::NonCanonical);
    }
    Ok(DecodedCapabilityClosureV1 {
        subject,
        target_model,
        launch_evidence_identity,
        decisions: decisions.into_boxed_slice(),
        owners: owners.into_boxed_slice(),
        records,
        artifact_only_requirements,
    })
}

struct CapabilityClosureReaderV1<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl CapabilityClosureReaderV1<'_> {
    fn array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], ProductionTargetCapabilityCanonicalErrorV1> {
        let end = self
            .offset
            .checked_add(N)
            .ok_or(ProductionTargetCapabilityCanonicalErrorV1::Truncated)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(ProductionTargetCapabilityCanonicalErrorV1::Truncated)?;
        self.offset = end;
        Ok(value
            .try_into()
            .expect("reader selected an exact fixed-size range"))
    }

    fn u8(&mut self) -> Result<u8, ProductionTargetCapabilityCanonicalErrorV1> {
        Ok(self.array::<1>()?[0])
    }

    fn bool(&mut self) -> Result<bool, ProductionTargetCapabilityCanonicalErrorV1> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(ProductionTargetCapabilityCanonicalErrorV1::InvalidBoolean(
                value,
            )),
        }
    }

    fn u16(&mut self) -> Result<u16, ProductionTargetCapabilityCanonicalErrorV1> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, ProductionTargetCapabilityCanonicalErrorV1> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, ProductionTargetCapabilityCanonicalErrorV1> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    fn is_finished(&self) -> bool {
        self.offset == self.bytes.len()
    }

    fn scalar(&mut self) -> Result<TargetScalarTypeV1, ProductionTargetCapabilityCanonicalErrorV1> {
        let kind = decode_scalar_kind(self.u8()?)?;
        let width = self.u16()?;
        let encoding = decode_scalar_encoding(self.u8()?)?;
        let scalar = TargetScalarTypeV1::with_encoding(kind, width, encoding);
        TargetCapabilityRequirementV1::ScalarType(scalar)
            .validate()
            .map_err(|_| ProductionTargetCapabilityCanonicalErrorV1::InvalidRequirement)?;
        Ok(scalar)
    }

    fn optional_numerical_mode(
        &mut self,
    ) -> Result<Option<TargetNumericalModeV1>, ProductionTargetCapabilityCanonicalErrorV1> {
        if self.bool()? {
            Ok(Some(decode_numerical_mode(self.u8()?)?))
        } else {
            Ok(None)
        }
    }

    fn requirement(
        &mut self,
    ) -> Result<TargetCapabilityRequirementV1, ProductionTargetCapabilityCanonicalErrorV1> {
        let requirement = match self.u8()? {
            1 => TargetCapabilityRequirementV1::ScalarType(self.scalar()?),
            2 => TargetCapabilityRequirementV1::SubgroupSize(self.u16()?),
            3 => TargetCapabilityRequirementV1::AddressSpace(
                decode_address_space(self.u8()?)?,
                decode_memory_access(self.u8()?)?,
            ),
            4 => {
                let value_type = self.scalar()?;
                let operation = decode_atomic_operation(self.u8()?)?;
                let ordering = decode_memory_ordering(self.u8()?)?;
                let failure = if self.bool()? {
                    Some(decode_memory_ordering(self.u8()?)?)
                } else {
                    None
                };
                let scope = decode_memory_scope(self.u8()?)?;
                let address_space = decode_address_space(self.u8()?)?;
                let value = match (operation, failure) {
                    (TargetAtomicOperationV1::CompareExchange, Some(failure)) => {
                        TargetAtomicRequirementV1::compare_exchange(
                            value_type,
                            ordering,
                            failure,
                            scope,
                            address_space,
                        )
                    }
                    (TargetAtomicOperationV1::CompareExchange, None) | (_, Some(_)) => {
                        return Err(ProductionTargetCapabilityCanonicalErrorV1::InvalidRequirement);
                    }
                    (_, None) => TargetAtomicRequirementV1::new(
                        value_type,
                        operation,
                        ordering,
                        scope,
                        address_space,
                    ),
                };
                TargetCapabilityRequirementV1::Atomic(value)
            }
            5 => TargetCapabilityRequirementV1::Barrier(TargetBarrierRequirementV1::new(
                decode_execution_scope(self.u8()?)?,
                decode_memory_scope(self.u8()?)?,
                decode_address_space(self.u8()?)?,
                decode_memory_ordering(self.u8()?)?,
                decode_barrier_participation(self.u8()?)?,
            )),
            6 => TargetCapabilityRequirementV1::Fence(TargetFenceRequirementV1::new(
                decode_memory_scope(self.u8()?)?,
                decode_address_space(self.u8()?)?,
                decode_memory_ordering(self.u8()?)?,
            )),
            7 => {
                let execution_scope = decode_execution_scope(self.u8()?)?;
                let operation = decode_collective_operation(self.u8()?)?;
                let value_type = self.scalar()?;
                let participants = self.u32()?;
                let participation = match self.u8()? {
                    1 => TargetCollectiveParticipationV1::Full,
                    2 => TargetCollectiveParticipationV1::UniformPrefix(self.u32()?),
                    3 => TargetCollectiveParticipationV1::DynamicMask,
                    tag => return Err(invalid_tag("collective participation", tag)),
                };
                TargetCapabilityRequirementV1::Collective(TargetCollectiveRequirementV1::new(
                    execution_scope,
                    operation,
                    value_type,
                    participants,
                    participation,
                    self.optional_numerical_mode()?,
                ))
            }
            8 => {
                let operation = decode_matrix_operation(self.u8()?)?;
                let shape = TargetMatrixShapeV1::new(self.u16()?, self.u16()?, self.u16()?);
                let lhs = self.scalar()?;
                let rhs = self.scalar()?;
                let accumulator = self.scalar()?;
                let layouts = TargetMatrixLayoutsV1::new(
                    decode_matrix_layout(self.u8()?)?,
                    decode_matrix_layout(self.u8()?)?,
                    decode_matrix_layout(self.u8()?)?,
                );
                let mode = decode_numerical_mode(self.u8()?)?;
                let subgroup_size = self.u16()?;
                let active_participants = self.u16()?;
                TargetCapabilityRequirementV1::Matrix(
                    TargetMatrixRequirementV1::with_complete_contract(
                        operation,
                        shape,
                        lhs,
                        rhs,
                        accumulator,
                        layouts,
                        mode,
                        subgroup_size,
                        active_participants,
                    ),
                )
            }
            9 => TargetCapabilityRequirementV1::AsyncCopy(TargetAsyncCopyRequirementV1::new(
                decode_address_space(self.u8()?)?,
                decode_address_space(self.u8()?)?,
                self.u32()?,
                self.u16()?,
            )),
            10 => TargetCapabilityRequirementV1::AsyncWait(TargetAsyncWaitRequirementV1::new(
                decode_execution_scope(self.u8()?)?,
                decode_memory_scope(self.u8()?)?,
                decode_memory_ordering(self.u8()?)?,
                self.u16()?,
            )),
            11 => TargetCapabilityRequirementV1::Numerical(TargetNumericalRequirementV1::new(
                self.scalar()?,
                decode_numerical_mode(self.u8()?)?,
            )),
            12 => TargetCapabilityRequirementV1::Resource(match self.u8()? {
                1 => TargetResourceRequirementV1::WorkgroupInvocationsAtMost(self.u32()?),
                2 => TargetResourceRequirementV1::StaticSharedMemoryBytesAtMost(self.u64()?),
                3 => TargetResourceRequirementV1::DynamicSharedMemoryBytesAtMost(self.u64()?),
                4 => {
                    TargetResourceRequirementV1::PrivateMemoryBytesPerInvocationAtMost(self.u64()?)
                }
                5 => TargetResourceRequirementV1::WorkgroupDimensions {
                    x: self.u32()?,
                    y: self.u32()?,
                    z: self.u32()?,
                },
                6 => TargetResourceRequirementV1::SubgroupsPerWorkgroupAtMost(self.u32()?),
                7 => TargetResourceRequirementV1::RegistersPerInvocationAtMost(self.u32()?),
                tag => return Err(invalid_tag("resource requirement", tag)),
            }),
            13 => TargetCapabilityRequirementV1::Abi(match self.u8()? {
                1 => TargetAbiConstraintV1::PointerWidth(self.u16()?),
                2 => TargetAbiConstraintV1::Endianness(decode_endianness(self.u8()?)?),
                3 => TargetAbiConstraintV1::KernelArgumentAlignmentAtMost(self.u16()?),
                4 => TargetAbiConstraintV1::KernelArgumentSegmentBytesAtMost(self.u32()?),
                tag => return Err(invalid_tag("ABI requirement", tag)),
            }),
            14 => TargetCapabilityRequirementV1::Object(match self.u8()? {
                1 => TargetObjectConstraintV1::Format(decode_object_format(self.u8()?)?),
                2 => TargetObjectConstraintV1::Relocatable(self.bool()?),
                tag => return Err(invalid_tag("object requirement", tag)),
            }),
            tag => return Err(invalid_tag("requirement", tag)),
        };
        requirement
            .validate()
            .map_err(|_| ProductionTargetCapabilityCanonicalErrorV1::InvalidRequirement)?;
        Ok(requirement)
    }

    fn outcome(
        &mut self,
    ) -> Result<TargetCapabilityDecisionOutcomeV1, ProductionTargetCapabilityCanonicalErrorV1> {
        match self.u8()? {
            1 => Ok(TargetCapabilityDecisionOutcomeV1::Supported),
            2 => Ok(TargetCapabilityDecisionOutcomeV1::Unsupported),
            3 => Ok(TargetCapabilityDecisionOutcomeV1::Incomplete),
            4 => Ok(TargetCapabilityDecisionOutcomeV1::Unreviewed),
            5 => Ok(
                TargetCapabilityDecisionOutcomeV1::DynamicLaunchEvidenceRequired(
                    decode_launch_evidence_kind(self.u8()?)?,
                ),
            ),
            tag => Err(invalid_tag("decision outcome", tag)),
        }
    }

    fn owner(
        &mut self,
    ) -> Result<ProductionAmdCapabilityOwnerV1, ProductionTargetCapabilityCanonicalErrorV1> {
        decode_capability_owner(self.u8()?)
    }

    fn artifact_requirement(
        &mut self,
    ) -> Result<ProductionArtifactOnlyRequirementV1, ProductionTargetCapabilityCanonicalErrorV1>
    {
        match self.u8()? {
            1 => Ok(ProductionArtifactOnlyRequirementV1::ExactKernelArgumentLayout),
            2 => Ok(ProductionArtifactOnlyRequirementV1::ExactRegistersPerInvocation),
            3 => Ok(ProductionArtifactOnlyRequirementV1::ExactPrivateSegmentBytes),
            4 => Ok(ProductionArtifactOnlyRequirementV1::ExactScratchBytes),
            5 => Ok(ProductionArtifactOnlyRequirementV1::ExactLoadableObject),
            tag => Err(invalid_tag("artifact-only requirement", tag)),
        }
    }
}

const fn capability_owner_tag(owner: ProductionAmdCapabilityOwnerV1) -> u8 {
    match owner {
        ProductionAmdCapabilityOwnerV1::ScalarMemoryLowering => 1,
        ProductionAmdCapabilityOwnerV1::AtomicLowering => 2,
        ProductionAmdCapabilityOwnerV1::SynchronizationLowering => 3,
        ProductionAmdCapabilityOwnerV1::WaveLowering => 4,
        ProductionAmdCapabilityOwnerV1::MatrixLowering => 5,
        ProductionAmdCapabilityOwnerV1::ResourceAdmission => 6,
        ProductionAmdCapabilityOwnerV1::KernelAbi => 7,
        ProductionAmdCapabilityOwnerV1::LoadableObjectEmitter => 8,
        ProductionAmdCapabilityOwnerV1::WorkgroupCollectiveLowering => 9,
        ProductionAmdCapabilityOwnerV1::AsyncCopyLowering => 10,
    }
}

const fn artifact_requirement_tag(value: ProductionArtifactOnlyRequirementV1) -> u8 {
    match value {
        ProductionArtifactOnlyRequirementV1::ExactKernelArgumentLayout => 1,
        ProductionArtifactOnlyRequirementV1::ExactRegistersPerInvocation => 2,
        ProductionArtifactOnlyRequirementV1::ExactPrivateSegmentBytes => 3,
        ProductionArtifactOnlyRequirementV1::ExactScratchBytes => 4,
        ProductionArtifactOnlyRequirementV1::ExactLoadableObject => 5,
    }
}

const fn outcome_tag(value: TargetCapabilityDecisionOutcomeV1) -> u8 {
    match value {
        TargetCapabilityDecisionOutcomeV1::Supported => 1,
        TargetCapabilityDecisionOutcomeV1::Unsupported => 2,
        TargetCapabilityDecisionOutcomeV1::Incomplete => 3,
        TargetCapabilityDecisionOutcomeV1::Unreviewed => 4,
        TargetCapabilityDecisionOutcomeV1::DynamicLaunchEvidenceRequired(_) => 5,
    }
}

const fn launch_evidence_kind_tag(value: TargetLaunchEvidenceKindV1) -> u8 {
    match value {
        TargetLaunchEvidenceKindV1::DynamicSharedMemoryBytes => 1,
        TargetLaunchEvidenceKindV1::WorkgroupDimensions => 2,
        TargetLaunchEvidenceKindV1::CooperativeGridAdmission => 3,
        TargetLaunchEvidenceKindV1::SystemAtomicMemoryEligibility => 4,
    }
}

const fn address_space_tag(value: TargetAddressSpaceV1) -> u8 {
    match value {
        TargetAddressSpaceV1::Global => 1,
        TargetAddressSpaceV1::Workgroup => 2,
        TargetAddressSpaceV1::Private => 3,
        TargetAddressSpaceV1::Constant => 4,
        TargetAddressSpaceV1::Generic => 5,
    }
}

const fn memory_access_tag(value: TargetMemoryAccessV1) -> u8 {
    match value {
        TargetMemoryAccessV1::Read => 1,
        TargetMemoryAccessV1::Write => 2,
        TargetMemoryAccessV1::ReadWrite => 3,
    }
}

const fn scalar_kind_tag(value: TargetScalarKindV1) -> u8 {
    match value {
        TargetScalarKindV1::Boolean => 1,
        TargetScalarKindV1::SignedInteger => 2,
        TargetScalarKindV1::UnsignedInteger => 3,
        TargetScalarKindV1::Float => 4,
    }
}

const fn scalar_encoding_tag(value: TargetScalarEncodingV1) -> u8 {
    match value {
        TargetScalarEncodingV1::Boolean => 1,
        TargetScalarEncodingV1::TwosComplement => 2,
        TargetScalarEncodingV1::IeeeBinary => 3,
        TargetScalarEncodingV1::BFloat => 4,
        TargetScalarEncodingV1::Float4E2M1Ocp => 5,
        TargetScalarEncodingV1::Float8E4M3Fnuz => 6,
        TargetScalarEncodingV1::Float8E5M2Fnuz => 7,
        TargetScalarEncodingV1::Float8E4M3Ocp => 8,
        TargetScalarEncodingV1::Float8E5M2Ocp => 9,
    }
}

const fn atomic_operation_tag(value: TargetAtomicOperationV1) -> u8 {
    match value {
        TargetAtomicOperationV1::Load => 1,
        TargetAtomicOperationV1::Store => 2,
        TargetAtomicOperationV1::Exchange => 3,
        TargetAtomicOperationV1::CompareExchange => 4,
        TargetAtomicOperationV1::Add => 5,
        TargetAtomicOperationV1::Sub => 6,
        TargetAtomicOperationV1::Min => 7,
        TargetAtomicOperationV1::Max => 8,
        TargetAtomicOperationV1::And => 9,
        TargetAtomicOperationV1::Nand => 10,
        TargetAtomicOperationV1::Or => 11,
        TargetAtomicOperationV1::Xor => 12,
    }
}

const fn memory_ordering_tag(value: TargetMemoryOrderingV1) -> u8 {
    match value {
        TargetMemoryOrderingV1::Relaxed => 1,
        TargetMemoryOrderingV1::Acquire => 2,
        TargetMemoryOrderingV1::Release => 3,
        TargetMemoryOrderingV1::AcquireRelease => 4,
        TargetMemoryOrderingV1::SequentiallyConsistent => 5,
    }
}

const fn memory_scope_tag(value: TargetMemoryScopeV1) -> u8 {
    match value {
        TargetMemoryScopeV1::Invocation => 1,
        TargetMemoryScopeV1::Subgroup => 2,
        TargetMemoryScopeV1::Workgroup => 3,
        TargetMemoryScopeV1::Device => 4,
        TargetMemoryScopeV1::System => 5,
    }
}

const fn execution_scope_tag(value: TargetExecutionScopeV1) -> u8 {
    match value {
        TargetExecutionScopeV1::Subgroup => 1,
        TargetExecutionScopeV1::Workgroup => 2,
        TargetExecutionScopeV1::Grid => 3,
    }
}

const fn barrier_participation_tag(value: TargetBarrierParticipationV1) -> u8 {
    match value {
        TargetBarrierParticipationV1::Uniform => 1,
        TargetBarrierParticipationV1::DynamicMask => 2,
    }
}

const fn collective_operation_tag(value: TargetCollectiveOperationV1) -> u8 {
    match value {
        TargetCollectiveOperationV1::Ballot => 1,
        TargetCollectiveOperationV1::Any => 2,
        TargetCollectiveOperationV1::All => 3,
        TargetCollectiveOperationV1::Broadcast => 4,
        TargetCollectiveOperationV1::ReduceAdd => 5,
        TargetCollectiveOperationV1::ReduceMin => 6,
        TargetCollectiveOperationV1::ReduceMax => 7,
        TargetCollectiveOperationV1::InclusiveScanAdd => 8,
        TargetCollectiveOperationV1::ExclusiveScanAdd => 9,
    }
}

const fn matrix_operation_tag(value: TargetMatrixOperationV1) -> u8 {
    match value {
        TargetMatrixOperationV1::MatrixMultiplyAccumulate => 1,
        TargetMatrixOperationV1::ScaledMatrixMultiplyAccumulate => 2,
        TargetMatrixOperationV1::TensorContraction => 3,
    }
}

const fn matrix_layout_tag(value: TargetMatrixLayoutV1) -> u8 {
    match value {
        TargetMatrixLayoutV1::RowMajor => 1,
        TargetMatrixLayoutV1::ColumnMajor => 2,
        TargetMatrixLayoutV1::CooperativeFragment => 3,
    }
}

const fn numerical_mode_tag(value: TargetNumericalModeV1) -> u8 {
    match value {
        TargetNumericalModeV1::IeeeStrict => 1,
        TargetNumericalModeV1::AllowContraction => 2,
        TargetNumericalModeV1::AllowApproximation => 3,
    }
}

const fn endianness_tag(value: TargetEndiannessV1) -> u8 {
    match value {
        TargetEndiannessV1::Little => 1,
        TargetEndiannessV1::Big => 2,
    }
}

const fn object_format_tag(value: TargetObjectFormatV1) -> u8 {
    match value {
        TargetObjectFormatV1::LoadableExecutable => 1,
        TargetObjectFormatV1::RelocatableObject => 2,
        TargetObjectFormatV1::PortableModule => 3,
    }
}

const fn invalid_tag(field: &'static str, tag: u8) -> ProductionTargetCapabilityCanonicalErrorV1 {
    ProductionTargetCapabilityCanonicalErrorV1::InvalidTag { field, tag }
}

fn decode_capability_owner(
    tag: u8,
) -> Result<ProductionAmdCapabilityOwnerV1, ProductionTargetCapabilityCanonicalErrorV1> {
    match tag {
        1 => Ok(ProductionAmdCapabilityOwnerV1::ScalarMemoryLowering),
        2 => Ok(ProductionAmdCapabilityOwnerV1::AtomicLowering),
        3 => Ok(ProductionAmdCapabilityOwnerV1::SynchronizationLowering),
        4 => Ok(ProductionAmdCapabilityOwnerV1::WaveLowering),
        5 => Ok(ProductionAmdCapabilityOwnerV1::MatrixLowering),
        6 => Ok(ProductionAmdCapabilityOwnerV1::ResourceAdmission),
        7 => Ok(ProductionAmdCapabilityOwnerV1::KernelAbi),
        8 => Ok(ProductionAmdCapabilityOwnerV1::LoadableObjectEmitter),
        9 => Ok(ProductionAmdCapabilityOwnerV1::WorkgroupCollectiveLowering),
        10 => Ok(ProductionAmdCapabilityOwnerV1::AsyncCopyLowering),
        tag => Err(invalid_tag("capability owner", tag)),
    }
}

fn decode_launch_evidence_kind(
    tag: u8,
) -> Result<TargetLaunchEvidenceKindV1, ProductionTargetCapabilityCanonicalErrorV1> {
    match tag {
        1 => Ok(TargetLaunchEvidenceKindV1::DynamicSharedMemoryBytes),
        2 => Ok(TargetLaunchEvidenceKindV1::WorkgroupDimensions),
        3 => Ok(TargetLaunchEvidenceKindV1::CooperativeGridAdmission),
        4 => Ok(TargetLaunchEvidenceKindV1::SystemAtomicMemoryEligibility),
        tag => Err(invalid_tag("launch-evidence kind", tag)),
    }
}

fn decode_address_space(
    tag: u8,
) -> Result<TargetAddressSpaceV1, ProductionTargetCapabilityCanonicalErrorV1> {
    match tag {
        1 => Ok(TargetAddressSpaceV1::Global),
        2 => Ok(TargetAddressSpaceV1::Workgroup),
        3 => Ok(TargetAddressSpaceV1::Private),
        4 => Ok(TargetAddressSpaceV1::Constant),
        5 => Ok(TargetAddressSpaceV1::Generic),
        tag => Err(invalid_tag("address space", tag)),
    }
}

fn decode_memory_access(
    tag: u8,
) -> Result<TargetMemoryAccessV1, ProductionTargetCapabilityCanonicalErrorV1> {
    match tag {
        1 => Ok(TargetMemoryAccessV1::Read),
        2 => Ok(TargetMemoryAccessV1::Write),
        3 => Ok(TargetMemoryAccessV1::ReadWrite),
        tag => Err(invalid_tag("memory access", tag)),
    }
}

fn decode_scalar_kind(
    tag: u8,
) -> Result<TargetScalarKindV1, ProductionTargetCapabilityCanonicalErrorV1> {
    match tag {
        1 => Ok(TargetScalarKindV1::Boolean),
        2 => Ok(TargetScalarKindV1::SignedInteger),
        3 => Ok(TargetScalarKindV1::UnsignedInteger),
        4 => Ok(TargetScalarKindV1::Float),
        tag => Err(invalid_tag("scalar kind", tag)),
    }
}

fn decode_scalar_encoding(
    tag: u8,
) -> Result<TargetScalarEncodingV1, ProductionTargetCapabilityCanonicalErrorV1> {
    match tag {
        1 => Ok(TargetScalarEncodingV1::Boolean),
        2 => Ok(TargetScalarEncodingV1::TwosComplement),
        3 => Ok(TargetScalarEncodingV1::IeeeBinary),
        4 => Ok(TargetScalarEncodingV1::BFloat),
        5 => Ok(TargetScalarEncodingV1::Float4E2M1Ocp),
        6 => Ok(TargetScalarEncodingV1::Float8E4M3Fnuz),
        7 => Ok(TargetScalarEncodingV1::Float8E5M2Fnuz),
        8 => Ok(TargetScalarEncodingV1::Float8E4M3Ocp),
        9 => Ok(TargetScalarEncodingV1::Float8E5M2Ocp),
        tag => Err(invalid_tag("scalar encoding", tag)),
    }
}

fn decode_atomic_operation(
    tag: u8,
) -> Result<TargetAtomicOperationV1, ProductionTargetCapabilityCanonicalErrorV1> {
    match tag {
        1 => Ok(TargetAtomicOperationV1::Load),
        2 => Ok(TargetAtomicOperationV1::Store),
        3 => Ok(TargetAtomicOperationV1::Exchange),
        4 => Ok(TargetAtomicOperationV1::CompareExchange),
        5 => Ok(TargetAtomicOperationV1::Add),
        6 => Ok(TargetAtomicOperationV1::Sub),
        7 => Ok(TargetAtomicOperationV1::Min),
        8 => Ok(TargetAtomicOperationV1::Max),
        9 => Ok(TargetAtomicOperationV1::And),
        10 => Ok(TargetAtomicOperationV1::Nand),
        11 => Ok(TargetAtomicOperationV1::Or),
        12 => Ok(TargetAtomicOperationV1::Xor),
        tag => Err(invalid_tag("atomic operation", tag)),
    }
}

fn decode_memory_ordering(
    tag: u8,
) -> Result<TargetMemoryOrderingV1, ProductionTargetCapabilityCanonicalErrorV1> {
    match tag {
        1 => Ok(TargetMemoryOrderingV1::Relaxed),
        2 => Ok(TargetMemoryOrderingV1::Acquire),
        3 => Ok(TargetMemoryOrderingV1::Release),
        4 => Ok(TargetMemoryOrderingV1::AcquireRelease),
        5 => Ok(TargetMemoryOrderingV1::SequentiallyConsistent),
        tag => Err(invalid_tag("memory ordering", tag)),
    }
}

fn decode_memory_scope(
    tag: u8,
) -> Result<TargetMemoryScopeV1, ProductionTargetCapabilityCanonicalErrorV1> {
    match tag {
        1 => Ok(TargetMemoryScopeV1::Invocation),
        2 => Ok(TargetMemoryScopeV1::Subgroup),
        3 => Ok(TargetMemoryScopeV1::Workgroup),
        4 => Ok(TargetMemoryScopeV1::Device),
        5 => Ok(TargetMemoryScopeV1::System),
        tag => Err(invalid_tag("memory scope", tag)),
    }
}

fn decode_execution_scope(
    tag: u8,
) -> Result<TargetExecutionScopeV1, ProductionTargetCapabilityCanonicalErrorV1> {
    match tag {
        1 => Ok(TargetExecutionScopeV1::Subgroup),
        2 => Ok(TargetExecutionScopeV1::Workgroup),
        3 => Ok(TargetExecutionScopeV1::Grid),
        tag => Err(invalid_tag("execution scope", tag)),
    }
}

fn decode_barrier_participation(
    tag: u8,
) -> Result<TargetBarrierParticipationV1, ProductionTargetCapabilityCanonicalErrorV1> {
    match tag {
        1 => Ok(TargetBarrierParticipationV1::Uniform),
        2 => Ok(TargetBarrierParticipationV1::DynamicMask),
        tag => Err(invalid_tag("barrier participation", tag)),
    }
}

fn decode_collective_operation(
    tag: u8,
) -> Result<TargetCollectiveOperationV1, ProductionTargetCapabilityCanonicalErrorV1> {
    match tag {
        1 => Ok(TargetCollectiveOperationV1::Ballot),
        2 => Ok(TargetCollectiveOperationV1::Any),
        3 => Ok(TargetCollectiveOperationV1::All),
        4 => Ok(TargetCollectiveOperationV1::Broadcast),
        5 => Ok(TargetCollectiveOperationV1::ReduceAdd),
        6 => Ok(TargetCollectiveOperationV1::ReduceMin),
        7 => Ok(TargetCollectiveOperationV1::ReduceMax),
        8 => Ok(TargetCollectiveOperationV1::InclusiveScanAdd),
        9 => Ok(TargetCollectiveOperationV1::ExclusiveScanAdd),
        tag => Err(invalid_tag("collective operation", tag)),
    }
}

fn decode_matrix_operation(
    tag: u8,
) -> Result<TargetMatrixOperationV1, ProductionTargetCapabilityCanonicalErrorV1> {
    match tag {
        1 => Ok(TargetMatrixOperationV1::MatrixMultiplyAccumulate),
        2 => Ok(TargetMatrixOperationV1::ScaledMatrixMultiplyAccumulate),
        3 => Ok(TargetMatrixOperationV1::TensorContraction),
        tag => Err(invalid_tag("matrix operation", tag)),
    }
}

fn decode_matrix_layout(
    tag: u8,
) -> Result<TargetMatrixLayoutV1, ProductionTargetCapabilityCanonicalErrorV1> {
    match tag {
        1 => Ok(TargetMatrixLayoutV1::RowMajor),
        2 => Ok(TargetMatrixLayoutV1::ColumnMajor),
        3 => Ok(TargetMatrixLayoutV1::CooperativeFragment),
        tag => Err(invalid_tag("matrix layout", tag)),
    }
}

fn decode_numerical_mode(
    tag: u8,
) -> Result<TargetNumericalModeV1, ProductionTargetCapabilityCanonicalErrorV1> {
    match tag {
        1 => Ok(TargetNumericalModeV1::IeeeStrict),
        2 => Ok(TargetNumericalModeV1::AllowContraction),
        3 => Ok(TargetNumericalModeV1::AllowApproximation),
        tag => Err(invalid_tag("numerical mode", tag)),
    }
}

fn decode_endianness(
    tag: u8,
) -> Result<TargetEndiannessV1, ProductionTargetCapabilityCanonicalErrorV1> {
    match tag {
        1 => Ok(TargetEndiannessV1::Little),
        2 => Ok(TargetEndiannessV1::Big),
        tag => Err(invalid_tag("endianness", tag)),
    }
}

fn decode_object_format(
    tag: u8,
) -> Result<TargetObjectFormatV1, ProductionTargetCapabilityCanonicalErrorV1> {
    match tag {
        1 => Ok(TargetObjectFormatV1::LoadableExecutable),
        2 => Ok(TargetObjectFormatV1::RelocatableObject),
        3 => Ok(TargetObjectFormatV1::PortableModule),
        tag => Err(invalid_tag("object format", tag)),
    }
}
