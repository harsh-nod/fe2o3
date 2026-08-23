//! Independent raw-HSACO inspection of sealed Worker V2 first-build evidence.
//!
//! This boundary consumes and retains the inert first-build evidence. It is deliberately not
//! canonical descriptor finalization and grants no publication, loading, or launch authority.

use std::{collections::BTreeSet, error::Error, fmt};

use fe2o3_artifact_transaction::{
    BuildAttempt, CompilerModuleHandoffIdentityV1, CompilerModuleHandoffIdentityV2,
    CompilerModuleHandoffSlotV2, CompilerModuleHandoffSlotV3,
    CompilerModuleHandoffTransactionIdentityV3,
};
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_compiler_ffi::{
    CodeObjectVersion as CompilerCodeObjectVersion, CompilerFfiEnvelopeIdentityV1,
    CompilerModuleSymbolManifestV1, CompilerModuleSymbolRoleV1,
    InertFinalCompilerModuleCommitmentV3, InertSemanticCompilerModuleHandoffIdentityV3,
    InertSemanticCompilerModuleHandoffV3,
};
use fe2o3_hsaco::{
    ArgumentAccess, ArgumentAddressSpace, CodeObjectVersion as InspectedCodeObjectVersion,
    ExplicitArgument, ExplicitValueKind, ExplicitValueType, HiddenArgument, HiddenValueKind,
    InspectedKernelBindings, KernelBindingError, inspect_and_bind_kernel_descriptors,
};
use fe2o3_kernel_descriptor::{
    BlockSizeV1, CodeObjectVersion, DeviceTargetV1, KernelDescriptorV1,
    ROW_SOFTMAX_V1_MAX_FLAT_WORKGROUP_SIZE, ROW_SOFTMAX_V1_WORKGROUP_SIZE,
    TILED_GEMM_V1_MAX_FLAT_WORKGROUP_SIZE, TILED_GEMM_V1_WORKGROUP_SIZE,
};
use object::{Object, ObjectSection, ObjectSymbol};
use sha2::{Digest, Sha256};

use crate::{
    ContentIdentityV1, DEVICE_DESCRIPTOR_SECTION_NAME, FinalizationError,
    FirstBuildWorkerV2IdentityV1, InertFirstBuildWorkerV2EvidenceV1,
    InertProtectedFirstBuildWorkerV2EvidenceV1, InertProtectedFirstBuildWorkerV3EvidenceV1,
    MAX_WORKER_SYMBOLS, MultiInputLinkPlanV1, ProtectedCompilerHandoffBindingIdentityV3,
    ProtectedCompilerHandoffExpectationV3, ProtectedFirstBuildWorkerV2IdentityV1,
    ProtectedFirstBuildWorkerV3IdentityV1, WorkerCompilerFfiEnvelopeIdentityV2,
    WorkerMeasurementV1, request_construction::decode_link_options,
};

const EXPECTED_PROCESSOR: &str = "gfx942";
const REQUIRED_WORKGROUP_SIZE: [u32; 3] = [256, 1, 1];
const REQUIRED_MAX_FLAT_WORKGROUP_SIZE: u32 = 256;
const REQUIRED_WAVEFRONT_SIZE: u32 = 64;
const POLICY_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/WORKER-V2-RAW-HSACO-POLICY/V1\0";
const RESPONSE_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/WORKER-V2-SEALED-RESPONSE/V1\0";
const INSPECTION_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/WORKER-V2-RAW-HSACO-INSPECTION/V1\0";
const PROTECTED_INSPECTION_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/CLOSURE-PROTECTED-WORKER-V2-RAW-HSACO-INSPECTION/V1\0";
const PROTECTED_V3_INSPECTION_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/STRICT-V3-PROTECTED-WORKER-RAW-HSACO-INSPECTION/V1\0";
const PROTECTED_SOURCE_EVIDENCE_DOMAIN_V1: &[u8] =
    b"FE2O3/PROTECTED-RAW-HSACO/SOURCE-EVIDENCE/V1\0";
const PROTECTED_ATTEMPT_DOMAIN_V1: &[u8] = b"FE2O3/PROTECTED-RAW-HSACO/ATTEMPT/V1\0";
const PROTECTED_HANDOFF_DOMAIN_V2: &[u8] = b"FE2O3/PROTECTED-RAW-HSACO/HANDOFF/V2\0";
const PROTECTED_COMPILER_CLOSURE_DOMAIN_V2: &[u8] =
    b"FE2O3/PROTECTED-RAW-HSACO/COMPILER-CLOSURE/V2\0";
const PROTECTED_WORKER_EXCHANGE_DOMAIN_V1: &[u8] =
    b"FE2O3/PROTECTED-RAW-HSACO/WORKER-REQUEST-RESPONSE/V1\0";
const PROTECTED_RAW_BYTES_DOMAIN_V1: &[u8] = b"FE2O3/PROTECTED-RAW-HSACO/EXACT-RAW-BYTES/V1\0";
const PROTECTED_TARGET_DOMAIN_V1: &[u8] = b"FE2O3/PROTECTED-RAW-HSACO/TARGET/V1\0";
const PROTECTED_CODE_OBJECT_VERSION_DOMAIN_V1: &[u8] =
    b"FE2O3/PROTECTED-RAW-HSACO/CODE-OBJECT-VERSION/V1\0";
const PROTECTED_POLICY_DOMAIN_V1: &[u8] = b"FE2O3/PROTECTED-RAW-HSACO/POLICY/V1\0";
const PROTECTED_DESCRIPTOR_DOMAIN_V1: &[u8] = b"FE2O3/PROTECTED-RAW-HSACO/AMDHSA-DESCRIPTORS/V1\0";
const PROTECTED_ABI_DOMAIN_V1: &[u8] = b"FE2O3/PROTECTED-RAW-HSACO/KERNEL-ABI/V1\0";
const PROTECTED_RESOURCES_DOMAIN_V1: &[u8] = b"FE2O3/PROTECTED-RAW-HSACO/KERNEL-RESOURCES/V1\0";

/// One exact kernel entry/descriptor pair independently observed in AMDHSA metadata and ELF.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ObservedWorkerV2KernelSymbolsV1 {
    entry: String,
    descriptor: String,
}

impl ObservedWorkerV2KernelSymbolsV1 {
    pub fn entry(&self) -> &str {
        &self.entry
    }

    pub fn descriptor(&self) -> &str {
        &self.descriptor
    }
}

/// Exact fixed launch properties required from every inspected gfx942 kernel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerV2RawLaunchContractV1 {
    required_workgroup_size: [u32; 3],
    max_flat_workgroup_size: u32,
    wavefront_size: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorkerV2RawLaunchDiagnosticProfileV1 {
    LegacyGfx942G1,
    ProductionV1,
    TiledGemmV1,
    GeneralGemmV1,
    RowSoftmaxV1,
    FlashAttentionV1,
    Wave64CollectivesV1,
    WorkgroupSyncV1,
    MoeTop2V1,
}

impl WorkerV2RawLaunchContractV1 {
    const GFX942_G1: Self = Self {
        required_workgroup_size: REQUIRED_WORKGROUP_SIZE,
        max_flat_workgroup_size: REQUIRED_MAX_FLAT_WORKGROUP_SIZE,
        wavefront_size: REQUIRED_WAVEFRONT_SIZE,
    };

    pub(crate) const TILED_GEMM_V1: Self = Self {
        required_workgroup_size: TILED_GEMM_V1_WORKGROUP_SIZE,
        max_flat_workgroup_size: TILED_GEMM_V1_MAX_FLAT_WORKGROUP_SIZE,
        wavefront_size: REQUIRED_WAVEFRONT_SIZE,
    };

    const GENERAL_GEMM_V1: Self = Self {
        required_workgroup_size: [64, 1, 1],
        max_flat_workgroup_size: 64,
        wavefront_size: REQUIRED_WAVEFRONT_SIZE,
    };

    const PRODUCTION_V1: Self = Self {
        required_workgroup_size: [64, 1, 1],
        max_flat_workgroup_size: 64,
        wavefront_size: REQUIRED_WAVEFRONT_SIZE,
    };

    pub(crate) const ROW_SOFTMAX_V1: Self = Self {
        required_workgroup_size: ROW_SOFTMAX_V1_WORKGROUP_SIZE,
        max_flat_workgroup_size: ROW_SOFTMAX_V1_MAX_FLAT_WORKGROUP_SIZE,
        wavefront_size: REQUIRED_WAVEFRONT_SIZE,
    };

    pub(crate) const FLASH_ATTENTION_V1: Self = Self {
        required_workgroup_size: [64, 1, 1],
        max_flat_workgroup_size: 64,
        wavefront_size: REQUIRED_WAVEFRONT_SIZE,
    };

    pub(crate) const WAVE64_COLLECTIVES_V1: Self = Self {
        required_workgroup_size: [64, 1, 1],
        max_flat_workgroup_size: 64,
        wavefront_size: REQUIRED_WAVEFRONT_SIZE,
    };

    pub(crate) const WORKGROUP_SYNC_V1: Self = Self {
        required_workgroup_size: [64, 1, 1],
        max_flat_workgroup_size: 64,
        wavefront_size: REQUIRED_WAVEFRONT_SIZE,
    };

    pub(crate) const MOE_TOP2_V1: Self = Self {
        required_workgroup_size: [64, 1, 1],
        max_flat_workgroup_size: 64,
        wavefront_size: REQUIRED_WAVEFRONT_SIZE,
    };

    pub const fn required_workgroup_size(self) -> [u32; 3] {
        self.required_workgroup_size
    }

    pub const fn max_flat_workgroup_size(self) -> u32 {
        self.max_flat_workgroup_size
    }

    pub const fn wavefront_size(self) -> u32 {
        self.wavefront_size
    }

    pub(crate) const fn from_transcript_parts(
        required_workgroup_size: [u32; 3],
        max_flat_workgroup_size: u32,
        wavefront_size: u32,
    ) -> Option<Self> {
        if required_workgroup_size[0] == 0
            || required_workgroup_size[1] == 0
            || required_workgroup_size[2] == 0
            || max_flat_workgroup_size == 0
            || wavefront_size == 0
        {
            return None;
        }
        Some(Self {
            required_workgroup_size,
            max_flat_workgroup_size,
            wavefront_size,
        })
    }
}

/// Stable identity of policy facts derived from retained evidence and raw-HSACO observations.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorkerV2RawHsacoPolicyIdentityV1([u8; 32]);

impl WorkerV2RawHsacoPolicyIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Policy reconstructed without accepting caller-supplied target, contract, or symbol claims.
///
/// Symbol roles come from the manifest retained by sealed first-build evidence. Kernel pairings
/// come from independent raw-HSACO inspection. The manifest and cooperative handoff record claims;
/// neither authenticates compiler authorship.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerV2RawHsacoPolicyV1 {
    identity: WorkerV2RawHsacoPolicyIdentityV1,
    target: DeviceTargetV1,
    code_object_version: CodeObjectVersion,
    compiler_envelope: CompilerFfiEnvelopeIdentityV1,
    symbol_manifest: CompilerModuleSymbolManifestV1,
    observed_kernels: Vec<ObservedWorkerV2KernelSymbolsV1>,
    expected_defined_symbols: Vec<String>,
    launch: WorkerV2RawLaunchContractV1,
}

impl WorkerV2RawHsacoPolicyV1 {
    pub const fn identity(&self) -> WorkerV2RawHsacoPolicyIdentityV1 {
        self.identity
    }

    pub const fn target(&self) -> DeviceTargetV1 {
        self.target
    }

    pub const fn code_object_version(&self) -> CodeObjectVersion {
        self.code_object_version
    }

    pub const fn compiler_envelope_identity(&self) -> CompilerFfiEnvelopeIdentityV1 {
        self.compiler_envelope
    }

    pub const fn symbol_manifest(&self) -> &CompilerModuleSymbolManifestV1 {
        &self.symbol_manifest
    }

    pub fn observed_kernels(&self) -> &[ObservedWorkerV2KernelSymbolsV1] {
        &self.observed_kernels
    }

    pub fn expected_defined_symbols(&self) -> &[String] {
        &self.expected_defined_symbols
    }

    pub const fn launch(&self) -> WorkerV2RawLaunchContractV1 {
        self.launch
    }

    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }
}

/// Observation of the canonical fe2o3 descriptor-table section in raw Worker output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalDescriptorSectionObservationV1 {
    Missing,
    PresentButNotFinalizedByThisInspection,
}

/// SHA-256 identity of the exact canonical sealed Worker V2 response bytes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SealedWorkerV2ResponseIdentityV1([u8; 32]);

impl SealedWorkerV2ResponseIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Stable identity binding complete first-build lineage, raw bytes, and reconstructed policy.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InspectedRawWorkerV2HsacoIdentityV1([u8; 32]);

impl InspectedRawWorkerV2HsacoIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Inert evidence that exact sealed Worker V2 output passed independent raw-HSACO inspection.
///
/// This retains the consumed first-build evidence. It is not a finalized HSACO, does not
/// authenticate compiler origin, and cannot publish, load, or launch an artifact.
#[derive(Debug, Eq, PartialEq)]
pub struct InspectedRawWorkerV2HsacoV1 {
    identity: InspectedRawWorkerV2HsacoIdentityV1,
    response_identity: SealedWorkerV2ResponseIdentityV1,
    descriptor_section: CanonicalDescriptorSectionObservationV1,
    policy: WorkerV2RawHsacoPolicyV1,
    source: InertFirstBuildWorkerV2EvidenceV1,
}

