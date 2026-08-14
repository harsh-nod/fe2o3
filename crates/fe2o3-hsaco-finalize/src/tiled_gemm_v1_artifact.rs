//! Sealed structural Worker V2 admission and finalization for tiled GEMM V1.

use std::{error::Error, fmt};

use fe2o3_artifact_transaction::BuildAttempt;
use fe2o3_hsaco::{
    ArgumentAddressSpace, CodeObjectVersion as InspectedCodeObjectVersion, ExplicitValueKind,
    ExplicitValueType, InspectedHsaco,
};
use fe2o3_kernel_descriptor::{
    AdmittedTiledGemmV1StructuralDescriptorV1, CodeObjectVersion, TILED_GEMM_V1_DESCRIPTOR_SYMBOL,
    TILED_GEMM_V1_ENTRY_NAME, TILED_GEMM_V1_EXPLICIT_KERNARG_BYTES,
    TILED_GEMM_V1_IMPLICIT_KERNARG_BYTES, TILED_GEMM_V1_MAX_FLAT_WORKGROUP_SIZE,
    TILED_GEMM_V1_TARGET, TILED_GEMM_V1_TOTAL_KERNARG_BYTES, TILED_GEMM_V1_WORKGROUP_SIZE,
    TiledGemmV1StructuralDescriptorErrorV1, TiledGemmV1StructuralDescriptorExpectationV1,
    admit_tiled_gemm_v1_structural_descriptor_v1,
};

use crate::{
    CanonicalDescriptorSectionObservationV1, ContentIdentityV1, FinalizationError,
    FinalizedWorkerV2HsacoIdentityV1, InertFirstBuildWorkerV2EvidenceV1,
    InspectedRawWorkerV2HsacoIdentityV1, InspectedRawWorkerV2HsacoV1,
    PreparedFinalizedWorkerV2HsacoV1, WorkerV2HsacoFinalizationError,
    WorkerV2RawHsacoInspectionError, WorkerV2RawLaunchContractV1,
    finalize_inspected_worker_v2_hsaco_v1, inspect_unfinalized, verify_finalized,
    worker_v2_hsaco_admission::inspect_worker_v2_raw_hsaco_with_launch_v1,
};

/// Raw Worker V2 evidence admitted through the tiled GEMM V1 structural profile.
///
/// Admission covers the ELF, metadata, descriptor declarations, and ABI. It
/// deliberately does not inspect or validate the kernel instruction body.
#[derive(Debug)]
pub struct InspectedTiledGemmV1StructuralWorkerV2HsacoV1 {
    raw: InspectedRawWorkerV2HsacoV1,
    descriptor: AdmittedTiledGemmV1StructuralDescriptorV1,
}

impl InspectedTiledGemmV1StructuralWorkerV2HsacoV1 {
    pub const fn attempt(&self) -> BuildAttempt {
        self.raw.attempt()
    }

    pub const fn raw_inspection_identity(&self) -> InspectedRawWorkerV2HsacoIdentityV1 {
        self.raw.identity()
    }

    pub const fn descriptor_admission(&self) -> AdmittedTiledGemmV1StructuralDescriptorV1 {
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

    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }

    pub const fn validates_kernel_body(&self) -> bool {
        false
    }

    pub const fn proves_bf16_isa_semantics(&self) -> bool {
        false
    }

