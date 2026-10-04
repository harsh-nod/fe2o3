use fe2o3_hsaco::{
    ArgumentAddressSpace, ExplicitArgument, ExplicitValueKind, HiddenArgument, HiddenValueKind,
    InspectedKernel,
};

use crate::{MaterializationError, ValidatedKernelEnvelope};

/// Stable identifier for the exact first-launch vecadd kernarg profile.
pub const VECADD_COV6_KERNARG_PROFILE_ID: &str = "fe2o3.vecadd.gfx942.cov6-kernarg-v1";
/// Complete COV6 kernarg size: 48 explicit bytes plus the 256-byte implicit block.
pub const VECADD_COV6_KERNARG_BYTES: usize = 304;
/// Alignment declared by the exact selected kernel metadata and descriptor.
pub const VECADD_COV6_KERNARG_ALIGNMENT: usize = 8;
/// SHA-256 of the immutable durable vecadd object admitted by this profile.
pub const VECADD_COV6_ARTIFACT_SHA256: [u8; 32] = [
    0xc4, 0x54, 0x7f, 0xe0, 0x45, 0xf8, 0x39, 0x71, 0x1f, 0x1f, 0x02, 0x2a, 0x48, 0x5f, 0x50, 0xc7,
    0xc1, 0xea, 0xfe, 0xd7, 0xf5, 0xe4, 0xa7, 0xe9, 0x65, 0x98, 0xe0, 0xd1, 0xc8, 0x25, 0x90, 0x8c,
];

const EXPLICIT_BYTES: u64 = 48;
const IMPLICIT_BYTES: u64 = 256;
const ELEMENT_BYTES: u64 = 4;
const WORKGROUP_X: u32 = 256;
const IMAGE_START: u64 = 0;
const IMAGE_END: u64 = 0x3000;
const DESCRIPTOR_ADDRESS: u64 = 0x9c0;
const DESCRIPTOR_BYTES: u64 = 64;
const ENTRY_ADDRESS: u64 = 0x1a00;
const ENTRY_BYTES: u64 = 188;
const EXPLICIT_FIELD_OFFSETS: [usize; 6] = [0, 8, 16, 24, 32, 40];
const BLOCK_COUNT_OFFSETS: [usize; 3] = [48, 52, 56];
const GROUP_SIZE_OFFSETS: [usize; 3] = [60, 62, 64];
const GRID_DIMENSIONS_OFFSET: usize = 112;

// This is the executable zero side of the pinned byte manifest. Separate ranges preserve why
// each interval is zero instead of treating the COV6 suffix as anonymous padding:
// remainder, geometry padding, global offsets, grid padding, absent printf, declared service
// pointers, absent dynamic-LDS/reserved, absent private/shared, declared queue, reserved tail.
const ZERO_POLICY_RANGES: [(usize, usize); 10] = [
    (66, 72),
    (72, 88),
    (88, 112),
    (114, 120),
    (120, 128),
    (128, 168),
    (168, 240),
    (240, 248),
    (248, 256),
    (256, 304),
];

#[derive(Clone, Copy)]
struct ExpectedExplicitArgument {
    name: &'static str,
    offset: u64,
    value_kind: ExplicitValueKind,
    address_space: Option<ArgumentAddressSpace>,
}

const EXPLICIT_ARGUMENTS: [ExpectedExplicitArgument; 6] = [
    ExpectedExplicitArgument {
        name: "arg0.data",
        offset: 0,
        value_kind: ExplicitValueKind::GlobalBuffer,
        address_space: Some(ArgumentAddressSpace::Global),
    },
    ExpectedExplicitArgument {
        name: "arg0.len",
        offset: 8,
        value_kind: ExplicitValueKind::ByValue,
        address_space: None,
    },
    ExpectedExplicitArgument {
        name: "arg1.data",
        offset: 16,
        value_kind: ExplicitValueKind::GlobalBuffer,
        address_space: Some(ArgumentAddressSpace::Global),
    },
    ExpectedExplicitArgument {
        name: "arg1.len",
        offset: 24,
        value_kind: ExplicitValueKind::ByValue,
        address_space: None,
    },
    ExpectedExplicitArgument {
        name: "arg2.data",
        offset: 32,
        value_kind: ExplicitValueKind::GlobalBuffer,
        address_space: Some(ArgumentAddressSpace::Global),
    },
    ExpectedExplicitArgument {
        name: "arg2.len",
        offset: 40,
        value_kind: ExplicitValueKind::ByValue,
        address_space: None,
    },
];

