//! Canonical descriptor finalization for independently inspected Worker V3 HSACO.
//!
//! This boundary consumes raw inspection evidence exactly once, runs the canonical
//! `.fe2o3.kd.v1` finalizer, and retains both sides of the resulting lineage. The prepared value
//! remains descriptive evidence: it authenticates neither the compiler nor Verus and grants no
//! publication, HSA loading, or launch authority.

use std::{error::Error, fmt};

use fe2o3_artifact_transaction::{
    BuildAttempt, CompilerModuleHandoffSlotV3, CompilerModuleHandoffTransactionIdentityV3,
};
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_compiler_ffi::{
    CompilerDescriptorSourceErrorV1, CompilerDescriptorSourceV1,
    InertSemanticCompilerModuleHandoffIdentityV3, InertSemanticCompilerModuleHandoffV3,
};
use fe2o3_hsaco::CodeObjectVersion as InspectedCodeObjectVersion;
use fe2o3_kernel_descriptor::{
    CANONICAL_CODE_OBJECT_DIGEST_OFFSET, CanonicalCodeObjectDigest, CodeObjectVersion,
    DeviceTargetV1, encode_device_descriptor_table_v1,
};
use sha2::{Digest, Sha256};

use crate::{
    CanonicalDescriptorSectionObservationV1, ContentIdentityV1, DEVICE_DESCRIPTOR_SECTION_NAME,
    FinalizationError, FinalizedHsaco, InertProtectedFirstBuildWorkerV3EvidenceV1,
    InspectedProtectedWorkerV3HsacoIdentityV1, InspectedProtectedWorkerV3HsacoV1,
    MultiInputLinkPlanV1, ObservedWorkerKernelSymbolsV1, ProtectedCompilerHandoffBindingIdentityV3,
    ProtectedCompilerHandoffExpectationV3, ProtectedFirstBuildWorkerV3IdentityV1,
    WorkerExecutionLimitsV1, WorkerMeasurementV1, WorkerV3HsacoPolicyIdentityV1,
    WorkerV3HsacoPolicyV1, finalize_allocated_read_only_unfinalized, finalize_unfinalized,
    verify_allocated_read_only_finalized, verify_finalized,
};

const PROTECTED_FINALIZED_IDENTITY_DOMAIN_V3: &[u8] =
    b"FE2O3/STRICT-V3-PROTECTED-WORKER-CANONICAL-FINALIZATION/V1\0";

/// Stable native-V3 identity of one protected canonical-finalization transition.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FinalizedProtectedWorkerV3HsacoIdentityV1([u8; 32]);

impl FinalizedProtectedWorkerV3HsacoIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    #[cfg(test)]
    pub(crate) const fn from_test_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

/// The exact evidence class needed before a missing descriptor table may be constructed.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DescriptorSourceEvidenceRequirementV1 {
    /// Authenticated compiler-derived source, ABI, layout, effect, and build-evidence claims.
    AuthenticatedCanonicalDescriptorTableV1,
}

/// Owning fail-closed V3 record for protected output with no canonical descriptor table.
///
/// The strict transaction, semantic outer handoff, compiler closure, measured worker, and exact
/// link execution remain retained as native V3 evidence. No descriptor claims are inferred from
/// executable metadata and no legacy fallback is attempted.
#[derive(Debug)]
pub struct MissingAuthenticatedProtectedDescriptorSourceEvidenceV3 {
    raw: InspectedProtectedWorkerV3HsacoV1,
}

impl MissingAuthenticatedProtectedDescriptorSourceEvidenceV3 {
    pub const fn requirement(&self) -> DescriptorSourceEvidenceRequirementV1 {
        DescriptorSourceEvidenceRequirementV1::AuthenticatedCanonicalDescriptorTableV1
    }

    pub const fn raw_inspection_identity(&self) -> InspectedProtectedWorkerV3HsacoIdentityV1 {
        self.raw.identity()
    }

    pub const fn source_evidence_identity(&self) -> ProtectedFirstBuildWorkerV3IdentityV1 {
        self.raw.source_evidence_identity()
    }

    pub const fn binding_identity(&self) -> ProtectedCompilerHandoffBindingIdentityV3 {
        self.raw.binding_identity()
    }

    pub const fn binding_expectation(&self) -> ProtectedCompilerHandoffExpectationV3 {
        self.raw.binding_expectation()
    }

    pub const fn raw_output_identity(&self) -> ContentIdentityV1 {
        self.raw.linked_output_identity()
    }

    pub const fn policy_identity(&self) -> WorkerV3HsacoPolicyIdentityV1 {
        self.raw.policy().identity()
    }

    pub const fn attempt(&self) -> BuildAttempt {
        self.raw.attempt()
    }

    pub const fn handoff_slot(&self) -> CompilerModuleHandoffSlotV3 {
        self.raw.handoff_slot()
    }

    pub const fn transaction_identity(&self) -> CompilerModuleHandoffTransactionIdentityV3 {
        self.raw.transaction_identity()
    }

