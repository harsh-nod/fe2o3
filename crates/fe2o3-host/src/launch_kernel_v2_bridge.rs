use crate::{
    ArtifactKernelIdentityV1, CurrentFinalizedWorkerV2BundleAdmissionV1,
    FinalizedWorkerV2BundleAdmissionError, PhysicalMetadataValueV1,
    PublishedKernelPhysicalLayoutV1, RecoveredWorkerV2PinnedDescriptorV1,
};
use fe2o3_amd_target::{AmdTargetId, FeatureState};
use fe2o3_artifacts::{DigestAlgorithm, Endianness as ArtifactEndianness, PointerWidth};
use fe2o3_hsaco::{CodeObjectVersion, ExplicitValueKind};
use fe2o3_kernel_descriptor::{
    AccessMode, AliasSemantics, BlockSizeV1, KernelDescriptorV1, OwnershipSemantics,
    PhysicalAbiComponentKind, ScalarTypeV1,
};
use fe2o3_kernel_ir::{
    AbiParameterKindV2, AbiParameterV2, ArtifactIdentityV2, BlockShapePolicyV2, DimensionsV2,
    Gfx942LaunchContractV2, Gfx942ResourceLimitsV2, Gfx942TargetBindingV2, KernelIdentityV2,
    KernelSignatureIdentityV2, KernelSignatureV2, KernelVariantV2, LaunchKernelFamilyV2,
    LaunchKernelLimitsV2, LaunchKernelValidationErrorV2, OccupancySubjectIdentityV2,
    SemanticTypeIdentityV2, TargetIdentityV2, UnsupportedLaunchFeaturesV2, WavefrontWidthV2,
    canonical_occupancy_subject_identity_v2,
};
use sha2::{Digest, Sha256};
use std::error::Error;
use std::fmt;

const TARGET_IDENTITY_DOMAIN_V2: &[u8] = b"fe2o3.host.launch-kernel.target.v2\0";
const SIGNATURE_IDENTITY_DOMAIN_V2: &[u8] = b"fe2o3.host.launch-kernel.signature.v2\0";
const SEMANTIC_PARAMETER_DOMAIN_V2: &[u8] = b"fe2o3.host.launch-kernel.semantic-parameter.v2\0";

/// Why inspected gfx942 metadata cannot authorize an occupancy-dependent launch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Gfx942OccupancyMetadataStatusV2 {
    /// Register and segment counts are descriptive inputs, but fe2o3 has no reviewed derivation
    /// that binds them to gfx942 allocation granularities and resident-wave limits.
    NoReviewedPhysicalDerivation,
}

/// Rejection returned when an inert metadata match is asked to establish occupancy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum OccupancyDependentLaunchAdmissionErrorV2 {
    PhysicalOccupancyUnavailable(Gfx942OccupancyMetadataStatusV2),
}

impl fmt::Display for OccupancyDependentLaunchAdmissionErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PhysicalOccupancyUnavailable(_) => formatter
                .write_str("inspected HSACO metadata has no reviewed gfx942 occupancy derivation"),
        }
    }
}

impl Error for OccupancyDependentLaunchAdmissionErrorV2 {}

/// Current, inert match between one launch-model variant and one recovered executable kernel.
///
/// The value retains the cooperative current-publication guard and contains only identities and
/// physical metadata derived from the recovered Worker V2 admission. Caller-supplied policy,
/// proof, occupancy-verifier, and occupancy-metadata identities are not retained and grant no
/// authority. The value has no transition into HSA loading or dispatch.
pub struct CurrentRecoveredLaunchKernelMetadataV2<'recovered> {
    _current: CurrentFinalizedWorkerV2BundleAdmissionV1<'recovered>,
    target: Gfx942TargetBindingV2,
    artifact_identity: ArtifactIdentityV2,
    kernel_identity: KernelIdentityV2,
    signature: KernelSignatureV2,
    resources: Gfx942ResourceLimitsV2,
    occupancy_subject: OccupancySubjectIdentityV2,
    variant_name: Box<str>,
}

impl fmt::Debug for CurrentRecoveredLaunchKernelMetadataV2<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CurrentRecoveredLaunchKernelMetadataV2")
            .field("target", &self.target)
            .field("artifact_identity", &self.artifact_identity)
            .field("kernel_identity", &self.kernel_identity)
            .field("signature", &self.signature)
            .field("resources", &self.resources)
            .field("occupancy_subject", &self.occupancy_subject)
            .field("variant_name", &self.variant_name)
            .field("occupancy_status", &self.occupancy_status())
            .finish_non_exhaustive()
    }
}

