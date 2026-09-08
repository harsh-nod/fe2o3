use core::fmt;

use crate::{TargetProfileSpecV1, TargetProfileValidationErrorV1};

/// Maximum number of requirements in one target-capability closure.
///
/// The bound keeps the target contract usable in `no_std` compiler components
/// without allocating. Exceeding it is an explicit admission failure rather
/// than a truncated answer.
pub const MAX_TARGET_CAPABILITY_CLOSURE_REQUIREMENTS_V1: usize = 128;

/// Logical address space used by a target-neutral kernel requirement.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TargetAddressSpaceV1 {
    /// Memory visible to all workgroups in a dispatch.
    Global,
    /// Memory shared by invocations in one workgroup.
    Workgroup,
    /// Memory private to one invocation.
    Private,
    /// Read-only target storage.
    Constant,
    /// A pointer whose concrete logical address space is not statically known.
    Generic,
}

/// Access required from an address space.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TargetMemoryAccessV1 {
    /// Read access.
    Read,
    /// Write access.
    Write,
    /// Read and write access.
    ReadWrite,
}

/// Target-neutral scalar category.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TargetScalarKindV1 {
    /// Boolean scalar.
    Boolean,
    /// Signed integer scalar.
    SignedInteger,
    /// Unsigned integer scalar.
    UnsignedInteger,
    /// Floating-point scalar.
    Float,
}

/// Exact semantic encoding of a scalar value.
///
/// Width and broad scalar kind are not sufficient to distinguish, for
/// example, IEEE binary16 from bfloat16 or the two incompatible FP8 families.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TargetScalarEncodingV1 {
    /// Canonical false/true predicate encoding.
    Boolean,
    /// Two's-complement integer bits.
    TwosComplement,
    /// IEEE-754 binary floating-point bits.
    IeeeBinary,
    /// Brain floating-point encoding.
    BFloat,
    /// Four-bit E2M1 Open Compute Project encoding.
    Float4E2M1Ocp,
    /// Eight-bit E4M3 finite-numbers-only-with-unsigned-zero encoding.
    Float8E4M3Fnuz,
    /// Eight-bit E5M2 finite-numbers-only-with-unsigned-zero encoding.
    Float8E5M2Fnuz,
    /// Eight-bit E4M3 Open Compute Project encoding.
    Float8E4M3Ocp,
    /// Eight-bit E5M2 Open Compute Project encoding.
    Float8E5M2Ocp,
}

/// Scalar type used by capability requirements.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TargetScalarTypeV1 {
    kind: TargetScalarKindV1,
    bit_width: u16,
    encoding: TargetScalarEncodingV1,
}

impl TargetScalarTypeV1 {
    /// Creates a scalar type.
    pub const fn new(kind: TargetScalarKindV1, bit_width: u16) -> Self {
        let encoding = match kind {
            TargetScalarKindV1::Boolean => TargetScalarEncodingV1::Boolean,
            TargetScalarKindV1::SignedInteger | TargetScalarKindV1::UnsignedInteger => {
                TargetScalarEncodingV1::TwosComplement
            }
            TargetScalarKindV1::Float => TargetScalarEncodingV1::IeeeBinary,
        };
        Self {
            kind,
            bit_width,
            encoding,
        }
    }

    /// Creates a scalar with an explicit encoding.
    pub const fn with_encoding(
        kind: TargetScalarKindV1,
        bit_width: u16,
        encoding: TargetScalarEncodingV1,
    ) -> Self {
        Self {
            kind,
            bit_width,
            encoding,
        }
    }

    /// Creates the exact bfloat16 scalar contract.
    pub const fn bfloat16() -> Self {
        Self::with_encoding(
            TargetScalarKindV1::Float,
            16,
            TargetScalarEncodingV1::BFloat,
        )
    }

    /// Returns the scalar category.
    pub const fn kind(self) -> TargetScalarKindV1 {
        self.kind
    }

    /// Returns the scalar bit width.
    pub const fn bit_width(self) -> u16 {
        self.bit_width
    }

    /// Returns the exact scalar encoding.
    pub const fn encoding(self) -> TargetScalarEncodingV1 {
        self.encoding
    }
}

/// Atomic operation requested by a kernel.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TargetAtomicOperationV1 {
    /// Atomic load.
    Load,
    /// Atomic store.
    Store,
    /// Atomic exchange.
    Exchange,
    /// Atomic compare-and-exchange.
    CompareExchange,
    /// Atomic addition.
    Add,
    /// Atomic subtraction.
    Sub,
    /// Atomic minimum.
    Min,
    /// Atomic maximum.
    Max,
    /// Atomic bitwise and.
    And,
    /// Atomic bitwise nand.
    Nand,
    /// Atomic bitwise or.
    Or,
    /// Atomic bitwise xor.
    Xor,
}

/// Memory ordering requested by an atomic or synchronization operation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TargetMemoryOrderingV1 {
    /// Relaxed ordering.
    Relaxed,
    /// Acquire ordering.
    Acquire,
    /// Release ordering.
    Release,
    /// Acquire-release ordering.
    AcquireRelease,
    /// Sequentially consistent ordering.
    SequentiallyConsistent,
}

/// Memory visibility scope requested by an operation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TargetMemoryScopeV1 {
    /// One invocation.
    Invocation,
    /// One subgroup.
    Subgroup,
    /// One workgroup.
    Workgroup,
    /// One device.
    Device,
    /// All participating devices and the host.
    System,
}

/// Complete atomic legality tuple.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TargetAtomicRequirementV1 {
    value_type: TargetScalarTypeV1,
    operation: TargetAtomicOperationV1,
    ordering: TargetMemoryOrderingV1,
    failure_ordering: Option<TargetMemoryOrderingV1>,
    scope: TargetMemoryScopeV1,
    address_space: TargetAddressSpaceV1,
}

impl TargetAtomicRequirementV1 {
    /// Creates an atomic requirement from its complete legality tuple.
    pub const fn new(
        value_type: TargetScalarTypeV1,
        operation: TargetAtomicOperationV1,
        ordering: TargetMemoryOrderingV1,
        scope: TargetMemoryScopeV1,
        address_space: TargetAddressSpaceV1,
    ) -> Self {
        Self {
            value_type,
            operation,
            ordering,
            failure_ordering: None,
            scope,
            address_space,
        }
    }

    /// Creates a compare-and-exchange requirement with complete success and
    /// failure ordering semantics.
    pub const fn compare_exchange(
        value_type: TargetScalarTypeV1,
        success_ordering: TargetMemoryOrderingV1,
        failure_ordering: TargetMemoryOrderingV1,
        scope: TargetMemoryScopeV1,
        address_space: TargetAddressSpaceV1,
    ) -> Self {
        Self {
            value_type,
            operation: TargetAtomicOperationV1::CompareExchange,
            ordering: success_ordering,
            failure_ordering: Some(failure_ordering),
            scope,
            address_space,
        }
    }

    /// Returns the atomic value type.
    pub const fn value_type(self) -> TargetScalarTypeV1 {
        self.value_type
    }

    /// Returns the atomic operation.
    pub const fn operation(self) -> TargetAtomicOperationV1 {
        self.operation
    }

    /// Returns the memory ordering.
    pub const fn ordering(self) -> TargetMemoryOrderingV1 {
        self.ordering
    }

    /// Returns compare-exchange failure ordering, when applicable.
    pub const fn failure_ordering(self) -> Option<TargetMemoryOrderingV1> {
        self.failure_ordering
    }

    /// Returns the memory scope.
    pub const fn scope(self) -> TargetMemoryScopeV1 {
        self.scope
    }

    /// Returns the logical address space.
    pub const fn address_space(self) -> TargetAddressSpaceV1 {
        self.address_space
    }
}

/// Fence semantics required by a kernel.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TargetFenceRequirementV1 {
    memory_scope: TargetMemoryScopeV1,
    address_space: TargetAddressSpaceV1,
    ordering: TargetMemoryOrderingV1,
}

impl TargetFenceRequirementV1 {
    /// Creates a memory-fence requirement.
    pub const fn new(
        memory_scope: TargetMemoryScopeV1,
        address_space: TargetAddressSpaceV1,
        ordering: TargetMemoryOrderingV1,
    ) -> Self {
        Self {
            memory_scope,
            address_space,
            ordering,
        }
    }

    /// Returns the memory visibility scope.
    pub const fn memory_scope(self) -> TargetMemoryScopeV1 {
        self.memory_scope
    }

    /// Returns the fenced logical address space.
    pub const fn address_space(self) -> TargetAddressSpaceV1 {
        self.address_space
    }

    /// Returns the fence ordering.
    pub const fn ordering(self) -> TargetMemoryOrderingV1 {
        self.ordering
    }
}

/// Invocation scope participating in a barrier or collective.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TargetExecutionScopeV1 {
    /// One subgroup.
    Subgroup,
    /// One workgroup.
    Workgroup,
    /// The complete dispatch grid.
    Grid,
}

/// Control-participation contract at an execution barrier.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TargetBarrierParticipationV1 {
    /// Every invocation in the named execution scope reaches the barrier uniformly.
    Uniform,
    /// Reachability is selected by an arbitrary dynamic mask.
    DynamicMask,
}

/// Barrier semantics required by a kernel.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TargetBarrierRequirementV1 {
    execution_scope: TargetExecutionScopeV1,
    memory_scope: TargetMemoryScopeV1,
    address_space: TargetAddressSpaceV1,
    ordering: TargetMemoryOrderingV1,
    participation: TargetBarrierParticipationV1,
}

impl TargetBarrierRequirementV1 {
    /// Creates a barrier requirement.
    pub const fn new(
        execution_scope: TargetExecutionScopeV1,
        memory_scope: TargetMemoryScopeV1,
        address_space: TargetAddressSpaceV1,
        ordering: TargetMemoryOrderingV1,
        participation: TargetBarrierParticipationV1,
    ) -> Self {
        Self {
            execution_scope,
            memory_scope,
            address_space,
            ordering,
            participation,
        }
    }

    /// Returns the invocation scope that participates.
    pub const fn execution_scope(self) -> TargetExecutionScopeV1 {
        self.execution_scope
    }

    /// Returns the memory visibility scope.
    pub const fn memory_scope(self) -> TargetMemoryScopeV1 {
        self.memory_scope
    }

    /// Returns the synchronized logical address space.
    pub const fn address_space(self) -> TargetAddressSpaceV1 {
        self.address_space
    }

