//! Canonical descriptor finalization for independently inspected Worker V2 HSACO.
//!
//! This boundary consumes raw inspection evidence exactly once, runs the canonical
//! `.fe2o3.kd.v1` finalizer, and retains both sides of the resulting lineage. The prepared value
//! remains descriptive evidence: it authenticates neither the compiler nor Verus and grants no
//! publication, HSA loading, or launch authority.

use std::{error::Error, fmt};

use fe2o3_artifact_transaction::{
    BuildAttempt, CompilerModuleHandoffIdentityV2, CompilerModuleHandoffSlotV2,
};
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_compiler_ffi::{
    CompilerFfiContractV1, CompilerFfiEnvelopeBuilderV1, CompilerFfiEnvelopeV1,
    CompilerFfiLinkRoleV1, CompilerFfiSourceOwnerV1, CompilerModuleSymbolManifestV1,
    MAX_COMPILER_FFI_ENVELOPE_BYTES_V1, MAX_COMPILER_MODULE_SYMBOL_MANIFEST_BYTES_V1,
};
use fe2o3_hsaco::{CodeObjectVersion as InspectedCodeObjectVersion, MAX_HSACO_BYTES};
use fe2o3_kernel_descriptor::{
    CanonicalCodeObjectDigest, CodeObjectVersion, DeviceDescriptorTableV1, DeviceTargetV1,
};
use reserved_fe2o3_symbols::{
    DeviceFfiContractIdV1, DeviceFfiDirectionV1, MAX_DEVICE_FFI_EFFECT_BYTES_V1,
    MAX_DEVICE_FFI_PHYSICAL_ABI_BYTES_V1, MAX_DEVICE_FFI_SYMBOL_BYTES_V1,
};
use sha2::{Digest, Sha256};

use crate::{
    CanonicalDescriptorSectionObservationV1, ContentIdentityV1, DEVICE_DESCRIPTOR_SECTION_NAME,
    FinalizationError, FinalizedHsaco, FirstBuildWorkerV2IdentityV1, InertDecodedWorkerExchangeV2,
    InspectedProtectedRawWorkerV2HsacoIdentityV1, InspectedProtectedRawWorkerV2HsacoV1,
    InspectedRawWorkerV2HsacoIdentityV1, InspectedRawWorkerV2HsacoV1, LinkInputV1, LinkOptionV1,
    LinkOutputV1, MAX_LINK_INPUTS, MAX_LINK_OPTION_NAME_BYTES, MAX_LINK_OPTION_VALUE_BYTES,
    MAX_LINK_OPTIONS, MAX_LINK_PROVENANCE_EDGES, MAX_LINK_PROVENANCE_NODES,
    MAX_WORKER_EXECUTABLE_BYTES, MAX_WORKER_REQUEST_BYTES, MAX_WORKER_RESPONSE_BYTES,
    MAX_WORKER_SYMBOLS, MAX_WORKER_TOOLCHAIN_ID_BYTES, MultiInputLinkPlanV1,
    ObservedWorkerV2KernelSymbolsV1, ProtectedFirstBuildWorkerV2IdentityV1, ProvenanceNodeV1,
    WorkerMeasurementV1, WorkerV2RawHsacoPolicyIdentityV1, WorkerV2RawHsacoPolicyV1,
    WorkerV2RawLaunchContractV1, finalize_allocated_read_only_unfinalized, finalize_unfinalized,
    first_build_worker_v2::{
        ProtectedFirstBuildReplayValidationV2, validate_protected_first_build_replay_v2,
    },
    verify_allocated_read_only_finalized, verify_finalized,
    worker_v2_hsaco_admission::{
        ProtectedInspectionIdentityPreimageV2, WorkerV2RawLaunchDiagnosticProfileV1,
        calculate_protected_inspection_identity_v2, calculate_response_identity_bytes_v1,
        inspect_worker_v2_raw_hsaco_preimage_v1,
    },
};

const FINALIZED_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/WORKER-V2-CANONICAL-FINALIZATION/V1\0";
const PROTECTED_FINALIZED_IDENTITY_DOMAIN_V2: &[u8] =
    b"FE2O3/CLOSURE-PROTECTED-WORKER-V2-CANONICAL-FINALIZATION/V2\0";
const PROTECTED_FINALIZED_INSPECTION_DOMAIN_V2: &[u8] =
    b"FE2O3/CLOSURE-PROTECTED-FINALIZATION/RAW-INSPECTION/V2\0";
const PROTECTED_FINALIZED_SOURCE_DOMAIN_V2: &[u8] =
    b"FE2O3/CLOSURE-PROTECTED-FINALIZATION/SOURCE-EVIDENCE/V2\0";
const PROTECTED_FINALIZED_HANDOFF_SLOT_DOMAIN_V2: &[u8] =
    b"FE2O3/CLOSURE-PROTECTED-FINALIZATION/HANDOFF-SLOT/V2\0";
const PROTECTED_FINALIZED_HANDOFF_IDENTITY_DOMAIN_V2: &[u8] =
    b"FE2O3/CLOSURE-PROTECTED-FINALIZATION/HANDOFF-IDENTITY/V2\0";
const PROTECTED_FINALIZED_COMPILER_CLOSURE_DOMAIN_V2: &[u8] =
    b"FE2O3/CLOSURE-PROTECTED-FINALIZATION/COMPILER-CLOSURE/V2\0";
const PROTECTED_LINEAGE_MAGIC_V2: [u8; 8] = *b"FE2PLV2\0";
const PROTECTED_LINEAGE_VERSION_V2: u16 = 2;
const PROTECTED_LINEAGE_CHECKSUM_DOMAIN_V2: &[u8] =
    b"FE2O3/FINALIZER/PROTECTED-WORKER-V2-LINEAGE-CHECKSUM/V2\0";
const PROTECTED_LINEAGE_IDENTITY_DOMAIN_V2: &[u8] =
    b"FE2O3/FINALIZER/PROTECTED-WORKER-V2-LINEAGE-IDENTITY/V2\0";
const COMPILER_FFI_ENVELOPE_DOMAIN_V1: &[u8] = b"FE2O3/COMPILER-FFI-ENVELOPE/V1\0";
const LINK_PLAN_DOMAIN_V1: &[u8] = b"FE2O3/AMDGPU-MULTI-INPUT-LINK-PLAN/V1\0";
const MAX_TARGET_TEXT_BYTES_V2: usize = 256;
const MAX_ATTEMPT_TEXT_BYTES_V2: usize = 128;
const MAX_LINK_PLAN_BYTES_V2: usize = 4 * 1024 * 1024;
const MAX_OBSERVATION_PREIMAGE_BYTES_V2: usize = MAX_HSACO_BYTES;
const PROTECTED_LINEAGE_FIXED_BYTES_V2: usize = 8
    + 2
    + 1
    + 32
    + 32
    + 1
    + 32
    + 2
    + 1
    + 32
    + ((6 * 32) + 2 + 32)
    + 2
    + 1
    + (5 * 4)
    + 40
    + 2
    + 2
    + (10 * 4)
    + 32
    + 40
    + 40
    + 32
    + 1
    + (3 * 32)
    + 32
    + 32;

/// Maximum aggregate canonical bytes accepted for one finalizer-owned protected lineage.
///
/// Component protocol limits are deliberately not additive here. Requests and responses can each
/// carry the same HSACO, so adding their independent maxima admitted nearly 500 MiB transcripts
/// and multi-gigabyte decode peaks. Larger evidence must use a future content-addressed schema.
pub const MAX_PROTECTED_WORKER_V2_FINALIZER_LINEAGE_BYTES_V2: usize = 16 * 1024 * 1024;

/// Stable identity of one raw-inspection-to-canonical-finalization transition.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FinalizedWorkerV2HsacoIdentityV1([u8; 32]);

impl FinalizedWorkerV2HsacoIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Stable V2-domain identity of one protected raw-to-canonical finalization transition.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FinalizedProtectedWorkerV2HsacoIdentityV2([u8; 32]);

impl FinalizedProtectedWorkerV2HsacoIdentityV2 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// The exact evidence class needed before a missing descriptor table may be constructed.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DescriptorSourceEvidenceRequirementV1 {
    /// Authenticated compiler-derived source, ABI, layout, effect, and build-evidence claims.
    AuthenticatedCanonicalDescriptorTableV1,
}

/// Owning fail-closed record for raw Worker V2 output that has no canonical descriptor table.
///
/// The retained raw inspection can only be retried by a later trusted bridge that supplies the
/// required authenticated descriptor source. This value does not expose a constructor or the raw
/// bytes and grants no authority.
#[derive(Debug)]
pub struct MissingAuthenticatedDescriptorSourceEvidenceV1 {
    raw: InspectedRawWorkerV2HsacoV1,
}

impl MissingAuthenticatedDescriptorSourceEvidenceV1 {
    pub const fn requirement(&self) -> DescriptorSourceEvidenceRequirementV1 {
        DescriptorSourceEvidenceRequirementV1::AuthenticatedCanonicalDescriptorTableV1
    }

    pub const fn raw_inspection_identity(&self) -> InspectedRawWorkerV2HsacoIdentityV1 {
        self.raw.identity()
    }

    pub const fn source_evidence_identity(&self) -> FirstBuildWorkerV2IdentityV1 {
        self.raw.source_evidence_identity()
    }

    pub const fn raw_output_identity(&self) -> ContentIdentityV1 {
        self.raw.linked_output_identity()
    }

    pub const fn policy_identity(&self) -> WorkerV2RawHsacoPolicyIdentityV1 {
        self.raw.policy().identity()
    }

    pub const fn attempt(&self) -> BuildAttempt {
        self.raw.attempt()
    }

    pub const fn target(&self) -> DeviceTargetV1 {
        self.raw.target()
    }

    pub const fn code_object_version(&self) -> CodeObjectVersion {
        self.raw.code_object_version()
    }

    pub fn observed_kernels(&self) -> &[ObservedWorkerV2KernelSymbolsV1] {
        self.raw.policy().observed_kernels()
    }

    pub const fn canonical_descriptor_section(&self) -> CanonicalDescriptorSectionObservationV1 {
        self.raw.canonical_descriptor_section()
    }

