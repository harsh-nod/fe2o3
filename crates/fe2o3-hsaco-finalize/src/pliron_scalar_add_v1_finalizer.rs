//! Inert inspection primitives for the exact Pliron scalar-add HSACO profile.
//!
//! This module deliberately owns no source lineage, worker policy, execution
//! admission, publication, load, or launch authority. The higher-level Pliron
//! bridge owns those joins because it can consume the opaque prepared lineage.

use std::{error::Error, fmt};

use fe2o3_hsaco::{
    ArgumentAddressSpace, CodeObjectVersion as InspectedCodeObjectVersion, ExplicitArgument,
    ExplicitValueKind, HiddenArgument, HiddenValueKind, KernelBindingError, KernelKind,
    inspect_and_bind_kernel_descriptors,
};
use fe2o3_kernel_descriptor::DeviceTargetV1;
use sha2::{Digest, Sha256};

use crate::ContentIdentityV1;

/// Exact kernel entry admitted by the dedicated scalar profile.
pub const PLIRON_SCALAR_ADD_V1_KERNEL: &str = "scalar_add";
/// Exact AMDHSA descriptor symbol admitted by the dedicated scalar profile.
pub const PLIRON_SCALAR_ADD_V1_DESCRIPTOR: &str = "scalar_add.kd";
/// Exact target admitted by the dedicated scalar profile.
pub const PLIRON_SCALAR_ADD_V1_TARGET: &str = "gfx942:xnack-";
/// Pinned upstream LLVM identity used by the exact worker profile.
pub const PLIRON_SCALAR_ADD_V1_LLVM_BUILD_IDENTITY: &str =
    "upstream-llvmorg-22.1.8-ca7933e47d3a3451d81e72ac174dcb5aa28b59d1";
/// Caller-populated prefix of the COV6 kernarg segment.
pub const PLIRON_SCALAR_ADD_V1_EXPLICIT_KERNARG_BYTES: u64 = 24;
/// Runtime-populated suffix of the COV6 kernarg segment.
pub const PLIRON_SCALAR_ADD_V1_IMPLICIT_KERNARG_BYTES: u64 = 256;
/// Complete pinned LLVM 22 COV6 kernarg segment size.
pub const PLIRON_SCALAR_ADD_V1_KERNARG_BYTES: u64 = 280;
/// Required kernarg segment alignment.
pub const PLIRON_SCALAR_ADD_V1_KERNARG_ALIGNMENT: u64 = 8;

const DESCRIPTOR_IDENTITY_DOMAIN: &[u8] = b"FE2O3/PLIRON-SCALAR-ADD-V1/AMDHSA-DESCRIPTOR/V1\0";
const MACHINE_IDENTITY_DOMAIN: &[u8] = b"FE2O3/PLIRON-SCALAR-ADD-V1/MACHINE-BYTES/V1\0";

/// One independently inspected HSACO profile field.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum PlironScalarAddV1HsacoField {
    /// Target processor and feature string.
    Target,
    /// Code-object version.
    CodeObjectVersion,
    /// AMDGPU metadata version.
    MetadataVersion,
    /// Unexpected printf metadata.
    PrintfMetadata,
    /// Kernel entry/descriptor closure.
    KernelClosure,
    /// Unexpected required-workgroup declaration.
    RequiredWorkgroup,
    /// Maximum flat workgroup size.
    MaxFlatWorkgroup,
    /// Wavefront size.
    Wavefront,
    /// Complete 280-byte kernarg contract.
    KernargSegment,
    /// Three explicit arguments.
    ExplicitArguments,
    /// Canonical COV6 hidden arguments.
    HiddenArguments,
    /// Fixed group segment size.
    GroupSegment,
    /// Fixed private segment size.
    PrivateSegment,
    /// SGPR or VGPR spill counts.
    SpillCounts,
    /// Dynamic-stack declaration or use.
    DynamicStack,
    /// Closed kernel metadata profile.
    KernelMetadataClosure,
    /// Structured descriptor resources.
    DescriptorResources,
    /// Exact descriptor byte span.
    DescriptorBytes,
    /// Exact nonempty machine-code span.
    MachineBytes,
}