    /// Returns the barrier memory ordering.
    pub const fn ordering(self) -> TargetMemoryOrderingV1 {
        self.ordering
    }

    /// Returns the barrier's control-participation contract.
    pub const fn participation(self) -> TargetBarrierParticipationV1 {
        self.participation
    }
}

/// Collective operation requested by a kernel.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TargetCollectiveOperationV1 {
    /// Produce one predicate bit per participating invocation.
    Ballot,
    /// True when any participating invocation supplies true.
    Any,
    /// True when every participating invocation supplies true.
    All,
    /// Broadcast one participant's value.
    Broadcast,
    /// Sum reduction.
    ReduceAdd,
    /// Minimum reduction.
    ReduceMin,
    /// Maximum reduction.
    ReduceMax,
    /// Inclusive additive scan.
    InclusiveScanAdd,
    /// Exclusive additive scan.
    ExclusiveScanAdd,
}

/// Participation contract for a collective operation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TargetCollectiveParticipationV1 {
    /// Every invocation in the named logical scope participates.
    Full,
    /// A uniform prefix participates; the exact active count is carried here.
    UniformPrefix(u32),
    /// Participation is selected by an arbitrary dynamic mask.
    DynamicMask,
}

/// Collective semantics required by a kernel.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TargetCollectiveRequirementV1 {
    execution_scope: TargetExecutionScopeV1,
    operation: TargetCollectiveOperationV1,
    value_type: TargetScalarTypeV1,
    participants: u32,
    participation: TargetCollectiveParticipationV1,
    numerical_mode: Option<TargetNumericalModeV1>,
}

impl TargetCollectiveRequirementV1 {
    /// Creates a collective requirement.
    pub const fn new(
        execution_scope: TargetExecutionScopeV1,
        operation: TargetCollectiveOperationV1,
        value_type: TargetScalarTypeV1,
        participants: u32,
        participation: TargetCollectiveParticipationV1,
        numerical_mode: Option<TargetNumericalModeV1>,
    ) -> Self {
        Self {
            execution_scope,
            operation,
            value_type,
            participants,
            participation,
            numerical_mode,
        }
    }

    /// Returns the participating invocation scope.
    pub const fn execution_scope(self) -> TargetExecutionScopeV1 {
        self.execution_scope
    }

    /// Returns the collective operation.
    pub const fn operation(self) -> TargetCollectiveOperationV1 {
        self.operation
    }

    /// Returns the collective value type.
    pub const fn value_type(self) -> TargetScalarTypeV1 {
        self.value_type
    }

    /// Returns the exact participant count.
    pub const fn participants(self) -> u32 {
        self.participants
    }

    /// Returns how invocations participate in the collective.
    pub const fn participation(self) -> TargetCollectiveParticipationV1 {
        self.participation
    }

    /// Returns the exact floating-point mode, when the value is floating point.
    pub const fn numerical_mode(self) -> Option<TargetNumericalModeV1> {
        self.numerical_mode
    }
}

/// Exact dimensions of one matrix instruction.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TargetMatrixShapeV1 {
    m: u16,
    n: u16,
    k: u16,
}

impl TargetMatrixShapeV1 {
    /// Creates an exact matrix instruction shape.
    pub const fn new(m: u16, n: u16, k: u16) -> Self {
        Self { m, n, k }
    }

    /// Returns the output row count.
    pub const fn m(self) -> u16 {
        self.m
    }

    /// Returns the output column count.
    pub const fn n(self) -> u16 {
        self.n
    }

    /// Returns the reduction dimension.
    pub const fn k(self) -> u16 {
        self.k
    }
}

/// Logical layouts of both matrix inputs and the output/accumulator.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TargetMatrixLayoutsV1 {
    lhs: TargetMatrixLayoutV1,
    rhs: TargetMatrixLayoutV1,
    output: TargetMatrixLayoutV1,
}

impl TargetMatrixLayoutsV1 {
    /// Creates the complete matrix layout tuple.
    pub const fn new(
        lhs: TargetMatrixLayoutV1,
        rhs: TargetMatrixLayoutV1,
        output: TargetMatrixLayoutV1,
    ) -> Self {
        Self { lhs, rhs, output }
    }

    /// Returns the left-input layout.
    pub const fn lhs(self) -> TargetMatrixLayoutV1 {
        self.lhs
    }

    /// Returns the right-input layout.
    pub const fn rhs(self) -> TargetMatrixLayoutV1 {
        self.rhs
    }

    /// Returns the output/accumulator layout.
    pub const fn output(self) -> TargetMatrixLayoutV1 {
        self.output
    }
}

/// Matrix instruction shape and scalar contract required by a kernel.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TargetMatrixRequirementV1 {
    operation: TargetMatrixOperationV1,
    shape: TargetMatrixShapeV1,
    lhs_type: TargetScalarTypeV1,
    rhs_type: TargetScalarTypeV1,
    accumulator_type: TargetScalarTypeV1,
    layouts: TargetMatrixLayoutsV1,
    numerical_mode: TargetNumericalModeV1,
    subgroup_size: u16,
    active_participants: u16,
}

/// Structured matrix or tensor operation requested by a kernel.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TargetMatrixOperationV1 {
    /// Two-dimensional matrix multiply-accumulate.
    MatrixMultiplyAccumulate,
    /// Two-dimensional low-precision scaled matrix multiply-accumulate.
    ScaledMatrixMultiplyAccumulate,
    /// A tensor contraction represented by one lowered instruction tile.
    TensorContraction,
}

/// Logical matrix layout required at a matrix-operation boundary.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TargetMatrixLayoutV1 {
    /// Consecutive elements advance along the column dimension.
    RowMajor,
    /// Consecutive elements advance along the row dimension.
    ColumnMajor,
    /// Values are already distributed in the operation family's canonical
    /// cooperative subgroup fragment layout.
    CooperativeFragment,
}

impl TargetMatrixRequirementV1 {
    /// Creates a matrix requirement.
    pub const fn new(
        shape: TargetMatrixShapeV1,
        input_type: TargetScalarTypeV1,
        accumulator_type: TargetScalarTypeV1,
        layouts: TargetMatrixLayoutsV1,
        numerical_mode: TargetNumericalModeV1,
    ) -> Self {
        Self::with_complete_contract(
            TargetMatrixOperationV1::MatrixMultiplyAccumulate,
            shape,
            input_type,
            input_type,
            accumulator_type,
            layouts,
            numerical_mode,
            64,
            64,
        )
    }

    /// Creates a matrix requirement from every operation-legality axis.
    #[allow(clippy::too_many_arguments)]
    pub const fn with_complete_contract(
        operation: TargetMatrixOperationV1,
        shape: TargetMatrixShapeV1,
        lhs_type: TargetScalarTypeV1,
        rhs_type: TargetScalarTypeV1,
        accumulator_type: TargetScalarTypeV1,
        layouts: TargetMatrixLayoutsV1,
        numerical_mode: TargetNumericalModeV1,
        subgroup_size: u16,
        active_participants: u16,
    ) -> Self {
        Self {
            operation,
            shape,
            lhs_type,
            rhs_type,
            accumulator_type,
            layouts,
            numerical_mode,
            subgroup_size,
            active_participants,
        }
    }

    /// Creates a tensor-contraction instruction requirement.
    pub const fn tensor_contraction(
        shape: TargetMatrixShapeV1,
        input_type: TargetScalarTypeV1,
        accumulator_type: TargetScalarTypeV1,
        layouts: TargetMatrixLayoutsV1,
        numerical_mode: TargetNumericalModeV1,
    ) -> Self {
        Self {
            operation: TargetMatrixOperationV1::TensorContraction,
            shape,
            lhs_type: input_type,
            rhs_type: input_type,
            accumulator_type,
            layouts,
            numerical_mode,
            subgroup_size: 64,
            active_participants: 64,
        }
    }

    /// Returns the requested structured-compute operation.
    pub const fn operation(self) -> TargetMatrixOperationV1 {
        self.operation
    }

    /// Returns the output row count.
    pub const fn m(self) -> u16 {
        self.shape.m()
    }

    /// Returns the output column count.
    pub const fn n(self) -> u16 {
        self.shape.n()
    }

    /// Returns the reduction dimension.
    pub const fn k(self) -> u16 {
        self.shape.k()
    }

    /// Returns the input scalar type.
    pub const fn input_type(self) -> TargetScalarTypeV1 {
        self.lhs_type
    }

    /// Returns the left-input scalar type.
    pub const fn lhs_type(self) -> TargetScalarTypeV1 {
        self.lhs_type
    }

    /// Returns the right-input scalar type.
    pub const fn rhs_type(self) -> TargetScalarTypeV1 {
        self.rhs_type
    }

    /// Returns the accumulator scalar type.
    pub const fn accumulator_type(self) -> TargetScalarTypeV1 {
        self.accumulator_type
    }

    /// Returns the left-input matrix layout.
    pub const fn lhs_layout(self) -> TargetMatrixLayoutV1 {
        self.layouts.lhs()
    }

    /// Returns the right-input matrix layout.
    pub const fn rhs_layout(self) -> TargetMatrixLayoutV1 {
        self.layouts.rhs()
    }

    /// Returns the output/accumulator matrix layout.
    pub const fn output_layout(self) -> TargetMatrixLayoutV1 {
        self.layouts.output()
    }

    /// Returns the required numerical behavior.
    pub const fn numerical_mode(self) -> TargetNumericalModeV1 {
        self.numerical_mode
    }

    /// Returns the physical subgroup width consumed by the instruction tile.
    pub const fn subgroup_size(self) -> u16 {
        self.subgroup_size
    }

    /// Returns the exact number of active subgroup participants.
    pub const fn active_participants(self) -> u16 {
        self.active_participants
    }
}

/// Asynchronous copy contract required by a kernel.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TargetAsyncCopyRequirementV1 {
    source: TargetAddressSpaceV1,
    destination: TargetAddressSpaceV1,
    bytes: u32,
    alignment: u16,
}

/// Completion contract for previously issued asynchronous transfers.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TargetAsyncWaitRequirementV1 {
    execution_scope: TargetExecutionScopeV1,
    memory_scope: TargetMemoryScopeV1,
    ordering: TargetMemoryOrderingV1,
    max_pending_groups: u16,
}

