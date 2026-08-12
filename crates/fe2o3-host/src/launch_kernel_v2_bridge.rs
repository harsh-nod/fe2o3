use crate::{
    ArtifactKernelIdentityV1, CurrentFinalizedWorkerV2BundleAdmissionV1,
    FinalizedWorkerV2BundleAdmissionError, PhysicalMetadataValueV1,
    PublishedKernelPhysicalLayoutV1, RecoveredWorkerV2PinnedDescriptorV1,
};
use fe2o3_amd_target::{AmdTargetId, FeatureState};
use fe2o3_artifacts::{
    AbiField as ArtifactAbiField, AbiKind as ArtifactAbiKind, AbiLayout as ArtifactAbiLayout,
    Access as ArtifactAccess, AddressSpace as ArtifactAddressSpace,
    AliasClass as ArtifactAliasClass, ArgumentOwnership as ArtifactOwnership,
    BlockSize as ArtifactBlockSize, DigestAlgorithm, Endianness as ArtifactEndianness,
    LaunchContract as ArtifactLaunchContract, PointerWidth, ScalarType as ArtifactScalarType,
};
use fe2o3_hsaco::{
    COV6_IMPLICIT_ARGUMENT_BYTES, CodeObjectVersion, ExplicitValueKind, ExplicitValueType,
    HiddenValueKind,
};
use fe2o3_kernel_descriptor::{
    AccessMode, AliasSemantics, BlockSizeV1, DeviceLayoutDescriptorV1, DeviceLayoutIdentity,
    KernelDescriptorV1, LaunchConstraintsV1, OwnershipSemantics, PhysicalAbiComponentKind,
    RustTypeIdentity, ScalarTypeV1, SourceTypeDescriptorV1,
};
use fe2o3_kernel_ir::{
    AbiParameterKindV2, AbiParameterV2, ArtifactIdentityV2, BlockShapePolicyV2, DimensionsV2,
    Gfx942LaunchContractV2, Gfx942TargetBindingV2, KernelIdentityV2, KernelSignatureIdentityV2,
    KernelSignatureV2, KernelVariantV2, LaunchKernelFamilyV2, LaunchKernelLimitsV2,
    LaunchKernelValidationErrorV2, SemanticTypeIdentityV2, TargetIdentityV2,
    UnsupportedLaunchFeaturesV2, WavefrontWidthV2,
};
use sha2::{Digest, Sha256};
use std::error::Error;
use std::fmt;

const TARGET_IDENTITY_DOMAIN_V2: &[u8] = b"fe2o3.host.launch-kernel.target.v2\0";
const SIGNATURE_IDENTITY_DOMAIN_V2: &[u8] = b"fe2o3.host.launch-kernel.signature.v2\0";
const SEMANTIC_PARAMETER_DOMAIN_V2: &[u8] = b"fe2o3.host.launch-kernel.semantic-parameter.v2\0";
const PHYSICAL_SIGNATURE_DOMAIN_V3: &[u8] = b"fe2o3.host.launch-kernel.physical-signature.v3\0";

/// Identity of the complete explicit and mandatory implicit ABI observed for one COV6 kernel.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Gfx942PhysicalKernelSignatureIdentityV2([u8; 32]);

impl Gfx942PhysicalKernelSignatureIdentityV2 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Runtime value semantics of one mandatory COV6 implicit ABI record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Gfx942ImplicitAbiKindV2 {
    BlockCountX,
    BlockCountY,
    BlockCountZ,
    GroupSizeX,
    GroupSizeY,
    GroupSizeZ,
    RemainderX,
    RemainderY,
    RemainderZ,
    GlobalOffsetX,
    GlobalOffsetY,
    GlobalOffsetZ,
    GridDimensions,
}

/// Exact physical position and semantics of one mandatory COV6 implicit ABI record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942ImplicitAbiParameterV2 {
    kind: Gfx942ImplicitAbiKindV2,
    offset: u32,
    size: u32,
    alignment: u32,
}

impl Gfx942ImplicitAbiParameterV2 {
    pub const fn kind(self) -> Gfx942ImplicitAbiKindV2 {
        self.kind
    }

    pub const fn offset(self) -> u32 {
        self.offset
    }

    pub const fn size(self) -> u32 {
        self.size
    }

    pub const fn alignment(self) -> u32 {
        self.alignment
    }
}

/// Complete physical signature for the narrow gfx942 COV6 bridge profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx942PhysicalKernelSignatureV2 {
    identity: Gfx942PhysicalKernelSignatureIdentityV2,
    explicit: KernelSignatureV2,
    implicit_argument_offset: u32,
    implicit_argument_bytes: u32,
    explicit_value_types: Box<[PhysicalMetadataValueV1<ExplicitValueType>]>,
    implicit_parameters: Box<[Gfx942ImplicitAbiParameterV2]>,
}

impl Gfx942PhysicalKernelSignatureV2 {
    pub const fn identity(&self) -> Gfx942PhysicalKernelSignatureIdentityV2 {
        self.identity
    }

    pub const fn explicit(&self) -> &KernelSignatureV2 {
        &self.explicit
    }

    pub const fn implicit_argument_offset(&self) -> u32 {
        self.implicit_argument_offset
    }

    pub const fn implicit_argument_bytes(&self) -> u32 {
        self.implicit_argument_bytes
    }

    /// Returns each normalized physical `.value_type` declaration in explicit ABI order.
    pub fn explicit_value_types(&self) -> &[PhysicalMetadataValueV1<ExplicitValueType>] {
        &self.explicit_value_types
    }

    pub fn implicit_parameters(&self) -> &[Gfx942ImplicitAbiParameterV2] {
        &self.implicit_parameters
    }
}

/// Physically and artifact-joined launch geometry with no occupancy fields.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942PhysicalLaunchProjectionV2 {
    declared_rank: u8,
    required_block_threads: DimensionsV2,
    declared_maximum_grid_blocks: DimensionsV2,
    physical_maximum_workgroups: DimensionsV2,
    required_flat_workgroup_size: u32,
    physical_maximum_flat_workgroup_size: u32,
    declared_maximum_total_workitems: u64,
    wavefront_width: u32,
}

impl Gfx942PhysicalLaunchProjectionV2 {
    /// Returns the artifact/descriptor rank. AMDHSA does not encode source rank.
    pub const fn declared_rank(self) -> u8 {
        self.declared_rank
    }

    pub const fn required_block_threads(self) -> DimensionsV2 {
        self.required_block_threads
    }

    /// Returns the artifact/descriptor grid ceiling after checking it against physical maxima.
    pub const fn declared_maximum_grid_blocks(self) -> DimensionsV2 {
        self.declared_maximum_grid_blocks
    }

    pub const fn physical_maximum_workgroups(self) -> DimensionsV2 {
        self.physical_maximum_workgroups
    }

    pub const fn required_flat_workgroup_size(self) -> u32 {
        self.required_flat_workgroup_size
    }

    pub const fn physical_maximum_flat_workgroup_size(self) -> u32 {
        self.physical_maximum_flat_workgroup_size
    }

    /// Returns the maximum implied by the checked artifact/descriptor grid and required block.
    pub const fn declared_maximum_total_workitems(self) -> u64 {
        self.declared_maximum_total_workitems
    }

    pub const fn wavefront_width(self) -> u32 {
        self.wavefront_width
    }
}

/// Dynamic-LDS fact established by the narrow bridge profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942DynamicLdsProjectionV2 {
    /// The artifact and descriptor both forbid dynamic LDS and no physical ABI record requests it.
    ArtifactForbidsAndPhysicalAbiOmits,
}

/// Physically and artifact-joined resource facts with no occupancy interpretation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942PhysicalResourceProjectionV2 {
    static_lds_bytes: u32,
    private_segment_bytes: u32,
    dynamic_lds: Gfx942DynamicLdsProjectionV2,
}