impl CurrentRecoveredLaunchKernelMetadataV2<'_> {
    pub const fn target(&self) -> Gfx942TargetBindingV2 {
        self.target
    }

    pub const fn artifact_identity(&self) -> ArtifactIdentityV2 {
        self.artifact_identity
    }

    pub const fn kernel_identity(&self) -> KernelIdentityV2 {
        self.kernel_identity
    }

    pub const fn signature(&self) -> &KernelSignatureV2 {
        &self.signature
    }

    pub const fn resources(&self) -> Gfx942ResourceLimitsV2 {
        self.resources
    }

    pub const fn occupancy_subject(&self) -> OccupancySubjectIdentityV2 {
        self.occupancy_subject
    }

    pub fn variant_name(&self) -> &str {
        &self.variant_name
    }

    pub const fn occupancy_status(&self) -> Gfx942OccupancyMetadataStatusV2 {
        Gfx942OccupancyMetadataStatusV2::NoReviewedPhysicalDerivation
    }

    pub const fn require_occupancy_dependent_admission(
        &self,
    ) -> Result<(), OccupancyDependentLaunchAdmissionErrorV2> {
        Err(
            OccupancyDependentLaunchAdmissionErrorV2::PhysicalOccupancyUnavailable(
                Gfx942OccupancyMetadataStatusV2::NoReviewedPhysicalDerivation,
            ),
        )
    }

    pub const fn authenticates_compiler_or_verus_provenance(&self) -> bool {
        false
    }

    pub const fn authenticates_rust_type_or_effect_semantics(&self) -> bool {
        false
    }

    pub const fn authenticates_policy_or_proof_claims(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_dispatch_authority(&self) -> bool {
        false
    }
}

/// Binds one valid launch-model variant to current recovered physical metadata.
///
/// This validates target, payload, kernel, symbol, the embedded descriptor's flattened signature
/// against AMDHSA physical arguments, non-occupancy launch geometry, static/private resources, and
/// the occupancy subject. Dynamic LDS is rejected because AMDHSA does not provide the maximum and
/// alignment required by the launch model. Occupancy-dependent admission remains unavailable even
/// on success.
pub fn bind_current_recovered_launch_kernel_metadata_v2<'recovered>(
    recovered: &'recovered RecoveredWorkerV2PinnedDescriptorV1,
    family: &LaunchKernelFamilyV2,
    variant_name: &str,
) -> Result<CurrentRecoveredLaunchKernelMetadataV2<'recovered>, LaunchKernelMetadataBridgeErrorV2> {
    family
        .validate(&LaunchKernelLimitsV2::default())
        .map_err(LaunchKernelMetadataBridgeErrorV2::InvalidLaunchModel)?;
    let variant = family
        .variants
        .iter()
        .find(|variant| variant.variant_name == variant_name)
        .ok_or(LaunchKernelMetadataBridgeErrorV2::UnknownVariant)?;

    let current = recovered
        .acquire_launch_kernel_v2_currentness()
        .map_err(LaunchKernelMetadataBridgeErrorV2::CurrentPublication)?;
    let admission = current.admission();
    let derived = derive_metadata(
        admission.target(),
        admission.code_object_version(),
        admission.artifact_identity(),
        recovered.descriptor(),
        admission.selected_kernel(),
    )?;
    validate_model_match(family, variant, &derived)?;

    Ok(CurrentRecoveredLaunchKernelMetadataV2 {
        _current: current,
        target: derived.target,
        artifact_identity: derived.artifact_identity,
        kernel_identity: derived.kernel_identity,
        signature: derived.signature,
        resources: derived.resources,
        occupancy_subject: derived.occupancy_subject,
        variant_name: variant.variant_name.clone().into_boxed_str(),
    })
}

struct DerivedLaunchMetadataV2 {
    target: Gfx942TargetBindingV2,
    artifact_identity: ArtifactIdentityV2,
    kernel_identity: KernelIdentityV2,
    logical_name: Box<str>,
    entry_name: Box<str>,
    signature: KernelSignatureV2,
    launch: DerivedLaunchGeometryV2,
    resources: Gfx942ResourceLimitsV2,
    occupancy_subject: OccupancySubjectIdentityV2,
}

#[derive(Clone, Copy)]
struct DerivedLaunchGeometryV2 {
    rank: u8,
    block: BlockShapePolicyV2,
    max_grid_blocks: DimensionsV2,
    minimum_flat_workgroup_size: u32,
    maximum_flat_workgroup_size: u32,
    max_total_workitems: u64,
}