const HIDDEN_ARGUMENTS: [(u64, u64, HiddenValueKind); 19] = [
    (48, 4, HiddenValueKind::BlockCountX),
    (52, 4, HiddenValueKind::BlockCountY),
    (56, 4, HiddenValueKind::BlockCountZ),
    (60, 2, HiddenValueKind::GroupSizeX),
    (62, 2, HiddenValueKind::GroupSizeY),
    (64, 2, HiddenValueKind::GroupSizeZ),
    (66, 2, HiddenValueKind::RemainderX),
    (68, 2, HiddenValueKind::RemainderY),
    (70, 2, HiddenValueKind::RemainderZ),
    (88, 8, HiddenValueKind::GlobalOffsetX),
    (96, 8, HiddenValueKind::GlobalOffsetY),
    (104, 8, HiddenValueKind::GlobalOffsetZ),
    (112, 2, HiddenValueKind::GridDimensions),
    (128, 8, HiddenValueKind::HostcallBuffer),
    (136, 8, HiddenValueKind::MultigridSyncArgument),
    (144, 8, HiddenValueKind::HeapV1),
    (152, 8, HiddenValueKind::DefaultQueue),
    (160, 8, HiddenValueKind::CompletionAction),
    (248, 8, HiddenValueKind::QueuePointer),
];

/// Exact checked offsets of the selected descriptor and entry in a materialized image.
///
/// All offsets are relative to the loader's checked link-time image base. This value has no
/// constructor from a native address and intentionally offers no method that resolves an offset
/// against a host or GPU pointer. A later allocation authority must perform that binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LoadedImageAddressPlanV1 {
    link_time_image_base: u64,
    image_byte_len: u64,
    descriptor_offset: u64,
    descriptor_byte_len: u64,
    entry_offset: u64,
    entry_byte_len: u64,
}

impl LoadedImageAddressPlanV1 {
    /// Link-time base from which every exposed offset was checked.
    pub const fn link_time_image_base(self) -> u64 {
        self.link_time_image_base
    }

    /// Exact byte length required by the materialized image.
    pub const fn image_byte_len(self) -> u64 {
        self.image_byte_len
    }

    /// Checked descriptor offset from [`Self::link_time_image_base`].
    pub const fn descriptor_offset(self) -> u64 {
        self.descriptor_offset
    }

    /// Exact selected descriptor length.
    pub const fn descriptor_byte_len(self) -> u64 {
        self.descriptor_byte_len
    }

    /// Checked entry-symbol offset from [`Self::link_time_image_base`].
    pub const fn entry_offset(self) -> u64 {
        self.entry_offset
    }

    /// Exact selected entry-symbol length.
    pub const fn entry_byte_len(self) -> u64 {
        self.entry_byte_len
    }
}

/// An address and element count intended to describe one GPU-resident `f32` slice.
///
/// This is deliberately an unbound numeric description. It proves no allocation ownership,
/// lifetime, accessibility, alias property, or device mapping. Those capabilities must be
/// supplied by a later KFD composition layer before dispatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnboundGpuF32SliceV1 {
    gpu_address: u64,
    element_count: u64,
}

impl UnboundGpuF32SliceV1 {
    /// Creates an unbound numeric slice description. Validation occurs during encoding.
    pub const fn new(gpu_address: u64, element_count: u64) -> Self {
        Self {
            gpu_address,
            element_count,
        }
    }

    /// Numeric GPU virtual address to encode.
    pub const fn gpu_address(self) -> u64 {
        self.gpu_address
    }

    /// Number of `f32` elements visible to the kernel.
    pub const fn element_count(self) -> u64 {
        self.element_count
    }
}