impl InspectedRawWorkerV2HsacoV1 {
    pub const fn identity(&self) -> InspectedRawWorkerV2HsacoIdentityV1 {
        self.identity
    }

    pub const fn source_evidence_identity(&self) -> FirstBuildWorkerV2IdentityV1 {
        self.source.identity()
    }

    pub const fn attempt(&self) -> BuildAttempt {
        self.source.attempt()
    }

    pub const fn handoff_identity(&self) -> CompilerModuleHandoffIdentityV1 {
        self.source.handoff_identity()
    }

    /// Returns the identity of the complete link plan retained by sealed first-build evidence.
    pub const fn link_plan_identity(&self) -> crate::LinkPlanIdentityV1 {
        self.source.link_plan_identity()
    }

    pub const fn worker_measurement(&self) -> &WorkerMeasurementV1 {
        self.source.worker_measurement()
    }

    /// Identity of the complete compiler FFI contract envelope retained from the V2 handoff.
    pub const fn compiler_envelope_identity(&self) -> CompilerFfiEnvelopeIdentityV1 {
        self.source.compiler_envelope_identity()
    }

    /// The matching envelope identity sealed into the independently checked Worker V2 response.
    pub const fn sealed_compiler_envelope_identity(&self) -> WorkerCompilerFfiEnvelopeIdentityV2 {
        self.source
            .authorized()
            .response()
            .compiler_envelope_identity()
    }

    /// Returns the sealed request id echoed by the already request-verified V2 response.
    pub fn sealed_request_id(&self) -> &[u8; 32] {
        self.source.authorized().response().request_id()
    }

    /// Returns the sealed request identity echoed by the already request-verified V2 response.
    pub fn sealed_request_identity(&self) -> &[u8; 32] {
        self.source.authorized().response().request_identity()
    }

    pub const fn response_identity(&self) -> SealedWorkerV2ResponseIdentityV1 {
        self.response_identity
    }

    pub const fn linked_output_identity(&self) -> ContentIdentityV1 {
        self.source.output_identity()
    }

    pub fn exact_bytes(&self) -> &[u8] {
        self.source.output_bytes()
    }

    pub const fn target(&self) -> DeviceTargetV1 {
        self.policy.target()
    }

    pub const fn code_object_version(&self) -> CodeObjectVersion {
        self.policy.code_object_version()
    }

    pub const fn policy(&self) -> &WorkerV2RawHsacoPolicyV1 {
        &self.policy
    }

    pub const fn canonical_descriptor_section(&self) -> CanonicalDescriptorSectionObservationV1 {
        self.descriptor_section
    }

    pub const fn canonical_descriptor_finalization_ran(&self) -> bool {
        false
    }

    pub const fn authenticates_compiler_origin(&self) -> bool {
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

/// Stable identity of one closure-protected raw Worker V2 HSACO inspection.
///
/// This identity has a distinct transcript from [`InspectedRawWorkerV2HsacoIdentityV1`]. It binds
/// the exact V2 handoff and complete compiler closure without representing either as V1 evidence.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InspectedProtectedRawWorkerV2HsacoIdentityV1([u8; 32]);

impl InspectedProtectedRawWorkerV2HsacoIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Stable identity of one strict-V3 protected raw-HSACO inspection.
///
/// This transcript is separate from both unprotected and protected V2 inspection. It retains the
/// complete V3 transaction, semantic handoff, compiler closure, worker exchange, and raw-HSACO
/// observations without converting any of them to V2 evidence.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InspectedProtectedRawWorkerV3HsacoIdentityV1([u8; 32]);

impl InspectedProtectedRawWorkerV3HsacoIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Inert strict-V3 evidence that exact direct-LLVM Worker output passed raw-HSACO inspection.
///
/// The complete V3 first-build evidence remains owned by this value. Structural inspection does
/// not authenticate the compiler, prove semantic correctness, or grant publication, load, or
/// launch authority.
#[derive(Debug, Eq, PartialEq)]
pub struct InspectedProtectedRawWorkerV3HsacoV1 {
    identity: InspectedProtectedRawWorkerV3HsacoIdentityV1,
    response_identity: SealedWorkerV2ResponseIdentityV1,
    descriptor_section: CanonicalDescriptorSectionObservationV1,
    descriptor_observation_preimage: Vec<u8>,
    abi_observation_preimage: Vec<u8>,
    resource_observation_preimage: Vec<u8>,
    policy: WorkerV2RawHsacoPolicyV1,
    source: InertProtectedFirstBuildWorkerV3EvidenceV1,
}

impl InspectedProtectedRawWorkerV3HsacoV1 {
    pub const fn identity(&self) -> InspectedProtectedRawWorkerV3HsacoIdentityV1 {
        self.identity
    }

    pub const fn source_evidence_identity(&self) -> ProtectedFirstBuildWorkerV3IdentityV1 {
        self.source.identity()
    }

    pub const fn binding_identity(&self) -> ProtectedCompilerHandoffBindingIdentityV3 {
        self.source.binding().identity()
    }

    pub const fn binding_expectation(&self) -> ProtectedCompilerHandoffExpectationV3 {
        self.source.binding().expectation()
    }

    pub const fn attempt(&self) -> BuildAttempt {
        self.binding_expectation().attempt()
    }

    pub const fn handoff_slot(&self) -> CompilerModuleHandoffSlotV3 {
        self.binding_expectation().slot()
    }

    pub const fn transaction_identity(&self) -> CompilerModuleHandoffTransactionIdentityV3 {
        self.binding_expectation().transaction_identity()
    }

    pub const fn outer_handoff_identity(&self) -> InertSemanticCompilerModuleHandoffIdentityV3 {
        self.binding_expectation().outer_handoff_identity()
    }

    pub const fn outer_handoff(&self) -> &InertSemanticCompilerModuleHandoffV3 {
        self.source.handoff()
    }

    pub const fn compiler_closure(&self) -> CompilerClosureV2 {
        self.binding_expectation().compiler_closure()
    }

    pub const fn plan(&self) -> &MultiInputLinkPlanV1 {
        self.source.plan()
    }

    pub const fn link_plan_identity(&self) -> crate::LinkPlanIdentityV1 {
        self.source.plan().identity()
    }

    pub const fn worker_measurement(&self) -> &WorkerMeasurementV1 {
        self.source.worker_measurement()
    }

    pub const fn compiler_envelope_identity(&self) -> CompilerFfiEnvelopeIdentityV1 {
        self.source.handoff().module_handoff().envelope().identity()
    }

    pub const fn sealed_compiler_envelope_identity(&self) -> WorkerCompilerFfiEnvelopeIdentityV2 {
        self.source
            .exact_replay()
            .response()
            .compiler_envelope_identity()
    }

    pub fn sealed_request_id(&self) -> &[u8; 32] {
        self.source.exact_replay().response().request_id()
    }

    pub fn sealed_request_identity(&self) -> &[u8; 32] {
        self.source.exact_replay().response().request_identity()
    }

    pub const fn response_identity(&self) -> SealedWorkerV2ResponseIdentityV1 {
        self.response_identity
    }

    pub const fn linked_output_identity(&self) -> ContentIdentityV1 {
        self.source.output_identity()
    }

    pub const fn raw_hsaco_identity(&self) -> ContentIdentityV1 {
        self.linked_output_identity()
    }

    pub fn exact_bytes(&self) -> &[u8] {
        self.source.output_bytes()
    }

    pub const fn target(&self) -> DeviceTargetV1 {
        self.policy.target()
    }

    pub const fn code_object_version(&self) -> CodeObjectVersion {
        self.policy.code_object_version()
    }

    pub const fn policy(&self) -> &WorkerV2RawHsacoPolicyV1 {
        &self.policy
    }

    /// Borrows the complete native V3 first-build evidence retained by this inspection.
    pub const fn source_evidence(&self) -> &InertProtectedFirstBuildWorkerV3EvidenceV1 {
        &self.source
    }

    pub(crate) fn into_source_evidence(self) -> InertProtectedFirstBuildWorkerV3EvidenceV1 {
        self.source
    }

    /// Returns the exact canonical descriptor observation transcript.
    pub fn descriptor_observation_preimage(&self) -> &[u8] {
        &self.descriptor_observation_preimage
    }

    /// Returns the exact canonical kernel-ABI observation transcript.
    pub fn abi_observation_preimage(&self) -> &[u8] {
        &self.abi_observation_preimage
    }

    /// Returns the exact canonical kernel-resource observation transcript.
    pub fn resource_observation_preimage(&self) -> &[u8] {
        &self.resource_observation_preimage
    }

    /// Returns descriptor, ABI, and resource identities in that order.
    pub fn observation_identities(&self) -> ([u8; 32], [u8; 32], [u8; 32]) {
        (
            calculate_observation_identity(
                PROTECTED_DESCRIPTOR_DOMAIN_V1,
                &self.descriptor_observation_preimage,
            ),
            calculate_observation_identity(PROTECTED_ABI_DOMAIN_V1, &self.abi_observation_preimage),
            calculate_observation_identity(
                PROTECTED_RESOURCES_DOMAIN_V1,
                &self.resource_observation_preimage,
            ),
        )
    }

    pub const fn canonical_descriptor_section(&self) -> CanonicalDescriptorSectionObservationV1 {
        self.descriptor_section
    }

    pub const fn canonical_descriptor_finalization_ran(&self) -> bool {
        false
    }

    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }

    pub const fn proves_semantic_correctness(&self) -> bool {
        false
    }

    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }

    pub const fn grants_link_authority(&self) -> bool {
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

/// Inert closure-protected evidence that exact Worker V2 output passed raw-HSACO inspection.
///
/// The exact V2 transaction identity and full compiler closure remain directly inspectable. The
/// retained plan and bytes are borrow-only restart inputs; this value grants no compiler, link,
/// publication, loading, or launch authority.
#[derive(Debug, Eq, PartialEq)]
pub struct InspectedProtectedRawWorkerV2HsacoV1 {
    identity: InspectedProtectedRawWorkerV2HsacoIdentityV1,
    response_identity: SealedWorkerV2ResponseIdentityV1,
    descriptor_section: CanonicalDescriptorSectionObservationV1,
    descriptor_observation_preimage: Vec<u8>,
    abi_observation_preimage: Vec<u8>,
    resource_observation_preimage: Vec<u8>,
    policy: WorkerV2RawHsacoPolicyV1,
    source: InertProtectedFirstBuildWorkerV2EvidenceV1,
}

impl InspectedProtectedRawWorkerV2HsacoV1 {
    pub const fn identity(&self) -> InspectedProtectedRawWorkerV2HsacoIdentityV1 {
        self.identity
    }

    /// Identity of the closure-protected first-build evidence consumed by this inspection.
    pub const fn source_evidence_identity(&self) -> ProtectedFirstBuildWorkerV2IdentityV1 {
        self.source.identity()
    }

    /// Schema-neutral name for the protected evidence identity used by restart persistence.
    pub const fn upstream_evidence_identity(&self) -> ProtectedFirstBuildWorkerV2IdentityV1 {
        self.source_evidence_identity()
    }

    pub const fn attempt(&self) -> BuildAttempt {
        self.source.attempt()
    }

    pub const fn handoff_slot(&self) -> CompilerModuleHandoffSlotV2 {
        self.source.slot()
    }

    pub const fn handoff_identity(&self) -> CompilerModuleHandoffIdentityV2 {
        self.source.handoff_identity()
    }

    pub const fn compiler_closure(&self) -> CompilerClosureV2 {
        self.source.compiler_closure()
    }

    /// Returns the complete retained link plan for schema-neutral restart persistence.
    pub const fn plan(&self) -> &MultiInputLinkPlanV1 {
        self.source.plan()
    }

    pub const fn link_plan_identity(&self) -> crate::LinkPlanIdentityV1 {
        self.source.link_plan_identity()
    }

    pub const fn worker_measurement(&self) -> &WorkerMeasurementV1 {
        self.source.worker_measurement()
    }

    pub const fn compiler_envelope_identity(&self) -> CompilerFfiEnvelopeIdentityV1 {
        self.source.compiler_envelope_identity()
    }

    pub const fn sealed_compiler_envelope_identity(&self) -> WorkerCompilerFfiEnvelopeIdentityV2 {
        self.source
            .authorized()
            .response()
            .compiler_envelope_identity()
    }

    pub fn sealed_request_id(&self) -> &[u8; 32] {
        self.source.authorized().response().request_id()
    }

    pub fn sealed_request_identity(&self) -> &[u8; 32] {
        self.source.authorized().response().request_identity()
    }

    pub const fn response_identity(&self) -> SealedWorkerV2ResponseIdentityV1 {
        self.response_identity
    }

    pub const fn linked_output_identity(&self) -> ContentIdentityV1 {
        self.source.output_identity()
    }

    /// Borrows the exact inspected raw HSACO bytes for inert persistence or further inspection.
    pub fn exact_bytes(&self) -> &[u8] {
        self.source.output_bytes()
    }

    pub const fn target(&self) -> DeviceTargetV1 {
        self.policy.target()
    }

    pub const fn code_object_version(&self) -> CodeObjectVersion {
        self.policy.code_object_version()
    }

    pub const fn policy(&self) -> &WorkerV2RawHsacoPolicyV1 {
        &self.policy
    }

    pub(crate) const fn source_evidence(&self) -> &InertProtectedFirstBuildWorkerV2EvidenceV1 {
        &self.source
    }

    pub(crate) fn descriptor_observation_preimage(&self) -> &[u8] {
        &self.descriptor_observation_preimage
    }

    pub(crate) fn abi_observation_preimage(&self) -> &[u8] {
        &self.abi_observation_preimage
    }

    pub(crate) fn resource_observation_preimage(&self) -> &[u8] {
        &self.resource_observation_preimage
    }

    pub(crate) fn observation_identities(&self) -> ([u8; 32], [u8; 32], [u8; 32]) {
        (
            calculate_observation_identity(
                PROTECTED_DESCRIPTOR_DOMAIN_V1,
                &self.descriptor_observation_preimage,
            ),
            calculate_observation_identity(PROTECTED_ABI_DOMAIN_V1, &self.abi_observation_preimage),
            calculate_observation_identity(
                PROTECTED_RESOURCES_DOMAIN_V1,
                &self.resource_observation_preimage,
            ),
        )
    }

    pub const fn canonical_descriptor_section(&self) -> CanonicalDescriptorSectionObservationV1 {
        self.descriptor_section
    }

    pub const fn canonical_descriptor_finalization_ran(&self) -> bool {
        false
    }

    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }

    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }

