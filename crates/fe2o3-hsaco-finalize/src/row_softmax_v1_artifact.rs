//! Sealed structural Worker V2 admission and finalization for row-softmax V1.

use std::{error::Error, fmt};

use fe2o3_artifact_transaction::BuildAttempt;
use fe2o3_hsaco::{
    ArgumentAddressSpace, CodeObjectVersion as InspectedCodeObjectVersion, ExplicitValueKind,
    ExplicitValueType, InspectedHsaco,
};
use fe2o3_kernel_descriptor::{
    AdmittedRowSoftmaxV1StructuralDescriptorV1, CodeObjectVersion,
    ROW_SOFTMAX_V1_DESCRIPTOR_SYMBOL, ROW_SOFTMAX_V1_ENTRY_NAME,
    ROW_SOFTMAX_V1_EXPLICIT_KERNARG_BYTES, ROW_SOFTMAX_V1_IMPLICIT_KERNARG_BYTES,
    ROW_SOFTMAX_V1_MAX_FLAT_WORKGROUP_SIZE, ROW_SOFTMAX_V1_MAX_GRID_SIZE, ROW_SOFTMAX_V1_TARGET,
    ROW_SOFTMAX_V1_TOTAL_KERNARG_BYTES, ROW_SOFTMAX_V1_WORKGROUP_SIZE,
    RowSoftmaxV1StructuralDescriptorErrorV1, RowSoftmaxV1StructuralDescriptorExpectationV1,
    admit_row_softmax_v1_structural_descriptor_v1,
};

use crate::{
    CanonicalDescriptorSectionObservationV1, ContentIdentityV1, FinalizationError,
    FinalizedWorkerV2HsacoIdentityV1, InertFirstBuildWorkerV2EvidenceV1,
    InspectedRawWorkerV2HsacoIdentityV1, InspectedRawWorkerV2HsacoV1,
    PreparedFinalizedWorkerV2HsacoV1, WorkerV2HsacoFinalizationError,
    WorkerV2RawHsacoInspectionError, WorkerV2RawLaunchContractV1,
    finalize_inspected_worker_v2_hsaco_v1, inspect_unfinalized, verify_finalized,
    worker_v2_hsaco_admission::{
        WorkerV2RawLaunchDiagnosticProfileV1, inspect_worker_v2_raw_hsaco_with_launch_v1,
    },
};

/// Raw Worker V2 evidence admitted through the row-softmax V1 structural profile.
///
/// Admission binds structural ELF, metadata, descriptor, ABI, and launch-limit
/// facts. It deliberately does not inspect the instructions implementing the entry.
#[derive(Debug)]
pub struct InspectedRowSoftmaxV1StructuralWorkerV2HsacoV1 {
    raw: InspectedRawWorkerV2HsacoV1,
    descriptor: AdmittedRowSoftmaxV1StructuralDescriptorV1,
}

impl InspectedRowSoftmaxV1StructuralWorkerV2HsacoV1 {
    pub const fn attempt(&self) -> BuildAttempt {
        self.raw.attempt()
    }

    pub const fn raw_inspection_identity(&self) -> InspectedRawWorkerV2HsacoIdentityV1 {
        self.raw.identity()
    }

    pub const fn descriptor_admission(&self) -> AdmittedRowSoftmaxV1StructuralDescriptorV1 {
        self.descriptor
    }

    pub fn exact_bytes(&self) -> &[u8] {
        self.raw.exact_bytes()
    }

    pub const fn target(&self) -> fe2o3_kernel_descriptor::DeviceTargetV1 {
        self.raw.target()
    }

    pub const fn code_object_version(&self) -> CodeObjectVersion {
        self.raw.code_object_version()
    }