    pub const fn outer_handoff_identity(&self) -> InertSemanticCompilerModuleHandoffIdentityV3 {
        self.raw.outer_handoff_identity()
    }

    pub const fn outer_handoff(&self) -> &InertSemanticCompilerModuleHandoffV3 {
        self.raw.outer_handoff()
    }

    pub const fn compiler_closure(&self) -> CompilerClosureV2 {
        self.raw.compiler_closure()
    }

    pub const fn worker_measurement(&self) -> &WorkerMeasurementV1 {
        self.raw.worker_measurement()
    }

    pub const fn execution_limits(&self) -> WorkerExecutionLimitsV1 {
        self.raw.source_evidence().execution_limits()
    }

    pub const fn link_plan_identity(&self) -> crate::LinkPlanIdentityV1 {
        self.raw.link_plan_identity()
    }

    pub const fn link_plan(&self) -> &MultiInputLinkPlanV1 {
        self.raw.plan()
    }

    pub const fn target(&self) -> DeviceTargetV1 {
        self.raw.target()
    }

    pub const fn code_object_version(&self) -> CodeObjectVersion {
        self.raw.code_object_version()
    }

    pub fn observed_kernels(&self) -> &[ObservedWorkerKernelSymbolsV1] {
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

/// Opaque native-V3 HSACO after structural canonical descriptor finalization.
///
/// This move-only owner retains the exact V3 inspection and the independently verified canonical
/// finalizer output. Its identity binds every retained transaction, semantic, compiler, worker,
/// plan, raw-output, descriptor, ABI, resource, and finalized-output axis. It remains descriptive
/// evidence and grants no compiler, publication, load, or launch authority.
#[derive(Debug)]
pub struct PreparedFinalizedProtectedWorkerV3HsacoV1 {
    identity: FinalizedProtectedWorkerV3HsacoIdentityV1,
    raw: InspectedProtectedWorkerV3HsacoV1,
    finalized: FinalizedHsaco,
    finalized_output: ContentIdentityV1,
    canonical_descriptor_evidence: ContentIdentityV1,
}

pub(crate) struct OwnedPreparedFinalizedProtectedWorkerV3ReplayPartsV1 {
    pub(crate) identity: FinalizedProtectedWorkerV3HsacoIdentityV1,
    pub(crate) source: InertProtectedFirstBuildWorkerV3EvidenceV1,
    pub(crate) finalized_bytes: Vec<u8>,
}

impl PreparedFinalizedProtectedWorkerV3HsacoV1 {
    pub const fn identity(&self) -> FinalizedProtectedWorkerV3HsacoIdentityV1 {
        self.identity
    }

    pub const fn raw_inspection_identity(&self) -> InspectedProtectedWorkerV3HsacoIdentityV1 {
        self.raw.identity()
    }

    pub const fn source_evidence_identity(&self) -> ProtectedFirstBuildWorkerV3IdentityV1 {
        self.raw.source_evidence_identity()
    }

    pub(crate) const fn source_evidence(&self) -> &InertProtectedFirstBuildWorkerV3EvidenceV1 {
        self.raw.source_evidence()
    }

    pub const fn binding_identity(&self) -> ProtectedCompilerHandoffBindingIdentityV3 {
        self.raw.binding_identity()
    }

    pub const fn binding_expectation(&self) -> ProtectedCompilerHandoffExpectationV3 {
        self.raw.binding_expectation()
    }

    pub const fn attempt(&self) -> BuildAttempt {
        self.raw.attempt()
    }

    pub const fn handoff_slot(&self) -> CompilerModuleHandoffSlotV3 {
        self.raw.handoff_slot()
    }

    pub const fn transaction_identity(&self) -> CompilerModuleHandoffTransactionIdentityV3 {
        self.raw.transaction_identity()
    }

    pub const fn outer_handoff_identity(&self) -> InertSemanticCompilerModuleHandoffIdentityV3 {
        self.raw.outer_handoff_identity()
    }

    pub const fn outer_handoff(&self) -> &InertSemanticCompilerModuleHandoffV3 {
        self.raw.outer_handoff()
    }

    pub const fn compiler_closure(&self) -> CompilerClosureV2 {
        self.raw.compiler_closure()
    }

    pub const fn worker_measurement(&self) -> &WorkerMeasurementV1 {
        self.raw.worker_measurement()
    }

    pub const fn execution_limits(&self) -> WorkerExecutionLimitsV1 {
        self.raw.source_evidence().execution_limits()
    }

    pub const fn link_plan_identity(&self) -> crate::LinkPlanIdentityV1 {
        self.raw.link_plan_identity()
    }

    pub const fn link_plan(&self) -> &MultiInputLinkPlanV1 {
        self.raw.plan()
    }

    pub const fn raw_output_identity(&self) -> ContentIdentityV1 {
        self.raw.linked_output_identity()
    }

    pub const fn finalized_output_identity(&self) -> ContentIdentityV1 {
        self.finalized_output
    }

    /// Identity of the exact canonical descriptor-table bytes reparsed from finalized HSACO.
    pub const fn canonical_descriptor_evidence_identity(&self) -> ContentIdentityV1 {
        self.canonical_descriptor_evidence
    }

    pub const fn policy_identity(&self) -> WorkerV3HsacoPolicyIdentityV1 {
        self.raw.policy().identity()
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

    pub(crate) fn into_compact_replay_parts(
        self,
    ) -> OwnedPreparedFinalizedProtectedWorkerV3ReplayPartsV1 {
        let Self {
            identity,
            raw,
            finalized,
            finalized_output: _,
            canonical_descriptor_evidence: _,
        } = self;
        OwnedPreparedFinalizedProtectedWorkerV3ReplayPartsV1 {
            identity,
            source: raw.into_source_evidence(),
            finalized_bytes: finalized.into_bytes(),
        }
    }
}

/// Failure while turning admitted raw Worker V3 output into inert canonical-finalization evidence.
#[derive(Debug)]
#[non_exhaustive]
pub enum WorkerV3HsacoFinalizationError {
    RawOutputIdentityMismatch,
    MissingAuthenticatedProtectedDescriptorSourceEvidenceV3(
        Box<MissingAuthenticatedProtectedDescriptorSourceEvidenceV3>,
    ),
    CanonicalFinalization(FinalizationError),
    FinalizedVerification(FinalizationError),
    CanonicalDescriptorEvidence(fe2o3_kernel_descriptor::ValidationError),
    CompilerDescriptorSource(CompilerDescriptorSourceErrorV1),
    CompilerDescriptorSourceMismatch,
    ExportManifestMismatch,
    FinalizedInspectionMismatch,
    TargetMismatch,
    CodeObjectVersionMismatch,
    KernelClosureMismatch,
    LaunchContractMismatch {
        kernel: String,
    },
    FinalizedOutputIdentityMismatch,
}

impl fmt::Display for WorkerV3HsacoFinalizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RawOutputIdentityMismatch => formatter
                .write_str("retained raw HSACO bytes do not match their admitted output identity"),
            Self::MissingAuthenticatedProtectedDescriptorSourceEvidenceV3(_) => write!(
                formatter,
                "strict-V3 protected raw Worker HSACO has no {DEVICE_DESCRIPTOR_SECTION_NAME}; \
                 canonical finalization requires authenticated descriptor-source evidence and \
                 will not infer Rust ABI, layout, effect, or build-evidence claims from \
                 executable metadata"
            ),
            Self::CanonicalFinalization(error) => {
                write!(
                    formatter,
                    "canonical Worker V3 HSACO finalization failed: {error}"
                )
            }
            Self::FinalizedVerification(error) => write!(
                formatter,
                "independent Worker V3 HSACO finalization verification failed: {error}"
            ),
            Self::CanonicalDescriptorEvidence(error) => write!(
                formatter,
                "canonical finalized descriptor evidence could not be encoded: {error}"
            ),
            Self::CompilerDescriptorSource(error) => {
                write!(formatter, "strict-V3 compiler descriptor source is invalid: {error}")
            }
            Self::CompilerDescriptorSourceMismatch => formatter.write_str(
                "zero-normalized finalized descriptor differs from the exact strict-V3 compiler source",
            ),
            Self::ExportManifestMismatch => formatter.write_str(
                "strict-V3 export receipt differs from the exact compiler-module symbol manifest",
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

impl Error for WorkerV3HsacoFinalizationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CanonicalFinalization(error) | Self::FinalizedVerification(error) => Some(error),
            Self::CanonicalDescriptorEvidence(error) => Some(error),
            Self::CompilerDescriptorSource(error) => Some(error),
            _ => None,
        }
    }
}

/// Consumes native strict-V3 inspection exactly once and runs canonical descriptor finalization.
///
/// The complete V3 transaction, outer semantic handoff, compiler closure, measured worker, and
/// link execution remain retained by the result. This route has no legacy finalization fallback.
pub fn finalize_protected_worker_v3_hsaco_v1(
    raw: InspectedProtectedWorkerV3HsacoV1,
) -> Result<PreparedFinalizedProtectedWorkerV3HsacoV1, WorkerV3HsacoFinalizationError> {
    if raw.canonical_descriptor_section() == CanonicalDescriptorSectionObservationV1::Missing {
        return Err(
            WorkerV3HsacoFinalizationError::MissingAuthenticatedProtectedDescriptorSourceEvidenceV3(
                Box::new(MissingAuthenticatedProtectedDescriptorSourceEvidenceV3 { raw }),
            ),
        );
    }
    let outer = raw.outer_handoff();
    let descriptor_source =
        CompilerDescriptorSourceV1::decode(outer.capsule().receipts().abi().canonical_preimage())
            .map_err(WorkerV3HsacoFinalizationError::CompilerDescriptorSource)?;
    if outer
        .capsule()
        .receipts()
        .export_manifest()
        .canonical_preimage()
        != outer.module_handoff().symbol_manifest().canonical_bytes()
    {
        return Err(WorkerV3HsacoFinalizationError::ExportManifestMismatch);
    }
    let core = finalize_worker_v3_hsaco(&raw, false)?
        .ok_or(WorkerV3HsacoFinalizationError::CompilerDescriptorSourceMismatch)?;
    let descriptor_bytes =
        encode_device_descriptor_table_v1(core.finalized.inspection().descriptor_table())
            .map_err(WorkerV3HsacoFinalizationError::CanonicalDescriptorEvidence)?;
    let digest_end = CANONICAL_CODE_OBJECT_DIGEST_OFFSET + 32;
    let mut zero_normalized_descriptor = descriptor_bytes.clone();
    zero_normalized_descriptor[CANONICAL_CODE_OBJECT_DIGEST_OFFSET..digest_end].fill(0);
    if zero_normalized_descriptor != descriptor_source.canonical_bytes() {
        return Err(WorkerV3HsacoFinalizationError::CompilerDescriptorSourceMismatch);
    }
    let canonical_descriptor_evidence = ContentIdentityV1::calculate(&descriptor_bytes);
    let identity = calculate_protected_v3_finalized_identity(
        &raw,
        &core.finalized,
        core.finalized_output,
        &descriptor_bytes,
        canonical_descriptor_evidence,
    );
    Ok(PreparedFinalizedProtectedWorkerV3HsacoV1 {
        identity,
        raw,
        finalized: core.finalized,
        finalized_output: core.finalized_output,
        canonical_descriptor_evidence,
    })
}

struct SharedCanonicalFinalizationV1 {
    finalized: FinalizedHsaco,
    finalized_output: ContentIdentityV1,
}

fn finalize_worker_v3_hsaco(
    raw: &InspectedProtectedWorkerV3HsacoV1,
    allocated_read_only: bool,
) -> Result<Option<SharedCanonicalFinalizationV1>, WorkerV3HsacoFinalizationError> {
    let raw_bytes = raw.exact_bytes();
    if !raw.linked_output_identity().matches(raw_bytes) {
        return Err(WorkerV3HsacoFinalizationError::RawOutputIdentityMismatch);
    }
    if raw.canonical_descriptor_section() == CanonicalDescriptorSectionObservationV1::Missing {
        return Ok(None);
    }

    let finalized = if allocated_read_only {
        finalize_allocated_read_only_unfinalized(raw_bytes)
    } else {
        finalize_unfinalized(raw_bytes)
    }
    .map_err(WorkerV3HsacoFinalizationError::CanonicalFinalization)?;
    let verified = if allocated_read_only {
        verify_allocated_read_only_finalized(finalized.as_bytes())
    } else {
        verify_finalized(finalized.as_bytes())
    }
    .map_err(WorkerV3HsacoFinalizationError::FinalizedVerification)?;
    if &verified != finalized.inspection() {
        return Err(WorkerV3HsacoFinalizationError::FinalizedInspectionMismatch);
    }

    validate_metadata_lineage(raw, &finalized)?;
    let finalized_output = ContentIdentityV1::calculate(finalized.as_bytes());
    if !finalized_output.matches(finalized.as_bytes())
        || verified.digest().as_bytes() == &[0; 32]
        || verified.digest() != finalized.inspection().digest()
    {
        return Err(WorkerV3HsacoFinalizationError::FinalizedOutputIdentityMismatch);
    }
    Ok(Some(SharedCanonicalFinalizationV1 {
        finalized,
        finalized_output,
    }))
}

fn validate_metadata_lineage(
    raw: &InspectedProtectedWorkerV3HsacoV1,
    finalized: &FinalizedHsaco,
) -> Result<(), WorkerV3HsacoFinalizationError> {
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
    policy: &WorkerV3HsacoPolicyV1,
    finalized: &FinalizedHsaco,
) -> Result<(), WorkerV3HsacoFinalizationError> {
    let inspection = finalized.inspection();
    let metadata = inspection.hsaco();
    if inspection.descriptor_table().device_target() != target
        || metadata.target() != target.as_amd_target_id()
    {
        return Err(WorkerV3HsacoFinalizationError::TargetMismatch);
    }
    if inspection.descriptor_table().code_object_version() != code_object_version
        || map_code_object_version(metadata.code_object_version()) != code_object_version
    {
        return Err(WorkerV3HsacoFinalizationError::CodeObjectVersionMismatch);
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
        return Err(WorkerV3HsacoFinalizationError::KernelClosureMismatch);
    }

    let launch = policy.launch();
    for kernel in metadata.kernels() {
        if kernel.required_workgroup_size() != Some(launch.required_workgroup_size())
            || kernel.max_flat_workgroup_size() != launch.max_flat_workgroup_size()
            || kernel.wavefront_size() != launch.wavefront_size()
        {
            return Err(WorkerV3HsacoFinalizationError::LaunchContractMismatch {
                kernel: kernel.name().to_owned(),
            });
        }
    }
    Ok(())
}

#[derive(Clone)]
struct ProtectedFinalizationIdentityPreimageV3<'a> {
    raw_inspection_identity: [u8; 32],
    source_evidence_identity: [u8; 32],
    binding_identity: [u8; 32],
    attempt: BuildAttempt,
    slot: CompilerModuleHandoffSlotV3,
    transaction_identity: [u8; 32],
    receipt_byte_len: u64,
    outer_handoff_sha256: [u8; 32],
    outer_handoff_byte_len: u64,
    exact_outer_handoff_bytes: &'a [u8],
    capsule_sha256: [u8; 32],
    capsule_byte_len: u64,
    invocation_digest: [u8; 32],
    pair_binding_sha256: [u8; 32],
    pair_binding_byte_len: u64,
    nested_handoff_sha256: [u8; 32],
    nested_handoff_byte_len: u64,
    final_commitment_receipt_sha256: [u8; 32],
    final_commitment_receipt_byte_len: u64,
    final_commitment_sha256: [u8; 32],
    final_commitment_byte_len: u64,
    compiler_closure: CompilerClosureV2,
    worker_executable: ContentIdentityV1,
    worker_build_identity: &'a str,
    llvm_build_identity: &'a str,
    worker_limits: WorkerExecutionLimitsV1,
    link_plan_identity: [u8; 32],
    exact_link_plan_bytes: &'a [u8],
    response_identity: [u8; 32],
    raw_output: ContentIdentityV1,
    exact_raw_bytes: &'a [u8],
    policy_identity: [u8; 32],
    descriptor_observation_identity: [u8; 32],
    abi_observation_identity: [u8; 32],
    resource_observation_identity: [u8; 32],
    target: &'a str,
    code_object_version: CodeObjectVersion,
    required_workgroup_size: [u32; 3],
    max_flat_workgroup_size: u32,
    wavefront_size: u32,
    observed_kernel_symbols_identity: [u8; 32],
    finalized_output: ContentIdentityV1,
    exact_finalized_bytes: &'a [u8],
    canonical_digest: [u8; 32],
    canonical_descriptor_evidence: ContentIdentityV1,
    exact_canonical_descriptor_bytes: &'a [u8],
}

