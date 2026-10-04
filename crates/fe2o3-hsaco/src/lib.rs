#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod elf_inspection;
mod error;
mod kernel_binding;
mod messagepack;
mod metadata;

use fe2o3_amd_target::AmdTargetId;

pub use error::{InspectionError, KernelBindingError, MessagePackLimit};
pub use kernel_binding::{
    AmdhsaKernelDescriptor, CodeObjectLoadLayout, InspectedKernelBindings, KernelDescriptorBinding,
};

/// Maximum accepted HSACO size (64 MiB).
pub const MAX_HSACO_BYTES: usize = 64 * 1024 * 1024;
/// Maximum accepted metadata note size (4 MiB).
pub const MAX_METADATA_BYTES: usize = 4 * 1024 * 1024;
/// Maximum number of ELF section headers.
pub const MAX_ELF_SECTIONS: usize = 256;
/// Maximum number of ELF program headers.
pub const MAX_ELF_SEGMENTS: usize = 64;
/// Maximum number of note records inspected across section and segment views.
pub const MAX_ELF_NOTES: usize = 64;
/// Maximum MessagePack container nesting.
pub const MAX_MESSAGEPACK_DEPTH: usize = 32;
/// Maximum MessagePack values, including map keys.
pub const MAX_MESSAGEPACK_NODES: usize = 32 * 1024;
/// Maximum entries in one MessagePack array or map.
pub const MAX_MESSAGEPACK_COLLECTION_ITEMS: usize = 1024;
/// Maximum entries summed across all MessagePack arrays and maps.
pub const MAX_MESSAGEPACK_TOTAL_ITEMS: usize = 32 * 1024;
/// Maximum MessagePack string length.
pub const MAX_MESSAGEPACK_STRING_BYTES: usize = 4096;
/// Maximum MessagePack binary or extension payload length.
pub const MAX_MESSAGEPACK_BLOB_BYTES: usize = 4096;
/// Maximum number of kernels in one metadata document.
pub const MAX_KERNELS: usize = 256;
/// Maximum number of physical arguments for one kernel.
pub const MAX_ARGUMENTS_PER_KERNEL: usize = 512;
/// Maximum accepted kernel argument segment size (1 MiB).
pub const MAX_KERNARG_BYTES: u64 = 1024 * 1024;
/// Exact implicit kernarg suffix required when code object V6 metadata declares hidden records.
pub const COV6_IMPLICIT_ARGUMENT_BYTES: u64 = 256;
/// Maximum number of static ELF symbols scanned during explicit binding.
pub const MAX_ELF_SYMBOLS: usize = 32 * 1024;

/// AMDGPU HSA code object version encoded by the ELF ABI byte.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodeObjectVersion {
    V4,
    V5,
    V6,
}

impl CodeObjectVersion {
    /// Returns the numeric AMDGPU HSA code object version.
    pub const fn number(self) -> u8 {
        match self {
            Self::V4 => 4,
            Self::V5 => 5,
            Self::V6 => 6,
        }
    }
}

/// Version tuple declared by the AMDHSA metadata document.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MetadataVersion {
    major: u32,
    minor: u32,
}

/// Exact file range of the MessagePack descriptor selected by inspection.
///
/// This range lets a composing loader confirm that its independently parsed
/// AMDGPU note refers to the same physical bytes. It carries no metadata,
/// symbol, load, or launch authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MetadataDescriptorRange {
    file_offset: u64,
    byte_len: u64,
}

impl MetadataDescriptorRange {
    /// Byte offset of the descriptor in the inspected input.
    pub const fn file_offset(self) -> u64 {
        self.file_offset
    }

    /// Exact descriptor length in bytes.
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
}

impl MetadataVersion {
    pub(crate) const fn new(major: u32, minor: u32) -> Self {
        Self { major, minor }
    }

    /// Returns the metadata major version.
    pub const fn major(self) -> u32 {
        self.major
    }

    /// Returns the metadata minor version.
    pub const fn minor(self) -> u32 {
        self.minor
    }
}

/// Physical non-runtime argument kind declared by AMDHSA metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExplicitValueKind {
    ByValue,
    GlobalBuffer,
    DynamicSharedPointer,
    Sampler,
    Image,
    Pipe,
    Queue,
}