impl TargetAsyncWaitRequirementV1 {
    /// Creates an asynchronous transfer completion requirement.
    pub const fn new(
        execution_scope: TargetExecutionScopeV1,
        memory_scope: TargetMemoryScopeV1,
        ordering: TargetMemoryOrderingV1,
        max_pending_groups: u16,
    ) -> Self {
        Self {
            execution_scope,
            memory_scope,
            ordering,
            max_pending_groups,
        }
    }

    /// Returns the participating execution scope.
    pub const fn execution_scope(self) -> TargetExecutionScopeV1 {
        self.execution_scope
    }

    /// Returns the memory visibility scope established by completion.
    pub const fn memory_scope(self) -> TargetMemoryScopeV1 {
        self.memory_scope
    }

    /// Returns the completion ordering.
    pub const fn ordering(self) -> TargetMemoryOrderingV1 {
        self.ordering
    }

    /// Returns the maximum number of transfer groups allowed to remain pending.
    pub const fn max_pending_groups(self) -> u16 {
        self.max_pending_groups
    }
}

impl TargetAsyncCopyRequirementV1 {
    /// Creates an asynchronous copy requirement.
    pub const fn new(
        source: TargetAddressSpaceV1,
        destination: TargetAddressSpaceV1,
        bytes: u32,
        alignment: u16,
    ) -> Self {
        Self {
            source,
            destination,
            bytes,
            alignment,
        }
    }

    /// Returns the source address space.
    pub const fn source(self) -> TargetAddressSpaceV1 {
        self.source
    }

    /// Returns the destination address space.
    pub const fn destination(self) -> TargetAddressSpaceV1 {
        self.destination
    }

    /// Returns the copy size in bytes.
    pub const fn bytes(self) -> u32 {
        self.bytes
    }

    /// Returns the required byte alignment.
    pub const fn alignment(self) -> u16 {
        self.alignment
    }
}

/// Numerical behavior required by a kernel operation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TargetNumericalModeV1 {
    /// IEEE behavior without contraction or approximation.
    IeeeStrict,
    /// Floating-point contraction is permitted.
    AllowContraction,
    /// Target approximation is permitted.
    AllowApproximation,
}

/// Numerical behavior required for one exact scalar representation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TargetNumericalRequirementV1 {
    value_type: TargetScalarTypeV1,
    mode: TargetNumericalModeV1,
}

impl TargetNumericalRequirementV1 {
    /// Creates an exact scalar numerical requirement.
    pub const fn new(value_type: TargetScalarTypeV1, mode: TargetNumericalModeV1) -> Self {
        Self { value_type, mode }
    }

    /// Returns the scalar representation whose behavior is constrained.
    pub const fn value_type(self) -> TargetScalarTypeV1 {
        self.value_type
    }

    /// Returns the required numerical mode.
    pub const fn mode(self) -> TargetNumericalModeV1 {
        self.mode
    }
}

/// Static or launch-dependent resource requirement.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TargetResourceRequirementV1 {
    /// Maximum invocations in one workgroup.
    WorkgroupInvocationsAtMost(u32),
    /// Statically allocated workgroup-shared bytes.
    StaticSharedMemoryBytesAtMost(u64),
    /// Dynamically selected workgroup-shared bytes.
    DynamicSharedMemoryBytesAtMost(u64),
    /// Private bytes per invocation.
    PrivateMemoryBytesPerInvocationAtMost(u64),
    /// Exact three-dimensional workgroup dimensions.
    WorkgroupDimensions {
        /// Extent along the first logical dimension.
        x: u32,
        /// Extent along the second logical dimension.
        y: u32,
        /// Extent along the third logical dimension.
        z: u32,
    },
    /// Maximum resident subgroups required by one workgroup.
    SubgroupsPerWorkgroupAtMost(u32),
    /// Maximum registers required by one invocation.
    RegistersPerInvocationAtMost(u32),
}

/// Target byte order.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TargetEndiannessV1 {
    /// Least-significant byte first.
    Little,
    /// Most-significant byte first.
    Big,
}

/// Physical kernel ABI requirement.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TargetAbiConstraintV1 {
    /// Pointer width in bits.
    PointerWidth(u16),
    /// Target byte order.
    Endianness(TargetEndiannessV1),
    /// Maximum required kernel-argument alignment in bytes.
    KernelArgumentAlignmentAtMost(u16),
    /// Maximum required kernel-argument segment size in bytes.
    KernelArgumentSegmentBytesAtMost(u32),
}

/// Target-neutral class of object emitted for the selected profile.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TargetObjectFormatV1 {
    /// A profile-native object that can be loaded for execution.
    LoadableExecutable,
    /// A profile-native object that requires a later link step.
    RelocatableObject,
    /// A portable module requiring target-specific translation.
    PortableModule,
}

/// Produced-object requirement.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TargetObjectConstraintV1 {
    /// Required profile-relative object class.
    Format(TargetObjectFormatV1),
    /// Whether the artifact must be relocatable.
    Relocatable(bool),
}

/// One target-neutral semantic or physical target requirement.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TargetCapabilityRequirementV1 {
    /// Scalar type used by an ordinary arithmetic or memory operation.
    ScalarType(TargetScalarTypeV1),
    /// Exact supported subgroup size.
    SubgroupSize(u16),
    /// Logical address-space access.
    AddressSpace(TargetAddressSpaceV1, TargetMemoryAccessV1),
    /// Complete atomic legality tuple.
    Atomic(TargetAtomicRequirementV1),
    /// Barrier semantics.
    Barrier(TargetBarrierRequirementV1),
    /// Memory-fence semantics.
    Fence(TargetFenceRequirementV1),
    /// Collective semantics.
    Collective(TargetCollectiveRequirementV1),
    /// Matrix operation semantics.
    Matrix(TargetMatrixRequirementV1),
    /// Asynchronous copy semantics.
    AsyncCopy(TargetAsyncCopyRequirementV1),
    /// Asynchronous transfer completion semantics.
    AsyncWait(TargetAsyncWaitRequirementV1),
    /// Numerical behavior.
    Numerical(TargetNumericalRequirementV1),
    /// Resource bound.
    Resource(TargetResourceRequirementV1),
    /// Kernel ABI constraint.
    Abi(TargetAbiConstraintV1),
    /// Produced-object constraint.
    Object(TargetObjectConstraintV1),
}

impl TargetCapabilityRequirementV1 {
    /// Writes the deterministic, target-neutral V1 requirement encoding.
    pub fn encode_canonical(self, writer: &mut impl fmt::Write) -> fmt::Result {
        write!(writer, "{self}")
    }

    /// Validates that every target-independent field has a meaningful canonical value.
    pub fn validate(self) -> Result<(), TargetCapabilityRequirementErrorV1> {
        let validate_scalar = |scalar: TargetScalarTypeV1| {
            if scalar.bit_width() == 0 {
                return Err(TargetCapabilityRequirementErrorV1::ZeroScalarBitWidth);
            }
            let compatible = matches!(
                (scalar.kind(), scalar.bit_width(), scalar.encoding()),
                (
                    TargetScalarKindV1::Boolean,
                    1,
                    TargetScalarEncodingV1::Boolean
                ) | (
                    TargetScalarKindV1::SignedInteger | TargetScalarKindV1::UnsignedInteger,
                    _,
                    TargetScalarEncodingV1::TwosComplement,
                ) | (
                    TargetScalarKindV1::Float,
                    16 | 32 | 64,
                    TargetScalarEncodingV1::IeeeBinary
                ) | (
                    TargetScalarKindV1::Float,
                    16,
                    TargetScalarEncodingV1::BFloat
                ) | (
                    TargetScalarKindV1::Float,
                    4,
                    TargetScalarEncodingV1::Float4E2M1Ocp
                ) | (
                    TargetScalarKindV1::Float,
                    8,
                    TargetScalarEncodingV1::Float8E4M3Fnuz
                        | TargetScalarEncodingV1::Float8E5M2Fnuz
                        | TargetScalarEncodingV1::Float8E4M3Ocp
                        | TargetScalarEncodingV1::Float8E5M2Ocp,
                )
            );
            if compatible {
                Ok(())
            } else {
                Err(TargetCapabilityRequirementErrorV1::InvalidScalarEncoding)
            }
        };

        match self {
            Self::ScalarType(scalar) => validate_scalar(scalar),
            Self::SubgroupSize(0) => Err(TargetCapabilityRequirementErrorV1::ZeroSubgroupSize),
            Self::SubgroupSize(_) | Self::AddressSpace(_, _) => Ok(()),
            Self::Atomic(atomic) => {
                validate_scalar(atomic.value_type())?;
                validate_atomic_orderings(atomic)
            }
            Self::Barrier(_) => Ok(()),
            Self::Fence(fence) if fence.ordering() == TargetMemoryOrderingV1::Relaxed => {
                Err(TargetCapabilityRequirementErrorV1::RelaxedFence)
            }
            Self::Fence(_) => Ok(()),
            Self::Collective(collective) => {
                validate_scalar(collective.value_type())?;
                if collective.participants() == 0
                    || matches!(
                        collective.participation(),
                        TargetCollectiveParticipationV1::UniformPrefix(0)
                    )
                {
                    Err(TargetCapabilityRequirementErrorV1::ZeroCollectiveParticipants)
                } else if matches!(
                    collective.participation(),
                    TargetCollectiveParticipationV1::UniformPrefix(active)
                        if active > collective.participants()
                ) {
                    Err(TargetCapabilityRequirementErrorV1::InvalidCollectiveParticipation)
                } else if (collective.value_type().kind() == TargetScalarKindV1::Float)
                    != collective.numerical_mode().is_some()
                {
                    Err(TargetCapabilityRequirementErrorV1::InvalidCollectiveNumericalMode)
                } else {
                    Ok(())
                }
            }
            Self::Matrix(matrix) => {
                validate_scalar(matrix.lhs_type())?;
                validate_scalar(matrix.rhs_type())?;
                validate_scalar(matrix.accumulator_type())?;
                if matrix.m() == 0 || matrix.n() == 0 || matrix.k() == 0 {
                    Err(TargetCapabilityRequirementErrorV1::ZeroMatrixDimension)
                } else if matrix.subgroup_size() == 0
                    || matrix.active_participants() == 0
                    || matrix.active_participants() > matrix.subgroup_size()
                {
                    Err(TargetCapabilityRequirementErrorV1::InvalidMatrixParticipation)
                } else {
                    Ok(())
                }
            }
            Self::AsyncCopy(copy) => {
                if copy.bytes() == 0 {
                    return Err(TargetCapabilityRequirementErrorV1::ZeroAsyncCopyBytes);
                }
                if !copy.alignment().is_power_of_two() {
                    return Err(TargetCapabilityRequirementErrorV1::InvalidAsyncCopyAlignment);
                }
                Ok(())
            }
            Self::AsyncWait(_) => Ok(()),
            Self::Numerical(numerical) => {
                validate_scalar(numerical.value_type())?;
                if numerical.value_type().kind() == TargetScalarKindV1::Float {
                    Ok(())
                } else {
                    Err(TargetCapabilityRequirementErrorV1::NonFloatNumericalRequirement)
                }
            }
            Self::Resource(TargetResourceRequirementV1::WorkgroupInvocationsAtMost(0)) => {
                Err(TargetCapabilityRequirementErrorV1::ZeroWorkgroupInvocations)
            }
            Self::Resource(TargetResourceRequirementV1::SubgroupsPerWorkgroupAtMost(0)) => {
                Err(TargetCapabilityRequirementErrorV1::ZeroSubgroupsPerWorkgroup)
            }
            Self::Resource(TargetResourceRequirementV1::RegistersPerInvocationAtMost(0)) => {
                Err(TargetCapabilityRequirementErrorV1::ZeroRegistersPerInvocation)
            }
            Self::Resource(TargetResourceRequirementV1::WorkgroupDimensions { x, y, z })
                if x == 0 || y == 0 || z == 0 =>
            {
                Err(TargetCapabilityRequirementErrorV1::ZeroWorkgroupDimension)
            }
            Self::Resource(_) => Ok(()),
            Self::Abi(TargetAbiConstraintV1::PointerWidth(0)) => {
                Err(TargetCapabilityRequirementErrorV1::ZeroAbiPointerWidth)
            }
            Self::Abi(TargetAbiConstraintV1::KernelArgumentAlignmentAtMost(alignment))
                if !alignment.is_power_of_two() =>
            {
                Err(TargetCapabilityRequirementErrorV1::InvalidAbiAlignment)
            }
            Self::Abi(_) | Self::Object(_) => Ok(()),
        }
    }
}