fn derive_metadata(
    inspected_target: AmdTargetId,
    code_object_version: CodeObjectVersion,
    artifact: &ArtifactKernelIdentityV1,
    descriptor: &KernelDescriptorV1,
    physical: &PublishedKernelPhysicalLayoutV1,
) -> Result<DerivedLaunchMetadataV2, LaunchKernelMetadataBridgeErrorV2> {
    validate_descriptor_artifact_identity(artifact, descriptor, physical)?;
    let target = derive_target(inspected_target, code_object_version, artifact)?;
    let artifact_identity = derive_artifact_identity(artifact)?;
    let kernel_identity = KernelIdentityV2::from_bytes(*descriptor.kernel_id().as_bytes());
    let signature = derive_signature(descriptor, physical)?;
    let launch = derive_launch_geometry(descriptor, physical)?;
    let resources = derive_resources(descriptor, physical)?;
    let occupancy_subject = canonical_occupancy_subject_identity_v2(
        &target,
        &signature,
        artifact_identity,
        physical.export_symbol(),
        resources,
    );

    Ok(DerivedLaunchMetadataV2 {
        target,
        artifact_identity,
        kernel_identity,
        logical_name: descriptor.logical_name().as_str().into(),
        entry_name: physical.export_symbol().into(),
        signature,
        launch,
        resources,
        occupancy_subject,
    })
}

fn validate_descriptor_artifact_identity(
    artifact: &ArtifactKernelIdentityV1,
    descriptor: &KernelDescriptorV1,
    physical: &PublishedKernelPhysicalLayoutV1,
) -> Result<(), LaunchKernelMetadataBridgeErrorV2> {
    let checks = [
        (
            artifact.kernel_id() == descriptor.kernel_id(),
            "kernel identity",
        ),
        (
            artifact.name().as_str() == descriptor.logical_name().as_str(),
            "logical name",
        ),
        (
            artifact.symbol().as_str() == descriptor.entry_name().as_str(),
            "entry name",
        ),
        (
            descriptor.entry_name().as_str() == physical.export_symbol(),
            "physical export symbol",
        ),
        (
            descriptor.descriptor_symbol().as_str() == physical.descriptor_symbol(),
            "physical descriptor symbol",
        ),
        (
            u64::from(descriptor.abi_layout().explicit_argument_size()) == artifact.abi().size(),
            "explicit argument size",
        ),
    ];
    for (matches, field) in checks {
        if !matches {
            return Err(LaunchKernelMetadataBridgeErrorV2::RecoveredMetadataInconsistent(field));
        }
    }
    Ok(())
}

fn derive_target(
    inspected: AmdTargetId,
    code_object_version: CodeObjectVersion,
    artifact: &ArtifactKernelIdentityV1,
) -> Result<Gfx942TargetBindingV2, LaunchKernelMetadataBridgeErrorV2> {
    if inspected.processor() != "gfx942" || inspected.xnack() != Some(FeatureState::Disabled) {
        return Err(LaunchKernelMetadataBridgeErrorV2::UnsupportedTarget);
    }
    if code_object_version != CodeObjectVersion::V6 {
        return Err(LaunchKernelMetadataBridgeErrorV2::UnsupportedCodeObjectVersion);
    }
    let artifact_target = artifact.target();
    if artifact_target.triple().as_str() != "amdgcn-amd-amdhsa"
        || artifact_target.architecture().as_str() != inspected.to_string()
        || artifact_target.pointer_width() != PointerWidth::Bits64
        || artifact_target.endianness() != ArtifactEndianness::Little
    {
        return Err(
            LaunchKernelMetadataBridgeErrorV2::RecoveredMetadataInconsistent("artifact target"),
        );
    }

    let mut digest = CanonicalDigestV2::new(TARGET_IDENTITY_DOMAIN_V2);
    digest.bytes(artifact_target.triple().as_str().as_bytes());
    digest.bytes(inspected.to_string().as_bytes());
    digest.u8(code_object_version.number());
    digest.u8(8);
    digest.u8(1);
    Ok(Gfx942TargetBindingV2::gfx942_xnack_minus(
        TargetIdentityV2::from_bytes(digest.finish()),
    ))
}

fn derive_artifact_identity(
    artifact: &ArtifactKernelIdentityV1,
) -> Result<ArtifactIdentityV2, LaunchKernelMetadataBridgeErrorV2> {
    let digest = artifact.payload_digest();
    if digest.algorithm() != DigestAlgorithm::Sha256 {
        return Err(LaunchKernelMetadataBridgeErrorV2::UnsupportedDigestAlgorithm);
    }
    Ok(ArtifactIdentityV2::from_bytes(*digest.bytes().as_bytes()))
}

