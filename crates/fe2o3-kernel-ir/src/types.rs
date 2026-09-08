use std::collections::BTreeSet;

use crate::{AtomicKind, FunctionId};

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
    WriteOnly,
    ReadWrite,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ScalarType {
    Bool,
    I8,
    I16,
    I32,
    I64,
    I128,
    U8,
    U16,
    U32,
    U64,
    U128,
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
                | Self::I128
                | Self::U8
                | Self::U16
                | Self::U32
                | Self::U64
                | Self::U128
                | Self::Index
        )
    }

    pub const fn is_signed_integer(self) -> bool {
        matches!(
            self,
            Self::I8 | Self::I16 | Self::I32 | Self::I64 | Self::I128
        )
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
            Self::I128 | Self::U128 => Some(128),
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

/// Stable logical identity carried by a compiler-issued kernel context value.
///
/// The three digests bind the source kernel marker, target brand, and launch
/// brand without exposing compiler-private identities in canonical Kernel IR.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct KernelContextTypeV1 {
    root: FunctionId,
    kernel_marker: [u8; 32],
    target: [u8; 32],
    launch: [u8; 32],
}

/// Exact source-level index mapping carried by a disjoint-write capability.
///
/// The mapping is part of the logical type and survives optimization. A target
/// backend may erase it only after canonical verification has established that
/// every projected write index carries this exact mapping.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GlobalDisjointIndexSpaceV1 {
    Index1d,
    ShiftedIndex1d {
        offset: u64,
    },
    BlockedIndex1d {
        lanes_per_block: u64,
        elements_per_lane: u64,
    },
    Tiled2dIndex1d {
        lanes_per_tile: u64,
        tile_rows: u64,
        tile_columns: u64,
        elements_per_lane: u64,
    },
    RowStriped2dIndex1d {
        lanes_per_row: u64,
        elements_per_lane: u64,
    },
    GridExclusive,
}

impl GlobalDisjointIndexSpaceV1 {
    /// Returns whether every nonzero extent in this mapping is valid.
    pub const fn is_complete(self) -> bool {
        match self {
            Self::Index1d | Self::ShiftedIndex1d { .. } | Self::GridExclusive => true,
            Self::BlockedIndex1d {
                lanes_per_block,
                elements_per_lane,
            } => lanes_per_block != 0 && elements_per_lane != 0,
            Self::Tiled2dIndex1d {
                lanes_per_tile,
                tile_rows,
                tile_columns,
                elements_per_lane,
            } => {
                lanes_per_tile != 0 && tile_rows != 0 && tile_columns != 0 && elements_per_lane != 0
            }
            Self::RowStriped2dIndex1d {
                lanes_per_row,
                elements_per_lane,
            } => lanes_per_row != 0 && elements_per_lane != 0,
        }
    }
}

/// Nominal and structural identity of a disjoint global-memory index space.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GlobalDisjointIndexContractV1 {
    nominal_identity: [u8; 32],
    mapping: GlobalDisjointIndexSpaceV1,
}

impl GlobalDisjointIndexContractV1 {
    pub const fn new(nominal_identity: [u8; 32], mapping: GlobalDisjointIndexSpaceV1) -> Self {
        Self {
            nominal_identity,
            mapping,
        }
    }

    pub const fn nominal_identity(self) -> [u8; 32] {
        self.nominal_identity
    }

    pub const fn mapping(self) -> GlobalDisjointIndexSpaceV1 {
        self.mapping
    }

    pub fn is_complete(self) -> bool {
        self.nominal_identity != [0; 32] && self.mapping.is_complete()
    }
}

/// Initialization and alias contract of one branded global-memory capability.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GlobalCapabilityRoleV1 {
    /// Immutable, initialized storage that may be read but never written.
    ReadOnly,
    /// Fully initialized storage held through one exclusive mutable borrow.
    ExclusiveReadWrite,
    /// Store-only storage indexed by a compiler-authenticated disjoint mapping.
    DisjointWrite(GlobalDisjointIndexContractV1),
}