fn validate_atomic_orderings(
    requirement: TargetAtomicRequirementV1,
) -> Result<(), TargetCapabilityRequirementErrorV1> {
    use TargetAtomicOperationV1::{CompareExchange, Load, Store};
    use TargetMemoryOrderingV1::{
        Acquire, AcquireRelease, Relaxed, Release, SequentiallyConsistent,
    };

    let success = requirement.ordering();
    let failure = requirement.failure_ordering();
    match requirement.operation() {
        CompareExchange => {
            let Some(failure) = failure else {
                return Err(TargetCapabilityRequirementErrorV1::MissingAtomicFailureOrdering);
            };
            let valid = match success {
                Relaxed => failure == Relaxed,
                Acquire => matches!(failure, Relaxed | Acquire),
                Release => failure == Relaxed,
                AcquireRelease => matches!(failure, Relaxed | Acquire),
                SequentiallyConsistent => {
                    matches!(failure, Relaxed | Acquire | SequentiallyConsistent)
                }
            };
            if valid {
                Ok(())
            } else {
                Err(TargetCapabilityRequirementErrorV1::InvalidAtomicOrdering)
            }
        }
        Load => {
            if failure.is_some() {
                return Err(TargetCapabilityRequirementErrorV1::UnexpectedAtomicFailureOrdering);
            }
            if matches!(success, Relaxed | Acquire | SequentiallyConsistent) {
                Ok(())
            } else {
                Err(TargetCapabilityRequirementErrorV1::InvalidAtomicOrdering)
            }
        }
        Store => {
            if failure.is_some() {
                return Err(TargetCapabilityRequirementErrorV1::UnexpectedAtomicFailureOrdering);
            }
            if matches!(success, Relaxed | Release | SequentiallyConsistent) {
                Ok(())
            } else {
                Err(TargetCapabilityRequirementErrorV1::InvalidAtomicOrdering)
            }
        }
        _ if failure.is_some() => {
            Err(TargetCapabilityRequirementErrorV1::UnexpectedAtomicFailureOrdering)
        }
        _ => Ok(()),
    }
}

/// Target-independent malformed capability requirement.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TargetCapabilityRequirementErrorV1 {
    /// A scalar type used zero bits.
    ZeroScalarBitWidth,
    /// Scalar kind, width, and encoding disagree.
    InvalidScalarEncoding,
    /// A subgroup requirement requested zero participants.
    ZeroSubgroupSize,
    /// An atomic operation used an ordering forbidden by its semantics.
    InvalidAtomicOrdering,
    /// Compare-and-exchange omitted its failure ordering.
    MissingAtomicFailureOrdering,
    /// A non-compare-exchange operation carried a failure ordering.
    UnexpectedAtomicFailureOrdering,
    /// A memory fence requested relaxed ordering.
    RelaxedFence,
    /// A collective requested zero participants.
    ZeroCollectiveParticipants,
    /// A collective participation count exceeded its logical participant set.
    InvalidCollectiveParticipation,
    /// Floating-point and non-floating collectives carried the wrong numerical-mode presence.
    InvalidCollectiveNumericalMode,
    /// A matrix shape contained a zero dimension.
    ZeroMatrixDimension,
    /// A matrix operation had zero or excessive active subgroup participants.
    InvalidMatrixParticipation,
    /// A numerical requirement named a non-floating scalar.
    NonFloatNumericalRequirement,
    /// An asynchronous transfer requested zero bytes.
    ZeroAsyncCopyBytes,
    /// An asynchronous transfer alignment was zero or not a power of two.
    InvalidAsyncCopyAlignment,
    /// A workgroup resource requirement requested zero invocations.
    ZeroWorkgroupInvocations,
    /// A workgroup dimension was zero.
    ZeroWorkgroupDimension,
    /// A subgroup resource limit was zero.
    ZeroSubgroupsPerWorkgroup,
    /// A register resource limit was zero.
    ZeroRegistersPerInvocation,
    /// A kernel ABI requested a zero-bit pointer.
    ZeroAbiPointerWidth,
    /// A kernel-argument alignment was zero or not a power of two.
    InvalidAbiAlignment,
}

impl fmt::Display for TargetCapabilityRequirementErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ZeroScalarBitWidth => "zero scalar bit width",
            Self::InvalidScalarEncoding => "scalar kind, width, and encoding disagree",
            Self::ZeroSubgroupSize => "zero subgroup size",
            Self::InvalidAtomicOrdering => "invalid atomic ordering",
            Self::MissingAtomicFailureOrdering => {
                "compare-and-exchange omitted its failure ordering"
            }
            Self::UnexpectedAtomicFailureOrdering => {
                "non-compare-exchange atomic carried a failure ordering"
            }
            Self::RelaxedFence => "memory fence requested relaxed ordering",
            Self::ZeroCollectiveParticipants => "zero collective participant count",
            Self::InvalidCollectiveParticipation => "invalid collective participation",
            Self::InvalidCollectiveNumericalMode => "invalid collective numerical-mode presence",
            Self::ZeroMatrixDimension => "zero matrix dimension",
            Self::InvalidMatrixParticipation => "invalid matrix subgroup participation",
            Self::NonFloatNumericalRequirement => {
                "numerical requirement named a non-floating scalar"
            }
            Self::ZeroAsyncCopyBytes => "zero asynchronous copy byte count",
            Self::InvalidAsyncCopyAlignment => "invalid asynchronous copy alignment",
            Self::ZeroWorkgroupInvocations => "zero workgroup invocation count",
            Self::ZeroWorkgroupDimension => "zero workgroup dimension",
            Self::ZeroSubgroupsPerWorkgroup => "zero subgroups-per-workgroup limit",
            Self::ZeroRegistersPerInvocation => "zero registers-per-invocation limit",
            Self::ZeroAbiPointerWidth => "zero ABI pointer width",
            Self::InvalidAbiAlignment => "invalid kernel-argument alignment",
        })
    }
}

impl core::error::Error for TargetCapabilityRequirementErrorV1 {}

/// Runtime evidence needed before a statically legal kernel may launch.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TargetLaunchEvidenceKindV1 {
    /// Exact dynamic shared-memory allocation for the launch.
    DynamicSharedMemoryBytes,
    /// Exact workgroup dimensions for the launch.
    WorkgroupDimensions,
    /// Device-wide cooperative launch admission.
    CooperativeGridAdmission,
    /// The atomic object's allocation and mapping are coherent at system scope.
    SystemAtomicMemoryEligibility,
}

/// Authoritative outcome of one capability query.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TargetCapabilityDecisionOutcomeV1 {
    /// The exact requirement is statically supported.
    Supported,
    /// The exact requirement is not supported.
    Unsupported,
    /// The target model lacks facts required to decide the query.
    Incomplete,
    /// Facts exist but have not passed the review required for authority.
    Unreviewed,
    /// Static support is conditional on exact launch evidence.
    DynamicLaunchEvidenceRequired(TargetLaunchEvidenceKindV1),
}

/// Canonical identity of the target model that answers capability queries.
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct TargetCapabilityModelIdentityV1 {
    profile: TargetProfileSpecV1,
    profile_fingerprint: TargetCapabilityProfileFingerprintV1,
    revision: &'static str,
    revision_fingerprint: TargetCapabilityModelRevisionFingerprintV1,
}

/// Stable opaque binding to the exact selected target profile.
///
/// This fingerprint prevents target-specific spellings from entering neutral
/// capability records. It is a deterministic identity key, not an attestation
/// or collision-resistant security digest.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TargetCapabilityProfileFingerprintV1([u64; 4]);

/// Stable opaque binding to a capability-model implementation revision.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TargetCapabilityModelRevisionFingerprintV1([u64; 4]);

impl TargetCapabilityProfileFingerprintV1 {
    /// Returns the four canonical 64-bit words.
    pub const fn words(self) -> [u64; 4] {
        self.0
    }
}

impl TargetCapabilityModelRevisionFingerprintV1 {
    /// Returns the four canonical 64-bit words.
    pub const fn words(self) -> [u64; 4] {
        self.0
    }
}