    pub const fn may_infer_descriptor_claims_from_executable_metadata(&self) -> bool {
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

/// Owning fail-closed record for protected raw output with no canonical descriptor table.
///
/// The complete V2 handoff and compiler closure stay retained without being represented as V1
/// lineage. This value is inert and intentionally exposes neither a constructor nor raw bytes.
#[derive(Debug)]
pub struct MissingAuthenticatedProtectedDescriptorSourceEvidenceV2 {
    raw: InspectedProtectedRawWorkerV2HsacoV1,
}

impl MissingAuthenticatedProtectedDescriptorSourceEvidenceV2 {
    pub const fn requirement(&self) -> DescriptorSourceEvidenceRequirementV1 {
        DescriptorSourceEvidenceRequirementV1::AuthenticatedCanonicalDescriptorTableV1
    }

    pub const fn raw_inspection_identity(&self) -> InspectedProtectedRawWorkerV2HsacoIdentityV1 {
        self.raw.identity()
    }

    pub const fn source_evidence_identity(&self) -> ProtectedFirstBuildWorkerV2IdentityV1 {
        self.raw.source_evidence_identity()
    }

    pub const fn raw_output_identity(&self) -> ContentIdentityV1 {
        self.raw.linked_output_identity()
    }

    pub const fn policy_identity(&self) -> WorkerV2RawHsacoPolicyIdentityV1 {
        self.raw.policy().identity()
    }

    pub const fn attempt(&self) -> BuildAttempt {
        self.raw.attempt()
    }

    pub const fn handoff_slot(&self) -> CompilerModuleHandoffSlotV2 {
        self.raw.handoff_slot()
    }

    pub const fn handoff_identity(&self) -> CompilerModuleHandoffIdentityV2 {
        self.raw.handoff_identity()
    }

    pub const fn compiler_closure(&self) -> CompilerClosureV2 {
        self.raw.compiler_closure()
    }

    pub const fn target(&self) -> DeviceTargetV1 {
        self.raw.target()
    }

    pub const fn code_object_version(&self) -> CodeObjectVersion {
        self.raw.code_object_version()
    }

    pub fn observed_kernels(&self) -> &[ObservedWorkerV2KernelSymbolsV1] {
        self.raw.policy().observed_kernels()
    }

    pub const fn canonical_descriptor_section(&self) -> CanonicalDescriptorSectionObservationV1 {
        self.raw.canonical_descriptor_section()
    }

    pub const fn may_infer_descriptor_claims_from_executable_metadata(&self) -> bool {
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

/// Opaque, inert Worker V2 HSACO after structural canonical descriptor finalization.
///
/// Ownership of the raw inspection is retained so callers cannot detach the finalized bytes from
/// their first-build lineage. The finalized object retains the independently reparsed descriptor
/// table and canonical digest. Descriptor source claims are not authenticated by this boundary,
/// so the result cannot authorize publication, loading, or launch. This type is intentionally not
/// `Clone`.
#[derive(Debug)]
pub struct PreparedFinalizedWorkerV2HsacoV1 {
    identity: FinalizedWorkerV2HsacoIdentityV1,
    raw: InspectedRawWorkerV2HsacoV1,
    finalized: FinalizedHsaco,
    finalized_output: ContentIdentityV1,
}

impl PreparedFinalizedWorkerV2HsacoV1 {
    pub const fn identity(&self) -> FinalizedWorkerV2HsacoIdentityV1 {
        self.identity
    }

    pub const fn raw_inspection_identity(&self) -> InspectedRawWorkerV2HsacoIdentityV1 {
        self.raw.identity()
    }

    pub const fn source_evidence_identity(&self) -> FirstBuildWorkerV2IdentityV1 {
        self.raw.source_evidence_identity()
    }

    pub const fn raw_output_identity(&self) -> ContentIdentityV1 {
        self.raw.linked_output_identity()
    }

    pub const fn finalized_output_identity(&self) -> ContentIdentityV1 {
        self.finalized_output
    }

    pub const fn policy_identity(&self) -> WorkerV2RawHsacoPolicyIdentityV1 {
        self.raw.policy().identity()
    }

    pub const fn attempt(&self) -> BuildAttempt {
        self.raw.attempt()
    }

    pub const fn target(&self) -> DeviceTargetV1 {
        self.raw.target()
    }

    pub const fn code_object_version(&self) -> CodeObjectVersion {
        self.raw.code_object_version()
    }

    pub const fn canonical_digest(&self) -> CanonicalCodeObjectDigest {
        self.finalized.inspection().digest()
    }

    /// Returns finalized bytes while retaining their complete typed lineage in this value.
    pub fn exact_finalized_bytes(&self) -> &[u8] {
        self.finalized.as_bytes()
    }

    pub(crate) const fn raw_inspection(&self) -> &InspectedRawWorkerV2HsacoV1 {
        &self.raw
    }

    pub const fn canonical_descriptor_finalization_ran(&self) -> bool {
        true
    }

    /// This bridge checks descriptor structure and executable agreement only.
    pub const fn has_authenticated_descriptor_source_evidence(&self) -> bool {
        false
    }

    pub const fn is_structural_only(&self) -> bool {
        true
    }

    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }

    pub const fn proves_verus_verification(&self) -> bool {
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

/// Opaque protected Worker V2 HSACO after structural canonical descriptor finalization.
///
/// This side-by-side V2 lineage retains the exact protected inspection, V2 handoff slot and
/// identity, and complete compiler closure. It remains descriptive evidence and is not cloneable.
#[derive(Debug)]
pub struct PreparedFinalizedProtectedWorkerV2HsacoV2 {
    identity: FinalizedProtectedWorkerV2HsacoIdentityV2,
    raw: InspectedProtectedRawWorkerV2HsacoV1,
    finalized: FinalizedHsaco,
    finalized_output: ContentIdentityV1,
}

impl PreparedFinalizedProtectedWorkerV2HsacoV2 {
    pub const fn identity(&self) -> FinalizedProtectedWorkerV2HsacoIdentityV2 {
        self.identity
    }

    pub const fn raw_inspection_identity(&self) -> InspectedProtectedRawWorkerV2HsacoIdentityV1 {
        self.raw.identity()
    }

    pub const fn source_evidence_identity(&self) -> ProtectedFirstBuildWorkerV2IdentityV1 {
        self.raw.source_evidence_identity()
    }

    pub const fn raw_output_identity(&self) -> ContentIdentityV1 {
        self.raw.linked_output_identity()
    }

    pub const fn finalized_output_identity(&self) -> ContentIdentityV1 {
        self.finalized_output
    }

    pub const fn policy_identity(&self) -> WorkerV2RawHsacoPolicyIdentityV1 {
        self.raw.policy().identity()
    }

    pub const fn attempt(&self) -> BuildAttempt {
        self.raw.attempt()
    }

    pub const fn handoff_slot(&self) -> CompilerModuleHandoffSlotV2 {
        self.raw.handoff_slot()
    }

    pub const fn handoff_identity(&self) -> CompilerModuleHandoffIdentityV2 {
        self.raw.handoff_identity()
    }

    pub const fn compiler_closure(&self) -> CompilerClosureV2 {
        self.raw.compiler_closure()
    }

    pub const fn target(&self) -> DeviceTargetV1 {
        self.raw.target()
    }

    pub const fn code_object_version(&self) -> CodeObjectVersion {
        self.raw.code_object_version()
    }

    pub const fn canonical_digest(&self) -> CanonicalCodeObjectDigest {
        self.finalized.inspection().digest()
    }

    pub fn exact_finalized_bytes(&self) -> &[u8] {
        self.finalized.as_bytes()
    }

    pub(crate) const fn raw_inspection(&self) -> &InspectedProtectedRawWorkerV2HsacoV1 {
        &self.raw
    }

    pub const fn canonical_descriptor_finalization_ran(&self) -> bool {
        true
    }

    pub const fn has_authenticated_descriptor_source_evidence(&self) -> bool {
        false
    }

    pub const fn is_structural_only(&self) -> bool {
        true
    }

    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }

    pub const fn proves_verus_verification(&self) -> bool {
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

/// Which exact finalizer-owned protected lineage is retained by a transcript.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtectedWorkerV2FinalizerLineageRouteV2 {
    InspectedRaw,
    CanonicallyFinalized,
}

/// Stable identity of one canonical finalizer-owned protected lineage transcript.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProtectedWorkerV2FinalizerLineageIdentityV2([u8; 32]);

impl ProtectedWorkerV2FinalizerLineageIdentityV2 {
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Bounded canonical replay transcript owned by the HSACO finalizer.
///
/// Construction is restricted to typed protected inspection/finalization evidence. Decoding
/// requires the exact raw and final bytes and reruns the finalizer's inspection and canonical
/// finalization algorithms. This record is descriptive only and grants no authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtectedWorkerV2FinalizerLineageV2 {
    route: ProtectedWorkerV2FinalizerLineageRouteV2,
    source_evidence_identity: [u8; 32],
    raw_inspection_identity: [u8; 32],
    canonical_finalization_identity: Option<[u8; 32]>,
    attempt: BuildAttempt,
    handoff_slot: CompilerModuleHandoffSlotV2,
    handoff_identity: CompilerModuleHandoffIdentityV2,
    compiler_closure: CompilerClosureV2,
    target: DeviceTargetV1,
    code_object_version: CodeObjectVersion,
    launch: WorkerV2RawLaunchContractV1,
    compiler_envelope_bytes: Vec<u8>,
    symbol_manifest_bytes: Vec<u8>,
    worker: WorkerMeasurementV1,
    link_plan_bytes: Vec<u8>,
    bootstrap_request_bytes: Vec<u8>,
    bootstrap_response_bytes: Vec<u8>,
    authorized_request_bytes: Vec<u8>,
    authorized_response_bytes: Vec<u8>,
    response_identity: [u8; 32],
    raw_output_identity: ContentIdentityV1,
    final_output_identity: ContentIdentityV1,
    policy_identity: [u8; 32],
    descriptor_section: CanonicalDescriptorSectionObservationV1,
    descriptor_identity: [u8; 32],
    abi_identity: [u8; 32],
    resource_identity: [u8; 32],
    descriptor_observation_preimage: Vec<u8>,
    abi_observation_preimage: Vec<u8>,
    resource_observation_preimage: Vec<u8>,
    canonical_code_object_digest: [u8; 32],
}

impl ProtectedWorkerV2FinalizerLineageV2 {
    pub fn from_inspected(
        source: &InspectedProtectedRawWorkerV2HsacoV1,
    ) -> Result<Self, ProtectedWorkerV2FinalizerLineageDecodeErrorV2> {
        Self::from_raw(source, None)
    }

    pub fn from_finalized(
        source: &PreparedFinalizedProtectedWorkerV2HsacoV2,
    ) -> Result<Self, ProtectedWorkerV2FinalizerLineageDecodeErrorV2> {
        Self::from_raw(source.raw_inspection(), Some(source))
    }

    fn from_raw(
        raw: &InspectedProtectedRawWorkerV2HsacoV1,
        finalized: Option<&PreparedFinalizedProtectedWorkerV2HsacoV2>,
    ) -> Result<Self, ProtectedWorkerV2FinalizerLineageDecodeErrorV2> {
        let source = raw.source_evidence();
        let (descriptor_identity, abi_identity, resource_identity) = raw.observation_identities();
        let route = if finalized.is_some() {
            ProtectedWorkerV2FinalizerLineageRouteV2::CanonicallyFinalized
        } else {
            ProtectedWorkerV2FinalizerLineageRouteV2::InspectedRaw
        };
        let link_plan_bytes = source.plan().canonical_bytes();
        validate_protected_lineage_encoded_length([
            raw.attempt().to_env_value().len(),
            raw.target().to_string().len(),
            source.worker_measurement().worker_build_identity().len(),
            source.worker_measurement().llvm_build_identity().len(),
            source.compiler_envelope().canonical_bytes().len(),
            source.symbol_manifest().canonical_bytes().len(),
            link_plan_bytes.len(),
            source.bootstrap_request_bytes().len(),
            source.bootstrap().response().canonical_bytes().len(),
            source.authorized_request_bytes().len(),
            source.authorized().response().canonical_bytes().len(),
            raw.descriptor_observation_preimage().len(),
            raw.abi_observation_preimage().len(),
            raw.resource_observation_preimage().len(),
        ])?;
        Ok(Self {
            route,
            source_evidence_identity: *raw.source_evidence_identity().as_bytes(),
            raw_inspection_identity: *raw.identity().as_bytes(),
            canonical_finalization_identity: finalized.map(|value| *value.identity().as_bytes()),
            attempt: raw.attempt(),
            handoff_slot: raw.handoff_slot(),
            handoff_identity: raw.handoff_identity(),
            compiler_closure: raw.compiler_closure(),
            target: raw.target(),
            code_object_version: raw.code_object_version(),
            launch: raw.policy().launch(),
            compiler_envelope_bytes: source.compiler_envelope().canonical_bytes().to_vec(),
            symbol_manifest_bytes: source.symbol_manifest().canonical_bytes().to_vec(),
            worker: source.worker_measurement().clone(),
            link_plan_bytes,
            bootstrap_request_bytes: source.bootstrap_request_bytes().to_vec(),
            bootstrap_response_bytes: source.bootstrap().response().canonical_bytes().to_vec(),
            authorized_request_bytes: source.authorized_request_bytes().to_vec(),
            authorized_response_bytes: source.authorized().response().canonical_bytes().to_vec(),
            response_identity: *raw.response_identity().as_bytes(),
            raw_output_identity: raw.linked_output_identity(),
            final_output_identity: finalized.map_or(raw.linked_output_identity(), |value| {
                value.finalized_output_identity()
            }),
            policy_identity: *raw.policy().identity().as_bytes(),
            descriptor_section: raw.canonical_descriptor_section(),
            descriptor_identity,
            abi_identity,
            resource_identity,
            descriptor_observation_preimage: raw.descriptor_observation_preimage().to_vec(),
            abi_observation_preimage: raw.abi_observation_preimage().to_vec(),
            resource_observation_preimage: raw.resource_observation_preimage().to_vec(),
            canonical_code_object_digest: finalized
                .map_or([0; 32], |value| *value.canonical_digest().as_bytes()),
        })
    }

    pub const fn route(&self) -> ProtectedWorkerV2FinalizerLineageRouteV2 {
        self.route
    }

    pub const fn source_evidence_identity(&self) -> [u8; 32] {
        self.source_evidence_identity
    }

    pub const fn raw_inspection_identity(&self) -> [u8; 32] {
        self.raw_inspection_identity
    }

    pub const fn canonical_finalization_identity(&self) -> Option<[u8; 32]> {
        self.canonical_finalization_identity
    }

    pub const fn attempt(&self) -> BuildAttempt {
        self.attempt
    }

    pub const fn handoff_slot(&self) -> CompilerModuleHandoffSlotV2 {
        self.handoff_slot
    }

    pub const fn handoff_identity(&self) -> CompilerModuleHandoffIdentityV2 {
        self.handoff_identity
    }

    /// Whether standalone V2 replay independently recomputes the artifact-transaction identity.
    ///
    /// V2 retains that identity and closes both worker request IDs over it, but its wire omits the
    /// producer and derived-slot preimages needed by the artifact-transaction identity algorithm.
    /// A later publication join must therefore compare this inert lineage with a trusted root.
    pub const fn independently_rederives_transaction_handoff_identity(&self) -> bool {
        false
    }

    pub const fn compiler_closure(&self) -> CompilerClosureV2 {
        self.compiler_closure
    }

    pub const fn target(&self) -> DeviceTargetV1 {
        self.target
    }

    pub const fn code_object_version(&self) -> CodeObjectVersion {
        self.code_object_version
    }

    pub const fn raw_output_identity(&self) -> ContentIdentityV1 {
        self.raw_output_identity
    }

    pub const fn final_output_identity(&self) -> ContentIdentityV1 {
        self.final_output_identity
    }

    pub const fn policy_identity(&self) -> [u8; 32] {
        self.policy_identity
    }

    pub const fn canonical_code_object_digest(&self) -> [u8; 32] {
        self.canonical_code_object_digest
    }

    pub const fn descriptor_identity(&self) -> [u8; 32] {
        self.descriptor_identity
    }

    pub const fn abi_identity(&self) -> [u8; 32] {
        self.abi_identity
    }

    pub const fn resource_identity(&self) -> [u8; 32] {
        self.resource_identity
    }

    pub fn identity(&self) -> ProtectedWorkerV2FinalizerLineageIdentityV2 {
        let bytes = self.canonical_bytes();
        ProtectedWorkerV2FinalizerLineageIdentityV2(hash_domain_bytes(
            PROTECTED_LINEAGE_IDENTITY_DOMAIN_V2,
            &bytes,
        ))
    }

    pub fn matches_inspected_source(&self, source: &InspectedProtectedRawWorkerV2HsacoV1) -> bool {
        Self::from_inspected(source).is_ok_and(|expected| self == &expected)
    }

    pub fn matches_finalized_source(
        &self,
        source: &PreparedFinalizedProtectedWorkerV2HsacoV2,
    ) -> bool {
        Self::from_finalized(source).is_ok_and(|expected| self == &expected)
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
}

#[derive(Debug)]
#[non_exhaustive]
pub enum ProtectedWorkerV2FinalizerLineageDecodeErrorV2 {
    Length,
    Magic,
    Version,
    Tag,
    Truncated,
    TrailingBytes,
    Checksum,
    NonCanonical,
    Attempt,
    CompilerClosure,
    Target,
    CompilerEnvelope,
    SymbolManifest,
    WorkerMeasurement,
    LinkPlan,
    WorkerExchange,
    SourceIdentity,
    RawInspection(crate::WorkerV2RawHsacoInspectionError),
    RawLineage(&'static str),
    CanonicalFinalization(FinalizationError),
    FinalLineage(&'static str),
    DescriptorJoin(&'static str),
}

impl fmt::Display for ProtectedWorkerV2FinalizerLineageDecodeErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length => formatter.write_str("protected lineage transcript exceeds its bound"),
            Self::Magic => formatter.write_str("invalid protected lineage transcript magic"),
            Self::Version => {
                formatter.write_str("unsupported protected lineage transcript version")
            }
            Self::Tag => formatter.write_str("invalid protected lineage transcript tag"),
            Self::Truncated => formatter.write_str("truncated protected lineage transcript"),
            Self::TrailingBytes => {
                formatter.write_str("protected lineage transcript has trailing bytes")
            }
            Self::Checksum => formatter.write_str("protected lineage transcript checksum mismatch"),
            Self::NonCanonical => formatter.write_str("non-canonical protected lineage transcript"),
            Self::Attempt => formatter.write_str("invalid protected lineage build attempt"),
            Self::CompilerClosure => {
                formatter.write_str("invalid protected lineage compiler closure")
            }
            Self::Target => formatter.write_str("invalid protected lineage target"),
            Self::CompilerEnvelope => {
                formatter.write_str("invalid protected lineage compiler envelope")
            }
            Self::SymbolManifest => {
                formatter.write_str("invalid protected lineage symbol manifest")
            }
            Self::WorkerMeasurement => {
                formatter.write_str("invalid protected lineage worker measurement")
            }
            Self::LinkPlan => formatter.write_str("invalid protected lineage link plan"),
            Self::WorkerExchange => {
                formatter.write_str("invalid protected lineage worker exchange")
            }
            Self::SourceIdentity => {
                formatter.write_str("protected first-build source identity mismatch")
            }
            Self::RawInspection(error) => {
                write!(formatter, "protected raw reinspection failed: {error}")
            }
            Self::RawLineage(field) => write!(formatter, "protected raw lineage mismatch: {field}"),
            Self::CanonicalFinalization(error) => {
                write!(formatter, "canonical replay finalization failed: {error}")
            }
            Self::FinalLineage(field) => write!(
                formatter,
                "protected finalization lineage mismatch: {field}"
            ),
            Self::DescriptorJoin(field) => {
                write!(formatter, "protected descriptor join mismatch: {field}")
            }
        }
    }
}

impl Error for ProtectedWorkerV2FinalizerLineageDecodeErrorV2 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::RawInspection(error) => Some(error),
            Self::CanonicalFinalization(error) => Some(error),
            _ => None,
        }
    }
}

impl ProtectedWorkerV2FinalizerLineageV2 {
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let attempt = self.attempt.to_env_value();
        let target = self.target.to_string();
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&PROTECTED_LINEAGE_MAGIC_V2);
        bytes.extend_from_slice(&PROTECTED_LINEAGE_VERSION_V2.to_le_bytes());
        bytes.push(match self.route {
            ProtectedWorkerV2FinalizerLineageRouteV2::InspectedRaw => 0,
            ProtectedWorkerV2FinalizerLineageRouteV2::CanonicallyFinalized => 1,
        });
        bytes.extend_from_slice(&self.source_evidence_identity);
        bytes.extend_from_slice(&self.raw_inspection_identity);
        bytes.push(u8::from(self.canonical_finalization_identity.is_some()));
        bytes.extend_from_slice(&self.canonical_finalization_identity.unwrap_or([0; 32]));
        push_u16_text(&mut bytes, &attempt);
        bytes.push(self.handoff_slot as u8);
        bytes.extend_from_slice(self.handoff_identity.as_bytes());
        encode_compiler_closure_v2(&mut bytes, self.compiler_closure);
        push_u16_text(&mut bytes, &target);
        bytes.push(code_object_version_tag(self.code_object_version));
        for dimension in self.launch.required_workgroup_size() {
            bytes.extend_from_slice(&dimension.to_le_bytes());
        }
        bytes.extend_from_slice(&self.launch.max_flat_workgroup_size().to_le_bytes());
        bytes.extend_from_slice(&self.launch.wavefront_size().to_le_bytes());
        encode_content_identity_v2(&mut bytes, self.worker.executable());
        push_u16_text(&mut bytes, self.worker.worker_build_identity());
        push_u16_text(&mut bytes, self.worker.llvm_build_identity());
        for segment in [
            self.compiler_envelope_bytes.as_slice(),
            self.symbol_manifest_bytes.as_slice(),
            self.link_plan_bytes.as_slice(),
            self.bootstrap_request_bytes.as_slice(),
            self.bootstrap_response_bytes.as_slice(),
            self.authorized_request_bytes.as_slice(),
            self.authorized_response_bytes.as_slice(),
            self.descriptor_observation_preimage.as_slice(),
            self.abi_observation_preimage.as_slice(),
            self.resource_observation_preimage.as_slice(),
        ] {
            push_u32_segment(&mut bytes, segment);
        }
        bytes.extend_from_slice(&self.response_identity);
        encode_content_identity_v2(&mut bytes, self.raw_output_identity);
        encode_content_identity_v2(&mut bytes, self.final_output_identity);
        bytes.extend_from_slice(&self.policy_identity);
        bytes.push(match self.descriptor_section {
            CanonicalDescriptorSectionObservationV1::Missing => 0,
            CanonicalDescriptorSectionObservationV1::PresentButNotFinalizedByThisInspection => 1,
        });
        bytes.extend_from_slice(&self.descriptor_identity);
        bytes.extend_from_slice(&self.abi_identity);
        bytes.extend_from_slice(&self.resource_identity);
        bytes.extend_from_slice(&self.canonical_code_object_digest);
        let checksum = hash_domain_bytes(PROTECTED_LINEAGE_CHECKSUM_DOMAIN_V2, &bytes);
        bytes.extend_from_slice(&checksum);
        debug_assert_eq!(
            Some(bytes.len()),
            protected_lineage_encoded_length(self.variable_encoded_lengths()),
            "protected lineage fixed-byte accounting drifted from its canonical schema"
        );
        bytes
    }

    fn variable_encoded_lengths(&self) -> [usize; 14] {
        [
            self.attempt.to_env_value().len(),
            self.target.to_string().len(),
            self.worker.worker_build_identity().len(),
            self.worker.llvm_build_identity().len(),
            self.compiler_envelope_bytes.len(),
            self.symbol_manifest_bytes.len(),
            self.link_plan_bytes.len(),
            self.bootstrap_request_bytes.len(),
            self.bootstrap_response_bytes.len(),
            self.authorized_request_bytes.len(),
            self.authorized_response_bytes.len(),
            self.descriptor_observation_preimage.len(),
            self.abi_observation_preimage.len(),
            self.resource_observation_preimage.len(),
        ]
    }

    pub fn decode_canonical(
        bytes: &[u8],
        exact_raw_bytes: &[u8],
        exact_final_bytes: &[u8],
    ) -> Result<Self, ProtectedWorkerV2FinalizerLineageDecodeErrorV2> {
        if bytes.len() < PROTECTED_LINEAGE_FIXED_BYTES_V2
            || bytes.len() > MAX_PROTECTED_WORKER_V2_FINALIZER_LINEAGE_BYTES_V2
        {
            return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::Length);
        }
        let (body, checksum) = bytes.split_at(bytes.len() - 32);
        if hash_domain_bytes(PROTECTED_LINEAGE_CHECKSUM_DOMAIN_V2, body) != checksum {
            return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::Checksum);
        }
        let mut reader = FinalizerLineageReaderV2::new(body);
        if reader.take(8)? != PROTECTED_LINEAGE_MAGIC_V2 {
            return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::Magic);
        }
        if reader.u16()? != PROTECTED_LINEAGE_VERSION_V2 {
            return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::Version);
        }
        let route = match reader.u8()? {
            0 => ProtectedWorkerV2FinalizerLineageRouteV2::InspectedRaw,
            1 => ProtectedWorkerV2FinalizerLineageRouteV2::CanonicallyFinalized,
            _ => return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::Tag),
        };
        let source_evidence_identity = reader.array()?;
        let raw_inspection_identity = reader.array()?;
        let final_present = reader.u8()?;
        let final_identity = reader.array()?;
        let canonical_finalization_identity = match (final_present, final_identity) {
            (0, value) if value == [0; 32] => None,
            (1, value) if value != [0; 32] => Some(value),
            _ => return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::NonCanonical),
        };
        let attempt_text = reader.text_u16(MAX_ATTEMPT_TEXT_BYTES_V2)?;
        let attempt = BuildAttempt::from_env_value(attempt_text)
            .map_err(|_| ProtectedWorkerV2FinalizerLineageDecodeErrorV2::Attempt)?;
        if attempt.to_env_value() != attempt_text {
            return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::NonCanonical);
        }
        let handoff_slot = decode_handoff_slot(reader.u8()?)?;
        let handoff_identity = CompilerModuleHandoffIdentityV2::from_bytes(reader.array()?);
        let compiler_closure = decode_compiler_closure_v2(&mut reader)?;
        let target_text = reader.text_u16(MAX_TARGET_TEXT_BYTES_V2)?;
        let target = DeviceTargetV1::parse(target_text)
            .map_err(|_| ProtectedWorkerV2FinalizerLineageDecodeErrorV2::Target)?;
        if target.to_string() != target_text {
            return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::NonCanonical);
        }
        let code_object_version = decode_code_object_version_v2(reader.u8()?)?;
        let launch = WorkerV2RawLaunchContractV1::from_transcript_parts(
            [reader.u32()?, reader.u32()?, reader.u32()?],
            reader.u32()?,
            reader.u32()?,
        )
        .ok_or(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::Tag)?;
        let worker_executable =
            decode_content_identity_v2(&mut reader, MAX_WORKER_EXECUTABLE_BYTES)?;
        let worker_build_identity = reader.text_u16(MAX_WORKER_TOOLCHAIN_ID_BYTES)?.to_owned();
        let llvm_build_identity = reader.text_u16(MAX_WORKER_TOOLCHAIN_ID_BYTES)?.to_owned();
        let worker = WorkerMeasurementV1::new(
            worker_executable,
            worker_build_identity,
            llvm_build_identity,
        )
        .map_err(|_| ProtectedWorkerV2FinalizerLineageDecodeErrorV2::WorkerMeasurement)?;
        let compiler_envelope_bytes = reader.segment(MAX_COMPILER_FFI_ENVELOPE_BYTES_V1)?;
        let symbol_manifest_bytes = reader.segment(MAX_COMPILER_MODULE_SYMBOL_MANIFEST_BYTES_V1)?;
        let link_plan_bytes = reader.segment(MAX_LINK_PLAN_BYTES_V2)?;
        let bootstrap_request_bytes = reader.segment(MAX_WORKER_REQUEST_BYTES)?;
        let bootstrap_response_bytes = reader.segment(MAX_WORKER_RESPONSE_BYTES)?;
        let authorized_request_bytes = reader.segment(MAX_WORKER_REQUEST_BYTES)?;
        let authorized_response_bytes = reader.segment(MAX_WORKER_RESPONSE_BYTES)?;
        let descriptor_observation_preimage = reader.segment(MAX_OBSERVATION_PREIMAGE_BYTES_V2)?;
        let abi_observation_preimage = reader.segment(MAX_OBSERVATION_PREIMAGE_BYTES_V2)?;
        let resource_observation_preimage = reader.segment(MAX_OBSERVATION_PREIMAGE_BYTES_V2)?;
        let value = Self {
            route,
            source_evidence_identity,
            raw_inspection_identity,
            canonical_finalization_identity,
            attempt,
            handoff_slot,
            handoff_identity,
            compiler_closure,
            target,
            code_object_version,
            launch,
            compiler_envelope_bytes,
            symbol_manifest_bytes,
            worker,
            link_plan_bytes,
            bootstrap_request_bytes,
            bootstrap_response_bytes,
            authorized_request_bytes,
            authorized_response_bytes,
            descriptor_observation_preimage,
            abi_observation_preimage,
            resource_observation_preimage,
            response_identity: reader.array()?,
            raw_output_identity: decode_content_identity_v2(&mut reader, MAX_HSACO_BYTES as u64)?,
            final_output_identity: decode_content_identity_v2(&mut reader, MAX_HSACO_BYTES as u64)?,
            policy_identity: reader.array()?,
            descriptor_section: match reader.u8()? {
                0 => CanonicalDescriptorSectionObservationV1::Missing,
                1 => {
                    CanonicalDescriptorSectionObservationV1::PresentButNotFinalizedByThisInspection
                }
                _ => return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::Tag),
            },
            descriptor_identity: reader.array()?,
            abi_identity: reader.array()?,
            resource_identity: reader.array()?,
            canonical_code_object_digest: reader.array()?,
        };
        if !reader.finished() {
            return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::TrailingBytes);
        }
        value.validate_recomputed(exact_raw_bytes, exact_final_bytes)?;
        Ok(value)
    }

    fn validate_recomputed(
        &self,
        exact_raw_bytes: &[u8],
        exact_final_bytes: &[u8],
    ) -> Result<(), ProtectedWorkerV2FinalizerLineageDecodeErrorV2> {
        if !self.raw_output_identity.matches(exact_raw_bytes) {
            return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::RawLineage(
                "exact raw bytes",
            ));
        }
        let compiler_envelope = decode_compiler_ffi_envelope_v1(&self.compiler_envelope_bytes)?;
        if compiler_envelope.target() != self.target
            || compiler_envelope.code_object_version() != self.code_object_version
        {
            return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::CompilerEnvelope);
        }
        let symbol_manifest =
            CompilerModuleSymbolManifestV1::decode(&self.symbol_manifest_bytes)
                .map_err(|_| ProtectedWorkerV2FinalizerLineageDecodeErrorV2::SymbolManifest)?;
        let link_plan = decode_link_plan_v1(&self.link_plan_bytes)?;
        if link_plan.target() != self.target
            || link_plan.output().identity() != self.raw_output_identity
        {
            return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::LinkPlan);
        }
        let bootstrap = InertDecodedWorkerExchangeV2::decode(
            &self.bootstrap_request_bytes,
            &self.bootstrap_response_bytes,
        )
        .map_err(|_| ProtectedWorkerV2FinalizerLineageDecodeErrorV2::WorkerExchange)?;
        let authorized = InertDecodedWorkerExchangeV2::decode(
            &self.authorized_request_bytes,
            &self.authorized_response_bytes,
        )
        .map_err(|_| ProtectedWorkerV2FinalizerLineageDecodeErrorV2::WorkerExchange)?;
        let replay_validation =
            validate_protected_first_build_replay_v2(ProtectedFirstBuildReplayValidationV2 {
                attempt: self.attempt,
                slot: self.handoff_slot,
                handoff_identity: self.handoff_identity,
                compiler_closure: self.compiler_closure,
                compiler_envelope: &compiler_envelope,
                symbol_manifest: &symbol_manifest,
                worker: &self.worker,
                plan: &link_plan,
                bootstrap_request_bytes: &self.bootstrap_request_bytes,
                bootstrap_request: bootstrap.request(),
                bootstrap_response: bootstrap.response(),
                authorized_request_bytes: &self.authorized_request_bytes,
                authorized_request: authorized.request(),
                authorized_response: authorized.response(),
                expected_output_identity: self.raw_output_identity,
                exact_output_bytes: exact_raw_bytes,
            })
            .map_err(|error| {
                ProtectedWorkerV2FinalizerLineageDecodeErrorV2::RawLineage(error.field())
            })?;
        if replay_validation.output_identity() != self.raw_output_identity {
            return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::RawLineage(
                "validated replay output",
            ));
        }
        if calculate_response_identity_bytes_v1(&self.authorized_response_bytes)
            != self.response_identity
        {
            return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::RawLineage(
                "sealed response identity",
            ));
        }
        let source_identity = *replay_validation.evidence_identity().as_bytes();
        if source_identity != self.source_evidence_identity {
            return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::SourceIdentity);
        }

        let raw = inspect_worker_v2_raw_hsaco_preimage_v1(
            self.target,
            self.code_object_version,
            symbol_manifest,
            compiler_envelope.identity(),
            self.raw_output_identity,
            exact_raw_bytes,
            self.launch,
            WorkerV2RawLaunchDiagnosticProfileV1::ProductionV1,
        )
        .map_err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::RawInspection)?;
        if raw.policy.identity().as_bytes() != &self.policy_identity
            || raw.descriptor_section != self.descriptor_section
            || raw.descriptor_identity != self.descriptor_identity
            || raw.abi_identity != self.abi_identity
            || raw.resource_identity != self.resource_identity
            || raw.descriptor_observation_preimage != self.descriptor_observation_preimage
            || raw.abi_observation_preimage != self.abi_observation_preimage
            || raw.resource_observation_preimage != self.resource_observation_preimage
        {
            return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::RawLineage(
                "policy or parsed observation",
            ));
        }
        let inspection_identity =
            calculate_protected_inspection_identity_v2(&ProtectedInspectionIdentityPreimageV2 {
                source_identity,
                attempt: self.attempt,
                slot: self.handoff_slot,
                handoff_identity: self.handoff_identity,
                compiler_closure: self.compiler_closure,
                worker: &self.worker,
                bootstrap_request_bytes: &self.bootstrap_request_bytes,
                bootstrap_response_bytes: &self.bootstrap_response_bytes,
                authorized_request_bytes: &self.authorized_request_bytes,
                authorized_response_bytes: &self.authorized_response_bytes,
                response_identity: self.response_identity,
                raw_output_identity: self.raw_output_identity,
                exact_raw_bytes,
                target: self.target,
                code_object_version: self.code_object_version,
                policy_identity: self.policy_identity,
                descriptor_section: self.descriptor_section,
                descriptor_identity: self.descriptor_identity,
                abi_identity: self.abi_identity,
                resource_identity: self.resource_identity,
            });
        if inspection_identity != self.raw_inspection_identity {
            return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::RawLineage(
                "protected inspection identity",
            ));
        }

        match self.route {
            ProtectedWorkerV2FinalizerLineageRouteV2::InspectedRaw => {
                if self.canonical_finalization_identity.is_some()
                    || self.final_output_identity != self.raw_output_identity
                    || exact_final_bytes != exact_raw_bytes
                    || self.canonical_code_object_digest != [0; 32]
                {
                    return Err(
                        ProtectedWorkerV2FinalizerLineageDecodeErrorV2::FinalLineage(
                            "raw route final fields",
                        ),
                    );
                }
            }
            ProtectedWorkerV2FinalizerLineageRouteV2::CanonicallyFinalized => {
                if self.descriptor_section
                    != CanonicalDescriptorSectionObservationV1::PresentButNotFinalizedByThisInspection
                    || !self.final_output_identity.matches(exact_final_bytes)
                {
                    return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::FinalLineage(
                        "exact finalized bytes",
                    ));
                }
                let finalized = finalize_unfinalized(exact_raw_bytes).map_err(
                    ProtectedWorkerV2FinalizerLineageDecodeErrorV2::CanonicalFinalization,
                )?;
                if finalized.as_bytes() != exact_final_bytes {
                    return Err(
                        ProtectedWorkerV2FinalizerLineageDecodeErrorV2::FinalLineage(
                            "canonical finalizer output",
                        ),
                    );
                }
                let verified = verify_finalized(exact_final_bytes).map_err(
                    ProtectedWorkerV2FinalizerLineageDecodeErrorV2::CanonicalFinalization,
                )?;
                if verified != *finalized.inspection()
                    || verified.digest().as_bytes() != &self.canonical_code_object_digest
                {
                    return Err(
                        ProtectedWorkerV2FinalizerLineageDecodeErrorV2::FinalLineage(
                            "canonical digest",
                        ),
                    );
                }
                validate_metadata_lineage_parts(
                    self.target,
                    self.code_object_version,
                    &raw.policy,
                    &finalized,
                )
                .map_err(|_| {
                    ProtectedWorkerV2FinalizerLineageDecodeErrorV2::FinalLineage(
                        "target, ABI, symbol, or launch metadata",
                    )
                })?;
                let final_identity = calculate_protected_finalized_identity_v2(
                    &ProtectedFinalizationIdentityPreimageV2 {
                        raw_inspection_identity: inspection_identity,
                        source_evidence_identity: source_identity,
                        handoff_slot: self.handoff_slot,
                        handoff_identity: self.handoff_identity,
                        compiler_closure: self.compiler_closure,
                        policy: &raw.policy,
                        raw_output: self.raw_output_identity,
                        finalized_output: self.final_output_identity,
                        canonical_digest: verified.digest(),
                    },
                );
                if self.canonical_finalization_identity != Some(final_identity) {
                    return Err(
                        ProtectedWorkerV2FinalizerLineageDecodeErrorV2::FinalLineage(
                            "canonical finalization identity",
                        ),
                    );
                }
            }
        }
        Ok(())
    }

    pub fn validate_descriptor_table(
        &self,
        table: &DeviceDescriptorTableV1,
    ) -> Result<(), ProtectedWorkerV2FinalizerLineageDecodeErrorV2> {
        if table.device_target() != self.target
            || table.code_object_version() != self.code_object_version
            || (self.route == ProtectedWorkerV2FinalizerLineageRouteV2::CanonicallyFinalized
                && table.canonical_code_object_digest().as_bytes()
                    != &self.canonical_code_object_digest)
        {
            return Err(
                ProtectedWorkerV2FinalizerLineageDecodeErrorV2::DescriptorJoin(
                    "target, code-object version, or canonical digest",
                ),
            );
        }
        let mut abi = decode_abi_observation_summaries_v2(&self.abi_observation_preimage)?;
        let mut resources =
            decode_resource_observation_summaries_v2(&self.resource_observation_preimage)?;
        if table.kernels().len() != abi.len() || table.kernels().len() != resources.len() {
            return Err(
                ProtectedWorkerV2FinalizerLineageDecodeErrorV2::DescriptorJoin("kernel count"),
            );
        }
        let mut kernels: Vec<_> = table.kernels().iter().collect();
        kernels.sort_unstable_by_key(|kernel| kernel.entry_name().as_str());
        abi.sort_unstable_by(|left, right| left.entry.cmp(&right.entry));
        resources.sort_unstable_by(|left, right| left.entry.cmp(&right.entry));
        validate_sorted_descriptor_entries_v2(
            kernels.iter().map(|kernel| kernel.entry_name().as_str()),
            abi.iter().map(|value| value.entry.as_str()),
            resources.iter().map(|value| value.entry.as_str()),
        )?;
        for ((kernel, abi), resource) in kernels.into_iter().zip(&abi).zip(&resources) {
            let required_block = resource
                .required_workgroup_size
                .map(|required| {
                    fe2o3_kernel_descriptor::DimensionsV1::new(
                        required[0],
                        required[1],
                        required[2],
                    )
                    .map(fe2o3_kernel_descriptor::BlockSizeV1::Exact)
                })
                .transpose()
                .map_err(|_| {
                    ProtectedWorkerV2FinalizerLineageDecodeErrorV2::DescriptorJoin(
                        "required workgroup size",
                    )
                })?;
            if kernel.descriptor_symbol().as_str() != abi.descriptor
                || u64::from(kernel.abi_layout().kernarg_segment_size()) != abi.kernarg_segment_size
                || u64::from(kernel.abi_layout().kernarg_segment_alignment())
                    != abi.kernarg_segment_alignment
                || kernel.launch().max_flat_workgroup_size() != resource.max_flat_workgroup_size
                || u64::from(kernel.launch().static_shared_memory_bytes())
                    != resource.group_segment_fixed_size
                || required_block.is_some_and(|required| kernel.launch().block_size() != required)
            {
                return Err(
                    ProtectedWorkerV2FinalizerLineageDecodeErrorV2::DescriptorJoin(
                        "symbol, kernarg segment, alignment, launch, or resource facet",
                    ),
                );
            }
        }
        Ok(())
    }
}