impl GlobalCapabilityRoleV1 {
    /// Physical pointer access retained after logical capability erasure.
    pub const fn access(self) -> AccessMode {
        match self {
            Self::ReadOnly => AccessMode::ReadOnly,
            Self::ExclusiveReadWrite => AccessMode::ReadWrite,
            Self::DisjointWrite(_) => AccessMode::WriteOnly,
        }
    }
}

/// A zero-forging-surface global-memory authority tied to one kernel context.
///
/// At runtime this has the same data-pointer/length representation as a slice.
/// It is nevertheless a distinct KIR type so optimizers cannot silently drop
/// the kernel brand, initialization role, or disjoint index-space invariant.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GlobalCapabilityTypeV1 {
    element: Box<Type>,
    context: KernelContextTypeV1,
    role: GlobalCapabilityRoleV1,
}

impl GlobalCapabilityTypeV1 {
    pub fn new(element: Type, context: KernelContextTypeV1, role: GlobalCapabilityRoleV1) -> Self {
        Self {
            element: Box::new(element),
            context,
            role,
        }
    }

    pub fn read_only(element: Type, context: KernelContextTypeV1) -> Self {
        Self::new(element, context, GlobalCapabilityRoleV1::ReadOnly)
    }

    pub fn exclusive_read_write(element: Type, context: KernelContextTypeV1) -> Self {
        Self::new(element, context, GlobalCapabilityRoleV1::ExclusiveReadWrite)
    }

    pub fn disjoint_write(
        element: Type,
        context: KernelContextTypeV1,
        index_space: GlobalDisjointIndexContractV1,
    ) -> Self {
        Self::new(
            element,
            context,
            GlobalCapabilityRoleV1::DisjointWrite(index_space),
        )
    }

    pub const fn element(&self) -> &Type {
        &self.element
    }

    pub const fn context(&self) -> &KernelContextTypeV1 {
        &self.context
    }

    pub const fn role(&self) -> GlobalCapabilityRoleV1 {
        self.role
    }

    pub fn physical_slice_type(&self) -> Type {
        Type::slice(
            (*self.element).clone(),
            AddressSpace::Global,
            self.role.access(),
        )
    }

    pub fn physical_pointer_type(&self) -> Type {
        Type::pointer(
            (*self.element).clone(),
            AddressSpace::Global,
            self.role.access(),
        )
    }

    pub fn is_complete(&self) -> bool {
        self.context.is_complete()
            && self.element.is_storable()
            && !self.element.contains_logical_capability()
            && match self.role {
                GlobalCapabilityRoleV1::ReadOnly | GlobalCapabilityRoleV1::ExclusiveReadWrite => {
                    true
                }
                GlobalCapabilityRoleV1::DisjointWrite(index_space) => index_space.is_complete(),
            }
    }
}

impl KernelContextTypeV1 {
    pub fn new(
        root: impl Into<FunctionId>,
        kernel_marker: [u8; 32],
        target: [u8; 32],
        launch: [u8; 32],
    ) -> Self {
        Self {
            root: root.into(),
            kernel_marker,
            target,
            launch,
        }
    }

    pub const fn root(&self) -> &FunctionId {
        &self.root
    }

    pub const fn kernel_marker(&self) -> &[u8; 32] {
        &self.kernel_marker
    }

    pub const fn target(&self) -> &[u8; 32] {
        &self.target
    }

    pub const fn launch(&self) -> &[u8; 32] {
        &self.launch
    }

    pub fn is_complete(&self) -> bool {
        !self.root.as_str().is_empty()
            && self.kernel_marker != [0; 32]
            && self.target != [0; 32]
            && self.launch != [0; 32]
    }
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
    /// A zero-runtime-size logical capability issued only in a kernel root.
    KernelContext(KernelContextTypeV1),
    /// Branded global-memory authority with a physical slice representation.
    GlobalCapability(GlobalCapabilityTypeV1),
    /// Branded target-neutral execution authority introduced by canonical V13.
    ExecutionCapability(crate::ExecutionCapabilityTypeV1),
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

    pub fn kernel_context(context: KernelContextTypeV1) -> Self {
        Self::KernelContext(context)
    }

    pub fn global_capability(capability: GlobalCapabilityTypeV1) -> Self {
        Self::GlobalCapability(capability)
    }