/// Typed explicit inputs for the exact vecadd kernel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VecaddCov6KernargInputsV1 {
    left: UnboundGpuF32SliceV1,
    right: UnboundGpuF32SliceV1,
    output: UnboundGpuF32SliceV1,
}

impl VecaddCov6KernargInputsV1 {
    /// Creates the ordered read/read/write argument tuple.
    pub const fn new(
        left: UnboundGpuF32SliceV1,
        right: UnboundGpuF32SliceV1,
        output: UnboundGpuF32SliceV1,
    ) -> Self {
        Self {
            left,
            right,
            output,
        }
    }

    /// First read-only input description.
    pub const fn left(self) -> UnboundGpuF32SliceV1 {
        self.left
    }

    /// Second read-only input description.
    pub const fn right(self) -> UnboundGpuF32SliceV1 {
        self.right
    }

    /// Write-only output description.
    pub const fn output(self) -> UnboundGpuF32SliceV1 {
        self.output
    }
}

/// One aligned, completely initialized kernarg segment for the pinned vecadd profile.
///
/// The first 48 bytes are the three explicit fat slices. Required COV6 geometry fields are
/// derived from the output length for a padded one-dimensional `256x1x1` launch. Global offsets,
/// partial-workgroup remainders, all compiler-declared optional runtime pointers, absent optional
/// slots, and every ABI-reserved range are zero. No byte is left uninitialized.
#[repr(C, align(8))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VecaddCov6KernargV1 {
    bytes: [u8; VECADD_COV6_KERNARG_BYTES],
}

impl VecaddCov6KernargV1 {
    /// Exact bytes to copy into a separately authorized kernarg allocation.
    pub const fn as_bytes(&self) -> &[u8; VECADD_COV6_KERNARG_BYTES] {
        &self.bytes
    }

    /// Non-partial workgroup counts encoded in the required hidden fields.
    pub fn block_counts(&self) -> [u32; 3] {
        [
            read_u32(&self.bytes, 48),
            read_u32(&self.bytes, 52),
            read_u32(&self.bytes, 56),
        ]
    }

    /// AQL work-item grid dimensions that the encoded hidden fields require.
    pub fn aql_grid_size(&self) -> [u32; 3] {
        [self.block_counts()[0] * WORKGROUP_X, 1, 1]
    }

    /// Exact AQL workgroup dimensions required by the selected metadata.
    pub const fn aql_workgroup_size(&self) -> [u16; 3] {
        [WORKGROUP_X as u16, 1, 1]
    }

    /// Dynamic group-segment bytes contracted by this profile.
    pub const fn dynamic_group_segment_bytes(&self) -> u32 {
        0
    }
}

/// Failure to bind a selected kernel closure to the exact vecadd artifact and ABI manifest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VecaddCov6BindError {
    /// The kernel did not occupy the pinned metadata index.
    SelectedKernelIndex { actual: usize },
    /// The selected kernel name drifted.
    KernelName,
    /// The selected descriptor symbol drifted.
    KernelSymbol,
    /// The complete kernarg size drifted.
    KernargSize { actual: u64 },
    /// The complete kernarg alignment drifted.
    KernargAlignment { actual: u64 },
    /// The explicit/implicit boundary drifted.
    ImplicitOffset { actual: Option<u64> },
    /// The COV6 implicit block size drifted.
    ImplicitSize { actual: u64 },
    /// The exact required workgroup dimensions drifted.
    RequiredWorkgroupSize { actual: Option<[u32; 3]> },
    /// The selected kernel unexpectedly requires static group or private memory.
    StaticResourceSize,
    /// The explicit argument count drifted.
    ExplicitArgumentCount { actual: usize },
    /// One exact explicit argument field drifted.
    ExplicitArgument { index: usize, field: &'static str },
    /// The hidden argument count drifted.
    HiddenArgumentCount { actual: usize },
    /// One hidden argument offset, size, or kind drifted.
    HiddenArgument { index: usize },
    /// The selected descriptor location or descriptor kernarg fact drifted.
    DescriptorLocation,
    /// The selected entry location or extent drifted.
    EntryLocation,
    /// The checked image base or span drifted.
    ImageRange,
    /// The exact durable artifact digest drifted.
    ArtifactIdentity { actual: [u8; 32] },
}