/// Canonical scalar spelling retained from the deprecated AMDHSA `.value_type` field.
///
/// Current LLVM may omit this field. When present, inspection preserves the declaration so
/// higher-level exact profiles can reject contradictions instead of silently discarding it.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExplicitValueType {
    Struct,
    I8,
    U8,
    I16,
    U16,
    F16,
    I32,
    U32,
    F32,
    I64,
    U64,
    F64,
}

/// Runtime-populated argument kind declared by AMDHSA metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HiddenValueKind {
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
    None,
    PrintfBuffer,
    HostcallBuffer,
    HeapV1,
    DefaultQueue,
    CompletionAction,
    MultigridSyncArgument,
    DynamicLdsSize,
    PrivateBase,
    SharedBase,
    QueuePointer,
}

/// Address-space qualifier attached to an explicit physical argument.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArgumentAddressSpace {
    Private,
    Global,
    Constant,
    Local,
    Generic,
    Region,
}

/// Access qualifier attached to an explicit physical argument.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArgumentAccess {
    ReadOnly,
    WriteOnly,
    ReadWrite,
}

/// Loader lifecycle classification declared for a kernel.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum KernelKind {
    /// An ordinary dispatchable kernel.
    #[default]
    Normal,
    /// A kernel that must run after loading the code object.
    Init,
    /// A kernel that must run before unloading the code object.
    Fini,
}

/// Temporary GFX1250-family stepping evidence emitted by the pinned LLVM.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx1250Revision {
    A0,
    B0,
}

/// One caller-provided physical kernel argument.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExplicitArgument {
    name: Option<Box<str>>,
    type_name: Option<Box<str>>,
    offset: u64,
    size: u64,
    alignment: Option<u64>,
    value_kind: ExplicitValueKind,
    value_type: Option<ExplicitValueType>,
    address_space: Option<ArgumentAddressSpace>,
    access: Option<ArgumentAccess>,
    actual_access: Option<ArgumentAccess>,
    pointee_alignment: Option<u64>,
    is_const: Option<bool>,
    is_restrict: Option<bool>,
    is_volatile: Option<bool>,
    is_pipe: Option<bool>,
}

impl ExplicitArgument {
    /// Returns the optional source-level argument name.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Returns the optional source-level type spelling.
    pub fn type_name(&self) -> Option<&str> {
        self.type_name.as_deref()
    }

    /// Returns the byte offset in the kernarg segment.
    pub const fn offset(&self) -> u64 {
        self.offset
    }

    /// Returns the physical byte size.
    pub const fn size(&self) -> u64 {
        self.size
    }

    /// Returns the optional physical argument alignment in bytes.
    pub const fn alignment(&self) -> Option<u64> {
        self.alignment
    }

    /// Returns the physical AMDHSA value kind.
    pub const fn value_kind(&self) -> ExplicitValueKind {
        self.value_kind
    }

    /// Returns the canonical deprecated `.value_type` declaration, retaining omission as `None`.
    pub const fn value_type(&self) -> Option<ExplicitValueType> {
        self.value_type
    }

    /// Returns the optional address-space qualifier.
    pub const fn address_space(&self) -> Option<ArgumentAddressSpace> {
        self.address_space
    }

    /// Returns the optional declared access qualifier.
    pub const fn access(&self) -> Option<ArgumentAccess> {
        self.access
    }

    /// Returns the optional backend-derived actual access qualifier.
    pub const fn actual_access(&self) -> Option<ArgumentAccess> {
        self.actual_access
    }

    /// Returns the optional pointee alignment in bytes.
    pub const fn pointee_alignment(&self) -> Option<u64> {
        self.pointee_alignment
    }

    /// Returns the emitted const qualifier, retaining absence as `None`.
    pub const fn is_const(&self) -> Option<bool> {
        self.is_const
    }

    /// Returns the emitted restrict qualifier, retaining absence as `None`.
    pub const fn is_restrict(&self) -> Option<bool> {
        self.is_restrict
    }

    /// Returns the emitted volatile qualifier, retaining absence as `None`.
    pub const fn is_volatile(&self) -> Option<bool> {
        self.is_volatile
    }

    /// Returns the emitted pipe qualifier, retaining absence as `None`.
    pub const fn is_pipe(&self) -> Option<bool> {
        self.is_pipe
    }
}

