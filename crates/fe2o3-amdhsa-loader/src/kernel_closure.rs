use fe2o3_hsaco::{
    AmdhsaKernelDescriptor, CodeObjectVersion, InspectedKernel, InspectedKernelBindings,
    KernelBindingError, KernelDescriptorBinding, KernelKind, MetadataDescriptorRange,
    inspect_and_bind_kernel_descriptors,
};
use sha2::{Digest, Sha256};

use super::{
    AdmittedProfile, LOADER_PROFILE_ID, LoadPlan, MaterializationError, MetadataNote,
    SegmentPermissions, ValidatedEnvelope,
};

const KERNEL_DESCRIPTOR_BYTES: u64 = 64;
const IDENTITY_DOMAIN: &[u8] = b"fe2o3.amdhsa.loaded-kernel-identity-inputs.v1";

/// Exact closed relocation policy enforced before a kernel closure is built.
pub const CLOSED_RELOCATION_POLICY_ID: &str = "fe2o3.amdhsa.no-relocations.v1";

/// Runtime semantics that this loader slice deliberately does not implement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnsupportedKernelSemantic {
    Init,
    Fini,
    DynamicStack,
    DeviceEnqueue,
}

/// Why exact semantic and selected-kernel closure failed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KernelClosureError {
    KernelNameTooLong,
    Inspection(KernelBindingError),
    UnsupportedCodeObjectVersion(u8),
    UnsupportedMetadataVersion {
        major: u32,
        minor: u32,
    },
    UnsupportedTarget,
    PrintfMetadataUnsupported,
    UnsupportedSemantic {
        kernel_index: usize,
        semantic: UnsupportedKernelSemantic,
    },
    MetadataRangeMismatch {
        loader: MetadataNote,
        inspector: MetadataDescriptorRange,
    },
    BindingCardinalityMismatch,
    KernelNotFound,
    BindingIndexMismatch,
    DescriptorRangeUnavailable,
    EntryRangeUnavailable,
    DescriptorLoadMappingMismatch,
    EntryLoadMappingMismatch,
}

impl From<KernelBindingError> for KernelClosureError {
    fn from(error: KernelBindingError) -> Self {
        Self::Inspection(error)
    }
}

/// Evidence that successful parsing found no relocation mechanism to apply.
///
/// Construction is private. A successful value means the envelope parser
/// rejected every `SHT_REL`/`SHT_RELA` section, every dynamic relocation tag,
/// and every unknown section or dynamic tag before semantic binding. It is
/// runtime-checked evidence, not a Verus proof and not load authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClosedRelocationEvidenceV1 {
    private: (),
}

impl ClosedRelocationEvidenceV1 {
    /// Stable identifier of the enforced policy.
    pub const fn policy_id(self) -> &'static str {
        CLOSED_RELOCATION_POLICY_ID
    }

    /// Number of admitted relocation records.
    pub const fn admitted_relocation_count(self) -> u64 {
        0
    }

    /// Number of relocations applied to the materialized image.
    pub const fn applied_relocation_count(self) -> u64 {
        0
    }
}

/// Selected metadata resources cross-checked against one exact descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectedKernelResourceBindingV1 {
    kernarg_segment_size: u64,
    kernarg_segment_alignment: u64,
    group_segment_fixed_size: u64,
    private_segment_fixed_size: u64,
    wavefront_size: u32,
    sgpr_count: u16,
    vgpr_count: u16,
    agpr_count: Option<u32>,
    sgpr_spill_count: Option<u32>,
    vgpr_spill_count: Option<u32>,
    max_flat_workgroup_size: u32,
    required_workgroup_size: Option<[u32; 3]>,
    max_workgroups: [Option<u32>; 3],
    cluster_dims: Option<[u32; 3]>,
    descriptor: AmdhsaKernelDescriptor,
}