    pub const fn as_scalar(&self) -> Option<ScalarType> {
        match self {
            Self::Scalar(scalar) => Some(*scalar),
            _ => None,
        }
    }

    pub const fn is_storable(&self) -> bool {
        !matches!(
            self,
            Self::Unit
                | Self::Slice(_)
                | Self::KernelContext(_)
                | Self::GlobalCapability(_)
                | Self::ExecutionCapability(_)
        )
    }

    pub fn contains_kernel_context(&self) -> bool {
        match self {
            Self::KernelContext(_) => true,
            Self::Pointer(pointer) => pointer.pointee.contains_kernel_context(),
            Self::Slice(slice) => slice.element.contains_kernel_context(),
            Self::GlobalCapability(_)
            | Self::ExecutionCapability(_)
            | Self::Unit
            | Self::Scalar(_) => false,
        }
    }

    /// Returns whether this type contains any compiler-issued logical authority.
    pub fn contains_logical_capability(&self) -> bool {
        match self {
            Self::KernelContext(_) | Self::GlobalCapability(_) | Self::ExecutionCapability(_) => {
                true
            }
            Self::Pointer(pointer) => pointer.pointee.contains_logical_capability(),
            Self::Slice(slice) => slice.element.contains_logical_capability(),
            Self::Unit | Self::Scalar(_) => false,
        }
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

/// Target-neutral collective semantic requested by a canonical kernel.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CollectiveCapabilityOperationV1 {
    Broadcast,
    ReduceAdd,
    ReduceMin,
    ReduceMax,
    InclusiveScanAdd,
    ExclusiveScanAdd,
    Any,
    All,
}

/// Completion mechanism required by one asynchronous copy family.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AsyncCopyCompletionV1 {
    /// The target exposes explicit wait-group completion with this pending bound.
    ExplicitWaitGroups { maximum_pending_groups: u16 },
    /// Completion is established by a convergent workgroup barrier.
    WorkgroupBarrier,
}

/// Numerical behavior required by a target-neutral operation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NumericalModeV1 {
    StrictIeee,
    AllowContraction,
    AllowApproximation,
}

/// Static or launch-dependent resource requirement.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ResourceCapabilityRequirementV1 {
    WorkgroupInvocationsAtMost(u32),
    StaticWorkgroupMemoryBytesAtMost(u64),
    DynamicWorkgroupMemoryBytesAtMost(u64),
    PrivateMemoryBytesPerInvocationAtMost(u64),
}

/// One closed target-neutral execution requirement retained by canonical KIR.
///
/// This is a requirement, not target-support evidence. A target adapter must
/// answer every field without projecting away operation, ordering, scope,
/// address-space, numerical, completion, or resource axes.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExecutionCapabilityRequirementV1 {
    AddressSpace {
        address_space: AddressSpace,
        access: AccessMode,
    },
    Atomic {
        value_type: ScalarType,
        operation: AtomicKind,
        ordering: MemoryOrdering,
        failure_ordering: Option<MemoryOrdering>,
        scope: SynchronizationScope,
        address_space: AddressSpace,
    },
    Barrier {
        execution_scope: SynchronizationScope,
        memory_scope: SynchronizationScope,
        ordering: MemoryOrdering,
        address_spaces: BTreeSet<AddressSpace>,
    },
    Collective {
        execution_scope: SynchronizationScope,
        operation: CollectiveCapabilityOperationV1,
        value_type: ScalarType,
        participants: u32,
    },
    Matrix {
        m: u16,
        n: u16,
        k: u16,
        input_type: ScalarType,
        accumulator_type: ScalarType,
    },
    AsyncCopy {
        source: AddressSpace,
        destination: AddressSpace,
        bytes: u32,
        alignment: u16,
        completion: AsyncCopyCompletionV1,
    },
    Numerical {
        value_type: ScalarType,
        mode: NumericalModeV1,
    },
    Resource(ResourceCapabilityRequirementV1),
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
    /// A closed portable execution requirement introduced by Kernel IR V12.
    Execution(ExecutionCapabilityRequirementV1),
}