/// One independently inspected ELF closure field.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum PlironScalarAddV1ElfField {
    /// ELF structure or bounds.
    Object,
    /// Unexpected generic fe2o3 descriptor section.
    CanonicalDescriptorSection,
    /// Final defined-symbol closure.
    DefinedSymbols,
    /// Undefined symbols or dynamic dependencies.
    UndefinedSymbols,
    /// Static or dynamic relocation closure.
    Relocations,
    /// Dynamic-loader sections, declarations, hashes, or mappings.
    DynamicLoader,
    /// Exact executable section, segment, entry address, or entry size.
    ExecutableRange,
}

/// Bounded failure from inert scalar-add HSACO inspection.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PlironScalarAddV1InspectionError {
    /// The bounded HSACO parser or descriptor binder rejected the object.
    HsacoBinding(KernelBindingError),
    /// One exact metadata, ABI, descriptor, or machine-span field changed.
    HsacoProfile(PlironScalarAddV1HsacoField),
    /// One exact ELF closure field changed.
    ElfProfile(PlironScalarAddV1ElfField),
}

impl fmt::Display for PlironScalarAddV1InspectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HsacoBinding(error) => write!(formatter, "scalar HSACO binding failed: {error}"),
            Self::HsacoProfile(field) => write!(formatter, "scalar HSACO substituted {field:?}"),
            Self::ElfProfile(field) => write!(formatter, "scalar ELF substituted {field:?}"),
        }
    }
}

impl Error for PlironScalarAddV1InspectionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::HsacoBinding(error) => Some(error),
            Self::HsacoProfile(_) | Self::ElfProfile(_) => None,
        }
    }
}

/// Stable identity of the exact 64-byte AMDHSA descriptor observation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PlironScalarAddV1AmdhsaDescriptorIdentity([u8; 32]);

impl PlironScalarAddV1AmdhsaDescriptorIdentity {
    /// Restores an independently recorded descriptor identity.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Returns the domain-separated descriptor digest.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Stable identity of the exact bound scalar kernel machine bytes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PlironScalarAddV1MachineIdentity([u8; 32]);

impl PlironScalarAddV1MachineIdentity {
    /// Restores an independently recorded machine-code identity.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Returns the domain-separated machine-code digest.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Inert identities observed from one structurally valid scalar-add HSACO.
///
/// Observation is not admission. A higher-level owner must compare all three
/// identities with an independently provisioned artifact policy before it can
/// claim that the artifact matches an approved profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InspectedPlironScalarAddV1Hsaco {
    output: ContentIdentityV1,
    descriptor: PlironScalarAddV1AmdhsaDescriptorIdentity,
    machine: PlironScalarAddV1MachineIdentity,
}

impl InspectedPlironScalarAddV1Hsaco {
    /// Returns the identity of the complete observed HSACO.
    pub const fn output_identity(&self) -> ContentIdentityV1 {
        self.output
    }

    /// Returns the identity of the exact 64-byte AMDHSA descriptor.
    pub const fn descriptor_identity(&self) -> PlironScalarAddV1AmdhsaDescriptorIdentity {
        self.descriptor
    }

    /// Returns the identity of the exact bound machine-code span.
    pub const fn machine_identity(&self) -> PlironScalarAddV1MachineIdentity {
        self.machine
    }