const fn target_profile_fingerprint_v1(
    profile: TargetProfileSpecV1,
) -> TargetCapabilityProfileFingerprintV1 {
    let mut state = [
        0xcbf2_9ce4_8422_2325,
        0x8422_2325_cbf2_9ce4,
        0x9e37_79b1_85eb_ca87,
        0xd6e8_feb8_6659_fd93,
    ];
    state = fingerprint_field(state, b"fe2o3.target-profile-fingerprint.v1");
    state = fingerprint_field(state, profile.vendor().as_str().as_bytes());
    state = fingerprint_field(state, profile.architecture_family().as_str().as_bytes());
    state = fingerprint_field(state, profile.architecture().as_bytes());
    state = fingerprint_optional_field(state, profile.rustc_target());
    state = fingerprint_optional_field(state, profile.llvm_target());
    state = fingerprint_field(state, profile.artifact_format().as_str().as_bytes());
    state = fingerprint_field(state, profile.execution_model().as_str().as_bytes());
    state = fingerprint_optional_field(state, profile.data_layout());

    let features = profile.features();
    state = fingerprint_u64(state, features.len() as u64);
    let mut index = 0;
    while index < features.len() {
        state = fingerprint_field(state, features[index].name().as_bytes());
        state = fingerprint_field(state, features[index].state().as_str().as_bytes());
        index += 1;
    }
    TargetCapabilityProfileFingerprintV1(state)
}

const fn fingerprint_optional_field(state: [u64; 4], field: Option<&str>) -> [u64; 4] {
    match field {
        Some(field) => fingerprint_field(fingerprint_u64(state, 1), field.as_bytes()),
        None => fingerprint_u64(state, 0),
    }
}

const fn fingerprint_field(mut state: [u64; 4], field: &[u8]) -> [u64; 4] {
    state = fingerprint_u64(state, field.len() as u64);
    let mut index = 0;
    while index < field.len() {
        let byte = field[index] as u64;
        let mut lane = 0;
        while lane < state.len() {
            state[lane] ^= byte.wrapping_add((lane as u64) << 8);
            state[lane] = state[lane].wrapping_mul(0x0000_0100_0000_01b3);
            state[lane] ^= state[lane] >> (29 + lane);
            lane += 1;
        }
        index += 1;
    }
    state
}

const fn fingerprint_u64(mut state: [u64; 4], value: u64) -> [u64; 4] {
    let bytes = value.to_le_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index] as u64;
        let mut lane = 0;
        while lane < state.len() {
            state[lane] ^= byte.wrapping_add((lane as u64) << 8);
            state[lane] = state[lane].wrapping_mul(0x0000_0100_0000_01b3);
            state[lane] ^= state[lane] >> (29 + lane);
            lane += 1;
        }
        index += 1;
    }
    state
}

impl TargetCapabilityModelIdentityV1 {
    /// Creates and validates an authoritative target-model identity.
    pub fn new(
        profile: TargetProfileSpecV1,
        revision: &'static str,
    ) -> Result<Self, TargetCapabilityModelIdentityErrorV1> {
        let identity = Self::new_unchecked(profile, revision);
        identity.validate()?;
        Ok(identity)
    }

    /// Creates an identity from a target profile and canonical model revision.
    ///
    /// This constructor exists for static declarations. A value created here
    /// cannot pass the public query boundary until [`Self::validate`] succeeds.
    pub const fn new_unchecked(profile: TargetProfileSpecV1, revision: &'static str) -> Self {
        Self {
            profile,
            profile_fingerprint: target_profile_fingerprint_v1(profile),
            revision,
            revision_fingerprint: target_model_revision_fingerprint_v1(revision),
        }
    }

    /// Returns the opaque profile binding used in neutral canonical records.
    pub const fn profile_fingerprint(self) -> TargetCapabilityProfileFingerprintV1 {
        self.profile_fingerprint
    }

    /// Returns the opaque revision binding used in neutral canonical records.
    pub const fn revision_fingerprint(self) -> TargetCapabilityModelRevisionFingerprintV1 {
        self.revision_fingerprint
    }

    /// Validates the profile and unambiguous canonical revision spelling.
    pub fn validate(self) -> Result<(), TargetCapabilityModelIdentityErrorV1> {
        self.profile
            .validate()
            .map_err(TargetCapabilityModelIdentityErrorV1::InvalidProfile)?;
        if self.revision.is_empty() {
            return Err(TargetCapabilityModelIdentityErrorV1::EmptyRevision);
        }
        if !self.revision.is_ascii() {
            return Err(TargetCapabilityModelIdentityErrorV1::NonAsciiRevision);
        }
        if !self.revision.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        }) {
            return Err(TargetCapabilityModelIdentityErrorV1::NonCanonicalRevision);
        }
        Ok(())
    }

    /// Writes the deterministic V1 model identity encoding.
    pub fn encode_canonical(self, writer: &mut impl fmt::Write) -> fmt::Result {
        write!(writer, "{self}")
    }
}

const fn target_model_revision_fingerprint_v1(
    revision: &str,
) -> TargetCapabilityModelRevisionFingerprintV1 {
    let state = [
        0xaf63_bd4c_8601_b7df,
        0x8601_b7df_af63_bd4c,
        0xa076_1d64_78bd_642f,
        0xe703_7ed1_a0b4_28db,
    ];
    TargetCapabilityModelRevisionFingerprintV1(fingerprint_field(
        fingerprint_field(state, b"fe2o3.target-model-revision-fingerprint.v1"),
        revision.as_bytes(),
    ))
}

/// Invalid canonical target-capability model identity.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TargetCapabilityModelIdentityErrorV1 {
    /// The embedded target profile is invalid.
    InvalidProfile(TargetProfileValidationErrorV1),
    /// The model revision is empty.
    EmptyRevision,
    /// The model revision is not ASCII.
    NonAsciiRevision,
    /// The model revision is not lowercase canonical text.
    NonCanonicalRevision,
}

impl fmt::Display for TargetCapabilityModelIdentityErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProfile(error) => write!(formatter, "invalid target profile: {error}"),
            Self::EmptyRevision => formatter.write_str("empty target capability model revision"),
            Self::NonAsciiRevision => {
                formatter.write_str("non-ASCII target capability model revision")
            }
            Self::NonCanonicalRevision => {
                formatter.write_str("noncanonical target capability model revision")
            }
        }
    }
}

impl core::error::Error for TargetCapabilityModelIdentityErrorV1 {}

/// Immutable, replayable result of one target-capability query.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TargetCapabilityDecisionV1 {
    model: TargetCapabilityModelIdentityV1,
    requirement: TargetCapabilityRequirementV1,
    outcome: TargetCapabilityDecisionOutcomeV1,
}

impl TargetCapabilityDecisionV1 {
    const fn new(
        model: TargetCapabilityModelIdentityV1,
        requirement: TargetCapabilityRequirementV1,
        outcome: TargetCapabilityDecisionOutcomeV1,
    ) -> Self {
        Self {
            model,
            requirement,
            outcome,
        }
    }

    /// Returns the target model that issued this decision.
    pub const fn model(self) -> TargetCapabilityModelIdentityV1 {
        self.model
    }

    /// Returns the exact queried requirement.
    pub const fn requirement(self) -> TargetCapabilityRequirementV1 {
        self.requirement
    }

    /// Returns the authoritative outcome.
    pub const fn outcome(self) -> TargetCapabilityDecisionOutcomeV1 {
        self.outcome
    }

    /// Revalidates the identities bound into a stored decision.
    pub fn validate(self) -> Result<(), TargetCapabilityQueryErrorV1> {
        self.model
            .validate()
            .map_err(TargetCapabilityQueryErrorV1::InvalidModel)?;
        self.requirement
            .validate()
            .map_err(TargetCapabilityQueryErrorV1::InvalidRequirement)
    }

    /// Writes the deterministic V1 decision encoding, including the exact
    /// model, requirement, and outcome.
    pub fn encode_canonical(self, writer: &mut impl fmt::Write) -> fmt::Result {
        write!(writer, "{self}")
    }
}

/// Failure before an authoritative target-capability decision can be constructed.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TargetCapabilityQueryErrorV1 {
    /// The target adapter returned an invalid model identity.
    InvalidModel(TargetCapabilityModelIdentityErrorV1),
    /// The requested capability tuple was malformed independently of any target.
    InvalidRequirement(TargetCapabilityRequirementErrorV1),
    /// The target model omitted an answer for a valid requirement.
    OmittedAnswer(TargetCapabilityRequirementV1),
}

impl fmt::Display for TargetCapabilityQueryErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidModel(error) => write!(formatter, "invalid target model: {error}"),
            Self::InvalidRequirement(error) => {
                write!(formatter, "invalid target capability requirement: {error}")
            }
            Self::OmittedAnswer(requirement) => {
                write!(formatter, "target model omitted requirement: {requirement}")
            }
        }
    }
}

impl core::error::Error for TargetCapabilityQueryErrorV1 {}

/// Target-neutral interface implemented by an exact target capability model.
pub trait TargetCapabilityQueryV1 {
    /// Returns the exact model identity bound into every decision receipt.
    fn model_identity(&self) -> TargetCapabilityModelIdentityV1;

    /// Evaluates one complete requirement without silently projecting away an axis.
    fn query_outcome(
        &self,
        requirement: TargetCapabilityRequirementV1,
    ) -> TargetCapabilityDecisionOutcomeV1;

    /// Returns an answer when this model has reviewed the requirement category.
    ///
    /// Existing total implementations inherit the default. Sparse table-backed
    /// providers override this method and return `None` for an omitted row; the
    /// public query boundary rejects that omission rather than interpreting it
    /// as unsupported.
    fn query_outcome_checked(
        &self,
        requirement: TargetCapabilityRequirementV1,
    ) -> Option<TargetCapabilityDecisionOutcomeV1> {
        Some(self.query_outcome(requirement))
    }
}

/// Queries a target model and binds its answer to the exact model and requirement.
///
/// Receipt construction is owned here rather than by target adapters, preserving
/// the identity and requirement-binding invariant at the interface boundary.
pub fn query_target_capability_v1<Q: TargetCapabilityQueryV1 + ?Sized>(
    target: &Q,
    requirement: TargetCapabilityRequirementV1,
) -> Result<TargetCapabilityDecisionV1, TargetCapabilityQueryErrorV1> {
    let model = target.model_identity();
    model
        .validate()
        .map_err(TargetCapabilityQueryErrorV1::InvalidModel)?;
    requirement
        .validate()
        .map_err(TargetCapabilityQueryErrorV1::InvalidRequirement)?;
    let outcome = target
        .query_outcome_checked(requirement)
        .ok_or(TargetCapabilityQueryErrorV1::OmittedAnswer(requirement))?;
    Ok(TargetCapabilityDecisionV1::new(model, requirement, outcome))
}