impl SelectedKernelResourceBindingV1 {
    fn new(kernel: &InspectedKernel, descriptor: AmdhsaKernelDescriptor) -> Self {
        Self {
            kernarg_segment_size: kernel.kernarg_segment_size(),
            kernarg_segment_alignment: kernel.kernarg_segment_alignment(),
            group_segment_fixed_size: kernel.group_segment_fixed_size(),
            private_segment_fixed_size: kernel.private_segment_fixed_size(),
            wavefront_size: kernel.wavefront_size(),
            sgpr_count: kernel.sgpr_count(),
            vgpr_count: kernel.vgpr_count(),
            agpr_count: kernel.agpr_count(),
            sgpr_spill_count: kernel.sgpr_spill_count(),
            vgpr_spill_count: kernel.vgpr_spill_count(),
            max_flat_workgroup_size: kernel.max_flat_workgroup_size(),
            required_workgroup_size: kernel.required_workgroup_size(),
            max_workgroups: kernel.max_workgroups(),
            cluster_dims: kernel.cluster_dims(),
            descriptor,
        }
    }

    pub const fn kernarg_segment_size(self) -> u64 {
        self.kernarg_segment_size
    }

    pub const fn kernarg_segment_alignment(self) -> u64 {
        self.kernarg_segment_alignment
    }

    pub const fn group_segment_fixed_size(self) -> u64 {
        self.group_segment_fixed_size
    }

    pub const fn private_segment_fixed_size(self) -> u64 {
        self.private_segment_fixed_size
    }

    pub const fn wavefront_size(self) -> u32 {
        self.wavefront_size
    }

    pub const fn sgpr_count(self) -> u16 {
        self.sgpr_count
    }

    pub const fn vgpr_count(self) -> u16 {
        self.vgpr_count
    }

    pub const fn agpr_count(self) -> Option<u32> {
        self.agpr_count
    }

    pub const fn sgpr_spill_count(self) -> Option<u32> {
        self.sgpr_spill_count
    }

    pub const fn vgpr_spill_count(self) -> Option<u32> {
        self.vgpr_spill_count
    }

    pub const fn max_flat_workgroup_size(self) -> u32 {
        self.max_flat_workgroup_size
    }

    pub const fn required_workgroup_size(self) -> Option<[u32; 3]> {
        self.required_workgroup_size
    }

    pub const fn max_workgroups(self) -> [Option<u32>; 3] {
        self.max_workgroups
    }

    pub const fn cluster_dims(self) -> Option<[u32; 3]> {
        self.cluster_dims
    }

    pub const fn descriptor(self) -> AmdhsaKernelDescriptor {
        self.descriptor
    }
}

/// Deterministic, content-bound inputs for one selected kernel identity.
///
/// The closure digest uses length-delimited, domain-separated fields. The
/// object digest binds every input byte; the component digests make the exact
/// metadata, descriptor, and entry dependencies directly auditable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KernelIdentityInputsV1 {
    object_sha256: [u8; 32],
    metadata_sha256: [u8; 32],
    descriptor_sha256: [u8; 32],
    entry_sha256: [u8; 32],
    closure_sha256: [u8; 32],
}

impl KernelIdentityInputsV1 {
    pub const fn object_sha256(self) -> [u8; 32] {
        self.object_sha256
    }

    pub const fn metadata_sha256(self) -> [u8; 32] {
        self.metadata_sha256
    }

    pub const fn descriptor_sha256(self) -> [u8; 32] {
        self.descriptor_sha256
    }

    pub const fn entry_sha256(self) -> [u8; 32] {
        self.entry_sha256
    }

    pub const fn closure_sha256(self) -> [u8; 32] {
        self.closure_sha256
    }
}

/// Exact envelope, semantic metadata, and selected ELF kernel binding.
///
/// This type owns the independently checked HSACO description and retains the
/// original input borrow through its envelope. It can describe or materialize
/// bytes, but cannot allocate GPU memory, map an executable, transition W^X,
/// resolve a device address, or authorize a dispatch.
pub struct ValidatedKernelEnvelope<'a> {
    envelope: ValidatedEnvelope<'a>,
    bindings: InspectedKernelBindings,
    selected_index: usize,
    descriptor_bytes: &'a [u8],
    entry_bytes: &'a [u8],
    resources: SelectedKernelResourceBindingV1,
    relocation: ClosedRelocationEvidenceV1,
    identity: KernelIdentityInputsV1,
}