/// One runtime-populated physical kernel argument.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HiddenArgument {
    offset: u64,
    size: u64,
    value_kind: HiddenValueKind,
}

impl HiddenArgument {
    /// Returns the byte offset in the kernarg segment.
    pub const fn offset(self) -> u64 {
        self.offset
    }

    /// Returns the physical byte size.
    pub const fn size(self) -> u64 {
        self.size
    }

    /// Returns the runtime-populated AMDHSA value kind.
    pub const fn value_kind(self) -> HiddenValueKind {
        self.value_kind
    }
}

/// Inspected metadata for one kernel entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InspectedKernel {
    pub(crate) name: Box<str>,
    pub(crate) symbol: Box<str>,
    pub(crate) kernarg_segment_size: u64,
    pub(crate) kernarg_segment_alignment: u64,
    pub(crate) group_segment_fixed_size: u64,
    pub(crate) private_segment_fixed_size: u64,
    pub(crate) wavefront_size: u32,
    pub(crate) sgpr_count: u16,
    pub(crate) vgpr_count: u16,
    pub(crate) agpr_count: Option<u32>,
    pub(crate) sgpr_spill_count: Option<u32>,
    pub(crate) vgpr_spill_count: Option<u32>,
    pub(crate) max_flat_workgroup_size: u32,
    pub(crate) required_workgroup_size: Option<[u32; 3]>,
    pub(crate) max_workgroups: [Option<u32>; 3],
    pub(crate) cluster_dims: Option<[u32; 3]>,
    pub(crate) kind: KernelKind,
    pub(crate) kind_was_emitted: bool,
    pub(crate) uniform_work_group_size: Option<bool>,
    pub(crate) uses_dynamic_stack: Option<bool>,
    pub(crate) workgroup_processor_mode: Option<bool>,
    pub(crate) gfx1250_revision: Option<Gfx1250Revision>,
    pub(crate) device_enqueue_symbol: Option<Box<str>>,
    pub(crate) source_language: Option<Box<str>>,
    pub(crate) source_language_version: Option<[u32; 2]>,
    pub(crate) workgroup_size_hint_was_emitted: bool,
    pub(crate) vector_type_hint_was_emitted: bool,
    pub(crate) arguments_were_emitted: bool,
    pub(crate) implicit_argument_offset: Option<u64>,
    pub(crate) implicit_argument_size: u64,
    pub(crate) explicit_arguments: Vec<ExplicitArgument>,
    pub(crate) hidden_arguments: Vec<HiddenArgument>,
}

impl InspectedKernel {
    /// Returns the kernel entry name used for function lookup.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the kernel descriptor symbol declared by the metadata.
    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    /// Returns the complete physical kernarg segment size.
    pub const fn kernarg_segment_size(&self) -> u64 {
        self.kernarg_segment_size
    }

    /// Returns the required kernarg segment alignment.
    pub const fn kernarg_segment_alignment(&self) -> u64 {
        self.kernarg_segment_alignment
    }

    /// Returns fixed workgroup-local memory required per workgroup.
    pub const fn group_segment_fixed_size(&self) -> u64 {
        self.group_segment_fixed_size
    }

    /// Returns fixed private memory required per workitem.
    pub const fn private_segment_fixed_size(&self) -> u64 {
        self.private_segment_fixed_size
    }

    /// Returns the compiled wavefront size.
    pub const fn wavefront_size(&self) -> u32 {
        self.wavefront_size
    }

    /// Returns the required scalar register count.
    pub const fn sgpr_count(&self) -> u16 {
        self.sgpr_count
    }

    /// Returns the required vector register count.
    pub const fn vgpr_count(&self) -> u16 {
        self.vgpr_count
    }

    /// Returns the optional accumulator register count.
    pub const fn agpr_count(&self) -> Option<u32> {
        self.agpr_count
    }

    /// Returns the emitted scalar-register spill count, preserving absence.
    pub const fn sgpr_spill_count(&self) -> Option<u32> {
        self.sgpr_spill_count
    }

    /// Returns the emitted vector-register spill count, preserving absence.
    pub const fn vgpr_spill_count(&self) -> Option<u32> {
        self.vgpr_spill_count
    }