fn derive_signature(
    descriptor: &KernelDescriptorV1,
    physical: &PublishedKernelPhysicalLayoutV1,
) -> Result<KernelSignatureV2, LaunchKernelMetadataBridgeErrorV2> {
    let mut parameters = Vec::with_capacity(physical.arguments().len());
    let mut physical_index = 0_usize;
    for argument in descriptor.arguments() {
        for (component_index, (kind, offset, size, alignment)) in
            argument.physical_components().enumerate()
        {
            let physical_argument = physical.arguments().get(physical_index).ok_or(
                LaunchKernelMetadataBridgeErrorV2::RecoveredMetadataInconsistent(
                    "physical argument count",
                ),
            )?;
            let model_kind = model_parameter_kind(kind, argument.ownership())?;
            validate_physical_component(kind, offset, size, alignment, physical_argument)?;
            let source_index = u16::try_from(physical_index).map_err(|_| {
                LaunchKernelMetadataBridgeErrorV2::NumericOverflow("physical argument index")
            })?;
            parameters.push(AbiParameterV2 {
                source_index,
                kind: model_kind,
                semantic_type: derive_semantic_parameter_identity(
                    argument,
                    component_index,
                    kind,
                    offset,
                    size,
                    alignment,
                ),
                offset,
                size: u32::from(size),
                alignment: u32::from(alignment),
            });
            physical_index += 1;
        }
    }
    if physical_index != physical.arguments().len() {
        return Err(
            LaunchKernelMetadataBridgeErrorV2::RecoveredMetadataInconsistent(
                "physical argument count",
            ),
        );
    }

    let physical_launch = physical.launch();
    let mut signature = KernelSignatureV2 {
        identity: KernelSignatureIdentityV2::from_bytes([0; 32]),
        explicit_argument_bytes: descriptor.abi_layout().explicit_argument_size(),
        kernarg_segment_bytes: u32::try_from(physical_launch.kernarg_segment_size()).map_err(
            |_| LaunchKernelMetadataBridgeErrorV2::NumericOverflow("kernarg segment size"),
        )?,
        kernarg_segment_alignment: u32::try_from(physical_launch.kernarg_segment_alignment())
            .map_err(|_| {
                LaunchKernelMetadataBridgeErrorV2::NumericOverflow("kernarg segment alignment")
            })?,
        parameters,
    };
    signature.identity = derive_signature_identity(&signature);
    Ok(signature)
}

fn model_parameter_kind(
    component: PhysicalAbiComponentKind,
    ownership: OwnershipSemantics,
) -> Result<AbiParameterKindV2, LaunchKernelMetadataBridgeErrorV2> {
    match component {
        PhysicalAbiComponentKind::ScalarByValue(_) | PhysicalAbiComponentKind::SliceLengthU64 => {
            Ok(AbiParameterKindV2::ByValue)
        }
        PhysicalAbiComponentKind::GlobalPointer => match ownership {
            OwnershipSemantics::SharedBorrow => Ok(AbiParameterKindV2::SharedGlobalPointer),
            OwnershipSemantics::UniqueBorrow => Ok(AbiParameterKindV2::UniqueGlobalPointer),
            OwnershipSemantics::ByValue => {
                Err(LaunchKernelMetadataBridgeErrorV2::UnsupportedPhysicalAbi(
                    "by-value global pointer",
                ))
            }
        },
    }
}

fn validate_physical_component(
    component: PhysicalAbiComponentKind,
    offset: u32,
    size: u16,
    alignment: u16,
    physical: &crate::PublishedPhysicalArgumentLayoutV1,
) -> Result<(), LaunchKernelMetadataBridgeErrorV2> {
    let expected_kind = match component {
        PhysicalAbiComponentKind::ScalarByValue(_) | PhysicalAbiComponentKind::SliceLengthU64 => {
            ExplicitValueKind::ByValue
        }
        PhysicalAbiComponentKind::GlobalPointer => ExplicitValueKind::GlobalBuffer,
    };
    if physical.offset() != u64::from(offset)
        || physical.size() != u64::from(size)
        || matches!(
            physical.alignment(),
            PhysicalMetadataValueV1::Known(value) if value != u64::from(alignment)
        )
        || physical.value_kind() != expected_kind
    {
        return Err(
            LaunchKernelMetadataBridgeErrorV2::RecoveredMetadataInconsistent(
                "physical argument component",
            ),
        );
    }
    Ok(())
}