impl<'a> ValidatedKernelEnvelope<'a> {
    pub const fn envelope(&self) -> &ValidatedEnvelope<'a> {
        &self.envelope
    }

    /// Materializes the exact closed object's canonical CPU-side image.
    ///
    /// This delegates to the envelope's fail-before-mutation, zero-then-copy
    /// operation and still grants no allocation, GPU mapping, W^X, or dispatch
    /// authority.
    pub fn materialize_into(&self, destination: &mut [u8]) -> Result<(), MaterializationError> {
        self.envelope.materialize_into(destination)
    }

    pub fn selected_kernel(&self) -> &InspectedKernel {
        &self.bindings.inspection().kernels()[self.selected_index]
    }

    pub const fn selected_kernel_index(&self) -> usize {
        self.selected_index
    }

    /// Exact static descriptor and entry-symbol location selected in the
    /// retained object. The value is descriptive and grants no device address.
    pub fn selected_binding(&self) -> KernelDescriptorBinding {
        self.bindings.bindings()[self.selected_index]
    }

    pub const fn descriptor_bytes(&self) -> &'a [u8] {
        self.descriptor_bytes
    }

    pub const fn entry_bytes(&self) -> &'a [u8] {
        self.entry_bytes
    }

    pub const fn resources(&self) -> SelectedKernelResourceBindingV1 {
        self.resources
    }

    pub const fn relocation_evidence(&self) -> ClosedRelocationEvidenceV1 {
        self.relocation
    }

    pub const fn identity_inputs(&self) -> KernelIdentityInputsV1 {
        self.identity
    }
}

impl<'a> ValidatedEnvelope<'a> {
    /// Consumes this envelope and closes exact COV6 semantic, symbol,
    /// descriptor, resource, mapping, relocation, and content identity for one
    /// metadata kernel name.
    pub fn bind_kernel(
        self,
        kernel_name: &str,
    ) -> Result<ValidatedKernelEnvelope<'a>, KernelClosureError> {
        if kernel_name.len() > fe2o3_hsaco::MAX_MESSAGEPACK_STRING_BYTES {
            return Err(KernelClosureError::KernelNameTooLong);
        }

        let bindings = inspect_and_bind_kernel_descriptors(self.bytes)?;
        let inspection = bindings.inspection();
        if inspection.code_object_version() != CodeObjectVersion::V6 {
            return Err(KernelClosureError::UnsupportedCodeObjectVersion(
                inspection.code_object_version().number(),
            ));
        }
        let metadata_version = inspection.metadata_version();
        if (metadata_version.major(), metadata_version.minor()) != (1, 2) {
            return Err(KernelClosureError::UnsupportedMetadataVersion {
                major: metadata_version.major(),
                minor: metadata_version.minor(),
            });
        }
        if inspection.target().processor() != "gfx942"
            || inspection.target().amdhsa_elf_flags_v4_plus()
                != AdmittedProfile::Gfx942XnackOffCov6.elf_flags()
        {
            return Err(KernelClosureError::UnsupportedTarget);
        }
        if inspection.has_printf_metadata() {
            return Err(KernelClosureError::PrintfMetadataUnsupported);
        }
        reject_unsupported_semantics(inspection.kernels())?;

        let loader_metadata = self.plan.metadata_note();
        let inspector_metadata = inspection.metadata_descriptor_range();
        if loader_metadata.file_offset() != inspector_metadata.file_offset()
            || loader_metadata.byte_len() != inspector_metadata.byte_len()
        {
            return Err(KernelClosureError::MetadataRangeMismatch {
                loader: loader_metadata,
                inspector: inspector_metadata,
            });
        }
        if bindings.bindings().len() != inspection.kernels().len() {
            return Err(KernelClosureError::BindingCardinalityMismatch);
        }