/// Failure to encode numeric slice descriptions into the exact vecadd ABI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VecaddCov6KernargError {
    /// A zero-work dispatch is not admitted by the first-launch profile.
    EmptyOutput,
    /// An input is shorter than the output and would reach the kernel's unreachable path.
    OutputExceedsInput { input: &'static str },
    /// A nonempty slice used the null numeric address.
    ZeroGpuAddress { argument: &'static str },
    /// An `f32` slice address was not four-byte aligned.
    MisalignedGpuAddress {
        argument: &'static str,
        address: u64,
    },
    /// Element count to byte count conversion overflowed.
    SliceByteLengthOverflow { argument: &'static str },
    /// Numeric address plus slice byte length overflowed.
    SliceAddressRangeOverflow { argument: &'static str },
    /// The padded one-dimensional work-item grid cannot fit an AQL `u32` field.
    AqlGridSizeOverflow,
    /// The private pinned writer violated an executable zero-policy range.
    InternalZeroPolicyViolation,
}

/// Exact artifact-specific semantic closure needed to construct vecadd kernargs.
///
/// This type has no public constructor. It retains the original selected-kernel closure, but it
/// still grants neither native address authority nor launch authority.
pub struct ValidatedVecaddCov6Kernel<'a> {
    closure: ValidatedKernelEnvelope<'a>,
    address_plan: LoadedImageAddressPlanV1,
}

impl<'a> ValidatedVecaddCov6Kernel<'a> {
    /// Returns the exact underlying selected-kernel closure.
    pub const fn closure(&self) -> &ValidatedKernelEnvelope<'a> {
        &self.closure
    }

    /// Returns the checked image-relative selected descriptor and entry plan.
    pub const fn address_plan(&self) -> LoadedImageAddressPlanV1 {
        self.address_plan
    }

    /// Materializes the retained object into the exact CPU-side image.
    pub fn materialize_into(&self, destination: &mut [u8]) -> Result<(), MaterializationError> {
        self.closure.materialize_into(destination)
    }

    /// Encodes the complete, aligned 304-byte COV6 kernarg after bounded shape/address checks.
    pub fn encode_kernarg(
        &self,
        inputs: VecaddCov6KernargInputsV1,
    ) -> Result<VecaddCov6KernargV1, VecaddCov6KernargError> {
        encode_kernarg(inputs)
    }

    /// Recovers the retained general selected-kernel closure.
    pub fn into_closure(self) -> ValidatedKernelEnvelope<'a> {
        self.closure
    }
}

impl<'a> ValidatedKernelEnvelope<'a> {
    /// Consumes a selected closure and binds it to the exact immutable vecadd artifact and ABI.
    pub fn bind_vecadd_cov6(self) -> Result<ValidatedVecaddCov6Kernel<'a>, VecaddCov6BindError> {
        validate_kernel_manifest(&self)?;
        let address_plan = validate_address_plan(&self)?;
        let actual = self.identity_inputs().object_sha256();
        if actual != VECADD_COV6_ARTIFACT_SHA256 {
            return Err(VecaddCov6BindError::ArtifactIdentity { actual });
        }
        Ok(ValidatedVecaddCov6Kernel {
            closure: self,
            address_plan,
        })
    }
}