fn validate_protected_lineage_encoded_length(
    variable_bytes: impl IntoIterator<Item = usize>,
) -> Result<(), ProtectedWorkerV2FinalizerLineageDecodeErrorV2> {
    let encoded_bytes = protected_lineage_encoded_length(variable_bytes)
        .ok_or(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::Length)?;
    if encoded_bytes > MAX_PROTECTED_WORKER_V2_FINALIZER_LINEAGE_BYTES_V2 {
        return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::Length);
    }
    Ok(())
}

fn protected_lineage_encoded_length(
    variable_bytes: impl IntoIterator<Item = usize>,
) -> Option<usize> {
    variable_bytes
        .into_iter()
        .try_fold(PROTECTED_LINEAGE_FIXED_BYTES_V2, usize::checked_add)
}

fn validate_sorted_descriptor_entries_v2<'a>(
    mut kernels: impl Iterator<Item = &'a str>,
    mut abi: impl Iterator<Item = &'a str>,
    mut resources: impl Iterator<Item = &'a str>,
) -> Result<(), ProtectedWorkerV2FinalizerLineageDecodeErrorV2> {
    let mut previous = None;
    loop {
        match (kernels.next(), abi.next(), resources.next()) {
            (None, None, None) => return Ok(()),
            (Some(kernel), Some(abi), Some(resource)) => {
                if previous == Some(kernel) {
                    return Err(
                        ProtectedWorkerV2FinalizerLineageDecodeErrorV2::DescriptorJoin(
                            "duplicate kernel entry",
                        ),
                    );
                }
                if kernel != abi || kernel != resource {
                    return Err(
                        ProtectedWorkerV2FinalizerLineageDecodeErrorV2::DescriptorJoin(
                            "kernel entry",
                        ),
                    );
                }
                previous = Some(kernel);
            }
            _ => {
                return Err(
                    ProtectedWorkerV2FinalizerLineageDecodeErrorV2::DescriptorJoin("kernel count"),
                );
            }
        }
    }
}