        let selected_index = inspection
            .kernels()
            .iter()
            .position(|kernel| kernel.name() == kernel_name)
            .ok_or(KernelClosureError::KernelNotFound)?;
        let selected_kernel = &inspection.kernels()[selected_index];
        let selected_binding = *bindings
            .bindings()
            .get(selected_index)
            .ok_or(KernelClosureError::BindingCardinalityMismatch)?;
        if selected_binding.kernel_index() != selected_index {
            return Err(KernelClosureError::BindingIndexMismatch);
        }

        let descriptor_bytes = exact_range(
            self.bytes,
            selected_binding.descriptor_file_offset(),
            KERNEL_DESCRIPTOR_BYTES,
        )
        .ok_or(KernelClosureError::DescriptorRangeUnavailable)?;
        let entry_bytes = exact_range(
            self.bytes,
            selected_binding.entry_file_offset(),
            selected_binding.entry_size(),
        )
        .ok_or(KernelClosureError::EntryRangeUnavailable)?;

        if !mapping_matches(
            &self.plan,
            selected_binding.descriptor_file_offset(),
            selected_binding.descriptor_address(),
            KERNEL_DESCRIPTOR_BYTES,
            SegmentPermissions::ReadOnly,
        ) {
            return Err(KernelClosureError::DescriptorLoadMappingMismatch);
        }
        if !mapping_matches(
            &self.plan,
            selected_binding.entry_file_offset(),
            selected_binding.entry_address(),
            selected_binding.entry_size(),
            SegmentPermissions::ReadExecute,
        ) {
            return Err(KernelClosureError::EntryLoadMappingMismatch);
        }

        let resources =
            SelectedKernelResourceBindingV1::new(selected_kernel, selected_binding.descriptor());
        let relocation = ClosedRelocationEvidenceV1 { private: () };
        let identity = identity_inputs(
            self.bytes,
            self.metadata_descriptor,
            descriptor_bytes,
            entry_bytes,
            loader_metadata,
            selected_index,
            selected_kernel,
            selected_binding.descriptor_file_offset(),
            selected_binding.descriptor_address(),
            selected_binding.entry_file_offset(),
            selected_binding.entry_address(),
            relocation,
        );

        Ok(ValidatedKernelEnvelope {
            envelope: self,
            bindings,
            selected_index,
            descriptor_bytes,
            entry_bytes,
            resources,
            relocation,
            identity,
        })
    }
}

fn reject_unsupported_semantics(kernels: &[InspectedKernel]) -> Result<(), KernelClosureError> {
    for (kernel_index, kernel) in kernels.iter().enumerate() {
        let semantic = match kernel.kind() {
            KernelKind::Init => Some(UnsupportedKernelSemantic::Init),
            KernelKind::Fini => Some(UnsupportedKernelSemantic::Fini),
            KernelKind::Normal if kernel.uses_dynamic_stack() => {
                Some(UnsupportedKernelSemantic::DynamicStack)
            }
            KernelKind::Normal if kernel.device_enqueue_symbol().is_some() => {
                Some(UnsupportedKernelSemantic::DeviceEnqueue)
            }
            KernelKind::Normal => None,
        };
        if let Some(semantic) = semantic {
            return Err(KernelClosureError::UnsupportedSemantic {
                kernel_index,
                semantic,
            });
        }
    }
    Ok(())
}

fn exact_range(bytes: &[u8], offset: u64, byte_len: u64) -> Option<&[u8]> {
    let end = offset.checked_add(byte_len)?;
    let start = usize::try_from(offset).ok()?;
    let end = usize::try_from(end).ok()?;
    bytes.get(start..end)
}

