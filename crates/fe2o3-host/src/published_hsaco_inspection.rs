use crate::published_direct_link::{
    PublishedPayloadKernelV1, ValidatedPublishedDirectLinkSelectionV1,
};
use fe2o3_amd_target::{AmdTargetId, ParseAmdTargetIdError};
use fe2o3_artifacts::{
    AbiKind, Access, AddressSpace, BlockSize, DirectLinkContainerIdentityV1,
    DirectLinkFinalizedPayloadIdentityV1, PayloadDigest,
};
use fe2o3_hsaco::{
    ArgumentAccess, ArgumentAddressSpace, CodeObjectVersion, ExplicitArgument, ExplicitValueKind,
    InspectedKernel, InspectedKernelBindings, KernelBindingError, KernelDescriptorBinding,
    KernelKind, inspect_and_bind_kernel_descriptors,
};
use std::fmt;
use std::sync::Arc;

use crate::KernelId;

/// Inert HSACO inspection bound to one exact published direct-link selection.
///
/// Construction first revalidates the admitted payload occurrence and exact bytes, then invokes
/// [`inspect_and_bind_kernel_descriptors`]. It also requires the inspected target, complete
/// payload-local kernel set, selected symbol, physical ABI metadata, and descriptor bindings to
/// agree with the admitted manifest snapshot.
///
/// This value is descriptive evidence only. It does not authenticate a filesystem object,
/// compiler, producer, or provenance chain and grants no module-loading or launch authority.
pub struct InspectedPublishedDirectLinkHsacoV1 {
    inspected: InspectedKernelBindings,
    payload: Arc<[u8]>,
    payload_digest: PayloadDigest,
    binding_index: usize,
    container_identity: DirectLinkContainerIdentityV1,
    finalized_payload_identity: DirectLinkFinalizedPayloadIdentityV1,
    selected_kernel_id: KernelId,
    selected_kernel_index: usize,
    payload_kernel_set: Box<[PublishedPayloadKernelV1]>,
}

impl fmt::Debug for InspectedPublishedDirectLinkHsacoV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InspectedPublishedDirectLinkHsacoV1")
            .field("payload_len", &self.payload.len())
            .field("payload_digest", &self.payload_digest)
            .field("binding_index", &self.binding_index)
            .field("container_identity", &self.container_identity)
            .field(
                "finalized_payload_identity",
                &self.finalized_payload_identity,
            )
            .field("selected_kernel_id", &self.selected_kernel_id)
            .field("selected_kernel_index", &self.selected_kernel_index)
            .field("payload_kernel_count", &self.payload_kernel_set.len())
            .finish_non_exhaustive()
    }
}

impl InspectedPublishedDirectLinkHsacoV1 {
    /// Inspects the exact bytes represented by an inert published selection.
    pub fn inspect(
        admitted: &ValidatedPublishedDirectLinkSelectionV1,
        exact_selected_payload_bytes: &[u8],
    ) -> Result<Self, PublishedHsacoInspectionError> {
        validate_payload_occurrence(admitted, exact_selected_payload_bytes)?;

        let inspected = inspect_and_bind_kernel_descriptors(exact_selected_payload_bytes)
            .map_err(PublishedHsacoInspectionError::Inspection)?;
        let selected_kernel_index = validate_inspection(admitted, &inspected)?;
        let selection = admitted.artifact_selection();

        Ok(Self {
            inspected,
            payload: Arc::from(exact_selected_payload_bytes),
            payload_digest: selection.identity().payload_digest(),
            binding_index: admitted.binding_index(),
            container_identity: admitted.container_identity(),
            finalized_payload_identity: admitted.finalized_payload_identity(),
            selected_kernel_id: selection.identity().kernel_id(),
            selected_kernel_index,
            payload_kernel_set: admitted.payload_kernel_set().into(),
        })
    }

    /// Revalidates the inert admission and exact payload bytes represented by this result.
    pub fn revalidate(
        &self,
        admitted: &ValidatedPublishedDirectLinkSelectionV1,
        exact_selected_payload_bytes: &[u8],
    ) -> Result<(), PublishedHsacoInspectionError> {
        if admitted.binding_index() != self.binding_index
            || admitted.container_identity() != self.container_identity
            || admitted.finalized_payload_identity() != self.finalized_payload_identity
            || admitted.artifact_selection().identity().kernel_id() != self.selected_kernel_id
            || admitted.payload_kernel_set() != self.payload_kernel_set.as_ref()
        {
            return Err(PublishedHsacoInspectionError::AdmissionSubstitution);
        }
        validate_payload_occurrence(admitted, exact_selected_payload_bytes)?;
        if exact_selected_payload_bytes != self.payload.as_ref() {
            return Err(PublishedHsacoInspectionError::PayloadSubstitution);
        }
        Ok(())
    }