fn hash_domain_bytes(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(bytes);
    hasher.finalize().into()
}

fn push_u16_text(bytes: &mut Vec<u8>, value: &str) {
    bytes.extend_from_slice(&(value.len() as u16).to_le_bytes());
    bytes.extend_from_slice(value.as_bytes());
}

fn push_u32_segment(bytes: &mut Vec<u8>, value: &[u8]) {
    bytes.extend_from_slice(&(value.len() as u32).to_le_bytes());
    bytes.extend_from_slice(value);
}

fn encode_content_identity_v2(bytes: &mut Vec<u8>, identity: ContentIdentityV1) {
    bytes.extend_from_slice(identity.sha256());
    bytes.extend_from_slice(&identity.byte_len().to_le_bytes());
}

fn encode_compiler_closure_v2(bytes: &mut Vec<u8>, closure: CompilerClosureV2) {
    for digest in [
        closure.cargo_executable_sha256(),
        closure.cargo_binding_trampoline_sha256(),
        closure.cargo_fe2o3_binding_wrapper_sha256(),
        closure.rustc_executable_sha256(),
        closure.rustc_runtime_tree_sha256(),
        closure.codegen_backend_sha256(),
    ] {
        bytes.extend_from_slice(&digest);
    }
    bytes.extend_from_slice(
        &closure
            .cargo_binding_transition_protocol_version()
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&closure.identity_sha256());
}