impl Gfx942PhysicalResourceProjectionV2 {
    pub const fn static_lds_bytes(self) -> u32 {
        self.static_lds_bytes
    }

    pub const fn private_segment_bytes(self) -> u32 {
        self.private_segment_bytes
    }

    pub const fn dynamic_lds(self) -> Gfx942DynamicLdsProjectionV2 {
        self.dynamic_lds
    }
}

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

/// Current, inert match between one occupancy-independent model projection and one recovered
/// executable kernel.
///
/// The value retains the cooperative current-publication guard, joined artifact/descriptor facts,
/// physical metadata derived from the recovered Worker V2 admission, and the inert caller label
/// used to select the model projection. That label is not an HSACO-observed fact. Caller-supplied
/// policy, proof, occupancy-verifier, and occupancy-metadata identities are not retained and grant
/// no authority. The value has no transition into HSA loading or dispatch.
pub struct CurrentRecoveredLaunchKernelMetadataV2<'recovered> {
    _current: CurrentFinalizedWorkerV2BundleAdmissionV1<'recovered>,
    target: Gfx942TargetBindingV2,
    artifact_identity: ArtifactIdentityV2,
    kernel_identity: KernelIdentityV2,
    physical_signature: Gfx942PhysicalKernelSignatureV2,
    launch: Gfx942PhysicalLaunchProjectionV2,
    resources: Gfx942PhysicalResourceProjectionV2,
    model_projection_name: Box<str>,
}

impl fmt::Debug for CurrentRecoveredLaunchKernelMetadataV2<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CurrentRecoveredLaunchKernelMetadataV2")
            .field("target", &self.target)
            .field("artifact_identity", &self.artifact_identity)
            .field("kernel_identity", &self.kernel_identity)
            .field("physical_signature", &self.physical_signature)
            .field("launch", &self.launch)
            .field("resources", &self.resources)
            .field("model_projection_name", &self.model_projection_name)
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

    pub const fn physical_signature(&self) -> &Gfx942PhysicalKernelSignatureV2 {
        &self.physical_signature
    }

    pub const fn launch_projection(&self) -> Gfx942PhysicalLaunchProjectionV2 {
        self.launch
    }

    pub const fn resource_projection(&self) -> Gfx942PhysicalResourceProjectionV2 {
        self.resources
    }

    pub fn model_projection_name(&self) -> &str {
        &self.model_projection_name
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

/// Binds one named occupancy-independent launch-model projection to current recovered metadata.
///
/// This validates target, payload, kernel, symbol, the embedded descriptor's flattened signature
/// against AMDHSA physical arguments, non-occupancy launch geometry, and static/private resources.
/// Occupancy bounds, witnesses, subjects, variant tuples, policy identities, capabilities, and
/// proof records are neither read nor retained. Dynamic LDS is rejected because the narrow model
/// has no complete physical maximum/alignment derivation. The V2 semantic profile is intentionally
/// limited to canonical descriptor scalars and scalar slices. Standalone pointers and nested
/// reference elements are rejected because descriptor V1 cannot express their semantic identity.
pub fn bind_current_recovered_launch_kernel_metadata_v2<'recovered>(
    recovered: &'recovered RecoveredWorkerV2PinnedDescriptorV1,
    family: &LaunchKernelFamilyV2,
    projection_name: &str,
) -> Result<CurrentRecoveredLaunchKernelMetadataV2<'recovered>, LaunchKernelMetadataBridgeErrorV2> {
    let variant = validate_and_select_projection(family, projection_name)?;
    let current = recovered
        .acquire_launch_kernel_v2_currentness()
        .map_err(LaunchKernelMetadataBridgeErrorV2::CurrentPublication)?;
    bind_with_current_and_physical_override(current, recovered, family, variant, None)
}

fn validate_and_select_projection<'family>(
    family: &'family LaunchKernelFamilyV2,
    projection_name: &str,
) -> Result<&'family KernelVariantV2, LaunchKernelMetadataBridgeErrorV2> {
    let limits = LaunchKernelLimitsV2::default();
    family
        .validate_variant_count(&limits)
        .map_err(LaunchKernelMetadataBridgeErrorV2::InvalidLaunchModel)?;
    validate_projection_name(projection_name, &limits)?;
    validate_projection_name(&family.logical_name, &limits)?;
    if family.signature.parameters.len() > limits.max_parameters {
        return Err(LaunchKernelMetadataBridgeErrorV2::InvalidLaunchModel(
            LaunchKernelValidationErrorV2::ResourceLimit {
                resource: "parameters",
                observed: family.signature.parameters.len(),
                limit: limits.max_parameters,
            },
        ));
    }
    let mut selected = None;
    for variant in &family.variants {
        validate_projection_name(&variant.variant_name, &limits)?;
        if variant.variant_name == projection_name {
            if selected.is_some() {
                return Err(LaunchKernelMetadataBridgeErrorV2::AmbiguousModelProjection);
            }
            selected = Some(variant);
        }
    }
    let selected = selected.ok_or(LaunchKernelMetadataBridgeErrorV2::UnknownModelProjection)?;
    validate_projection_name(&selected.entry_name, &limits)?;
    Ok(selected)
}

fn validate_projection_name(
    name: &str,
    limits: &LaunchKernelLimitsV2,
) -> Result<(), LaunchKernelMetadataBridgeErrorV2> {
    if name.len() > limits.max_name_bytes {
        return Err(LaunchKernelMetadataBridgeErrorV2::InvalidLaunchModel(
            LaunchKernelValidationErrorV2::ResourceLimit {
                resource: "name bytes",
                observed: name.len(),
                limit: limits.max_name_bytes,
            },
        ));
    }
    let bytes = name.as_bytes();
    if bytes.is_empty()
        || !matches!(bytes[0], b'A'..=b'Z' | b'a'..=b'z' | b'_')
        || !bytes.iter().all(|byte| {
            matches!(byte, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'$' | b'-')
        })
    {
        return Err(LaunchKernelMetadataBridgeErrorV2::InvalidLaunchModel(
            LaunchKernelValidationErrorV2::InvalidName,
        ));
    }
    Ok(())
}

fn bind_with_current_and_physical_override<'recovered>(
    current: CurrentFinalizedWorkerV2BundleAdmissionV1<'recovered>,
    recovered: &RecoveredWorkerV2PinnedDescriptorV1,
    family: &LaunchKernelFamilyV2,
    variant: &KernelVariantV2,
    physical_override: Option<&PublishedKernelPhysicalLayoutV1>,
) -> Result<CurrentRecoveredLaunchKernelMetadataV2<'recovered>, LaunchKernelMetadataBridgeErrorV2> {
    let derived = {
        let admission = current.admission();
        derive_metadata(
            admission.target(),
            admission.code_object_version(),
            admission.artifact_identity(),
            recovered.descriptor(),
            physical_override.unwrap_or_else(|| admission.selected_kernel()),
        )?
    };
    validate_model_match(family, variant, &derived)?;

    Ok(CurrentRecoveredLaunchKernelMetadataV2 {
        _current: current,
        target: derived.target,
        artifact_identity: derived.artifact_identity,
        kernel_identity: derived.kernel_identity,
        physical_signature: derived.physical_signature,
        launch: derived.launch,
        resources: derived.resources,
        model_projection_name: variant.variant_name.clone().into_boxed_str(),
    })
}

#[cfg(test)]
pub(crate) fn bind_current_recovered_launch_kernel_metadata_with_physical_probe_v2<'recovered>(
    recovered: &'recovered RecoveredWorkerV2PinnedDescriptorV1,
    family: &LaunchKernelFamilyV2,
    projection_name: &str,
    physical: &PublishedKernelPhysicalLayoutV1,
) -> Result<CurrentRecoveredLaunchKernelMetadataV2<'recovered>, LaunchKernelMetadataBridgeErrorV2> {
    let variant = validate_and_select_projection(family, projection_name)?;
    let current = recovered
        .acquire_launch_kernel_v2_currentness()
        .map_err(LaunchKernelMetadataBridgeErrorV2::CurrentPublication)?;
    bind_with_current_and_physical_override(current, recovered, family, variant, Some(physical))
}