fn derive_semantic_parameter_identity(
    argument: &fe2o3_kernel_descriptor::LogicalArgumentV1,
    component_index: usize,
    component: PhysicalAbiComponentKind,
    offset: u32,
    size: u16,
    alignment: u16,
) -> SemanticTypeIdentityV2 {
    let mut digest = CanonicalDigestV2::new(SEMANTIC_PARAMETER_DOMAIN_V2);
    digest.u16(argument.source_index());
    digest.bytes(argument.name().as_str().as_bytes());
    digest.bytes(argument.source_type().as_bytes());
    digest.bytes(argument.device_layout().as_bytes());
    digest.u8(ownership_tag(argument.ownership()));
    digest.u8(access_tag(argument.access()));
    digest.u8(alias_tag(argument.alias()));
    digest.u64(component_index as u64);
    digest.u8(component_tag(component));
    digest.u32(offset);
    digest.u16(size);
    digest.u16(alignment);
    SemanticTypeIdentityV2::from_bytes(digest.finish())
}

fn derive_signature_identity(signature: &KernelSignatureV2) -> KernelSignatureIdentityV2 {
    let mut digest = CanonicalDigestV2::new(SIGNATURE_IDENTITY_DOMAIN_V2);
    digest.u32(signature.explicit_argument_bytes);
    digest.u32(signature.kernarg_segment_bytes);
    digest.u32(signature.kernarg_segment_alignment);
    digest.u64(signature.parameters.len() as u64);
    for parameter in &signature.parameters {
        digest.u16(parameter.source_index);
        digest.u8(parameter.kind as u8);
        digest.bytes(&parameter.semantic_type.0);
        digest.u32(parameter.offset);
        digest.u32(parameter.size);
        digest.u32(parameter.alignment);
    }
    KernelSignatureIdentityV2::from_bytes(digest.finish())
}

fn derive_launch_geometry(
    descriptor: &KernelDescriptorV1,
    physical: &PublishedKernelPhysicalLayoutV1,
) -> Result<DerivedLaunchGeometryV2, LaunchKernelMetadataBridgeErrorV2> {
    let launch = descriptor.launch();
    let block = match launch.block_size() {
        BlockSizeV1::Exact(dimensions) => dimensions,
        BlockSizeV1::Any | BlockSizeV1::AtMost(_) => {
            return Err(
                LaunchKernelMetadataBridgeErrorV2::UnsupportedPhysicalLaunchContract(
                    "non-exact block policy",
                ),
            );
        }
    };
    if physical.launch().wavefront_size() != 64 {
        return Err(LaunchKernelMetadataBridgeErrorV2::UnsupportedTarget);
    }
    let block = DimensionsV2::new(block.x(), block.y(), block.z());
    let max_grid = launch.max_grid();
    let max_grid_blocks = DimensionsV2::new(max_grid.x(), max_grid.y(), max_grid.z());
    let flat = checked_dimensions_product(block, "flat workgroup size")?;
    let flat = u32::try_from(flat)
        .map_err(|_| LaunchKernelMetadataBridgeErrorV2::NumericOverflow("flat workgroup size"))?;
    if flat != launch.max_flat_workgroup_size()
        || flat != physical.launch().max_flat_workgroup_size()
    {
        return Err(
            LaunchKernelMetadataBridgeErrorV2::RecoveredMetadataInconsistent(
                "maximum flat workgroup size",
            ),
        );
    }
    let grid_blocks = checked_dimensions_product(max_grid_blocks, "grid block count")?;
    let max_total_workitems = grid_blocks.checked_mul(u64::from(flat)).ok_or(
        LaunchKernelMetadataBridgeErrorV2::NumericOverflow("maximum total workitems"),
    )?;
    Ok(DerivedLaunchGeometryV2 {
        rank: launch.rank(),
        block: BlockShapePolicyV2::Exact(block),
        max_grid_blocks,
        minimum_flat_workgroup_size: flat,
        maximum_flat_workgroup_size: flat,
        max_total_workitems,
    })
}

fn derive_resources(
    descriptor: &KernelDescriptorV1,
    physical: &PublishedKernelPhysicalLayoutV1,
) -> Result<Gfx942ResourceLimitsV2, LaunchKernelMetadataBridgeErrorV2> {
    if descriptor.launch().max_dynamic_shared_memory_bytes() != 0 {
        return Err(LaunchKernelMetadataBridgeErrorV2::UnsupportedDynamicLds);
    }
    let physical = physical.launch();
    let static_lds_bytes = u32::try_from(physical.group_segment_fixed_size()).map_err(|_| {
        LaunchKernelMetadataBridgeErrorV2::NumericOverflow("static LDS segment size")
    })?;
    if static_lds_bytes != descriptor.launch().static_shared_memory_bytes() {
        return Err(
            LaunchKernelMetadataBridgeErrorV2::RecoveredMetadataInconsistent(
                "static LDS segment size",
            ),
        );
    }
    let private_segment_bytes = u32::try_from(physical.private_segment_fixed_size())
        .map_err(|_| LaunchKernelMetadataBridgeErrorV2::NumericOverflow("private segment size"))?;
    Ok(Gfx942ResourceLimitsV2 {
        static_lds_bytes,
        maximum_dynamic_lds_bytes: 0,
        dynamic_lds_alignment: 1,
        private_segment_bytes,
    })
}