fn validate_kernel_manifest(
    closure: &ValidatedKernelEnvelope<'_>,
) -> Result<(), VecaddCov6BindError> {
    if closure.selected_kernel_index() != 0 {
        return Err(VecaddCov6BindError::SelectedKernelIndex {
            actual: closure.selected_kernel_index(),
        });
    }
    let kernel = closure.selected_kernel();
    if kernel.name() != "vecadd" {
        return Err(VecaddCov6BindError::KernelName);
    }
    if kernel.symbol() != "vecadd.kd" {
        return Err(VecaddCov6BindError::KernelSymbol);
    }
    if kernel.kernarg_segment_size() != VECADD_COV6_KERNARG_BYTES as u64 {
        return Err(VecaddCov6BindError::KernargSize {
            actual: kernel.kernarg_segment_size(),
        });
    }
    if kernel.kernarg_segment_alignment() != VECADD_COV6_KERNARG_ALIGNMENT as u64 {
        return Err(VecaddCov6BindError::KernargAlignment {
            actual: kernel.kernarg_segment_alignment(),
        });
    }
    if kernel.implicit_argument_offset() != Some(EXPLICIT_BYTES) {
        return Err(VecaddCov6BindError::ImplicitOffset {
            actual: kernel.implicit_argument_offset(),
        });
    }
    if kernel.implicit_argument_size() != IMPLICIT_BYTES {
        return Err(VecaddCov6BindError::ImplicitSize {
            actual: kernel.implicit_argument_size(),
        });
    }
    if kernel.required_workgroup_size() != Some([WORKGROUP_X, 1, 1]) {
        return Err(VecaddCov6BindError::RequiredWorkgroupSize {
            actual: kernel.required_workgroup_size(),
        });
    }
    if kernel.group_segment_fixed_size() != 0 || kernel.private_segment_fixed_size() != 0 {
        return Err(VecaddCov6BindError::StaticResourceSize);
    }
    validate_explicit_arguments(kernel)?;
    validate_hidden_arguments(kernel)
}

fn validate_explicit_arguments(kernel: &InspectedKernel) -> Result<(), VecaddCov6BindError> {
    let actual = kernel.explicit_arguments();
    if actual.len() != EXPLICIT_ARGUMENTS.len() {
        return Err(VecaddCov6BindError::ExplicitArgumentCount {
            actual: actual.len(),
        });
    }
    for (index, (argument, expected)) in actual.iter().zip(EXPLICIT_ARGUMENTS).enumerate() {
        validate_explicit_argument(index, argument, expected)?;
    }
    Ok(())
}

fn validate_explicit_argument(
    index: usize,
    argument: &ExplicitArgument,
    expected: ExpectedExplicitArgument,
) -> Result<(), VecaddCov6BindError> {
    let mismatch = |field| VecaddCov6BindError::ExplicitArgument { index, field };
    if argument.name() != Some(expected.name) {
        return Err(mismatch("name"));
    }
    if argument.offset() != expected.offset {
        return Err(mismatch("offset"));
    }
    if argument.size() != 8 {
        return Err(mismatch("size"));
    }
    if argument.value_kind() != expected.value_kind {
        return Err(mismatch("value_kind"));
    }
    if argument.address_space() != expected.address_space {
        return Err(mismatch("address_space"));
    }
    if argument.type_name().is_some()
        || argument.alignment().is_some()
        || argument.value_type().is_some()
        || argument.access().is_some()
        || argument.actual_access().is_some()
        || argument.pointee_alignment().is_some()
        || argument.is_const().is_some()
        || argument.is_restrict().is_some()
        || argument.is_volatile().is_some()
        || argument.is_pipe().is_some()
    {
        return Err(mismatch("unexpected optional qualifier"));
    }
    Ok(())
}

fn validate_hidden_arguments(kernel: &InspectedKernel) -> Result<(), VecaddCov6BindError> {
    let actual = kernel.hidden_arguments();
    if actual.len() != HIDDEN_ARGUMENTS.len() {
        return Err(VecaddCov6BindError::HiddenArgumentCount {
            actual: actual.len(),
        });
    }
    for (index, (argument, expected)) in actual.iter().copied().zip(HIDDEN_ARGUMENTS).enumerate() {
        if !hidden_argument_matches(argument, expected) {
            return Err(VecaddCov6BindError::HiddenArgument { index });
        }
    }
    Ok(())
}

fn hidden_argument_matches(
    argument: HiddenArgument,
    expected: (u64, u64, HiddenValueKind),
) -> bool {
    argument.offset() == expected.0
        && argument.size() == expected.1
        && argument.value_kind() == expected.2
}