struct DerivedLaunchMetadataV2 {
    target: Gfx942TargetBindingV2,
    artifact_identity: ArtifactIdentityV2,
    kernel_identity: KernelIdentityV2,
    logical_name: Box<str>,
    entry_name: Box<str>,
    signature: KernelSignatureV2,
    physical_signature: Gfx942PhysicalKernelSignatureV2,
    launch: Gfx942PhysicalLaunchProjectionV2,
    resources: Gfx942PhysicalResourceProjectionV2,
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
    let signature = derive_signature(artifact.abi(), descriptor, physical)?;
    let physical_signature = derive_physical_signature(signature.clone(), physical)?;
    let launch = derive_launch_geometry(artifact.launch(), physical)?;
    let resources = derive_resources(artifact.launch(), physical)?;

    Ok(DerivedLaunchMetadataV2 {
        target,
        artifact_identity,
        kernel_identity,
        logical_name: descriptor.logical_name().as_str().into(),
        entry_name: physical.export_symbol().into(),
        signature,
        physical_signature,
        launch,
        resources,
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
    validate_descriptor_artifact_launch(artifact.launch(), descriptor.launch())?;
    validate_descriptor_artifact_abi(artifact.abi(), descriptor)
}

fn validate_descriptor_artifact_abi(
    artifact: &ArtifactAbiLayout,
    descriptor: &KernelDescriptorV1,
) -> Result<(), LaunchKernelMetadataBridgeErrorV2> {
    if artifact.fields().len() != descriptor.arguments().len() {
        return Err(
            LaunchKernelMetadataBridgeErrorV2::RecoveredMetadataInconsistent(
                "artifact ABI argument count",
            ),
        );
    }
    for (index, (field, argument)) in artifact
        .fields()
        .iter()
        .zip(descriptor.arguments())
        .enumerate()
    {
        if usize::from(argument.source_index()) != index {
            return Err(
                LaunchKernelMetadataBridgeErrorV2::RecoveredMetadataInconsistent(
                    "artifact ABI argument order",
                ),
            );
        }
        let checks = [
            (
                field.name().as_str() == argument.name().as_str(),
                "artifact ABI argument name",
            ),
            (
                artifact_ownership_matches(field.ownership(), argument.ownership()),
                "artifact ABI argument ownership",
            ),
            (
                artifact_access_matches(field.access(), argument.access()),
                "artifact ABI argument access",
            ),
            (
                artifact_alias_matches(field.alias_class(), argument.alias()),
                "artifact ABI argument aliasing",
            ),
        ];
        for (matches, name) in checks {
            if !matches {
                return Err(LaunchKernelMetadataBridgeErrorV2::RecoveredMetadataInconsistent(name));
            }
        }
        validate_artifact_argument_semantics(field, argument)?;
        validate_artifact_argument_components(artifact, field, argument)?;
    }
    Ok(())
}

fn validate_artifact_argument_semantics(
    field: &ArtifactAbiField,
    argument: &fe2o3_kernel_descriptor::LogicalArgumentV1,
) -> Result<(), LaunchKernelMetadataBridgeErrorV2> {
    if field.type_identity().rust_type().bytes().as_bytes() != argument.source_type().as_bytes() {
        return Err(
            LaunchKernelMetadataBridgeErrorV2::RecoveredMetadataInconsistent(
                "artifact ABI source type identity",
            ),
        );
    }
    if field.type_identity().layout().bytes().as_bytes() != argument.device_layout().as_bytes() {
        return Err(
            LaunchKernelMetadataBridgeErrorV2::RecoveredMetadataInconsistent(
                "artifact ABI device layout identity",
            ),
        );
    }

    match field.kind() {
        ArtifactAbiKind::Scalar(scalar) => validate_descriptor_semantic_identity(
            argument,
            SourceTypeDescriptorV1::scalar(descriptor_scalar(scalar)),
            DeviceLayoutDescriptorV1::scalar(descriptor_scalar(scalar)),
        ),
        ArtifactAbiKind::Slice {
            element_size,
            element_alignment,
        } => {
            let scalar = canonical_slice_scalar(argument).ok_or(
                LaunchKernelMetadataBridgeErrorV2::RecoveredMetadataInconsistent(
                    "artifact ABI slice semantic identity",
                ),
            )?;
            if element_size != u64::from(scalar.size_bytes())
                || element_alignment != u32::from(scalar.alignment_bytes())
            {
                return Err(
                    LaunchKernelMetadataBridgeErrorV2::RecoveredMetadataInconsistent(
                        "artifact ABI slice element layout",
                    ),
                );
            }
            Ok(())
        }
        ArtifactAbiKind::Pointer { .. } => {
            Err(LaunchKernelMetadataBridgeErrorV2::UnsupportedPhysicalAbi(
                "standalone pointers without a descriptor semantic kind",
            ))
        }
    }
}

fn canonical_slice_scalar(
    argument: &fe2o3_kernel_descriptor::LogicalArgumentV1,
) -> Option<ScalarTypeV1> {
    const SCALARS: [ScalarTypeV1; 11] = [
        ScalarTypeV1::I8,
        ScalarTypeV1::U8,
        ScalarTypeV1::I16,
        ScalarTypeV1::U16,
        ScalarTypeV1::I32,
        ScalarTypeV1::U32,
        ScalarTypeV1::I64,
        ScalarTypeV1::U64,
        ScalarTypeV1::F16,
        ScalarTypeV1::F32,
        ScalarTypeV1::F64,
    ];
    SCALARS.into_iter().find(|scalar| {
        let (source, layout) = match argument.ownership() {
            OwnershipSemantics::SharedBorrow => (
                SourceTypeDescriptorV1::shared_slice(*scalar),
                DeviceLayoutDescriptorV1::shared_slice(*scalar),
            ),
            OwnershipSemantics::UniqueBorrow => (
                SourceTypeDescriptorV1::disjoint_slice(*scalar),
                DeviceLayoutDescriptorV1::disjoint_slice(*scalar),
            ),
            OwnershipSemantics::ByValue => return false,
        };
        descriptor_semantic_identity_matches(argument, &source, &layout)
    })
}

fn validate_descriptor_semantic_identity(
    argument: &fe2o3_kernel_descriptor::LogicalArgumentV1,
    source: SourceTypeDescriptorV1,
    layout: DeviceLayoutDescriptorV1,
) -> Result<(), LaunchKernelMetadataBridgeErrorV2> {
    if descriptor_semantic_identity_matches(argument, &source, &layout) {
        Ok(())
    } else {
        Err(
            LaunchKernelMetadataBridgeErrorV2::RecoveredMetadataInconsistent(
                "artifact ABI scalar semantic identity",
            ),
        )
    }
}

fn descriptor_semantic_identity_matches(
    argument: &fe2o3_kernel_descriptor::LogicalArgumentV1,
    source: &SourceTypeDescriptorV1,
    layout: &DeviceLayoutDescriptorV1,
) -> bool {
    argument.source_type() == RustTypeIdentity::for_descriptor(source)
        && argument.device_layout() == DeviceLayoutIdentity::for_descriptor(layout)
}

fn validate_artifact_argument_components(
    artifact: &ArtifactAbiLayout,
    field: &ArtifactAbiField,
    argument: &fe2o3_kernel_descriptor::LogicalArgumentV1,
) -> Result<(), LaunchKernelMetadataBridgeErrorV2> {
    let components = argument.physical_components().collect::<Vec<_>>();
    let field_offset = u32::try_from(field.offset())
        .map_err(|_| LaunchKernelMetadataBridgeErrorV2::NumericOverflow("artifact ABI offset"))?;
    let field_size = u16::try_from(field.size())
        .map_err(|_| LaunchKernelMetadataBridgeErrorV2::NumericOverflow("artifact ABI size"))?;
    let field_alignment = u16::try_from(field.alignment()).map_err(|_| {
        LaunchKernelMetadataBridgeErrorV2::NumericOverflow("artifact ABI alignment")
    })?;
    let pointer_bytes = u16::try_from(artifact.pointer_width().bytes()).map_err(|_| {
        LaunchKernelMetadataBridgeErrorV2::NumericOverflow("artifact ABI pointer width")
    })?;
    let matches = match field.kind() {
        ArtifactAbiKind::Scalar(scalar) => {
            components.as_slice()
                == [(
                    PhysicalAbiComponentKind::ScalarByValue(descriptor_scalar(scalar)),
                    field_offset,
                    field_size,
                    field_alignment,
                )]
        }
        ArtifactAbiKind::Pointer { .. } => {
            field.address_space() == ArtifactAddressSpace::Global
                && components.as_slice()
                    == [(
                        PhysicalAbiComponentKind::GlobalPointer,
                        field_offset,
                        pointer_bytes,
                        field_alignment,
                    )]
        }
        ArtifactAbiKind::Slice { .. } => {
            let length_offset = field_offset.checked_add(u32::from(pointer_bytes)).ok_or(
                LaunchKernelMetadataBridgeErrorV2::NumericOverflow(
                    "artifact ABI slice length offset",
                ),
            )?;
            field.address_space() == ArtifactAddressSpace::Global
                && field_size == pointer_bytes.saturating_mul(2)
                && components.as_slice()
                    == [
                        (
                            PhysicalAbiComponentKind::GlobalPointer,
                            field_offset,
                            pointer_bytes,
                            field_alignment,
                        ),
                        (
                            PhysicalAbiComponentKind::SliceLengthU64,
                            length_offset,
                            pointer_bytes,
                            field_alignment,
                        ),
                    ]
        }
    };
    if !matches {
        return Err(
            LaunchKernelMetadataBridgeErrorV2::RecoveredMetadataInconsistent(
                "artifact ABI physical components",
            ),
        );
    }
    Ok(())
}

const fn descriptor_scalar(value: ArtifactScalarType) -> ScalarTypeV1 {
    match value {
        ArtifactScalarType::I8 => ScalarTypeV1::I8,
        ArtifactScalarType::U8 => ScalarTypeV1::U8,
        ArtifactScalarType::I16 => ScalarTypeV1::I16,
        ArtifactScalarType::U16 => ScalarTypeV1::U16,
        ArtifactScalarType::I32 => ScalarTypeV1::I32,
        ArtifactScalarType::U32 => ScalarTypeV1::U32,
        ArtifactScalarType::I64 => ScalarTypeV1::I64,
        ArtifactScalarType::U64 => ScalarTypeV1::U64,
        ArtifactScalarType::F16 => ScalarTypeV1::F16,
        ArtifactScalarType::F32 => ScalarTypeV1::F32,
        ArtifactScalarType::F64 => ScalarTypeV1::F64,
    }
}

const fn artifact_ownership_matches(
    artifact: ArtifactOwnership,
    descriptor: OwnershipSemantics,
) -> bool {
    matches!(
        (artifact, descriptor),
        (ArtifactOwnership::ByValue, OwnershipSemantics::ByValue)
            | (
                ArtifactOwnership::SharedBorrow,
                OwnershipSemantics::SharedBorrow
            )
            | (
                ArtifactOwnership::UniqueBorrow,
                OwnershipSemantics::UniqueBorrow
            )
    )
}

const fn artifact_access_matches(artifact: ArtifactAccess, descriptor: AccessMode) -> bool {
    matches!(
        (artifact, descriptor),
        (ArtifactAccess::ByValue, AccessMode::ByValue)
            | (ArtifactAccess::ReadOnly, AccessMode::ReadOnly)
            | (ArtifactAccess::WriteOnly, AccessMode::WriteOnly)
            | (ArtifactAccess::ReadWrite, AccessMode::ReadWrite)
    )
}

const fn artifact_alias_matches(artifact: ArtifactAliasClass, descriptor: AliasSemantics) -> bool {
    matches!(
        (artifact, descriptor),
        (ArtifactAliasClass::Value, AliasSemantics::Value)
            | (
                ArtifactAliasClass::SharedReadOnly,
                AliasSemantics::SharedReadOnly
            )
            | (ArtifactAliasClass::Exclusive, AliasSemantics::Exclusive)
    )
}

fn validate_descriptor_artifact_launch(
    artifact: &ArtifactLaunchContract,
    descriptor: &LaunchConstraintsV1,
) -> Result<(), LaunchKernelMetadataBridgeErrorV2> {
    let artifact_block = artifact.block_size();
    let descriptor_block = descriptor.block_size();
    let block_matches = match (artifact_block, descriptor_block) {
        (ArtifactBlockSize::Any, BlockSizeV1::Any) => true,
        (ArtifactBlockSize::Exact(artifact), BlockSizeV1::Exact(descriptor))
        | (ArtifactBlockSize::AtMost(artifact), BlockSizeV1::AtMost(descriptor)) => {
            [artifact.x(), artifact.y(), artifact.z()]
                == [descriptor.x(), descriptor.y(), descriptor.z()]
        }
        _ => false,
    };
    let artifact_grid = artifact.max_grid();
    let descriptor_grid = descriptor.max_grid();
    let checks = [
        (artifact.rank() == descriptor.rank(), "artifact launch rank"),
        (block_matches, "artifact launch block size"),
        (
            [artifact_grid.x(), artifact_grid.y(), artifact_grid.z()]
                == [
                    descriptor_grid.x(),
                    descriptor_grid.y(),
                    descriptor_grid.z(),
                ],
            "artifact maximum grid",
        ),
        (
            artifact.static_shared_memory_bytes() == descriptor.static_shared_memory_bytes(),
            "artifact static LDS limit",
        ),
        (
            artifact.max_dynamic_shared_memory_bytes()
                == descriptor.max_dynamic_shared_memory_bytes(),
            "artifact dynamic LDS limit",
        ),
    ];
    for (matches, field) in checks {
        if !matches {
            return Err(LaunchKernelMetadataBridgeErrorV2::RecoveredMetadataInconsistent(field));
        }
    }

    let ArtifactBlockSize::Exact(block) = artifact_block else {
        return Err(
            LaunchKernelMetadataBridgeErrorV2::UnsupportedPhysicalLaunchContract(
                "non-exact artifact block policy",
            ),
        );
    };
    let flat = u64::from(block.x())
        .checked_mul(u64::from(block.y()))
        .and_then(|xy| xy.checked_mul(u64::from(block.z())))
        .ok_or(LaunchKernelMetadataBridgeErrorV2::NumericOverflow(
            "artifact flat workgroup size",
        ))?;
    if flat != u64::from(descriptor.max_flat_workgroup_size()) {
        return Err(
            LaunchKernelMetadataBridgeErrorV2::RecoveredMetadataInconsistent(
                "artifact maximum flat workgroup size",
            ),
        );
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
    artifact_abi: &ArtifactAbiLayout,
    descriptor: &KernelDescriptorV1,
    physical: &PublishedKernelPhysicalLayoutV1,
) -> Result<KernelSignatureV2, LaunchKernelMetadataBridgeErrorV2> {
    let mut parameters = Vec::with_capacity(physical.arguments().len());
    let mut physical_index = 0_usize;
    for (artifact_field, argument) in artifact_abi.fields().iter().zip(descriptor.arguments()) {
        for (component_index, (kind, offset, size, alignment)) in
            argument.physical_components().enumerate()
        {
            let physical_argument = physical.arguments().get(physical_index).ok_or(
                LaunchKernelMetadataBridgeErrorV2::RecoveredMetadataInconsistent(
                    "physical argument count",
                ),
            )?;
            let model_kind = model_parameter_kind(kind, artifact_field.ownership())?;
            validate_physical_component(
                kind,
                offset,
                size,
                alignment,
                artifact_field,
                argument,
                physical_argument,
            )?;
            let source_index = u16::try_from(physical_index).map_err(|_| {
                LaunchKernelMetadataBridgeErrorV2::NumericOverflow("physical argument index")
            })?;
            parameters.push(AbiParameterV2 {
                source_index,
                kind: model_kind,
                semantic_type: derive_semantic_parameter_identity(
                    argument,
                    artifact_field,
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

const COV6_MANDATORY_IMPLICIT_ABI_V2: [(u64, u64, HiddenValueKind, Gfx942ImplicitAbiKindV2); 13] = [
    (
        0,
        4,
        HiddenValueKind::BlockCountX,
        Gfx942ImplicitAbiKindV2::BlockCountX,
    ),
    (
        4,
        4,
        HiddenValueKind::BlockCountY,
        Gfx942ImplicitAbiKindV2::BlockCountY,
    ),
    (
        8,
        4,
        HiddenValueKind::BlockCountZ,
        Gfx942ImplicitAbiKindV2::BlockCountZ,
    ),
    (
        12,
        2,
        HiddenValueKind::GroupSizeX,
        Gfx942ImplicitAbiKindV2::GroupSizeX,
    ),
    (
        14,
        2,
        HiddenValueKind::GroupSizeY,
        Gfx942ImplicitAbiKindV2::GroupSizeY,
    ),
    (
        16,
        2,
        HiddenValueKind::GroupSizeZ,
        Gfx942ImplicitAbiKindV2::GroupSizeZ,
    ),
    (
        18,
        2,
        HiddenValueKind::RemainderX,
        Gfx942ImplicitAbiKindV2::RemainderX,
    ),
    (
        20,
        2,
        HiddenValueKind::RemainderY,
        Gfx942ImplicitAbiKindV2::RemainderY,
    ),
    (
        22,
        2,
        HiddenValueKind::RemainderZ,
        Gfx942ImplicitAbiKindV2::RemainderZ,
    ),
    (
        40,
        8,
        HiddenValueKind::GlobalOffsetX,
        Gfx942ImplicitAbiKindV2::GlobalOffsetX,
    ),
    (
        48,
        8,
        HiddenValueKind::GlobalOffsetY,
        Gfx942ImplicitAbiKindV2::GlobalOffsetY,
    ),
    (
        56,
        8,
        HiddenValueKind::GlobalOffsetZ,
        Gfx942ImplicitAbiKindV2::GlobalOffsetZ,
    ),
    (
        64,
        2,
        HiddenValueKind::GridDimensions,
        Gfx942ImplicitAbiKindV2::GridDimensions,
    ),
];

fn derive_physical_signature(
    explicit: KernelSignatureV2,
    physical: &PublishedKernelPhysicalLayoutV1,
) -> Result<Gfx942PhysicalKernelSignatureV2, LaunchKernelMetadataBridgeErrorV2> {
    let launch = physical.launch();
    if launch.implicit_argument_size() != COV6_IMPLICIT_ARGUMENT_BYTES {
        return Err(
            LaunchKernelMetadataBridgeErrorV2::RecoveredMetadataInconsistent(
                "COV6 implicit argument span",
            ),
        );
    }
    let implicit_argument_offset = match launch.implicit_argument_offset() {
        PhysicalMetadataValueV1::Known(value) => u32::try_from(value).map_err(|_| {
            LaunchKernelMetadataBridgeErrorV2::NumericOverflow("implicit argument offset")
        })?,
        PhysicalMetadataValueV1::Unknown => {
            return Err(LaunchKernelMetadataBridgeErrorV2::MissingPhysicalMetadata(
                "COV6 implicit argument offset",
            ));
        }
    };
    let hidden = physical.hidden_arguments();
    if let Some(optional) = hidden.get(COV6_MANDATORY_IMPLICIT_ABI_V2.len()) {
        if optional.value_kind() == HiddenValueKind::DynamicLdsSize {
            return Err(LaunchKernelMetadataBridgeErrorV2::UnsupportedDynamicLds);
        }
        return Err(LaunchKernelMetadataBridgeErrorV2::UnsupportedPhysicalAbi(
            "optional COV6 hidden arguments",
        ));
    }
    if hidden.len() != COV6_MANDATORY_IMPLICIT_ABI_V2.len() {
        return Err(LaunchKernelMetadataBridgeErrorV2::MissingPhysicalMetadata(
            "mandatory COV6 implicit ABI records",
        ));
    }

    let base = u64::from(implicit_argument_offset);
    let mut implicit_parameters = Vec::with_capacity(COV6_MANDATORY_IMPLICIT_ABI_V2.len());
    for (actual, &(relative_offset, size, physical_kind, model_kind)) in
        hidden.iter().zip(COV6_MANDATORY_IMPLICIT_ABI_V2.iter())
    {
        let expected_offset = base.checked_add(relative_offset).ok_or(
            LaunchKernelMetadataBridgeErrorV2::NumericOverflow("implicit argument offset"),
        )?;
        if actual.offset() != expected_offset
            || actual.size() != size
            || actual.value_kind() != physical_kind
        {
            return Err(
                LaunchKernelMetadataBridgeErrorV2::RecoveredMetadataInconsistent(
                    "mandatory COV6 implicit ABI record",
                ),
            );
        }
        implicit_parameters.push(Gfx942ImplicitAbiParameterV2 {
            kind: model_kind,
            offset: u32::try_from(expected_offset).map_err(|_| {
                LaunchKernelMetadataBridgeErrorV2::NumericOverflow("implicit argument offset")
            })?,
            size: u32::try_from(size).map_err(|_| {
                LaunchKernelMetadataBridgeErrorV2::NumericOverflow("implicit argument size")
            })?,
            alignment: u32::try_from(size).map_err(|_| {
                LaunchKernelMetadataBridgeErrorV2::NumericOverflow("implicit argument alignment")
            })?,
        });
    }
    let implicit_argument_bytes = u32::try_from(launch.implicit_argument_size()).map_err(|_| {
        LaunchKernelMetadataBridgeErrorV2::NumericOverflow("implicit argument span")
    })?;
    let explicit_value_types = physical
        .arguments()
        .iter()
        .map(crate::PublishedPhysicalArgumentLayoutV1::value_type)
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let mut value = Gfx942PhysicalKernelSignatureV2 {
        identity: Gfx942PhysicalKernelSignatureIdentityV2([0; 32]),
        explicit,
        implicit_argument_offset,
        implicit_argument_bytes,
        explicit_value_types,
        implicit_parameters: implicit_parameters.into_boxed_slice(),
    };
    value.identity = derive_physical_signature_identity(&value);
    Ok(value)
}

fn derive_physical_signature_identity(
    signature: &Gfx942PhysicalKernelSignatureV2,
) -> Gfx942PhysicalKernelSignatureIdentityV2 {
    let mut digest = CanonicalDigestV2::new(PHYSICAL_SIGNATURE_DOMAIN_V3);
    digest.bytes(&signature.explicit.identity.0);
    digest.u32(signature.implicit_argument_offset);
    digest.u32(signature.implicit_argument_bytes);
    digest.u64(signature.explicit_value_types.len() as u64);
    for value_type in &signature.explicit_value_types {
        match value_type {
            PhysicalMetadataValueV1::Unknown => digest.u8(0),
            PhysicalMetadataValueV1::Known(value_type) => {
                digest.u8(1);
                digest.u8(explicit_value_type_tag(*value_type));
            }
        }
    }
    digest.u64(signature.implicit_parameters.len() as u64);
    for parameter in &signature.implicit_parameters {
        digest.u8(implicit_kind_tag(parameter.kind));
        digest.u32(parameter.offset);
        digest.u32(parameter.size);
        digest.u32(parameter.alignment);
    }
    Gfx942PhysicalKernelSignatureIdentityV2(digest.finish())
}

const fn implicit_kind_tag(value: Gfx942ImplicitAbiKindV2) -> u8 {
    match value {
        Gfx942ImplicitAbiKindV2::BlockCountX => 1,
        Gfx942ImplicitAbiKindV2::BlockCountY => 2,
        Gfx942ImplicitAbiKindV2::BlockCountZ => 3,
        Gfx942ImplicitAbiKindV2::GroupSizeX => 4,
        Gfx942ImplicitAbiKindV2::GroupSizeY => 5,
        Gfx942ImplicitAbiKindV2::GroupSizeZ => 6,
        Gfx942ImplicitAbiKindV2::RemainderX => 7,
        Gfx942ImplicitAbiKindV2::RemainderY => 8,
        Gfx942ImplicitAbiKindV2::RemainderZ => 9,
        Gfx942ImplicitAbiKindV2::GlobalOffsetX => 10,
        Gfx942ImplicitAbiKindV2::GlobalOffsetY => 11,
        Gfx942ImplicitAbiKindV2::GlobalOffsetZ => 12,
        Gfx942ImplicitAbiKindV2::GridDimensions => 13,
    }
}

fn model_parameter_kind(
    component: PhysicalAbiComponentKind,
    ownership: ArtifactOwnership,
) -> Result<AbiParameterKindV2, LaunchKernelMetadataBridgeErrorV2> {
    match component {
        PhysicalAbiComponentKind::ScalarByValue(_) | PhysicalAbiComponentKind::SliceLengthU64 => {
            Ok(AbiParameterKindV2::ByValue)
        }
        PhysicalAbiComponentKind::GlobalPointer => match ownership {
            ArtifactOwnership::SharedBorrow => Ok(AbiParameterKindV2::SharedGlobalPointer),
            ArtifactOwnership::UniqueBorrow => Ok(AbiParameterKindV2::UniqueGlobalPointer),
            ArtifactOwnership::ByValue | ArtifactOwnership::RawPointer => {
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
    artifact_field: &ArtifactAbiField,
    logical_argument: &fe2o3_kernel_descriptor::LogicalArgumentV1,
    physical: &crate::PublishedPhysicalArgumentLayoutV1,
) -> Result<(), LaunchKernelMetadataBridgeErrorV2> {
    let expected_kind = match component {
        PhysicalAbiComponentKind::ScalarByValue(_) | PhysicalAbiComponentKind::SliceLengthU64 => {
            ExplicitValueKind::ByValue
        }
        PhysicalAbiComponentKind::GlobalPointer => ExplicitValueKind::GlobalBuffer,
    };
    let physical_alignment = match physical.alignment() {
        PhysicalMetadataValueV1::Known(value) => value,
        PhysicalMetadataValueV1::Unknown => {
            return Err(LaunchKernelMetadataBridgeErrorV2::MissingPhysicalMetadata(
                "physical argument alignment",
            ));
        }
    };
    if physical.offset() != u64::from(offset)
        || physical.size() != u64::from(size)
        || physical_alignment != u64::from(alignment)
        || physical.value_kind() != expected_kind
    {
        return Err(
            LaunchKernelMetadataBridgeErrorV2::RecoveredMetadataInconsistent(
                "physical argument component",
            ),
        );
    }
    let expected_value_type = match component {
        PhysicalAbiComponentKind::ScalarByValue(scalar) => explicit_value_type(scalar),
        PhysicalAbiComponentKind::SliceLengthU64 => ExplicitValueType::U64,
        PhysicalAbiComponentKind::GlobalPointer => {
            let scalar = canonical_slice_scalar(logical_argument).ok_or(
                LaunchKernelMetadataBridgeErrorV2::UnsupportedPhysicalAbi(
                    "pointer value type without canonical slice semantics",
                ),
            )?;
            explicit_value_type(scalar)
        }
    };
    if matches!(
        physical.value_type(),
        PhysicalMetadataValueV1::Known(value_type) if value_type != expected_value_type
    ) {
        return Err(
            LaunchKernelMetadataBridgeErrorV2::RecoveredMetadataInconsistent(
                "physical argument value type",
            ),
        );
    }
    match component {
        PhysicalAbiComponentKind::GlobalPointer => {
            let expected = match artifact_field.kind() {
                ArtifactAbiKind::Slice {
                    element_alignment, ..
                } => u64::from(element_alignment),
                ArtifactAbiKind::Pointer {
                    pointee_alignment, ..
                } => u64::from(pointee_alignment),
                ArtifactAbiKind::Scalar(_) => {
                    return Err(
                        LaunchKernelMetadataBridgeErrorV2::RecoveredMetadataInconsistent(
                            "pointer component semantic kind",
                        ),
                    );
                }
            };
            match physical.pointee_alignment() {
                PhysicalMetadataValueV1::Known(value) if value == expected => {}
                PhysicalMetadataValueV1::Known(_) => {
                    return Err(
                        LaunchKernelMetadataBridgeErrorV2::RecoveredMetadataInconsistent(
                            "physical pointee alignment",
                        ),
                    );
                }
                PhysicalMetadataValueV1::Unknown => {
                    return Err(LaunchKernelMetadataBridgeErrorV2::MissingPhysicalMetadata(
                        "physical pointee alignment",
                    ));
                }
            }
        }
        PhysicalAbiComponentKind::ScalarByValue(_) | PhysicalAbiComponentKind::SliceLengthU64 => {
            if !matches!(
                physical.pointee_alignment(),
                PhysicalMetadataValueV1::Unknown
            ) {
                return Err(
                    LaunchKernelMetadataBridgeErrorV2::RecoveredMetadataInconsistent(
                        "non-pointer pointee alignment",
                    ),
                );
            }
        }
    }
    Ok(())
}

const fn explicit_value_type(value: ScalarTypeV1) -> ExplicitValueType {
    match value {
        ScalarTypeV1::I8 => ExplicitValueType::I8,
        ScalarTypeV1::U8 => ExplicitValueType::U8,
        ScalarTypeV1::I16 => ExplicitValueType::I16,
        ScalarTypeV1::U16 => ExplicitValueType::U16,
        ScalarTypeV1::I32 => ExplicitValueType::I32,
        ScalarTypeV1::U32 => ExplicitValueType::U32,
        ScalarTypeV1::I64 => ExplicitValueType::I64,
        ScalarTypeV1::U64 => ExplicitValueType::U64,
        ScalarTypeV1::F16 => ExplicitValueType::F16,
        ScalarTypeV1::F32 => ExplicitValueType::F32,
        ScalarTypeV1::F64 => ExplicitValueType::F64,
    }
}

const fn explicit_value_type_tag(value: ExplicitValueType) -> u8 {
    match value {
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

fn derive_semantic_parameter_identity(
    argument: &fe2o3_kernel_descriptor::LogicalArgumentV1,
    artifact: &ArtifactAbiField,
    component_index: usize,
    component: PhysicalAbiComponentKind,
    offset: u32,
    size: u16,
    alignment: u16,
) -> SemanticTypeIdentityV2 {
    let mut digest = CanonicalDigestV2::new(SEMANTIC_PARAMETER_DOMAIN_V2);
    digest.u16(argument.source_index());
    digest.bytes(artifact.name().as_str().as_bytes());
    digest.u64(artifact.offset());
    digest.u64(artifact.size());
    digest.u32(artifact.alignment());
    digest_artifact_kind(&mut digest, artifact.kind());
    digest.u8(artifact_mutability_tag(artifact.mutability()));
    digest.u8(artifact_ownership_tag(artifact.ownership()));
    digest.u8(artifact_access_tag(artifact.access()));
    digest.u8(artifact_address_space_tag(artifact.address_space()));
    digest.u8(artifact_alias_tag(artifact.alias_class()));
    digest.bytes(artifact.type_identity().rust_type().bytes().as_bytes());
    digest.bytes(artifact.type_identity().layout().bytes().as_bytes());
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

fn digest_artifact_kind(digest: &mut CanonicalDigestV2, value: ArtifactAbiKind) {
    match value {
        ArtifactAbiKind::Scalar(scalar) => {
            digest.u8(0);
            digest.u8(component_tag(PhysicalAbiComponentKind::ScalarByValue(
                descriptor_scalar(scalar),
            )));
        }
        ArtifactAbiKind::Pointer {
            pointee_size,
            pointee_alignment,
        } => {
            digest.u8(1);
            digest.u64(pointee_size);
            digest.u32(pointee_alignment);
        }
        ArtifactAbiKind::Slice {
            element_size,
            element_alignment,
        } => {
            digest.u8(2);
            digest.u64(element_size);
            digest.u32(element_alignment);
        }
    }
}

const fn artifact_mutability_tag(value: fe2o3_artifacts::Mutability) -> u8 {
    match value {
        fe2o3_artifacts::Mutability::Immutable => 0,
        fe2o3_artifacts::Mutability::Mutable => 1,
    }
}

const fn artifact_ownership_tag(value: ArtifactOwnership) -> u8 {
    match value {
        ArtifactOwnership::ByValue => 0,
        ArtifactOwnership::SharedBorrow => 1,
        ArtifactOwnership::UniqueBorrow => 2,
        ArtifactOwnership::RawPointer => 3,
    }
}

const fn artifact_access_tag(value: ArtifactAccess) -> u8 {
    match value {
        ArtifactAccess::ByValue => 0,
        ArtifactAccess::ReadOnly => 1,
        ArtifactAccess::WriteOnly => 2,
        ArtifactAccess::ReadWrite => 3,
    }
}

const fn artifact_address_space_tag(value: ArtifactAddressSpace) -> u8 {
    match value {
        ArtifactAddressSpace::Value => 0,
        ArtifactAddressSpace::Global => 1,
        ArtifactAddressSpace::Constant => 2,
        ArtifactAddressSpace::Workgroup => 3,
        ArtifactAddressSpace::Private => 4,
        ArtifactAddressSpace::Generic => 5,
    }
}

const fn artifact_alias_tag(value: ArtifactAliasClass) -> u8 {
    match value {
        ArtifactAliasClass::Value => 0,
        ArtifactAliasClass::SharedReadOnly => 1,
        ArtifactAliasClass::Exclusive => 2,
        ArtifactAliasClass::SharedAtomic => 3,
        ArtifactAliasClass::Unrestricted => 4,
    }
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
    launch: &ArtifactLaunchContract,
    physical: &PublishedKernelPhysicalLayoutV1,
) -> Result<Gfx942PhysicalLaunchProjectionV2, LaunchKernelMetadataBridgeErrorV2> {
    let physical_launch = physical.launch();
    let physical_block = match physical_launch.required_workgroup_size() {
        PhysicalMetadataValueV1::Known(value) => value,
        PhysicalMetadataValueV1::Unknown => {
            return Err(LaunchKernelMetadataBridgeErrorV2::MissingPhysicalMetadata(
                "required workgroup size",
            ));
        }
    };
    let block = match launch.block_size() {
        ArtifactBlockSize::Exact(dimensions) => dimensions,
        ArtifactBlockSize::Any | ArtifactBlockSize::AtMost(_) => {
            return Err(
                LaunchKernelMetadataBridgeErrorV2::UnsupportedPhysicalLaunchContract(
                    "non-exact block policy",
                ),
            );
        }
    };
    if physical_block != [block.x(), block.y(), block.z()] {
        return Err(
            LaunchKernelMetadataBridgeErrorV2::RecoveredMetadataInconsistent(
                "required workgroup size",
            ),
        );
    }
    if physical_launch.wavefront_size() != 64 {
        return Err(LaunchKernelMetadataBridgeErrorV2::UnsupportedTarget);
    }
    let block = DimensionsV2::new(block.x(), block.y(), block.z());
    let max_grid = launch.max_grid();
    let max_grid_blocks = DimensionsV2::new(max_grid.x(), max_grid.y(), max_grid.z());
    let mut physical_maximum_workgroups = [0_u32; 3];
    for (axis, (declared, observed, field)) in [
        (
            max_grid_blocks.x,
            physical_launch.max_workgroups()[0],
            "maximum workgroups X",
        ),
        (
            max_grid_blocks.y,
            physical_launch.max_workgroups()[1],
            "maximum workgroups Y",
        ),
        (
            max_grid_blocks.z,
            physical_launch.max_workgroups()[2],
            "maximum workgroups Z",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let maximum = match observed {
            PhysicalMetadataValueV1::Known(value) => value,
            PhysicalMetadataValueV1::Unknown => {
                return Err(LaunchKernelMetadataBridgeErrorV2::MissingPhysicalMetadata(
                    field,
                ));
            }
        };
        if declared > maximum {
            return Err(
                LaunchKernelMetadataBridgeErrorV2::PhysicalLaunchLimitExceeded {
                    axis,
                    declared,
                    maximum,
                },
            );
        }
        physical_maximum_workgroups[axis] = maximum;
    }
    let flat = checked_dimensions_product(block, "flat workgroup size")?;
    let flat = u32::try_from(flat)
        .map_err(|_| LaunchKernelMetadataBridgeErrorV2::NumericOverflow("flat workgroup size"))?;
    if flat != physical_launch.max_flat_workgroup_size() {
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
    Ok(Gfx942PhysicalLaunchProjectionV2 {
        declared_rank: launch.rank(),
        required_block_threads: block,
        declared_maximum_grid_blocks: max_grid_blocks,
        physical_maximum_workgroups: DimensionsV2::new(
            physical_maximum_workgroups[0],
            physical_maximum_workgroups[1],
            physical_maximum_workgroups[2],
        ),
        required_flat_workgroup_size: flat,
        physical_maximum_flat_workgroup_size: physical_launch.max_flat_workgroup_size(),
        declared_maximum_total_workitems: max_total_workitems,
        wavefront_width: physical_launch.wavefront_size(),
    })
}

fn derive_resources(
    launch: &ArtifactLaunchContract,
    physical: &PublishedKernelPhysicalLayoutV1,
) -> Result<Gfx942PhysicalResourceProjectionV2, LaunchKernelMetadataBridgeErrorV2> {
    if launch.max_dynamic_shared_memory_bytes() != 0 {
        return Err(LaunchKernelMetadataBridgeErrorV2::UnsupportedDynamicLds);
    }
    let physical = physical.launch();
    if matches!(
        physical.dynamic_shared_memory_indicator(),
        PhysicalMetadataValueV1::Known(true)
    ) {
        return Err(LaunchKernelMetadataBridgeErrorV2::UnsupportedDynamicLds);
    }
    let static_lds_bytes = u32::try_from(physical.group_segment_fixed_size()).map_err(|_| {
        LaunchKernelMetadataBridgeErrorV2::NumericOverflow("static LDS segment size")
    })?;
    if static_lds_bytes != launch.static_shared_memory_bytes() {
        return Err(
            LaunchKernelMetadataBridgeErrorV2::RecoveredMetadataInconsistent(
                "static LDS segment size",
            ),
        );
    }
    let private_segment_bytes = u32::try_from(physical.private_segment_fixed_size())
        .map_err(|_| LaunchKernelMetadataBridgeErrorV2::NumericOverflow("private segment size"))?;
    Ok(Gfx942PhysicalResourceProjectionV2 {
        static_lds_bytes,
        private_segment_bytes,
        dynamic_lds: Gfx942DynamicLdsProjectionV2::ArtifactForbidsAndPhysicalAbiOmits,
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
    if variant.resources.static_lds_bytes != derived.resources.static_lds_bytes
        || variant.resources.maximum_dynamic_lds_bytes != 0
        || variant.resources.dynamic_lds_alignment != 1
        || variant.resources.private_segment_bytes != derived.resources.private_segment_bytes
    {
        return Err(LaunchKernelMetadataBridgeErrorV2::ResourceSubstitution);
    }
    Ok(())
}

fn validate_launch_geometry_match(
    model: Gfx942LaunchContractV2,
    derived: Gfx942PhysicalLaunchProjectionV2,
) -> Result<(), LaunchKernelMetadataBridgeErrorV2> {
    let BlockShapePolicyV2::Exact(model_block) = model.block else {
        return Err(
            LaunchKernelMetadataBridgeErrorV2::UnsupportedPhysicalLaunchContract(
                "non-exact block policy",
            ),
        );
    };
    if model.rank != derived.declared_rank
        || model_block != derived.required_block_threads
        || model.max_grid_blocks != derived.declared_maximum_grid_blocks
        || model.minimum_flat_workgroup_size != derived.required_flat_workgroup_size
        || model.maximum_flat_workgroup_size != derived.required_flat_workgroup_size
        || model.wavefront != WavefrontWidthV2::Wave64
        || model.require_full_waves
        || model.max_total_workitems != derived.declared_maximum_total_workitems
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

/// Failure to bind a launch-model projection to current recovered executable metadata.
#[derive(Debug)]
#[non_exhaustive]
pub enum LaunchKernelMetadataBridgeErrorV2 {
    CurrentPublication(FinalizedWorkerV2BundleAdmissionError),
    InvalidLaunchModel(LaunchKernelValidationErrorV2),
    UnknownModelProjection,
    AmbiguousModelProjection,
    UnsupportedTarget,
    UnsupportedCodeObjectVersion,
    UnsupportedDigestAlgorithm,
    MissingPhysicalMetadata(&'static str),
    UnsupportedPhysicalAbi(&'static str),
    UnsupportedPhysicalLaunchContract(&'static str),
    UnsupportedDynamicLds,
    NumericOverflow(&'static str),
    RecoveredMetadataInconsistent(&'static str),
    PhysicalLaunchLimitExceeded {
        axis: usize,
        declared: u32,
        maximum: u32,
    },
    TargetSubstitution,
    LogicalNameSubstitution,
    EntryNameSubstitution,
    ArtifactSubstitution,
    KernelSubstitution,
    SignatureSubstitution,
    LaunchGeometrySubstitution,
    ResourceSubstitution,
}

impl fmt::Display for LaunchKernelMetadataBridgeErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CurrentPublication(error) => error.fmt(formatter),
            Self::InvalidLaunchModel(error) => write!(formatter, "invalid launch model: {error:?}"),
            Self::UnknownModelProjection => {
                formatter.write_str("launch model projection is absent from the family")
            }
            Self::AmbiguousModelProjection => {
                formatter.write_str("launch model projection name is ambiguous")
            }
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
                "launch metadata bridge requires zero declared and physically observed dynamic LDS",
            ),
            Self::NumericOverflow(field) => write!(formatter, "{field} exceeds launch V2 bounds"),
            Self::RecoveredMetadataInconsistent(field) => {
                write!(
                    formatter,
                    "recovered executable metadata disagrees on {field}"
                )
            }
            Self::PhysicalLaunchLimitExceeded {
                axis,
                declared,
                maximum,
            } => {
                let axis = ["X", "Y", "Z"].get(*axis).copied().unwrap_or("unknown");
                write!(
                    formatter,
                    "artifact maximum grid {axis}={declared} exceeds physical maximum workgroups {axis}={maximum}"
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
    let resources = fe2o3_kernel_ir::Gfx942ResourceLimitsV2 {
        static_lds_bytes: derived.resources.static_lds_bytes,
        maximum_dynamic_lds_bytes: 0,
        dynamic_lds_alignment: 1,
        private_segment_bytes: derived.resources.private_segment_bytes,
    };
    let occupancy_subject = fe2o3_kernel_ir::canonical_occupancy_subject_identity_v2(
        &derived.target,
        &derived.signature,
        derived.artifact_identity,
        &derived.entry_name,
        resources,
    );
    let variant = KernelVariantV2 {
        kernel_identity: derived.kernel_identity,
        policy_identity: KernelPolicyIdentityV2::from_bytes([0x72; 32]),
        artifact_identity: derived.artifact_identity,
        tuple_identity: KernelVariantTupleIdentityV2::from_bytes([0; 32]),
        variant_name: "recovered-exact-wave64".to_owned(),
        entry_name: derived.entry_name.into(),
        launch: Gfx942LaunchContractV2 {
            rank: derived.launch.declared_rank,
            block: BlockShapePolicyV2::Exact(derived.launch.required_block_threads),
            max_grid_blocks: derived.launch.declared_maximum_grid_blocks,
            minimum_flat_workgroup_size: derived.launch.required_flat_workgroup_size,
            maximum_flat_workgroup_size: derived.launch.required_flat_workgroup_size,
            wavefront: WavefrontWidthV2::Wave64,
            require_full_waves: false,
            minimum_waves_per_execution_unit: 1,
            maximum_waves_per_execution_unit: 8,
            max_total_workitems: derived.launch.declared_maximum_total_workitems,
            unsupported: UnsupportedLaunchFeaturesV2::NONE,
        },
        resources,
        occupancy_witness: Some(Gfx942OccupancyWitnessV2 {
            verifier_identity: OccupancyVerifierIdentityV2::from_bytes([0x73; 32]),
            metadata_identity: OccupancyMetadataIdentityV2::from_bytes([0x74; 32]),
            subject_identity: occupancy_subject,
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
        LaunchProofKindV2, LaunchProofObligationV2, canonical_occupancy_subject_identity_v2,
        canonical_variant_tuple_identity_v2,
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

#[cfg(test)]
mod semantic_join_tests {
    use super::*;
    use fe2o3_artifacts::{
        AbiField, Access, AddressSpace, AliasClass, ArgumentOwnership, DeclaredRustLayoutIdentity,
        DeclaredRustTypeIdentity, DigestBytes, Mutability, Name, TypeIdentity,
    };
    use fe2o3_kernel_descriptor::{
        DeviceLayoutRecordV1, LogicalArgumentV1, SourceTypeRecordV1, ValidName,
    };

    #[test]
    fn standalone_and_nested_reference_profile_fails_closed() {
        let source =
            SourceTypeRecordV1::new(SourceTypeDescriptorV1::shared_slice(ScalarTypeV1::F32));
        let layout =
            DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::shared_slice(ScalarTypeV1::F32));
        let argument = LogicalArgumentV1::shared_slice(
            0,
            ValidName::new("values").unwrap(),
            &source,
            &layout,
            0,
        )
        .unwrap();
        let field = AbiField::new(
            Name::new("values").unwrap(),
            0,
            8,
            8,
            ArtifactAbiKind::Pointer {
                pointee_size: 4,
                pointee_alignment: 4,
            },
            Mutability::Immutable,
            Access::ReadOnly,
            AddressSpace::Global,
            TypeIdentity::new(
                DeclaredRustTypeIdentity::from_untrusted_bytes(DigestBytes::from_bytes(
                    *source.identity().as_bytes(),
                )),
                DeclaredRustLayoutIdentity::from_untrusted_bytes(DigestBytes::from_bytes(
                    *layout.identity().as_bytes(),
                )),
            ),
            ArgumentOwnership::SharedBorrow,
            AliasClass::SharedReadOnly,
        )
        .unwrap();

        assert!(matches!(
            validate_artifact_argument_semantics(&field, &argument),
            Err(LaunchKernelMetadataBridgeErrorV2::UnsupportedPhysicalAbi(
                "standalone pointers without a descriptor semantic kind"
            ))
        ));
    }
}
