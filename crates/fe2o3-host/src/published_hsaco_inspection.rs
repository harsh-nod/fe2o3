use crate::published_direct_link::{
    PublishedPayloadKernelV1, ValidatedPublishedDirectLinkSelectionV1,
};
use crate::{ObservedContext, PublishedDirectLinkAdmissionError};
use fe2o3_amd_target::{AmdTargetId, ParseAmdTargetIdError};
use fe2o3_artifacts::{
    AbiKind, AddressSpace, ArtifactContainerV1, BlockSize,
    ManifestClaimDirectLinkCurrentPublicationTokenV1, ManifestClaimDirectLinkPublicationBridgeV1,
    SelectedNativeKernel, ValidatedDirectLinkBundleEvidenceV1,
};
use fe2o3_hsaco::{
    ArgumentAccess, ArgumentAddressSpace, CodeObjectVersion, ExplicitArgument, ExplicitValueKind,
    Gfx1250Revision, HiddenValueKind, InspectedKernel, InspectedKernelBindings, KernelBindingError,
    KernelDescriptorBinding, KernelKind, inspect_and_bind_kernel_descriptors,
};
use std::fmt;

/// Version of the AMDHSA export-to-descriptor symbol rule used by this bridge.
///
/// Rule V1 maps a manifest loader/export symbol `S` to metadata `.name = S` and metadata
/// `.symbol = S + ".kd"` for code-object versions 4 through 6. Manifest logical names do not
/// participate in AMDHSA symbol matching.
pub const AMDHSA_KERNEL_IDENTITY_RULE_V1: u16 = 1;

/// Whether optional physical metadata was present in the inspected HSACO.
///
/// `Unknown` is preserved rather than interpreted as agreement with a manifest declaration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhysicalMetadataValueV1<T> {
    Unknown,
    Known(T),
}

impl<T> PhysicalMetadataValueV1<T> {
    fn from_option(value: Option<T>) -> Self {
        match value {
            Some(value) => Self::Known(value),
            None => Self::Unknown,
        }
    }
}

/// Descriptive physical metadata for one explicit AMDHSA argument.
///
/// Value kind and address/access qualifiers are producer metadata. They do not establish a Rust
/// scalar type, pointee or slice element type, ownership, aliasing, effects, or provenance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishedPhysicalArgumentLayoutV1 {
    offset: u64,
    size: u64,
    alignment: PhysicalMetadataValueV1<u64>,
    value_kind: ExplicitValueKind,
    address_space: PhysicalMetadataValueV1<ArgumentAddressSpace>,
    declared_access: PhysicalMetadataValueV1<ArgumentAccess>,
    actual_access: PhysicalMetadataValueV1<ArgumentAccess>,
}

impl PublishedPhysicalArgumentLayoutV1 {
    pub const fn offset(&self) -> u64 {
        self.offset
    }

    pub const fn size(&self) -> u64 {
        self.size
    }

    pub const fn alignment(&self) -> PhysicalMetadataValueV1<u64> {
        self.alignment
    }

    pub const fn value_kind(&self) -> ExplicitValueKind {
        self.value_kind
    }

    pub const fn address_space(&self) -> PhysicalMetadataValueV1<ArgumentAddressSpace> {
        self.address_space
    }

    pub const fn declared_access(&self) -> PhysicalMetadataValueV1<ArgumentAccess> {
        self.declared_access
    }

    pub const fn actual_access(&self) -> PhysicalMetadataValueV1<ArgumentAccess> {
        self.actual_access
    }
}