/// One requirement and its direct semantic prerequisites in an exact closure.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TargetCapabilityClosureNodeV1<'a> {
    requirement: TargetCapabilityRequirementV1,
    dependencies: &'a [TargetCapabilityRequirementV1],
}

impl<'a> TargetCapabilityClosureNodeV1<'a> {
    /// Creates one closure node.
    pub const fn new(
        requirement: TargetCapabilityRequirementV1,
        dependencies: &'a [TargetCapabilityRequirementV1],
    ) -> Self {
        Self {
            requirement,
            dependencies,
        }
    }

    /// Returns this node's requirement.
    pub const fn requirement(self) -> TargetCapabilityRequirementV1 {
        self.requirement
    }

    /// Returns direct prerequisites in canonical ascending order.
    pub const fn dependencies(self) -> &'a [TargetCapabilityRequirementV1] {
        self.dependencies
    }
}

/// Borrowed exact dependency graph submitted for target admission.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TargetCapabilityClosureSpecV1<'a> {
    roots: &'a [TargetCapabilityRequirementV1],
    nodes: &'a [TargetCapabilityClosureNodeV1<'a>],
}

impl<'a> TargetCapabilityClosureSpecV1<'a> {
    /// Creates a closure specification.
    ///
    /// Validation is performed by the public closure query. Roots, nodes, and
    /// each dependency list must be strictly ascending; every node must be
    /// reachable from a root.
    pub const fn new(
        roots: &'a [TargetCapabilityRequirementV1],
        nodes: &'a [TargetCapabilityClosureNodeV1<'a>],
    ) -> Self {
        Self { roots, nodes }
    }

    /// Returns roots in canonical ascending order.
    pub const fn roots(self) -> &'a [TargetCapabilityRequirementV1] {
        self.roots
    }

    /// Returns all reachable nodes in canonical ascending order.
    pub const fn nodes(self) -> &'a [TargetCapabilityClosureNodeV1<'a>] {
        self.nodes
    }
}

/// Exact, deterministic target decisions for a complete capability closure.
///
/// This value is evidence input only. It grants no lowering, publication,
/// artifact, load, or launch authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TargetCapabilityClosureV1<'a> {
    model: TargetCapabilityModelIdentityV1,
    spec: TargetCapabilityClosureSpecV1<'a>,
    outcomes:
        [Option<TargetCapabilityDecisionOutcomeV1>; MAX_TARGET_CAPABILITY_CLOSURE_REQUIREMENTS_V1],
    decision_count: usize,
}

impl<'a> TargetCapabilityClosureV1<'a> {
    /// Returns the exact model that answered the closure.
    pub const fn model(&self) -> TargetCapabilityModelIdentityV1 {
        self.model
    }

    /// Returns the exact dependency graph bound by this result.
    pub const fn spec(&self) -> TargetCapabilityClosureSpecV1<'a> {
        self.spec
    }

    /// Returns the number of decisions, equal to the number of graph nodes.
    pub const fn len(&self) -> usize {
        self.decision_count
    }

    /// Returns whether this closure has no decisions.
    pub const fn is_empty(&self) -> bool {
        self.decision_count == 0
    }

    /// Returns one canonical decision by index.
    pub const fn decision(&self, index: usize) -> Option<TargetCapabilityDecisionV1> {
        if index >= self.decision_count {
            return None;
        }
        match self.outcomes[index] {
            Some(outcome) => Some(TargetCapabilityDecisionV1::new(
                self.model,
                self.spec.nodes[index].requirement,
                outcome,
            )),
            None => None,
        }
    }

    /// Returns the first launch-dependent decision, if any.
    pub fn first_dynamic_requirement(
        &self,
    ) -> Option<(TargetCapabilityRequirementV1, TargetLaunchEvidenceKindV1)> {
        let mut index = 0;
        while index < self.decision_count {
            let outcome = self.outcomes[index]
                .expect("validated closure outcomes are dense through decision_count");
            if let TargetCapabilityDecisionOutcomeV1::DynamicLaunchEvidenceRequired(kind) = outcome
            {
                return Some((self.spec.nodes[index].requirement, kind));
            }
            index += 1;
        }
        None
    }

    /// Writes the deterministic V1 closure encoding.
    pub fn encode_canonical(&self, writer: &mut impl fmt::Write) -> fmt::Result {
        write!(writer, "{self}")
    }
}

/// Failure to resolve or statically admit an exact target-capability closure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetCapabilityClosureErrorV1 {
    /// No root requirement was supplied.
    EmptyRoots,
    /// The closure exceeded the bounded no-allocation contract.
    TooManyRequirements {
        /// Observed node count.
        count: usize,
        /// Maximum admitted node count.
        maximum: usize,
    },
    /// Roots were not strictly ascending and unique.
    NonCanonicalRootOrder,
    /// Nodes were not strictly ascending and unique.
    NonCanonicalNodeOrder,
    /// One node's dependencies were not strictly ascending and unique.
    NonCanonicalDependencyOrder(TargetCapabilityRequirementV1),
    /// A root did not have a corresponding node.
    MissingRootNode(TargetCapabilityRequirementV1),
    /// A dependency did not have a corresponding node.
    MissingDependency {
        /// Node containing the dependency edge.
        requirement: TargetCapabilityRequirementV1,
        /// Missing dependency node.
        dependency: TargetCapabilityRequirementV1,
    },
    /// The graph contains a dependency cycle.
    DependencyCycle(TargetCapabilityRequirementV1),
    /// A listed node was not reachable from any root.
    UnreachableNode(TargetCapabilityRequirementV1),
    /// A requirement or model failed the hardened query boundary.
    Query {
        /// Requirement being queried.
        requirement: TargetCapabilityRequirementV1,
        /// Hardened query failure.
        source: TargetCapabilityQueryErrorV1,
    },
    /// Repeating the same exact query produced a different decision.
    NondeterministicAnswer(TargetCapabilityRequirementV1),
    /// A reviewed target does not support the operation.
    Unsupported(TargetCapabilityRequirementV1),
    /// The target model lacks facts needed for the operation.
    Incomplete(TargetCapabilityRequirementV1),
    /// The operation's facts have not passed authoritative review.
    Unreviewed(TargetCapabilityRequirementV1),
    /// Static admission encountered a launch-dependent requirement.
    DynamicLaunchEvidenceRequired {
        /// Launch-dependent requirement.
        requirement: TargetCapabilityRequirementV1,
        /// Exact evidence kind needed before admission.
        kind: TargetLaunchEvidenceKindV1,
    },
}

impl fmt::Display for TargetCapabilityClosureErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyRoots => formatter.write_str("empty target-capability closure roots"),
            Self::TooManyRequirements { count, maximum } => write!(
                formatter,
                "target-capability closure has {count} requirements; maximum is {maximum}"
            ),
            Self::NonCanonicalRootOrder => {
                formatter.write_str("noncanonical target-capability root order")
            }
            Self::NonCanonicalNodeOrder => {
                formatter.write_str("noncanonical target-capability node order")
            }
            Self::NonCanonicalDependencyOrder(requirement) => write!(
                formatter,
                "noncanonical dependency order for requirement {requirement}"
            ),
            Self::MissingRootNode(requirement) => {
                write!(formatter, "missing closure node for root {requirement}")
            }
            Self::MissingDependency {
                requirement,
                dependency,
            } => write!(
                formatter,
                "requirement {requirement} omits dependency node {dependency}"
            ),
            Self::DependencyCycle(requirement) => {
                write!(formatter, "dependency cycle at requirement {requirement}")
            }
            Self::UnreachableNode(requirement) => {
                write!(formatter, "unreachable closure node {requirement}")
            }
            Self::Query {
                requirement,
                source,
            } => write!(formatter, "query failed for {requirement}: {source}"),
            Self::NondeterministicAnswer(requirement) => {
                write!(formatter, "nondeterministic answer for {requirement}")
            }
            Self::Unsupported(requirement) => {
                write!(formatter, "unsupported requirement {requirement}")
            }
            Self::Incomplete(requirement) => {
                write!(formatter, "incomplete requirement {requirement}")
            }
            Self::Unreviewed(requirement) => {
                write!(formatter, "unreviewed requirement {requirement}")
            }
            Self::DynamicLaunchEvidenceRequired { requirement, kind } => write!(
                formatter,
                "requirement {requirement} needs dynamic launch evidence {kind}"
            ),
        }
    }
}

impl core::error::Error for TargetCapabilityClosureErrorV1 {}

/// Resolves a complete exact capability closure.
///
/// Unsupported, incomplete, unreviewed, omitted, malformed, cyclic, and
/// nondeterministic closures fail. Launch-dependent decisions are retained for
/// a later exact launch-evidence join and therefore do not count as static
/// admission.
pub fn query_target_capability_closure_v1<'a, Q: TargetCapabilityQueryV1 + ?Sized>(
    target: &Q,
    spec: TargetCapabilityClosureSpecV1<'a>,
) -> Result<TargetCapabilityClosureV1<'a>, TargetCapabilityClosureErrorV1> {
    validate_capability_closure_spec_v1(spec)?;
    let model = target.model_identity();
    model
        .validate()
        .map_err(|source| TargetCapabilityClosureErrorV1::Query {
            requirement: spec.nodes[0].requirement,
            source: TargetCapabilityQueryErrorV1::InvalidModel(source),
        })?;

    let mut outcomes = [None; MAX_TARGET_CAPABILITY_CLOSURE_REQUIREMENTS_V1];
    let mut index = 0;
    while index < spec.nodes.len() {
        let requirement = spec.nodes[index].requirement;
        let first = query_target_capability_v1(target, requirement).map_err(|source| {
            TargetCapabilityClosureErrorV1::Query {
                requirement,
                source,
            }
        })?;
        let second = query_target_capability_v1(target, requirement).map_err(|source| {
            TargetCapabilityClosureErrorV1::Query {
                requirement,
                source,
            }
        })?;
        if first != second {
            return Err(TargetCapabilityClosureErrorV1::NondeterministicAnswer(
                requirement,
            ));
        }
        match first.outcome() {
            TargetCapabilityDecisionOutcomeV1::Supported
            | TargetCapabilityDecisionOutcomeV1::DynamicLaunchEvidenceRequired(_) => {}
            TargetCapabilityDecisionOutcomeV1::Unsupported => {
                return Err(TargetCapabilityClosureErrorV1::Unsupported(requirement));
            }
            TargetCapabilityDecisionOutcomeV1::Incomplete => {
                return Err(TargetCapabilityClosureErrorV1::Incomplete(requirement));
            }
            TargetCapabilityDecisionOutcomeV1::Unreviewed => {
                return Err(TargetCapabilityClosureErrorV1::Unreviewed(requirement));
            }
        }
        outcomes[index] = Some(first.outcome());
        index += 1;
    }

    Ok(TargetCapabilityClosureV1 {
        model,
        spec,
        outcomes,
        decision_count: spec.nodes.len(),
    })
}