    /// This observation grants no publication authority.
    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    /// This observation grants no load authority.
    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    /// This observation grants no launch authority.
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Structurally inspects one exact-profile scalar-add HSACO without admitting it.
///
/// The result deliberately records descriptor and machine identities instead
/// of treating newly observed hashes as approved values.
pub fn inspect_pliron_scalar_add_v1_hsaco(
    bytes: &[u8],
) -> Result<InspectedPlironScalarAddV1Hsaco, PlironScalarAddV1InspectionError> {
    validate_elf_closure(bytes)?;
    let inspected = inspect_and_bind_kernel_descriptors(bytes)
        .map_err(PlironScalarAddV1InspectionError::HsacoBinding)?;
    let metadata = inspected.inspection();
    let exact_target = DeviceTargetV1::parse(PLIRON_SCALAR_ADD_V1_TARGET)
        .expect("the fixed scalar target is canonical")
        .as_amd_target_id();
    if metadata.code_object_version() != InspectedCodeObjectVersion::V6
        || metadata.target() != exact_target
    {
        return Err(PlironScalarAddV1InspectionError::HsacoProfile(
            if metadata.code_object_version() != InspectedCodeObjectVersion::V6 {
                PlironScalarAddV1HsacoField::CodeObjectVersion
            } else {
                PlironScalarAddV1HsacoField::Target
            },
        ));
    }
    if metadata.metadata_version().major() != 1 || metadata.metadata_version().minor() != 2 {
        return Err(PlironScalarAddV1InspectionError::HsacoProfile(
            PlironScalarAddV1HsacoField::MetadataVersion,
        ));
    }
    if metadata.has_printf_metadata() {
        return Err(PlironScalarAddV1InspectionError::HsacoProfile(
            PlironScalarAddV1HsacoField::PrintfMetadata,
        ));
    }
    let [kernel] = metadata.kernels() else {
        return Err(PlironScalarAddV1InspectionError::HsacoProfile(
            PlironScalarAddV1HsacoField::KernelClosure,
        ));
    };
    let [binding] = inspected.bindings() else {
        return Err(PlironScalarAddV1InspectionError::HsacoProfile(
            PlironScalarAddV1HsacoField::KernelClosure,
        ));
    };
    validate_kernel(kernel)?;
    let descriptor = binding.descriptor();
    if descriptor.group_segment_fixed_size() != 0
        || descriptor.private_segment_fixed_size() != 0
        || descriptor.kernarg_size() != PLIRON_SCALAR_ADD_V1_KERNARG_BYTES as u32
        || descriptor.wavefront_size() != 64
        || descriptor.uses_dynamic_stack()
        || descriptor.private_segment_enabled()
        || descriptor.kernarg_preload() != 0
    {
        return Err(PlironScalarAddV1InspectionError::HsacoProfile(
            PlironScalarAddV1HsacoField::DescriptorResources,
        ));
    }
    let descriptor_bytes = bounded_slice(bytes, binding.descriptor_file_offset(), 64).ok_or(
        PlironScalarAddV1InspectionError::HsacoProfile(
            PlironScalarAddV1HsacoField::DescriptorBytes,
        ),
    )?;
    let machine_bytes = bounded_slice(bytes, binding.entry_file_offset(), binding.entry_size())
        .filter(|machine| !machine.is_empty())
        .ok_or(PlironScalarAddV1InspectionError::HsacoProfile(
            PlironScalarAddV1HsacoField::MachineBytes,
        ))?;
    Ok(InspectedPlironScalarAddV1Hsaco {
        output: ContentIdentityV1::calculate(bytes),
        descriptor: PlironScalarAddV1AmdhsaDescriptorIdentity(domain_hash(
            DESCRIPTOR_IDENTITY_DOMAIN,
            descriptor_bytes,
        )),
        machine: PlironScalarAddV1MachineIdentity(domain_hash(
            MACHINE_IDENTITY_DOMAIN,
            machine_bytes,
        )),
    })
}

fn validate_kernel(
    kernel: &fe2o3_hsaco::InspectedKernel,
) -> Result<(), PlironScalarAddV1InspectionError> {
    use PlironScalarAddV1HsacoField as Field;

    if kernel.name() != PLIRON_SCALAR_ADD_V1_KERNEL
        || kernel.symbol() != PLIRON_SCALAR_ADD_V1_DESCRIPTOR
    {
        return Err(PlironScalarAddV1InspectionError::HsacoProfile(
            Field::KernelClosure,
        ));
    }
    if kernel.required_workgroup_size().is_some() {
        return Err(PlironScalarAddV1InspectionError::HsacoProfile(
            Field::RequiredWorkgroup,
        ));
    }
    if kernel.max_flat_workgroup_size() != 64 {
        return Err(PlironScalarAddV1InspectionError::HsacoProfile(
            Field::MaxFlatWorkgroup,
        ));
    }
    if kernel.wavefront_size() != 64 {
        return Err(PlironScalarAddV1InspectionError::HsacoProfile(
            Field::Wavefront,
        ));
    }
    let reviewed_total =
        crate::general_v3_cov6_total_kernarg_size_v1(PLIRON_SCALAR_ADD_V1_EXPLICIT_KERNARG_BYTES);
    if reviewed_total != Some(PLIRON_SCALAR_ADD_V1_KERNARG_BYTES)
        || kernel.kernarg_segment_size() != PLIRON_SCALAR_ADD_V1_KERNARG_BYTES
        || kernel.kernarg_segment_alignment() != PLIRON_SCALAR_ADD_V1_KERNARG_ALIGNMENT
        || kernel.implicit_argument_offset() != Some(PLIRON_SCALAR_ADD_V1_EXPLICIT_KERNARG_BYTES)
        || !crate::general_v3_cov6_implicit_span_is_canonical_v1(
            kernel.implicit_argument_size(),
            true,
        )
    {
        return Err(PlironScalarAddV1InspectionError::HsacoProfile(
            Field::KernargSegment,
        ));
    }
    validate_explicit_arguments(kernel.explicit_arguments())?;
    validate_hidden_arguments(kernel.hidden_arguments())?;
    if kernel.group_segment_fixed_size() != 0 {
        return Err(PlironScalarAddV1InspectionError::HsacoProfile(
            Field::GroupSegment,
        ));
    }
    if kernel.private_segment_fixed_size() != 0 {
        return Err(PlironScalarAddV1InspectionError::HsacoProfile(
            Field::PrivateSegment,
        ));
    }
    if kernel.sgpr_spill_count() != Some(0) || kernel.vgpr_spill_count() != Some(0) {
        return Err(PlironScalarAddV1InspectionError::HsacoProfile(
            Field::SpillCounts,
        ));
    }
    if kernel.uses_dynamic_stack() || kernel.uses_dynamic_stack_declaration() != Some(false) {
        return Err(PlironScalarAddV1InspectionError::HsacoProfile(
            Field::DynamicStack,
        ));
    }
    if kernel.kind() != KernelKind::Normal
        || kernel.kind_was_emitted()
        || kernel.uniform_work_group_size()
        || kernel.workgroup_processor_mode() == Some(true)
        || kernel.max_workgroups() != [None; 3]
        || kernel.cluster_dims().is_some()
        || kernel.device_enqueue_symbol().is_some()
        || kernel.source_language().is_some()
        || kernel.source_language_version().is_some()
        || kernel.workgroup_size_hint_was_emitted()
        || kernel.vector_type_hint_was_emitted()
        || !kernel.arguments_were_emitted()
        || kernel.sgpr_count() == 0
        || kernel.vgpr_count() == 0
    {
        return Err(PlironScalarAddV1InspectionError::HsacoProfile(
            Field::KernelMetadataClosure,
        ));
    }
    Ok(())
}

fn validate_explicit_arguments(
    arguments: &[ExplicitArgument],
) -> Result<(), PlironScalarAddV1InspectionError> {
    if arguments.len() != 3 {
        return Err(PlironScalarAddV1InspectionError::HsacoProfile(
            PlironScalarAddV1HsacoField::ExplicitArguments,
        ));
    }
    for (index, argument) in arguments[..2].iter().enumerate() {
        let expected_name = if index == 0 { "input" } else { "output" };
        if argument.name() != Some(expected_name)
            || argument.type_name().is_some()
            || argument.offset() != (index as u64) * 8
            || argument.size() != 8
            || argument.alignment().is_some()
            || argument.value_kind() != ExplicitValueKind::GlobalBuffer
            || argument.value_type().is_some()
            || argument.address_space() != Some(ArgumentAddressSpace::Global)
            || !empty_argument_qualifiers(argument)
        {
            return Err(PlironScalarAddV1InspectionError::HsacoProfile(
                PlironScalarAddV1HsacoField::ExplicitArguments,
            ));
        }
    }
    let addend = &arguments[2];
    if addend.name() != Some("addend")
        || addend.type_name().is_some()
        || addend.offset() != 16
        || addend.size() != 4
        || addend.alignment().is_some()
        || addend.value_kind() != ExplicitValueKind::ByValue
        || addend.value_type().is_some()
        || addend.address_space().is_some()
        || !empty_argument_qualifiers(addend)
    {
        return Err(PlironScalarAddV1InspectionError::HsacoProfile(
            PlironScalarAddV1HsacoField::ExplicitArguments,
        ));
    }
    Ok(())
}

fn empty_argument_qualifiers(argument: &ExplicitArgument) -> bool {
    argument.access().is_none()
        && argument.actual_access().is_none()
        && argument.pointee_alignment().is_none()
        && argument.is_const().is_none()
        && argument.is_restrict().is_none()
        && argument.is_volatile().is_none()
        && argument.is_pipe().is_none()
}

fn validate_hidden_arguments(
    arguments: &[HiddenArgument],
) -> Result<(), PlironScalarAddV1InspectionError> {
    const EXACT: [(u64, u64, HiddenValueKind); 13] = [
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
    ];
    if arguments.len() != EXACT.len() {
        return Err(PlironScalarAddV1InspectionError::HsacoProfile(
            PlironScalarAddV1HsacoField::HiddenArguments,
        ));
    }
    for (argument, (relative_offset, size, kind)) in arguments.iter().copied().zip(EXACT) {
        if argument.offset() != PLIRON_SCALAR_ADD_V1_EXPLICIT_KERNARG_BYTES + relative_offset
            || argument.size() != size
            || argument.value_kind() != kind
        {
            return Err(PlironScalarAddV1InspectionError::HsacoProfile(
                PlironScalarAddV1HsacoField::HiddenArguments,
            ));
        }
    }
    Ok(())
}

fn validate_elf_closure(bytes: &[u8]) -> Result<(), PlironScalarAddV1InspectionError> {
    crate::pliron_scalar_add_v1_elf::validate_scalar_add_v1_elf(bytes)
}

fn domain_hash(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    digest.finalize().into()
}

fn bounded_slice(bytes: &[u8], offset: u64, size: u64) -> Option<&[u8]> {
    let start = usize::try_from(offset).ok()?;
    let size = usize::try_from(size).ok()?;
    let end = start.checked_add(size)?;
    bytes.get(start..end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(dead_code)]
    mod fixture {
        use crate as fe2o3_hsaco_finalize;

        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/worker_v2_hsaco_test_support.rs"
        ));
    }

