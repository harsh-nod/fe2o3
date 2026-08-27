use std::{collections::BTreeSet, fmt};

use fe2o3_amd_target::AmdTargetId;

use crate::{CanonicalCodeObjectDigest, DeviceLayoutIdentity, RustTypeIdentity, ValidationError};

pub const MAX_DESCRIPTOR_TABLE_BYTES: usize = 256 * 1024;
pub const MAX_KERNELS: usize = 128;
pub const MAX_ARGUMENTS_PER_KERNEL: usize = 64;
pub const MAX_PHYSICAL_COMPONENTS_PER_KERNEL: usize = 128;
pub const MAX_TYPE_RECORDS: usize = 256;
pub const MAX_LAYOUT_RECORDS: usize = 256;
pub const MAX_NAME_BYTES: usize = 128;
pub const MAX_TEXT_BYTES: usize = 256;
/// Stack-wide upper bound for one kernel's complete kernarg segment.
pub const MAX_KERNARG_SEGMENT_BYTES: u32 = 1 << 20;
pub(crate) const MAX_CAPABILITIES: usize = 64;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ValidName(String);

impl ValidName {
    pub fn new(value: impl Into<String>) -> Result<Self, ValidationError> {
        let value = value.into();
        validate_name(&value, "name")?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Text(String);

impl Text {
    pub fn new(value: impl Into<String>) -> Result<Self, ValidationError> {
        let value = value.into();
        validate_text(&value, "text")?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A canonical, concrete AMD target declaration carried by a descriptor table.
///
/// This value is parsed and canonically spelled, but remains untrusted artifact
/// data. It does not attest the loaded code object or the observed device.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DeviceTargetV1(AmdTargetId);

impl DeviceTargetV1 {
    pub const fn new(target: AmdTargetId) -> Self {
        Self(target)
    }

    /// Parses only the canonical textual spelling of a concrete AMD target ID.
    pub fn parse(value: &str) -> Result<Self, ValidationError> {
        let target = AmdTargetId::parse(value).map_err(|_| ValidationError::InvalidValue {
            field: "device target",
        })?;
        if target.to_string() != value {
            return Err(ValidationError::NonCanonicalOrder {
                field: "device target features",
            });
        }
        Ok(Self(target))
    }

    pub const fn as_amd_target_id(self) -> AmdTargetId {
        self.0
    }
}

impl fmt::Display for DeviceTargetV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct KernelId([u8; 32]);

impl KernelId {
    /// Constructs an opaque selector assigned by the manifest or macro
    /// pipeline. These bytes are the logical identity; no hash preimage is
    /// implied. Construction grants no authority; only a later exact trusted
    /// binding can do so.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct EvidenceIdentity([u8; 32]);

impl EvidenceIdentity {
    pub const fn from_opaque_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct EvidenceDigest([u8; 32]);

impl EvidenceDigest {
    pub const fn from_sha256_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BuildEvidenceV1 {
    pub(crate) identity: EvidenceIdentity,
    pub(crate) digest: EvidenceDigest,
}

impl BuildEvidenceV1 {
    /// Constructs producer-namespaced opaque build evidence. V1 defines no
    /// verifiable preimage; construction does not verify its producer,
    /// identity, content, digest, or Verus authority.
    pub const fn new(identity: EvidenceIdentity, digest: EvidenceDigest) -> Self {
        Self { identity, digest }
    }

    pub const fn identity(self) -> EvidenceIdentity {
        self.identity
    }

    pub const fn digest(self) -> EvidenceDigest {
        self.digest
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerIdentityV1 {
    pub(crate) name: Text,
    pub(crate) release: Text,
    pub(crate) commit: [u8; 20],
}

impl CompilerIdentityV1 {
    pub const fn new(name: Text, release: Text, commit: [u8; 20]) -> Self {
        Self {
            name,
            release,
            commit,
        }
    }

    pub const fn name(&self) -> &Text {
        &self.name
    }

    pub const fn release(&self) -> &Text {
        &self.release
    }

    pub const fn commit(&self) -> &[u8; 20] {
        &self.commit
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProducerIdentityV1 {
    pub(crate) name: Text,
    pub(crate) version: Text,
}

impl ProducerIdentityV1 {
    pub const fn new(name: Text, version: Text) -> Self {
        Self { name, version }
    }

    pub const fn name(&self) -> &Text {
        &self.name
    }

    pub const fn version(&self) -> &Text {
        &self.version
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CodeObjectVersion {
    V4,
    V5,
    V6,
}

impl CodeObjectVersion {
    /// Returns the numeric AMDGPU HSA code-object version.
    pub const fn number(self) -> u8 {
        match self {
            Self::V4 => 4,
            Self::V5 => 5,
            Self::V6 => 6,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ScalarTypeV1 {
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    I64,
    U64,
    F16,
    F32,
    F64,
}

impl ScalarTypeV1 {
    pub const fn size_bytes(self) -> u16 {
        match self {
            Self::I8 | Self::U8 => 1,
            Self::I16 | Self::U16 | Self::F16 => 2,
            Self::I32 | Self::U32 | Self::F32 => 4,
            Self::I64 | Self::U64 | Self::F64 => 8,
        }
    }

    pub const fn alignment_bytes(self) -> u16 {
        self.size_bytes()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DescriptorKind {
    Scalar,
    SharedSlice,
    DisjointSlice,
    GlobalMutPointer,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceTypeDescriptorV1 {
    pub(crate) kind: DescriptorKind,
    pub(crate) element: ScalarTypeV1,
}

impl SourceTypeDescriptorV1 {
    pub const fn scalar(scalar: ScalarTypeV1) -> Self {
        Self {
            kind: DescriptorKind::Scalar,
            element: scalar,
        }
    }

    pub const fn shared_slice(element: ScalarTypeV1) -> Self {
        Self {
            kind: DescriptorKind::SharedSlice,
            element,
        }
    }

    pub const fn disjoint_slice(element: ScalarTypeV1) -> Self {
        Self {
            kind: DescriptorKind::DisjointSlice,
            element,
        }
    }

    pub const fn global_mut_pointer(pointee: ScalarTypeV1) -> Self {
        Self {
            kind: DescriptorKind::GlobalMutPointer,
            element: pointee,
        }
    }

    pub const fn scalar_type(&self) -> ScalarTypeV1 {
        self.element
    }

    pub const fn is_scalar(&self) -> bool {
        matches!(self.kind, DescriptorKind::Scalar)
    }

    pub const fn is_shared_slice(&self) -> bool {
        matches!(self.kind, DescriptorKind::SharedSlice)
    }

    pub const fn is_disjoint_slice(&self) -> bool {
        matches!(self.kind, DescriptorKind::DisjointSlice)
    }

    pub const fn is_global_mut_pointer(&self) -> bool {
        matches!(self.kind, DescriptorKind::GlobalMutPointer)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceLayoutDescriptorV1 {
    pub(crate) kind: DescriptorKind,
    pub(crate) element: ScalarTypeV1,
    pub(crate) size: u16,
    pub(crate) alignment: u16,
    pub(crate) pointer_width: u8,
    pub(crate) length_width: u8,
}

impl DeviceLayoutDescriptorV1 {
    pub const fn scalar(scalar: ScalarTypeV1) -> Self {
        Self {
            kind: DescriptorKind::Scalar,
            element: scalar,
            size: scalar.size_bytes(),
            alignment: scalar.alignment_bytes(),
            pointer_width: 0,
            length_width: 0,
        }
    }

    pub const fn shared_slice(element: ScalarTypeV1) -> Self {
        Self::slice(DescriptorKind::SharedSlice, element)
    }

    pub const fn disjoint_slice(element: ScalarTypeV1) -> Self {
        Self::slice(DescriptorKind::DisjointSlice, element)
    }

    pub const fn global_mut_pointer(pointee: ScalarTypeV1) -> Self {
        Self {
            kind: DescriptorKind::GlobalMutPointer,
            element: pointee,
            size: 8,
            alignment: 8,
            pointer_width: 8,
            length_width: 0,
        }
    }

    const fn slice(kind: DescriptorKind, element: ScalarTypeV1) -> Self {
        Self {
            kind,
            element,
            size: 16,
            alignment: 8,
            pointer_width: 8,
            length_width: 8,
        }
    }

    pub const fn scalar_type(&self) -> ScalarTypeV1 {
        self.element
    }

    pub const fn size_bytes(&self) -> u16 {
        self.size
    }

    pub const fn alignment_bytes(&self) -> u16 {
        self.alignment
    }

    pub const fn pointer_width_bytes(&self) -> u8 {
        self.pointer_width
    }

    pub const fn length_width_bytes(&self) -> u8 {
        self.length_width
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceTypeRecordV1 {
    pub(crate) identity: RustTypeIdentity,
    pub(crate) descriptor: SourceTypeDescriptorV1,
}

impl SourceTypeRecordV1 {
    pub fn new(descriptor: SourceTypeDescriptorV1) -> Self {
        let identity = RustTypeIdentity::for_descriptor(&descriptor);
        Self {
            identity,
            descriptor,
        }
    }

    pub const fn identity(&self) -> RustTypeIdentity {
        self.identity
    }

    pub const fn descriptor(&self) -> &SourceTypeDescriptorV1 {
        &self.descriptor
    }

    pub(crate) fn from_wire(
        identity: RustTypeIdentity,
        descriptor: SourceTypeDescriptorV1,
    ) -> Result<Self, ValidationError> {
        if identity != RustTypeIdentity::for_descriptor(&descriptor) {
            return Err(ValidationError::IdentityMismatch { field: "Rust type" });
        }
        Ok(Self {
            identity,
            descriptor,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceLayoutRecordV1 {
    pub(crate) identity: DeviceLayoutIdentity,
    pub(crate) descriptor: DeviceLayoutDescriptorV1,
}

impl DeviceLayoutRecordV1 {
    pub fn new(descriptor: DeviceLayoutDescriptorV1) -> Self {
        let identity = DeviceLayoutIdentity::for_descriptor(&descriptor);
        Self {
            identity,
            descriptor,
        }
    }

    pub const fn identity(&self) -> DeviceLayoutIdentity {
        self.identity
    }

    pub const fn descriptor(&self) -> &DeviceLayoutDescriptorV1 {
        &self.descriptor
    }

    pub(crate) fn from_wire(
        identity: DeviceLayoutIdentity,
        descriptor: DeviceLayoutDescriptorV1,
    ) -> Result<Self, ValidationError> {
        if identity != DeviceLayoutIdentity::for_descriptor(&descriptor) {
            return Err(ValidationError::IdentityMismatch {
                field: "device layout",
            });
        }
        Ok(Self {
            identity,
            descriptor,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum OwnershipSemantics {
    ByValue,
    SharedBorrow,
    UniqueBorrow,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AccessMode {
    ByValue,
    ReadOnly,
    WriteOnly,
    ReadWrite,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AliasSemantics {
    Value,
    SharedReadOnly,
    Exclusive,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhysicalAbiComponentKind {
    ScalarByValue(ScalarTypeV1),
    GlobalPointer,
    SliceLengthU64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PhysicalAbiComponentV1 {
    pub(crate) kind: PhysicalAbiComponentKind,
    pub(crate) offset: u32,
    pub(crate) size: u16,
    pub(crate) alignment: u16,
    pub(crate) access: AccessMode,
    pub(crate) alias: AliasSemantics,
}

impl PhysicalAbiComponentV1 {
    fn scalar(offset: u32, scalar: ScalarTypeV1) -> Self {
        Self {
            kind: PhysicalAbiComponentKind::ScalarByValue(scalar),
            offset,
            size: scalar.size_bytes(),
            alignment: scalar.alignment_bytes(),
            access: AccessMode::ByValue,
            alias: AliasSemantics::Value,
        }
    }

    fn global_pointer(offset: u32, access: AccessMode, alias: AliasSemantics) -> Self {
        Self {
            kind: PhysicalAbiComponentKind::GlobalPointer,
            offset,
            size: 8,
            alignment: 8,
            access,
            alias,
        }
    }

    fn slice_length(offset: u32) -> Self {
        Self {
            kind: PhysicalAbiComponentKind::SliceLengthU64,
            offset,
            size: 8,
            alignment: 8,
            access: AccessMode::ByValue,
            alias: AliasSemantics::Value,
        }
    }

    pub(crate) fn end(&self) -> Result<u64, ValidationError> {
        u64::from(self.offset)
            .checked_add(u64::from(self.size))
            .ok_or(ValidationError::Overflow {
                field: "physical ABI component end",
            })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogicalArgumentV1 {
    pub(crate) source_index: u16,
    pub(crate) name: ValidName,
    pub(crate) source_type: RustTypeIdentity,
    pub(crate) device_layout: DeviceLayoutIdentity,
    pub(crate) ownership: OwnershipSemantics,
    pub(crate) access: AccessMode,
    pub(crate) alias: AliasSemantics,
    pub(crate) components: Vec<PhysicalAbiComponentV1>,
}

impl LogicalArgumentV1 {
    pub fn scalar(
        source_index: u16,
        name: ValidName,
        source_type: &SourceTypeRecordV1,
        device_layout: &DeviceLayoutRecordV1,
        offset: u32,
    ) -> Result<Self, ValidationError> {
        require_matching_descriptors(source_type, device_layout, DescriptorKind::Scalar)?;
        let scalar = source_type.descriptor.element;
        let value = Self {
            source_index,
            name,
            source_type: source_type.identity,
            device_layout: device_layout.identity,
            ownership: OwnershipSemantics::ByValue,
            access: AccessMode::ByValue,
            alias: AliasSemantics::Value,
            components: vec![PhysicalAbiComponentV1::scalar(offset, scalar)],
        };
        value.validate_local()?;
        Ok(value)
    }

    pub fn shared_slice(
        source_index: u16,
        name: ValidName,
        source_type: &SourceTypeRecordV1,
        device_layout: &DeviceLayoutRecordV1,
        pointer_offset: u32,
    ) -> Result<Self, ValidationError> {
        require_matching_descriptors(source_type, device_layout, DescriptorKind::SharedSlice)?;
        Self::slice(
            source_index,
            name,
            source_type.identity,
            device_layout.identity,
            OwnershipSemantics::SharedBorrow,
            AccessMode::ReadOnly,
            AliasSemantics::SharedReadOnly,
            pointer_offset,
        )
    }

    pub fn disjoint_slice(
        source_index: u16,
        name: ValidName,
        source_type: &SourceTypeRecordV1,
        device_layout: &DeviceLayoutRecordV1,
        access: AccessMode,
        pointer_offset: u32,
    ) -> Result<Self, ValidationError> {
        require_matching_descriptors(source_type, device_layout, DescriptorKind::DisjointSlice)?;
        if !matches!(
            access,
            AccessMode::ReadOnly | AccessMode::WriteOnly | AccessMode::ReadWrite
        ) {
            return Err(ValidationError::InvalidArgument(
                "a DisjointSlice must use a memory access mode",
            ));
        }
        Self::slice(
            source_index,
            name,
            source_type.identity,
            device_layout.identity,
            OwnershipSemantics::UniqueBorrow,
            access,
            AliasSemantics::Exclusive,
            pointer_offset,
        )
    }

    pub fn global_mut_pointer(
        source_index: u16,
        name: ValidName,
        source_type: &SourceTypeRecordV1,
        device_layout: &DeviceLayoutRecordV1,
        pointer_offset: u32,
    ) -> Result<Self, ValidationError> {
        require_matching_descriptors(source_type, device_layout, DescriptorKind::GlobalMutPointer)?;
        let value = Self {
            source_index,
            name,
            source_type: source_type.identity,
            device_layout: device_layout.identity,
            ownership: OwnershipSemantics::UniqueBorrow,
            access: AccessMode::ReadWrite,
            alias: AliasSemantics::Exclusive,
            components: vec![PhysicalAbiComponentV1::global_pointer(
                pointer_offset,
                AccessMode::ReadWrite,
                AliasSemantics::Exclusive,
            )],
        };
        value.validate_local()?;
        Ok(value)
    }

    #[allow(clippy::too_many_arguments)]
    fn slice(
        source_index: u16,
        name: ValidName,
        source_type: RustTypeIdentity,
        device_layout: DeviceLayoutIdentity,
        ownership: OwnershipSemantics,
        access: AccessMode,
        alias: AliasSemantics,
        pointer_offset: u32,
    ) -> Result<Self, ValidationError> {
        let length_offset = pointer_offset
            .checked_add(8)
            .ok_or(ValidationError::Overflow {
                field: "slice length offset",
            })?;
        let value = Self {
            source_index,
            name,
            source_type,
            device_layout,
            ownership,
            access,
            alias,
            components: vec![
                PhysicalAbiComponentV1::global_pointer(pointer_offset, access, alias),
                PhysicalAbiComponentV1::slice_length(length_offset),
            ],
        };
        value.validate_local()?;
        Ok(value)
    }

    pub const fn source_index(&self) -> u16 {
        self.source_index
    }

    pub const fn name(&self) -> &ValidName {
        &self.name
    }

    pub const fn source_type(&self) -> RustTypeIdentity {
        self.source_type
    }

    pub const fn device_layout(&self) -> DeviceLayoutIdentity {
        self.device_layout
    }

    pub const fn ownership(&self) -> OwnershipSemantics {
        self.ownership
    }

    pub const fn access(&self) -> AccessMode {
        self.access
    }

    pub const fn alias(&self) -> AliasSemantics {
        self.alias
    }

    pub fn physical_components(
        &self,
    ) -> impl ExactSizeIterator<Item = (PhysicalAbiComponentKind, u32, u16, u16)> + '_ {
        self.components
            .iter()
            .map(|value| (value.kind, value.offset, value.size, value.alignment))
    }

    pub(crate) fn validate_local(&self) -> Result<(), ValidationError> {
        if self.components.is_empty() {
            return Err(ValidationError::InvalidPhysicalAbi(
                "an argument must have physical components",
            ));
        }
        let mut previous_end = None;
        for component in &self.components {
            if component.size == 0
                || component.alignment == 0
                || !component.alignment.is_power_of_two()
                || !component
                    .offset
                    .is_multiple_of(u32::from(component.alignment))
            {
                return Err(ValidationError::InvalidPhysicalAbi(
                    "component size, alignment, or offset is invalid",
                ));
            }
            if previous_end.is_some_and(|end| u64::from(component.offset) < end) {
                return Err(ValidationError::InvalidPhysicalAbi(
                    "components overlap or are out of order",
                ));
            }
            previous_end = Some(component.end()?);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DimensionsV1 {
    x: u32,
    y: u32,
    z: u32,
}

impl DimensionsV1 {
    pub fn new(x: u32, y: u32, z: u32) -> Result<Self, ValidationError> {
        if x == 0 || y == 0 || z == 0 {
            return Err(ValidationError::InvalidValue {
                field: "launch dimensions",
            });
        }
        u64::from(x)
            .checked_mul(u64::from(y))
            .and_then(|xy| xy.checked_mul(u64::from(z)))
            .ok_or(ValidationError::Overflow {
                field: "launch dimensions",
            })?;
        Ok(Self { x, y, z })
    }

    pub const fn x(self) -> u32 {
        self.x
    }

    pub const fn y(self) -> u32 {
        self.y
    }

    pub const fn z(self) -> u32 {
        self.z
    }

    fn validate_rank(self, rank: u8) -> Result<(), ValidationError> {
        if (rank < 2 && self.y != 1) || (rank < 3 && self.z != 1) {
            return Err(ValidationError::InvalidValue {
                field: "unused launch dimension",
            });
        }
        Ok(())
    }

    fn product(self) -> u64 {
        u64::from(self.x) * u64::from(self.y) * u64::from(self.z)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlockSizeV1 {
    Any,
    Exact(DimensionsV1),
    AtMost(DimensionsV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchConstraintsV1 {
    pub(crate) rank: u8,
    pub(crate) block_size: BlockSizeV1,
    pub(crate) max_grid: DimensionsV1,
    pub(crate) max_flat_workgroup_size: u32,
    pub(crate) static_shared_memory_bytes: u32,
    pub(crate) max_dynamic_shared_memory_bytes: u32,
}

impl LaunchConstraintsV1 {
    pub fn new(
        rank: u8,
        block_size: BlockSizeV1,
        max_grid: DimensionsV1,
        max_flat_workgroup_size: u32,
        static_shared_memory_bytes: u32,
        max_dynamic_shared_memory_bytes: u32,
    ) -> Result<Self, ValidationError> {
        if !(1..=3).contains(&rank) {
            return Err(ValidationError::InvalidValue {
                field: "launch rank",
            });
        }
        if max_flat_workgroup_size == 0 {
            return Err(ValidationError::InvalidValue {
                field: "maximum flat workgroup size",
            });
        }
        max_grid.validate_rank(rank)?;
        if let BlockSizeV1::Exact(dimensions) | BlockSizeV1::AtMost(dimensions) = block_size {
            dimensions.validate_rank(rank)?;
            if dimensions.product() > u64::from(max_flat_workgroup_size) {
                return Err(ValidationError::InvalidValue {
                    field: "block size",
                });
            }
        }
        static_shared_memory_bytes
            .checked_add(max_dynamic_shared_memory_bytes)
            .ok_or(ValidationError::Overflow {
                field: "shared memory limit",
            })?;
        Ok(Self {
            rank,
            block_size,
            max_grid,
            max_flat_workgroup_size,
            static_shared_memory_bytes,
            max_dynamic_shared_memory_bytes,
        })
    }

    pub const fn rank(&self) -> u8 {
        self.rank
    }

    pub const fn block_size(&self) -> BlockSizeV1 {
        self.block_size
    }

    pub const fn max_grid(&self) -> DimensionsV1 {
        self.max_grid
    }

    pub const fn max_flat_workgroup_size(&self) -> u32 {
        self.max_flat_workgroup_size
    }

    pub const fn static_shared_memory_bytes(&self) -> u32 {
        self.static_shared_memory_bytes
    }

    pub const fn max_dynamic_shared_memory_bytes(&self) -> u32 {
        self.max_dynamic_shared_memory_bytes
    }
}

/// One independent execution requirement declared by a kernel.
///
/// No dependency closure is implied. Trusted compiler derivation and observed
/// target matching belong to later layers.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CapabilityV1 {
    Subgroup,
    Ballot,
    Shuffle,
    WorkgroupMemory,
    MatrixMultiply,
    AsyncCopy,
    Atomics,
    AmdWave,
    AmdMfma,
    AmdWmma,
    AmdDsPermute,
}

/// The bounded physical argument layout declared for one kernel entry point.
///
/// `explicit_argument_size` encloses every V1 physical component.
/// `kernarg_segment_size` is the complete compiler-declared segment and may
/// include implicit arguments. In particular, V4 does not require it to be a
/// multiple of `kernarg_segment_alignment`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KernelAbiLayoutV1 {
    pub(crate) explicit_argument_size: u32,
    pub(crate) kernarg_segment_size: u32,
    pub(crate) kernarg_segment_alignment: u32,
}

impl KernelAbiLayoutV1 {
    pub fn new(
        explicit_argument_size: u32,
        kernarg_segment_size: u32,
        kernarg_segment_alignment: u32,
    ) -> Result<Self, ValidationError> {
        let value = Self {
            explicit_argument_size,
            kernarg_segment_size,
            kernarg_segment_alignment,
        };
        value.validate_sizes()?;
        Ok(value)
    }

    pub const fn explicit_argument_size(self) -> u32 {
        self.explicit_argument_size
    }

    pub const fn kernarg_segment_size(self) -> u32 {
        self.kernarg_segment_size
    }

    pub const fn kernarg_segment_alignment(self) -> u32 {
        self.kernarg_segment_alignment
    }

    fn validate_sizes(self) -> Result<(), ValidationError> {
        if self.explicit_argument_size > self.kernarg_segment_size {
            return Err(ValidationError::InvalidPhysicalAbi(
                "explicit argument size exceeds the complete kernarg segment",
            ));
        }
        if self.kernarg_segment_size > MAX_KERNARG_SEGMENT_BYTES {
            return Err(ValidationError::InvalidPhysicalAbi(
                "kernarg segment exceeds the stack-wide size ceiling",
            ));
        }
        if self.kernarg_segment_alignment == 0
            || !self.kernarg_segment_alignment.is_power_of_two()
            || self.kernarg_segment_alignment > MAX_KERNARG_SEGMENT_BYTES
        {
            return Err(ValidationError::InvalidPhysicalAbi(
                "kernarg segment alignment is invalid",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KernelDescriptorV1 {
    pub(crate) kernel_id: KernelId,
    pub(crate) logical_name: ValidName,
    pub(crate) entry_name: ValidName,
    pub(crate) descriptor_symbol: ValidName,
    pub(crate) source_evidence: BuildEvidenceV1,
    pub(crate) executable_ir_evidence: BuildEvidenceV1,
    pub(crate) capabilities: Vec<CapabilityV1>,
    pub(crate) abi_layout: KernelAbiLayoutV1,
    pub(crate) launch: LaunchConstraintsV1,
    pub(crate) arguments: Vec<LogicalArgumentV1>,
}

impl KernelDescriptorV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        kernel_id: KernelId,
        logical_name: ValidName,
        entry_name: ValidName,
        descriptor_symbol: ValidName,
        source_evidence: BuildEvidenceV1,
        executable_ir_evidence: BuildEvidenceV1,
        mut capabilities: Vec<CapabilityV1>,
        abi_layout: KernelAbiLayoutV1,
        launch: LaunchConstraintsV1,
        arguments: Vec<LogicalArgumentV1>,
    ) -> Result<Self, ValidationError> {
        check_count(capabilities.len(), "kernel capabilities", MAX_CAPABILITIES)?;
        check_count(
            arguments.len(),
            "kernel arguments",
            MAX_ARGUMENTS_PER_KERNEL,
        )?;
        capabilities.sort_unstable();
        reject_adjacent_duplicates(&capabilities, "kernel capability")?;
        let value = Self {
            kernel_id,
            logical_name,
            entry_name,
            descriptor_symbol,
            source_evidence,
            executable_ir_evidence,
            capabilities,
            abi_layout,
            launch,
            arguments,
        };
        value.validate()?;
        Ok(value)
    }

    pub const fn kernel_id(&self) -> KernelId {
        self.kernel_id
    }

    pub const fn logical_name(&self) -> &ValidName {
        &self.logical_name
    }

    pub const fn entry_name(&self) -> &ValidName {
        &self.entry_name
    }

    pub const fn descriptor_symbol(&self) -> &ValidName {
        &self.descriptor_symbol
    }

    pub const fn source_evidence(&self) -> BuildEvidenceV1 {
        self.source_evidence
    }

    pub const fn executable_ir_evidence(&self) -> BuildEvidenceV1 {
        self.executable_ir_evidence
    }

    pub fn capabilities(&self) -> &[CapabilityV1] {
        &self.capabilities
    }

    pub const fn abi_layout(&self) -> KernelAbiLayoutV1 {
        self.abi_layout
    }

    pub const fn launch(&self) -> &LaunchConstraintsV1 {
        &self.launch
    }

    pub fn arguments(&self) -> &[LogicalArgumentV1] {
        &self.arguments
    }

    pub(crate) fn validate(&self) -> Result<(), ValidationError> {
        self.abi_layout.validate_sizes()?;
        if self.arguments.len() > MAX_ARGUMENTS_PER_KERNEL {
            return Err(ValidationError::TooMany {
                field: "kernel arguments",
                max: MAX_ARGUMENTS_PER_KERNEL,
            });
        }
        if self.capabilities.len() > MAX_CAPABILITIES {
            return Err(ValidationError::TooMany {
                field: "kernel capabilities",
                max: MAX_CAPABILITIES,
            });
        }
        require_strict_order(&self.capabilities, "kernel capabilities")?;

        let mut names = BTreeSet::new();
        let mut component_count = 0usize;
        let mut previous_end = None;
        let mut maximum_component_alignment = 1_u64;
        for (expected_index, argument) in self.arguments.iter().enumerate() {
            if usize::from(argument.source_index) != expected_index {
                return Err(ValidationError::InvalidArgument(
                    "source indices must be contiguous and start at zero",
                ));
            }
            if !names.insert(argument.name.as_str()) {
                return Err(ValidationError::Duplicate {
                    field: "argument name",
                });
            }
            argument.validate_local()?;
            component_count = component_count
                .checked_add(argument.components.len())
                .ok_or(ValidationError::Overflow {
                    field: "physical component count",
                })?;
            for component in &argument.components {
                if component.end()? > u64::from(self.abi_layout.explicit_argument_size) {
                    return Err(ValidationError::InvalidPhysicalAbi(
                        "physical component exceeds the explicit argument region",
                    ));
                }
                if u32::from(component.alignment) > self.abi_layout.kernarg_segment_alignment {
                    return Err(ValidationError::InvalidPhysicalAbi(
                        "physical component alignment exceeds the kernarg segment alignment",
                    ));
                }
                maximum_component_alignment =
                    maximum_component_alignment.max(u64::from(component.alignment));
                if previous_end.is_some_and(|end| u64::from(component.offset) < end) {
                    return Err(ValidationError::InvalidPhysicalAbi(
                        "kernel components overlap or are out of source order",
                    ));
                }
                previous_end = Some(component.end()?);
            }
        }
        if component_count > MAX_PHYSICAL_COMPONENTS_PER_KERNEL {
            return Err(ValidationError::TooMany {
                field: "physical ABI components",
                max: MAX_PHYSICAL_COMPONENTS_PER_KERNEL,
            });
        }
        let component_end = previous_end.unwrap_or(0);
        let canonical_explicit_size = if component_end == 0 {
            0
        } else {
            component_end
                .checked_add(maximum_component_alignment - 1)
                .ok_or(ValidationError::Overflow {
                    field: "explicit argument size",
                })?
                & !(maximum_component_alignment - 1)
        };
        if u64::from(self.abi_layout.explicit_argument_size) != canonical_explicit_size {
            return Err(ValidationError::InvalidPhysicalAbi(
                "explicit argument size must equal the canonically aligned end of the final physical component",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceDescriptorTableV1 {
    pub(crate) canonical_code_object_digest: CanonicalCodeObjectDigest,
    pub(crate) code_object_version: CodeObjectVersion,
    pub(crate) compiler: CompilerIdentityV1,
    pub(crate) producer: ProducerIdentityV1,
    pub(crate) device_target: DeviceTargetV1,
    pub(crate) type_records: Vec<SourceTypeRecordV1>,
    pub(crate) layout_records: Vec<DeviceLayoutRecordV1>,
    pub(crate) kernels: Vec<KernelDescriptorV1>,
}

impl DeviceDescriptorTableV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        canonical_code_object_digest: CanonicalCodeObjectDigest,
        code_object_version: CodeObjectVersion,
        compiler: CompilerIdentityV1,
        producer: ProducerIdentityV1,
        device_target: DeviceTargetV1,
        mut type_records: Vec<SourceTypeRecordV1>,
        mut layout_records: Vec<DeviceLayoutRecordV1>,
        mut kernels: Vec<KernelDescriptorV1>,
    ) -> Result<Self, ValidationError> {
        check_count(kernels.len(), "kernels", MAX_KERNELS)?;
        check_count(type_records.len(), "type records", MAX_TYPE_RECORDS)?;
        check_count(layout_records.len(), "layout records", MAX_LAYOUT_RECORDS)?;
        type_records.sort_unstable_by_key(SourceTypeRecordV1::identity);
        layout_records.sort_unstable_by_key(DeviceLayoutRecordV1::identity);
        kernels.sort_unstable_by_key(KernelDescriptorV1::kernel_id);
        let value = Self {
            canonical_code_object_digest,
            code_object_version,
            compiler,
            producer,
            device_target,
            type_records,
            layout_records,
            kernels,
        };
        value.validate()?;
        crate::encode::validate_encoded_size(&value)?;
        Ok(value)
    }

    pub const fn canonical_code_object_digest(&self) -> CanonicalCodeObjectDigest {
        self.canonical_code_object_digest
    }

    pub const fn code_object_version(&self) -> CodeObjectVersion {
        self.code_object_version
    }

    pub const fn compiler(&self) -> &CompilerIdentityV1 {
        &self.compiler
    }

    pub const fn producer(&self) -> &ProducerIdentityV1 {
        &self.producer
    }

    pub const fn device_target(&self) -> DeviceTargetV1 {
        self.device_target
    }

    pub fn type_records(&self) -> &[SourceTypeRecordV1] {
        &self.type_records
    }

    pub fn layout_records(&self) -> &[DeviceLayoutRecordV1] {
        &self.layout_records
    }

    pub fn kernels(&self) -> &[KernelDescriptorV1] {
        &self.kernels
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_wire(
        canonical_code_object_digest: CanonicalCodeObjectDigest,
        code_object_version: CodeObjectVersion,
        compiler: CompilerIdentityV1,
        producer: ProducerIdentityV1,
        device_target: DeviceTargetV1,
        type_records: Vec<SourceTypeRecordV1>,
        layout_records: Vec<DeviceLayoutRecordV1>,
        kernels: Vec<KernelDescriptorV1>,
    ) -> Result<Self, ValidationError> {
        let value = Self {
            canonical_code_object_digest,
            code_object_version,
            compiler,
            producer,
            device_target,
            type_records,
            layout_records,
            kernels,
        };
        value.validate()?;
        Ok(value)
    }

    fn validate(&self) -> Result<(), ValidationError> {
        if self.kernels.is_empty() {
            return Err(ValidationError::Empty { field: "kernels" });
        }
        check_count(self.kernels.len(), "kernels", MAX_KERNELS)?;
        check_count(self.type_records.len(), "type records", MAX_TYPE_RECORDS)?;
        check_count(
            self.layout_records.len(),
            "layout records",
            MAX_LAYOUT_RECORDS,
        )?;
        require_strict_order_by(
            &self.type_records,
            SourceTypeRecordV1::identity,
            "type records",
        )?;
        require_strict_order_by(
            &self.layout_records,
            DeviceLayoutRecordV1::identity,
            "layout records",
        )?;
        require_strict_order_by(&self.kernels, KernelDescriptorV1::kernel_id, "kernels")?;

        reject_duplicate_kernel_field(
            &self.kernels,
            |kernel| kernel.logical_name.as_str(),
            "kernel logical name",
        )?;
        reject_duplicate_kernel_field(
            &self.kernels,
            |kernel| kernel.entry_name.as_str(),
            "kernel entry name",
        )?;
        reject_duplicate_kernel_field(
            &self.kernels,
            |kernel| kernel.descriptor_symbol.as_str(),
            "kernel descriptor symbol",
        )?;

        let mut reached_types = vec![false; self.type_records.len()];
        let mut reached_layouts = vec![false; self.layout_records.len()];
        for kernel in &self.kernels {
            kernel.validate()?;
            for argument in &kernel.arguments {
                let type_index = self
                    .type_records
                    .binary_search_by_key(&argument.source_type, SourceTypeRecordV1::identity)
                    .map_err(|_| ValidationError::DanglingReference { field: "Rust type" })?;
                let layout_index = self
                    .layout_records
                    .binary_search_by_key(&argument.device_layout, DeviceLayoutRecordV1::identity)
                    .map_err(|_| ValidationError::DanglingReference {
                        field: "device layout",
                    })?;
                reached_types[type_index] = true;
                reached_layouts[layout_index] = true;
                validate_argument_against_records(
                    argument,
                    &self.type_records[type_index],
                    &self.layout_records[layout_index],
                )?;
            }
        }
        if reached_types.iter().any(|reached| !reached) {
            return Err(ValidationError::UnreachableRecord { field: "Rust type" });
        }
        if reached_layouts.iter().any(|reached| !reached) {
            return Err(ValidationError::UnreachableRecord {
                field: "device layout",
            });
        }
        Ok(())
    }
}

fn validate_argument_against_records(
    argument: &LogicalArgumentV1,
    source_type: &SourceTypeRecordV1,
    layout: &DeviceLayoutRecordV1,
) -> Result<(), ValidationError> {
    if source_type.descriptor.kind != layout.descriptor.kind
        || source_type.descriptor.element != layout.descriptor.element
    {
        return Err(ValidationError::InvalidArgument(
            "source type and device layout disagree",
        ));
    }
    let expected_layout = match source_type.descriptor.kind {
        DescriptorKind::Scalar => DeviceLayoutDescriptorV1::scalar(source_type.descriptor.element),
        DescriptorKind::SharedSlice => {
            DeviceLayoutDescriptorV1::shared_slice(source_type.descriptor.element)
        }
        DescriptorKind::DisjointSlice => {
            DeviceLayoutDescriptorV1::disjoint_slice(source_type.descriptor.element)
        }
        DescriptorKind::GlobalMutPointer => {
            DeviceLayoutDescriptorV1::global_mut_pointer(source_type.descriptor.element)
        }
    };
    if layout.descriptor != expected_layout {
        return Err(ValidationError::InvalidArgument(
            "device layout is not the canonical V1 lowering",
        ));
    }

    match source_type.descriptor.kind {
        DescriptorKind::Scalar => {
            validate_scalar_argument(argument, source_type.descriptor.element)
        }
        DescriptorKind::SharedSlice => validate_shared_slice_argument(argument),
        DescriptorKind::DisjointSlice => validate_disjoint_slice_argument(argument),
        DescriptorKind::GlobalMutPointer => validate_global_mut_pointer_argument(argument),
    }
}

fn validate_scalar_argument(
    argument: &LogicalArgumentV1,
    scalar: ScalarTypeV1,
) -> Result<(), ValidationError> {
    if argument.ownership != OwnershipSemantics::ByValue
        || argument.access != AccessMode::ByValue
        || argument.alias != AliasSemantics::Value
        || argument.components.len() != 1
    {
        return Err(ValidationError::InvalidArgument(
            "scalar semantics or lowering is inconsistent",
        ));
    }
    let expected = PhysicalAbiComponentV1::scalar(argument.components[0].offset, scalar);
    if argument.components[0] != expected {
        return Err(ValidationError::InvalidPhysicalAbi(
            "scalar component is not canonical",
        ));
    }
    Ok(())
}

fn validate_shared_slice_argument(argument: &LogicalArgumentV1) -> Result<(), ValidationError> {
    if argument.ownership != OwnershipSemantics::SharedBorrow
        || argument.access != AccessMode::ReadOnly
        || argument.alias != AliasSemantics::SharedReadOnly
    {
        return Err(ValidationError::InvalidArgument(
            "shared slice must be a shared read-only borrow",
        ));
    }
    validate_slice_components(argument)
}

fn validate_disjoint_slice_argument(argument: &LogicalArgumentV1) -> Result<(), ValidationError> {
    if argument.ownership != OwnershipSemantics::UniqueBorrow
        || !matches!(
            argument.access,
            AccessMode::ReadOnly | AccessMode::WriteOnly | AccessMode::ReadWrite
        )
        || argument.alias != AliasSemantics::Exclusive
    {
        return Err(ValidationError::InvalidArgument(
            "DisjointSlice must be an exclusive memory borrow",
        ));
    }
    validate_slice_components(argument)
}

fn validate_global_mut_pointer_argument(
    argument: &LogicalArgumentV1,
) -> Result<(), ValidationError> {
    if argument.ownership != OwnershipSemantics::UniqueBorrow
        || argument.access != AccessMode::ReadWrite
        || argument.alias != AliasSemantics::Exclusive
        || argument.components.len() != 1
    {
        return Err(ValidationError::InvalidArgument(
            "global mutable pointer must be one exclusive read-write borrow",
        ));
    }
    let pointer = &argument.components[0];
    let expected = PhysicalAbiComponentV1::global_pointer(
        pointer.offset,
        AccessMode::ReadWrite,
        AliasSemantics::Exclusive,
    );
    if *pointer != expected {
        return Err(ValidationError::InvalidPhysicalAbi(
            "global mutable pointer component is not canonical",
        ));
    }
    Ok(())
}

fn validate_slice_components(argument: &LogicalArgumentV1) -> Result<(), ValidationError> {
    if argument.components.len() != 2 {
        return Err(ValidationError::InvalidPhysicalAbi(
            "a slice must lower to exactly two components",
        ));
    }
    let pointer = &argument.components[0];
    let length = &argument.components[1];
    let expected_pointer =
        PhysicalAbiComponentV1::global_pointer(pointer.offset, argument.access, argument.alias);
    let length_offset = pointer
        .offset
        .checked_add(8)
        .ok_or(ValidationError::Overflow {
            field: "slice length offset",
        })?;
    let expected_length = PhysicalAbiComponentV1::slice_length(length_offset);
    if *pointer != expected_pointer || *length != expected_length {
        return Err(ValidationError::InvalidPhysicalAbi(
            "slice must be a global pointer immediately followed by a u64 length",
        ));
    }
    Ok(())
}

fn require_matching_descriptors(
    source_type: &SourceTypeRecordV1,
    layout: &DeviceLayoutRecordV1,
    kind: DescriptorKind,
) -> Result<(), ValidationError> {
    if source_type.descriptor.kind != kind
        || layout.descriptor.kind != kind
        || source_type.descriptor.element != layout.descriptor.element
    {
        return Err(ValidationError::InvalidArgument(
            "source type and device layout do not match the argument constructor",
        ));
    }
    Ok(())
}

fn check_count(count: usize, field: &'static str, max: usize) -> Result<(), ValidationError> {
    if count > max {
        Err(ValidationError::TooMany { field, max })
    } else {
        Ok(())
    }
}

fn reject_adjacent_duplicates<T: Eq>(
    values: &[T],
    field: &'static str,
) -> Result<(), ValidationError> {
    if values.windows(2).any(|pair| pair[0] == pair[1]) {
        Err(ValidationError::Duplicate { field })
    } else {
        Ok(())
    }
}

fn require_strict_order<T: Ord>(values: &[T], field: &'static str) -> Result<(), ValidationError> {
    for pair in values.windows(2) {
        match pair[0].cmp(&pair[1]) {
            std::cmp::Ordering::Less => {}
            std::cmp::Ordering::Equal => return Err(ValidationError::Duplicate { field }),
            std::cmp::Ordering::Greater => {
                return Err(ValidationError::NonCanonicalOrder { field });
            }
        }
    }
    Ok(())
}

fn require_strict_order_by<T, K: Ord + Copy>(
    values: &[T],
    key: impl Fn(&T) -> K,
    field: &'static str,
) -> Result<(), ValidationError> {
    for pair in values.windows(2) {
        match key(&pair[0]).cmp(&key(&pair[1])) {
            std::cmp::Ordering::Less => {}
            std::cmp::Ordering::Equal => return Err(ValidationError::Duplicate { field }),
            std::cmp::Ordering::Greater => {
                return Err(ValidationError::NonCanonicalOrder { field });
            }
        }
    }
    Ok(())
}

fn reject_duplicate_kernel_field<'a>(
    kernels: &'a [KernelDescriptorV1],
    field_value: impl Fn(&'a KernelDescriptorV1) -> &'a str,
    field: &'static str,
) -> Result<(), ValidationError> {
    let mut values = BTreeSet::new();
    if kernels
        .iter()
        .any(|kernel| !values.insert(field_value(kernel)))
    {
        Err(ValidationError::Duplicate { field })
    } else {
        Ok(())
    }
}

fn validate_name(value: &str, field: &'static str) -> Result<(), ValidationError> {
    if value.is_empty() {
        return Err(ValidationError::Empty { field });
    }
    if value.len() > MAX_NAME_BYTES {
        return Err(ValidationError::TooLong {
            field,
            max: MAX_NAME_BYTES,
        });
    }
    let mut bytes = value.bytes();
    let first = bytes.next().ok_or(ValidationError::Empty { field })?;
    if !(first.is_ascii_alphabetic() || first == b'_')
        || !bytes.all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':' | b'$')
        })
    {
        return Err(ValidationError::InvalidText { field });
    }
    Ok(())
}

fn validate_text(value: &str, field: &'static str) -> Result<(), ValidationError> {
    if value.is_empty() {
        return Err(ValidationError::Empty { field });
    }
    if value.len() > MAX_TEXT_BYTES {
        return Err(ValidationError::TooLong {
            field,
            max: MAX_TEXT_BYTES,
        });
    }
    if value.trim() != value || !value.bytes().all(|byte| (0x20..=0x7e).contains(&byte)) {
        return Err(ValidationError::InvalidText { field });
    }
    Ok(())
}