/// Directional launch and resource metadata observed for one kernel.
///
/// Known values were read from the executable. Unknown optional values remain unknown. Validation
/// only establishes that represented executable constraints do not contradict the narrower
/// manifest launch contract; it does not establish complete launch-contract equality.
/// AMDHSA does not constrain a source-level static rank, so `rank` is always `Unknown`, including
/// when hidden runtime grid-dimension arguments are present.
///
/// Workgroup requirements, maximum workgroups, maximum flat size, kernarg layout, fixed group
/// memory, and dynamic-LDS presence are compared where the manifest has matching semantics.
/// Cluster dimensions, private memory, wavefront/register resources, execution mode, and the
/// temporary GFX1250 revision have no matching manifest field and remain descriptive only. Kernel
/// lifecycle, dynamic-stack, and device-enqueue metadata are fail-closed admission checks and are
/// not converted into positive evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishedPhysicalLaunchLayoutV1 {
    rank: PhysicalMetadataValueV1<u8>,
    required_workgroup_size: PhysicalMetadataValueV1<[u32; 3]>,
    max_workgroups: [PhysicalMetadataValueV1<u32>; 3],
    cluster_dimensions: PhysicalMetadataValueV1<[u32; 3]>,
    max_flat_workgroup_size: u32,
    kernarg_segment_size: u64,
    kernarg_segment_alignment: u64,
    implicit_argument_offset: PhysicalMetadataValueV1<u64>,
    implicit_argument_size: u64,
    group_segment_fixed_size: u64,
    private_segment_fixed_size: u64,
    wavefront_size: u32,
    scalar_register_count: u16,
    vector_register_count: u16,
    accumulator_register_count: PhysicalMetadataValueV1<u32>,
    scalar_register_spill_count: PhysicalMetadataValueV1<u32>,
    vector_register_spill_count: PhysicalMetadataValueV1<u32>,
    workgroup_processor_mode: PhysicalMetadataValueV1<bool>,
    gfx1250_revision: PhysicalMetadataValueV1<Gfx1250Revision>,
    uniform_workgroup_size_indicator: PhysicalMetadataValueV1<bool>,
    dynamic_shared_memory_indicator: PhysicalMetadataValueV1<bool>,
}

impl PublishedPhysicalLaunchLayoutV1 {
    /// AMDHSA carries runtime grid dimensions but no static source-rank constraint.
    pub const fn rank(&self) -> PhysicalMetadataValueV1<u8> {
        self.rank
    }

    pub const fn required_workgroup_size(&self) -> PhysicalMetadataValueV1<[u32; 3]> {
        self.required_workgroup_size
    }

    pub const fn max_workgroups(&self) -> [PhysicalMetadataValueV1<u32>; 3] {
        self.max_workgroups
    }

    pub const fn cluster_dimensions(&self) -> PhysicalMetadataValueV1<[u32; 3]> {
        self.cluster_dimensions
    }

    pub const fn max_flat_workgroup_size(&self) -> u32 {
        self.max_flat_workgroup_size
    }

    pub const fn kernarg_segment_size(&self) -> u64 {
        self.kernarg_segment_size
    }

    pub const fn kernarg_segment_alignment(&self) -> u64 {
        self.kernarg_segment_alignment
    }

    pub const fn implicit_argument_offset(&self) -> PhysicalMetadataValueV1<u64> {
        self.implicit_argument_offset
    }

    pub const fn implicit_argument_size(&self) -> u64 {
        self.implicit_argument_size
    }

    pub const fn group_segment_fixed_size(&self) -> u64 {
        self.group_segment_fixed_size
    }

    pub const fn private_segment_fixed_size(&self) -> u64 {
        self.private_segment_fixed_size
    }

    pub const fn wavefront_size(&self) -> u32 {
        self.wavefront_size
    }

    pub const fn scalar_register_count(&self) -> u16 {
        self.scalar_register_count
    }

    pub const fn vector_register_count(&self) -> u16 {
        self.vector_register_count
    }

    pub const fn accumulator_register_count(&self) -> PhysicalMetadataValueV1<u32> {
        self.accumulator_register_count
    }

    pub const fn scalar_register_spill_count(&self) -> PhysicalMetadataValueV1<u32> {
        self.scalar_register_spill_count
    }

    pub const fn vector_register_spill_count(&self) -> PhysicalMetadataValueV1<u32> {
        self.vector_register_spill_count
    }

    pub const fn workgroup_processor_mode(&self) -> PhysicalMetadataValueV1<bool> {
        self.workgroup_processor_mode
    }

    pub const fn gfx1250_revision(&self) -> PhysicalMetadataValueV1<Gfx1250Revision> {
        self.gfx1250_revision
    }