/// Admits only a closure whose every requirement is statically supported.
pub fn admit_static_target_capability_closure_v1<'a, Q: TargetCapabilityQueryV1 + ?Sized>(
    target: &Q,
    spec: TargetCapabilityClosureSpecV1<'a>,
) -> Result<TargetCapabilityClosureV1<'a>, TargetCapabilityClosureErrorV1> {
    let closure = query_target_capability_closure_v1(target, spec)?;
    if let Some((requirement, kind)) = closure.first_dynamic_requirement() {
        return Err(
            TargetCapabilityClosureErrorV1::DynamicLaunchEvidenceRequired { requirement, kind },
        );
    }
    Ok(closure)
}

fn validate_capability_closure_spec_v1(
    spec: TargetCapabilityClosureSpecV1<'_>,
) -> Result<(), TargetCapabilityClosureErrorV1> {
    if spec.roots.is_empty() {
        return Err(TargetCapabilityClosureErrorV1::EmptyRoots);
    }
    if spec.nodes.len() > MAX_TARGET_CAPABILITY_CLOSURE_REQUIREMENTS_V1 {
        return Err(TargetCapabilityClosureErrorV1::TooManyRequirements {
            count: spec.nodes.len(),
            maximum: MAX_TARGET_CAPABILITY_CLOSURE_REQUIREMENTS_V1,
        });
    }
    if !strictly_sorted(spec.roots) {
        return Err(TargetCapabilityClosureErrorV1::NonCanonicalRootOrder);
    }
    if !strictly_sorted_nodes(spec.nodes) {
        return Err(TargetCapabilityClosureErrorV1::NonCanonicalNodeOrder);
    }

    for root in spec.roots {
        validate_closure_requirement(*root)?;
        if find_node(spec.nodes, *root).is_none() {
            return Err(TargetCapabilityClosureErrorV1::MissingRootNode(*root));
        }
    }
    for node in spec.nodes {
        validate_closure_requirement(node.requirement)?;
        if !strictly_sorted(node.dependencies) {
            return Err(TargetCapabilityClosureErrorV1::NonCanonicalDependencyOrder(
                node.requirement,
            ));
        }
        for dependency in node.dependencies {
            validate_closure_requirement(*dependency)?;
            if find_node(spec.nodes, *dependency).is_none() {
                return Err(TargetCapabilityClosureErrorV1::MissingDependency {
                    requirement: node.requirement,
                    dependency: *dependency,
                });
            }
        }
    }

    let mut colors = [0_u8; MAX_TARGET_CAPABILITY_CLOSURE_REQUIREMENTS_V1];
    let mut index = 0;
    while index < spec.nodes.len() {
        visit_for_cycles(spec.nodes, index, &mut colors)?;
        index += 1;
    }

    let mut reachable = [false; MAX_TARGET_CAPABILITY_CLOSURE_REQUIREMENTS_V1];
    for root in spec.roots {
        mark_reachable(
            spec.nodes,
            find_node(spec.nodes, *root).expect("root existence checked above"),
            &mut reachable,
        );
    }
    index = 0;
    while index < spec.nodes.len() {
        if !reachable[index] {
            return Err(TargetCapabilityClosureErrorV1::UnreachableNode(
                spec.nodes[index].requirement,
            ));
        }
        index += 1;
    }
    Ok(())
}

fn validate_closure_requirement(
    requirement: TargetCapabilityRequirementV1,
) -> Result<(), TargetCapabilityClosureErrorV1> {
    requirement
        .validate()
        .map_err(|source| TargetCapabilityClosureErrorV1::Query {
            requirement,
            source: TargetCapabilityQueryErrorV1::InvalidRequirement(source),
        })
}

fn strictly_sorted(requirements: &[TargetCapabilityRequirementV1]) -> bool {
    requirements.windows(2).all(|pair| pair[0] < pair[1])
}

fn strictly_sorted_nodes(nodes: &[TargetCapabilityClosureNodeV1<'_>]) -> bool {
    nodes
        .windows(2)
        .all(|pair| pair[0].requirement < pair[1].requirement)
}

fn find_node(
    nodes: &[TargetCapabilityClosureNodeV1<'_>],
    requirement: TargetCapabilityRequirementV1,
) -> Option<usize> {
    nodes
        .binary_search_by_key(&requirement, |node| node.requirement)
        .ok()
}

fn visit_for_cycles(
    nodes: &[TargetCapabilityClosureNodeV1<'_>],
    index: usize,
    colors: &mut [u8; MAX_TARGET_CAPABILITY_CLOSURE_REQUIREMENTS_V1],
) -> Result<(), TargetCapabilityClosureErrorV1> {
    if colors[index] == 2 {
        return Ok(());
    }
    if colors[index] == 1 {
        return Err(TargetCapabilityClosureErrorV1::DependencyCycle(
            nodes[index].requirement,
        ));
    }
    colors[index] = 1;
    for dependency in nodes[index].dependencies {
        visit_for_cycles(
            nodes,
            find_node(nodes, *dependency).expect("dependency existence checked before traversal"),
            colors,
        )?;
    }
    colors[index] = 2;
    Ok(())
}

fn mark_reachable(
    nodes: &[TargetCapabilityClosureNodeV1<'_>],
    index: usize,
    reachable: &mut [bool; MAX_TARGET_CAPABILITY_CLOSURE_REQUIREMENTS_V1],
) {
    if reachable[index] {
        return;
    }
    reachable[index] = true;
    for dependency in nodes[index].dependencies {
        mark_reachable(
            nodes,
            find_node(nodes, *dependency).expect("dependency existence checked before traversal"),
            reachable,
        );
    }
}

impl fmt::Display for TargetCapabilityModelIdentityV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "fe2o3.target-capability-model.v1;profile-fingerprint={};revision-fingerprint={}",
            self.profile_fingerprint, self.revision_fingerprint
        )
    }
}

impl fmt::Debug for TargetCapabilityModelIdentityV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TargetCapabilityModelIdentityV1")
            .field("profile_fingerprint", &self.profile_fingerprint)
            .field("revision_fingerprint", &self.revision_fingerprint)
            .finish()
    }
}

impl fmt::Display for TargetCapabilityProfileFingerprintV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for word in self.0 {
            write!(formatter, "{word:016x}")?;
        }
        Ok(())
    }
}

impl fmt::Display for TargetCapabilityModelRevisionFingerprintV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for word in self.0 {
            write!(formatter, "{word:016x}")?;
        }
        Ok(())
    }
}

impl fmt::Display for TargetCapabilityDecisionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "fe2o3.target-capability-decision.v1;model=[{}];requirement=[{}];outcome={}",
            self.model, self.requirement, self.outcome
        )
    }
}

impl fmt::Display for TargetCapabilityClosureV1<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "fe2o3.target-capability-closure.v1;model=[{}];roots=[",
            self.model
        )?;
        for (index, root) in self.spec.roots.iter().enumerate() {
            if index != 0 {
                formatter.write_str(",")?;
            }
            write!(formatter, "{root}")?;
        }
        formatter.write_str("];nodes=[")?;
        for (index, node) in self.spec.nodes.iter().enumerate() {
            if index != 0 {
                formatter.write_str(",")?;
            }
            write!(formatter, "{}<-", node.requirement)?;
            if node.dependencies.is_empty() {
                formatter.write_str("<none>")?;
            } else {
                for (dependency_index, dependency) in node.dependencies.iter().enumerate() {
                    if dependency_index != 0 {
                        formatter.write_str("+")?;
                    }
                    write!(formatter, "{dependency}")?;
                }
            }
            let outcome = self.outcomes[index]
                .expect("validated closure outcomes are dense through the node count");
            write!(formatter, "=>{outcome}")?;
        }
        formatter.write_str("]")
    }
}

macro_rules! display_enum {
    ($type:ty, {$($variant:path => $text:literal),+ $(,)?}) => {
        impl fmt::Display for $type {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(match self { $($variant => $text),+ })
            }
        }
    };
}