fn validate_model_match(
    family: &LaunchKernelFamilyV2,
    variant: &KernelVariantV2,
    derived: &DerivedLaunchMetadataV2,
) -> Result<(), LaunchKernelMetadataBridgeErrorV2> {
    if family.target != derived.target {
        return Err(LaunchKernelMetadataBridgeErrorV2::TargetSubstitution);
    }
    if family.logical_name != derived.logical_name.as_ref() {
        return Err(LaunchKernelMetadataBridgeErrorV2::LogicalNameSubstitution);
    }
    if family.signature != derived.signature {
        return Err(LaunchKernelMetadataBridgeErrorV2::SignatureSubstitution);
    }
    if variant.entry_name != derived.entry_name.as_ref() {
        return Err(LaunchKernelMetadataBridgeErrorV2::EntryNameSubstitution);
    }
    if variant.artifact_identity != derived.artifact_identity {
        return Err(LaunchKernelMetadataBridgeErrorV2::ArtifactSubstitution);
    }
    if variant.kernel_identity != derived.kernel_identity {
        return Err(LaunchKernelMetadataBridgeErrorV2::KernelSubstitution);
    }
    validate_launch_geometry_match(variant.launch, derived.launch)?;
    if variant.resources != derived.resources {
        return Err(LaunchKernelMetadataBridgeErrorV2::ResourceSubstitution);
    }
    let model_subject = variant
        .occupancy_witness
        .as_ref()
        .expect("validated launch model requires an occupancy witness")
        .subject_identity;
    if model_subject != derived.occupancy_subject {
        return Err(LaunchKernelMetadataBridgeErrorV2::OccupancySubjectSubstitution);
    }
    Ok(())
}

fn validate_launch_geometry_match(
    model: Gfx942LaunchContractV2,
    derived: DerivedLaunchGeometryV2,
) -> Result<(), LaunchKernelMetadataBridgeErrorV2> {
    if model.rank != derived.rank
        || model.block != derived.block
        || model.max_grid_blocks != derived.max_grid_blocks
        || model.minimum_flat_workgroup_size != derived.minimum_flat_workgroup_size
        || model.maximum_flat_workgroup_size != derived.maximum_flat_workgroup_size
        || model.wavefront != WavefrontWidthV2::Wave64
        || model.require_full_waves
        || model.max_total_workitems != derived.max_total_workitems
        || model.unsupported != UnsupportedLaunchFeaturesV2::NONE
    {
        return Err(LaunchKernelMetadataBridgeErrorV2::LaunchGeometrySubstitution);
    }
    Ok(())
}

fn checked_dimensions_product(
    value: DimensionsV2,
    field: &'static str,
) -> Result<u64, LaunchKernelMetadataBridgeErrorV2> {
    u64::from(value.x)
        .checked_mul(u64::from(value.y))
        .and_then(|xy| xy.checked_mul(u64::from(value.z)))
        .ok_or(LaunchKernelMetadataBridgeErrorV2::NumericOverflow(field))
}

const fn ownership_tag(value: OwnershipSemantics) -> u8 {
    match value {
        OwnershipSemantics::ByValue => 0,
        OwnershipSemantics::SharedBorrow => 1,
        OwnershipSemantics::UniqueBorrow => 2,
    }
}

const fn access_tag(value: AccessMode) -> u8 {
    match value {
        AccessMode::ByValue => 0,
        AccessMode::ReadOnly => 1,
        AccessMode::WriteOnly => 2,
        AccessMode::ReadWrite => 3,
    }
}

const fn alias_tag(value: AliasSemantics) -> u8 {
    match value {
        AliasSemantics::Value => 0,
        AliasSemantics::SharedReadOnly => 1,
        AliasSemantics::Exclusive => 2,
    }
}

const fn component_tag(value: PhysicalAbiComponentKind) -> u8 {
    match value {
        PhysicalAbiComponentKind::ScalarByValue(scalar) => match scalar {
            ScalarTypeV1::I8 => 1,
            ScalarTypeV1::U8 => 2,
            ScalarTypeV1::I16 => 3,
            ScalarTypeV1::U16 => 4,
            ScalarTypeV1::I32 => 5,
            ScalarTypeV1::U32 => 6,
            ScalarTypeV1::I64 => 7,
            ScalarTypeV1::U64 => 8,
            ScalarTypeV1::F16 => 9,
            ScalarTypeV1::F32 => 10,
            ScalarTypeV1::F64 => 11,
        },
        PhysicalAbiComponentKind::GlobalPointer => 32,
        PhysicalAbiComponentKind::SliceLengthU64 => 33,
    }
}