    /// Returns `Known(true)` only when AMDHSA requires uniform workgroup sizes.
    /// Metadata absence and explicit false both remain `Unknown`.
    pub const fn uniform_workgroup_size_indicator(&self) -> PhysicalMetadataValueV1<bool> {
        self.uniform_workgroup_size_indicator
    }

    /// Returns `Known(true)` only when metadata contains a dynamic-LDS physical argument.
    /// Absence remains `Unknown`; this API never infers `Known(false)`.
    pub const fn dynamic_shared_memory_indicator(&self) -> PhysicalMetadataValueV1<bool> {
        self.dynamic_shared_memory_indicator
    }
}

/// Physical metadata for one exact manifest export symbol.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishedKernelPhysicalLayoutV1 {
    export_symbol: Box<str>,
    descriptor_symbol: Box<str>,
    arguments: Box<[PublishedPhysicalArgumentLayoutV1]>,
    launch: PublishedPhysicalLaunchLayoutV1,
}

impl PublishedKernelPhysicalLayoutV1 {
    /// Returns the manifest loader/export symbol, which is metadata `.name` under rule V1.
    pub fn export_symbol(&self) -> &str {
        &self.export_symbol
    }

    /// Returns the descriptor symbol derived and checked under rule V1.
    pub fn descriptor_symbol(&self) -> &str {
        &self.descriptor_symbol
    }

    pub fn arguments(&self) -> &[PublishedPhysicalArgumentLayoutV1] {
        &self.arguments
    }

    pub const fn launch(&self) -> &PublishedPhysicalLaunchLayoutV1 {
        &self.launch
    }
}

/// Inert physical-layout inspection bound to one complete published direct-link admission.
///
/// Construction consumes and retains the full admission, including its observed context, bridge,
/// publication lease, container, payload occurrence, kernel set, and artifact selection. It
/// acquires a fresh currentness token and parses only the retained descriptor snapshot; no caller
/// pathname or byte slice participates.
///
/// The result retains a locally authenticated exact-file lease but is physical, directional
/// evidence only. It authenticates no compiler, producer, Rust type, ownership, alias/effect
/// contract, or executable behavior and grants no module-loading or launch authority.
pub struct InspectedPublishedDirectLinkPhysicalLayoutV1 {
    admission: ValidatedPublishedDirectLinkSelectionV1,
    inspected: InspectedKernelBindings,
    kernels: Box<[PublishedKernelPhysicalLayoutV1]>,
    selected_kernel_index: usize,
}

impl fmt::Debug for InspectedPublishedDirectLinkPhysicalLayoutV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InspectedPublishedDirectLinkPhysicalLayoutV1")
            .field("admission", &self.admission)
            .field("payload_len", &self.admission.exact_artifact_bytes().len())
            .field("kernel_count", &self.kernels.len())
            .field("selected_kernel_index", &self.selected_kernel_index)
            .finish_non_exhaustive()
    }
}

impl InspectedPublishedDirectLinkPhysicalLayoutV1 {
    /// Inspects the retained exact descriptor snapshot and consumes its complete admission.
    pub fn inspect(
        admission: ValidatedPublishedDirectLinkSelectionV1,
    ) -> Result<Self, PublishedPhysicalLayoutInspectionError> {
        let current = admission
            .acquire_current_token()
            .map_err(PublishedPhysicalLayoutInspectionError::current_publication)?;
        Self::inspect_with_current_token(admission, &current)
    }

    /// Inspects with an already-held currentness token without reacquiring its lock.
    pub fn inspect_with_current_token(
        admission: ValidatedPublishedDirectLinkSelectionV1,
        current: &ManifestClaimDirectLinkCurrentPublicationTokenV1,
    ) -> Result<Self, PublishedPhysicalLayoutInspectionError> {
        admission
            .validate_current_token(current)
            .map_err(PublishedPhysicalLayoutInspectionError::current_publication)?;
        let exact_selected_payload_bytes = current.exact_artifact_bytes();
        validate_payload_occurrence(&admission, exact_selected_payload_bytes)?;
        let inspected = inspect_and_bind_kernel_descriptors(exact_selected_payload_bytes)
            .map_err(PublishedPhysicalLayoutInspectionError::Inspection)?;
        let (selected_kernel_index, kernels) = validate_inspection(&admission, &inspected)?;

        Ok(Self {
            admission,
            inspected,
            kernels,
            selected_kernel_index,
        })
    }