fn calculate_protected_v3_finalized_identity(
    raw: &InspectedProtectedWorkerV3HsacoV1,
    finalized: &FinalizedHsaco,
    finalized_output: ContentIdentityV1,
    descriptor_bytes: &[u8],
    canonical_descriptor_evidence: ContentIdentityV1,
) -> FinalizedProtectedWorkerV3HsacoIdentityV1 {
    let expectation = raw.binding_expectation();
    let outer_identity = expectation.outer_handoff_identity();
    let nested_identity = expectation.nested_handoff_identity();
    let worker = raw.worker_measurement();
    let limits = raw.source_evidence().execution_limits();
    let plan_bytes = raw.plan().canonical_bytes();
    let observation_identities = raw.observation_identities();
    let target = raw.target().to_string();
    let launch = raw.policy().launch();
    let observed_kernel_symbols_identity =
        calculate_observed_kernel_symbols_identity(raw.policy().observed_kernels());
    let preimage = ProtectedFinalizationIdentityPreimageV3 {
        raw_inspection_identity: *raw.identity().as_bytes(),
        source_evidence_identity: *raw.source_evidence_identity().as_bytes(),
        binding_identity: *raw.binding_identity().as_bytes(),
        attempt: expectation.attempt(),
        slot: expectation.slot(),
        transaction_identity: *expectation.transaction_identity().as_bytes(),
        receipt_byte_len: expectation.receipt_byte_len(),
        outer_handoff_sha256: *outer_identity.sha256(),
        outer_handoff_byte_len: outer_identity.byte_len(),
        exact_outer_handoff_bytes: raw.outer_handoff().canonical_bytes(),
        capsule_sha256: expectation.capsule_sha256(),
        capsule_byte_len: expectation.capsule_byte_len(),
        invocation_digest: expectation.invocation_digest(),
        pair_binding_sha256: expectation.pair_binding_sha256(),
        pair_binding_byte_len: expectation.pair_binding_byte_len(),
        nested_handoff_sha256: *nested_identity.sha256(),
        nested_handoff_byte_len: nested_identity.byte_len(),
        final_commitment_receipt_sha256: expectation.final_commitment_receipt_sha256(),
        final_commitment_receipt_byte_len: expectation.final_commitment_receipt_byte_len(),
        final_commitment_sha256: expectation.final_commitment_sha256(),
        final_commitment_byte_len: expectation.final_commitment_byte_len(),
        compiler_closure: expectation.compiler_closure(),
        worker_executable: worker.executable(),
        worker_build_identity: worker.worker_build_identity(),
        llvm_build_identity: worker.llvm_build_identity(),
        worker_limits: limits,
        link_plan_identity: *raw.link_plan_identity().as_bytes(),
        exact_link_plan_bytes: &plan_bytes,
        response_identity: *raw.response_identity().as_bytes(),
        raw_output: raw.linked_output_identity(),
        exact_raw_bytes: raw.exact_bytes(),
        policy_identity: *raw.policy().identity().as_bytes(),
        descriptor_observation_identity: observation_identities.0,
        abi_observation_identity: observation_identities.1,
        resource_observation_identity: observation_identities.2,
        target: &target,
        code_object_version: raw.code_object_version(),
        required_workgroup_size: launch.required_workgroup_size(),
        max_flat_workgroup_size: launch.max_flat_workgroup_size(),
        wavefront_size: launch.wavefront_size(),
        observed_kernel_symbols_identity,
        finalized_output,
        exact_finalized_bytes: finalized.as_bytes(),
        canonical_digest: *finalized.inspection().digest().as_bytes(),
        canonical_descriptor_evidence,
        exact_canonical_descriptor_bytes: descriptor_bytes,
    };
    FinalizedProtectedWorkerV3HsacoIdentityV1(calculate_protected_finalized_identity_v3(&preimage))
}