struct CanonicalDigestV2(Sha256);

impl CanonicalDigestV2 {
    fn new(domain: &[u8]) -> Self {
        let mut digest = Sha256::new();
        digest.update(domain);
        Self(digest)
    }

    fn bytes(&mut self, value: &[u8]) {
        self.u64(value.len() as u64);
        self.0.update(value);
    }

    fn u8(&mut self, value: u8) {
        self.0.update([value]);
    }

    fn u16(&mut self, value: u16) {
        self.0.update(value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.0.update(value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.0.update(value.to_le_bytes());
    }

    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

/// Failure to bind a launch-model variant to current recovered executable metadata.
#[derive(Debug)]
#[non_exhaustive]
pub enum LaunchKernelMetadataBridgeErrorV2 {
    CurrentPublication(FinalizedWorkerV2BundleAdmissionError),
    InvalidLaunchModel(LaunchKernelValidationErrorV2),
    UnknownVariant,
    UnsupportedTarget,
    UnsupportedCodeObjectVersion,
    UnsupportedDigestAlgorithm,
    MissingPhysicalMetadata(&'static str),
    UnsupportedPhysicalAbi(&'static str),
    UnsupportedPhysicalLaunchContract(&'static str),
    UnsupportedDynamicLds,
    NumericOverflow(&'static str),
    RecoveredMetadataInconsistent(&'static str),
    TargetSubstitution,
    LogicalNameSubstitution,
    EntryNameSubstitution,
    ArtifactSubstitution,
    KernelSubstitution,
    SignatureSubstitution,
    LaunchGeometrySubstitution,
    ResourceSubstitution,
    OccupancySubjectSubstitution,
}

impl fmt::Display for LaunchKernelMetadataBridgeErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CurrentPublication(error) => error.fmt(formatter),
            Self::InvalidLaunchModel(error) => write!(formatter, "invalid launch model: {error:?}"),
            Self::UnknownVariant => formatter.write_str("launch variant is absent from the family"),
            Self::UnsupportedTarget => {
                formatter.write_str("launch metadata bridge requires gfx942:xnack- Wave64")
            }
            Self::UnsupportedCodeObjectVersion => {
                formatter.write_str("launch metadata bridge requires code object V6")
            }
            Self::UnsupportedDigestAlgorithm => {
                formatter.write_str("launch metadata bridge requires a SHA-256 payload identity")
            }
            Self::MissingPhysicalMetadata(field) => {
                write!(formatter, "inspected HSACO omitted required {field}")
            }
            Self::UnsupportedPhysicalAbi(field) => {
                write!(formatter, "launch metadata bridge does not support {field}")
            }
            Self::UnsupportedPhysicalLaunchContract(field) => {
                write!(formatter, "launch metadata bridge does not support {field}")
            }
            Self::UnsupportedDynamicLds => formatter.write_str(
                "inspected HSACO cannot establish the dynamic LDS maximum and alignment",
            ),
            Self::NumericOverflow(field) => write!(formatter, "{field} exceeds launch V2 bounds"),
            Self::RecoveredMetadataInconsistent(field) => {
                write!(
                    formatter,
                    "recovered executable metadata disagrees on {field}"
                )
            }
            Self::TargetSubstitution => formatter.write_str("launch target was substituted"),
            Self::LogicalNameSubstitution => {
                formatter.write_str("launch logical name was substituted")
            }
            Self::EntryNameSubstitution => formatter.write_str("launch entry name was substituted"),
            Self::ArtifactSubstitution => formatter.write_str("launch artifact was substituted"),
            Self::KernelSubstitution => formatter.write_str("launch kernel was substituted"),
            Self::SignatureSubstitution => {
                formatter.write_str("launch physical signature was substituted")
            }
            Self::LaunchGeometrySubstitution => {
                formatter.write_str("launch geometry policy was substituted")
            }
            Self::ResourceSubstitution => formatter.write_str("launch resources were substituted"),
            Self::OccupancySubjectSubstitution => {
                formatter.write_str("launch occupancy subject was substituted")
            }
        }
    }
}

impl Error for LaunchKernelMetadataBridgeErrorV2 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CurrentPublication(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
pub(crate) fn canonical_family_for_recovered_launch_bridge_test(
    recovered: &RecoveredWorkerV2PinnedDescriptorV1,
) -> LaunchKernelFamilyV2 {
    use fe2o3_kernel_ir::{
        Gfx942OccupancyWitnessV2, KernelFamilyIdentityV2, KernelPolicyIdentityV2,
        KernelVariantTupleIdentityV2, LaunchCapabilityV2, OccupancyMetadataIdentityV2,
        OccupancyVerifierIdentityV2,
    };

    let current = recovered.acquire_launch_kernel_v2_currentness().unwrap();
    let admission = current.admission();
    let derived = derive_metadata(
        admission.target(),
        admission.code_object_version(),
        admission.artifact_identity(),
        recovered.descriptor(),
        admission.selected_kernel(),
    )
    .unwrap();
    drop(current);

    let mut capabilities = vec![LaunchCapabilityV2::ExactWaveMode];
    if derived.resources.static_lds_bytes != 0 {
        capabilities.push(LaunchCapabilityV2::StaticLds);
    }
    let variant = KernelVariantV2 {
        kernel_identity: derived.kernel_identity,
        policy_identity: KernelPolicyIdentityV2::from_bytes([0x72; 32]),
        artifact_identity: derived.artifact_identity,
        tuple_identity: KernelVariantTupleIdentityV2::from_bytes([0; 32]),
        variant_name: "recovered-exact-wave64".to_owned(),
        entry_name: derived.entry_name.into(),
        launch: Gfx942LaunchContractV2 {
            rank: derived.launch.rank,
            block: derived.launch.block,
            max_grid_blocks: derived.launch.max_grid_blocks,
            minimum_flat_workgroup_size: derived.launch.minimum_flat_workgroup_size,
            maximum_flat_workgroup_size: derived.launch.maximum_flat_workgroup_size,
            wavefront: WavefrontWidthV2::Wave64,
            require_full_waves: false,
            minimum_waves_per_execution_unit: 1,
            maximum_waves_per_execution_unit: 8,
            max_total_workitems: derived.launch.max_total_workitems,
            unsupported: UnsupportedLaunchFeaturesV2::NONE,
        },
        resources: derived.resources,
        occupancy_witness: Some(Gfx942OccupancyWitnessV2 {
            verifier_identity: OccupancyVerifierIdentityV2::from_bytes([0x73; 32]),
            metadata_identity: OccupancyMetadataIdentityV2::from_bytes([0x74; 32]),
            subject_identity: derived.occupancy_subject,
            minimum_waves_per_execution_unit: 1,
            maximum_waves_per_execution_unit: 8,
        }),
        capabilities,
        proof_obligations: vec![],
    };
    let mut family = LaunchKernelFamilyV2 {
        target: derived.target,
        family_identity: KernelFamilyIdentityV2::from_bytes([0x71; 32]),
        logical_name: derived.logical_name.into(),
        signature: derived.signature,
        variants: vec![variant],
    };
    rebind_launch_family_for_bridge_test(&mut family);
    family
}

#[cfg(test)]
pub(crate) fn rebind_launch_family_for_bridge_test(family: &mut LaunchKernelFamilyV2) {
    use fe2o3_kernel_ir::{
        LaunchProofKindV2, LaunchProofObligationV2, canonical_variant_tuple_identity_v2,
    };

    for variant in &mut family.variants {
        let subject = canonical_occupancy_subject_identity_v2(
            &family.target,
            &family.signature,
            variant.artifact_identity,
            &variant.entry_name,
            variant.resources,
        );
        let witness = variant
            .occupancy_witness
            .as_mut()
            .expect("bridge test models always carry an inert occupancy witness");
        witness.subject_identity = subject;
        witness.minimum_waves_per_execution_unit = variant.launch.minimum_waves_per_execution_unit;
        witness.maximum_waves_per_execution_unit = variant.launch.maximum_waves_per_execution_unit;
    }
    for variant in &mut family.variants {
        let tuple = canonical_variant_tuple_identity_v2(
            &family.target,
            family.family_identity,
            &family.logical_name,
            &family.signature,
            variant,
        );
        variant.tuple_identity = tuple;
        variant.proof_obligations = [
            LaunchProofKindV2::TargetAuthenticated,
            LaunchProofKindV2::ArtifactAuthenticated,
            LaunchProofKindV2::KernelIdentityAuthenticated,
            LaunchProofKindV2::SignatureLayoutAuthenticated,
            LaunchProofKindV2::PolicySelectionAuthenticated,
            LaunchProofKindV2::GeometryAndResourcesProved,
        ]
        .into_iter()
        .map(|kind| LaunchProofObligationV2::new(kind, tuple))
        .collect();
    }
}