    pub const fn authenticates_source_origin(&self) -> bool {
        false
    }

    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }

    pub const fn validates_runtime_slice_lengths(&self) -> bool {
        false
    }

    pub const fn validates_kernel_body(&self) -> bool {
        false
    }

    pub const fn proves_functional_softmax(&self) -> bool {
        false
    }

    pub const fn proves_exp_implementation(&self) -> bool {
        false
    }

    pub const fn proves_numerical_contract(&self) -> bool {
        false
    }

    pub const fn proves_race_freedom(&self) -> bool {
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

/// Canonically finalized bytes retaining row-softmax V1 structural admission.
///
/// Canonical finalization adds byte integrity, not source, body, numerical, or
/// execution authority.
#[derive(Debug)]
pub struct FinalizedRowSoftmaxV1StructuralHsacoV1 {
    finalized: PreparedFinalizedWorkerV2HsacoV1,
    descriptor: AdmittedRowSoftmaxV1StructuralDescriptorV1,
}

impl FinalizedRowSoftmaxV1StructuralHsacoV1 {
    pub const fn identity(&self) -> FinalizedWorkerV2HsacoIdentityV1 {
        self.finalized.identity()
    }

    pub const fn attempt(&self) -> BuildAttempt {
        self.finalized.attempt()
    }

    pub const fn raw_output_identity(&self) -> ContentIdentityV1 {
        self.finalized.raw_output_identity()
    }

    pub const fn finalized_output_identity(&self) -> ContentIdentityV1 {
        self.finalized.finalized_output_identity()
    }

    pub const fn descriptor_admission(&self) -> AdmittedRowSoftmaxV1StructuralDescriptorV1 {
        self.descriptor
    }

    pub fn exact_finalized_bytes(&self) -> &[u8] {
        self.finalized.exact_finalized_bytes()
    }

    pub const fn canonical_descriptor_finalization_ran(&self) -> bool {
        true
    }

    pub const fn authenticates_source_origin(&self) -> bool {
        false
    }

    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }

    pub const fn validates_runtime_slice_lengths(&self) -> bool {
        false
    }

    pub const fn validates_kernel_body(&self) -> bool {
        false
    }

    pub const fn proves_functional_softmax(&self) -> bool {
        false
    }

    pub const fn proves_exp_implementation(&self) -> bool {
        false
    }

    pub const fn proves_numerical_contract(&self) -> bool {
        false
    }

    pub const fn proves_race_freedom(&self) -> bool {
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

#[derive(Debug)]
#[non_exhaustive]
pub enum RowSoftmaxV1StructuralArtifactErrorV1 {
    RawInspection(WorkerV2RawHsacoInspectionError),
    DescriptorInspection(FinalizationError),
    DescriptorPolicy(RowSoftmaxV1StructuralDescriptorErrorV1),
    ArtifactProfile(&'static str),
    CanonicalFinalization(WorkerV2HsacoFinalizationError),
    FinalizedVerification(FinalizationError),
    FinalizedDescriptorDrift,
}

impl fmt::Display for RowSoftmaxV1StructuralArtifactErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RawInspection(error) => {
                write!(formatter, "row-softmax raw inspection failed: {error}")
            }
            Self::DescriptorInspection(error) => {
                write!(
                    formatter,
                    "row-softmax descriptor inspection failed: {error}"
                )
            }
            Self::DescriptorPolicy(error) => {
                write!(formatter, "row-softmax descriptor policy failed: {error}")
            }
            Self::ArtifactProfile(field) => {
                write!(formatter, "row-softmax structural artifact {field} drifted")
            }
            Self::CanonicalFinalization(error) => {
                write!(
                    formatter,
                    "row-softmax canonical finalization failed: {error}"
                )
            }
            Self::FinalizedVerification(error) => {
                write!(
                    formatter,
                    "row-softmax finalized verification failed: {error}"
                )
            }
            Self::FinalizedDescriptorDrift => formatter.write_str(
                "row-softmax finalized descriptor differs from its raw admitted descriptor",
            ),
        }
    }
}

impl Error for RowSoftmaxV1StructuralArtifactErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::RawInspection(error) => Some(error),
            Self::DescriptorInspection(error) | Self::FinalizedVerification(error) => Some(error),
            Self::DescriptorPolicy(error) => Some(error),
            Self::CanonicalFinalization(error) => Some(error),
            Self::ArtifactProfile(_) | Self::FinalizedDescriptorDrift => None,
        }
    }
}