fn calculate_protected_finalized_identity_v3(
    preimage: &ProtectedFinalizationIdentityPreimageV3<'_>,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(PROTECTED_FINALIZED_IDENTITY_DOMAIN_V3);
    // This stage is structural. A zero authority tag prevents identity reuse by a future
    // authenticated finalization schema.
    hasher.update([0]);
    hasher.update(preimage.raw_inspection_identity);
    hasher.update(preimage.source_evidence_identity);
    hasher.update(preimage.binding_identity);
    hash_attempt_v3(&mut hasher, preimage.attempt);
    hasher.update([preimage.slot as u8]);
    hasher.update(preimage.transaction_identity);
    hasher.update(preimage.receipt_byte_len.to_le_bytes());
    hasher.update(preimage.outer_handoff_sha256);
    hasher.update(preimage.outer_handoff_byte_len.to_le_bytes());
    hash_blob_v3(&mut hasher, preimage.exact_outer_handoff_bytes);
    hasher.update(preimage.capsule_sha256);
    hasher.update(preimage.capsule_byte_len.to_le_bytes());
    hasher.update(preimage.invocation_digest);
    hasher.update(preimage.pair_binding_sha256);
    hasher.update(preimage.pair_binding_byte_len.to_le_bytes());
    hasher.update(preimage.nested_handoff_sha256);
    hasher.update(preimage.nested_handoff_byte_len.to_le_bytes());
    hasher.update(preimage.final_commitment_receipt_sha256);
    hasher.update(preimage.final_commitment_receipt_byte_len.to_le_bytes());
    hasher.update(preimage.final_commitment_sha256);
    hasher.update(preimage.final_commitment_byte_len.to_le_bytes());
    hash_compiler_closure_v2(&mut hasher, preimage.compiler_closure);
    hash_content(&mut hasher, preimage.worker_executable);
    hash_blob_v3(&mut hasher, preimage.worker_build_identity.as_bytes());
    hash_blob_v3(&mut hasher, preimage.llvm_build_identity.as_bytes());
    hasher.update(preimage.worker_limits.timeout().as_secs().to_le_bytes());
    hasher.update(
        preimage
            .worker_limits
            .timeout()
            .subsec_nanos()
            .to_le_bytes(),
    );
    hasher.update((preimage.worker_limits.stdout_bytes() as u64).to_le_bytes());
    hasher.update((preimage.worker_limits.stderr_bytes() as u64).to_le_bytes());
    hasher.update(preimage.link_plan_identity);
    hash_blob_v3(&mut hasher, preimage.exact_link_plan_bytes);
    hasher.update(preimage.response_identity);
    hash_content(&mut hasher, preimage.raw_output);
    hash_blob_v3(&mut hasher, preimage.exact_raw_bytes);
    hasher.update(preimage.policy_identity);
    hasher.update(preimage.descriptor_observation_identity);
    hasher.update(preimage.abi_observation_identity);
    hasher.update(preimage.resource_observation_identity);
    hash_blob_v3(&mut hasher, preimage.target.as_bytes());
    hasher.update([code_object_version_tag(preimage.code_object_version)]);
    for dimension in preimage.required_workgroup_size {
        hasher.update(dimension.to_le_bytes());
    }
    hasher.update(preimage.max_flat_workgroup_size.to_le_bytes());
    hasher.update(preimage.wavefront_size.to_le_bytes());
    hasher.update(preimage.observed_kernel_symbols_identity);
    hash_content(&mut hasher, preimage.finalized_output);
    hash_blob_v3(&mut hasher, preimage.exact_finalized_bytes);
    hasher.update(preimage.canonical_digest);
    hash_content(&mut hasher, preimage.canonical_descriptor_evidence);
    hash_blob_v3(&mut hasher, preimage.exact_canonical_descriptor_bytes);
    hasher.finalize().into()
}