struct FinalizerLineageReaderV2<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> FinalizerLineageReaderV2<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn take(
        &mut self,
        length: usize,
    ) -> Result<&'a [u8], ProtectedWorkerV2FinalizerLineageDecodeErrorV2> {
        let end = self
            .position
            .checked_add(length)
            .ok_or(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::Length)?;
        let value = self
            .bytes
            .get(self.position..end)
            .ok_or(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::Truncated)?;
        self.position = end;
        Ok(value)
    }

    fn array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], ProtectedWorkerV2FinalizerLineageDecodeErrorV2> {
        self.take(N)?
            .try_into()
            .map_err(|_| ProtectedWorkerV2FinalizerLineageDecodeErrorV2::Truncated)
    }

    fn u8(&mut self) -> Result<u8, ProtectedWorkerV2FinalizerLineageDecodeErrorV2> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, ProtectedWorkerV2FinalizerLineageDecodeErrorV2> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, ProtectedWorkerV2FinalizerLineageDecodeErrorV2> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, ProtectedWorkerV2FinalizerLineageDecodeErrorV2> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    fn text_u16(
        &mut self,
        maximum: usize,
    ) -> Result<&'a str, ProtectedWorkerV2FinalizerLineageDecodeErrorV2> {
        let length = usize::from(self.u16()?);
        self.text(length, maximum)
    }

    fn text_u32(
        &mut self,
        maximum: usize,
    ) -> Result<&'a str, ProtectedWorkerV2FinalizerLineageDecodeErrorV2> {
        let length = usize::try_from(self.u32()?)
            .map_err(|_| ProtectedWorkerV2FinalizerLineageDecodeErrorV2::Length)?;
        self.text(length, maximum)
    }

    fn text_u32_allow_empty(
        &mut self,
        maximum: usize,
    ) -> Result<&'a str, ProtectedWorkerV2FinalizerLineageDecodeErrorV2> {
        let length = usize::try_from(self.u32()?)
            .map_err(|_| ProtectedWorkerV2FinalizerLineageDecodeErrorV2::Length)?;
        if length > maximum {
            return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::Length);
        }
        std::str::from_utf8(self.take(length)?)
            .map_err(|_| ProtectedWorkerV2FinalizerLineageDecodeErrorV2::NonCanonical)
    }

    fn text(
        &mut self,
        length: usize,
        maximum: usize,
    ) -> Result<&'a str, ProtectedWorkerV2FinalizerLineageDecodeErrorV2> {
        if length == 0 || length > maximum {
            return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::Length);
        }
        std::str::from_utf8(self.take(length)?)
            .map_err(|_| ProtectedWorkerV2FinalizerLineageDecodeErrorV2::NonCanonical)
    }

    fn segment(
        &mut self,
        maximum: usize,
    ) -> Result<Vec<u8>, ProtectedWorkerV2FinalizerLineageDecodeErrorV2> {
        let length = usize::try_from(self.u32()?)
            .map_err(|_| ProtectedWorkerV2FinalizerLineageDecodeErrorV2::Length)?;
        if length == 0 || length > maximum {
            return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::Length);
        }
        Ok(self.take(length)?.to_vec())
    }

    const fn finished(&self) -> bool {
        self.position == self.bytes.len()
    }
}

fn decode_content_identity_v2(
    reader: &mut FinalizerLineageReaderV2<'_>,
    maximum_byte_len: u64,
) -> Result<ContentIdentityV1, ProtectedWorkerV2FinalizerLineageDecodeErrorV2> {
    let sha256 = reader.array()?;
    let byte_len = reader.u64()?;
    if sha256 == [0; 32] || byte_len == 0 || byte_len > maximum_byte_len {
        return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::Length);
    }
    Ok(ContentIdentityV1::from_parts(sha256, byte_len))
}

fn decode_compiler_closure_v2(
    reader: &mut FinalizerLineageReaderV2<'_>,
) -> Result<CompilerClosureV2, ProtectedWorkerV2FinalizerLineageDecodeErrorV2> {
    CompilerClosureV2::from_pins_and_identity(
        reader.array()?,
        reader.array()?,
        reader.array()?,
        reader.array()?,
        reader.array()?,
        reader.array()?,
        reader.u16()?,
        reader.array()?,
    )
    .map_err(|_| ProtectedWorkerV2FinalizerLineageDecodeErrorV2::CompilerClosure)
}

fn decode_handoff_slot(
    value: u8,
) -> Result<CompilerModuleHandoffSlotV2, ProtectedWorkerV2FinalizerLineageDecodeErrorV2> {
    match value {
        0 => Ok(CompilerModuleHandoffSlotV2::Default),
        1 => Ok(CompilerModuleHandoffSlotV2::GeneralGemmReference),
        2 => Ok(CompilerModuleHandoffSlotV2::GeneralGemmVectorizedAOnly),
        _ => Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::Tag),
    }
}