    /// Revalidates the complete original admission tuple and exact payload bytes.
    #[allow(clippy::too_many_arguments)]
    pub fn revalidate(
        &self,
        validated_bundle: &ValidatedDirectLinkBundleEvidenceV1<'_>,
        bridge: &ManifestClaimDirectLinkPublicationBridgeV1,
        container: &ArtifactContainerV1,
        selected: SelectedNativeKernel<'_>,
        observed: &ObservedContext,
    ) -> Result<(), PublishedPhysicalLayoutInspectionError> {
        let current = self
            .admission
            .acquire_current_token()
            .map_err(PublishedPhysicalLayoutInspectionError::current_publication)?;
        self.revalidate_with_current_token(
            &current,
            validated_bundle,
            bridge,
            container,
            selected,
            observed,
        )
    }

    /// Revalidates with an already-held currentness token without reacquiring its lock.
    #[allow(clippy::too_many_arguments)]
    pub fn revalidate_with_current_token(
        &self,
        current: &ManifestClaimDirectLinkCurrentPublicationTokenV1,
        validated_bundle: &ValidatedDirectLinkBundleEvidenceV1<'_>,
        bridge: &ManifestClaimDirectLinkPublicationBridgeV1,
        container: &ArtifactContainerV1,
        selected: SelectedNativeKernel<'_>,
        observed: &ObservedContext,
    ) -> Result<(), PublishedPhysicalLayoutInspectionError> {
        self.admission
            .revalidate_with_current_token(
                current,
                validated_bundle,
                bridge,
                container,
                selected,
                observed,
            )
            .map_err(PublishedPhysicalLayoutInspectionError::AdmissionRevalidation)
    }

    pub fn code_object_version(&self) -> CodeObjectVersion {
        self.inspected.inspection().code_object_version()
    }

    pub fn target(&self) -> AmdTargetId {
        self.inspected.inspection().target()
    }

    pub fn kernel_count(&self) -> usize {
        self.kernels.len()
    }

    pub fn selected_kernel(&self) -> &PublishedKernelPhysicalLayoutV1 {
        &self.kernels[self.selected_kernel_index]
    }

    pub fn selected_descriptor_binding(&self) -> KernelDescriptorBinding {
        self.inspected.bindings()[self.selected_kernel_index]
    }

    /// Revalidates currentness and holds the cooperative publication lock for a later stage.
    pub fn acquire_current_publication_token(
        &self,
    ) -> Result<
        ManifestClaimDirectLinkCurrentPublicationTokenV1,
        PublishedPhysicalLayoutInspectionError,
    > {
        self.admission
            .acquire_current_token()
            .map_err(PublishedPhysicalLayoutInspectionError::current_publication)
    }

    pub const fn authenticates_filesystem_artifact(&self) -> bool {
        true
    }

    pub const fn proves_compiler_provenance(&self) -> bool {
        false
    }

    pub const fn proves_rust_type_or_abi_agreement(&self) -> bool {
        false
    }

    pub const fn proves_ownership_alias_or_effects(&self) -> bool {
        false
    }