fn calculate_observed_kernel_symbols_identity(
    kernels: &[ObservedWorkerKernelSymbolsV1],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"FE2O3/STRICT-V3-FINALIZATION/OBSERVED-KERNEL-SYMBOLS/V1\0");
    hasher.update((kernels.len() as u64).to_le_bytes());
    for kernel in kernels {
        hash_text(&mut hasher, kernel.entry());
        hash_text(&mut hasher, kernel.descriptor());
    }
    hasher.finalize().into()
}

fn hash_attempt_v3(hasher: &mut Sha256, attempt: BuildAttempt) {
    hasher.update(attempt.generation().to_le_bytes());
    hasher.update(attempt.session().as_bytes());
    hasher.update(attempt.invocation().as_bytes());
}

fn hash_blob_v3(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
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
mod v3_tests {
    use std::time::Duration;

    use super::*;

    const OUTER_BYTES: &[u8] = b"strict-v3-outer";
    const OTHER_OUTER_BYTES: &[u8] = b"strict-v3-outer-mutated";
    const PLAN_BYTES: &[u8] = b"canonical-link-plan";
    const OTHER_PLAN_BYTES: &[u8] = b"canonical-link-plan-mutated";
    const RAW_BYTES: &[u8] = b"raw-hsaco";
    const OTHER_RAW_BYTES: &[u8] = b"raw-hsaco-mutated";
    const FINAL_BYTES: &[u8] = b"finalized-hsaco";
    const OTHER_FINAL_BYTES: &[u8] = b"finalized-hsaco-mutated";
    const DESCRIPTOR_BYTES: &[u8] = b"canonical-descriptor-table";
    const OTHER_DESCRIPTOR_BYTES: &[u8] = b"canonical-descriptor-table-mutated";

    #[test]
    fn native_v3_finalization_identity_is_deterministic_and_nonzero() {
        let preimage = fixture_preimage();
        let first = calculate_protected_finalized_identity_v3(&preimage);
        let second = calculate_protected_finalized_identity_v3(&preimage);
        assert_eq!(first, second);
        assert_ne!(first, [0; 32]);
    }

    #[test]
    fn native_v3_finalization_identity_binds_every_lineage_axis() {
        let base = fixture_preimage();
        let expected = calculate_protected_finalized_identity_v3(&base);
        macro_rules! assert_axis {
            ($field:ident, $value:expr) => {{
                let mut changed = base.clone();
                changed.$field = $value;
                assert_ne!(
                    calculate_protected_finalized_identity_v3(&changed),
                    expected,
                    "V3 finalization identity omitted {}",
                    stringify!($field)
                );
            }};
        }

        assert_axis!(raw_inspection_identity, digest(2));
        assert_axis!(source_evidence_identity, digest(3));
        assert_axis!(binding_identity, digest(4));
        assert_axis!(attempt, attempt(2));
        assert_axis!(transaction_identity, digest(5));
        assert_axis!(receipt_byte_len, 101);
        assert_axis!(outer_handoff_sha256, digest(6));
        assert_axis!(outer_handoff_byte_len, 102);
        assert_axis!(exact_outer_handoff_bytes, OTHER_OUTER_BYTES);
        assert_axis!(capsule_sha256, digest(7));
        assert_axis!(capsule_byte_len, 103);
        assert_axis!(invocation_digest, digest(8));
        assert_axis!(pair_binding_sha256, digest(9));
        assert_axis!(pair_binding_byte_len, 104);
        assert_axis!(nested_handoff_sha256, digest(10));
        assert_axis!(nested_handoff_byte_len, 105);
        assert_axis!(final_commitment_receipt_sha256, digest(11));
        assert_axis!(final_commitment_receipt_byte_len, 106);
        assert_axis!(final_commitment_sha256, digest(12));
        assert_axis!(final_commitment_byte_len, 107);
        assert_axis!(compiler_closure, compiler_closure(0x30));
        assert_axis!(
            worker_executable,
            ContentIdentityV1::calculate(b"other-worker")
        );
        assert_axis!(worker_build_identity, "worker-build-mutated");
        assert_axis!(llvm_build_identity, "upstream-llvm-mutated");
        assert_axis!(
            worker_limits,
            WorkerExecutionLimitsV1::new(Duration::from_secs(4), 4096, 512).unwrap()
        );
        assert_axis!(link_plan_identity, digest(13));
        assert_axis!(exact_link_plan_bytes, OTHER_PLAN_BYTES);
        assert_axis!(response_identity, digest(14));
        assert_axis!(raw_output, ContentIdentityV1::calculate(OTHER_RAW_BYTES));
        assert_axis!(exact_raw_bytes, OTHER_RAW_BYTES);
        assert_axis!(policy_identity, digest(15));
        assert_axis!(descriptor_observation_identity, digest(16));
        assert_axis!(abi_observation_identity, digest(17));
        assert_axis!(resource_observation_identity, digest(18));
        assert_axis!(target, "gfx942:xnack+");
        assert_axis!(code_object_version, CodeObjectVersion::V5);
        assert_axis!(required_workgroup_size, [128, 1, 1]);
        assert_axis!(max_flat_workgroup_size, 128);
        assert_axis!(wavefront_size, 32);
        assert_axis!(observed_kernel_symbols_identity, digest(19));
        assert_axis!(
            finalized_output,
            ContentIdentityV1::calculate(OTHER_FINAL_BYTES)
        );
        assert_axis!(exact_finalized_bytes, OTHER_FINAL_BYTES);
        assert_axis!(canonical_digest, digest(20));
        assert_axis!(
            canonical_descriptor_evidence,
            ContentIdentityV1::calculate(OTHER_DESCRIPTOR_BYTES)
        );
        assert_axis!(exact_canonical_descriptor_bytes, OTHER_DESCRIPTOR_BYTES);
    }

    #[test]
    fn finalized_and_descriptor_byte_mutations_are_independently_bound() {
        let base = fixture_preimage();
        let expected = calculate_protected_finalized_identity_v3(&base);

        let mut finalized_bytes_changed = base.clone();
        finalized_bytes_changed.exact_finalized_bytes = OTHER_FINAL_BYTES;
        assert_ne!(
            calculate_protected_finalized_identity_v3(&finalized_bytes_changed),
            expected
        );

        let mut descriptor_bytes_changed = base.clone();
        descriptor_bytes_changed.exact_canonical_descriptor_bytes = OTHER_DESCRIPTOR_BYTES;
        assert_ne!(
            calculate_protected_finalized_identity_v3(&descriptor_bytes_changed),
            expected
        );

        let mut descriptor_digest_changed = base;
        descriptor_digest_changed.canonical_digest = digest(0xee);
        assert_ne!(
            calculate_protected_finalized_identity_v3(&descriptor_digest_changed),
            expected
        );
    }

    fn fixture_preimage() -> ProtectedFinalizationIdentityPreimageV3<'static> {
        ProtectedFinalizationIdentityPreimageV3 {
            raw_inspection_identity: digest(0x01),
            source_evidence_identity: digest(0x02),
            binding_identity: digest(0x03),
            attempt: attempt(1),
            slot: CompilerModuleHandoffSlotV3::Production,
            transaction_identity: digest(0x04),
            receipt_byte_len: 100,
            outer_handoff_sha256: digest(0x05),
            outer_handoff_byte_len: OUTER_BYTES.len() as u64,
            exact_outer_handoff_bytes: OUTER_BYTES,
            capsule_sha256: digest(0x06),
            capsule_byte_len: 200,
            invocation_digest: digest(0x07),
            pair_binding_sha256: digest(0x08),
            pair_binding_byte_len: 201,
            nested_handoff_sha256: digest(0x09),
            nested_handoff_byte_len: 202,
            final_commitment_receipt_sha256: digest(0x0a),
            final_commitment_receipt_byte_len: 203,
            final_commitment_sha256: digest(0x0b),
            final_commitment_byte_len: 204,
            compiler_closure: compiler_closure(0x10),
            worker_executable: ContentIdentityV1::calculate(b"worker"),
            worker_build_identity: "worker-build",
            llvm_build_identity: "upstream-llvm-build",
            worker_limits: WorkerExecutionLimitsV1::new(Duration::from_secs(3), 2048, 256).unwrap(),
            link_plan_identity: digest(0x0c),
            exact_link_plan_bytes: PLAN_BYTES,
            response_identity: digest(0x0d),
            raw_output: ContentIdentityV1::calculate(RAW_BYTES),
            exact_raw_bytes: RAW_BYTES,
            policy_identity: digest(0x0e),
            descriptor_observation_identity: digest(0x0f),
            abi_observation_identity: digest(0x10),
            resource_observation_identity: digest(0x11),
            target: "gfx942:xnack-",
            code_object_version: CodeObjectVersion::V6,
            required_workgroup_size: [256, 1, 1],
            max_flat_workgroup_size: 256,
            wavefront_size: 64,
            observed_kernel_symbols_identity: digest(0x12),
            finalized_output: ContentIdentityV1::calculate(FINAL_BYTES),
            exact_finalized_bytes: FINAL_BYTES,
            canonical_digest: CanonicalCodeObjectDigest::calculate_from_canonicalized_hsaco(
                FINAL_BYTES,
            )
            .as_bytes()
            .to_owned(),
            canonical_descriptor_evidence: ContentIdentityV1::calculate(DESCRIPTOR_BYTES),
            exact_canonical_descriptor_bytes: DESCRIPTOR_BYTES,
        }
    }

    fn attempt(generation: u64) -> BuildAttempt {
        BuildAttempt::from_env_value(&format!(
            "{generation}:{}:{}",
            "11".repeat(16),
            "22".repeat(32)
        ))
        .unwrap()
    }

    fn compiler_closure(seed: u8) -> CompilerClosureV2 {
        CompilerClosureV2::new(
            digest(seed),
            digest(seed.wrapping_add(1)),
            digest(seed.wrapping_add(2)),
            digest(seed.wrapping_add(3)),
            digest(seed.wrapping_add(4)),
            digest(seed.wrapping_add(5)),
        )
        .unwrap()
    }

    const fn digest(seed: u8) -> [u8; 32] {
        [seed; 32]
    }
}
