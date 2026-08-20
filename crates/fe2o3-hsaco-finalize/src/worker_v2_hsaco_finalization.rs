//! Canonical descriptor finalization for independently inspected Worker V2 HSACO.
//!
//! This boundary consumes raw inspection evidence exactly once, runs the canonical
//! `.fe2o3.kd.v1` finalizer, and retains both sides of the resulting lineage. The prepared value
//! remains descriptive evidence: it authenticates neither the compiler nor Verus and grants no
//! publication, HSA loading, or launch authority.

use std::{error::Error, fmt};

use fe2o3_artifact_transaction::BuildAttempt;
use fe2o3_hsaco::CodeObjectVersion as InspectedCodeObjectVersion;
use fe2o3_kernel_descriptor::{CanonicalCodeObjectDigest, CodeObjectVersion, DeviceTargetV1};
use sha2::{Digest, Sha256};

use crate::{
    CanonicalDescriptorSectionObservationV1, ContentIdentityV1, DEVICE_DESCRIPTOR_SECTION_NAME,
    FinalizationError, FinalizedHsaco, FirstBuildWorkerV2IdentityV1,
    InspectedRawWorkerV2HsacoIdentityV1, InspectedRawWorkerV2HsacoV1,
    ObservedWorkerV2KernelSymbolsV1, WorkerV2RawHsacoPolicyIdentityV1,
    finalize_allocated_read_only_unfinalized, finalize_unfinalized,
    verify_allocated_read_only_finalized, verify_finalized,
};

const FINALIZED_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/WORKER-V2-CANONICAL-FINALIZATION/V1\0";

/// Stable identity of one raw-inspection-to-canonical-finalization transition.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FinalizedWorkerV2HsacoIdentityV1([u8; 32]);

impl FinalizedWorkerV2HsacoIdentityV1 {
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

/// Failure while turning admitted raw Worker V2 output into inert canonical-finalization evidence.
#[derive(Debug)]
#[non_exhaustive]
pub enum WorkerV2HsacoFinalizationError {
    RawOutputIdentityMismatch,
    MissingAuthenticatedDescriptorSourceEvidence(
        Box<MissingAuthenticatedDescriptorSourceEvidenceV1>,
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
    let raw_bytes = raw.exact_bytes();
    if !raw.linked_output_identity().matches(raw_bytes) {
        return Err(WorkerV2HsacoFinalizationError::RawOutputIdentityMismatch);
    }

    if raw.canonical_descriptor_section() == CanonicalDescriptorSectionObservationV1::Missing {
        return Err(
            WorkerV2HsacoFinalizationError::MissingAuthenticatedDescriptorSourceEvidence(Box::new(
                MissingAuthenticatedDescriptorSourceEvidenceV1 { raw },
            )),
        );
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

    validate_metadata_lineage(&raw, &finalized)?;

    let finalized_output = ContentIdentityV1::calculate(finalized.as_bytes());
    if !finalized_output.matches(finalized.as_bytes())
        || verified.digest().as_bytes() == &[0; 32]
        || verified.digest() != finalized.inspection().digest()
    {
        return Err(WorkerV2HsacoFinalizationError::FinalizedOutputIdentityMismatch);
    }

    let identity = calculate_finalized_identity(&raw, &finalized, finalized_output);
    Ok(PreparedFinalizedWorkerV2HsacoV1 {
        identity,
        raw,
        finalized,
        finalized_output,
    })
}

fn validate_metadata_lineage(
    raw: &InspectedRawWorkerV2HsacoV1,
    finalized: &FinalizedHsaco,
) -> Result<(), WorkerV2HsacoFinalizationError> {
    let inspection = finalized.inspection();
    let metadata = inspection.hsaco();
    if inspection.descriptor_table().device_target() != raw.target()
        || metadata.target() != raw.target().as_amd_target_id()
    {
        return Err(WorkerV2HsacoFinalizationError::TargetMismatch);
    }
    if inspection.descriptor_table().code_object_version() != raw.code_object_version()
        || map_code_object_version(metadata.code_object_version()) != raw.code_object_version()
    {
        return Err(WorkerV2HsacoFinalizationError::CodeObjectVersionMismatch);
    }

    let mut observed: Vec<_> = metadata
        .kernels()
        .iter()
        .map(|kernel| (kernel.name(), kernel.symbol()))
        .collect();
    observed.sort_unstable();
    let expected: Vec<_> = raw
        .policy()
        .observed_kernels()
        .iter()
        .map(|kernel| (kernel.entry(), kernel.descriptor()))
        .collect();
    if observed != expected {
        return Err(WorkerV2HsacoFinalizationError::KernelClosureMismatch);
    }

    let launch = raw.policy().launch();
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