    pub const fn proves_complete_launch_contract(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

fn validate_payload_occurrence(
    admission: &ValidatedPublishedDirectLinkSelectionV1,
    exact_selected_payload_bytes: &[u8],
) -> Result<(), PublishedPhysicalLayoutInspectionError> {
    let selection = admission.artifact_selection();
    let selected_digest = selection.identity().payload_digest();
    if admission.finalized_payload_identity().digest() != selected_digest {
        return Err(PublishedPhysicalLayoutInspectionError::PayloadOccurrenceMismatch);
    }
    if selection.identity().code_object().byte_len()
        != u64::try_from(exact_selected_payload_bytes.len()).unwrap_or(u64::MAX)
    {
        return Err(PublishedPhysicalLayoutInspectionError::PayloadLengthMismatch);
    }
    if selected_digest
        .verify(exact_selected_payload_bytes)
        .is_err()
    {
        return Err(PublishedPhysicalLayoutInspectionError::PayloadDigestMismatch);
    }
    if selection.payload() != exact_selected_payload_bytes {
        return Err(PublishedPhysicalLayoutInspectionError::PayloadSubstitution);
    }
    Ok(())
}

fn validate_inspection(
    admission: &ValidatedPublishedDirectLinkSelectionV1,
    inspected: &InspectedKernelBindings,
) -> Result<(usize, Box<[PublishedKernelPhysicalLayoutV1]>), PublishedPhysicalLayoutInspectionError>
{
    let code_object_version = inspected.inspection().code_object_version();
    match code_object_version {
        CodeObjectVersion::V4 | CodeObjectVersion::V5 | CodeObjectVersion::V6 => {}
    }

    let declared_target = AmdTargetId::parse(
        admission
            .artifact_selection()
            .identity()
            .target()
            .architecture()
            .as_str(),
    )
    .map_err(PublishedPhysicalLayoutInspectionError::InvalidManifestTarget)?;
    if inspected.inspection().target() != declared_target {
        return Err(PublishedPhysicalLayoutInspectionError::TargetMismatch);
    }

    let expected = admission.payload_kernel_set();
    let actual = inspected.inspection().kernels();
    if expected.len() != actual.len() || inspected.bindings().len() != actual.len() {
        return Err(PublishedPhysicalLayoutInspectionError::KernelSetMismatch);
    }

    let mut expected_symbols = expected
        .iter()
        .map(|kernel| {
            let export = kernel.symbol().as_str().to_owned();
            let descriptor = descriptor_symbol_v1(kernel.symbol().as_str(), code_object_version);
            (export, descriptor)
        })
        .collect::<Vec<_>>();
    let mut actual_symbols = actual
        .iter()
        .map(|kernel| (kernel.name().to_owned(), kernel.symbol().to_owned()))
        .collect::<Vec<_>>();
    expected_symbols.sort_unstable();
    actual_symbols.sort_unstable();
    if expected_symbols != actual_symbols {
        return Err(PublishedPhysicalLayoutInspectionError::KernelSetMismatch);
    }

    let mut layouts = Vec::with_capacity(actual.len());
    for actual_kernel in actual {
        let expected_kernel = expected
            .iter()
            .find(|kernel| kernel.symbol().as_str() == actual_kernel.name())
            .ok_or(PublishedPhysicalLayoutInspectionError::KernelSetMismatch)?;
        layouts.push(validate_kernel_physical_layout(
            expected_kernel,
            actual_kernel,
            code_object_version,
        )?);
    }

    let selected_export = admission.artifact_selection().identity().symbol().as_str();
    let selected_kernel_index = actual
        .iter()
        .position(|kernel| kernel.name() == selected_export)
        .ok_or(PublishedPhysicalLayoutInspectionError::SelectedKernelMismatch)?;
    Ok((selected_kernel_index, layouts.into_boxed_slice()))
}

fn descriptor_symbol_v1(export: &str, code_object_version: CodeObjectVersion) -> String {
    match code_object_version {
        CodeObjectVersion::V4 | CodeObjectVersion::V5 | CodeObjectVersion::V6 => {
            format!("{export}.kd")
        }
    }
}

fn validate_kernel_physical_layout(
    expected: &PublishedPayloadKernelV1,
    actual: &InspectedKernel,
    code_object_version: CodeObjectVersion,
) -> Result<PublishedKernelPhysicalLayoutV1, PublishedPhysicalLayoutInspectionError> {
    let export = expected.symbol().as_str();
    if actual.name() != export
        || actual.symbol() != descriptor_symbol_v1(export, code_object_version)
    {
        return physical_mismatch(expected, "AMDHSA kernel identity rule V1");
    }
    if actual.kind() != KernelKind::Normal {
        return physical_mismatch(expected, "kernel kind");
    }
    if actual.uses_dynamic_stack() || actual.device_enqueue_symbol().is_some() {
        return physical_mismatch(expected, "unsupported loader lifecycle");
    }

    let arguments = validate_physical_arguments(expected, actual)?;
    let launch = validate_launch_evidence(expected, actual)?;
    Ok(PublishedKernelPhysicalLayoutV1 {
        export_symbol: export.into(),
        descriptor_symbol: actual.symbol().into(),
        arguments,
        launch,
    })
}

fn validate_physical_arguments(
    expected: &PublishedPayloadKernelV1,
    actual: &InspectedKernel,
) -> Result<Box<[PublishedPhysicalArgumentLayoutV1]>, PublishedPhysicalLayoutInspectionError> {
    let abi = expected.abi();
    let explicit_size = actual
        .implicit_argument_offset()
        .unwrap_or_else(|| actual.kernarg_segment_size());
    if abi.size() != explicit_size || actual.kernarg_segment_size() < explicit_size {
        return physical_mismatch(expected, "explicit kernarg size");
    }
    let abi_alignment = u64::from(abi.alignment());
    if !abi.fields().is_empty() && actual.kernarg_segment_alignment() != abi_alignment {
        return physical_mismatch(expected, "kernarg segment alignment");
    }

    let mut expected_arguments = Vec::new();
    for field in abi.fields() {
        match field.kind() {
            AbiKind::Scalar(_) => expected_arguments.push(ExpectedPhysicalArgument {
                offset: field.offset(),
                size: field.size(),
                alignment: u64::from(field.alignment()),
                value_kind: ExplicitValueKind::ByValue,
                address_space: None,
            }),
            AbiKind::Pointer { .. } => expected_arguments.push(ExpectedPhysicalArgument {
                offset: field.offset(),
                size: field.size(),
                alignment: u64::from(field.alignment()),
                value_kind: pointer_value_kind(field.address_space()),
                address_space: map_address_space(field.address_space()),
            }),
            AbiKind::Slice { .. } => {
                let pointer_bytes = abi.pointer_width().bytes();
                expected_arguments.push(ExpectedPhysicalArgument {
                    offset: field.offset(),
                    size: pointer_bytes,
                    alignment: u64::from(field.alignment()),
                    value_kind: pointer_value_kind(field.address_space()),
                    address_space: map_address_space(field.address_space()),
                });
                expected_arguments.push(ExpectedPhysicalArgument {
                    offset: field.offset() + pointer_bytes,
                    size: pointer_bytes,
                    alignment: u64::from(field.alignment()),
                    value_kind: ExplicitValueKind::ByValue,
                    address_space: None,
                });
            }
        }
    }
    if expected_arguments.len() != actual.explicit_arguments().len() {
        return physical_mismatch(expected, "physical argument count");
    }

    let mut evidence = Vec::with_capacity(expected_arguments.len());
    for (expected_argument, actual_argument) in
        expected_arguments.iter().zip(actual.explicit_arguments())
    {
        validate_argument(expected, expected_argument, actual_argument)?;
        evidence.push(PublishedPhysicalArgumentLayoutV1 {
            offset: actual_argument.offset(),
            size: actual_argument.size(),
            alignment: PhysicalMetadataValueV1::from_option(actual_argument.alignment()),
            value_kind: actual_argument.value_kind(),
            address_space: PhysicalMetadataValueV1::from_option(actual_argument.address_space()),
            declared_access: PhysicalMetadataValueV1::from_option(actual_argument.access()),
            actual_access: PhysicalMetadataValueV1::from_option(actual_argument.actual_access()),
        });
    }
    Ok(evidence.into_boxed_slice())
}

#[derive(Clone, Copy)]
struct ExpectedPhysicalArgument {
    offset: u64,
    size: u64,
    alignment: u64,
    value_kind: ExplicitValueKind,
    address_space: Option<ArgumentAddressSpace>,
}

fn validate_argument(
    kernel: &PublishedPayloadKernelV1,
    expected: &ExpectedPhysicalArgument,
    actual: &ExplicitArgument,
) -> Result<(), PublishedPhysicalLayoutInspectionError> {
    if actual.offset() != expected.offset
        || actual.size() != expected.size
        || actual
            .alignment()
            .is_some_and(|alignment| alignment != expected.alignment)
        || actual.value_kind() != expected.value_kind
        || actual.address_space() != expected.address_space
    {
        return physical_mismatch(kernel, "physical argument layout");
    }
    Ok(())
}

fn pointer_value_kind(address_space: AddressSpace) -> ExplicitValueKind {
    match address_space {
        AddressSpace::Workgroup => ExplicitValueKind::DynamicSharedPointer,
        AddressSpace::Value
        | AddressSpace::Global
        | AddressSpace::Constant
        | AddressSpace::Private
        | AddressSpace::Generic => ExplicitValueKind::GlobalBuffer,
    }
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

fn validate_launch_evidence(
    expected: &PublishedPayloadKernelV1,
    actual: &InspectedKernel,
) -> Result<PublishedPhysicalLaunchLayoutV1, PublishedPhysicalLayoutInspectionError> {
    let launch = expected.launch();
    if actual.group_segment_fixed_size() != u64::from(launch.static_shared_memory_bytes()) {
        return physical_mismatch(expected, "static group segment size");
    }

    let required_workgroup_size = match actual.required_workgroup_size() {
        Some(required) => {
            let BlockSize::Exact(dimensions) = launch.block_size() else {
                return physical_mismatch(expected, "required workgroup size");
            };
            if required != [dimensions.x(), dimensions.y(), dimensions.z()] {
                return physical_mismatch(expected, "required workgroup size");
            }
            PhysicalMetadataValueV1::Known(required)
        }
        None => PhysicalMetadataValueV1::Unknown,
    };

    if let BlockSize::Exact(dimensions) | BlockSize::AtMost(dimensions) = launch.block_size() {
        let flat = u64::from(dimensions.x())
            .checked_mul(u64::from(dimensions.y()))
            .and_then(|value| value.checked_mul(u64::from(dimensions.z())))
            .ok_or_else(
                || PublishedPhysicalLayoutInspectionError::PhysicalLayoutMismatch {
                    export_symbol: expected.symbol().as_str().to_owned(),
                    field: "workgroup dimensions",
                },
            )?;
        if flat > u64::from(actual.max_flat_workgroup_size()) {
            return physical_mismatch(expected, "maximum flat workgroup size");
        }
    }

    let manifest_grid = launch.max_grid();
    let manifest_grid_axes = [manifest_grid.x(), manifest_grid.y(), manifest_grid.z()];
    let inspected_max = actual.max_workgroups();
    let mut max_workgroups = [PhysicalMetadataValueV1::Unknown; 3];
    for axis in 0..3 {
        if let Some(maximum) = inspected_max[axis] {
            if manifest_grid_axes[axis] > maximum {
                return physical_mismatch(expected, "maximum workgroups");
            }
            max_workgroups[axis] = PhysicalMetadataValueV1::Known(maximum);
        }
    }

    let dynamic_shared_memory_indicator = if actual
        .explicit_arguments()
        .iter()
        .any(|argument| argument.value_kind() == ExplicitValueKind::DynamicSharedPointer)
        || actual
            .hidden_arguments()
            .iter()
            .any(|argument| argument.value_kind() == HiddenValueKind::DynamicLdsSize)
    {
        if launch.max_dynamic_shared_memory_bytes() == 0 {
            return physical_mismatch(expected, "dynamic shared-memory relation");
        }
        PhysicalMetadataValueV1::Known(true)
    } else {
        PhysicalMetadataValueV1::Unknown
    };

    Ok(PublishedPhysicalLaunchLayoutV1 {
        rank: PhysicalMetadataValueV1::Unknown,
        required_workgroup_size,
        max_workgroups,
        cluster_dimensions: PhysicalMetadataValueV1::from_option(actual.cluster_dims()),
        max_flat_workgroup_size: actual.max_flat_workgroup_size(),
        kernarg_segment_size: actual.kernarg_segment_size(),
        kernarg_segment_alignment: actual.kernarg_segment_alignment(),
        implicit_argument_offset: PhysicalMetadataValueV1::from_option(
            actual.implicit_argument_offset(),
        ),
        implicit_argument_size: actual.implicit_argument_size(),
        group_segment_fixed_size: actual.group_segment_fixed_size(),
        private_segment_fixed_size: actual.private_segment_fixed_size(),
        wavefront_size: actual.wavefront_size(),
        scalar_register_count: actual.sgpr_count(),
        vector_register_count: actual.vgpr_count(),
        accumulator_register_count: PhysicalMetadataValueV1::from_option(actual.agpr_count()),
        scalar_register_spill_count: PhysicalMetadataValueV1::from_option(
            actual.sgpr_spill_count(),
        ),
        vector_register_spill_count: PhysicalMetadataValueV1::from_option(
            actual.vgpr_spill_count(),
        ),
        workgroup_processor_mode: PhysicalMetadataValueV1::from_option(
            actual.workgroup_processor_mode(),
        ),
        gfx1250_revision: PhysicalMetadataValueV1::from_option(actual.gfx1250_revision()),
        uniform_workgroup_size_indicator: if actual.uniform_work_group_size() {
            PhysicalMetadataValueV1::Known(true)
        } else {
            PhysicalMetadataValueV1::Unknown
        },
        dynamic_shared_memory_indicator,
    })
}

fn physical_mismatch<T>(
    kernel: &PublishedPayloadKernelV1,
    field: &'static str,
) -> Result<T, PublishedPhysicalLayoutInspectionError> {
    Err(
        PublishedPhysicalLayoutInspectionError::PhysicalLayoutMismatch {
            export_symbol: kernel.symbol().as_str().to_owned(),
            field,
        },
    )
}

/// Failure to bind inert physical-layout inspection to an exact published admission.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PublishedPhysicalLayoutInspectionError {
    Busy,
    PayloadOccurrenceMismatch,
    PayloadLengthMismatch,
    PayloadDigestMismatch,
    PayloadSubstitution,
    Inspection(KernelBindingError),
    InvalidManifestTarget(ParseAmdTargetIdError),
    TargetMismatch,
    KernelSetMismatch,
    SelectedKernelMismatch,
    PhysicalLayoutMismatch {
        export_symbol: String,
        field: &'static str,
    },
    AdmissionRevalidation(PublishedDirectLinkAdmissionError),
    CurrentPublication {
        reason: String,
    },
}

impl fmt::Display for PublishedPhysicalLayoutInspectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Busy => formatter.write_str("durable publication lock is busy"),
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
                formatter.write_str("payload bytes differ from the retained admission")
            }
            Self::Inspection(error) => error.fmt(formatter),
            Self::InvalidManifestTarget(error) => {
                write!(formatter, "manifest AMD target is invalid: {error}")
            }
            Self::TargetMismatch => {
                formatter.write_str("inspected HSACO target differs from the manifest target")
            }
            Self::KernelSetMismatch => formatter.write_str(
                "inspected AMDHSA export and descriptor symbols differ from the manifest payload set",
            ),
            Self::SelectedKernelMismatch => formatter
                .write_str("selected manifest export symbol is absent from inspected metadata"),
            Self::PhysicalLayoutMismatch {
                export_symbol,
                field,
            } => write!(
                formatter,
                "kernel export {export_symbol} physical metadata conflicts for {field}"
            ),
            Self::AdmissionRevalidation(error) => error.fmt(formatter),
            Self::CurrentPublication { reason } => {
                write!(formatter, "current publication lease failed revalidation: {reason}")
            }
        }
    }
}

impl std::error::Error for PublishedPhysicalLayoutInspectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Inspection(error) => Some(error),
            Self::InvalidManifestTarget(error) => Some(error),
            Self::AdmissionRevalidation(error) => Some(error),
            Self::PayloadOccurrenceMismatch
            | Self::Busy
            | Self::PayloadLengthMismatch
            | Self::PayloadDigestMismatch
            | Self::PayloadSubstitution
            | Self::CurrentPublication { .. }
            | Self::TargetMismatch
            | Self::KernelSetMismatch
            | Self::SelectedKernelMismatch
            | Self::PhysicalLayoutMismatch { .. } => None,
        }
    }
}

impl PublishedPhysicalLayoutInspectionError {
    fn current_publication(error: fe2o3_artifact_transaction::DurableLinkPublicationError) -> Self {
        match error {
            fe2o3_artifact_transaction::DurableLinkPublicationError::Busy => Self::Busy,
            error => Self::CurrentPublication {
                reason: error.to_string(),
            },
        }
    }
}