fn validate_address_plan(
    closure: &ValidatedKernelEnvelope<'_>,
) -> Result<LoadedImageAddressPlanV1, VecaddCov6BindError> {
    let plan = closure.envelope().plan();
    let binding = closure.selected_binding();
    if binding.descriptor_address() != DESCRIPTOR_ADDRESS
        || binding.descriptor_file_offset() != DESCRIPTOR_ADDRESS
        || binding.descriptor().kernarg_size() != VECADD_COV6_KERNARG_BYTES as u32
    {
        return Err(VecaddCov6BindError::DescriptorLocation);
    }
    if binding.entry_address() != ENTRY_ADDRESS
        || binding.entry_file_offset() != 0xa00
        || binding.entry_size() != ENTRY_BYTES
    {
        return Err(VecaddCov6BindError::EntryLocation);
    }
    if plan.image_start() != IMAGE_START
        || plan.image_end() != IMAGE_END
        || closure.envelope().materialization().image_len() != IMAGE_END - IMAGE_START
    {
        return Err(VecaddCov6BindError::ImageRange);
    }
    let descriptor_offset = checked_image_offset(
        plan.image_start(),
        plan.image_end(),
        binding.descriptor_address(),
        DESCRIPTOR_BYTES,
    )
    .ok_or(VecaddCov6BindError::ImageRange)?;
    let entry_offset = checked_image_offset(
        plan.image_start(),
        plan.image_end(),
        binding.entry_address(),
        binding.entry_size(),
    )
    .ok_or(VecaddCov6BindError::ImageRange)?;
    Ok(LoadedImageAddressPlanV1 {
        link_time_image_base: plan.image_start(),
        image_byte_len: plan.image_end() - plan.image_start(),
        descriptor_offset,
        descriptor_byte_len: DESCRIPTOR_BYTES,
        entry_offset,
        entry_byte_len: binding.entry_size(),
    })
}

fn checked_image_offset(base: u64, end: u64, address: u64, byte_len: u64) -> Option<u64> {
    let offset = address.checked_sub(base)?;
    let range_end = address.checked_add(byte_len)?;
    (range_end <= end).then_some(offset)
}

fn encode_kernarg(
    inputs: VecaddCov6KernargInputsV1,
) -> Result<VecaddCov6KernargV1, VecaddCov6KernargError> {
    validate_slice("left", inputs.left)?;
    validate_slice("right", inputs.right)?;
    validate_slice("output", inputs.output)?;
    if inputs.output.element_count == 0 {
        return Err(VecaddCov6KernargError::EmptyOutput);
    }
    if inputs.output.element_count > inputs.left.element_count {
        return Err(VecaddCov6KernargError::OutputExceedsInput { input: "left" });
    }
    if inputs.output.element_count > inputs.right.element_count {
        return Err(VecaddCov6KernargError::OutputExceedsInput { input: "right" });
    }
    let block_count = inputs
        .output
        .element_count
        .checked_add(u64::from(WORKGROUP_X) - 1)
        .map(|value| value / u64::from(WORKGROUP_X))
        .and_then(|value| u32::try_from(value).ok())
        .ok_or(VecaddCov6KernargError::AqlGridSizeOverflow)?;
    block_count
        .checked_mul(WORKGROUP_X)
        .ok_or(VecaddCov6KernargError::AqlGridSizeOverflow)?;

    // Zero initialization is the manifest policy for every reserved byte, absent optional COV6
    // slot, global offset, partial-group remainder, and declared runtime pointer. Only the exact
    // explicit fields and required one-dimensional geometry below may become nonzero.
    let mut bytes = [0u8; VECADD_COV6_KERNARG_BYTES];
    for (offset, value) in EXPLICIT_FIELD_OFFSETS.into_iter().zip([
        inputs.left.gpu_address,
        inputs.left.element_count,
        inputs.right.gpu_address,
        inputs.right.element_count,
        inputs.output.gpu_address,
        inputs.output.element_count,
    ]) {
        put_u64(&mut bytes, offset, value);
    }
    for (offset, value) in BLOCK_COUNT_OFFSETS.into_iter().zip([block_count, 1, 1]) {
        put_u32(&mut bytes, offset, value);
    }
    for (offset, value) in GROUP_SIZE_OFFSETS
        .into_iter()
        .zip([WORKGROUP_X as u16, 1, 1])
    {
        put_u16(&mut bytes, offset, value);
    }
    put_u16(&mut bytes, GRID_DIMENSIONS_OFFSET, 1);
    if !zero_policy_holds(&bytes) {
        return Err(VecaddCov6KernargError::InternalZeroPolicyViolation);
    }
    Ok(VecaddCov6KernargV1 { bytes })
}