    /// Returns the explicitly parsed AMDGPU HSA code-object version (V4, V5, or V6).
    pub fn code_object_version(&self) -> CodeObjectVersion {
        self.inspected.inspection().code_object_version()
    }

    /// Returns the exact target ID inspected from the HSACO metadata and ELF flags.
    pub fn target(&self) -> AmdTargetId {
        self.inspected.inspection().target()
    }

    /// Returns the complete number of manifest and inspected kernels bound to this payload.
    pub fn kernel_count(&self) -> usize {
        self.payload_kernel_set.len()
    }

    /// Returns descriptive metadata for the selected manifest kernel.
    pub fn selected_kernel(&self) -> &InspectedKernel {
        &self.inspected.inspection().kernels()[self.selected_kernel_index]
    }

    /// Returns the selected kernel's descriptive ELF descriptor binding.
    pub fn selected_descriptor_binding(&self) -> KernelDescriptorBinding {
        self.inspected.bindings()[self.selected_kernel_index]
    }

    /// Inspection does not authenticate a current filesystem publication.
    pub const fn authenticates_filesystem_artifact(&self) -> bool {
        false
    }

    /// Inspection does not authenticate compiler or producer provenance.
    pub const fn proves_compiler_provenance(&self) -> bool {
        false
    }

    /// Inspection never grants module-loading authority.
    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    /// Inspection never grants kernel-launch authority.
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

fn validate_payload_occurrence(
    admitted: &ValidatedPublishedDirectLinkSelectionV1,
    exact_selected_payload_bytes: &[u8],
) -> Result<(), PublishedHsacoInspectionError> {
    let selection = admitted.artifact_selection();
    let selected_digest = selection.identity().payload_digest();
    if admitted.finalized_payload_identity().digest() != selected_digest {
        return Err(PublishedHsacoInspectionError::PayloadOccurrenceMismatch);
    }
    if selection.identity().code_object().byte_len()
        != u64::try_from(exact_selected_payload_bytes.len()).unwrap_or(u64::MAX)
    {
        return Err(PublishedHsacoInspectionError::PayloadLengthMismatch);
    }
    if selected_digest
        .verify(exact_selected_payload_bytes)
        .is_err()
    {
        return Err(PublishedHsacoInspectionError::PayloadDigestMismatch);
    }
    if selection.payload() != exact_selected_payload_bytes {
        return Err(PublishedHsacoInspectionError::PayloadSubstitution);
    }
    Ok(())
}

fn validate_inspection(
    admitted: &ValidatedPublishedDirectLinkSelectionV1,
    inspected: &InspectedKernelBindings,
) -> Result<usize, PublishedHsacoInspectionError> {
    // The parser rejects every value outside V4 through V6 and validates the matching metadata
    // schema. Keep the supported versions explicit here because the manifest model cannot yet
    // carry an independently authenticated code-object-version declaration.
    match inspected.inspection().code_object_version() {
        CodeObjectVersion::V4 | CodeObjectVersion::V5 | CodeObjectVersion::V6 => {}
    }

    let declared_target = AmdTargetId::parse(
        admitted
            .artifact_selection()
            .identity()
            .target()
            .architecture()
            .as_str(),
    )
    .map_err(PublishedHsacoInspectionError::InvalidManifestTarget)?;
    if inspected.inspection().target() != declared_target {
        return Err(PublishedHsacoInspectionError::TargetMismatch);
    }

    let expected = admitted.payload_kernel_set();
    let actual = inspected.inspection().kernels();
    if expected.len() != actual.len() || inspected.bindings().len() != actual.len() {
        return Err(PublishedHsacoInspectionError::KernelSetMismatch);
    }

    let mut expected_names = expected
        .iter()
        .map(|kernel| (kernel.name().as_str(), kernel.symbol().as_str()))
        .collect::<Vec<_>>();
    let mut actual_names = actual
        .iter()
        .map(|kernel| (kernel.name(), kernel.symbol()))
        .collect::<Vec<_>>();
    expected_names.sort_unstable();
    actual_names.sort_unstable();
    if expected_names != actual_names {
        return Err(PublishedHsacoInspectionError::KernelSetMismatch);
    }

    for expected_kernel in expected {
        let inspected_kernel = actual
            .iter()
            .find(|kernel| {
                kernel.name() == expected_kernel.name().as_str()
                    && kernel.symbol() == expected_kernel.symbol().as_str()
            })
            .ok_or(PublishedHsacoInspectionError::KernelSetMismatch)?;
        validate_kernel_metadata(expected_kernel, inspected_kernel)?;
    }

    let selected_identity = admitted.artifact_selection().identity();
    actual
        .iter()
        .position(|kernel| {
            kernel.name() == selected_identity.name().as_str()
                && kernel.symbol() == selected_identity.symbol().as_str()
        })
        .ok_or(PublishedHsacoInspectionError::SelectedKernelMismatch)
}

fn validate_kernel_metadata(
    expected: &PublishedPayloadKernelV1,
    actual: &InspectedKernel,
) -> Result<(), PublishedHsacoInspectionError> {
    if actual.kind() != KernelKind::Normal {
        return metadata_mismatch(expected, "kernel kind");
    }
    if actual.uses_dynamic_stack() || actual.device_enqueue_symbol().is_some() {
        return metadata_mismatch(expected, "unsupported loader lifecycle");
    }
    if actual.group_segment_fixed_size()
        != u64::from(expected.launch().static_shared_memory_bytes())
    {
        return metadata_mismatch(expected, "static shared memory");
    }
    validate_launch_metadata(expected, actual)?;
    validate_physical_abi(expected, actual)
}

fn validate_launch_metadata(
    expected: &PublishedPayloadKernelV1,
    actual: &InspectedKernel,
) -> Result<(), PublishedHsacoInspectionError> {
    let block = expected.launch().block_size();
    let expected_dimensions = match block {
        BlockSize::Any => None,
        BlockSize::Exact(dimensions) | BlockSize::AtMost(dimensions) => Some(dimensions),
    };
    if let Some(dimensions) = expected_dimensions {
        let flat = u64::from(dimensions.x())
            .checked_mul(u64::from(dimensions.y()))
            .and_then(|value| value.checked_mul(u64::from(dimensions.z())))
            .ok_or_else(|| PublishedHsacoInspectionError::KernelMetadataMismatch {
                kernel: expected.name().as_str().to_owned(),
                field: "workgroup dimensions",
            })?;
        if flat > u64::from(actual.max_flat_workgroup_size()) {
            return metadata_mismatch(expected, "maximum flat workgroup size");
        }
    }
    if let Some(required) = actual.required_workgroup_size() {
        let exact = match block {
            BlockSize::Exact(dimensions) => [dimensions.x(), dimensions.y(), dimensions.z()],
            BlockSize::Any | BlockSize::AtMost(_) => {
                return metadata_mismatch(expected, "required workgroup size");
            }
        };
        if required != exact {
            return metadata_mismatch(expected, "required workgroup size");
        }
    }
    Ok(())
}

fn validate_physical_abi(
    expected: &PublishedPayloadKernelV1,
    actual: &InspectedKernel,
) -> Result<(), PublishedHsacoInspectionError> {
    let abi = expected.abi();
    let explicit_size = actual
        .implicit_argument_offset()
        .unwrap_or_else(|| actual.kernarg_segment_size());
    if abi.size() != explicit_size {
        return metadata_mismatch(expected, "explicit kernarg size");
    }
    if !abi.fields().is_empty() && u64::from(abi.alignment()) != actual.kernarg_segment_alignment()
    {
        return metadata_mismatch(expected, "kernarg alignment");
    }

    let mut physical_index = 0usize;
    for field in abi.fields() {
        match field.kind() {
            AbiKind::Scalar(_) => {
                validate_argument(
                    expected,
                    actual.explicit_arguments().get(physical_index),
                    field.offset(),
                    field.size(),
                    field.alignment(),
                    ExplicitValueKind::ByValue,
                    None,
                    field.access(),
                )?;
                physical_index += 1;
            }
            AbiKind::Pointer { .. } => {
                validate_argument(
                    expected,
                    actual.explicit_arguments().get(physical_index),
                    field.offset(),
                    field.size(),
                    field.alignment(),
                    ExplicitValueKind::GlobalBuffer,
                    Some(field.address_space()),
                    field.access(),
                )?;
                physical_index += 1;
            }
            AbiKind::Slice { .. } => {
                let pointer_bytes = abi.pointer_width().bytes();
                validate_argument(
                    expected,
                    actual.explicit_arguments().get(physical_index),
                    field.offset(),
                    pointer_bytes,
                    field.alignment(),
                    ExplicitValueKind::GlobalBuffer,
                    Some(field.address_space()),
                    field.access(),
                )?;
                validate_argument(
                    expected,
                    actual.explicit_arguments().get(physical_index + 1),
                    field.offset() + pointer_bytes,
                    pointer_bytes,
                    field.alignment(),
                    ExplicitValueKind::ByValue,
                    None,
                    Access::ByValue,
                )?;
                physical_index += 2;
            }
        }
    }
    if physical_index != actual.explicit_arguments().len() {
        return metadata_mismatch(expected, "physical argument count");
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_argument(
    kernel: &PublishedPayloadKernelV1,
    actual: Option<&ExplicitArgument>,
    offset: u64,
    size: u64,
    alignment: u32,
    value_kind: ExplicitValueKind,
    address_space: Option<AddressSpace>,
    access: Access,
) -> Result<(), PublishedHsacoInspectionError> {
    let Some(actual) = actual else {
        return metadata_mismatch(kernel, "physical argument count");
    };
    let expected_address_space = address_space.and_then(map_address_space);
    let expected_access = map_access(access);
    if actual.offset() != offset
        || actual.size() != size
        || actual
            .alignment()
            .is_some_and(|actual| actual != u64::from(alignment))
        || actual.value_kind() != value_kind
        || actual.address_space() != expected_address_space
        || actual
            .access()
            .is_some_and(|actual| Some(actual) != expected_access)
        || actual
            .actual_access()
            .is_some_and(|actual| Some(actual) != expected_access)
    {
        return metadata_mismatch(kernel, "physical argument layout");
    }
    Ok(())
}

fn map_address_space(value: AddressSpace) -> Option<ArgumentAddressSpace> {
    match value {
        AddressSpace::Global => Some(ArgumentAddressSpace::Global),
        AddressSpace::Constant => Some(ArgumentAddressSpace::Constant),
        AddressSpace::Workgroup => Some(ArgumentAddressSpace::Local),
        AddressSpace::Private => Some(ArgumentAddressSpace::Private),
        AddressSpace::Generic => Some(ArgumentAddressSpace::Generic),
        AddressSpace::Value => None,
    }
}

fn map_access(value: Access) -> Option<ArgumentAccess> {
    match value {
        Access::ReadOnly => Some(ArgumentAccess::ReadOnly),
        Access::WriteOnly => Some(ArgumentAccess::WriteOnly),
        Access::ReadWrite => Some(ArgumentAccess::ReadWrite),
        Access::ByValue => None,
    }
}

fn metadata_mismatch<T>(
    kernel: &PublishedPayloadKernelV1,
    field: &'static str,
) -> Result<T, PublishedHsacoInspectionError> {
    Err(PublishedHsacoInspectionError::KernelMetadataMismatch {
        kernel: kernel.name().as_str().to_owned(),
        field,
    })
}

/// Failure to bind inert HSACO inspection to an admitted published selection.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PublishedHsacoInspectionError {
    PayloadOccurrenceMismatch,
    PayloadLengthMismatch,
    PayloadDigestMismatch,
    PayloadSubstitution,
    Inspection(KernelBindingError),
    InvalidManifestTarget(ParseAmdTargetIdError),
    TargetMismatch,
    KernelSetMismatch,
    SelectedKernelMismatch,
    KernelMetadataMismatch { kernel: String, field: &'static str },
    AdmissionSubstitution,
}

impl fmt::Display for PublishedHsacoInspectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PayloadOccurrenceMismatch => {
                formatter.write_str("admitted finalized payload occurrence is inconsistent")
            }
            Self::PayloadLengthMismatch => {
                formatter.write_str("selected payload length differs from the manifest")
            }
            Self::PayloadDigestMismatch => {
                formatter.write_str("selected payload digest differs from the manifest")
            }
            Self::PayloadSubstitution => {
                formatter.write_str("payload bytes differ from the admitted selection")
            }
            Self::Inspection(error) => error.fmt(formatter),
            Self::InvalidManifestTarget(error) => {
                write!(formatter, "manifest AMD target is invalid: {error}")
            }
            Self::TargetMismatch => {
                formatter.write_str("inspected HSACO target differs from the manifest target")
            }
            Self::KernelSetMismatch => formatter.write_str(
                "inspected HSACO kernel names and symbols differ from the manifest payload set",
            ),
            Self::SelectedKernelMismatch => formatter
                .write_str("selected manifest kernel is absent from inspected HSACO metadata"),
            Self::KernelMetadataMismatch { kernel, field } => {
                write!(formatter, "kernel {kernel} metadata differs for {field}")
            }
            Self::AdmissionSubstitution => formatter
                .write_str("published direct-link admission differs from the inspected one"),
        }
    }
}

impl std::error::Error for PublishedHsacoInspectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Inspection(error) => Some(error),
            Self::InvalidManifestTarget(error) => Some(error),
            Self::PayloadOccurrenceMismatch
            | Self::PayloadLengthMismatch
            | Self::PayloadDigestMismatch
            | Self::PayloadSubstitution
            | Self::TargetMismatch
            | Self::KernelSetMismatch
            | Self::SelectedKernelMismatch
            | Self::KernelMetadataMismatch { .. }
            | Self::AdmissionSubstitution => None,
        }
    }
}