    pub const fn grants_link_authority(&self) -> bool {
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

/// Why sealed Worker V2 evidence failed independent raw-HSACO inspection.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WorkerV2RawHsacoInspectionError {
    LineageMismatch(&'static str),
    UnsupportedTarget(String),
    LinkPolicy,
    HsacoBinding(KernelBindingError),
    TargetMismatch {
        expected: String,
        actual: String,
    },
    CodeObjectVersionMismatch {
        expected: CodeObjectVersion,
        actual: CodeObjectVersion,
    },
    MissingKernel,
    KernelEntryRoleMismatch,
    KernelDescriptorRoleMismatch,
    CompilerEnvelopeImportRoleMismatch,
    CompilerEnvelopeExportRoleMismatch,
    DefinedSymbolInspection,
    DefinedSymbolClosureMismatch {
        expected: Vec<String>,
        actual: Vec<String>,
    },
    RequiredWorkgroupSizeMismatch {
        kernel: String,
        actual: Option<[u32; 3]>,
    },
    MaxFlatWorkgroupSizeMismatch {
        kernel: String,
        actual: u32,
    },
    MetadataWavefrontSizeMismatch {
        kernel: String,
        actual: u32,
    },
    DescriptorWavefrontSizeMismatch {
        kernel: String,
        actual: u32,
    },
    ProductionV1RequiredWorkgroupSizeMismatch {
        kernel: String,
        actual: Option<[u32; 3]>,
        expected: [u32; 3],
    },
    ProductionV1MaxFlatWorkgroupSizeMismatch {
        kernel: String,
        actual: u32,
        expected: u32,
    },
    ProductionV1MetadataWavefrontSizeMismatch {
        kernel: String,
        actual: u32,
        expected: u32,
    },
    ProductionV1DescriptorWavefrontSizeMismatch {
        kernel: String,
        actual: u32,
        expected: u32,
    },
    StrictV3DescriptorLaunchContract(&'static str),
    TiledGemmV1RequiredWorkgroupSizeMismatch {
        kernel: String,
        actual: Option<[u32; 3]>,
        expected: [u32; 3],
    },
    TiledGemmV1MaxFlatWorkgroupSizeMismatch {
        kernel: String,
        actual: u32,
        expected: u32,
    },
    TiledGemmV1MetadataWavefrontSizeMismatch {
        kernel: String,
        actual: u32,
        expected: u32,
    },
    TiledGemmV1DescriptorWavefrontSizeMismatch {
        kernel: String,
        actual: u32,
        expected: u32,
    },
    GeneralGemmV1RequiredWorkgroupSizeMismatch {
        kernel: String,
        actual: Option<[u32; 3]>,
        expected: [u32; 3],
    },
    GeneralGemmV1MaxFlatWorkgroupSizeMismatch {
        kernel: String,
        actual: u32,
        expected: u32,
    },
    GeneralGemmV1MetadataWavefrontSizeMismatch {
        kernel: String,
        actual: u32,
        expected: u32,
    },
    GeneralGemmV1DescriptorWavefrontSizeMismatch {
        kernel: String,
        actual: u32,
        expected: u32,
    },
    RowSoftmaxV1RequiredWorkgroupSizeMismatch {
        kernel: String,
        actual: Option<[u32; 3]>,
        expected: [u32; 3],
    },
    RowSoftmaxV1MaxFlatWorkgroupSizeMismatch {
        kernel: String,
        actual: u32,
        expected: u32,
    },
    RowSoftmaxV1MetadataWavefrontSizeMismatch {
        kernel: String,
        actual: u32,
        expected: u32,
    },
    RowSoftmaxV1DescriptorWavefrontSizeMismatch {
        kernel: String,
        actual: u32,
        expected: u32,
    },
    FlashAttentionV1RequiredWorkgroupSizeMismatch {
        kernel: String,
        actual: Option<[u32; 3]>,
        expected: [u32; 3],
    },
    FlashAttentionV1MaxFlatWorkgroupSizeMismatch {
        kernel: String,
        actual: u32,
        expected: u32,
    },
    FlashAttentionV1MetadataWavefrontSizeMismatch {
        kernel: String,
        actual: u32,
        expected: u32,
    },
    FlashAttentionV1DescriptorWavefrontSizeMismatch {
        kernel: String,
        actual: u32,
        expected: u32,
    },
    Wave64CollectivesV1RequiredWorkgroupSizeMismatch {
        kernel: String,
        actual: Option<[u32; 3]>,
        expected: [u32; 3],
    },
    Wave64CollectivesV1MaxFlatWorkgroupSizeMismatch {
        kernel: String,
        actual: u32,
        expected: u32,
    },
    Wave64CollectivesV1MetadataWavefrontSizeMismatch {
        kernel: String,
        actual: u32,
        expected: u32,
    },
    Wave64CollectivesV1DescriptorWavefrontSizeMismatch {
        kernel: String,
        actual: u32,
        expected: u32,
    },
    WorkgroupSyncV1RequiredWorkgroupSizeMismatch {
        kernel: String,
        actual: Option<[u32; 3]>,
        expected: [u32; 3],
    },
    WorkgroupSyncV1MaxFlatWorkgroupSizeMismatch {
        kernel: String,
        actual: u32,
        expected: u32,
    },
    WorkgroupSyncV1MetadataWavefrontSizeMismatch {
        kernel: String,
        actual: u32,
        expected: u32,
    },
    WorkgroupSyncV1DescriptorWavefrontSizeMismatch {
        kernel: String,
        actual: u32,
        expected: u32,
    },
}

impl fmt::Display for WorkerV2RawHsacoInspectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LineageMismatch(field) => {
                write!(formatter, "Worker V2 lineage mismatch: {field}")
            }
            Self::UnsupportedTarget(target) => {
                write!(
                    formatter,
                    "raw Worker V2 inspection supports only gfx942, found {target}"
                )
            }
            Self::LinkPolicy => formatter.write_str("retained link policy is invalid"),
            Self::HsacoBinding(error) => write!(formatter, "raw HSACO binding failed: {error}"),
            Self::TargetMismatch { expected, actual } => {
                write!(
                    formatter,
                    "raw HSACO target mismatch: expected {expected}, found {actual}"
                )
            }
            Self::CodeObjectVersionMismatch { expected, actual } => write!(
                formatter,
                "raw HSACO code-object version mismatch: expected {expected:?}, found {actual:?}"
            ),
            Self::MissingKernel => formatter.write_str("raw HSACO contains no kernel metadata"),
            Self::KernelEntryRoleMismatch => {
                formatter.write_str("raw HSACO kernel entries differ from retained manifest roles")
            }
            Self::KernelDescriptorRoleMismatch => formatter
                .write_str("raw HSACO kernel descriptors differ from retained manifest roles"),
            Self::CompilerEnvelopeImportRoleMismatch => formatter.write_str(
                "retained compiler envelope imports differ from retained manifest roles",
            ),
            Self::CompilerEnvelopeExportRoleMismatch => formatter.write_str(
                "retained compiler envelope exports differ from retained manifest roles",
            ),
            Self::DefinedSymbolInspection => {
                formatter.write_str("failed to inspect raw HSACO static symbols")
            }
            Self::DefinedSymbolClosureMismatch { expected, actual } => write!(
                formatter,
                "defined raw HSACO symbols mismatch: expected {expected:?}, found {actual:?}"
            ),
            Self::RequiredWorkgroupSizeMismatch { kernel, actual } => write!(
                formatter,
                "kernel {kernel} requires {actual:?}, expected {REQUIRED_WORKGROUP_SIZE:?}"
            ),
            Self::MaxFlatWorkgroupSizeMismatch { kernel, actual } => write!(
                formatter,
                "kernel {kernel} max flat workgroup is {actual}, expected {REQUIRED_MAX_FLAT_WORKGROUP_SIZE}"
            ),
            Self::MetadataWavefrontSizeMismatch { kernel, actual } => write!(
                formatter,
                "kernel {kernel} metadata wavefront is {actual}, expected {REQUIRED_WAVEFRONT_SIZE}"
            ),
            Self::DescriptorWavefrontSizeMismatch { kernel, actual } => write!(
                formatter,
                "kernel {kernel} descriptor wavefront is {actual}, expected {REQUIRED_WAVEFRONT_SIZE}"
            ),
            Self::ProductionV1RequiredWorkgroupSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "production-v1 kernel {kernel} requires {actual:?}, expected {expected:?}"
            ),
            Self::ProductionV1MaxFlatWorkgroupSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "production-v1 kernel {kernel} max flat workgroup is {actual}, expected {expected}"
            ),
            Self::ProductionV1MetadataWavefrontSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "production-v1 kernel {kernel} metadata wavefront is {actual}, expected {expected}"
            ),
            Self::ProductionV1DescriptorWavefrontSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "production-v1 kernel {kernel} descriptor wavefront is {actual}, expected {expected}"
            ),
            Self::StrictV3DescriptorLaunchContract(field) => {
                write!(
                    formatter,
                    "strict V3 descriptor launch contract rejected {field}"
                )
            }
            Self::TiledGemmV1RequiredWorkgroupSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "tiled GEMM V1 kernel {kernel} requires {actual:?}, expected {expected:?}"
            ),
            Self::TiledGemmV1MaxFlatWorkgroupSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "tiled GEMM V1 kernel {kernel} max flat workgroup is {actual}, expected {expected}"
            ),
            Self::TiledGemmV1MetadataWavefrontSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "tiled GEMM V1 kernel {kernel} metadata wavefront is {actual}, expected {expected}"
            ),
            Self::TiledGemmV1DescriptorWavefrontSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "tiled GEMM V1 kernel {kernel} descriptor wavefront is {actual}, expected {expected}"
            ),
            Self::GeneralGemmV1RequiredWorkgroupSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "general GEMM V1 kernel {kernel} requires {actual:?}, expected {expected:?}"
            ),
            Self::GeneralGemmV1MaxFlatWorkgroupSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "general GEMM V1 kernel {kernel} max flat workgroup is {actual}, expected {expected}"
            ),
            Self::GeneralGemmV1MetadataWavefrontSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "general GEMM V1 kernel {kernel} metadata wavefront is {actual}, expected {expected}"
            ),
            Self::GeneralGemmV1DescriptorWavefrontSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "general GEMM V1 kernel {kernel} descriptor wavefront is {actual}, expected {expected}"
            ),
            Self::RowSoftmaxV1RequiredWorkgroupSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "row-softmax V1 kernel {kernel} requires {actual:?}, expected {expected:?}"
            ),
            Self::RowSoftmaxV1MaxFlatWorkgroupSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "row-softmax V1 kernel {kernel} max flat workgroup is {actual}, expected {expected}"
            ),
            Self::RowSoftmaxV1MetadataWavefrontSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "row-softmax V1 kernel {kernel} metadata wavefront is {actual}, expected {expected}"
            ),
            Self::RowSoftmaxV1DescriptorWavefrontSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "row-softmax V1 kernel {kernel} descriptor wavefront is {actual}, expected {expected}"
            ),
            Self::FlashAttentionV1RequiredWorkgroupSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "FlashAttention V1 kernel {kernel} requires {actual:?}, expected {expected:?}"
            ),
            Self::FlashAttentionV1MaxFlatWorkgroupSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "FlashAttention V1 kernel {kernel} max flat workgroup is {actual}, expected {expected}"
            ),
            Self::FlashAttentionV1MetadataWavefrontSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "FlashAttention V1 kernel {kernel} metadata wavefront is {actual}, expected {expected}"
            ),
            Self::FlashAttentionV1DescriptorWavefrontSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "FlashAttention V1 kernel {kernel} descriptor wavefront is {actual}, expected {expected}"
            ),
            Self::Wave64CollectivesV1RequiredWorkgroupSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "Wave64 collectives V1 kernel {kernel} requires {actual:?}, expected {expected:?}"
            ),
            Self::Wave64CollectivesV1MaxFlatWorkgroupSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "Wave64 collectives V1 kernel {kernel} max flat workgroup is {actual}, expected {expected}"
            ),
            Self::Wave64CollectivesV1MetadataWavefrontSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "Wave64 collectives V1 kernel {kernel} metadata wavefront is {actual}, expected {expected}"
            ),
            Self::Wave64CollectivesV1DescriptorWavefrontSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "Wave64 collectives V1 kernel {kernel} descriptor wavefront is {actual}, expected {expected}"
            ),
            Self::WorkgroupSyncV1RequiredWorkgroupSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "workgroup-sync V1 kernel {kernel} requires {actual:?}, expected {expected:?}"
            ),
            Self::WorkgroupSyncV1MaxFlatWorkgroupSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "workgroup-sync V1 kernel {kernel} max flat workgroup is {actual}, expected {expected}"
            ),
            Self::WorkgroupSyncV1MetadataWavefrontSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "workgroup-sync V1 kernel {kernel} metadata wavefront is {actual}, expected {expected}"
            ),
            Self::WorkgroupSyncV1DescriptorWavefrontSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "workgroup-sync V1 kernel {kernel} descriptor wavefront is {actual}, expected {expected}"
            ),
        }
    }
}