fn decode_code_object_version_v2(
    value: u8,
) -> Result<CodeObjectVersion, ProtectedWorkerV2FinalizerLineageDecodeErrorV2> {
    match value {
        4 => Ok(CodeObjectVersion::V4),
        5 => Ok(CodeObjectVersion::V5),
        6 => Ok(CodeObjectVersion::V6),
        _ => Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::Tag),
    }
}

fn decode_compiler_ffi_envelope_v1(
    bytes: &[u8],
) -> Result<CompilerFfiEnvelopeV1, ProtectedWorkerV2FinalizerLineageDecodeErrorV2> {
    if bytes.is_empty() || bytes.len() > MAX_COMPILER_FFI_ENVELOPE_BYTES_V1 {
        return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::CompilerEnvelope);
    }
    let mut reader = FinalizerLineageReaderV2::new(bytes);
    if reader.take(COMPILER_FFI_ENVELOPE_DOMAIN_V1.len())? != COMPILER_FFI_ENVELOPE_DOMAIN_V1 {
        return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::CompilerEnvelope);
    }
    let target = DeviceTargetV1::parse(reader.text_u32(MAX_TARGET_TEXT_BYTES_V2)?)
        .map_err(|_| ProtectedWorkerV2FinalizerLineageDecodeErrorV2::CompilerEnvelope)?;
    let code_object_version = decode_code_object_version_v2(reader.u8()?)?;
    let count = usize::try_from(reader.u32()?)
        .map_err(|_| ProtectedWorkerV2FinalizerLineageDecodeErrorV2::CompilerEnvelope)?;
    if count == 0 {
        if !reader.finished() {
            return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::CompilerEnvelope);
        }
        let value =
            CompilerFfiEnvelopeV1::for_module_without_device_ffi(target, code_object_version)
                .map_err(|_| ProtectedWorkerV2FinalizerLineageDecodeErrorV2::CompilerEnvelope)?;
        if value.canonical_bytes() != bytes {
            return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::NonCanonical);
        }
        return Ok(value);
    }
    let mut builder = CompilerFfiEnvelopeBuilderV1::new(target, code_object_version, count)
        .map_err(|_| ProtectedWorkerV2FinalizerLineageDecodeErrorV2::CompilerEnvelope)?;
    for _ in 0..count {
        let contract_identity = DeviceFfiContractIdV1::from_bytes(reader.array()?);
        let direction = match reader.u8()? {
            1 => DeviceFfiDirectionV1::Import,
            2 => DeviceFfiDirectionV1::Export,
            _ => return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::CompilerEnvelope),
        };
        let link_role = match reader.u8()? {
            1 => CompilerFfiLinkRoleV1::RequiresExternalDefinition,
            2 => CompilerFfiLinkRoleV1::RequiresCompilerModuleDefinition,
            _ => return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::CompilerEnvelope),
        };
        let declared_owner_identity: [u8; 32] = reader.array()?;
        let crate_label = reader.text_u32(MAX_COMPILER_FFI_ENVELOPE_BYTES_V1)?;
        let item_path = reader.text_u32(MAX_COMPILER_FFI_ENVELOPE_BYTES_V1)?;
        let def_path_hash = reader.array()?;
        let instance_symbol = reader.text_u32(MAX_COMPILER_FFI_ENVELOPE_BYTES_V1)?;
        let symbol = reader.text_u32(MAX_DEVICE_FFI_SYMBOL_BYTES_V1)?;
        let contract_target = DeviceTargetV1::parse(reader.text_u32(MAX_TARGET_TEXT_BYTES_V2)?)
            .map_err(|_| ProtectedWorkerV2FinalizerLineageDecodeErrorV2::CompilerEnvelope)?;
        let physical_abi = reader.text_u32(MAX_DEVICE_FFI_PHYSICAL_ABI_BYTES_V1)?;
        let effects = reader.text_u32(MAX_DEVICE_FFI_EFFECT_BYTES_V1)?;
        let declared_effect_abi_identity: [u8; 32] = reader.array()?;
        let semantic_identity = reader.array()?;
        if contract_target != target {
            return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::CompilerEnvelope);
        }
        let owner =
            CompilerFfiSourceOwnerV1::new(crate_label, item_path, def_path_hash, instance_symbol)
                .map_err(|_| ProtectedWorkerV2FinalizerLineageDecodeErrorV2::CompilerEnvelope)?;
        if owner.identity().as_bytes() != declared_owner_identity {
            return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::CompilerEnvelope);
        }
        let contract = CompilerFfiContractV1::new(
            contract_identity,
            direction,
            link_role,
            target,
            code_object_version,
            owner,
            symbol,
            physical_abi,
            effects,
            semantic_identity,
        )
        .map_err(|_| ProtectedWorkerV2FinalizerLineageDecodeErrorV2::CompilerEnvelope)?;
        if contract.effect_abi_identity().as_bytes() != declared_effect_abi_identity {
            return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::CompilerEnvelope);
        }
        builder
            .push(contract)
            .map_err(|_| ProtectedWorkerV2FinalizerLineageDecodeErrorV2::CompilerEnvelope)?;
    }
    if !reader.finished() {
        return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::TrailingBytes);
    }
    let value = builder
        .finish()
        .map_err(|_| ProtectedWorkerV2FinalizerLineageDecodeErrorV2::CompilerEnvelope)?;
    if value.canonical_bytes() != bytes {
        return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::NonCanonical);
    }
    Ok(value)
}

fn decode_link_plan_v1(
    bytes: &[u8],
) -> Result<MultiInputLinkPlanV1, ProtectedWorkerV2FinalizerLineageDecodeErrorV2> {
    if bytes.is_empty() || bytes.len() > MAX_LINK_PLAN_BYTES_V2 {
        return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::LinkPlan);
    }
    let mut reader = FinalizerLineageReaderV2::new(bytes);
    if reader.take(LINK_PLAN_DOMAIN_V1.len())? != LINK_PLAN_DOMAIN_V1 {
        return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::LinkPlan);
    }
    let target = DeviceTargetV1::parse(reader.text_u32(MAX_TARGET_TEXT_BYTES_V2)?)
        .map_err(|_| ProtectedWorkerV2FinalizerLineageDecodeErrorV2::LinkPlan)?;
    let input_count = usize::try_from(reader.u32()?)
        .map_err(|_| ProtectedWorkerV2FinalizerLineageDecodeErrorV2::LinkPlan)?;
    if input_count == 0 || input_count > MAX_LINK_INPUTS {
        return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::LinkPlan);
    }
    let mut inputs = Vec::with_capacity(input_count);
    for _ in 0..input_count {
        inputs.push(LinkInputV1::new(
            decode_content_identity_v2(&mut reader, MAX_HSACO_BYTES as u64)?,
            target,
        ));
    }
    let option_count = usize::try_from(reader.u32()?)
        .map_err(|_| ProtectedWorkerV2FinalizerLineageDecodeErrorV2::LinkPlan)?;
    if option_count > MAX_LINK_OPTIONS {
        return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::LinkPlan);
    }
    let mut options = Vec::with_capacity(option_count);
    for _ in 0..option_count {
        options.push(
            LinkOptionV1::new(
                reader.text_u32(MAX_LINK_OPTION_NAME_BYTES)?,
                reader.text_u32_allow_empty(MAX_LINK_OPTION_VALUE_BYTES)?,
            )
            .map_err(|_| ProtectedWorkerV2FinalizerLineageDecodeErrorV2::LinkPlan)?,
        );
    }
    let output = LinkOutputV1::new(
        decode_content_identity_v2(&mut reader, MAX_HSACO_BYTES as u64)?,
        target,
    );
    let node_count = usize::try_from(reader.u32()?)
        .map_err(|_| ProtectedWorkerV2FinalizerLineageDecodeErrorV2::LinkPlan)?;
    if node_count == 0 || node_count > MAX_LINK_PROVENANCE_NODES {
        return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::LinkPlan);
    }
    let mut provenance = Vec::with_capacity(node_count);
    let mut edge_count = 0_usize;
    for _ in 0..node_count {
        let identity = decode_content_identity_v2(&mut reader, MAX_HSACO_BYTES as u64)?;
        let parent_count = usize::try_from(reader.u32()?)
            .map_err(|_| ProtectedWorkerV2FinalizerLineageDecodeErrorV2::LinkPlan)?;
        edge_count = edge_count
            .checked_add(parent_count)
            .ok_or(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::LinkPlan)?;
        if parent_count > MAX_LINK_PROVENANCE_EDGES || edge_count > MAX_LINK_PROVENANCE_EDGES {
            return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::LinkPlan);
        }
        let mut parents = Vec::with_capacity(parent_count);
        for _ in 0..parent_count {
            parents.push(decode_content_identity_v2(
                &mut reader,
                MAX_HSACO_BYTES as u64,
            )?);
        }
        provenance.push(
            ProvenanceNodeV1::new(identity, parents)
                .map_err(|_| ProtectedWorkerV2FinalizerLineageDecodeErrorV2::LinkPlan)?,
        );
    }
    if !reader.finished() {
        return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::TrailingBytes);
    }
    let value = MultiInputLinkPlanV1::new(target, inputs, options, output, provenance)
        .map_err(|_| ProtectedWorkerV2FinalizerLineageDecodeErrorV2::LinkPlan)?;
    if value.canonical_bytes() != bytes {
        return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::NonCanonical);
    }
    Ok(value)
}

struct AbiObservationSummaryV2 {
    entry: String,
    descriptor: String,
    kernarg_segment_size: u64,
    kernarg_segment_alignment: u64,
}

struct ResourceObservationSummaryV2 {
    entry: String,
    group_segment_fixed_size: u64,
    max_flat_workgroup_size: u32,
    required_workgroup_size: Option<[u32; 3]>,
}

fn decode_abi_observation_summaries_v2(
    bytes: &[u8],
) -> Result<Vec<AbiObservationSummaryV2>, ProtectedWorkerV2FinalizerLineageDecodeErrorV2> {
    let mut reader = FinalizerLineageReaderV2::new(bytes);
    let _metadata_major = reader.u32()?;
    let _metadata_minor = reader.u32()?;
    let count = bounded_observation_count(reader.u64()?)?;
    let mut summaries = Vec::with_capacity(count);
    for _ in 0..count {
        let entry = reader.text_u64(MAX_HSACO_BYTES)?.to_owned();
        let descriptor = reader.text_u64(MAX_HSACO_BYTES)?.to_owned();
        let kernarg_segment_size = reader.u64()?;
        let kernarg_segment_alignment = reader.u64()?;
        skip_optional_u64(&mut reader)?;
        let _implicit_argument_size = reader.u64()?;
        let explicit_count = bounded_observation_count(reader.u64()?)?;
        for _ in 0..explicit_count {
            skip_optional_text_u64(&mut reader)?;
            skip_optional_text_u64(&mut reader)?;
            reader.take(16)?;
            skip_optional_u64(&mut reader)?;
            reader.u8()?;
            for _ in 0..4 {
                skip_optional_tag(&mut reader)?;
            }
            skip_optional_u64(&mut reader)?;
            for _ in 0..4 {
                skip_optional_bool(&mut reader)?;
            }
        }
        let hidden_count = bounded_observation_count(reader.u64()?)?;
        for _ in 0..hidden_count {
            reader.take(17)?;
        }
        summaries.push(AbiObservationSummaryV2 {
            entry,
            descriptor,
            kernarg_segment_size,
            kernarg_segment_alignment,
        });
    }
    if !reader.finished() {
        return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::TrailingBytes);
    }
    Ok(summaries)
}