    /// Returns the maximum total workitems allowed in one workgroup.
    pub const fn max_flat_workgroup_size(&self) -> u32 {
        self.max_flat_workgroup_size
    }

    /// Returns the exact required workgroup dimensions, if constrained.
    pub const fn required_workgroup_size(&self) -> Option<[u32; 3]> {
        self.required_workgroup_size
    }

    /// Returns optional per-axis limits on launched workgroup counts.
    pub const fn max_workgroups(&self) -> [Option<u32>; 3] {
        self.max_workgroups
    }

    /// Returns fixed workgroup-cluster dimensions for code object V6.
    pub const fn cluster_dims(&self) -> Option<[u32; 3]> {
        self.cluster_dims
    }

    /// Returns the kernel lifecycle classification.
    pub const fn kind(&self) -> KernelKind {
        self.kind
    }

    /// Returns whether `.kind` was serialized instead of taking the normal default.
    pub const fn kind_was_emitted(&self) -> bool {
        self.kind_was_emitted
    }

    /// Returns whether every workgroup must have uniform dimensions.
    pub const fn uniform_work_group_size(&self) -> bool {
        match self.uniform_work_group_size {
            Some(value) => value,
            None => false,
        }
    }

    /// Returns the serialized uniform-workgroup declaration, preserving absence.
    pub const fn uniform_work_group_size_declaration(&self) -> Option<bool> {
        self.uniform_work_group_size
    }

    /// Returns whether the kernel uses a dynamically sized stack.
    pub const fn uses_dynamic_stack(&self) -> bool {
        match self.uses_dynamic_stack {
            Some(value) => value,
            None => false,
        }
    }

    /// Returns the serialized dynamic-stack declaration, preserving absence.
    pub const fn uses_dynamic_stack_declaration(&self) -> Option<bool> {
        self.uses_dynamic_stack
    }

    /// Returns the optional WGP execution-mode setting.
    pub const fn workgroup_processor_mode(&self) -> Option<bool> {
        self.workgroup_processor_mode
    }

    /// Returns temporary GFX1250-family stepping evidence, if emitted.
    pub const fn gfx1250_revision(&self) -> Option<Gfx1250Revision> {
        self.gfx1250_revision
    }

    /// Returns the optional device-enqueue symbol.
    pub fn device_enqueue_symbol(&self) -> Option<&str> {
        self.device_enqueue_symbol.as_deref()
    }

    /// Returns the serialized source-language declaration, preserving absence.
    pub fn source_language(&self) -> Option<&str> {
        self.source_language.as_deref()
    }

    /// Returns the serialized source-language version, preserving absence.
    pub const fn source_language_version(&self) -> Option<[u32; 2]> {
        self.source_language_version
    }

    /// Returns whether a source workgroup-size hint was serialized.
    pub const fn workgroup_size_hint_was_emitted(&self) -> bool {
        self.workgroup_size_hint_was_emitted
    }

    /// Returns whether a source vector-type hint was serialized.
    pub const fn vector_type_hint_was_emitted(&self) -> bool {
        self.vector_type_hint_was_emitted
    }

    /// Returns whether the kernel argument array was serialized.
    pub const fn arguments_were_emitted(&self) -> bool {
        self.arguments_were_emitted
    }

    /// Returns the start of the compiler-declared implicit argument span.
    pub const fn implicit_argument_offset(&self) -> Option<u64> {
        self.implicit_argument_offset
    }

    /// Returns the complete compiler-declared implicit argument span size.
    pub const fn implicit_argument_size(&self) -> u64 {
        self.implicit_argument_size
    }

    /// Returns caller-provided arguments in physical offset order.
    pub fn explicit_arguments(&self) -> &[ExplicitArgument] {
        &self.explicit_arguments
    }

    /// Returns runtime-populated arguments in physical offset order.
    pub fn hidden_arguments(&self) -> &[HiddenArgument] {
        &self.hidden_arguments
    }
}

/// A bounded description extracted from one HSACO.
///
/// This value carries no module-loading or launch authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InspectedHsaco {
    code_object_version: CodeObjectVersion,
    metadata_version: MetadataVersion,
    metadata_descriptor_range: MetadataDescriptorRange,
    target: AmdTargetId,
    has_printf_metadata: bool,
    kernels: Vec<InspectedKernel>,
}