/// Structurally admits an unfinalized HSACO for the row-softmax V1 ABI profile.
///
/// The `.text` body and runtime values of both slice lengths remain outside
/// this function's authority.
pub fn inspect_row_softmax_v1_structural_worker_v2_hsaco_v1(
    source: InertFirstBuildWorkerV2EvidenceV1,
    expected: RowSoftmaxV1StructuralDescriptorExpectationV1,
) -> Result<InspectedRowSoftmaxV1StructuralWorkerV2HsacoV1, RowSoftmaxV1StructuralArtifactErrorV1> {
    let raw = inspect_worker_v2_raw_hsaco_with_launch_v1(
        source,
        WorkerV2RawLaunchContractV1::ROW_SOFTMAX_V1,
        WorkerV2RawLaunchDiagnosticProfileV1::RowSoftmaxV1,
    )
    .map_err(RowSoftmaxV1StructuralArtifactErrorV1::RawInspection)?;
    if raw.target().to_string() != ROW_SOFTMAX_V1_TARGET {
        return Err(RowSoftmaxV1StructuralArtifactErrorV1::ArtifactProfile(
            "target",
        ));
    }
    if raw.code_object_version() != CodeObjectVersion::V6 {
        return Err(RowSoftmaxV1StructuralArtifactErrorV1::ArtifactProfile(
            "code-object version",
        ));
    }
    if raw.canonical_descriptor_section()
        != CanonicalDescriptorSectionObservationV1::PresentButNotFinalizedByThisInspection
    {
        return Err(RowSoftmaxV1StructuralArtifactErrorV1::ArtifactProfile(
            "descriptor section",
        ));
    }
    let inspected = inspect_unfinalized(raw.exact_bytes())
        .map_err(RowSoftmaxV1StructuralArtifactErrorV1::DescriptorInspection)?;
    let descriptor =
        admit_row_softmax_v1_structural_descriptor_v1(inspected.descriptor_table(), expected)
            .map_err(RowSoftmaxV1StructuralArtifactErrorV1::DescriptorPolicy)?;
    validate_exact_artifact_metadata(inspected.hsaco())?;
    Ok(InspectedRowSoftmaxV1StructuralWorkerV2HsacoV1 { raw, descriptor })
}

/// Finalizes and structurally reinspects an admitted row-softmax V1 artifact.
pub fn finalize_row_softmax_v1_structural_worker_v2_hsaco_v1(
    inspected: InspectedRowSoftmaxV1StructuralWorkerV2HsacoV1,
) -> Result<FinalizedRowSoftmaxV1StructuralHsacoV1, RowSoftmaxV1StructuralArtifactErrorV1> {
    let expected = RowSoftmaxV1StructuralDescriptorExpectationV1::new(
        inspected.descriptor.kernel_id(),
        inspected.descriptor.source_evidence(),
        inspected.descriptor.executable_ir_evidence(),
    )
    .map_err(RowSoftmaxV1StructuralArtifactErrorV1::DescriptorPolicy)?;
    let raw_descriptor = inspected.descriptor;
    let finalized = finalize_inspected_worker_v2_hsaco_v1(inspected.raw)
        .map_err(RowSoftmaxV1StructuralArtifactErrorV1::CanonicalFinalization)?;
    let verified = verify_finalized(finalized.exact_finalized_bytes())
        .map_err(RowSoftmaxV1StructuralArtifactErrorV1::FinalizedVerification)?;
    validate_exact_artifact_metadata(verified.hsaco())?;
    let descriptor =
        admit_row_softmax_v1_structural_descriptor_v1(verified.descriptor_table(), expected)
            .map_err(RowSoftmaxV1StructuralArtifactErrorV1::DescriptorPolicy)?;
    if descriptor != raw_descriptor {
        return Err(RowSoftmaxV1StructuralArtifactErrorV1::FinalizedDescriptorDrift);
    }
    Ok(FinalizedRowSoftmaxV1StructuralHsacoV1 {
        finalized,
        descriptor,
    })
}