fn decode_resource_observation_summaries_v2(
    bytes: &[u8],
) -> Result<Vec<ResourceObservationSummaryV2>, ProtectedWorkerV2FinalizerLineageDecodeErrorV2> {
    let mut reader = FinalizerLineageReaderV2::new(bytes);
    let count = bounded_observation_count(reader.u64()?)?;
    let mut summaries = Vec::with_capacity(count);
    for _ in 0..count {
        let entry = reader.text_u64(MAX_HSACO_BYTES)?.to_owned();
        let group_segment_fixed_size = reader.u64()?;
        let _private_segment_fixed_size = reader.u64()?;
        let _wavefront_size = reader.u32()?;
        reader.take(4)?;
        for _ in 0..3 {
            skip_optional_u32(&mut reader)?;
        }
        let max_flat_workgroup_size = reader.u32()?;
        let required_workgroup_size = decode_optional_dimensions(&mut reader)?;
        for _ in 0..3 {
            skip_optional_u32(&mut reader)?;
        }
        let _cluster_dims = decode_optional_dimensions(&mut reader)?;
        for _ in 0..3 {
            skip_optional_bool(&mut reader)?;
        }
        summaries.push(ResourceObservationSummaryV2 {
            entry,
            group_segment_fixed_size,
            max_flat_workgroup_size,
            required_workgroup_size,
        });
    }
    if !reader.finished() {
        return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::TrailingBytes);
    }
    Ok(summaries)
}

fn bounded_observation_count(
    value: u64,
) -> Result<usize, ProtectedWorkerV2FinalizerLineageDecodeErrorV2> {
    let value = usize::try_from(value)
        .map_err(|_| ProtectedWorkerV2FinalizerLineageDecodeErrorV2::Length)?;
    if value > MAX_WORKER_SYMBOLS {
        return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::Length);
    }
    Ok(value)
}

impl<'a> FinalizerLineageReaderV2<'a> {
    fn text_u64(
        &mut self,
        maximum: usize,
    ) -> Result<&'a str, ProtectedWorkerV2FinalizerLineageDecodeErrorV2> {
        let length = usize::try_from(self.u64()?)
            .map_err(|_| ProtectedWorkerV2FinalizerLineageDecodeErrorV2::Length)?;
        self.text(length, maximum)
    }
}

fn optional_tag(
    reader: &mut FinalizerLineageReaderV2<'_>,
) -> Result<bool, ProtectedWorkerV2FinalizerLineageDecodeErrorV2> {
    match reader.u8()? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::Tag),
    }
}

fn skip_optional_text_u64(
    reader: &mut FinalizerLineageReaderV2<'_>,
) -> Result<(), ProtectedWorkerV2FinalizerLineageDecodeErrorV2> {
    if optional_tag(reader)? {
        reader.text_u64(MAX_HSACO_BYTES)?;
    }
    Ok(())
}

fn skip_optional_u64(
    reader: &mut FinalizerLineageReaderV2<'_>,
) -> Result<(), ProtectedWorkerV2FinalizerLineageDecodeErrorV2> {
    if optional_tag(reader)? {
        reader.u64()?;
    }
    Ok(())
}

fn skip_optional_u32(
    reader: &mut FinalizerLineageReaderV2<'_>,
) -> Result<(), ProtectedWorkerV2FinalizerLineageDecodeErrorV2> {
    if optional_tag(reader)? {
        reader.u32()?;
    }
    Ok(())
}

fn skip_optional_tag(
    reader: &mut FinalizerLineageReaderV2<'_>,
) -> Result<(), ProtectedWorkerV2FinalizerLineageDecodeErrorV2> {
    if optional_tag(reader)? {
        reader.u8()?;
    }
    Ok(())
}

fn skip_optional_bool(
    reader: &mut FinalizerLineageReaderV2<'_>,
) -> Result<(), ProtectedWorkerV2FinalizerLineageDecodeErrorV2> {
    if optional_tag(reader)? && reader.u8()? > 1 {
        return Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::Tag);
    }
    Ok(())
}

fn decode_optional_dimensions(
    reader: &mut FinalizerLineageReaderV2<'_>,
) -> Result<Option<[u32; 3]>, ProtectedWorkerV2FinalizerLineageDecodeErrorV2> {
    if optional_tag(reader)? {
        Ok(Some([reader.u32()?, reader.u32()?, reader.u32()?]))
    } else {
        Ok(None)
    }
}

/// Failure while turning admitted raw Worker V2 output into inert canonical-finalization evidence.
#[derive(Debug)]
#[non_exhaustive]
pub enum WorkerV2HsacoFinalizationError {
    RawOutputIdentityMismatch,
    MissingAuthenticatedDescriptorSourceEvidence(
        Box<MissingAuthenticatedDescriptorSourceEvidenceV1>,
    ),
    MissingAuthenticatedProtectedDescriptorSourceEvidence(
        Box<MissingAuthenticatedProtectedDescriptorSourceEvidenceV2>,
    ),
    CanonicalFinalization(FinalizationError),
    FinalizedVerification(FinalizationError),
    FinalizedInspectionMismatch,
    TargetMismatch,
    CodeObjectVersionMismatch,
    KernelClosureMismatch,
    LaunchContractMismatch {
        kernel: String,
    },
    FinalizedOutputIdentityMismatch,
}

impl fmt::Display for WorkerV2HsacoFinalizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RawOutputIdentityMismatch => formatter
                .write_str("retained raw HSACO bytes do not match their admitted output identity"),
            Self::MissingAuthenticatedDescriptorSourceEvidence(_) => write!(
                formatter,
                "raw Worker V2 HSACO has no {DEVICE_DESCRIPTOR_SECTION_NAME}; canonical \
                 finalization requires authenticated descriptor-source evidence and will not \
                 infer Rust ABI, layout, effect, or build-evidence claims from executable metadata"
            ),
            Self::MissingAuthenticatedProtectedDescriptorSourceEvidence(_) => write!(
                formatter,
                "protected raw Worker V2 HSACO has no {DEVICE_DESCRIPTOR_SECTION_NAME}; \
                 canonical finalization requires authenticated descriptor-source evidence and \
                 will not infer Rust ABI, layout, effect, or build-evidence claims from \
                 executable metadata"
            ),
            Self::CanonicalFinalization(error) => {
                write!(
                    formatter,
                    "canonical Worker V2 HSACO finalization failed: {error}"
                )
            }
            Self::FinalizedVerification(error) => write!(
                formatter,
                "independent Worker V2 HSACO finalization verification failed: {error}"
            ),
            Self::FinalizedInspectionMismatch => formatter.write_str(
                "independent finalized inspection differs from the finalizer inspection",
            ),
            Self::TargetMismatch => formatter
                .write_str("finalized descriptor and metadata target differs from raw policy"),
            Self::CodeObjectVersionMismatch => formatter.write_str(
                "finalized descriptor and metadata code-object version differs from raw policy",
            ),
            Self::KernelClosureMismatch => formatter
                .write_str("finalized kernel entry/descriptor closure differs from raw policy"),
            Self::LaunchContractMismatch { kernel } => write!(
                formatter,
                "finalized kernel {kernel} launch metadata differs from raw policy"
            ),
            Self::FinalizedOutputIdentityMismatch => formatter.write_str(
                "finalized HSACO bytes do not match their derived content identity or digest",
            ),
        }
    }
}

impl Error for WorkerV2HsacoFinalizationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CanonicalFinalization(error) | Self::FinalizedVerification(error) => Some(error),
            _ => None,
        }
    }
}

/// Consumes admitted raw Worker V2 output and runs canonical `.fe2o3.kd.v1` finalization.
///
/// All policy facts are recovered from the consumed inspection. No target, identity, descriptor,
/// or launch claim is accepted from the caller.
pub fn finalize_inspected_worker_v2_hsaco_v1(
    raw: InspectedRawWorkerV2HsacoV1,
) -> Result<PreparedFinalizedWorkerV2HsacoV1, WorkerV2HsacoFinalizationError> {
    finalize_inspected_worker_v2_hsaco_with_placement_v1(raw, false)
}

/// Consumes protected raw inspection exactly once and runs canonical descriptor finalization.
///
/// The route preserves the exact V2 handoff slot/identity and complete compiler closure. It does
/// not convert protected evidence to V1 or grant publication, loading, or launch authority.
pub fn finalize_inspected_protected_worker_v2_hsaco_v2(
    raw: InspectedProtectedRawWorkerV2HsacoV1,
) -> Result<PreparedFinalizedProtectedWorkerV2HsacoV2, WorkerV2HsacoFinalizationError> {
    let Some(core) = finalize_worker_v2_hsaco_shared(&raw, false)? else {
        return Err(
            WorkerV2HsacoFinalizationError::MissingAuthenticatedProtectedDescriptorSourceEvidence(
                Box::new(MissingAuthenticatedProtectedDescriptorSourceEvidenceV2 { raw }),
            ),
        );
    };
    let identity =
        calculate_protected_finalized_identity(&raw, &core.finalized, core.finalized_output);
    Ok(PreparedFinalizedProtectedWorkerV2HsacoV2 {
        identity,
        raw,
        finalized: core.finalized,
        finalized_output: core.finalized_output,
    })
}

#[cfg(feature = "general-gemm-v1")]
pub(crate) fn finalize_allocated_general_gemm_worker_v2_hsaco_v1(
    raw: InspectedRawWorkerV2HsacoV1,
) -> Result<PreparedFinalizedWorkerV2HsacoV1, WorkerV2HsacoFinalizationError> {
    finalize_inspected_worker_v2_hsaco_with_placement_v1(raw, true)
}

fn finalize_inspected_worker_v2_hsaco_with_placement_v1(
    raw: InspectedRawWorkerV2HsacoV1,
    allocated_read_only: bool,
) -> Result<PreparedFinalizedWorkerV2HsacoV1, WorkerV2HsacoFinalizationError> {
    let Some(core) = finalize_worker_v2_hsaco_shared(&raw, allocated_read_only)? else {
        return Err(
            WorkerV2HsacoFinalizationError::MissingAuthenticatedDescriptorSourceEvidence(Box::new(
                MissingAuthenticatedDescriptorSourceEvidenceV1 { raw },
            )),
        );
    };
    let identity = calculate_finalized_identity(&raw, &core.finalized, core.finalized_output);
    Ok(PreparedFinalizedWorkerV2HsacoV1 {
        identity,
        raw,
        finalized: core.finalized,
        finalized_output: core.finalized_output,
    })
}

struct SharedCanonicalFinalizationV1 {
    finalized: FinalizedHsaco,
    finalized_output: ContentIdentityV1,
}

trait WorkerV2HsacoFinalizationSourceV1 {
    fn exact_bytes(&self) -> &[u8];
    fn linked_output_identity(&self) -> ContentIdentityV1;
    fn canonical_descriptor_section(&self) -> CanonicalDescriptorSectionObservationV1;
    fn target(&self) -> DeviceTargetV1;
    fn code_object_version(&self) -> CodeObjectVersion;
    fn policy(&self) -> &WorkerV2RawHsacoPolicyV1;
}

macro_rules! impl_finalization_source {
    ($source:ty) => {
        impl WorkerV2HsacoFinalizationSourceV1 for $source {
            fn exact_bytes(&self) -> &[u8] {
                self.exact_bytes()
            }

            fn linked_output_identity(&self) -> ContentIdentityV1 {
                self.linked_output_identity()
            }

            fn canonical_descriptor_section(&self) -> CanonicalDescriptorSectionObservationV1 {
                self.canonical_descriptor_section()
            }

            fn target(&self) -> DeviceTargetV1 {
                self.target()
            }

            fn code_object_version(&self) -> CodeObjectVersion {
                self.code_object_version()
            }

            fn policy(&self) -> &WorkerV2RawHsacoPolicyV1 {
                self.policy()
            }
        }
    };
}

impl_finalization_source!(InspectedRawWorkerV2HsacoV1);
impl_finalization_source!(InspectedProtectedRawWorkerV2HsacoV1);