    #[test]
    fn reviewed_cov6_route_reconciles_the_complete_280_byte_segment() {
        assert_eq!(
            crate::general_v3_cov6_total_kernarg_size_v1(
                PLIRON_SCALAR_ADD_V1_EXPLICIT_KERNARG_BYTES
            ),
            Some(PLIRON_SCALAR_ADD_V1_KERNARG_BYTES)
        );
        assert!(crate::general_v3_cov6_implicit_span_is_canonical_v1(
            PLIRON_SCALAR_ADD_V1_IMPLICIT_KERNARG_BYTES,
            true
        ));
    }

    #[test]
    fn inspection_is_deterministic_and_rejects_hostile_substitutions() {
        use fixture::ScalarAddFixtureMutation as Mutation;

        let first = fixture::scalar_add_fixture();
        let replay = fixture::scalar_add_fixture();
        let first_observation = inspect_pliron_scalar_add_v1_hsaco(&first.bytes).unwrap();
        let replay_observation = inspect_pliron_scalar_add_v1_hsaco(&replay.bytes).unwrap();
        assert_eq!(first.bytes, replay.bytes);
        assert_eq!(first_observation, replay_observation);
        assert_ne!(first_observation.descriptor_identity().as_bytes(), &[0; 32]);
        assert_ne!(first_observation.machine_identity().as_bytes(), &[0; 32]);

        for mutation in [
            Mutation::Target,
            Mutation::CodeObjectVersion,
            Mutation::EntrySymbol,
            Mutation::DescriptorSymbol,
            Mutation::RequiredWorkgroup,
            Mutation::MaxFlatWorkgroup,
            Mutation::Wave32,
            Mutation::KernargSize,
            Mutation::KernargAlignment,
            Mutation::ExplicitArgumentOffset,
            Mutation::HiddenArgument,
            Mutation::GroupSegment,
            Mutation::PrivateSegment,
            Mutation::SpillCount,
            Mutation::DynamicStack,
            Mutation::CanonicalDescriptorSection,
            Mutation::ExtraDefinedSymbol,
            Mutation::ExtraLocalSymbol,
            Mutation::UndefinedStaticSymbol,
            Mutation::ExtraDynamicSymbol,
            Mutation::UndefinedDynamicSymbol,
            Mutation::RelSection,
            Mutation::RelaSection,
            Mutation::DynamicNeeded,
            Mutation::DynamicForbiddenTag,
            Mutation::DynamicDuplicateTag,
            Mutation::DynamicMissingNull,
            Mutation::DynamicMissingRequiredTags,
        ] {
            let hostile = fixture::scalar_add_fixture_with(mutation);
            assert!(
                inspect_pliron_scalar_add_v1_hsaco(&hostile.bytes).is_err(),
                "hostile scalar HSACO substitution was admitted"
            );
        }
    }
}