fn validate_exact_artifact_metadata(
    hsaco: &InspectedHsaco,
) -> Result<(), RowSoftmaxV1StructuralArtifactErrorV1> {
    if hsaco.code_object_version() != InspectedCodeObjectVersion::V6 {
        return Err(RowSoftmaxV1StructuralArtifactErrorV1::ArtifactProfile(
            "code-object version",
        ));
    }
    if hsaco.target().to_string() != ROW_SOFTMAX_V1_TARGET {
        return Err(RowSoftmaxV1StructuralArtifactErrorV1::ArtifactProfile(
            "target",
        ));
    }
    let [kernel] = hsaco.kernels() else {
        return Err(RowSoftmaxV1StructuralArtifactErrorV1::ArtifactProfile(
            "kernel closure",
        ));
    };
    if kernel.name() != ROW_SOFTMAX_V1_ENTRY_NAME {
        return Err(RowSoftmaxV1StructuralArtifactErrorV1::ArtifactProfile(
            "entry symbol",
        ));
    }
    if kernel.symbol() != ROW_SOFTMAX_V1_DESCRIPTOR_SYMBOL {
        return Err(RowSoftmaxV1StructuralArtifactErrorV1::ArtifactProfile(
            "descriptor symbol",
        ));
    }
    if kernel.required_workgroup_size() != Some(ROW_SOFTMAX_V1_WORKGROUP_SIZE)
        || kernel.max_flat_workgroup_size() != ROW_SOFTMAX_V1_MAX_FLAT_WORKGROUP_SIZE
    {
        return Err(RowSoftmaxV1StructuralArtifactErrorV1::ArtifactProfile(
            "workgroup metadata",
        ));
    }
    if kernel.max_workgroups() != ROW_SOFTMAX_V1_MAX_GRID_SIZE.map(Some) {
        return Err(RowSoftmaxV1StructuralArtifactErrorV1::ArtifactProfile(
            "maximum grid metadata",
        ));
    }
    if kernel.wavefront_size() != 64 {
        return Err(RowSoftmaxV1StructuralArtifactErrorV1::ArtifactProfile(
            "wavefront metadata",
        ));
    }
    if kernel.group_segment_fixed_size() != 0 {
        return Err(RowSoftmaxV1StructuralArtifactErrorV1::ArtifactProfile(
            "static LDS",
        ));
    }
    if kernel.kernarg_segment_size() != u64::from(ROW_SOFTMAX_V1_TOTAL_KERNARG_BYTES)
        || kernel.kernarg_segment_alignment() != 8
        || kernel.implicit_argument_offset()
            != Some(u64::from(ROW_SOFTMAX_V1_EXPLICIT_KERNARG_BYTES))
        || kernel.implicit_argument_size() != u64::from(ROW_SOFTMAX_V1_IMPLICIT_KERNARG_BYTES)
    {
        return Err(RowSoftmaxV1StructuralArtifactErrorV1::ArtifactProfile(
            "kernarg span",
        ));
    }
    if kernel.explicit_arguments().len() != 4 {
        return Err(RowSoftmaxV1StructuralArtifactErrorV1::ArtifactProfile(
            "explicit argument count",
        ));
    }
    for slice in 0..2_usize {
        let base = u64::try_from(slice).expect("bounded index") * 16;
        let pointer = &kernel.explicit_arguments()[slice * 2];
        let length = &kernel.explicit_arguments()[slice * 2 + 1];
        let pointer_name = format!("arg{slice}.data");
        let length_name = format!("arg{slice}.len");
        if pointer.name() != Some(pointer_name.as_str())
            || pointer.offset() != base
            || pointer.size() != 8
            || pointer.alignment().is_some_and(|actual| actual != 8)
            || pointer.value_kind() != ExplicitValueKind::GlobalBuffer
            || pointer
                .value_type()
                .is_some_and(|actual| actual != ExplicitValueType::F32)
            || pointer.address_space() != Some(ArgumentAddressSpace::Global)
            || length.name() != Some(length_name.as_str())
            || length.offset() != base + 8
            || length.size() != 8
            || length.alignment().is_some_and(|actual| actual != 8)
            || length.value_kind() != ExplicitValueKind::ByValue
            || length
                .value_type()
                .is_some_and(|actual| actual != ExplicitValueType::U64)
        {
            return Err(RowSoftmaxV1StructuralArtifactErrorV1::ArtifactProfile(
                "explicit slice argument layout",
            ));
        }
    }
    Ok(())
}