impl InspectedHsaco {
    /// Returns the AMDGPU HSA code object version.
    pub const fn code_object_version(&self) -> CodeObjectVersion {
        self.code_object_version
    }

    /// Returns the metadata schema version.
    pub const fn metadata_version(&self) -> MetadataVersion {
        self.metadata_version
    }

    /// Returns the exact file range of the decoded metadata descriptor.
    pub const fn metadata_descriptor_range(&self) -> MetadataDescriptorRange {
        self.metadata_descriptor_range
    }

    /// Returns the target ID parsed from the metadata note.
    pub const fn target(&self) -> AmdTargetId {
        self.target
    }

    /// Returns whether the metadata root contains `amdhsa.printf`.
    pub const fn has_printf_metadata(&self) -> bool {
        self.has_printf_metadata
    }

    /// Returns kernels in metadata declaration order.
    pub fn kernels(&self) -> &[InspectedKernel] {
        &self.kernels
    }
}

/// Inspects untrusted HSACO bytes without loading or executing them.
pub fn inspect(bytes: &[u8]) -> Result<InspectedHsaco, InspectionError> {
    let envelope = elf_inspection::inspect_envelope(bytes)?;
    metadata::inspect_metadata(
        envelope.code_object_version,
        envelope.e_flags,
        MetadataDescriptorRange {
            file_offset: envelope.metadata_offset as u64,
            byte_len: envelope.metadata.len() as u64,
        },
        envelope.metadata,
    )
}

/// Inspects one byte slice and explicitly binds every metadata kernel to its
/// static ELF entry and 64-byte AMDHSA descriptor symbols.
///
/// This operation is deliberately separate from [`inspect`]. Its result is
/// descriptive evidence only and cannot load a module or authorize a launch.
pub fn inspect_and_bind_kernel_descriptors(
    bytes: &[u8],
) -> Result<InspectedKernelBindings, KernelBindingError> {
    let inspection = inspect(bytes)?;
    kernel_binding::bind(bytes, inspection)
}

pub(crate) struct ParsedExplicitArgument {
    pub(crate) name: Option<Box<str>>,
    pub(crate) type_name: Option<Box<str>>,
    pub(crate) offset: u64,
    pub(crate) size: u64,
    pub(crate) alignment: Option<u64>,
    pub(crate) value_kind: ExplicitValueKind,
    pub(crate) value_type: Option<ExplicitValueType>,
    pub(crate) address_space: Option<ArgumentAddressSpace>,
    pub(crate) access: Option<ArgumentAccess>,
    pub(crate) actual_access: Option<ArgumentAccess>,
    pub(crate) pointee_alignment: Option<u64>,
    pub(crate) is_const: Option<bool>,
    pub(crate) is_restrict: Option<bool>,
    pub(crate) is_volatile: Option<bool>,
    pub(crate) is_pipe: Option<bool>,
}

impl From<ParsedExplicitArgument> for ExplicitArgument {
    fn from(argument: ParsedExplicitArgument) -> Self {
        Self {
            name: argument.name,
            type_name: argument.type_name,
            offset: argument.offset,
            size: argument.size,
            alignment: argument.alignment,
            value_kind: argument.value_kind,
            value_type: argument.value_type,
            address_space: argument.address_space,
            access: argument.access,
            actual_access: argument.actual_access,
            pointee_alignment: argument.pointee_alignment,
            is_const: argument.is_const,
            is_restrict: argument.is_restrict,
            is_volatile: argument.is_volatile,
            is_pipe: argument.is_pipe,
        }
    }
}

pub(crate) const fn hidden_argument(
    offset: u64,
    size: u64,
    value_kind: HiddenValueKind,
) -> HiddenArgument {
    HiddenArgument {
        offset,
        size,
        value_kind,
    }
}

pub(crate) fn inspected_hsaco(
    code_object_version: CodeObjectVersion,
    metadata_version: MetadataVersion,
    metadata_descriptor_range: MetadataDescriptorRange,
    target: AmdTargetId,
    has_printf_metadata: bool,
    kernels: Vec<InspectedKernel>,
) -> InspectedHsaco {
    InspectedHsaco {
        code_object_version,
        metadata_version,
        metadata_descriptor_range,
        target,
        has_printf_metadata,
        kernels,
    }
}
