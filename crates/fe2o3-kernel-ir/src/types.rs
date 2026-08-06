use std::collections::BTreeSet;

/// A memory address space with target-independent semantics.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AddressSpace {
    /// Memory private to one invocation.
    Private,
    /// Memory shared by a workgroup.
    Workgroup,
    /// Device-visible global memory.
    Global,
    /// Read-only device-visible memory.
    Constant,
    /// A pointer whose concrete address space is not statically known.
    Generic,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AccessMode {
    ReadOnly,
    ReadWrite,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ScalarType {
    Bool,
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    /// The target's pointer-sized unsigned indexing type.
    Index,
    F16,
    Bf16,
    F32,
    F64,
}

impl ScalarType {
    pub const fn is_integer(self) -> bool {
        matches!(
            self,
            Self::I8
                | Self::I16
                | Self::I32
                | Self::I64
                | Self::U8
                | Self::U16
                | Self::U32
                | Self::U64
                | Self::Index
        )
    }

    pub const fn is_signed_integer(self) -> bool {
        matches!(self, Self::I8 | Self::I16 | Self::I32 | Self::I64)
    }

    pub const fn is_float(self) -> bool {
        matches!(self, Self::F16 | Self::Bf16 | Self::F32 | Self::F64)
    }

    pub const fn is_numeric(self) -> bool {
        self.is_integer() || self.is_float()
    }

    pub const fn bit_width(self) -> Option<u16> {
        match self {
            Self::Bool => Some(1),
            Self::I8 | Self::U8 => Some(8),
            Self::I16 | Self::U16 | Self::F16 | Self::Bf16 => Some(16),
            Self::I32 | Self::U32 | Self::F32 => Some(32),
            Self::I64 | Self::U64 | Self::F64 => Some(64),
            Self::Index => None,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PointerType {
    pub pointee: Box<Type>,
    pub address_space: AddressSpace,
    pub access: AccessMode,
}

impl PointerType {
    pub fn new(pointee: Type, address_space: AddressSpace, access: AccessMode) -> Self {
        Self {
            pointee: Box::new(pointee),
            address_space,
            access,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SliceType {
    pub element: Box<Type>,
    pub address_space: AddressSpace,
    pub access: AccessMode,
}

impl SliceType {
    pub fn new(element: Type, address_space: AddressSpace, access: AccessMode) -> Self {
        Self {
            element: Box::new(element),
            address_space,
            access,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Type {
    Unit,
    Scalar(ScalarType),
    Pointer(PointerType),
    Slice(SliceType),
}

impl Type {
    pub const BOOL: Self = Self::Scalar(ScalarType::Bool);
    pub const INDEX: Self = Self::Scalar(ScalarType::Index);
    pub const F32: Self = Self::Scalar(ScalarType::F32);
    pub const F64: Self = Self::Scalar(ScalarType::F64);

    pub fn pointer(pointee: Type, address_space: AddressSpace, access: AccessMode) -> Self {
        Self::Pointer(PointerType::new(pointee, address_space, access))
    }

    pub fn slice(element: Type, address_space: AddressSpace, access: AccessMode) -> Self {
        Self::Slice(SliceType::new(element, address_space, access))
    }

    pub const fn as_scalar(&self) -> Option<ScalarType> {
        match self {
            Self::Scalar(scalar) => Some(*scalar),
            _ => None,
        }
    }

    pub const fn is_storable(&self) -> bool {
        !matches!(self, Self::Unit | Self::Slice(_))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Axis {
    X,
    Y,
    Z,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LaunchExtent {
    Dynamic,
    Static(u32),
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LaunchDomain {
    D1 {
        x: LaunchExtent,
    },
    D2 {
        x: LaunchExtent,
        y: LaunchExtent,
    },
    D3 {
        x: LaunchExtent,
        y: LaunchExtent,
        z: LaunchExtent,
    },
}

impl LaunchDomain {
    pub const fn rank(&self) -> u8 {
        match self {
            Self::D1 { .. } => 1,
            Self::D2 { .. } => 2,
            Self::D3 { .. } => 3,
        }
    }

    pub const fn contains_axis(&self, axis: Axis) -> bool {
        matches!(
            (self.rank(), axis),
            (_, Axis::X) | (2.., Axis::Y) | (3, Axis::Z)
        )
    }

    pub fn extents(&self) -> impl Iterator<Item = LaunchExtent> {
        let extents = match *self {
            Self::D1 { x } => [Some(x), None, None],
            Self::D2 { x, y } => [Some(x), Some(y), None],
            Self::D3 { x, y, z } => [Some(x), Some(y), Some(z)],
        };
        extents.into_iter().flatten()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorkgroupSize {
    pub x: u32,
    pub y: u32,
    pub z: u32,
}

impl WorkgroupSize {
    pub const fn new(x: u32, y: u32, z: u32) -> Self {
        Self { x, y, z }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SynchronizationScope {
    Invocation,
    Subgroup,
    Workgroup,
    Device,
    System,
}

impl SynchronizationScope {
    pub const fn rank(self) -> u8 {
        match self {
            Self::Invocation => 0,
            Self::Subgroup => 1,
            Self::Workgroup => 2,
            Self::Device => 3,
            Self::System => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MemoryOrdering {
    Relaxed,
    Acquire,
    Release,
    AcquireRelease,
    SequentiallyConsistent,
}

/// A physical AMD-style wave width required by a kernel or helper.
///
/// This is deliberately narrower than [`TargetCapability::SubgroupSize`]:
/// target-neutral subgroup algorithms may use the latter, while lowering
/// that depends on an exact wave32 or wave64 execution mode uses this type.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WaveWidth {
    Wave32,
    Wave64,
}

impl WaveWidth {
    pub const fn lanes(self) -> u32 {
        match self {
            Self::Wave32 => 32,
            Self::Wave64 => 64,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BarrierSemantics {
    pub ordering: MemoryOrdering,
    pub address_spaces: BTreeSet<AddressSpace>,
}

impl BarrierSemantics {
    pub fn new(
        ordering: MemoryOrdering,
        address_spaces: impl IntoIterator<Item = AddressSpace>,
    ) -> Self {
        Self {
            ordering,
            address_spaces: address_spaces.into_iter().collect(),
        }
    }
}

/// A target feature required to lower or execute a module or kernel.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TargetCapability {
    Float16,
    BFloat16,
    Float64,
    Int64,
    Subgroups,
    SubgroupSize(u32),
    WorkgroupMemory,
    WorkgroupBarrier,
    Atomic {
        width_bits: u16,
        address_space: AddressSpace,
        max_scope: SynchronizationScope,
    },
    DynamicWorkgroupMemory,
    /// An extension point for capabilities standardized outside this crate.
    Extension {
        namespace: String,
        name: String,
    },
    /// Requires the target to execute this code with an exact wave width.
    WaveWidth(WaveWidth),
}
