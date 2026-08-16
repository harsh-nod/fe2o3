//! Sealed structural Worker V2 admission and finalization for row-softmax V1.

use std::{error::Error, fmt};

use fe2o3_artifact_transaction::BuildAttempt;
use fe2o3_hsaco::{
    ArgumentAddressSpace, CodeObjectVersion as InspectedCodeObjectVersion, ExplicitValueKind,
    HiddenValueKind, InspectedHsaco, KernelKind,
};
use fe2o3_kernel_descriptor::{
    AdmittedRowSoftmaxV1StructuralDescriptorV1, CodeObjectVersion,
    ROW_SOFTMAX_V1_DESCRIPTOR_SYMBOL, ROW_SOFTMAX_V1_ENTRY_NAME,
    ROW_SOFTMAX_V1_EXPLICIT_KERNARG_BYTES, ROW_SOFTMAX_V1_IMPLICIT_KERNARG_BYTES,
    ROW_SOFTMAX_V1_MAX_FLAT_WORKGROUP_SIZE, ROW_SOFTMAX_V1_TARGET,
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
    if hsaco.has_printf_metadata() {
        return Err(RowSoftmaxV1StructuralArtifactErrorV1::ArtifactProfile(
            "printf metadata",
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
    if kernel.kind() != KernelKind::Normal || kernel.kind_was_emitted() {
        return Err(RowSoftmaxV1StructuralArtifactErrorV1::ArtifactProfile(
            "kernel kind",
        ));
    }
    if kernel.cluster_dims().is_some() {
        return Err(RowSoftmaxV1StructuralArtifactErrorV1::ArtifactProfile(
            "cluster dimensions",
        ));
    }
    if kernel.uses_dynamic_stack_declaration() != Some(false) {
        return Err(RowSoftmaxV1StructuralArtifactErrorV1::ArtifactProfile(
            "dynamic stack declaration",
        ));
    }
    if kernel.uniform_work_group_size_declaration().is_some() {
        return Err(RowSoftmaxV1StructuralArtifactErrorV1::ArtifactProfile(
            "uniform workgroup declaration",
        ));
    }
    if kernel.workgroup_processor_mode().is_some()
        || kernel.gfx1250_revision().is_some()
        || kernel.device_enqueue_symbol().is_some()
    {
        return Err(RowSoftmaxV1StructuralArtifactErrorV1::ArtifactProfile(
            "optional execution metadata",
        ));
    }
    if kernel.source_language() != Some("OpenCL C")
        || kernel.source_language_version() != Some([2, 0])
        || kernel.workgroup_size_hint_was_emitted()
        || kernel.vector_type_hint_was_emitted()
    {
        return Err(RowSoftmaxV1StructuralArtifactErrorV1::ArtifactProfile(
            "source metadata",
        ));
    }
    if kernel.required_workgroup_size() != Some(ROW_SOFTMAX_V1_WORKGROUP_SIZE)
        || kernel.max_flat_workgroup_size() != ROW_SOFTMAX_V1_MAX_FLAT_WORKGROUP_SIZE
    {
        return Err(RowSoftmaxV1StructuralArtifactErrorV1::ArtifactProfile(
            "workgroup metadata",
        ));
    }
    // LLVM 22.1.8 does not serialize max-num-workgroups for this kernel. The
    // one-block launch limit remains an independently admitted descriptor
    // fact; accepting synthesized metadata here would misstate the HSACO.
    if kernel.max_workgroups() != [None; 3] {
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
    if kernel.private_segment_fixed_size() != 0 {
        return Err(RowSoftmaxV1StructuralArtifactErrorV1::ArtifactProfile(
            "private segment",
        ));
    }
    if kernel.sgpr_count() != 42
        || kernel.vgpr_count() != 88
        || kernel.agpr_count() != Some(44)
        || kernel.sgpr_spill_count() != Some(44)
        || kernel.vgpr_spill_count() != Some(28)
    {
        return Err(RowSoftmaxV1StructuralArtifactErrorV1::ArtifactProfile(
            "register metadata",
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
    if !kernel.arguments_were_emitted() || kernel.explicit_arguments().len() != 4 {
        return Err(RowSoftmaxV1StructuralArtifactErrorV1::ArtifactProfile(
            "explicit argument count",
        ));
    }
    validate_exact_hidden_arguments(kernel)?;
    for slice in 0..2_usize {
        let base = u64::try_from(slice).expect("bounded index") * 16;
        let pointer = &kernel.explicit_arguments()[slice * 2];
        let length = &kernel.explicit_arguments()[slice * 2 + 1];
        let pointer_name = format!("arg{slice}.data");
        let length_name = format!("arg{slice}.len");
        if pointer.name() != Some(pointer_name.as_str())
            || pointer.offset() != base
            || pointer.size() != 8
            || pointer.type_name().is_some()
            || pointer.alignment().is_some()
            || pointer.value_kind() != ExplicitValueKind::GlobalBuffer
            || pointer.value_type().is_some()
            || pointer.address_space() != Some(ArgumentAddressSpace::Global)
            || has_optional_explicit_qualifier(pointer)
            || length.name() != Some(length_name.as_str())
            || length.offset() != base + 8
            || length.size() != 8
            || length.type_name().is_some()
            || length.alignment().is_some()
            || length.value_kind() != ExplicitValueKind::ByValue
            || length.value_type().is_some()
            || length.address_space().is_some()
            || has_optional_explicit_qualifier(length)
        {
            return Err(RowSoftmaxV1StructuralArtifactErrorV1::ArtifactProfile(
                "explicit slice argument layout",
            ));
        }
    }
    Ok(())
}

fn has_optional_explicit_qualifier(argument: &fe2o3_hsaco::ExplicitArgument) -> bool {
    argument.access().is_some()
        || argument.actual_access().is_some()
        || argument.pointee_alignment().is_some()
        || argument.is_const().is_some()
        || argument.is_restrict().is_some()
        || argument.is_volatile().is_some()
        || argument.is_pipe().is_some()
}

fn validate_exact_hidden_arguments(
    kernel: &fe2o3_hsaco::InspectedKernel,
) -> Result<(), RowSoftmaxV1StructuralArtifactErrorV1> {
    // Exact COV6 suffix emitted by the pinned upstream LLVM 22.1.8
    // TargetMachine. The final six fields are physical ABI inputs, not source,
    // compiler-origin, publication, load, or launch authority.
    const REQUIRED: [(u64, u64, HiddenValueKind); 19] = [
        (0, 4, HiddenValueKind::BlockCountX),
        (4, 4, HiddenValueKind::BlockCountY),
        (8, 4, HiddenValueKind::BlockCountZ),
        (12, 2, HiddenValueKind::GroupSizeX),
        (14, 2, HiddenValueKind::GroupSizeY),
        (16, 2, HiddenValueKind::GroupSizeZ),
        (18, 2, HiddenValueKind::RemainderX),
        (20, 2, HiddenValueKind::RemainderY),
        (22, 2, HiddenValueKind::RemainderZ),
        (40, 8, HiddenValueKind::GlobalOffsetX),
        (48, 8, HiddenValueKind::GlobalOffsetY),
        (56, 8, HiddenValueKind::GlobalOffsetZ),
        (64, 2, HiddenValueKind::GridDimensions),
        (80, 8, HiddenValueKind::HostcallBuffer),
        (88, 8, HiddenValueKind::MultigridSyncArgument),
        (96, 8, HiddenValueKind::HeapV1),
        (104, 8, HiddenValueKind::DefaultQueue),
        (112, 8, HiddenValueKind::CompletionAction),
        (200, 8, HiddenValueKind::QueuePointer),
    ];
    let hidden = kernel.hidden_arguments();
    if hidden.len() != REQUIRED.len() {
        return Err(RowSoftmaxV1StructuralArtifactErrorV1::ArtifactProfile(
            "hidden argument profile",
        ));
    }
    for (argument, (relative_offset, size, kind)) in hidden.iter().copied().zip(REQUIRED) {
        if argument.offset() != u64::from(ROW_SOFTMAX_V1_EXPLICIT_KERNARG_BYTES) + relative_offset
            || argument.size() != size
            || argument.value_kind() != kind
        {
            return Err(RowSoftmaxV1StructuralArtifactErrorV1::ArtifactProfile(
                "hidden argument profile",
            ));
        }
    }
    Ok(())
}