impl Error for WorkerV2RawHsacoInspectionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::HsacoBinding(error) => Some(error),
            _ => None,
        }
    }
}

/// Consumes sealed first-build evidence and independently inspects its exact raw HSACO output.
///
/// No target, symbol, compiler-contract, or launch policy is accepted from the caller. Available
/// symbol roles are recovered from the exact manifest retained in the first-build evidence.
pub fn inspect_worker_v2_raw_hsaco_v1(
    source: InertFirstBuildWorkerV2EvidenceV1,
) -> Result<InspectedRawWorkerV2HsacoV1, WorkerV2RawHsacoInspectionError> {
    inspect_worker_v2_raw_hsaco_with_launch_v1(
        source,
        WorkerV2RawLaunchContractV1::GFX942_G1,
        WorkerV2RawLaunchDiagnosticProfileV1::LegacyGfx942G1,
    )
}

/// Consumes sealed production-v1 evidence under its compiler-owned wave64 launch contract.
///
/// This route is closed over the current production compiler profile. The caller cannot supply or
/// weaken its required 64-thread workgroup or wavefront properties.
pub fn inspect_production_v1_worker_v2_raw_hsaco_v1(
    source: InertFirstBuildWorkerV2EvidenceV1,
) -> Result<InspectedRawWorkerV2HsacoV1, WorkerV2RawHsacoInspectionError> {
    inspect_worker_v2_raw_hsaco_with_launch_v1(
        source,
        WorkerV2RawLaunchContractV1::PRODUCTION_V1,
        WorkerV2RawLaunchDiagnosticProfileV1::ProductionV1,
    )
}

/// Consumes closure-protected first-build evidence and inspects its exact raw HSACO output.
///
/// This route retains the exact V2 handoff identity and full compiler closure. It is side-by-side
/// with [`inspect_worker_v2_raw_hsaco_v1`] and never converts protected lineage to V1.
pub fn inspect_protected_worker_v2_raw_hsaco_v1(
    source: InertProtectedFirstBuildWorkerV2EvidenceV1,
) -> Result<InspectedProtectedRawWorkerV2HsacoV1, WorkerV2RawHsacoInspectionError> {
    inspect_protected_worker_v2_raw_hsaco_with_launch_v1(
        source,
        WorkerV2RawLaunchContractV1::GFX942_G1,
        WorkerV2RawLaunchDiagnosticProfileV1::LegacyGfx942G1,
    )
}

/// Consumes closure-protected production-v1 evidence under the fixed wave64 launch contract.
///
/// The caller cannot supply or weaken the required 64-thread workgroup or wavefront properties.
pub fn inspect_protected_production_v1_worker_v2_raw_hsaco_v1(
    source: InertProtectedFirstBuildWorkerV2EvidenceV1,
) -> Result<InspectedProtectedRawWorkerV2HsacoV1, WorkerV2RawHsacoInspectionError> {
    inspect_protected_worker_v2_raw_hsaco_with_launch_v1(
        source,
        WorkerV2RawLaunchContractV1::PRODUCTION_V1,
        WorkerV2RawLaunchDiagnosticProfileV1::ProductionV1,
    )
}

/// Consumes native strict-V3 first-build evidence under its descriptor-bound production contract.
///
/// The complete V3 transaction and outer semantic handoff remain owned by the result. This route
/// neither constructs protected V2 evidence nor falls back to a V1/V2 admission entry. When the
/// raw artifact has a canonical descriptor table, its exact physically cross-checked launch facts
/// define the inspection policy; callers cannot inject or weaken those facts.
pub fn inspect_protected_production_v1_worker_v3_raw_hsaco_v1(
    source: InertProtectedFirstBuildWorkerV3EvidenceV1,
) -> Result<InspectedProtectedRawWorkerV3HsacoV1, WorkerV2RawHsacoInspectionError> {
    validate_protected_v3_lineage(&source)?;
    let launch = strict_v3_launch_contract(&source)?;
    let raw = inspect_worker_v2_raw_hsaco_shared_v1(
        &source,
        launch,
        WorkerV2RawLaunchDiagnosticProfileV1::ProductionV1,
    )?;
    let response_identity =
        calculate_response_identity(source.exact_replay().response().canonical_bytes());
    let identity = calculate_protected_v3_inspection_identity(&source, &raw, response_identity);
    Ok(InspectedProtectedRawWorkerV3HsacoV1 {
        identity,
        response_identity,
        descriptor_section: raw.descriptor_section,
        descriptor_observation_preimage: raw.descriptor_observation_preimage,
        abi_observation_preimage: raw.abi_observation_preimage,
        resource_observation_preimage: raw.resource_observation_preimage,
        policy: raw.policy,
        source,
    })
}

fn strict_v3_launch_contract(
    source: &InertProtectedFirstBuildWorkerV3EvidenceV1,
) -> Result<WorkerV2RawLaunchContractV1, WorkerV2RawHsacoInspectionError> {
    let inspection = match crate::inspect_unfinalized(source.output_bytes()) {
        Ok(inspection) => inspection,
        // Preserve the existing descriptive inspection stage for legacy tests that deliberately
        // omit compiler descriptor-source evidence. Finalization still rejects those artifacts.
        Err(FinalizationError::MissingDescriptorSection) => {
            return Ok(WorkerV2RawLaunchContractV1::PRODUCTION_V1);
        }
        Err(_) => {
            return Err(
                WorkerV2RawHsacoInspectionError::StrictV3DescriptorLaunchContract(
                    "embedded descriptor inspection",
                ),
            );
        }
    };
    let mut kernels = inspection.descriptor_table().kernels().iter();
    let first = kernels
        .next()
        .ok_or(WorkerV2RawHsacoInspectionError::StrictV3DescriptorLaunchContract("kernel set"))?;
    let expected = strict_v3_kernel_launch_contract(first)?;
    for kernel in kernels {
        if strict_v3_kernel_launch_contract(kernel)? != expected {
            return Err(
                WorkerV2RawHsacoInspectionError::StrictV3DescriptorLaunchContract(
                    "heterogeneous per-kernel launch policy",
                ),
            );
        }
    }
    Ok(expected)
}

fn strict_v3_kernel_launch_contract(
    kernel: &KernelDescriptorV1,
) -> Result<WorkerV2RawLaunchContractV1, WorkerV2RawHsacoInspectionError> {
    let block = match kernel.launch().block_size() {
        BlockSizeV1::Exact(block) => block,
        BlockSizeV1::Any | BlockSizeV1::AtMost(_) => {
            return Err(
                WorkerV2RawHsacoInspectionError::StrictV3DescriptorLaunchContract(
                    "non-exact block size",
                ),
            );
        }
    };
    Ok(WorkerV2RawLaunchContractV1 {
        required_workgroup_size: [block.x(), block.y(), block.z()],
        max_flat_workgroup_size: kernel.launch().max_flat_workgroup_size(),
        wavefront_size: REQUIRED_WAVEFRONT_SIZE,
    })
}

/// Consumes sealed Worker V2 evidence under the exact general-GEMM V1 launch contract.
///
/// The caller cannot supply or weaken the required 64-thread wave64 launch profile.
pub fn inspect_general_gemm_worker_v2_raw_hsaco_v1(
    source: InertFirstBuildWorkerV2EvidenceV1,
) -> Result<InspectedRawWorkerV2HsacoV1, WorkerV2RawHsacoInspectionError> {
    inspect_worker_v2_raw_hsaco_with_launch_v1(
        source,
        WorkerV2RawLaunchContractV1::GENERAL_GEMM_V1,
        WorkerV2RawLaunchDiagnosticProfileV1::GeneralGemmV1,
    )
}

pub(crate) fn inspect_worker_v2_raw_hsaco_with_launch_v1(
    source: InertFirstBuildWorkerV2EvidenceV1,
    launch: WorkerV2RawLaunchContractV1,
    diagnostic_profile: WorkerV2RawLaunchDiagnosticProfileV1,
) -> Result<InspectedRawWorkerV2HsacoV1, WorkerV2RawHsacoInspectionError> {
    validate_lineage(&source)?;
    let raw = inspect_worker_v2_raw_hsaco_shared_v1(&source, launch, diagnostic_profile)?;
    let response_identity =
        calculate_response_identity(source.authorized().response().canonical_bytes());
    let identity = calculate_inspection_identity(
        &source,
        &raw.policy,
        response_identity,
        raw.descriptor_section,
    );
    Ok(InspectedRawWorkerV2HsacoV1 {
        identity,
        response_identity,
        descriptor_section: raw.descriptor_section,
        policy: raw.policy,
        source,
    })
}

fn inspect_protected_worker_v2_raw_hsaco_with_launch_v1(
    source: InertProtectedFirstBuildWorkerV2EvidenceV1,
    launch: WorkerV2RawLaunchContractV1,
    diagnostic_profile: WorkerV2RawLaunchDiagnosticProfileV1,
) -> Result<InspectedProtectedRawWorkerV2HsacoV1, WorkerV2RawHsacoInspectionError> {
    validate_protected_lineage(&source)?;
    let raw = inspect_worker_v2_raw_hsaco_shared_v1(&source, launch, diagnostic_profile)?;
    let response_identity =
        calculate_response_identity(source.authorized().response().canonical_bytes());
    let identity = calculate_protected_inspection_identity(&source, &raw, response_identity);
    Ok(InspectedProtectedRawWorkerV2HsacoV1 {
        identity,
        response_identity,
        descriptor_section: raw.descriptor_section,
        descriptor_observation_preimage: raw.descriptor_observation_preimage,
        abi_observation_preimage: raw.abi_observation_preimage,
        resource_observation_preimage: raw.resource_observation_preimage,
        policy: raw.policy,
        source,
    })
}

trait RawWorkerV2HsacoSourceV1 {
    fn plan(&self) -> &MultiInputLinkPlanV1;
    fn symbol_manifest(&self) -> &CompilerModuleSymbolManifestV1;
    fn compiler_envelope_identity(&self) -> CompilerFfiEnvelopeIdentityV1;
    fn output_identity(&self) -> ContentIdentityV1;
    fn output_bytes(&self) -> &[u8];
}

impl RawWorkerV2HsacoSourceV1 for InertFirstBuildWorkerV2EvidenceV1 {
    fn plan(&self) -> &MultiInputLinkPlanV1 {
        self.plan()
    }

    fn symbol_manifest(&self) -> &CompilerModuleSymbolManifestV1 {
        self.symbol_manifest()
    }

    fn compiler_envelope_identity(&self) -> CompilerFfiEnvelopeIdentityV1 {
        self.compiler_envelope_identity()
    }

    fn output_identity(&self) -> ContentIdentityV1 {
        self.output_identity()
    }

    fn output_bytes(&self) -> &[u8] {
        self.output_bytes()
    }
}

impl RawWorkerV2HsacoSourceV1 for InertProtectedFirstBuildWorkerV2EvidenceV1 {
    fn plan(&self) -> &MultiInputLinkPlanV1 {
        self.plan()
    }

    fn symbol_manifest(&self) -> &CompilerModuleSymbolManifestV1 {
        self.symbol_manifest()
    }

    fn compiler_envelope_identity(&self) -> CompilerFfiEnvelopeIdentityV1 {
        self.compiler_envelope_identity()
    }

    fn output_identity(&self) -> ContentIdentityV1 {
        self.output_identity()
    }

    fn output_bytes(&self) -> &[u8] {
        self.output_bytes()
    }
}

impl RawWorkerV2HsacoSourceV1 for InertProtectedFirstBuildWorkerV3EvidenceV1 {
    fn plan(&self) -> &MultiInputLinkPlanV1 {
        self.plan()
    }

    fn symbol_manifest(&self) -> &CompilerModuleSymbolManifestV1 {
        self.handoff().module_handoff().symbol_manifest()
    }

    fn compiler_envelope_identity(&self) -> CompilerFfiEnvelopeIdentityV1 {
        self.handoff().module_handoff().envelope().identity()
    }

    fn output_identity(&self) -> ContentIdentityV1 {
        self.output_identity()
    }

    fn output_bytes(&self) -> &[u8] {
        self.output_bytes()
    }
}