display_enum!(TargetAddressSpaceV1, {
    TargetAddressSpaceV1::Global => "global",
    TargetAddressSpaceV1::Workgroup => "workgroup",
    TargetAddressSpaceV1::Private => "private",
    TargetAddressSpaceV1::Constant => "constant",
    TargetAddressSpaceV1::Generic => "generic",
});
display_enum!(TargetMemoryAccessV1, {
    TargetMemoryAccessV1::Read => "read",
    TargetMemoryAccessV1::Write => "write",
    TargetMemoryAccessV1::ReadWrite => "read-write",
});
display_enum!(TargetScalarKindV1, {
    TargetScalarKindV1::Boolean => "boolean",
    TargetScalarKindV1::SignedInteger => "signed-integer",
    TargetScalarKindV1::UnsignedInteger => "unsigned-integer",
    TargetScalarKindV1::Float => "float",
});
display_enum!(TargetScalarEncodingV1, {
    TargetScalarEncodingV1::Boolean => "boolean",
    TargetScalarEncodingV1::TwosComplement => "twos-complement",
    TargetScalarEncodingV1::IeeeBinary => "ieee-binary",
    TargetScalarEncodingV1::BFloat => "bfloat",
    TargetScalarEncodingV1::Float4E2M1Ocp => "float4-e2m1-ocp",
    TargetScalarEncodingV1::Float8E4M3Fnuz => "float8-e4m3-fnuz",
    TargetScalarEncodingV1::Float8E5M2Fnuz => "float8-e5m2-fnuz",
    TargetScalarEncodingV1::Float8E4M3Ocp => "float8-e4m3-ocp",
    TargetScalarEncodingV1::Float8E5M2Ocp => "float8-e5m2-ocp",
});
display_enum!(TargetAtomicOperationV1, {
    TargetAtomicOperationV1::Load => "load",
    TargetAtomicOperationV1::Store => "store",
    TargetAtomicOperationV1::Exchange => "exchange",
    TargetAtomicOperationV1::CompareExchange => "compare-exchange",
    TargetAtomicOperationV1::Add => "add",
    TargetAtomicOperationV1::Sub => "sub",
    TargetAtomicOperationV1::Min => "min",
    TargetAtomicOperationV1::Max => "max",
    TargetAtomicOperationV1::And => "and",
    TargetAtomicOperationV1::Nand => "nand",
    TargetAtomicOperationV1::Or => "or",
    TargetAtomicOperationV1::Xor => "xor",
});
display_enum!(TargetMatrixOperationV1, {
    TargetMatrixOperationV1::MatrixMultiplyAccumulate => "matrix-multiply-accumulate",
    TargetMatrixOperationV1::ScaledMatrixMultiplyAccumulate => "scaled-matrix-multiply-accumulate",
    TargetMatrixOperationV1::TensorContraction => "tensor-contraction",
});
display_enum!(TargetMemoryOrderingV1, {
    TargetMemoryOrderingV1::Relaxed => "relaxed",
    TargetMemoryOrderingV1::Acquire => "acquire",
    TargetMemoryOrderingV1::Release => "release",
    TargetMemoryOrderingV1::AcquireRelease => "acquire-release",
    TargetMemoryOrderingV1::SequentiallyConsistent => "sequentially-consistent",
});
display_enum!(TargetMemoryScopeV1, {
    TargetMemoryScopeV1::Invocation => "invocation",
    TargetMemoryScopeV1::Subgroup => "subgroup",
    TargetMemoryScopeV1::Workgroup => "workgroup",
    TargetMemoryScopeV1::Device => "device",
    TargetMemoryScopeV1::System => "system",
});
display_enum!(TargetExecutionScopeV1, {
    TargetExecutionScopeV1::Subgroup => "subgroup",
    TargetExecutionScopeV1::Workgroup => "workgroup",
    TargetExecutionScopeV1::Grid => "grid",
});
display_enum!(TargetBarrierParticipationV1, {
    TargetBarrierParticipationV1::Uniform => "uniform",
    TargetBarrierParticipationV1::DynamicMask => "dynamic-mask",
});
display_enum!(TargetCollectiveOperationV1, {
    TargetCollectiveOperationV1::Ballot => "ballot",
    TargetCollectiveOperationV1::Any => "any",
    TargetCollectiveOperationV1::All => "all",
    TargetCollectiveOperationV1::Broadcast => "broadcast",
    TargetCollectiveOperationV1::ReduceAdd => "reduce-add",
    TargetCollectiveOperationV1::ReduceMin => "reduce-min",
    TargetCollectiveOperationV1::ReduceMax => "reduce-max",
    TargetCollectiveOperationV1::InclusiveScanAdd => "inclusive-scan-add",
    TargetCollectiveOperationV1::ExclusiveScanAdd => "exclusive-scan-add",
});
display_enum!(TargetMatrixLayoutV1, {
    TargetMatrixLayoutV1::RowMajor => "row-major",
    TargetMatrixLayoutV1::ColumnMajor => "column-major",
    TargetMatrixLayoutV1::CooperativeFragment => "cooperative-fragment",
});
display_enum!(TargetNumericalModeV1, {
    TargetNumericalModeV1::IeeeStrict => "ieee-strict",
    TargetNumericalModeV1::AllowContraction => "allow-contraction",
    TargetNumericalModeV1::AllowApproximation => "allow-approximation",
});
display_enum!(TargetEndiannessV1, {
    TargetEndiannessV1::Little => "little",
    TargetEndiannessV1::Big => "big",
});
display_enum!(TargetLaunchEvidenceKindV1, {
    TargetLaunchEvidenceKindV1::DynamicSharedMemoryBytes => "dynamic-shared-memory-bytes",
    TargetLaunchEvidenceKindV1::WorkgroupDimensions => "workgroup-dimensions",
    TargetLaunchEvidenceKindV1::CooperativeGridAdmission => "cooperative-grid-admission",
    TargetLaunchEvidenceKindV1::SystemAtomicMemoryEligibility => "system-atomic-memory-eligibility",
});

impl fmt::Display for TargetScalarTypeV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}:{}:{}",
            self.kind, self.bit_width, self.encoding
        )
    }
}

impl fmt::Display for TargetCollectiveParticipationV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Full => formatter.write_str("full"),
            Self::UniformPrefix(active) => write!(formatter, "uniform-prefix:{active}"),
            Self::DynamicMask => formatter.write_str("dynamic-mask"),
        }
    }
}

impl fmt::Display for TargetCapabilityRequirementV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ScalarType(scalar) => write!(formatter, "scalar-type:{scalar}"),
            Self::SubgroupSize(lanes) => write!(formatter, "subgroup-size:{lanes}"),
            Self::AddressSpace(space, access) => {
                write!(formatter, "address-space:{space}:{access}")
            }
            Self::Atomic(value) => write!(
                formatter,
                "atomic:{}:{}:{}:{}:{}:{}",
                value.value_type,
                value.operation,
                value.ordering,
                DisplayOptionalOrdering(value.failure_ordering),
                value.scope,
                value.address_space
            ),
            Self::Barrier(value) => write!(
                formatter,
                "barrier:{}:{}:{}:{}:{}",
                value.execution_scope,
                value.memory_scope,
                value.address_space,
                value.ordering,
                value.participation
            ),
            Self::Fence(value) => write!(
                formatter,
                "fence:{}:{}:{}",
                value.memory_scope, value.address_space, value.ordering
            ),
            Self::Collective(value) => write!(
                formatter,
                "collective:{}:{}:{}:{}:{}:{}",
                value.execution_scope,
                value.operation,
                value.value_type,
                value.participants,
                value.participation,
                DisplayOptionalNumericalMode(value.numerical_mode)
            ),
            Self::Matrix(value) => write!(
                formatter,
                "matrix:{}:{}x{}x{}:{}:{}:{}:{}:{}:{}:{}:{}:{}",
                value.operation(),
                value.m(),
                value.n(),
                value.k(),
                value.lhs_type(),
                value.rhs_type(),
                value.accumulator_type(),
                value.lhs_layout(),
                value.rhs_layout(),
                value.output_layout(),
                value.numerical_mode(),
                value.subgroup_size(),
                value.active_participants()
            ),
            Self::AsyncCopy(value) => write!(
                formatter,
                "async-copy:{}:{}:{}:{}",
                value.source, value.destination, value.bytes, value.alignment
            ),
            Self::AsyncWait(value) => write!(
                formatter,
                "async-wait:{}:{}:{}:{}",
                value.execution_scope, value.memory_scope, value.ordering, value.max_pending_groups
            ),
            Self::Numerical(value) => {
                write!(formatter, "numerical:{}:{}", value.value_type, value.mode)
            }
            Self::Resource(value) => write!(formatter, "resource:{value}"),
            Self::Abi(value) => write!(formatter, "abi:{value}"),
            Self::Object(value) => write!(formatter, "object:{value}"),
        }
    }
}

impl fmt::Display for TargetResourceRequirementV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WorkgroupInvocationsAtMost(value) => {
                write!(formatter, "workgroup-invocations-at-most:{value}")
            }
            Self::StaticSharedMemoryBytesAtMost(value) => {
                write!(formatter, "static-shared-memory-bytes-at-most:{value}")
            }
            Self::DynamicSharedMemoryBytesAtMost(value) => {
                write!(formatter, "dynamic-shared-memory-bytes-at-most:{value}")
            }
            Self::PrivateMemoryBytesPerInvocationAtMost(value) => {
                write!(
                    formatter,
                    "private-memory-bytes-per-invocation-at-most:{value}"
                )
            }
            Self::WorkgroupDimensions { x, y, z } => {
                write!(formatter, "workgroup-dimensions:{x}x{y}x{z}")
            }
            Self::SubgroupsPerWorkgroupAtMost(value) => {
                write!(formatter, "subgroups-per-workgroup-at-most:{value}")
            }
            Self::RegistersPerInvocationAtMost(value) => {
                write!(formatter, "registers-per-invocation-at-most:{value}")
            }
        }
    }
}

impl fmt::Display for TargetAbiConstraintV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PointerWidth(value) => write!(formatter, "pointer-width:{value}"),
            Self::Endianness(value) => write!(formatter, "endianness:{value}"),
            Self::KernelArgumentAlignmentAtMost(value) => {
                write!(formatter, "kernel-argument-alignment-at-most:{value}")
            }
            Self::KernelArgumentSegmentBytesAtMost(value) => {
                write!(formatter, "kernel-argument-segment-bytes-at-most:{value}")
            }
        }
    }
}

impl fmt::Display for TargetObjectConstraintV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Format(value) => write!(formatter, "format:{value}"),
            Self::Relocatable(value) => write!(formatter, "relocatable:{value}"),
        }
    }
}

display_enum!(TargetObjectFormatV1, {
    TargetObjectFormatV1::LoadableExecutable => "loadable-executable",
    TargetObjectFormatV1::RelocatableObject => "relocatable-object",
    TargetObjectFormatV1::PortableModule => "portable-module",
});

struct DisplayOptionalOrdering(Option<TargetMemoryOrderingV1>);

impl fmt::Display for DisplayOptionalOrdering {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(ordering) => write!(formatter, "{ordering}"),
            None => formatter.write_str("<none>"),
        }
    }
}

struct DisplayOptionalNumericalMode(Option<TargetNumericalModeV1>);

impl fmt::Display for DisplayOptionalNumericalMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(mode) => write!(formatter, "{mode}"),
            None => formatter.write_str("<none>"),
        }
    }
}

impl fmt::Display for TargetCapabilityDecisionOutcomeV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Supported => formatter.write_str("supported"),
            Self::Unsupported => formatter.write_str("unsupported"),
            Self::Incomplete => formatter.write_str("incomplete"),
            Self::Unreviewed => formatter.write_str("unreviewed"),
            Self::DynamicLaunchEvidenceRequired(kind) => {
                write!(formatter, "dynamic-launch-evidence-required:{kind}")
            }
        }
    }
}