fn zero_policy_holds(bytes: &[u8; VECADD_COV6_KERNARG_BYTES]) -> bool {
    ZERO_POLICY_RANGES.iter().all(|(start, end)| {
        bytes
            .get(*start..*end)
            .is_some_and(|range| range.iter().all(|byte| *byte == 0))
    })
}

fn validate_slice(
    argument: &'static str,
    slice: UnboundGpuF32SliceV1,
) -> Result<(), VecaddCov6KernargError> {
    if slice.gpu_address == 0 {
        return Err(VecaddCov6KernargError::ZeroGpuAddress { argument });
    }
    if !slice.gpu_address.is_multiple_of(ELEMENT_BYTES) {
        return Err(VecaddCov6KernargError::MisalignedGpuAddress {
            argument,
            address: slice.gpu_address,
        });
    }
    let byte_len = slice
        .element_count
        .checked_mul(ELEMENT_BYTES)
        .ok_or(VecaddCov6KernargError::SliceByteLengthOverflow { argument })?;
    slice
        .gpu_address
        .checked_add(byte_len)
        .ok_or(VecaddCov6KernargError::SliceAddressRangeOverflow { argument })?;
    Ok(())
}

fn put_u16(bytes: &mut [u8; VECADD_COV6_KERNARG_BYTES], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(bytes: &mut [u8; VECADD_COV6_KERNARG_BYTES], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_u64(bytes: &mut [u8; VECADD_COV6_KERNARG_BYTES], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn read_u32(bytes: &[u8; VECADD_COV6_KERNARG_BYTES], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("fixed COV6 field"),
    )
}

#[cfg(test)]
mod tests {
    use core::mem::{align_of, size_of};

    use super::*;

    #[derive(Debug, Eq, PartialEq)]
    struct OracleKernarg {
        slices: [(u64, u64); 3],
        block_count: [u32; 3],
        group_size: [u16; 3],
        remainder: [u16; 3],
        global_offset: [u64; 3],
        grid_dimensions: u16,
        runtime_pointers: [u64; 6],
    }

    #[test]
    fn exact_layout_encodes_and_independent_oracle_decodes_every_field() {
        let encoded = encode_kernarg(inputs(1025, 1025, 1025)).unwrap();
        assert_eq!(size_of::<VecaddCov6KernargV1>(), 304);
        assert_eq!(align_of::<VecaddCov6KernargV1>(), 8);
        assert_eq!(encoded.block_counts(), [5, 1, 1]);
        assert_eq!(encoded.aql_grid_size(), [1280, 1, 1]);
        assert_eq!(encoded.aql_workgroup_size(), [256, 1, 1]);
        assert_eq!(encoded.dynamic_group_segment_bytes(), 0);
        assert_eq!(
            oracle_decode(encoded.as_bytes()),
            OracleKernarg {
                slices: [(0x1000, 1025), (0x4000, 1025), (0x8000, 1025)],
                block_count: [5, 1, 1],
                group_size: [256, 1, 1],
                remainder: [0, 0, 0],
                global_offset: [0, 0, 0],
                grid_dimensions: 1,
                runtime_pointers: [0; 6],
            }
        );
        // 72..88 is COV6 geometry padding, 114..128 is grid padding plus the absent printf slot,
        // 168..248 is absent dynamic-LDS/private/shared slots plus reserved ABI bytes, and
        // 256..304 is the reserved tail of the exact 256-byte implicit block.
        for (start, end) in ZERO_POLICY_RANGES {
            assert!(encoded.as_bytes()[start..end].iter().all(|byte| *byte == 0));
        }
    }

    #[test]
    fn hostile_shapes_addresses_and_overflow_fail_closed() {
        assert_eq!(
            encode_kernarg(inputs(1, 1, 0)),
            Err(VecaddCov6KernargError::EmptyOutput)
        );
        assert_eq!(
            encode_kernarg(inputs(7, 8, 8)),
            Err(VecaddCov6KernargError::OutputExceedsInput { input: "left" })
        );
        let mut hostile = inputs(1, 1, 1);
        hostile.left.gpu_address = 0;
        assert_eq!(
            encode_kernarg(hostile),
            Err(VecaddCov6KernargError::ZeroGpuAddress { argument: "left" })
        );
        let mut hostile = inputs(1, 1, 1);
        hostile.output.gpu_address = 0x8002;
        assert_eq!(
            encode_kernarg(hostile),
            Err(VecaddCov6KernargError::MisalignedGpuAddress {
                argument: "output",
                address: 0x8002,
            })
        );
        let mut hostile = inputs(1, 1, 1);
        hostile.right.element_count = u64::MAX;
        assert_eq!(
            encode_kernarg(hostile),
            Err(VecaddCov6KernargError::SliceByteLengthOverflow { argument: "right" })
        );
        let mut hostile = inputs(1, 1, 1);
        hostile.output.gpu_address = u64::MAX - 3;
        assert_eq!(
            encode_kernarg(hostile),
            Err(VecaddCov6KernargError::SliceAddressRangeOverflow { argument: "output" })
        );
        let too_many_workitems = u64::from(u32::MAX / WORKGROUP_X) * u64::from(WORKGROUP_X) + 1;
        assert_eq!(
            encode_kernarg(inputs(
                too_many_workitems,
                too_many_workitems,
                too_many_workitems,
            )),
            Err(VecaddCov6KernargError::AqlGridSizeOverflow)
        );
    }

    fn inputs(left: u64, right: u64, output: u64) -> VecaddCov6KernargInputsV1 {
        VecaddCov6KernargInputsV1::new(
            UnboundGpuF32SliceV1::new(0x1000, left),
            UnboundGpuF32SliceV1::new(0x4000, right),
            UnboundGpuF32SliceV1::new(0x8000, output),
        )
    }

    // Deliberately independent of the production writer and its manifest arrays.
    fn oracle_decode(bytes: &[u8; 304]) -> OracleKernarg {
        OracleKernarg {
            slices: [
                (oracle_u64(bytes, 0), oracle_u64(bytes, 8)),
                (oracle_u64(bytes, 16), oracle_u64(bytes, 24)),
                (oracle_u64(bytes, 32), oracle_u64(bytes, 40)),
            ],
            block_count: [
                oracle_u32(bytes, 48),
                oracle_u32(bytes, 52),
                oracle_u32(bytes, 56),
            ],
            group_size: [
                oracle_u16(bytes, 60),
                oracle_u16(bytes, 62),
                oracle_u16(bytes, 64),
            ],
            remainder: [
                oracle_u16(bytes, 66),
                oracle_u16(bytes, 68),
                oracle_u16(bytes, 70),
            ],
            global_offset: [
                oracle_u64(bytes, 88),
                oracle_u64(bytes, 96),
                oracle_u64(bytes, 104),
            ],
            grid_dimensions: oracle_u16(bytes, 112),
            runtime_pointers: [
                oracle_u64(bytes, 128),
                oracle_u64(bytes, 136),
                oracle_u64(bytes, 144),
                oracle_u64(bytes, 152),
                oracle_u64(bytes, 160),
                oracle_u64(bytes, 248),
            ],
        }
    }

    fn oracle_u16(bytes: &[u8], offset: usize) -> u16 {
        u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
    }

    fn oracle_u32(bytes: &[u8], offset: usize) -> u32 {
        u32::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ])
    }

    fn oracle_u64(bytes: &[u8], offset: usize) -> u64 {
        u64::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
            bytes[offset + 4],
            bytes[offset + 5],
            bytes[offset + 6],
            bytes[offset + 7],
        ])
    }
}