    pub const fn proves_mfma_isa_semantics(&self) -> bool {
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

/// Canonically finalized bytes with retained tiled GEMM V1 structural admission.
///
/// Canonical finalization does not add kernel-body or ISA-semantic validation.
#[derive(Debug)]
pub struct FinalizedTiledGemmV1StructuralHsacoV1 {
    finalized: PreparedFinalizedWorkerV2HsacoV1,
    descriptor: AdmittedTiledGemmV1StructuralDescriptorV1,
}

impl FinalizedTiledGemmV1StructuralHsacoV1 {
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

    pub const fn descriptor_admission(&self) -> AdmittedTiledGemmV1StructuralDescriptorV1 {
        self.descriptor
    }

    pub fn exact_finalized_bytes(&self) -> &[u8] {
        self.finalized.exact_finalized_bytes()
    }

    pub const fn canonical_descriptor_finalization_ran(&self) -> bool {
        true
    }

    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }

    pub const fn validates_kernel_body(&self) -> bool {
        false
    }

    pub const fn proves_bf16_isa_semantics(&self) -> bool {
        false
    }

    pub const fn proves_mfma_isa_semantics(&self) -> bool {
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
pub enum TiledGemmV1StructuralArtifactErrorV1 {
    RawInspection(WorkerV2RawHsacoInspectionError),
    DescriptorInspection(FinalizationError),
    DescriptorPolicy(TiledGemmV1StructuralDescriptorErrorV1),
    ArtifactProfile(&'static str),
    CanonicalFinalization(WorkerV2HsacoFinalizationError),
    FinalizedVerification(FinalizationError),
    FinalizedDescriptorDrift,
}

impl fmt::Display for TiledGemmV1StructuralArtifactErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RawInspection(error) => {
                write!(formatter, "tiled GEMM raw inspection failed: {error}")
            }
            Self::DescriptorInspection(error) => {
                write!(
                    formatter,
                    "tiled GEMM descriptor inspection failed: {error}"
                )
            }
            Self::DescriptorPolicy(error) => {
                write!(formatter, "tiled GEMM descriptor policy failed: {error}")
            }
            Self::ArtifactProfile(field) => {
                write!(formatter, "tiled GEMM structural artifact {field} drifted")
            }
            Self::CanonicalFinalization(error) => {
                write!(
                    formatter,
                    "tiled GEMM canonical finalization failed: {error}"
                )
            }
            Self::FinalizedVerification(error) => {
                write!(
                    formatter,
                    "tiled GEMM finalized verification failed: {error}"
                )
            }
            Self::FinalizedDescriptorDrift => formatter.write_str(
                "tiled GEMM finalized descriptor differs from its raw admitted descriptor",
            ),
        }
    }
}

impl Error for TiledGemmV1StructuralArtifactErrorV1 {
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

/// Structurally admits an unfinalized HSACO for the exact tiled GEMM V1 ABI profile.
///
/// The `.text` kernel body is outside this function's authority.
pub fn inspect_tiled_gemm_v1_structural_worker_v2_hsaco_v1(
    source: InertFirstBuildWorkerV2EvidenceV1,
    expected: TiledGemmV1StructuralDescriptorExpectationV1,
) -> Result<InspectedTiledGemmV1StructuralWorkerV2HsacoV1, TiledGemmV1StructuralArtifactErrorV1> {
    let raw = inspect_worker_v2_raw_hsaco_with_launch_v1(
        source,
        WorkerV2RawLaunchContractV1::TILED_GEMM_V1,
    )
    .map_err(TiledGemmV1StructuralArtifactErrorV1::RawInspection)?;
    if raw.target().to_string() != TILED_GEMM_V1_TARGET {
        return Err(TiledGemmV1StructuralArtifactErrorV1::ArtifactProfile(
            "target",
        ));
    }
    if raw.code_object_version() != CodeObjectVersion::V6 {
        return Err(TiledGemmV1StructuralArtifactErrorV1::ArtifactProfile(
            "code-object version",
        ));
    }
    if raw.canonical_descriptor_section()
        != CanonicalDescriptorSectionObservationV1::PresentButNotFinalizedByThisInspection
    {
        return Err(TiledGemmV1StructuralArtifactErrorV1::ArtifactProfile(
            "descriptor section",
        ));
    }
    let inspected = inspect_unfinalized(raw.exact_bytes())
        .map_err(TiledGemmV1StructuralArtifactErrorV1::DescriptorInspection)?;
    let descriptor =
        admit_tiled_gemm_v1_structural_descriptor_v1(inspected.descriptor_table(), expected)
            .map_err(TiledGemmV1StructuralArtifactErrorV1::DescriptorPolicy)?;
    validate_exact_artifact_metadata(inspected.hsaco())?;
    Ok(InspectedTiledGemmV1StructuralWorkerV2HsacoV1 { raw, descriptor })
}

/// Finalizes and structurally reinspects an admitted tiled GEMM V1 artifact.
pub fn finalize_tiled_gemm_v1_structural_worker_v2_hsaco_v1(
    inspected: InspectedTiledGemmV1StructuralWorkerV2HsacoV1,
) -> Result<FinalizedTiledGemmV1StructuralHsacoV1, TiledGemmV1StructuralArtifactErrorV1> {
    let expected = TiledGemmV1StructuralDescriptorExpectationV1::new(
        inspected.descriptor.kernel_id(),
        inspected.descriptor.source_evidence(),
        inspected.descriptor.executable_ir_evidence(),
    )
    .map_err(TiledGemmV1StructuralArtifactErrorV1::DescriptorPolicy)?;
    let raw_descriptor = inspected.descriptor;
    let finalized = finalize_inspected_worker_v2_hsaco_v1(inspected.raw)
        .map_err(TiledGemmV1StructuralArtifactErrorV1::CanonicalFinalization)?;
    let verified = verify_finalized(finalized.exact_finalized_bytes())
        .map_err(TiledGemmV1StructuralArtifactErrorV1::FinalizedVerification)?;
    validate_exact_artifact_metadata(verified.hsaco())?;
    let descriptor =
        admit_tiled_gemm_v1_structural_descriptor_v1(verified.descriptor_table(), expected)
            .map_err(TiledGemmV1StructuralArtifactErrorV1::DescriptorPolicy)?;
    if descriptor != raw_descriptor {
        return Err(TiledGemmV1StructuralArtifactErrorV1::FinalizedDescriptorDrift);
    }
    Ok(FinalizedTiledGemmV1StructuralHsacoV1 {
        finalized,
        descriptor,
    })
}

fn validate_exact_artifact_metadata(
    hsaco: &InspectedHsaco,
) -> Result<(), TiledGemmV1StructuralArtifactErrorV1> {
    if hsaco.code_object_version() != InspectedCodeObjectVersion::V6 {
        return Err(TiledGemmV1StructuralArtifactErrorV1::ArtifactProfile(
            "code-object version",
        ));
    }
    if hsaco.target().to_string() != TILED_GEMM_V1_TARGET {
        return Err(TiledGemmV1StructuralArtifactErrorV1::ArtifactProfile(
            "target",
        ));
    }
    let [kernel] = hsaco.kernels() else {
        return Err(TiledGemmV1StructuralArtifactErrorV1::ArtifactProfile(
            "kernel closure",
        ));
    };
    if kernel.name() != TILED_GEMM_V1_ENTRY_NAME {
        return Err(TiledGemmV1StructuralArtifactErrorV1::ArtifactProfile(
            "entry symbol",
        ));
    }
    if kernel.symbol() != TILED_GEMM_V1_DESCRIPTOR_SYMBOL {
        return Err(TiledGemmV1StructuralArtifactErrorV1::ArtifactProfile(
            "descriptor symbol",
        ));
    }
    if kernel.required_workgroup_size() != Some(TILED_GEMM_V1_WORKGROUP_SIZE)
        || kernel.max_flat_workgroup_size() != TILED_GEMM_V1_MAX_FLAT_WORKGROUP_SIZE
    {
        return Err(TiledGemmV1StructuralArtifactErrorV1::ArtifactProfile(
            "workgroup metadata",
        ));
    }
    if kernel.wavefront_size() != 64 {
        return Err(TiledGemmV1StructuralArtifactErrorV1::ArtifactProfile(
            "wavefront metadata",
        ));
    }
    if kernel.group_segment_fixed_size() != 0 {
        return Err(TiledGemmV1StructuralArtifactErrorV1::ArtifactProfile(
            "static LDS",
        ));
    }
    if kernel.kernarg_segment_size() != u64::from(TILED_GEMM_V1_TOTAL_KERNARG_BYTES)
        || kernel.kernarg_segment_alignment() != 8
        || kernel.implicit_argument_offset()
            != Some(u64::from(TILED_GEMM_V1_EXPLICIT_KERNARG_BYTES))
        || kernel.implicit_argument_size() != u64::from(TILED_GEMM_V1_IMPLICIT_KERNARG_BYTES)
    {
        return Err(TiledGemmV1StructuralArtifactErrorV1::ArtifactProfile(
            "kernarg span",
        ));
    }
    if kernel.explicit_arguments().len() != 8 {
        return Err(TiledGemmV1StructuralArtifactErrorV1::ArtifactProfile(
            "explicit argument count",
        ));
    }
    for slice in 0..4_usize {
        let base = u64::try_from(slice).expect("bounded index") * 16;
        let pointer_type = if slice < 2 {
            ExplicitValueType::U16
        } else {
            ExplicitValueType::F32
        };
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
                .is_some_and(|actual| actual != pointer_type)
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
            return Err(TiledGemmV1StructuralArtifactErrorV1::ArtifactProfile(
                "explicit slice argument layout",
            ));
        }
    }
    Ok(())
}