fn mapping_matches(
    plan: &LoadPlan,
    file_offset: u64,
    virtual_address: u64,
    byte_len: u64,
    permissions: SegmentPermissions,
) -> bool {
    let Some(file_end) = file_offset.checked_add(byte_len) else {
        return false;
    };
    let Some(virtual_end) = virtual_address.checked_add(byte_len) else {
        return false;
    };
    let mut matches = 0usize;
    for segment in plan.segments() {
        let Some(segment_file_end) = segment.file_offset().checked_add(segment.file_size()) else {
            return false;
        };
        let Some(segment_virtual_end) = segment.virtual_address().checked_add(segment.file_size())
        else {
            return false;
        };
        if file_offset < segment.file_offset()
            || file_end > segment_file_end
            || virtual_address < segment.virtual_address()
            || virtual_end > segment_virtual_end
        {
            continue;
        }
        let Some(delta) = file_offset.checked_sub(segment.file_offset()) else {
            return false;
        };
        let Some(translated) = segment.virtual_address().checked_add(delta) else {
            return false;
        };
        if translated != virtual_address || segment.permissions() != permissions {
            return false;
        }
        matches += 1;
    }
    matches == 1
}

#[allow(clippy::too_many_arguments)]
fn identity_inputs(
    object: &[u8],
    metadata: &[u8],
    descriptor: &[u8],
    entry: &[u8],
    metadata_range: MetadataNote,
    selected_index: usize,
    kernel: &InspectedKernel,
    descriptor_file_offset: u64,
    descriptor_address: u64,
    entry_file_offset: u64,
    entry_address: u64,
    relocation: ClosedRelocationEvidenceV1,
) -> KernelIdentityInputsV1 {
    let object_sha256 = digest(object);
    let metadata_sha256 = digest(metadata);
    let descriptor_sha256 = digest(descriptor);
    let entry_sha256 = digest(entry);

    let mut hasher = Sha256::new();
    update_field(&mut hasher, b"domain", IDENTITY_DOMAIN);
    update_field(&mut hasher, b"loader-profile", LOADER_PROFILE_ID.as_bytes());
    update_field(
        &mut hasher,
        b"relocation-policy",
        relocation.policy_id().as_bytes(),
    );
    update_field(&mut hasher, b"object-sha256", &object_sha256);
    update_u64(&mut hasher, b"object-length", object.len() as u64);
    update_field(&mut hasher, b"metadata-sha256", &metadata_sha256);
    update_u64(
        &mut hasher,
        b"metadata-file-offset",
        metadata_range.file_offset(),
    );
    update_u64(&mut hasher, b"metadata-length", metadata_range.byte_len());
    update_u64(&mut hasher, b"kernel-index", selected_index as u64);
    update_field(&mut hasher, b"kernel-name", kernel.name().as_bytes());
    update_field(&mut hasher, b"kernel-symbol", kernel.symbol().as_bytes());
    update_field(&mut hasher, b"descriptor-sha256", &descriptor_sha256);
    update_u64(
        &mut hasher,
        b"descriptor-file-offset",
        descriptor_file_offset,
    );
    update_u64(&mut hasher, b"descriptor-address", descriptor_address);
    update_field(&mut hasher, b"entry-sha256", &entry_sha256);
    update_u64(&mut hasher, b"entry-file-offset", entry_file_offset);
    update_u64(&mut hasher, b"entry-address", entry_address);
    update_u64(&mut hasher, b"entry-length", entry.len() as u64);
    let closure_sha256 = hasher.finalize().into();

    KernelIdentityInputsV1 {
        object_sha256,
        metadata_sha256,
        descriptor_sha256,
        entry_sha256,
        closure_sha256,
    }
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn update_u64(hasher: &mut Sha256, label: &[u8], value: u64) {
    update_field(hasher, label, &value.to_le_bytes());
}

fn update_field(hasher: &mut Sha256, label: &[u8], value: &[u8]) {
    hasher.update((label.len() as u64).to_le_bytes());
    hasher.update(label);
    hasher.update((value.len() as u64).to_le_bytes());
    hasher.update(value);
}