fn finalize_worker_v2_hsaco_shared(
    raw: &impl WorkerV2HsacoFinalizationSourceV1,
    allocated_read_only: bool,
) -> Result<Option<SharedCanonicalFinalizationV1>, WorkerV2HsacoFinalizationError> {
    let raw_bytes = raw.exact_bytes();
    if !raw.linked_output_identity().matches(raw_bytes) {
        return Err(WorkerV2HsacoFinalizationError::RawOutputIdentityMismatch);
    }
    if raw.canonical_descriptor_section() == CanonicalDescriptorSectionObservationV1::Missing {
        return Ok(None);
    }

    let finalized = if allocated_read_only {
        finalize_allocated_read_only_unfinalized(raw_bytes)
    } else {
        finalize_unfinalized(raw_bytes)
    }
    .map_err(WorkerV2HsacoFinalizationError::CanonicalFinalization)?;
    let verified = if allocated_read_only {
        verify_allocated_read_only_finalized(finalized.as_bytes())
    } else {
        verify_finalized(finalized.as_bytes())
    }
    .map_err(WorkerV2HsacoFinalizationError::FinalizedVerification)?;
    if &verified != finalized.inspection() {
        return Err(WorkerV2HsacoFinalizationError::FinalizedInspectionMismatch);
    }

    validate_metadata_lineage(raw, &finalized)?;
    let finalized_output = ContentIdentityV1::calculate(finalized.as_bytes());
    if !finalized_output.matches(finalized.as_bytes())
        || verified.digest().as_bytes() == &[0; 32]
        || verified.digest() != finalized.inspection().digest()
    {
        return Err(WorkerV2HsacoFinalizationError::FinalizedOutputIdentityMismatch);
    }
    Ok(Some(SharedCanonicalFinalizationV1 {
        finalized,
        finalized_output,
    }))
}

fn validate_metadata_lineage(
    raw: &impl WorkerV2HsacoFinalizationSourceV1,
    finalized: &FinalizedHsaco,
) -> Result<(), WorkerV2HsacoFinalizationError> {
    validate_metadata_lineage_parts(
        raw.target(),
        raw.code_object_version(),
        raw.policy(),
        finalized,
    )
}

fn validate_metadata_lineage_parts(
    target: DeviceTargetV1,
    code_object_version: CodeObjectVersion,
    policy: &WorkerV2RawHsacoPolicyV1,
    finalized: &FinalizedHsaco,
) -> Result<(), WorkerV2HsacoFinalizationError> {
    let inspection = finalized.inspection();
    let metadata = inspection.hsaco();
    if inspection.descriptor_table().device_target() != target
        || metadata.target() != target.as_amd_target_id()
    {
        return Err(WorkerV2HsacoFinalizationError::TargetMismatch);
    }
    if inspection.descriptor_table().code_object_version() != code_object_version
        || map_code_object_version(metadata.code_object_version()) != code_object_version
    {
        return Err(WorkerV2HsacoFinalizationError::CodeObjectVersionMismatch);
    }

    let mut observed: Vec<_> = metadata
        .kernels()
        .iter()
        .map(|kernel| (kernel.name(), kernel.symbol()))
        .collect();
    observed.sort_unstable();
    let expected: Vec<_> = policy
        .observed_kernels()
        .iter()
        .map(|kernel| (kernel.entry(), kernel.descriptor()))
        .collect();
    if observed != expected {
        return Err(WorkerV2HsacoFinalizationError::KernelClosureMismatch);
    }

    let launch = policy.launch();
    for kernel in metadata.kernels() {
        if kernel.required_workgroup_size() != Some(launch.required_workgroup_size())
            || kernel.max_flat_workgroup_size() != launch.max_flat_workgroup_size()
            || kernel.wavefront_size() != launch.wavefront_size()
        {
            return Err(WorkerV2HsacoFinalizationError::LaunchContractMismatch {
                kernel: kernel.name().to_owned(),
            });
        }
    }
    Ok(())
}

fn calculate_finalized_identity(
    raw: &InspectedRawWorkerV2HsacoV1,
    finalized: &FinalizedHsaco,
    finalized_output: ContentIdentityV1,
) -> FinalizedWorkerV2HsacoIdentityV1 {
    let mut hasher = Sha256::new();
    hasher.update(FINALIZED_IDENTITY_DOMAIN_V1);
    // V1 is structural-only and deliberately records the absence of authenticated source claims.
    hasher.update([0]);
    hasher.update(raw.identity().as_bytes());
    hasher.update(raw.source_evidence_identity().as_bytes());
    hasher.update(raw.policy().identity().as_bytes());
    hash_content(&mut hasher, raw.linked_output_identity());
    hash_content(&mut hasher, finalized_output);
    hasher.update(finalized.inspection().digest().as_bytes());
    hash_text(&mut hasher, &raw.target().to_string());
    hasher.update([code_object_version_tag(raw.code_object_version())]);
    hasher.update((raw.policy().observed_kernels().len() as u64).to_le_bytes());
    for kernel in raw.policy().observed_kernels() {
        hash_kernel(&mut hasher, kernel);
    }
    FinalizedWorkerV2HsacoIdentityV1(hasher.finalize().into())
}

fn calculate_protected_finalized_identity(
    raw: &InspectedProtectedRawWorkerV2HsacoV1,
    finalized: &FinalizedHsaco,
    finalized_output: ContentIdentityV1,
) -> FinalizedProtectedWorkerV2HsacoIdentityV2 {
    FinalizedProtectedWorkerV2HsacoIdentityV2(calculate_protected_finalized_identity_v2(
        &ProtectedFinalizationIdentityPreimageV2 {
            raw_inspection_identity: *raw.identity().as_bytes(),
            source_evidence_identity: *raw.source_evidence_identity().as_bytes(),
            handoff_slot: raw.handoff_slot(),
            handoff_identity: raw.handoff_identity(),
            compiler_closure: raw.compiler_closure(),
            policy: raw.policy(),
            raw_output: raw.linked_output_identity(),
            finalized_output,
            canonical_digest: finalized.inspection().digest(),
        },
    ))
}

struct ProtectedFinalizationIdentityPreimageV2<'a> {
    raw_inspection_identity: [u8; 32],
    source_evidence_identity: [u8; 32],
    handoff_slot: CompilerModuleHandoffSlotV2,
    handoff_identity: CompilerModuleHandoffIdentityV2,
    compiler_closure: CompilerClosureV2,
    policy: &'a WorkerV2RawHsacoPolicyV1,
    raw_output: ContentIdentityV1,
    finalized_output: ContentIdentityV1,
    canonical_digest: CanonicalCodeObjectDigest,
}

fn calculate_protected_finalized_identity_v2(
    preimage: &ProtectedFinalizationIdentityPreimageV2<'_>,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(PROTECTED_FINALIZED_IDENTITY_DOMAIN_V2);
    // This structural bridge still records the absence of authenticated source claims.
    hasher.update([0]);
    hash_domain_component(
        &mut hasher,
        PROTECTED_FINALIZED_INSPECTION_DOMAIN_V2,
        |component| component.update(preimage.raw_inspection_identity),
    );
    hash_domain_component(
        &mut hasher,
        PROTECTED_FINALIZED_SOURCE_DOMAIN_V2,
        |component| component.update(preimage.source_evidence_identity),
    );
    hash_domain_component(
        &mut hasher,
        PROTECTED_FINALIZED_HANDOFF_SLOT_DOMAIN_V2,
        |component| component.update([preimage.handoff_slot as u8]),
    );
    hash_domain_component(
        &mut hasher,
        PROTECTED_FINALIZED_HANDOFF_IDENTITY_DOMAIN_V2,
        |component| component.update(preimage.handoff_identity.as_bytes()),
    );
    hash_domain_component(
        &mut hasher,
        PROTECTED_FINALIZED_COMPILER_CLOSURE_DOMAIN_V2,
        |component| hash_compiler_closure_v2(component, preimage.compiler_closure),
    );
    hasher.update(preimage.policy.identity().as_bytes());
    hash_content(&mut hasher, preimage.raw_output);
    hash_content(&mut hasher, preimage.finalized_output);
    hasher.update(preimage.canonical_digest.as_bytes());
    hash_text(&mut hasher, &preimage.policy.target().to_string());
    hasher.update([code_object_version_tag(
        preimage.policy.code_object_version(),
    )]);
    hasher.update((preimage.policy.observed_kernels().len() as u64).to_le_bytes());
    for kernel in preimage.policy.observed_kernels() {
        hash_kernel(&mut hasher, kernel);
    }
    hasher.finalize().into()
}

fn hash_domain_component(hasher: &mut Sha256, domain: &[u8], update: impl FnOnce(&mut Sha256)) {
    let mut component = Sha256::new();
    component.update(domain);
    update(&mut component);
    hasher.update(component.finalize());
}

fn hash_compiler_closure_v2(hasher: &mut Sha256, closure: CompilerClosureV2) {
    hasher.update(
        closure
            .cargo_binding_transition_protocol_version()
            .to_le_bytes(),
    );
    hasher.update(closure.cargo_executable_sha256());
    hasher.update(closure.cargo_binding_trampoline_sha256());
    hasher.update(closure.cargo_fe2o3_binding_wrapper_sha256());
    hasher.update(closure.rustc_executable_sha256());
    hasher.update(closure.rustc_runtime_tree_sha256());
    hasher.update(closure.codegen_backend_sha256());
    hasher.update(closure.identity_sha256());
}

fn hash_content(hasher: &mut Sha256, identity: ContentIdentityV1) {
    hasher.update(identity.sha256());
    hasher.update(identity.byte_len().to_le_bytes());
}

fn hash_kernel(hasher: &mut Sha256, kernel: &ObservedWorkerV2KernelSymbolsV1) {
    hash_text(hasher, kernel.entry());
    hash_text(hasher, kernel.descriptor());
}

fn hash_text(hasher: &mut Sha256, value: &str) {
    hasher.update((value.len() as u64).to_le_bytes());
    hasher.update(value.as_bytes());
}

const fn map_code_object_version(version: InspectedCodeObjectVersion) -> CodeObjectVersion {
    match version {
        InspectedCodeObjectVersion::V4 => CodeObjectVersion::V4,
        InspectedCodeObjectVersion::V5 => CodeObjectVersion::V5,
        InspectedCodeObjectVersion::V6 => CodeObjectVersion::V6,
    }
}

const fn code_object_version_tag(version: CodeObjectVersion) -> u8 {
    match version {
        CodeObjectVersion::V4 => 4,
        CodeObjectVersion::V5 => 5,
        CodeObjectVersion::V6 => 6,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_PROTECTED_WORKER_V2_FINALIZER_LINEAGE_BYTES_V2, PROTECTED_LINEAGE_FIXED_BYTES_V2,
        ProtectedWorkerV2FinalizerLineageDecodeErrorV2, validate_protected_lineage_encoded_length,
        validate_sorted_descriptor_entries_v2,
    };

    #[test]
    fn aggregate_lineage_bound_is_exact_without_allocating_the_limit() {
        let maximum_variable_bytes =
            MAX_PROTECTED_WORKER_V2_FINALIZER_LINEAGE_BYTES_V2 - PROTECTED_LINEAGE_FIXED_BYTES_V2;
        assert!(
            validate_protected_lineage_encoded_length([maximum_variable_bytes]).is_ok(),
            "the exact aggregate maximum must remain encodable"
        );
        assert!(matches!(
            validate_protected_lineage_encoded_length([maximum_variable_bytes, 1]),
            Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::Length)
        ));
        assert!(matches!(
            validate_protected_lineage_encoded_length([usize::MAX]),
            Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::Length)
        ));
    }

    #[test]
    fn bounded_multi_kernel_entry_join_is_total_and_rejects_aliases() {
        let kernels = ["attention", "gemm", "moe", "softmax"];
        assert!(
            validate_sorted_descriptor_entries_v2(
                kernels.into_iter(),
                kernels.into_iter(),
                kernels.into_iter(),
            )
            .is_ok()
        );

        let wrong_resource = ["attention", "gemm", "moe", "transpose"];
        assert!(matches!(
            validate_sorted_descriptor_entries_v2(
                kernels.into_iter(),
                kernels.into_iter(),
                wrong_resource.into_iter(),
            ),
            Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::DescriptorJoin("kernel entry"))
        ));

        let duplicate = ["attention", "gemm", "gemm", "softmax"];
        assert!(matches!(
            validate_sorted_descriptor_entries_v2(
                duplicate.into_iter(),
                duplicate.into_iter(),
                duplicate.into_iter(),
            ),
            Err(
                ProtectedWorkerV2FinalizerLineageDecodeErrorV2::DescriptorJoin(
                    "duplicate kernel entry"
                )
            )
        ));
    }
}