pub(crate) struct SharedRawWorkerV2HsacoInspectionV1 {
    pub(crate) descriptor_section: CanonicalDescriptorSectionObservationV1,
    pub(crate) policy: WorkerV2RawHsacoPolicyV1,
    pub(crate) descriptor_identity: [u8; 32],
    pub(crate) abi_identity: [u8; 32],
    pub(crate) resource_identity: [u8; 32],
    pub(crate) descriptor_observation_preimage: Vec<u8>,
    pub(crate) abi_observation_preimage: Vec<u8>,
    pub(crate) resource_observation_preimage: Vec<u8>,
}

fn inspect_worker_v2_raw_hsaco_shared_v1(
    source: &impl RawWorkerV2HsacoSourceV1,
    launch: WorkerV2RawLaunchContractV1,
    diagnostic_profile: WorkerV2RawLaunchDiagnosticProfileV1,
) -> Result<SharedRawWorkerV2HsacoInspectionV1, WorkerV2RawHsacoInspectionError> {
    let target = source.plan().target();
    if target.as_amd_target_id().processor() != EXPECTED_PROCESSOR {
        return Err(WorkerV2RawHsacoInspectionError::UnsupportedTarget(
            target.to_string(),
        ));
    }
    let (code_object_version, _) = decode_link_options(source.plan().options())
        .map_err(|_| WorkerV2RawHsacoInspectionError::LinkPolicy)?;
    inspect_worker_v2_raw_hsaco_preimage_v1(
        target,
        code_object_version,
        source.symbol_manifest().clone(),
        source.compiler_envelope_identity(),
        source.output_identity(),
        source.output_bytes(),
        launch,
        diagnostic_profile,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn inspect_worker_v2_raw_hsaco_preimage_v1(
    target: DeviceTargetV1,
    code_object_version: CodeObjectVersion,
    symbol_manifest: CompilerModuleSymbolManifestV1,
    compiler_envelope: CompilerFfiEnvelopeIdentityV1,
    output_identity: ContentIdentityV1,
    exact_bytes: &[u8],
    launch: WorkerV2RawLaunchContractV1,
    diagnostic_profile: WorkerV2RawLaunchDiagnosticProfileV1,
) -> Result<SharedRawWorkerV2HsacoInspectionV1, WorkerV2RawHsacoInspectionError> {
    if target.as_amd_target_id().processor() != EXPECTED_PROCESSOR {
        return Err(WorkerV2RawHsacoInspectionError::UnsupportedTarget(
            target.to_string(),
        ));
    }
    if !output_identity.matches(exact_bytes) {
        return Err(WorkerV2RawHsacoInspectionError::LineageMismatch(
            "linked output identity",
        ));
    }

    let inspected = inspect_and_bind_kernel_descriptors(exact_bytes)
        .map_err(WorkerV2RawHsacoInspectionError::HsacoBinding)?;
    let metadata = inspected.inspection();
    if metadata.target() != target.as_amd_target_id() {
        return Err(WorkerV2RawHsacoInspectionError::TargetMismatch {
            expected: target.to_string(),
            actual: metadata.target().to_string(),
        });
    }
    let actual_code_object_version = map_code_object_version(metadata.code_object_version());
    if actual_code_object_version != code_object_version {
        return Err(WorkerV2RawHsacoInspectionError::CodeObjectVersionMismatch {
            expected: code_object_version,
            actual: actual_code_object_version,
        });
    }
    if metadata.kernels().is_empty() {
        return Err(WorkerV2RawHsacoInspectionError::MissingKernel);
    }

    let mut observed_kernels: Vec<_> = metadata
        .kernels()
        .iter()
        .map(|kernel| ObservedWorkerV2KernelSymbolsV1 {
            entry: kernel.name().to_owned(),
            descriptor: kernel.symbol().to_owned(),
        })
        .collect();
    observed_kernels.sort();
    let observed_entries: Vec<&str> = observed_kernels
        .iter()
        .map(|kernel| kernel.entry())
        .collect();
    let manifest_entries: Vec<&str> = symbol_manifest
        .symbols(CompilerModuleSymbolRoleV1::KernelEntry)
        .collect();
    if observed_entries != manifest_entries {
        return Err(WorkerV2RawHsacoInspectionError::KernelEntryRoleMismatch);
    }
    let mut observed_descriptors: Vec<&str> = observed_kernels
        .iter()
        .map(|kernel| kernel.descriptor())
        .collect();
    observed_descriptors.sort_unstable();
    let manifest_descriptors: Vec<&str> = symbol_manifest
        .symbols(CompilerModuleSymbolRoleV1::KernelDescriptor)
        .collect();
    if observed_descriptors != manifest_descriptors {
        return Err(WorkerV2RawHsacoInspectionError::KernelDescriptorRoleMismatch);
    }

    let expected_symbols = expected_defined_symbols(&symbol_manifest);
    let actual_defined_symbols = inspect_defined_global_symbols(exact_bytes)?;
    if actual_defined_symbols != expected_symbols {
        return Err(
            WorkerV2RawHsacoInspectionError::DefinedSymbolClosureMismatch {
                expected: expected_symbols,
                actual: actual_defined_symbols,
            },
        );
    }
    for (kernel, binding) in metadata.kernels().iter().zip(inspected.bindings()) {
        if kernel.required_workgroup_size() != Some(launch.required_workgroup_size()) {
            return Err(required_workgroup_size_mismatch(
                launch,
                diagnostic_profile,
                kernel.name(),
                kernel.required_workgroup_size(),
            ));
        }
        if kernel.max_flat_workgroup_size() != launch.max_flat_workgroup_size() {
            return Err(max_flat_workgroup_size_mismatch(
                launch,
                diagnostic_profile,
                kernel.name(),
                kernel.max_flat_workgroup_size(),
            ));
        }
        if kernel.wavefront_size() != launch.wavefront_size() {
            return Err(metadata_wavefront_size_mismatch(
                launch,
                diagnostic_profile,
                kernel.name(),
                kernel.wavefront_size(),
            ));
        }
        let descriptor_wavefront = binding.descriptor().wavefront_size();
        if descriptor_wavefront != launch.wavefront_size() {
            return Err(descriptor_wavefront_size_mismatch(
                launch,
                diagnostic_profile,
                kernel.name(),
                descriptor_wavefront,
            ));
        }
    }

    let descriptor_section = inspect_descriptor_section(exact_bytes)?;
    let descriptor_observation_preimage = encode_descriptor_observation_preimage(&inspected);
    let abi_observation_preimage = encode_abi_observation_preimage(&inspected);
    let resource_observation_preimage = encode_resource_observation_preimage(&inspected);
    let descriptor_identity = calculate_observation_identity(
        PROTECTED_DESCRIPTOR_DOMAIN_V1,
        &descriptor_observation_preimage,
    );
    let abi_identity =
        calculate_observation_identity(PROTECTED_ABI_DOMAIN_V1, &abi_observation_preimage);
    let resource_identity = calculate_observation_identity(
        PROTECTED_RESOURCES_DOMAIN_V1,
        &resource_observation_preimage,
    );
    let expected_defined_symbols = expected_defined_symbols(&symbol_manifest);
    let policy = WorkerV2RawHsacoPolicyV1 {
        identity: calculate_policy_identity(
            target,
            code_object_version,
            compiler_envelope,
            &symbol_manifest,
            &observed_kernels,
            &expected_defined_symbols,
            launch,
        ),
        target,
        code_object_version,
        compiler_envelope,
        symbol_manifest,
        observed_kernels,
        expected_defined_symbols,
        launch,
    };
    Ok(SharedRawWorkerV2HsacoInspectionV1 {
        descriptor_section,
        policy,
        descriptor_identity,
        abi_identity,
        resource_identity,
        descriptor_observation_preimage,
        abi_observation_preimage,
        resource_observation_preimage,
    })
}

fn required_workgroup_size_mismatch(
    launch: WorkerV2RawLaunchContractV1,
    diagnostic_profile: WorkerV2RawLaunchDiagnosticProfileV1,
    kernel: &str,
    actual: Option<[u32; 3]>,
) -> WorkerV2RawHsacoInspectionError {
    match diagnostic_profile {
        WorkerV2RawLaunchDiagnosticProfileV1::LegacyGfx942G1 => {
            WorkerV2RawHsacoInspectionError::RequiredWorkgroupSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::ProductionV1 => {
            WorkerV2RawHsacoInspectionError::ProductionV1RequiredWorkgroupSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.required_workgroup_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::TiledGemmV1 => {
            WorkerV2RawHsacoInspectionError::TiledGemmV1RequiredWorkgroupSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.required_workgroup_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::GeneralGemmV1 => {
            WorkerV2RawHsacoInspectionError::GeneralGemmV1RequiredWorkgroupSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.required_workgroup_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::RowSoftmaxV1 => {
            WorkerV2RawHsacoInspectionError::RowSoftmaxV1RequiredWorkgroupSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.required_workgroup_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::FlashAttentionV1 => {
            WorkerV2RawHsacoInspectionError::FlashAttentionV1RequiredWorkgroupSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.required_workgroup_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::Wave64CollectivesV1 => {
            WorkerV2RawHsacoInspectionError::Wave64CollectivesV1RequiredWorkgroupSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.required_workgroup_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::WorkgroupSyncV1 => {
            WorkerV2RawHsacoInspectionError::WorkgroupSyncV1RequiredWorkgroupSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.required_workgroup_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::MoeTop2V1 => {
            WorkerV2RawHsacoInspectionError::RequiredWorkgroupSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
            }
        }
    }
}

fn max_flat_workgroup_size_mismatch(
    launch: WorkerV2RawLaunchContractV1,
    diagnostic_profile: WorkerV2RawLaunchDiagnosticProfileV1,
    kernel: &str,
    actual: u32,
) -> WorkerV2RawHsacoInspectionError {
    match diagnostic_profile {
        WorkerV2RawLaunchDiagnosticProfileV1::LegacyGfx942G1 => {
            WorkerV2RawHsacoInspectionError::MaxFlatWorkgroupSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::ProductionV1 => {
            WorkerV2RawHsacoInspectionError::ProductionV1MaxFlatWorkgroupSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.max_flat_workgroup_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::TiledGemmV1 => {
            WorkerV2RawHsacoInspectionError::TiledGemmV1MaxFlatWorkgroupSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.max_flat_workgroup_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::GeneralGemmV1 => {
            WorkerV2RawHsacoInspectionError::GeneralGemmV1MaxFlatWorkgroupSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.max_flat_workgroup_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::RowSoftmaxV1 => {
            WorkerV2RawHsacoInspectionError::RowSoftmaxV1MaxFlatWorkgroupSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.max_flat_workgroup_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::FlashAttentionV1 => {
            WorkerV2RawHsacoInspectionError::FlashAttentionV1MaxFlatWorkgroupSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.max_flat_workgroup_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::Wave64CollectivesV1 => {
            WorkerV2RawHsacoInspectionError::Wave64CollectivesV1MaxFlatWorkgroupSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.max_flat_workgroup_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::WorkgroupSyncV1 => {
            WorkerV2RawHsacoInspectionError::WorkgroupSyncV1MaxFlatWorkgroupSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.max_flat_workgroup_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::MoeTop2V1 => {
            WorkerV2RawHsacoInspectionError::MaxFlatWorkgroupSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
            }
        }
    }
}

fn metadata_wavefront_size_mismatch(
    launch: WorkerV2RawLaunchContractV1,
    diagnostic_profile: WorkerV2RawLaunchDiagnosticProfileV1,
    kernel: &str,
    actual: u32,
) -> WorkerV2RawHsacoInspectionError {
    match diagnostic_profile {
        WorkerV2RawLaunchDiagnosticProfileV1::LegacyGfx942G1 => {
            WorkerV2RawHsacoInspectionError::MetadataWavefrontSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::ProductionV1 => {
            WorkerV2RawHsacoInspectionError::ProductionV1MetadataWavefrontSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.wavefront_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::TiledGemmV1 => {
            WorkerV2RawHsacoInspectionError::TiledGemmV1MetadataWavefrontSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.wavefront_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::GeneralGemmV1 => {
            WorkerV2RawHsacoInspectionError::GeneralGemmV1MetadataWavefrontSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.wavefront_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::RowSoftmaxV1 => {
            WorkerV2RawHsacoInspectionError::RowSoftmaxV1MetadataWavefrontSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.wavefront_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::FlashAttentionV1 => {
            WorkerV2RawHsacoInspectionError::FlashAttentionV1MetadataWavefrontSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.wavefront_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::Wave64CollectivesV1 => {
            WorkerV2RawHsacoInspectionError::Wave64CollectivesV1MetadataWavefrontSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.wavefront_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::WorkgroupSyncV1 => {
            WorkerV2RawHsacoInspectionError::WorkgroupSyncV1MetadataWavefrontSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.wavefront_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::MoeTop2V1 => {
            WorkerV2RawHsacoInspectionError::MetadataWavefrontSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
            }
        }
    }
}

fn descriptor_wavefront_size_mismatch(
    launch: WorkerV2RawLaunchContractV1,
    diagnostic_profile: WorkerV2RawLaunchDiagnosticProfileV1,
    kernel: &str,
    actual: u32,
) -> WorkerV2RawHsacoInspectionError {
    match diagnostic_profile {
        WorkerV2RawLaunchDiagnosticProfileV1::LegacyGfx942G1 => {
            WorkerV2RawHsacoInspectionError::DescriptorWavefrontSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::ProductionV1 => {
            WorkerV2RawHsacoInspectionError::ProductionV1DescriptorWavefrontSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.wavefront_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::TiledGemmV1 => {
            WorkerV2RawHsacoInspectionError::TiledGemmV1DescriptorWavefrontSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.wavefront_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::GeneralGemmV1 => {
            WorkerV2RawHsacoInspectionError::GeneralGemmV1DescriptorWavefrontSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.wavefront_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::RowSoftmaxV1 => {
            WorkerV2RawHsacoInspectionError::RowSoftmaxV1DescriptorWavefrontSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.wavefront_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::FlashAttentionV1 => {
            WorkerV2RawHsacoInspectionError::FlashAttentionV1DescriptorWavefrontSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.wavefront_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::Wave64CollectivesV1 => {
            WorkerV2RawHsacoInspectionError::Wave64CollectivesV1DescriptorWavefrontSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.wavefront_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::WorkgroupSyncV1 => {
            WorkerV2RawHsacoInspectionError::WorkgroupSyncV1DescriptorWavefrontSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.wavefront_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::MoeTop2V1 => {
            WorkerV2RawHsacoInspectionError::DescriptorWavefrontSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
            }
        }
    }
}

fn validate_lineage(
    source: &InertFirstBuildWorkerV2EvidenceV1,
) -> Result<(), WorkerV2RawHsacoInspectionError> {
    let authorized = source.authorized();
    let response = authorized.response();
    if source.attempt() != authorized.attempt() {
        return Err(WorkerV2RawHsacoInspectionError::LineageMismatch(
            "build attempt",
        ));
    }
    if source.handoff_identity() != authorized.handoff_identity() {
        return Err(WorkerV2RawHsacoInspectionError::LineageMismatch(
            "handoff identity",
        ));
    }
    if source.compiler_envelope().target().to_string() != source.plan().target().to_string()
        || map_compiler_code_object_version(source.compiler_envelope().code_object_version())
            != decode_link_options(source.plan().options())
                .map_err(|_| WorkerV2RawHsacoInspectionError::LinkPolicy)?
                .0
    {
        return Err(WorkerV2RawHsacoInspectionError::LineageMismatch(
            "compiler envelope target/code-object version",
        ));
    }
    if response.compiler_envelope_identity().as_bytes()
        != source.compiler_envelope_identity().as_bytes()
    {
        return Err(WorkerV2RawHsacoInspectionError::LineageMismatch(
            "compiler envelope identity",
        ));
    }
    let directional = source.compiler_envelope().directional_symbols();
    if !source
        .symbol_manifest()
        .symbols(CompilerModuleSymbolRoleV1::UnresolvedExternalImport)
        .eq(directional.imports())
    {
        return Err(WorkerV2RawHsacoInspectionError::CompilerEnvelopeImportRoleMismatch);
    }
    if !source
        .symbol_manifest()
        .symbols(CompilerModuleSymbolRoleV1::DeviceFfiExport)
        .eq(directional.exports())
    {
        return Err(WorkerV2RawHsacoInspectionError::CompilerEnvelopeExportRoleMismatch);
    }
    if source.worker_measurement().executable() != authorized.worker_executable()
        || source.worker_measurement().worker_build_identity() != response.worker_build_identity()
    {
        return Err(WorkerV2RawHsacoInspectionError::LineageMismatch(
            "worker measurement",
        ));
    }
    let output = response
        .output()
        .ok_or(WorkerV2RawHsacoInspectionError::LineageMismatch(
            "missing linked output",
        ))?;
    if output.identity() != source.output_identity()
        || output.request_identity() != response.request_identity()
        || output.compiler_envelope_identity() != response.compiler_envelope_identity()
    {
        return Err(WorkerV2RawHsacoInspectionError::LineageMismatch(
            "sealed request/response/output identity",
        ));
    }
    Ok(())
}

fn validate_protected_lineage(
    source: &InertProtectedFirstBuildWorkerV2EvidenceV1,
) -> Result<(), WorkerV2RawHsacoInspectionError> {
    let bootstrap = source.bootstrap();
    let authorized = source.authorized();
    for execution in [bootstrap, authorized] {
        if source.attempt() != execution.attempt() {
            return Err(WorkerV2RawHsacoInspectionError::LineageMismatch(
                "protected build attempt",
            ));
        }
        if source.slot() != execution.slot() {
            return Err(WorkerV2RawHsacoInspectionError::LineageMismatch(
                "protected handoff slot",
            ));
        }
        if source.handoff_identity() != execution.handoff_identity() {
            return Err(WorkerV2RawHsacoInspectionError::LineageMismatch(
                "protected V2 handoff identity",
            ));
        }
        if source.compiler_closure() != execution.compiler_closure() {
            return Err(WorkerV2RawHsacoInspectionError::LineageMismatch(
                "protected compiler closure",
            ));
        }
        if source.worker_measurement().executable() != execution.worker_executable()
            || source.worker_measurement().worker_build_identity()
                != execution.response().worker_build_identity()
        {
            return Err(WorkerV2RawHsacoInspectionError::LineageMismatch(
                "protected worker measurement",
            ));
        }
        if execution.response().compiler_envelope_identity().as_bytes()
            != source.compiler_envelope_identity().as_bytes()
        {
            return Err(WorkerV2RawHsacoInspectionError::LineageMismatch(
                "protected compiler envelope identity",
            ));
        }
    }

    if source.compiler_envelope().target().to_string() != source.plan().target().to_string()
        || map_compiler_code_object_version(source.compiler_envelope().code_object_version())
            != decode_link_options(source.plan().options())
                .map_err(|_| WorkerV2RawHsacoInspectionError::LinkPolicy)?
                .0
    {
        return Err(WorkerV2RawHsacoInspectionError::LineageMismatch(
            "protected compiler envelope target/code-object version",
        ));
    }
    let directional = source.compiler_envelope().directional_symbols();
    if !source
        .symbol_manifest()
        .symbols(CompilerModuleSymbolRoleV1::UnresolvedExternalImport)
        .eq(directional.imports())
    {
        return Err(WorkerV2RawHsacoInspectionError::CompilerEnvelopeImportRoleMismatch);
    }
    if !source
        .symbol_manifest()
        .symbols(CompilerModuleSymbolRoleV1::DeviceFfiExport)
        .eq(directional.exports())
    {
        return Err(WorkerV2RawHsacoInspectionError::CompilerEnvelopeExportRoleMismatch);
    }

    let bootstrap_response = bootstrap.response();
    let bootstrap_output =
        bootstrap_response
            .output()
            .ok_or(WorkerV2RawHsacoInspectionError::LineageMismatch(
                "missing protected bootstrap output",
            ))?;
    let authorized_response = authorized.response();
    let authorized_output =
        authorized_response
            .output()
            .ok_or(WorkerV2RawHsacoInspectionError::LineageMismatch(
                "missing protected linked output",
            ))?;
    for (response, output) in [
        (bootstrap_response, bootstrap_output),
        (authorized_response, authorized_output),
    ] {
        if output.identity() != source.output_identity()
            || output.request_identity() != response.request_identity()
            || output.compiler_envelope_identity() != response.compiler_envelope_identity()
        {
            return Err(WorkerV2RawHsacoInspectionError::LineageMismatch(
                "protected sealed request/response/output identity",
            ));
        }
    }
    if bootstrap_output.bytes() != authorized_output.bytes() {
        return Err(WorkerV2RawHsacoInspectionError::LineageMismatch(
            "protected reproducible output bytes",
        ));
    }
    Ok(())
}

fn validate_protected_v3_lineage(
    source: &InertProtectedFirstBuildWorkerV3EvidenceV1,
) -> Result<(), WorkerV2RawHsacoInspectionError> {
    let binding = source.binding();
    let expected = binding.expectation();
    let handoff = source.handoff();
    let capsule = handoff.capsule();
    let nested = handoff.module_handoff();
    let pair = handoff.pair_binding();

    if source.bootstrap().binding() != binding || source.exact_replay().binding() != binding {
        return Err(WorkerV2RawHsacoInspectionError::LineageMismatch(
            "strict V3 worker binding",
        ));
    }
    if expected.outer_handoff_identity() != handoff.identity()
        || !expected
            .outer_handoff_identity()
            .matches_canonical_bytes(handoff.canonical_bytes())
    {
        return Err(WorkerV2RawHsacoInspectionError::LineageMismatch(
            "strict V3 outer handoff",
        ));
    }

    let capsule_identity = capsule.identity();
    let pair_identity = pair.identity();
    if expected.capsule_sha256() != *capsule_identity.sha256()
        || expected.capsule_byte_len() != capsule_identity.byte_len()
        || expected.invocation_digest() != *capsule.invocation_digest().as_bytes()
        || expected.pair_binding_sha256() != *pair_identity.sha256()
        || expected.pair_binding_byte_len() != pair_identity.byte_len()
        || expected.nested_handoff_identity() != nested.identity()
        || pair.capsule_identity() != capsule_identity
        || pair.module_handoff_identity() != nested.identity()
        || expected.compiler_closure() != *capsule.compiler_closure()
        || expected.compiler_closure() != *capsule.invocation().compiler_closure()
    {
        return Err(WorkerV2RawHsacoInspectionError::LineageMismatch(
            "strict V3 semantic handoff association",
        ));
    }

    let final_receipt = capsule.receipts().final_compiler_module_commitment();
    let final_receipt_identity = final_receipt.identity();
    let final_commitment = InertFinalCompilerModuleCommitmentV3::decode(
        final_receipt.canonical_preimage(),
    )
    .map_err(|_| {
        WorkerV2RawHsacoInspectionError::LineageMismatch(
            "strict V3 final compiler-module commitment",
        )
    })?;
    let final_identity = final_commitment.identity();
    if expected.final_commitment_receipt_sha256() != *final_receipt_identity.sha256()
        || expected.final_commitment_receipt_byte_len() != final_receipt_identity.byte_len()
        || expected.final_commitment_sha256() != *final_identity.sha256()
        || expected.final_commitment_byte_len() != final_identity.byte_len()
        || !final_commitment.matches_handoff(nested)
    {
        return Err(WorkerV2RawHsacoInspectionError::LineageMismatch(
            "strict V3 final compiler-module commitment association",
        ));
    }

    if nested.target().to_string() != source.plan().target().to_string()
        || map_compiler_code_object_version(nested.code_object_version())
            != decode_link_options(source.plan().options())
                .map_err(|_| WorkerV2RawHsacoInspectionError::LinkPolicy)?
                .0
    {
        return Err(WorkerV2RawHsacoInspectionError::LineageMismatch(
            "strict V3 compiler envelope target/code-object version",
        ));
    }
    let directional = nested.envelope().directional_symbols();
    if !nested
        .symbol_manifest()
        .symbols(CompilerModuleSymbolRoleV1::UnresolvedExternalImport)
        .eq(directional.imports())
    {
        return Err(WorkerV2RawHsacoInspectionError::CompilerEnvelopeImportRoleMismatch);
    }
    if !nested
        .symbol_manifest()
        .symbols(CompilerModuleSymbolRoleV1::DeviceFfiExport)
        .eq(directional.exports())
    {
        return Err(WorkerV2RawHsacoInspectionError::CompilerEnvelopeExportRoleMismatch);
    }

    let expected_envelope = nested.envelope().identity();
    let bootstrap = source.bootstrap();
    let replay = source.exact_replay();
    for execution in [bootstrap, replay] {
        let response = execution.response();
        if source.worker_measurement().executable() != execution.worker_executable()
            || source.worker_measurement().worker_build_identity()
                != response.worker_build_identity()
        {
            return Err(WorkerV2RawHsacoInspectionError::LineageMismatch(
                "strict V3 worker measurement",
            ));
        }
        if response.compiler_envelope_identity().as_bytes() != expected_envelope.as_bytes() {
            return Err(WorkerV2RawHsacoInspectionError::LineageMismatch(
                "strict V3 compiler envelope identity",
            ));
        }
        let output = response
            .output()
            .ok_or(WorkerV2RawHsacoInspectionError::LineageMismatch(
                "missing strict V3 linked output",
            ))?;
        if output.identity() != source.output_identity()
            || output.request_identity() != response.request_identity()
            || output.compiler_envelope_identity() != response.compiler_envelope_identity()
        {
            return Err(WorkerV2RawHsacoInspectionError::LineageMismatch(
                "strict V3 sealed request/response/output identity",
            ));
        }
    }

    let bootstrap_output = bootstrap
        .response()
        .output()
        .expect("strict V3 bootstrap output checked above");
    let replay_output = replay
        .response()
        .output()
        .expect("strict V3 replay output checked above");
    if bootstrap_output.bytes() != replay_output.bytes()
        || replay_output.bytes() != source.output_bytes()
    {
        return Err(WorkerV2RawHsacoInspectionError::LineageMismatch(
            "strict V3 reproducible output bytes",
        ));
    }
    Ok(())
}

fn expected_defined_symbols(manifest: &CompilerModuleSymbolManifestV1) -> Vec<String> {
    let mut symbols = Vec::new();
    for role in [
        CompilerModuleSymbolRoleV1::KernelEntry,
        CompilerModuleSymbolRoleV1::KernelDescriptor,
        CompilerModuleSymbolRoleV1::DeviceFfiExport,
        CompilerModuleSymbolRoleV1::UnresolvedExternalImport,
    ] {
        symbols.extend(manifest.symbols(role).map(str::to_owned));
    }
    symbols.sort();
    symbols
}

fn inspect_defined_global_symbols(
    bytes: &[u8],
) -> Result<Vec<String>, WorkerV2RawHsacoInspectionError> {
    let file = object::File::parse(bytes)
        .map_err(|_| WorkerV2RawHsacoInspectionError::DefinedSymbolInspection)?;
    let mut symbols = BTreeSet::new();
    for symbol in file.symbols() {
        if !symbol.is_definition() || (!symbol.is_global() && !symbol.is_weak()) {
            continue;
        }
        let name = symbol
            .name()
            .map_err(|_| WorkerV2RawHsacoInspectionError::DefinedSymbolInspection)?;
        if name.is_empty() || !symbols.insert(name.to_owned()) {
            return Err(WorkerV2RawHsacoInspectionError::DefinedSymbolInspection);
        }
    }
    if symbols.len() > MAX_WORKER_SYMBOLS {
        return Err(WorkerV2RawHsacoInspectionError::DefinedSymbolInspection);
    }
    Ok(symbols.into_iter().collect())
}

fn inspect_descriptor_section(
    bytes: &[u8],
) -> Result<CanonicalDescriptorSectionObservationV1, WorkerV2RawHsacoInspectionError> {
    let file = object::File::parse(bytes)
        .map_err(|_| WorkerV2RawHsacoInspectionError::DefinedSymbolInspection)?;
    for section in file.sections() {
        if section
            .name()
            .map_err(|_| WorkerV2RawHsacoInspectionError::DefinedSymbolInspection)?
            == DEVICE_DESCRIPTOR_SECTION_NAME
        {
            return Ok(
                CanonicalDescriptorSectionObservationV1::PresentButNotFinalizedByThisInspection,
            );
        }
    }
    Ok(CanonicalDescriptorSectionObservationV1::Missing)
}

fn encode_descriptor_observation_preimage(inspected: &InspectedKernelBindings) -> Vec<u8> {
    let mut bytes = Vec::new();
    push_u64(&mut bytes, inspected.bindings().len() as u64);
    for binding in inspected.bindings() {
        let descriptor = binding.descriptor();
        push_u64(&mut bytes, binding.kernel_index() as u64);
        push_u64(&mut bytes, binding.descriptor_address());
        push_u64(&mut bytes, binding.descriptor_file_offset());
        push_u64(&mut bytes, binding.entry_address());
        push_u64(&mut bytes, binding.entry_file_offset());
        push_u64(&mut bytes, binding.entry_size());
        push_u32(&mut bytes, descriptor.group_segment_fixed_size());
        push_u32(&mut bytes, descriptor.private_segment_fixed_size());
        push_u32(&mut bytes, descriptor.kernarg_size());
        push_i64(&mut bytes, descriptor.kernel_code_entry_byte_offset());
        push_u32(&mut bytes, descriptor.compute_pgm_rsrc3());
        push_u32(&mut bytes, descriptor.compute_pgm_rsrc1());
        push_u32(&mut bytes, descriptor.compute_pgm_rsrc2());
        push_u16(&mut bytes, descriptor.kernel_code_properties());
        push_u16(&mut bytes, descriptor.kernarg_preload());
    }
    bytes
}

fn encode_abi_observation_preimage(inspected: &InspectedKernelBindings) -> Vec<u8> {
    let mut bytes = Vec::new();
    let metadata = inspected.inspection();
    push_u32(&mut bytes, metadata.metadata_version().major());
    push_u32(&mut bytes, metadata.metadata_version().minor());
    push_u64(&mut bytes, metadata.kernels().len() as u64);
    for kernel in metadata.kernels() {
        push_text(&mut bytes, kernel.name());
        push_text(&mut bytes, kernel.symbol());
        push_u64(&mut bytes, kernel.kernarg_segment_size());
        push_u64(&mut bytes, kernel.kernarg_segment_alignment());
        push_optional_u64(&mut bytes, kernel.implicit_argument_offset());
        push_u64(&mut bytes, kernel.implicit_argument_size());
        push_u64(&mut bytes, kernel.explicit_arguments().len() as u64);
        for argument in kernel.explicit_arguments() {
            push_explicit_argument(&mut bytes, argument);
        }
        push_u64(&mut bytes, kernel.hidden_arguments().len() as u64);
        for argument in kernel.hidden_arguments() {
            push_hidden_argument(&mut bytes, *argument);
        }
    }
    bytes
}

fn encode_resource_observation_preimage(inspected: &InspectedKernelBindings) -> Vec<u8> {
    let mut bytes = Vec::new();
    let kernels = inspected.inspection().kernels();
    push_u64(&mut bytes, kernels.len() as u64);
    for kernel in kernels {
        push_text(&mut bytes, kernel.name());
        push_u64(&mut bytes, kernel.group_segment_fixed_size());
        push_u64(&mut bytes, kernel.private_segment_fixed_size());
        push_u32(&mut bytes, kernel.wavefront_size());
        push_u16(&mut bytes, kernel.sgpr_count());
        push_u16(&mut bytes, kernel.vgpr_count());
        push_optional_u32(&mut bytes, kernel.agpr_count());
        push_optional_u32(&mut bytes, kernel.sgpr_spill_count());
        push_optional_u32(&mut bytes, kernel.vgpr_spill_count());
        push_u32(&mut bytes, kernel.max_flat_workgroup_size());
        push_optional_dimensions(&mut bytes, kernel.required_workgroup_size());
        for limit in kernel.max_workgroups() {
            push_optional_u32(&mut bytes, limit);
        }
        push_optional_dimensions(&mut bytes, kernel.cluster_dims());
        push_optional_bool(&mut bytes, kernel.uniform_work_group_size_declaration());
        push_optional_bool(&mut bytes, kernel.uses_dynamic_stack_declaration());
        push_optional_bool(&mut bytes, kernel.workgroup_processor_mode());
    }
    bytes
}

fn calculate_observation_identity(domain: &[u8], preimage: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(preimage);
    hasher.finalize().into()
}

fn push_explicit_argument(bytes: &mut Vec<u8>, argument: &ExplicitArgument) {
    push_optional_text(bytes, argument.name());
    push_optional_text(bytes, argument.type_name());
    push_u64(bytes, argument.offset());
    push_u64(bytes, argument.size());
    push_optional_u64(bytes, argument.alignment());
    bytes.push(explicit_value_kind_tag(argument.value_kind()));
    push_optional_tag(bytes, argument.value_type().map(explicit_value_type_tag));
    push_optional_tag(bytes, argument.address_space().map(address_space_tag));
    push_optional_tag(bytes, argument.access().map(argument_access_tag));
    push_optional_tag(bytes, argument.actual_access().map(argument_access_tag));
    push_optional_u64(bytes, argument.pointee_alignment());
    push_optional_bool(bytes, argument.is_const());
    push_optional_bool(bytes, argument.is_restrict());
    push_optional_bool(bytes, argument.is_volatile());
    push_optional_bool(bytes, argument.is_pipe());
}

fn push_hidden_argument(bytes: &mut Vec<u8>, argument: HiddenArgument) {
    push_u64(bytes, argument.offset());
    push_u64(bytes, argument.size());
    bytes.push(hidden_value_kind_tag(argument.value_kind()));
}

fn push_optional_text(bytes: &mut Vec<u8>, value: Option<&str>) {
    match value {
        Some(value) => {
            bytes.push(1);
            push_text(bytes, value);
        }
        None => bytes.push(0),
    }
}

fn push_optional_u64(bytes: &mut Vec<u8>, value: Option<u64>) {
    match value {
        Some(value) => {
            bytes.push(1);
            push_u64(bytes, value);
        }
        None => bytes.push(0),
    }
}

fn push_optional_u32(bytes: &mut Vec<u8>, value: Option<u32>) {
    match value {
        Some(value) => {
            bytes.push(1);
            push_u32(bytes, value);
        }
        None => bytes.push(0),
    }
}

fn push_optional_bool(bytes: &mut Vec<u8>, value: Option<bool>) {
    match value {
        Some(value) => bytes.extend_from_slice(&[1, u8::from(value)]),
        None => bytes.push(0),
    }
}

fn push_optional_tag(bytes: &mut Vec<u8>, value: Option<u8>) {
    match value {
        Some(value) => bytes.extend_from_slice(&[1, value]),
        None => bytes.push(0),
    }
}

fn push_optional_dimensions(bytes: &mut Vec<u8>, value: Option<[u32; 3]>) {
    match value {
        Some(value) => {
            bytes.push(1);
            for dimension in value {
                push_u32(bytes, dimension);
            }
        }
        None => bytes.push(0),
    }
}

fn push_text(bytes: &mut Vec<u8>, value: &str) {
    push_u64(bytes, value.len() as u64);
    bytes.extend_from_slice(value.as_bytes());
}

fn push_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_i64(bytes: &mut Vec<u8>, value: i64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

const fn explicit_value_kind_tag(kind: ExplicitValueKind) -> u8 {
    match kind {
        ExplicitValueKind::ByValue => 0,
        ExplicitValueKind::GlobalBuffer => 1,
        ExplicitValueKind::DynamicSharedPointer => 2,
        ExplicitValueKind::Sampler => 3,
        ExplicitValueKind::Image => 4,
        ExplicitValueKind::Pipe => 5,
        ExplicitValueKind::Queue => 6,
    }
}

const fn explicit_value_type_tag(value_type: ExplicitValueType) -> u8 {
    match value_type {
        ExplicitValueType::Struct => 0,
        ExplicitValueType::I8 => 1,
        ExplicitValueType::U8 => 2,
        ExplicitValueType::I16 => 3,
        ExplicitValueType::U16 => 4,
        ExplicitValueType::F16 => 5,
        ExplicitValueType::I32 => 6,
        ExplicitValueType::U32 => 7,
        ExplicitValueType::F32 => 8,
        ExplicitValueType::I64 => 9,
        ExplicitValueType::U64 => 10,
        ExplicitValueType::F64 => 11,
    }
}

const fn address_space_tag(address_space: ArgumentAddressSpace) -> u8 {
    match address_space {
        ArgumentAddressSpace::Private => 0,
        ArgumentAddressSpace::Global => 1,
        ArgumentAddressSpace::Constant => 2,
        ArgumentAddressSpace::Local => 3,
        ArgumentAddressSpace::Generic => 4,
        ArgumentAddressSpace::Region => 5,
    }
}

const fn argument_access_tag(access: ArgumentAccess) -> u8 {
    match access {
        ArgumentAccess::ReadOnly => 0,
        ArgumentAccess::WriteOnly => 1,
        ArgumentAccess::ReadWrite => 2,
    }
}

const fn hidden_value_kind_tag(kind: HiddenValueKind) -> u8 {
    match kind {
        HiddenValueKind::BlockCountX => 0,
        HiddenValueKind::BlockCountY => 1,
        HiddenValueKind::BlockCountZ => 2,
        HiddenValueKind::GroupSizeX => 3,
        HiddenValueKind::GroupSizeY => 4,
        HiddenValueKind::GroupSizeZ => 5,
        HiddenValueKind::RemainderX => 6,
        HiddenValueKind::RemainderY => 7,
        HiddenValueKind::RemainderZ => 8,
        HiddenValueKind::GlobalOffsetX => 9,
        HiddenValueKind::GlobalOffsetY => 10,
        HiddenValueKind::GlobalOffsetZ => 11,
        HiddenValueKind::GridDimensions => 12,
        HiddenValueKind::None => 13,
        HiddenValueKind::PrintfBuffer => 14,
        HiddenValueKind::HostcallBuffer => 15,
        HiddenValueKind::HeapV1 => 16,
        HiddenValueKind::DefaultQueue => 17,
        HiddenValueKind::CompletionAction => 18,
        HiddenValueKind::MultigridSyncArgument => 19,
        HiddenValueKind::DynamicLdsSize => 20,
        HiddenValueKind::PrivateBase => 21,
        HiddenValueKind::SharedBase => 22,
        HiddenValueKind::QueuePointer => 23,
    }
}

fn calculate_policy_identity(
    target: DeviceTargetV1,
    code_object_version: CodeObjectVersion,
    compiler_envelope: CompilerFfiEnvelopeIdentityV1,
    manifest: &CompilerModuleSymbolManifestV1,
    observed_kernels: &[ObservedWorkerV2KernelSymbolsV1],
    expected_defined_symbols: &[String],
    launch: WorkerV2RawLaunchContractV1,
) -> WorkerV2RawHsacoPolicyIdentityV1 {
    let mut hasher = Sha256::new();
    hasher.update(POLICY_IDENTITY_DOMAIN_V1);
    hash_text(&mut hasher, &target.to_string());
    hasher.update([code_object_version_tag(code_object_version)]);
    hasher.update(compiler_envelope.as_bytes());
    hasher.update(manifest.identity().sha256());
    hasher.update(manifest.identity().byte_len().to_le_bytes());
    hasher.update((observed_kernels.len() as u64).to_le_bytes());
    for kernel in observed_kernels {
        hash_text(&mut hasher, &kernel.entry);
        hash_text(&mut hasher, &kernel.descriptor);
    }
    hash_strings(&mut hasher, expected_defined_symbols);
    for dimension in launch.required_workgroup_size() {
        hasher.update(dimension.to_le_bytes());
    }
    hasher.update(launch.max_flat_workgroup_size().to_le_bytes());
    hasher.update(launch.wavefront_size().to_le_bytes());
    WorkerV2RawHsacoPolicyIdentityV1(hasher.finalize().into())
}

fn calculate_response_identity(bytes: &[u8]) -> SealedWorkerV2ResponseIdentityV1 {
    SealedWorkerV2ResponseIdentityV1(calculate_response_identity_bytes_v1(bytes))
}

pub(crate) fn calculate_response_identity_bytes_v1(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(RESPONSE_IDENTITY_DOMAIN_V1);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    hasher.finalize().into()
}

fn calculate_inspection_identity(
    source: &InertFirstBuildWorkerV2EvidenceV1,
    policy: &WorkerV2RawHsacoPolicyV1,
    response: SealedWorkerV2ResponseIdentityV1,
    descriptor_section: CanonicalDescriptorSectionObservationV1,
) -> InspectedRawWorkerV2HsacoIdentityV1 {
    let worker = source.worker_measurement();
    let sealed_response = source.authorized().response();
    let mut hasher = Sha256::new();
    hasher.update(INSPECTION_IDENTITY_DOMAIN_V1);
    hasher.update(source.identity().as_bytes());
    hash_attempt(&mut hasher, source.attempt());
    hasher.update(source.handoff_identity().as_bytes());
    hash_content(&mut hasher, worker.executable());
    hash_text(&mut hasher, worker.worker_build_identity());
    hash_text(&mut hasher, worker.llvm_build_identity());
    hasher.update(sealed_response.request_id());
    hasher.update(sealed_response.request_identity());
    hasher.update(response.0);
    hash_content(&mut hasher, source.output_identity());
    hash_text(&mut hasher, &policy.target.to_string());
    hasher.update([code_object_version_tag(policy.code_object_version)]);
    hasher.update(policy.identity.0);
    hasher.update([descriptor_section_tag(descriptor_section)]);
    InspectedRawWorkerV2HsacoIdentityV1(hasher.finalize().into())
}

fn calculate_protected_inspection_identity(
    source: &InertProtectedFirstBuildWorkerV2EvidenceV1,
    raw: &SharedRawWorkerV2HsacoInspectionV1,
    response: SealedWorkerV2ResponseIdentityV1,
) -> InspectedProtectedRawWorkerV2HsacoIdentityV1 {
    InspectedProtectedRawWorkerV2HsacoIdentityV1(calculate_protected_inspection_identity_v2(
        &ProtectedInspectionIdentityPreimageV2 {
            source_identity: *source.identity().as_bytes(),
            attempt: source.attempt(),
            slot: source.slot(),
            handoff_identity: source.handoff_identity(),
            compiler_closure: source.compiler_closure(),
            worker: source.worker_measurement(),
            bootstrap_request_bytes: source.bootstrap_request_bytes(),
            bootstrap_response_bytes: source.bootstrap().response().canonical_bytes(),
            authorized_request_bytes: source.authorized_request_bytes(),
            authorized_response_bytes: source.authorized().response().canonical_bytes(),
            response_identity: response.0,
            raw_output_identity: source.output_identity(),
            exact_raw_bytes: source.output_bytes(),
            target: raw.policy.target,
            code_object_version: raw.policy.code_object_version,
            policy_identity: raw.policy.identity.0,
            descriptor_section: raw.descriptor_section,
            descriptor_identity: raw.descriptor_identity,
            abi_identity: raw.abi_identity,
            resource_identity: raw.resource_identity,
        },
    ))
}

fn calculate_protected_v3_inspection_identity(
    source: &InertProtectedFirstBuildWorkerV3EvidenceV1,
    raw: &SharedRawWorkerV2HsacoInspectionV1,
    response: SealedWorkerV2ResponseIdentityV1,
) -> InspectedProtectedRawWorkerV3HsacoIdentityV1 {
    let binding = source.binding();
    let expected = binding.expectation();
    let worker = source.worker_measurement();
    let outer_identity = expected.outer_handoff_identity();
    let nested_identity = expected.nested_handoff_identity();
    let mut hasher = Sha256::new();
    hasher.update(PROTECTED_V3_INSPECTION_IDENTITY_DOMAIN_V1);
    hasher.update(source.identity().as_bytes());
    hasher.update(binding.identity().as_bytes());
    hash_attempt(&mut hasher, expected.attempt());
    hasher.update([expected.slot() as u8]);
    hasher.update(expected.transaction_identity().as_bytes());
    hasher.update(outer_identity.sha256());
    hasher.update(outer_identity.byte_len().to_le_bytes());
    hasher.update(expected.capsule_sha256());
    hasher.update(expected.capsule_byte_len().to_le_bytes());
    hasher.update(expected.invocation_digest());
    hasher.update(expected.pair_binding_sha256());
    hasher.update(expected.pair_binding_byte_len().to_le_bytes());
    hasher.update(nested_identity.sha256());
    hasher.update(nested_identity.byte_len().to_le_bytes());
    hasher.update(expected.final_commitment_receipt_sha256());
    hasher.update(expected.final_commitment_receipt_byte_len().to_le_bytes());
    hasher.update(expected.final_commitment_sha256());
    hasher.update(expected.final_commitment_byte_len().to_le_bytes());
    hash_compiler_closure_v2(&mut hasher, expected.compiler_closure());
    hash_bytes(&mut hasher, source.handoff().canonical_bytes());
    hash_content(&mut hasher, worker.executable());
    hash_text(&mut hasher, worker.worker_build_identity());
    hash_text(&mut hasher, worker.llvm_build_identity());
    hash_bytes(&mut hasher, &source.plan().canonical_bytes());
    hash_bytes(&mut hasher, source.bootstrap_request_bytes());
    hash_bytes(&mut hasher, source.bootstrap().response().canonical_bytes());
    hash_bytes(&mut hasher, source.exact_replay_request_bytes());
    hash_bytes(
        &mut hasher,
        source.exact_replay().response().canonical_bytes(),
    );
    hasher.update(response.0);
    hash_content(&mut hasher, source.output_identity());
    hash_bytes(&mut hasher, source.output_bytes());
    hash_text(&mut hasher, &raw.policy.target.to_string());
    hasher.update([code_object_version_tag(raw.policy.code_object_version)]);
    hasher.update(raw.policy.identity.0);
    hasher.update([descriptor_section_tag(raw.descriptor_section)]);
    hasher.update(raw.descriptor_identity);
    hasher.update(raw.abi_identity);
    hasher.update(raw.resource_identity);
    InspectedProtectedRawWorkerV3HsacoIdentityV1(hasher.finalize().into())
}

pub(crate) struct ProtectedInspectionIdentityPreimageV2<'a> {
    pub(crate) source_identity: [u8; 32],
    pub(crate) attempt: BuildAttempt,
    pub(crate) slot: CompilerModuleHandoffSlotV2,
    pub(crate) handoff_identity: CompilerModuleHandoffIdentityV2,
    pub(crate) compiler_closure: CompilerClosureV2,
    pub(crate) worker: &'a WorkerMeasurementV1,
    pub(crate) bootstrap_request_bytes: &'a [u8],
    pub(crate) bootstrap_response_bytes: &'a [u8],
    pub(crate) authorized_request_bytes: &'a [u8],
    pub(crate) authorized_response_bytes: &'a [u8],
    pub(crate) response_identity: [u8; 32],
    pub(crate) raw_output_identity: ContentIdentityV1,
    pub(crate) exact_raw_bytes: &'a [u8],
    pub(crate) target: DeviceTargetV1,
    pub(crate) code_object_version: CodeObjectVersion,
    pub(crate) policy_identity: [u8; 32],
    pub(crate) descriptor_section: CanonicalDescriptorSectionObservationV1,
    pub(crate) descriptor_identity: [u8; 32],
    pub(crate) abi_identity: [u8; 32],
    pub(crate) resource_identity: [u8; 32],
}

pub(crate) fn calculate_protected_inspection_identity_v2(
    preimage: &ProtectedInspectionIdentityPreimageV2<'_>,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(PROTECTED_INSPECTION_IDENTITY_DOMAIN_V1);
    hasher.update(protected_component_identity(
        PROTECTED_SOURCE_EVIDENCE_DOMAIN_V1,
        |component| component.update(preimage.source_identity),
    ));
    hasher.update(protected_component_identity(
        PROTECTED_ATTEMPT_DOMAIN_V1,
        |component| hash_attempt(component, preimage.attempt),
    ));
    hasher.update(protected_component_identity(
        PROTECTED_HANDOFF_DOMAIN_V2,
        |component| {
            component.update([preimage.slot as u8]);
            component.update(preimage.handoff_identity.as_bytes());
        },
    ));
    hasher.update(protected_component_identity(
        PROTECTED_COMPILER_CLOSURE_DOMAIN_V2,
        |component| hash_compiler_closure_v2(component, preimage.compiler_closure),
    ));
    hasher.update(protected_component_identity(
        PROTECTED_WORKER_EXCHANGE_DOMAIN_V1,
        |component| {
            let worker = preimage.worker;
            hash_content(component, worker.executable());
            hash_text(component, worker.worker_build_identity());
            hash_text(component, worker.llvm_build_identity());
            hash_bytes(component, preimage.bootstrap_request_bytes);
            hash_bytes(component, preimage.bootstrap_response_bytes);
            hash_bytes(component, preimage.authorized_request_bytes);
            hash_bytes(component, preimage.authorized_response_bytes);
            component.update(preimage.response_identity);
        },
    ));
    hasher.update(protected_component_identity(
        PROTECTED_RAW_BYTES_DOMAIN_V1,
        |component| {
            hash_content(component, preimage.raw_output_identity);
            hash_bytes(component, preimage.exact_raw_bytes);
        },
    ));
    hasher.update(protected_component_identity(
        PROTECTED_TARGET_DOMAIN_V1,
        |component| hash_text(component, &preimage.target.to_string()),
    ));
    hasher.update(protected_component_identity(
        PROTECTED_CODE_OBJECT_VERSION_DOMAIN_V1,
        |component| component.update([code_object_version_tag(preimage.code_object_version)]),
    ));
    hasher.update(protected_component_identity(
        PROTECTED_POLICY_DOMAIN_V1,
        |component| {
            component.update(preimage.policy_identity);
            component.update([descriptor_section_tag(preimage.descriptor_section)]);
        },
    ));
    hasher.update(preimage.descriptor_identity);
    hasher.update(preimage.abi_identity);
    hasher.update(preimage.resource_identity);
    hasher.finalize().into()
}

fn protected_component_identity(domain: &[u8], update: impl FnOnce(&mut Sha256)) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    update(&mut hasher);
    hasher.finalize().into()
}

fn hash_attempt(hasher: &mut Sha256, attempt: BuildAttempt) {
    hasher.update(attempt.generation().to_le_bytes());
    hasher.update(attempt.session().as_bytes());
    hasher.update(attempt.invocation().as_bytes());
}

fn hash_compiler_closure_v2(hasher: &mut Sha256, closure: CompilerClosureV2) {
    hasher.update(closure.cargo_executable_sha256());
    hasher.update(closure.cargo_binding_trampoline_sha256());
    hasher.update(closure.cargo_fe2o3_binding_wrapper_sha256());
    hasher.update(closure.rustc_executable_sha256());
    hasher.update(closure.rustc_runtime_tree_sha256());
    hasher.update(closure.codegen_backend_sha256());
    hasher.update(
        closure
            .cargo_binding_transition_protocol_version()
            .to_le_bytes(),
    );
    hasher.update(closure.identity_sha256());
}

fn hash_content(hasher: &mut Sha256, identity: ContentIdentityV1) {
    hasher.update(identity.sha256());
    hasher.update(identity.byte_len().to_le_bytes());
}

fn hash_bytes(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

fn hash_strings(hasher: &mut Sha256, strings: &[String]) {
    hasher.update((strings.len() as u64).to_le_bytes());
    for value in strings {
        hash_text(hasher, value);
    }
}

fn hash_text(hasher: &mut Sha256, value: &str) {
    hasher.update((value.len() as u64).to_le_bytes());
    hasher.update(value.as_bytes());
}

const fn descriptor_section_tag(section: CanonicalDescriptorSectionObservationV1) -> u8 {
    match section {
        CanonicalDescriptorSectionObservationV1::Missing => 0,
        CanonicalDescriptorSectionObservationV1::PresentButNotFinalizedByThisInspection => 1,
    }
}

const fn map_code_object_version(version: InspectedCodeObjectVersion) -> CodeObjectVersion {
    match version {
        InspectedCodeObjectVersion::V4 => CodeObjectVersion::V4,
        InspectedCodeObjectVersion::V5 => CodeObjectVersion::V5,
        InspectedCodeObjectVersion::V6 => CodeObjectVersion::V6,
    }
}

const fn map_compiler_code_object_version(version: CompilerCodeObjectVersion) -> CodeObjectVersion {
    match version {
        CompilerCodeObjectVersion::V4 => CodeObjectVersion::V4,
        CompilerCodeObjectVersion::V5 => CodeObjectVersion::V5,
        CompilerCodeObjectVersion::V6 => CodeObjectVersion::V6,
    }
}

const fn code_object_version_tag(version: CodeObjectVersion) -> u8 {
    match version {
        CodeObjectVersion::V4 => 4,
        CodeObjectVersion::V5 => 5,
        CodeObjectVersion::V6 => 6,
    }
}
