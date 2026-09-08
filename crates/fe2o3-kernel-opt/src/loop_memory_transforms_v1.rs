//! Checked, bounded target-neutral loop and memory transformations.
//!
//! The accepted subset is intentionally narrow. Loop rewrites require a
//! reducible natural loop and exact SSA edge arguments. Unrolling additionally
//! requires a statically proved unsigned-index trip count. Memory forwarding
//! uses exact allocation/projection facts propagated through the CFG. Facts
//! must agree on every incoming edge after a bounded loop fixed point. Every
//! volatile, atomic, synchronization, convergent, assembly, capability, or
//! unknown-call operation starts a fresh synchronization epoch. Writable
//! shared-memory facts additionally require a canonical single-invocation
//! proof; broader inter-invocation cases remain fail closed.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId, CastKind, ComparePredicate, Constant,
    ControlFlowError, Function, FunctionId, GlobalCapabilityRoleV1, IndexedControlFlow,
    MemoryAccess, Module, Operation, OperationKind, Terminator, Type, ValueDef, ValueId,
    analyze_control_flow, verify_module,
};

use crate::{
    BoundedTargetNeutralCostModelV1, OptimizerQueryFailureV1, TargetNeutralCostDecisionV1,
    TargetNeutralCostModelV1, TargetNeutralCostQueryV1, TargetNeutralCostRejectionV1,
    TargetNeutralCostTransformV1, TargetNeutralModuleTransformErrorV1,
    find_dominating_index_equals_constant_guard_v1, find_dominating_unsigned_less_than_guard_v1,
    fresh_value_id, remap_supported_operation, remap_terminator,
};

pub const HARD_MAX_LOOP_MEMORY_BLOCKS_V1: usize = 65_536;
pub const HARD_MAX_LOOP_MEMORY_OPERATIONS_V1: usize = 1_000_000;
pub const HARD_MAX_NATURAL_LOOPS_V1: usize = 16_384;
pub const HARD_MAX_LOOP_MEMBERS_V1: usize = 65_536;
pub const HARD_MAX_INSERTED_LOOP_BLOCKS_V1: usize = 16_384;
pub const HARD_MAX_FULL_UNROLL_TRIP_COUNT_V1: u64 = 32;
pub const HARD_MAX_PARTIAL_UNROLL_TRIP_COUNT_V1: u64 = 65_536;
pub const HARD_MAX_UNROLL_GROWTH_OPERATIONS_V1: usize = 65_536;
pub const HARD_MAX_MEMORY_VERSION_EVENTS_V1: usize = 1_000_000;
pub const HARD_MAX_MEMORY_DATAFLOW_WORK_V1: usize = 16_777_216;
pub const HARD_MAX_MEMORY_DATAFLOW_ITERATIONS_V1: usize = 4_096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LoopMemoryTransformLimitsV1 {
    max_blocks: usize,
    max_operations: usize,
    max_natural_loops: usize,
    max_loop_members: usize,
    max_inserted_loop_blocks: usize,
    max_full_unroll_trip_count: u64,
    max_partial_unroll_trip_count: u64,
    partial_unroll_factor: u32,
    max_unroll_growth_operations: usize,
    max_memory_version_events: usize,
    max_memory_dataflow_work: usize,
    max_memory_dataflow_iterations: usize,
}

impl LoopMemoryTransformLimitsV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        max_blocks: usize,
        max_operations: usize,
        max_natural_loops: usize,
        max_loop_members: usize,
        max_inserted_loop_blocks: usize,
        max_full_unroll_trip_count: u64,
        max_partial_unroll_trip_count: u64,
        partial_unroll_factor: u32,
        max_unroll_growth_operations: usize,
        max_memory_version_events: usize,
    ) -> Result<Self, LoopMemoryTransformErrorV1> {
        let limits = Self {
            max_blocks,
            max_operations,
            max_natural_loops,
            max_loop_members,
            max_inserted_loop_blocks,
            max_full_unroll_trip_count,
            max_partial_unroll_trip_count,
            partial_unroll_factor,
            max_unroll_growth_operations,
            max_memory_version_events,
            max_memory_dataflow_work: HARD_MAX_MEMORY_DATAFLOW_WORK_V1,
            max_memory_dataflow_iterations: 128,
        };
        limits.validate()?;
        Ok(limits)
    }

    fn validate(self) -> Result<(), LoopMemoryTransformErrorV1> {
        let valid = self.max_blocks != 0
            && self.max_blocks <= HARD_MAX_LOOP_MEMORY_BLOCKS_V1
            && self.max_operations != 0
            && self.max_operations <= HARD_MAX_LOOP_MEMORY_OPERATIONS_V1
            && self.max_natural_loops != 0
            && self.max_natural_loops <= HARD_MAX_NATURAL_LOOPS_V1
            && self.max_loop_members != 0
            && self.max_loop_members <= HARD_MAX_LOOP_MEMBERS_V1
            && self.max_inserted_loop_blocks != 0
            && self.max_inserted_loop_blocks <= HARD_MAX_INSERTED_LOOP_BLOCKS_V1
            && self.max_full_unroll_trip_count <= HARD_MAX_FULL_UNROLL_TRIP_COUNT_V1
            && self.max_partial_unroll_trip_count >= self.max_full_unroll_trip_count
            && self.max_partial_unroll_trip_count <= HARD_MAX_PARTIAL_UNROLL_TRIP_COUNT_V1
            && self.partial_unroll_factor >= 2
            && self.partial_unroll_factor <= 16
            && self.max_unroll_growth_operations != 0
            && self.max_unroll_growth_operations <= HARD_MAX_UNROLL_GROWTH_OPERATIONS_V1
            && self.max_memory_version_events != 0
            && self.max_memory_version_events <= HARD_MAX_MEMORY_VERSION_EVENTS_V1
            && self.max_memory_dataflow_work != 0
            && self.max_memory_dataflow_work <= HARD_MAX_MEMORY_DATAFLOW_WORK_V1
            && self.max_memory_dataflow_iterations != 0
            && self.max_memory_dataflow_iterations <= HARD_MAX_MEMORY_DATAFLOW_ITERATIONS_V1;
        valid
            .then_some(())
            .ok_or(LoopMemoryTransformErrorV1::InvalidLimits)
    }

    pub fn with_memory_dataflow_work_limit(
        mut self,
        max_memory_dataflow_work: usize,
    ) -> Result<Self, LoopMemoryTransformErrorV1> {
        self.max_memory_dataflow_work = max_memory_dataflow_work;
        self.validate()?;
        Ok(self)
    }

    pub const fn max_memory_dataflow_work(self) -> usize {
        self.max_memory_dataflow_work
    }

    pub fn with_memory_dataflow_iteration_limit(
        mut self,
        max_memory_dataflow_iterations: usize,
    ) -> Result<Self, LoopMemoryTransformErrorV1> {
        self.max_memory_dataflow_iterations = max_memory_dataflow_iterations;
        self.validate()?;
        Ok(self)
    }

    pub const fn max_memory_dataflow_iterations(self) -> usize {
        self.max_memory_dataflow_iterations
    }
}

impl Default for LoopMemoryTransformLimitsV1 {
    fn default() -> Self {
        Self {
            max_blocks: HARD_MAX_LOOP_MEMORY_BLOCKS_V1,
            max_operations: HARD_MAX_LOOP_MEMORY_OPERATIONS_V1,
            max_natural_loops: HARD_MAX_NATURAL_LOOPS_V1,
            max_loop_members: HARD_MAX_LOOP_MEMBERS_V1,
            max_inserted_loop_blocks: HARD_MAX_INSERTED_LOOP_BLOCKS_V1,
            max_full_unroll_trip_count: 8,
            max_partial_unroll_trip_count: 1_024,
            partial_unroll_factor: 2,
            max_unroll_growth_operations: HARD_MAX_UNROLL_GROWTH_OPERATIONS_V1,
            max_memory_version_events: HARD_MAX_MEMORY_VERSION_EVENTS_V1,
            max_memory_dataflow_work: HARD_MAX_MEMORY_DATAFLOW_WORK_V1,
            max_memory_dataflow_iterations: 128,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LoopMemoryTransformV1 {
    LoopCanonicalization,
    InductionVariableSimplification,
    LoopInvariantCodeMotion,
    MemoryEffectVersioning,
    MemorySimplification,
    FullLoopUnrolling,
    PartialLoopUnrolling,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct OperationCoordinateV1 {
    function: FunctionId,
    block: BlockId,
    operation: u32,
}

impl OperationCoordinateV1 {
    pub(crate) fn new(
        function: &FunctionId,
        block: BlockId,
        operation: usize,
    ) -> Result<Self, LoopMemoryTransformErrorV1> {
        Ok(Self {
            function: function.clone(),
            block,
            operation: u32::try_from(operation)
                .map_err(|_| LoopMemoryTransformErrorV1::CoordinateOverflow)?,
        })
    }

    pub const fn function(&self) -> &FunctionId {
        &self.function
    }

    pub const fn block(&self) -> BlockId {
        self.block
    }

    pub const fn operation(&self) -> u32 {
        self.operation
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CanonicalLoopBlockRoleV1 {
    Preheader,
    Latch,
    DedicatedExit,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MemoryForwardKindV1 {
    StoreToLoad,
    LoadToLoad,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MemorySynchronizationEpochV1 {
    Root,
    Boundary(OperationCoordinateV1),
    Ambiguous,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OperationCoordinateLineageV1 {
    Retained {
        before: OperationCoordinateV1,
        after: OperationCoordinateV1,
    },
    Moved {
        before: OperationCoordinateV1,
        after: OperationCoordinateV1,
    },
    Duplicated {
        source: OperationCoordinateV1,
        copy: OperationCoordinateV1,
    },
    Eliminated {
        before: OperationCoordinateV1,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LoopMemoryEditV1 {
    CanonicalBlockInserted {
        function: FunctionId,
        header: BlockId,
        block: BlockId,
        target: BlockId,
        role: CanonicalLoopBlockRoleV1,
        redirected_sources: Vec<BlockId>,
    },
    InductionUpdateNormalized {
        function: FunctionId,
        header: BlockId,
        update: OperationCoordinateV1,
    },
    OperationMoved {
        source: OperationCoordinateV1,
        destination_block: BlockId,
    },
    LoopFullyUnrolled {
        function: FunctionId,
        header: BlockId,
        body: BlockId,
        trip_count: u64,
        copied_operations: usize,
    },
    LoopPartiallyUnrolled {
        function: FunctionId,
        header: BlockId,
        body: BlockId,
        factor: u32,
        copied_operations: usize,
    },
    MemoryValueForwarded {
        producer: OperationCoordinateV1,
        load: OperationCoordinateV1,
        kind: MemoryForwardKindV1,
        synchronization_epoch: MemorySynchronizationEpochV1,
    },
    DeadStoreEliminated {
        store: OperationCoordinateV1,
        overwriting_store: OperationCoordinateV1,
        synchronization_epoch: MemorySynchronizationEpochV1,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InductionVariableFactV1 {
    function: FunctionId,
    header: BlockId,
    preheader: BlockId,
    latch: BlockId,
    body: BlockId,
    exit: BlockId,
    parameter: ValueId,
    start_value: ValueId,
    bound_value: ValueId,
    start: u64,
    step: u64,
    bound: Option<u64>,
    trip_count: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct AffineIndexTermV1 {
    value: ValueId,
    coefficient: i128,
}

impl AffineIndexTermV1 {
    pub const fn value(self) -> ValueId {
        self.value
    }

    pub const fn coefficient(self) -> i128 {
        self.coefficient
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AffineIndexExpressionV1 {
    constant: i128,
    terms: Vec<AffineIndexTermV1>,
}

impl AffineIndexExpressionV1 {
    pub const fn constant(&self) -> i128 {
        self.constant
    }

    pub fn terms(&self) -> &[AffineIndexTermV1] {
        &self.terms
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DynamicTripProofV1 {
    function: FunctionId,
    header: BlockId,
    bound_value: ValueId,
    expression: AffineIndexExpressionV1,
    exact_bound: u64,
    guard: crate::OptimizerQueryCoordinateV1,
}

impl DynamicTripProofV1 {
    pub const fn function(&self) -> &FunctionId {
        &self.function
    }

    pub const fn header(&self) -> BlockId {
        self.header
    }

    pub const fn bound_value(&self) -> ValueId {
        self.bound_value
    }

    pub const fn expression(&self) -> &AffineIndexExpressionV1 {
        &self.expression
    }

    pub const fn exact_bound(&self) -> u64 {
        self.exact_bound
    }

    pub const fn guard(&self) -> &crate::OptimizerQueryCoordinateV1 {
        &self.guard
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnsupportedDynamicTripExpansionV1 {
    function: FunctionId,
    header: BlockId,
    bound_value: ValueId,
}

impl UnsupportedDynamicTripExpansionV1 {
    pub const fn function(&self) -> &FunctionId {
        &self.function
    }

    pub const fn header(&self) -> BlockId {
        self.header
    }

    pub const fn bound_value(&self) -> ValueId {
        self.bound_value
    }

    pub const fn reason(&self) -> &'static str {
        "dynamic expansion has no statically exact bounded trip count"
    }
}

impl InductionVariableFactV1 {
    pub const fn function(&self) -> &FunctionId {
        &self.function
    }

    pub const fn header(&self) -> BlockId {
        self.header
    }

    pub const fn preheader(&self) -> BlockId {
        self.preheader
    }

    pub const fn latch(&self) -> BlockId {
        self.latch
    }

    pub const fn body(&self) -> BlockId {
        self.body
    }

    pub const fn exit(&self) -> BlockId {
        self.exit
    }

    pub const fn parameter(&self) -> ValueId {
        self.parameter
    }

    pub const fn start_value(&self) -> ValueId {
        self.start_value
    }

    pub const fn bound_value(&self) -> ValueId {
        self.bound_value
    }

    pub const fn start(&self) -> u64 {
        self.start
    }

    pub const fn step(&self) -> u64 {
        self.step
    }

    pub const fn bound(&self) -> Option<u64> {
        self.bound
    }

    pub const fn trip_count(&self) -> Option<u64> {
        self.trip_count
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MemoryLocationV1 {
    pointer: ValueId,
    root: ValueId,
    constant_projection: Option<Vec<u64>>,
    address_space: AddressSpace,
    private_allocation: bool,
    workgroup_allocation: bool,
    exclusive_global: bool,
    read_only_external: bool,
}

impl MemoryLocationV1 {
    pub const fn pointer(&self) -> ValueId {
        self.pointer
    }

    pub const fn root(&self) -> ValueId {
        self.root
    }

    pub fn constant_projection(&self) -> Option<&[u64]> {
        self.constant_projection.as_deref()
    }

    pub const fn address_space(&self) -> AddressSpace {
        self.address_space
    }

    pub const fn is_private_allocation(&self) -> bool {
        self.private_allocation
    }

    pub const fn is_workgroup_allocation(&self) -> bool {
        self.workgroup_allocation
    }

    pub const fn is_exclusive_global(&self) -> bool {
        self.exclusive_global
    }

    pub const fn has_exact_writable_provenance(&self) -> bool {
        self.private_allocation || self.workgroup_allocation || self.exclusive_global
    }

    pub const fn is_read_only_external(&self) -> bool {
        self.read_only_external
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MemoryVersionEventKindV1 {
    Load,
    Store,
    VolatileLoadBoundary,
    VolatileStoreBoundary,
    AtomicBoundary,
    BarrierBoundary,
    FenceBoundary,
    ConvergentBoundary,
    UnknownCallBoundary,
    UnknownEffectBoundary,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryVersionEventV1 {
    site: OperationCoordinateV1,
    before: u64,
    after: u64,
    kind: MemoryVersionEventKindV1,
    location: Option<MemoryLocationV1>,
}

impl MemoryVersionEventV1 {
    pub fn site(&self) -> OperationCoordinateV1 {
        self.site.clone()
    }

    pub const fn before(&self) -> u64 {
        self.before
    }

    pub const fn after(&self) -> u64 {
        self.after
    }

    pub const fn kind(&self) -> MemoryVersionEventKindV1 {
        self.kind
    }

    pub const fn location(&self) -> Option<&MemoryLocationV1> {
        self.location.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryBlockVersionV1 {
    function: FunctionId,
    block: BlockId,
    entry: u64,
    exit: u64,
}

impl MemoryBlockVersionV1 {
    pub const fn function(&self) -> &FunctionId {
        &self.function
    }

    pub const fn block(&self) -> BlockId {
        self.block
    }

    pub const fn entry(&self) -> u64 {
        self.entry
    }

    pub const fn exit(&self) -> u64 {
        self.exit
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IncomingMemoryVersionV1 {
    predecessor: BlockId,
    version: u64,
}

impl IncomingMemoryVersionV1 {
    pub const fn predecessor(self) -> BlockId {
        self.predecessor
    }

    pub const fn version(self) -> u64 {
        self.version
    }
}

/// The effect version entering a block and the exact predecessor-exit
/// versions it joins. An empty input list denotes a CFG root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryBlockEntryVersionV1 {
    function: FunctionId,
    block: BlockId,
    version: u64,
    incoming: Vec<IncomingMemoryVersionV1>,
}

impl MemoryBlockEntryVersionV1 {
    pub const fn function(&self) -> &FunctionId {
        &self.function
    }

    pub const fn block(&self) -> BlockId {
        self.block
    }

    pub const fn version(&self) -> u64 {
        self.version
    }

    pub fn incoming(&self) -> &[IncomingMemoryVersionV1] {
        &self.incoming
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VersionedMemoryEffectGraphV1 {
    block_versions: Vec<MemoryBlockVersionV1>,
    block_entries: Vec<MemoryBlockEntryVersionV1>,
    events: Vec<MemoryVersionEventV1>,
    final_version: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoopCarriedMemoryPhiV1 {
    function: FunctionId,
    block: BlockId,
    predecessors: Vec<BlockId>,
    synchronization_epoch: MemorySynchronizationEpochV1,
    available_producers: Vec<OperationCoordinateV1>,
}

impl LoopCarriedMemoryPhiV1 {
    pub const fn function(&self) -> &FunctionId {
        &self.function
    }

    pub const fn block(&self) -> BlockId {
        self.block
    }

    pub fn predecessors(&self) -> &[BlockId] {
        &self.predecessors
    }

    pub const fn synchronization_epoch(&self) -> &MemorySynchronizationEpochV1 {
        &self.synchronization_epoch
    }

    pub fn available_producers(&self) -> &[OperationCoordinateV1] {
        &self.available_producers
    }
}

impl VersionedMemoryEffectGraphV1 {
    pub fn block_versions(&self) -> &[MemoryBlockVersionV1] {
        &self.block_versions
    }

    pub fn block_entries(&self) -> &[MemoryBlockEntryVersionV1] {
        &self.block_entries
    }

    pub fn events(&self) -> &[MemoryVersionEventV1] {
        &self.events
    }

    pub const fn final_version(&self) -> u64 {
        self.final_version
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoopMemoryTransformReportV1 {
    transform: LoopMemoryTransformV1,
    changed: bool,
    edits: Vec<LoopMemoryEditV1>,
    induction_facts: Vec<InductionVariableFactV1>,
    dynamic_trip_proofs: Vec<DynamicTripProofV1>,
    unsupported_dynamic_trip_expansions: Vec<UnsupportedDynamicTripExpansionV1>,
    memory_graph: Option<VersionedMemoryEffectGraphV1>,
    loop_carried_memory_phis: Vec<LoopCarriedMemoryPhiV1>,
    cost_decisions: Vec<TargetNeutralCostDecisionV1>,
    coordinate_lineage: Vec<OperationCoordinateLineageV1>,
    inserted_blocks: usize,
    inserted_operations: usize,
    eliminated_operations: usize,
}

impl LoopMemoryTransformReportV1 {
    pub const fn transform(&self) -> LoopMemoryTransformV1 {
        self.transform
    }

    pub const fn changed(&self) -> bool {
        self.changed
    }

    pub fn edits(&self) -> &[LoopMemoryEditV1] {
        &self.edits
    }

    pub fn induction_facts(&self) -> &[InductionVariableFactV1] {
        &self.induction_facts
    }

    pub fn dynamic_trip_proofs(&self) -> &[DynamicTripProofV1] {
        &self.dynamic_trip_proofs
    }

    pub fn unsupported_dynamic_trip_expansions(&self) -> &[UnsupportedDynamicTripExpansionV1] {
        &self.unsupported_dynamic_trip_expansions
    }

    pub const fn memory_graph(&self) -> Option<&VersionedMemoryEffectGraphV1> {
        self.memory_graph.as_ref()
    }

    pub fn loop_carried_memory_phis(&self) -> &[LoopCarriedMemoryPhiV1] {
        &self.loop_carried_memory_phis
    }

    pub fn cost_decisions(&self) -> &[TargetNeutralCostDecisionV1] {
        &self.cost_decisions
    }

    pub fn coordinate_lineage(&self) -> &[OperationCoordinateLineageV1] {
        &self.coordinate_lineage
    }

    pub const fn inserted_blocks(&self) -> usize {
        self.inserted_blocks
    }

    pub const fn inserted_operations(&self) -> usize {
        self.inserted_operations
    }

    pub const fn eliminated_operations(&self) -> usize {
        self.eliminated_operations
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum LoopMemoryTransformErrorV1 {
    InvalidLimits,
    InputRejected,
    OutputRejected,
    ControlFlow(ControlFlowError),
    OptimizerQuery(OptimizerQueryFailureV1),
    BlockLimitExceeded { required: usize, limit: usize },
    OperationLimitExceeded { required: usize, limit: usize },
    LoopLimitExceeded { required: usize, limit: usize },
    LoopMemberLimitExceeded { required: usize, limit: usize },
    InsertedBlockLimitExceeded { limit: usize },
    UnrollGrowthLimitExceeded { required: usize, limit: usize },
    MemoryEventLimitExceeded { limit: usize },
    MemoryDataflowWorkLimitExceeded { required: usize, limit: usize },
    MemoryDataflowIterationLimitExceeded { limit: usize },
    IdentityOverflow,
    CoordinateOverflow,
    Remap(TargetNeutralModuleTransformErrorV1),
}

impl fmt::Display for LoopMemoryTransformErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLimits => formatter.write_str("loop/memory transform limits are invalid"),
            Self::InputRejected => {
                formatter.write_str("loop/memory input is invalid canonical KIR")
            }
            Self::OutputRejected => {
                formatter.write_str("loop/memory output or reconstructed relation was rejected")
            }
            Self::ControlFlow(error) => error.fmt(formatter),
            Self::OptimizerQuery(error) => error.fmt(formatter),
            Self::BlockLimitExceeded { required, limit } => {
                write!(
                    formatter,
                    "module has {required} blocks but the limit is {limit}"
                )
            }
            Self::OperationLimitExceeded { required, limit } => write!(
                formatter,
                "module has {required} operations but the limit is {limit}"
            ),
            Self::LoopLimitExceeded { required, limit } => {
                write!(
                    formatter,
                    "function has {required} loops but the limit is {limit}"
                )
            }
            Self::LoopMemberLimitExceeded { required, limit } => write!(
                formatter,
                "natural loop has {required} blocks but the limit is {limit}"
            ),
            Self::InsertedBlockLimitExceeded { limit } => write!(
                formatter,
                "loop canonicalization exceeded its {limit}-block insertion limit"
            ),
            Self::UnrollGrowthLimitExceeded { required, limit } => write!(
                formatter,
                "loop unrolling requires {required} operations but the growth limit is {limit}"
            ),
            Self::MemoryEventLimitExceeded { limit } => write!(
                formatter,
                "memory version construction exceeded its {limit}-event limit"
            ),
            Self::MemoryDataflowWorkLimitExceeded { required, limit } => write!(
                formatter,
                "memory dataflow requires {required} work units but the limit is {limit}"
            ),
            Self::MemoryDataflowIterationLimitExceeded { limit } => write!(
                formatter,
                "memory dataflow did not converge within its {limit}-iteration limit"
            ),
            Self::IdentityOverflow => formatter.write_str("loop/memory identity space overflowed"),
            Self::CoordinateOverflow => {
                formatter.write_str("loop/memory operation coordinate overflowed")
            }
            Self::Remap(error) => error.fmt(formatter),
        }
    }
}

impl Error for LoopMemoryTransformErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ControlFlow(error) => Some(error),
            Self::OptimizerQuery(error) => Some(error),
            Self::Remap(error) => Some(error),
            _ => None,
        }
    }
}

pub fn execute_loop_memory_transform_v1(
    transform: LoopMemoryTransformV1,
    input: &Module,
    limits: LoopMemoryTransformLimitsV1,
) -> Result<(Module, LoopMemoryTransformReportV1), LoopMemoryTransformErrorV1> {
    limits.validate()?;
    verify_module(input).map_err(|_| LoopMemoryTransformErrorV1::InputRejected)?;
    enforce_limits(input, limits)?;
    let mut output = input.clone();
    let mut report = LoopMemoryTransformReportV1 {
        transform,
        changed: false,
        edits: Vec::new(),
        induction_facts: Vec::new(),
        dynamic_trip_proofs: Vec::new(),
        unsupported_dynamic_trip_expansions: Vec::new(),
        memory_graph: None,
        loop_carried_memory_phis: Vec::new(),
        cost_decisions: Vec::new(),
        coordinate_lineage: Vec::new(),
        inserted_blocks: 0,
        inserted_operations: 0,
        eliminated_operations: 0,
    };

    match transform {
        LoopMemoryTransformV1::LoopCanonicalization => {
            canonicalize_natural_loops(&mut output, limits, &mut report)?;
        }
        LoopMemoryTransformV1::InductionVariableSimplification => {
            simplify_induction_variables(&mut output, limits, &mut report)?;
        }
        LoopMemoryTransformV1::LoopInvariantCodeMotion => {
            hoist_loop_invariants(&mut output, limits, &mut report)?;
        }
        LoopMemoryTransformV1::MemoryEffectVersioning => {
            report.memory_graph = Some(build_versioned_memory_effect_graph_v1(&output, limits)?);
        }
        LoopMemoryTransformV1::MemorySimplification => {
            report.memory_graph = Some(build_versioned_memory_effect_graph_v1(&output, limits)?);
            simplify_versioned_memory(&mut output, limits, &mut report)?;
        }
        LoopMemoryTransformV1::FullLoopUnrolling => {
            unroll_static_loops(&mut output, limits, true, &mut report)?;
        }
        LoopMemoryTransformV1::PartialLoopUnrolling => {
            unroll_static_loops(&mut output, limits, false, &mut report)?;
        }
    }

    enforce_limits(&output, limits)?;
    verify_module(&output).map_err(|_| LoopMemoryTransformErrorV1::OutputRejected)?;
    report.changed = &output != input;
    Ok((output, report))
}

/// Re-executes the deterministic relation from immutable input. Neither a pass
/// change bit nor the supplied report participates in acceptance.
pub fn check_loop_memory_transform_relation_v1(
    transform: LoopMemoryTransformV1,
    before: &Module,
    after: &Module,
    limits: LoopMemoryTransformLimitsV1,
) -> Result<LoopMemoryTransformReportV1, LoopMemoryTransformErrorV1> {
    let (expected, report) = execute_loop_memory_transform_v1(transform, before, limits)?;
    (expected == *after)
        .then_some(report)
        .ok_or(LoopMemoryTransformErrorV1::OutputRejected)
}

fn enforce_limits(
    module: &Module,
    limits: LoopMemoryTransformLimitsV1,
) -> Result<(), LoopMemoryTransformErrorV1> {
    let blocks = module
        .functions
        .iter()
        .filter_map(|function| function.body.as_ref())
        .map(|body| body.blocks.len())
        .sum::<usize>();
    if blocks > limits.max_blocks {
        return Err(LoopMemoryTransformErrorV1::BlockLimitExceeded {
            required: blocks,
            limit: limits.max_blocks,
        });
    }
    let operations = module
        .functions
        .iter()
        .filter_map(|function| function.body.as_ref())
        .flat_map(|body| &body.blocks)
        .map(|block| block.operations.len())
        .sum::<usize>();
    if operations > limits.max_operations {
        return Err(LoopMemoryTransformErrorV1::OperationLimitExceeded {
            required: operations,
            limit: limits.max_operations,
        });
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct NaturalLoopV1 {
    header: BlockId,
    blocks: BTreeSet<BlockId>,
    backedges: Vec<BlockId>,
    external_predecessors: Vec<BlockId>,
    exits: Vec<BlockId>,
}

fn discover_natural_loops(
    function: &Function,
    limits: LoopMemoryTransformLimitsV1,
) -> Result<(IndexedControlFlow, Vec<NaturalLoopV1>), LoopMemoryTransformErrorV1> {
    let cfg = analyze_control_flow(function).map_err(LoopMemoryTransformErrorV1::ControlFlow)?;
    if !cfg.is_reducible() {
        return Ok((cfg, Vec::new()));
    }
    let body = function
        .body
        .as_ref()
        .ok_or(LoopMemoryTransformErrorV1::InputRejected)?;
    let mut loops = BTreeMap::<BlockId, (BTreeSet<BlockId>, BTreeSet<BlockId>)>::new();
    for source in body.blocks.iter().map(|block| block.id) {
        for target in cfg
            .successor_blocks(source)
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
        {
            if !cfg.is_reachable(source) || !cfg.dominates(target, source) {
                continue;
            }
            let mut members = BTreeSet::from([target, source]);
            let mut pending = vec![source];
            while let Some(block) = pending.pop() {
                for predecessor in cfg
                    .predecessor_blocks(block)
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                {
                    if predecessor != target
                        && cfg.dominates(target, predecessor)
                        && members.insert(predecessor)
                    {
                        pending.push(predecessor);
                    }
                }
            }
            if members.len() > limits.max_loop_members {
                return Err(LoopMemoryTransformErrorV1::LoopMemberLimitExceeded {
                    required: members.len(),
                    limit: limits.max_loop_members,
                });
            }
            let entry = loops.entry(target).or_default();
            entry.0.extend(members);
            entry.1.insert(source);
        }
    }
    if loops.len() > limits.max_natural_loops {
        return Err(LoopMemoryTransformErrorV1::LoopLimitExceeded {
            required: loops.len(),
            limit: limits.max_natural_loops,
        });
    }
    let loops = loops
        .into_iter()
        .map(|(header, (blocks, backedges))| {
            let external_predecessors = cfg
                .predecessor_blocks(header)
                .into_iter()
                .flatten()
                .filter(|predecessor| !blocks.contains(predecessor))
                .collect::<Vec<_>>();
            let exits = blocks
                .iter()
                .flat_map(|block| {
                    cfg.successor_blocks(*block)
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>()
                })
                .filter(|successor| !blocks.contains(successor))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            NaturalLoopV1 {
                header,
                blocks,
                backedges: backedges.into_iter().collect(),
                external_predecessors,
                exits,
            }
        })
        .collect();
    Ok((cfg, loops))
}

#[derive(Clone, Debug)]
struct CanonicalBlockActionV1 {
    header: BlockId,
    target: BlockId,
    role: CanonicalLoopBlockRoleV1,
    sources: Vec<BlockId>,
}

fn next_canonical_block_action(
    function: &Function,
    limits: LoopMemoryTransformLimitsV1,
) -> Result<Option<CanonicalBlockActionV1>, LoopMemoryTransformErrorV1> {
    let (cfg, loops) = discover_natural_loops(function, limits)?;
    for natural_loop in loops {
        if natural_loop.external_predecessors.is_empty() || natural_loop.backedges.is_empty() {
            continue;
        }
        if natural_loop.external_predecessors.len() != 1
            || cfg
                .successor_blocks(natural_loop.external_predecessors[0])
                .is_none_or(|mut successors| {
                    successors.next() != Some(natural_loop.header) || successors.next().is_some()
                })
        {
            return Ok(Some(CanonicalBlockActionV1 {
                header: natural_loop.header,
                target: natural_loop.header,
                role: CanonicalLoopBlockRoleV1::Preheader,
                sources: natural_loop.external_predecessors,
            }));
        }
        if natural_loop.backedges.len() != 1
            || cfg
                .successor_blocks(natural_loop.backedges[0])
                .is_none_or(|mut successors| {
                    successors.next() != Some(natural_loop.header) || successors.next().is_some()
                })
        {
            return Ok(Some(CanonicalBlockActionV1 {
                header: natural_loop.header,
                target: natural_loop.header,
                role: CanonicalLoopBlockRoleV1::Latch,
                sources: natural_loop.backedges,
            }));
        }
        for exit in natural_loop.exits {
            let has_external_predecessor = cfg
                .predecessor_blocks(exit)
                .into_iter()
                .flatten()
                .any(|predecessor| !natural_loop.blocks.contains(&predecessor));
            if has_external_predecessor {
                let sources = cfg
                    .predecessor_blocks(exit)
                    .into_iter()
                    .flatten()
                    .filter(|predecessor| natural_loop.blocks.contains(predecessor))
                    .collect::<Vec<_>>();
                return Ok(Some(CanonicalBlockActionV1 {
                    header: natural_loop.header,
                    target: exit,
                    role: CanonicalLoopBlockRoleV1::DedicatedExit,
                    sources,
                }));
            }
        }
    }
    Ok(None)
}

fn canonicalize_natural_loops(
    module: &mut Module,
    limits: LoopMemoryTransformLimitsV1,
    report: &mut LoopMemoryTransformReportV1,
) -> Result<(), LoopMemoryTransformErrorV1> {
    for function in &mut module.functions {
        if function.body.is_none() {
            continue;
        }
        while let Some(action) = next_canonical_block_action(function, limits)? {
            if report.inserted_blocks == limits.max_inserted_loop_blocks {
                return Err(LoopMemoryTransformErrorV1::InsertedBlockLimitExceeded {
                    limit: limits.max_inserted_loop_blocks,
                });
            }
            insert_passthrough_block(function, action, report)?;
        }
    }
    Ok(())
}

fn insert_passthrough_block(
    function: &mut Function,
    action: CanonicalBlockActionV1,
    report: &mut LoopMemoryTransformReportV1,
) -> Result<(), LoopMemoryTransformErrorV1> {
    let function_id = function.id.clone();
    let mut next_value = fresh_value_id(function).map_err(LoopMemoryTransformErrorV1::Remap)?;
    let body = function
        .body
        .as_mut()
        .ok_or(LoopMemoryTransformErrorV1::InputRejected)?;
    let next_block = body
        .blocks
        .iter()
        .map(|block| block.id.0)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or(LoopMemoryTransformErrorV1::IdentityOverflow)?;
    let target_parameters = body
        .blocks
        .iter()
        .find(|block| block.id == action.target)
        .ok_or(LoopMemoryTransformErrorV1::InputRejected)?
        .parameters
        .iter()
        .map(|parameter| {
            let id = ValueId(next_value);
            next_value = next_value
                .checked_add(1)
                .ok_or(LoopMemoryTransformErrorV1::IdentityOverflow)?;
            Ok(ValueDef::new(id, parameter.ty.clone()))
        })
        .collect::<Result<Vec<_>, LoopMemoryTransformErrorV1>>()?;
    let bridge_id = BlockId(next_block);
    let mut redirected = 0_usize;
    for source in &action.sources {
        let block = body
            .blocks
            .iter_mut()
            .find(|block| block.id == *source)
            .ok_or(LoopMemoryTransformErrorV1::InputRejected)?;
        redirected += retarget_edges(
            block
                .terminator
                .as_mut()
                .ok_or(LoopMemoryTransformErrorV1::InputRejected)?,
            action.target,
            bridge_id,
        );
    }
    if redirected == 0 {
        return Err(LoopMemoryTransformErrorV1::InputRejected);
    }
    let mut bridge = BasicBlock::new(bridge_id);
    bridge.parameters = target_parameters;
    bridge.terminator = Some(Terminator::Branch {
        target: action.target,
        arguments: bridge
            .parameters
            .iter()
            .map(|parameter| parameter.id)
            .collect(),
    });
    body.blocks.push(bridge);
    report.inserted_blocks += 1;
    report.edits.push(LoopMemoryEditV1::CanonicalBlockInserted {
        function: function_id,
        header: action.header,
        block: bridge_id,
        target: action.target,
        role: action.role,
        redirected_sources: action.sources,
    });
    Ok(())
}

fn retarget_edges(terminator: &mut Terminator, old: BlockId, new: BlockId) -> usize {
    let mut changed = 0;
    let mut replace = |target: &mut BlockId| {
        if *target == old {
            *target = new;
            changed += 1;
        }
    };
    match terminator {
        Terminator::Branch { target, .. } => replace(target),
        Terminator::ConditionalBranch {
            then_target,
            else_target,
            ..
        } => {
            replace(then_target);
            replace(else_target);
        }
        Terminator::Switch {
            cases,
            default_target,
            ..
        } => {
            for case in cases {
                replace(&mut case.target);
            }
            replace(default_target);
        }
        Terminator::IntegerSwitch {
            cases,
            default_target,
            ..
        } => {
            for case in cases {
                replace(&mut case.target);
            }
            replace(default_target);
        }
        Terminator::Return { .. } | Terminator::Unreachable => {}
    }
    changed
}

#[derive(Clone, Debug)]
struct RecognizedInductionV1 {
    fact: InductionVariableFactV1,
    start_value: ValueId,
    bound_value: ValueId,
    step_value: ValueId,
    next_value: ValueId,
    compare: OperationCoordinateV1,
    update: OperationCoordinateV1,
    update_reversed: bool,
    dynamic_bound_proof: Option<DynamicTripProofV1>,
}

fn affine_index_expression(
    function: &Function,
    value: ValueId,
    max_work: usize,
) -> Result<Option<AffineIndexExpressionV1>, OptimizerQueryFailureV1> {
    fn evaluate(
        value: ValueId,
        definitions: &BTreeMap<ValueId, (BlockId, usize, &Operation)>,
        types: &BTreeMap<ValueId, Type>,
        cache: &mut BTreeMap<ValueId, Option<AffineIndexExpressionV1>>,
        active: &mut BTreeSet<ValueId>,
        work: &mut usize,
        max_work: usize,
    ) -> Result<Option<AffineIndexExpressionV1>, OptimizerQueryFailureV1> {
        if let Some(cached) = cache.get(&value) {
            return Ok(cached.clone());
        }
        *work = work
            .checked_add(1)
            .ok_or(OptimizerQueryFailureV1::GraphWorkLimit {
                required: usize::MAX,
                limit: max_work,
            })?;
        if *work > max_work {
            return Err(OptimizerQueryFailureV1::GraphWorkLimit {
                required: *work,
                limit: max_work,
            });
        }
        if types.get(&value) != Some(&Type::INDEX) || !active.insert(value) {
            return Ok(None);
        }
        let expression = match definitions
            .get(&value)
            .map(|(_, _, operation)| &operation.kind)
        {
            None => Some(AffineIndexExpressionV1 {
                constant: 0,
                terms: vec![AffineIndexTermV1 {
                    value,
                    coefficient: 1,
                }],
            }),
            Some(OperationKind::Constant(Constant::Index(constant))) => {
                Some(AffineIndexExpressionV1 {
                    constant: i128::from(*constant),
                    terms: Vec::new(),
                })
            }
            Some(OperationKind::Binary { op, lhs, rhs })
                if matches!(op, BinaryOp::Add | BinaryOp::Subtract | BinaryOp::Multiply) =>
            {
                let left = evaluate(*lhs, definitions, types, cache, active, work, max_work)?;
                let right = evaluate(*rhs, definitions, types, cache, active, work, max_work)?;
                match (op, left, right) {
                    (BinaryOp::Add, Some(left), Some(right)) => combine_affine(left, right, 1),
                    (BinaryOp::Subtract, Some(left), Some(right)) => {
                        combine_affine(left, right, -1)
                    }
                    (BinaryOp::Multiply, Some(left), Some(right)) if left.terms.is_empty() => {
                        scale_affine(right, left.constant)
                    }
                    (BinaryOp::Multiply, Some(left), Some(right)) if right.terms.is_empty() => {
                        scale_affine(left, right.constant)
                    }
                    _ => None,
                }
            }
            _ => None,
        };
        active.remove(&value);
        cache.insert(value, expression.clone());
        Ok(expression)
    }

    fn combine_affine(
        left: AffineIndexExpressionV1,
        right: AffineIndexExpressionV1,
        right_sign: i128,
    ) -> Option<AffineIndexExpressionV1> {
        let constant = left
            .constant
            .checked_add(right.constant.checked_mul(right_sign)?)?;
        let mut terms = left
            .terms
            .into_iter()
            .map(|term| (term.value, term.coefficient))
            .collect::<BTreeMap<_, _>>();
        for term in right.terms {
            let scaled = term.coefficient.checked_mul(right_sign)?;
            let coefficient = terms
                .get(&term.value)
                .copied()
                .unwrap_or(0)
                .checked_add(scaled)?;
            if coefficient == 0 {
                terms.remove(&term.value);
            } else {
                terms.insert(term.value, coefficient);
            }
        }
        Some(AffineIndexExpressionV1 {
            constant,
            terms: terms
                .into_iter()
                .map(|(value, coefficient)| AffineIndexTermV1 { value, coefficient })
                .collect(),
        })
    }

    fn scale_affine(
        mut expression: AffineIndexExpressionV1,
        scale: i128,
    ) -> Option<AffineIndexExpressionV1> {
        expression.constant = expression.constant.checked_mul(scale)?;
        for term in &mut expression.terms {
            term.coefficient = term.coefficient.checked_mul(scale)?;
        }
        expression.terms.retain(|term| term.coefficient != 0);
        Some(expression)
    }

    let definitions = operation_definitions(function);
    let types = function_value_types(function);
    evaluate(
        value,
        &definitions,
        &types,
        &mut BTreeMap::new(),
        &mut BTreeSet::new(),
        &mut 0,
        max_work,
    )
}

fn recognize_inductions(
    function: &Function,
    limits: LoopMemoryTransformLimitsV1,
) -> Result<Vec<RecognizedInductionV1>, LoopMemoryTransformErrorV1> {
    let (_, loops) = discover_natural_loops(function, limits)?;
    let Some(body) = &function.body else {
        return Ok(Vec::new());
    };
    let constants = constant_index_values(function);
    let definitions = operation_definitions(function);
    let mut recognized = Vec::new();
    for natural_loop in loops {
        let ([preheader], [latch]) = (
            natural_loop.external_predecessors.as_slice(),
            natural_loop.backedges.as_slice(),
        ) else {
            continue;
        };
        let Some(header) = body
            .blocks
            .iter()
            .find(|block| block.id == natural_loop.header)
        else {
            continue;
        };
        let Some(Terminator::ConditionalBranch {
            condition,
            then_target,
            else_target,
            ..
        }) = &header.terminator
        else {
            continue;
        };
        if !natural_loop.blocks.contains(then_target) || natural_loop.blocks.contains(else_target) {
            continue;
        }
        let Some((compare_index, compare_operation)) =
            header.operations.iter().enumerate().find(|(_, operation)| {
                operation
                    .results
                    .iter()
                    .any(|result| result.id == *condition)
            })
        else {
            continue;
        };
        let OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs,
            rhs,
        } = compare_operation.kind
        else {
            continue;
        };
        let Some(parameter_index) = header
            .parameters
            .iter()
            .position(|parameter| parameter.id == lhs && parameter.ty == Type::INDEX)
        else {
            continue;
        };
        let (Some(entry_arguments), Some(latch_arguments)) = (
            edge_arguments(body, *preheader, natural_loop.header),
            edge_arguments(body, *latch, natural_loop.header),
        ) else {
            continue;
        };
        let (Some(start_value), Some(next_value)) = (
            entry_arguments.get(parameter_index).copied(),
            latch_arguments.get(parameter_index).copied(),
        ) else {
            continue;
        };
        let (Some(start), Some((update_block, update_index, update_operation))) = (
            constants.get(&start_value).copied(),
            definitions.get(&next_value),
        ) else {
            continue;
        };
        if *update_block != *latch {
            continue;
        }
        let OperationKind::Binary {
            op: BinaryOp::Add,
            lhs: update_lhs,
            rhs: update_rhs,
        } = update_operation.kind
        else {
            continue;
        };
        let (step_value, update_reversed) = if update_lhs == lhs {
            (update_rhs, false)
        } else if update_rhs == lhs {
            (update_lhs, true)
        } else {
            continue;
        };
        let Some(step) = constants
            .get(&step_value)
            .copied()
            .filter(|step| *step != 0)
        else {
            continue;
        };
        let mut dynamic_bound_proof = None;
        let mut bound = constants.get(&rhs).copied();
        if bound.is_none()
            && let Some(expression) = affine_index_expression(function, rhs, limits.max_operations)
                .map_err(LoopMemoryTransformErrorV1::OptimizerQuery)?
            && !expression.terms.is_empty()
        {
            match find_dominating_index_equals_constant_guard_v1(
                function,
                *preheader,
                rhs,
                limits.max_operations,
            ) {
                Ok(Some((guard, exact_bound, _))) => {
                    bound = Some(exact_bound);
                    dynamic_bound_proof = Some(DynamicTripProofV1 {
                        function: function.id.clone(),
                        header: natural_loop.header,
                        bound_value: rhs,
                        expression,
                        exact_bound,
                        guard,
                    });
                }
                Ok(None) | Err(OptimizerQueryFailureV1::Unsupported) => {}
                Err(error) => return Err(LoopMemoryTransformErrorV1::OptimizerQuery(error)),
            }
        }
        let trip_count = bound.and_then(|bound| exact_trip_count(start, step, bound));
        if bound.is_some() && trip_count.is_none() {
            continue;
        }
        recognized.push(RecognizedInductionV1 {
            fact: InductionVariableFactV1 {
                function: function.id.clone(),
                header: natural_loop.header,
                preheader: *preheader,
                latch: *latch,
                body: *then_target,
                exit: *else_target,
                parameter: lhs,
                start_value,
                bound_value: rhs,
                start,
                step,
                bound,
                trip_count,
            },
            start_value,
            bound_value: rhs,
            step_value,
            next_value,
            compare: OperationCoordinateV1::new(&function.id, header.id, compare_index)?,
            update: OperationCoordinateV1::new(&function.id, *update_block, *update_index)?,
            update_reversed,
            dynamic_bound_proof,
        });
    }
    Ok(recognized)
}

fn record_inductions(
    report: &mut LoopMemoryTransformReportV1,
    inductions: &[RecognizedInductionV1],
) {
    report
        .induction_facts
        .extend(inductions.iter().map(|induction| induction.fact.clone()));
    report.dynamic_trip_proofs.extend(
        inductions
            .iter()
            .filter_map(|induction| induction.dynamic_bound_proof.clone()),
    );
}

fn exact_trip_count(start: u64, step: u64, bound: u64) -> Option<u64> {
    if start >= bound {
        return Some(0);
    }
    let distance = bound.checked_sub(start)?;
    let quotient = distance / step;
    let trip = quotient.checked_add(u64::from(distance % step != 0))?;
    start.checked_add(step.checked_mul(trip)?)?;
    Some(trip)
}

fn simplify_induction_variables(
    module: &mut Module,
    limits: LoopMemoryTransformLimitsV1,
    report: &mut LoopMemoryTransformReportV1,
) -> Result<(), LoopMemoryTransformErrorV1> {
    for function in &mut module.functions {
        let inductions = recognize_inductions(function, limits)?;
        record_inductions(report, &inductions);
        let Some(body) = &mut function.body else {
            continue;
        };
        for induction in inductions
            .into_iter()
            .filter(|induction| induction.update_reversed)
        {
            let operation = body
                .blocks
                .iter_mut()
                .find(|block| block.id == induction.update.block)
                .and_then(|block| {
                    block
                        .operations
                        .get_mut(induction.update.operation as usize)
                })
                .ok_or(LoopMemoryTransformErrorV1::InputRejected)?;
            operation.kind = OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: induction.fact.parameter,
                rhs: induction.step_value,
            };
            report
                .edits
                .push(LoopMemoryEditV1::InductionUpdateNormalized {
                    function: function.id.clone(),
                    header: induction.fact.header,
                    update: induction.update,
                });
        }
    }
    Ok(())
}

fn hoist_loop_invariants(
    module: &mut Module,
    limits: LoopMemoryTransformLimitsV1,
    report: &mut LoopMemoryTransformReportV1,
) -> Result<(), LoopMemoryTransformErrorV1> {
    for function in &mut module.functions {
        loop {
            let (cfg, loops) = discover_natural_loops(function, limits)?;
            let inductions = recognize_inductions(function, limits)?;
            let definitions = value_definition_blocks(function);
            let mut candidate = None;
            for induction in &inductions {
                let executes_at_least_once = match induction.fact.trip_count {
                    Some(0) => false,
                    Some(_) => true,
                    None => match find_dominating_unsigned_less_than_guard_v1(
                        function,
                        induction.fact.preheader,
                        induction.start_value,
                        induction.bound_value,
                        limits.max_operations,
                    ) {
                        Ok(Some(_)) => true,
                        Ok(None) | Err(OptimizerQueryFailureV1::Unsupported) => false,
                        Err(error) => {
                            return Err(LoopMemoryTransformErrorV1::OptimizerQuery(error));
                        }
                    },
                };
                if !executes_at_least_once {
                    continue;
                }
                let Some(natural_loop) = loops
                    .iter()
                    .find(|natural_loop| natural_loop.header == induction.fact.header)
                else {
                    continue;
                };
                let Some(body) = &function.body else {
                    continue;
                };
                for block in &body.blocks {
                    if block.id == natural_loop.header
                        || !natural_loop.blocks.contains(&block.id)
                        || !cfg.dominates(block.id, induction.fact.latch)
                    {
                        continue;
                    }
                    for (operation_index, operation) in block.operations.iter().enumerate() {
                        if !safe_total_loop_invariant(operation)
                            || operation.kind.operands().iter().any(|operand| {
                                definitions.get(operand).is_some_and(|definition| {
                                    definition.is_some_and(|block| {
                                        natural_loop.blocks.contains(&block)
                                            || !cfg.dominates(block, induction.fact.preheader)
                                    })
                                })
                            })
                        {
                            continue;
                        }
                        candidate = Some((induction.fact.clone(), block.id, operation_index));
                        break;
                    }
                    if candidate.is_some() {
                        break;
                    }
                }
                if candidate.is_some() {
                    break;
                }
            }
            let Some((induction, source_block, operation_index)) = candidate else {
                record_inductions(report, &inductions);
                break;
            };
            let function_id = function.id.clone();
            let body = function
                .body
                .as_mut()
                .ok_or(LoopMemoryTransformErrorV1::InputRejected)?;
            let source_position = body
                .blocks
                .iter()
                .position(|block| block.id == source_block)
                .ok_or(LoopMemoryTransformErrorV1::InputRejected)?;
            let source_len = body.blocks[source_position].operations.len();
            let source = OperationCoordinateV1::new(&function_id, source_block, operation_index)?;
            let operation = body.blocks[source_position]
                .operations
                .remove(operation_index);
            let destination = body
                .blocks
                .iter_mut()
                .find(|block| block.id == induction.preheader)
                .ok_or(LoopMemoryTransformErrorV1::InputRejected)?;
            let destination_operation = destination.operations.len();
            destination.operations.push(operation);
            report.edits.push(LoopMemoryEditV1::OperationMoved {
                source: source.clone(),
                destination_block: induction.preheader,
            });
            report
                .coordinate_lineage
                .push(OperationCoordinateLineageV1::Moved {
                    before: source,
                    after: OperationCoordinateV1::new(
                        &function_id,
                        induction.preheader,
                        destination_operation,
                    )?,
                });
            for shifted in operation_index + 1..source_len {
                report
                    .coordinate_lineage
                    .push(OperationCoordinateLineageV1::Retained {
                        before: OperationCoordinateV1::new(&function_id, source_block, shifted)?,
                        after: OperationCoordinateV1::new(&function_id, source_block, shifted - 1)?,
                    });
            }
        }
    }
    Ok(())
}

fn safe_total_loop_invariant(operation: &Operation) -> bool {
    operation.has_complete_effect_summary()
        && operation.effect_summary().is_pure()
        && operation
            .results
            .iter()
            .all(|result| matches!(result.ty, Type::Scalar(_)))
        && matches!(
            operation.kind,
            OperationKind::Constant(_)
                | OperationKind::Unary {
                    op: fe2o3_kernel_ir::UnaryOp::Not,
                    ..
                }
                | OperationKind::Binary {
                    op: BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor,
                    ..
                }
                | OperationKind::Compare { .. }
                | OperationKind::Cast {
                    kind: CastKind::Bitcast,
                    ..
                }
                | OperationKind::Select { .. }
        )
}

fn unroll_static_loops(
    module: &mut Module,
    limits: LoopMemoryTransformLimitsV1,
    full: bool,
    report: &mut LoopMemoryTransformReportV1,
) -> Result<(), LoopMemoryTransformErrorV1> {
    for function in &mut module.functions {
        let inductions = recognize_inductions(function, limits)?;
        record_inductions(report, &inductions);
        for induction in inductions {
            let Some(trip_count) = induction.fact.trip_count else {
                report.unsupported_dynamic_trip_expansions.push(
                    UnsupportedDynamicTripExpansionV1 {
                        function: induction.fact.function.clone(),
                        header: induction.fact.header,
                        bound_value: induction.fact.bound_value,
                    },
                );
                continue;
            };
            if full {
                if trip_count > limits.max_full_unroll_trip_count {
                    continue;
                }
                if can_unroll_simple_loop(function, &induction, limits)? {
                    fully_unroll(function, &induction, trip_count, limits, report)?;
                }
            } else {
                let factor = u64::from(limits.partial_unroll_factor);
                if trip_count <= limits.max_full_unroll_trip_count
                    || trip_count > limits.max_partial_unroll_trip_count
                    || trip_count % factor != 0
                {
                    continue;
                }
                if can_unroll_simple_loop(function, &induction, limits)? {
                    partially_unroll(function, &induction, limits, report)?;
                }
            }
            break;
        }
    }
    Ok(())
}

fn can_unroll_simple_loop(
    function: &Function,
    induction: &RecognizedInductionV1,
    limits: LoopMemoryTransformLimitsV1,
) -> Result<bool, LoopMemoryTransformErrorV1> {
    let body = function
        .body
        .as_ref()
        .ok_or(LoopMemoryTransformErrorV1::InputRejected)?;
    let (_, loops) = discover_natural_loops(function, limits)?;
    let Some(natural_loop) = loops
        .iter()
        .find(|natural_loop| natural_loop.header == induction.fact.header)
    else {
        return Ok(false);
    };
    let Some(header) = body
        .blocks
        .iter()
        .find(|block| block.id == induction.fact.header)
    else {
        return Ok(false);
    };
    let Some(loop_body) = body
        .blocks
        .iter()
        .find(|block| block.id == induction.fact.body)
    else {
        return Ok(false);
    };
    let Some(exit) = body
        .blocks
        .iter()
        .find(|block| block.id == induction.fact.exit)
    else {
        return Ok(false);
    };
    let preheader_branches_directly = body
        .blocks
        .iter()
        .find(|block| block.id == induction.fact.preheader)
        .and_then(|block| block.terminator.as_ref())
        .is_some_and(|terminator| {
            matches!(terminator, Terminator::Branch { target, .. } if *target == induction.fact.header)
        });
    let body_branch = matches!(
        loop_body.terminator,
        Some(Terminator::Branch { target, ref arguments })
            if target == induction.fact.header && arguments.as_slice() == [induction.next_value]
    );
    let definitions = value_definition_blocks(function);
    Ok(
        natural_loop.blocks == BTreeSet::from([induction.fact.header, induction.fact.body])
            && induction.fact.body == induction.fact.latch
            && header.parameters.len() == 1
            && header.operations.len() == 1
            && induction.compare.operation == 0
            && loop_body.parameters.is_empty()
            && loop_body.operations.iter().all(safe_unroll_operation)
            && header_exit_arguments(header, induction.fact.exit).is_some_and(|arguments| {
                arguments.len() == exit.parameters.len()
                    && arguments.iter().all(|argument| {
                        *argument == induction.fact.parameter
                            || definitions
                                .get(argument)
                                .copied()
                                .flatten()
                                .is_none_or(|block| !natural_loop.blocks.contains(&block))
                    })
            })
            && preheader_branches_directly
            && body_branch,
    )
}

fn safe_unroll_operation(operation: &Operation) -> bool {
    operation.has_complete_effect_summary()
        && operation.effect_summary().is_pure()
        && operation
            .results
            .iter()
            .all(|result| matches!(result.ty, Type::Scalar(_)))
        && matches!(
            operation.kind,
            OperationKind::Constant(_)
                | OperationKind::Unary {
                    op: fe2o3_kernel_ir::UnaryOp::Not,
                    ..
                }
                | OperationKind::Binary {
                    op: BinaryOp::Add
                        | BinaryOp::Subtract
                        | BinaryOp::Multiply
                        | BinaryOp::BitAnd
                        | BinaryOp::BitOr
                        | BinaryOp::BitXor,
                    ..
                }
                | OperationKind::Compare { .. }
                | OperationKind::Cast {
                    kind: CastKind::Bitcast,
                    ..
                }
                | OperationKind::Select { .. }
        )
}

fn function_operation_count(function: &Function) -> usize {
    function
        .body
        .as_ref()
        .into_iter()
        .flat_map(|body| &body.blocks)
        .map(|block| block.operations.len())
        .sum()
}

fn evaluate_unroll_cost(
    limits: LoopMemoryTransformLimitsV1,
    query: TargetNeutralCostQueryV1,
) -> Result<TargetNeutralCostDecisionV1, LoopMemoryTransformErrorV1> {
    let decision = BoundedTargetNeutralCostModelV1::new(
        limits.max_unroll_growth_operations,
        limits.max_operations,
    )
    .evaluate(query);
    match decision.rejection() {
        None => Ok(decision),
        Some(TargetNeutralCostRejectionV1::AddedOperationLimit { required, limit }) => {
            Err(LoopMemoryTransformErrorV1::UnrollGrowthLimitExceeded { required, limit })
        }
        Some(TargetNeutralCostRejectionV1::OutputOperationLimit { required, limit }) => {
            Err(LoopMemoryTransformErrorV1::OperationLimitExceeded { required, limit })
        }
        Some(TargetNeutralCostRejectionV1::ArithmeticOverflow) => {
            Err(LoopMemoryTransformErrorV1::IdentityOverflow)
        }
    }
}

fn fully_unroll(
    function: &mut Function,
    induction: &RecognizedInductionV1,
    trip_count: u64,
    limits: LoopMemoryTransformLimitsV1,
    report: &mut LoopMemoryTransformReportV1,
) -> Result<(), LoopMemoryTransformErrorV1> {
    let body_operations = function
        .body
        .as_ref()
        .and_then(|body| {
            body.blocks
                .iter()
                .find(|block| block.id == induction.fact.body)
        })
        .map(|block| block.operations.clone())
        .ok_or(LoopMemoryTransformErrorV1::InputRejected)?;
    let copies = usize::try_from(trip_count)
        .ok()
        .and_then(|trip| body_operations.len().checked_mul(trip))
        .ok_or(LoopMemoryTransformErrorV1::IdentityOverflow)?;
    let eliminated = function
        .body
        .as_ref()
        .into_iter()
        .flat_map(|body| &body.blocks)
        .filter(|block| block.id == induction.fact.header || block.id == induction.fact.body)
        .map(|block| block.operations.len())
        .sum::<usize>();
    let decision = evaluate_unroll_cost(
        limits,
        TargetNeutralCostQueryV1::new(
            TargetNeutralCostTransformV1::FullLoopUnrolling,
            function_operation_count(function),
            copies,
            eliminated,
            Some(trip_count),
        ),
    )?;
    report.cost_decisions.push(decision);
    let function_id = function.id.clone();
    let eliminated_coordinates = function
        .body
        .as_ref()
        .into_iter()
        .flat_map(|body| &body.blocks)
        .filter(|block| block.id == induction.fact.header || block.id == induction.fact.body)
        .flat_map(|block| {
            block
                .operations
                .iter()
                .enumerate()
                .map(|(index, _)| OperationCoordinateV1::new(&function_id, block.id, index))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut next_value = fresh_value_id(function).map_err(LoopMemoryTransformErrorV1::Remap)?;
    let mut carried = induction.start_value;
    let mut cloned = Vec::with_capacity(copies);
    for _ in 0..trip_count {
        let mut replacements = BTreeMap::from([(induction.fact.parameter, carried)]);
        clone_operations(
            &body_operations,
            &mut replacements,
            &mut next_value,
            &mut cloned,
        )?;
        carried = resolve_value(induction.next_value, &replacements);
    }
    let header = function
        .body
        .as_ref()
        .and_then(|body| {
            body.blocks
                .iter()
                .find(|block| block.id == induction.fact.header)
        })
        .ok_or(LoopMemoryTransformErrorV1::InputRejected)?;
    let mut exit_arguments = header_exit_arguments(header, induction.fact.exit)
        .ok_or(LoopMemoryTransformErrorV1::InputRejected)?;
    for argument in &mut exit_arguments {
        if *argument == induction.fact.parameter {
            *argument = carried;
        }
    }
    let body = function
        .body
        .as_mut()
        .ok_or(LoopMemoryTransformErrorV1::InputRejected)?;
    let preheader = body
        .blocks
        .iter_mut()
        .find(|block| block.id == induction.fact.preheader)
        .ok_or(LoopMemoryTransformErrorV1::InputRejected)?;
    let destination_base = preheader.operations.len();
    preheader.operations.extend(cloned);
    preheader.terminator = Some(Terminator::Branch {
        target: induction.fact.exit,
        arguments: exit_arguments,
    });
    body.blocks
        .retain(|block| block.id != induction.fact.header && block.id != induction.fact.body);
    report.inserted_operations += copies;
    report.eliminated_operations += eliminated;
    for (copy_index, source_index) in (0..copies).zip((0..body_operations.len()).cycle()) {
        report
            .coordinate_lineage
            .push(OperationCoordinateLineageV1::Duplicated {
                source: OperationCoordinateV1::new(
                    &function_id,
                    induction.fact.body,
                    source_index,
                )?,
                copy: OperationCoordinateV1::new(
                    &function_id,
                    induction.fact.preheader,
                    destination_base + copy_index,
                )?,
            });
    }
    report.coordinate_lineage.extend(
        eliminated_coordinates
            .into_iter()
            .map(|before| OperationCoordinateLineageV1::Eliminated { before }),
    );
    report.edits.push(LoopMemoryEditV1::LoopFullyUnrolled {
        function: function_id,
        header: induction.fact.header,
        body: induction.fact.body,
        trip_count,
        copied_operations: copies,
    });
    Ok(())
}

fn partially_unroll(
    function: &mut Function,
    induction: &RecognizedInductionV1,
    limits: LoopMemoryTransformLimitsV1,
    report: &mut LoopMemoryTransformReportV1,
) -> Result<(), LoopMemoryTransformErrorV1> {
    let original = function
        .body
        .as_ref()
        .and_then(|body| {
            body.blocks
                .iter()
                .find(|block| block.id == induction.fact.body)
        })
        .map(|block| block.operations.clone())
        .ok_or(LoopMemoryTransformErrorV1::InputRejected)?;
    let extra_copies = usize::try_from(limits.partial_unroll_factor - 1)
        .ok()
        .and_then(|factor| original.len().checked_mul(factor))
        .ok_or(LoopMemoryTransformErrorV1::IdentityOverflow)?;
    let decision = evaluate_unroll_cost(
        limits,
        TargetNeutralCostQueryV1::new(
            TargetNeutralCostTransformV1::PartialLoopUnrolling,
            function_operation_count(function),
            extra_copies,
            0,
            induction.fact.trip_count,
        ),
    )?;
    report.cost_decisions.push(decision);
    let function_id = function.id.clone();
    let mut next_value = fresh_value_id(function).map_err(LoopMemoryTransformErrorV1::Remap)?;
    let mut carried = induction.next_value;
    let mut cloned = Vec::with_capacity(extra_copies);
    for _ in 1..limits.partial_unroll_factor {
        let mut replacements = BTreeMap::from([(induction.fact.parameter, carried)]);
        clone_operations(&original, &mut replacements, &mut next_value, &mut cloned)?;
        carried = resolve_value(induction.next_value, &replacements);
    }
    let body = function
        .body
        .as_mut()
        .ok_or(LoopMemoryTransformErrorV1::InputRejected)?;
    let loop_body = body
        .blocks
        .iter_mut()
        .find(|block| block.id == induction.fact.body)
        .ok_or(LoopMemoryTransformErrorV1::InputRejected)?;
    let destination_base = loop_body.operations.len();
    loop_body.operations.extend(cloned);
    let Some(Terminator::Branch { arguments, .. }) = &mut loop_body.terminator else {
        return Err(LoopMemoryTransformErrorV1::InputRejected);
    };
    arguments[0] = carried;
    report.inserted_operations += extra_copies;
    for (copy_index, source_index) in (0..extra_copies).zip((0..original.len()).cycle()) {
        report
            .coordinate_lineage
            .push(OperationCoordinateLineageV1::Duplicated {
                source: OperationCoordinateV1::new(
                    &function_id,
                    induction.fact.body,
                    source_index,
                )?,
                copy: OperationCoordinateV1::new(
                    &function_id,
                    induction.fact.body,
                    destination_base + copy_index,
                )?,
            });
    }
    report.edits.push(LoopMemoryEditV1::LoopPartiallyUnrolled {
        function: function_id,
        header: induction.fact.header,
        body: induction.fact.body,
        factor: limits.partial_unroll_factor,
        copied_operations: extra_copies,
    });
    Ok(())
}

fn clone_operations(
    operations: &[Operation],
    replacements: &mut BTreeMap<ValueId, ValueId>,
    next_value: &mut u32,
    output: &mut Vec<Operation>,
) -> Result<(), LoopMemoryTransformErrorV1> {
    for operation in operations {
        let mut cloned = operation.clone();
        remap_supported_operation(&mut cloned, replacements)
            .map_err(LoopMemoryTransformErrorV1::Remap)?;
        for result in &mut cloned.results {
            let original = result.id;
            result.id = ValueId(*next_value);
            *next_value = next_value
                .checked_add(1)
                .ok_or(LoopMemoryTransformErrorV1::IdentityOverflow)?;
            replacements.insert(original, result.id);
        }
        output.push(cloned);
    }
    Ok(())
}

fn resolve_value(mut value: ValueId, replacements: &BTreeMap<ValueId, ValueId>) -> ValueId {
    for _ in 0..=replacements.len() {
        let Some(next) = replacements.get(&value).copied() else {
            break;
        };
        value = next;
    }
    value
}

fn header_exit_arguments(header: &BasicBlock, exit: BlockId) -> Option<Vec<ValueId>> {
    edge_arguments_from_terminator(header.terminator.as_ref()?, exit)
}

fn edge_arguments(
    body: &fe2o3_kernel_ir::FunctionBody,
    source: BlockId,
    target: BlockId,
) -> Option<Vec<ValueId>> {
    let terminator = body
        .blocks
        .iter()
        .find(|block| block.id == source)?
        .terminator
        .as_ref()?;
    edge_arguments_from_terminator(terminator, target)
}

fn edge_arguments_from_terminator(
    terminator: &Terminator,
    target: BlockId,
) -> Option<Vec<ValueId>> {
    let mut matches = Vec::new();
    match terminator {
        Terminator::Branch {
            target: edge_target,
            arguments,
        } => {
            if *edge_target == target {
                matches.push(arguments.clone());
            }
        }
        Terminator::ConditionalBranch {
            then_target,
            then_arguments,
            else_target,
            else_arguments,
            ..
        } => {
            if *then_target == target {
                matches.push(then_arguments.clone());
            }
            if *else_target == target {
                matches.push(else_arguments.clone());
            }
        }
        Terminator::Switch {
            cases,
            default_target,
            default_arguments,
            ..
        } => {
            matches.extend(
                cases
                    .iter()
                    .filter(|case| case.target == target)
                    .map(|case| case.arguments.clone()),
            );
            if *default_target == target {
                matches.push(default_arguments.clone());
            }
        }
        Terminator::IntegerSwitch {
            cases,
            default_target,
            default_arguments,
            ..
        } => {
            matches.extend(
                cases
                    .iter()
                    .filter(|case| case.target == target)
                    .map(|case| case.arguments.clone()),
            );
            if *default_target == target {
                matches.push(default_arguments.clone());
            }
        }
        Terminator::Return { .. } | Terminator::Unreachable => {}
    }
    let [arguments] = matches.as_slice() else {
        return None;
    };
    Some(arguments.clone())
}

fn constant_index_values(function: &Function) -> BTreeMap<ValueId, u64> {
    function
        .body
        .as_ref()
        .into_iter()
        .flat_map(|body| &body.blocks)
        .flat_map(|block| &block.operations)
        .filter_map(|operation| {
            let [result] = operation.results.as_slice() else {
                return None;
            };
            let OperationKind::Constant(Constant::Index(value)) = operation.kind else {
                return None;
            };
            Some((result.id, value))
        })
        .collect()
}

fn operation_definitions(function: &Function) -> BTreeMap<ValueId, (BlockId, usize, &Operation)> {
    function
        .body
        .as_ref()
        .into_iter()
        .flat_map(|body| &body.blocks)
        .flat_map(|block| {
            block
                .operations
                .iter()
                .enumerate()
                .flat_map(move |(index, operation)| {
                    operation
                        .results
                        .iter()
                        .map(move |result| (result.id, (block.id, index, operation)))
                })
        })
        .collect()
}

fn value_definition_blocks(function: &Function) -> BTreeMap<ValueId, Option<BlockId>> {
    let mut definitions = function
        .body
        .as_ref()
        .into_iter()
        .flat_map(|body| body.parameters.iter().copied())
        .map(|parameter| (parameter, None))
        .collect::<BTreeMap<_, _>>();
    if let Some(body) = &function.body {
        for block in &body.blocks {
            definitions.extend(
                block
                    .parameters
                    .iter()
                    .map(|parameter| (parameter.id, Some(block.id))),
            );
            definitions.extend(
                block
                    .operations
                    .iter()
                    .flat_map(|operation| &operation.results)
                    .map(|result| (result.id, Some(block.id))),
            );
        }
    }
    definitions
}

pub fn build_versioned_memory_effect_graph_v1(
    module: &Module,
    limits: LoopMemoryTransformLimitsV1,
) -> Result<VersionedMemoryEffectGraphV1, LoopMemoryTransformErrorV1> {
    verify_module(module).map_err(|_| LoopMemoryTransformErrorV1::InputRejected)?;
    let mut version = 0_u64;
    let mut block_versions = Vec::new();
    let mut block_entries = Vec::new();
    let mut events = Vec::new();
    let (single_invocation_workgroup_entries, single_invocation_dispatch_entries) =
        exact_shared_memory_entries(module);
    for function in &module.functions {
        let Some(body) = &function.body else {
            continue;
        };
        let cfg =
            analyze_control_flow(function).map_err(LoopMemoryTransformErrorV1::ControlFlow)?;
        let pointers = PointerFactsV1::new(
            function,
            single_invocation_workgroup_entries.contains(&function.id),
            single_invocation_dispatch_entries.contains(&function.id),
        );
        let mut entries = BTreeMap::new();
        let mut exits = BTreeMap::new();
        for block in &body.blocks {
            version = version
                .checked_add(1)
                .ok_or(LoopMemoryTransformErrorV1::IdentityOverflow)?;
            let entry = version;
            entries.insert(block.id, entry);
            for (operation_index, operation) in block.operations.iter().enumerate() {
                let classified = classify_memory_event(operation);
                let Some((kind, mutates_version, pointer, access)) = classified else {
                    continue;
                };
                if events.len() == limits.max_memory_version_events {
                    return Err(LoopMemoryTransformErrorV1::MemoryEventLimitExceeded {
                        limit: limits.max_memory_version_events,
                    });
                }
                let before = version;
                if mutates_version {
                    version = version
                        .checked_add(1)
                        .ok_or(LoopMemoryTransformErrorV1::IdentityOverflow)?;
                }
                events.push(MemoryVersionEventV1 {
                    site: OperationCoordinateV1::new(&function.id, block.id, operation_index)?,
                    before,
                    after: version,
                    kind,
                    location: pointer
                        .zip(access)
                        .map(|(pointer, access)| pointers.location(pointer, access.address_space)),
                });
            }
            block_versions.push(MemoryBlockVersionV1 {
                function: function.id.clone(),
                block: block.id,
                entry,
                exit: version,
            });
            exits.insert(block.id, version);
        }
        for block in &body.blocks {
            let mut predecessors = cfg
                .predecessor_blocks(block.id)
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();
            predecessors.sort_unstable();
            let incoming = predecessors
                .into_iter()
                .map(|predecessor| {
                    exits
                        .get(&predecessor)
                        .copied()
                        .map(|version| IncomingMemoryVersionV1 {
                            predecessor,
                            version,
                        })
                        .ok_or(LoopMemoryTransformErrorV1::InputRejected)
                })
                .collect::<Result<Vec<_>, _>>()?;
            block_entries.push(MemoryBlockEntryVersionV1 {
                function: function.id.clone(),
                block: block.id,
                version: entries
                    .get(&block.id)
                    .copied()
                    .ok_or(LoopMemoryTransformErrorV1::InputRejected)?,
                incoming,
            });
        }
    }
    Ok(VersionedMemoryEffectGraphV1 {
        block_versions,
        block_entries,
        events,
        final_version: version,
    })
}

fn classify_memory_event(
    operation: &Operation,
) -> Option<(
    MemoryVersionEventKindV1,
    bool,
    Option<ValueId>,
    Option<MemoryAccess>,
)> {
    match &operation.kind {
        OperationKind::Load { pointer, access } if access.volatile => Some((
            MemoryVersionEventKindV1::VolatileLoadBoundary,
            true,
            Some(*pointer),
            Some(*access),
        )),
        OperationKind::Load { pointer, access } => Some((
            MemoryVersionEventKindV1::Load,
            false,
            Some(*pointer),
            Some(*access),
        )),
        OperationKind::Store {
            pointer, access, ..
        } if access.volatile => Some((
            MemoryVersionEventKindV1::VolatileStoreBoundary,
            true,
            Some(*pointer),
            Some(*access),
        )),
        OperationKind::Store {
            pointer, access, ..
        } => Some((
            MemoryVersionEventKindV1::Store,
            true,
            Some(*pointer),
            Some(*access),
        )),
        OperationKind::Atomic(_) => {
            Some((MemoryVersionEventKindV1::AtomicBoundary, true, None, None))
        }
        OperationKind::Barrier(_) | OperationKind::WorkgroupBarrier(_) => {
            Some((MemoryVersionEventKindV1::BarrierBoundary, true, None, None))
        }
        OperationKind::Fence(_) => {
            Some((MemoryVersionEventKindV1::FenceBoundary, true, None, None))
        }
        OperationKind::Wave(_)
        | OperationKind::Matrix(_)
        | OperationKind::Gfx950LdsTranspose(_) => Some((
            MemoryVersionEventKindV1::ConvergentBoundary,
            true,
            None,
            None,
        )),
        OperationKind::Call { .. } => Some((
            MemoryVersionEventKindV1::UnknownCallBoundary,
            true,
            None,
            None,
        )),
        OperationKind::GuardedLoad { .. }
        | OperationKind::GuardedStore { .. }
        | OperationKind::MemoryIntrinsic(_)
        | OperationKind::InlineAssembly(_)
        | OperationKind::WorkgroupMemory(_)
        | OperationKind::KernelContextIssue(_)
        | OperationKind::GlobalCapabilityBind(_)
        | OperationKind::GlobalCapabilityIndex(_)
        | OperationKind::ExecutionCapability(_) => Some((
            MemoryVersionEventKindV1::UnknownEffectBoundary,
            true,
            None,
            None,
        )),
        _ if !operation.has_complete_effect_summary() || !operation.effect_summary().is_pure() => {
            Some((
                MemoryVersionEventKindV1::UnknownEffectBoundary,
                true,
                None,
                None,
            ))
        }
        _ => None,
    }
}

#[derive(Clone)]
struct PointerFactsV1 {
    types: BTreeMap<ValueId, Type>,
    constants: BTreeMap<ValueId, u64>,
    definitions: BTreeMap<ValueId, OperationKind>,
    private_allocations: BTreeSet<ValueId>,
    workgroup_allocations: BTreeSet<ValueId>,
    exclusive_global_roots: BTreeSet<ValueId>,
    read_only_external_roots: BTreeSet<ValueId>,
}

impl PointerFactsV1 {
    fn new(
        function: &Function,
        single_invocation_workgroup: bool,
        single_invocation_dispatch: bool,
    ) -> Self {
        let mut types = function
            .body
            .as_ref()
            .into_iter()
            .flat_map(|body| body.parameters.iter().copied())
            .zip(function.signature.parameters.iter().cloned())
            .collect::<BTreeMap<_, _>>();
        let mut definitions = BTreeMap::new();
        let mut private_allocations = BTreeSet::new();
        let mut workgroup_allocations = BTreeSet::new();
        let mut exclusive_global_roots = BTreeSet::new();
        let mut read_only_external_roots: BTreeSet<ValueId> = function
            .body
            .as_ref()
            .into_iter()
            .flat_map(|body| body.parameters.iter().copied())
            .zip(&function.signature.parameters)
            .filter_map(|(value, ty)| match ty {
                Type::Pointer(pointer)
                    if pointer.access == AccessMode::ReadOnly
                        && matches!(
                            pointer.address_space,
                            AddressSpace::Global | AddressSpace::Constant
                        ) =>
                {
                    Some(value)
                }
                Type::Slice(slice)
                    if slice.access == AccessMode::ReadOnly
                        && matches!(
                            slice.address_space,
                            AddressSpace::Global | AddressSpace::Constant
                        ) =>
                {
                    Some(value)
                }
                _ => None,
            })
            .collect();
        if let Some(body) = &function.body {
            for block in &body.blocks {
                types.extend(
                    block
                        .parameters
                        .iter()
                        .map(|parameter| (parameter.id, parameter.ty.clone())),
                );
                for operation in &block.operations {
                    for result in &operation.results {
                        types.insert(result.id, result.ty.clone());
                        definitions.insert(result.id, operation.kind.clone());
                    }
                    if let ([result], OperationKind::Alloca { address_space, .. }) =
                        (operation.results.as_slice(), &operation.kind)
                    {
                        match address_space {
                            AddressSpace::Private => {
                                private_allocations.insert(result.id);
                            }
                            AddressSpace::Workgroup if single_invocation_workgroup => {
                                workgroup_allocations.insert(result.id);
                            }
                            _ => {}
                        }
                    }
                    if let ([result], OperationKind::GlobalCapabilityBind(_)) =
                        (operation.results.as_slice(), &operation.kind)
                    {
                        match &result.ty {
                            Type::GlobalCapability(capability)
                                if single_invocation_dispatch
                                    && capability.is_complete()
                                    && capability.role()
                                        == GlobalCapabilityRoleV1::ExclusiveReadWrite =>
                            {
                                exclusive_global_roots.insert(result.id);
                            }
                            Type::GlobalCapability(capability)
                                if capability.is_complete()
                                    && capability.role() == GlobalCapabilityRoleV1::ReadOnly =>
                            {
                                read_only_external_roots.insert(result.id);
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        Self {
            types,
            constants: constant_index_values(function),
            definitions,
            private_allocations,
            workgroup_allocations,
            exclusive_global_roots,
            read_only_external_roots,
        }
    }

    fn location(&self, pointer: ValueId, address_space: AddressSpace) -> MemoryLocationV1 {
        let mut current = pointer;
        let mut projection = Vec::new();
        let mut exact = true;
        let mut visited = BTreeSet::new();
        while visited.insert(current) {
            let Some(operation) = self.definitions.get(&current) else {
                break;
            };
            match operation {
                OperationKind::GetElementPointer { base, offset } => {
                    let Some(offset) = self.constants.get(offset).copied() else {
                        exact = false;
                        current = *base;
                        break;
                    };
                    projection.push(offset);
                    current = *base;
                }
                OperationKind::Cast {
                    kind: CastKind::RestrictPointerAccess,
                    value,
                    ..
                } => current = *value,
                OperationKind::SliceData { slice } => current = *slice,
                _ => break,
            }
        }
        projection.reverse();
        let type_address_space = self.types.get(&pointer).and_then(|ty| match ty {
            Type::Pointer(pointer) => Some(pointer.address_space),
            _ => None,
        });
        MemoryLocationV1 {
            pointer,
            root: current,
            constant_projection: exact.then_some(projection),
            address_space: type_address_space.unwrap_or(address_space),
            private_allocation: self.private_allocations.contains(&current),
            workgroup_allocation: self.workgroup_allocations.contains(&current),
            exclusive_global: self.exclusive_global_roots.contains(&current),
            read_only_external: self.read_only_external_roots.contains(&current),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AliasRelationV1 {
    Must,
    No,
    May,
}

fn alias_relation(left: &MemoryLocationV1, right: &MemoryLocationV1) -> AliasRelationV1 {
    if left.pointer == right.pointer
        || (left.root == right.root
            && left.constant_projection.is_some()
            && left.constant_projection == right.constant_projection)
    {
        AliasRelationV1::Must
    } else if left.address_space != right.address_space
        || (left.private_allocation && right.private_allocation && left.root != right.root)
        || (left.workgroup_allocation && right.workgroup_allocation && left.root != right.root)
    {
        AliasRelationV1::No
    } else {
        AliasRelationV1::May
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AvailableMemoryValueV1 {
    location: MemoryLocationV1,
    value: ValueId,
    producer: OperationCoordinateV1,
    from_store: bool,
    store_observed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AvailableMemoryStateV1 {
    synchronization_epoch: MemorySynchronizationEpochV1,
    values: Vec<AvailableMemoryValueV1>,
}

impl AvailableMemoryStateV1 {
    fn root() -> Self {
        Self {
            synchronization_epoch: MemorySynchronizationEpochV1::Root,
            values: Vec::new(),
        }
    }
}

#[derive(Default)]
struct MemoryDataflowResultV1 {
    replacements: BTreeMap<ValueId, ValueId>,
    removed: BTreeSet<OperationCoordinateV1>,
    edits: Vec<LoopMemoryEditV1>,
    loop_carried_phis: Vec<LoopCarriedMemoryPhiV1>,
}

fn simplify_versioned_memory(
    module: &mut Module,
    limits: LoopMemoryTransformLimitsV1,
    report: &mut LoopMemoryTransformReportV1,
) -> Result<(), LoopMemoryTransformErrorV1> {
    let (single_invocation_workgroup_entries, single_invocation_dispatch_entries) =
        exact_shared_memory_entries(module);
    for function in &mut module.functions {
        let result = analyze_memory_dataflow(
            function,
            limits,
            single_invocation_workgroup_entries.contains(&function.id),
            single_invocation_dispatch_entries.contains(&function.id),
        )?;
        report.eliminated_operations = report
            .eliminated_operations
            .checked_add(result.removed.len())
            .ok_or(LoopMemoryTransformErrorV1::IdentityOverflow)?;
        report.edits.extend(result.edits);
        report
            .loop_carried_memory_phis
            .extend(result.loop_carried_phis);
        let Some(body) = &mut function.body else {
            continue;
        };
        for block in &mut body.blocks {
            let original = std::mem::take(&mut block.operations);
            let mut rewritten = Vec::with_capacity(original.len());
            for (index, mut operation) in original.into_iter().enumerate() {
                let before = OperationCoordinateV1::new(&function.id, block.id, index)?;
                if result.removed.contains(&before) {
                    report
                        .coordinate_lineage
                        .push(OperationCoordinateLineageV1::Eliminated { before });
                    continue;
                }
                remap_supported_operation(&mut operation, &result.replacements)
                    .map_err(LoopMemoryTransformErrorV1::Remap)?;
                let after = OperationCoordinateV1::new(&function.id, block.id, rewritten.len())?;
                report
                    .coordinate_lineage
                    .push(OperationCoordinateLineageV1::Retained { before, after });
                rewritten.push(operation);
            }
            remap_terminator(block.terminator.as_mut(), &result.replacements);
            block.operations = rewritten;
        }
    }
    Ok(())
}

fn kernel_entries_with_all(
    module: &Module,
    predicate: impl Fn(&fe2o3_kernel_ir::Kernel) -> bool,
) -> BTreeSet<FunctionId> {
    let mut entries = BTreeMap::<FunctionId, bool>::new();
    for kernel in &module.kernels {
        entries
            .entry(kernel.entry.clone())
            .and_modify(|accepted| *accepted &= predicate(kernel))
            .or_insert_with(|| predicate(kernel));
    }
    entries
        .into_iter()
        .filter_map(|(entry, accepted)| accepted.then_some(entry))
        .collect()
}

fn exact_shared_memory_entries(module: &Module) -> (BTreeSet<FunctionId>, BTreeSet<FunctionId>) {
    let workgroup = kernel_entries_with_all(module, |kernel| {
        kernel
            .workgroup_size
            .is_some_and(|size| size.x == 1 && size.y == 1 && size.z == 1)
    });
    let dispatch = kernel_entries_with_all(module, |kernel| {
        kernel
            .workgroup_size
            .is_some_and(|size| size.x == 1 && size.y == 1 && size.z == 1)
            && kernel
                .domain
                .extents()
                .all(|extent| matches!(extent, fe2o3_kernel_ir::LaunchExtent::Static(1)))
    });
    (workgroup, dispatch)
}

fn analyze_memory_dataflow(
    function: &Function,
    limits: LoopMemoryTransformLimitsV1,
    single_invocation_workgroup: bool,
    single_invocation_dispatch: bool,
) -> Result<MemoryDataflowResultV1, LoopMemoryTransformErrorV1> {
    let Some(body) = &function.body else {
        return Ok(MemoryDataflowResultV1::default());
    };
    let cfg = analyze_control_flow(function).map_err(LoopMemoryTransformErrorV1::ControlFlow)?;
    let pointers = PointerFactsV1::new(
        function,
        single_invocation_workgroup,
        single_invocation_dispatch,
    );
    let value_types = function_value_types(function);
    let order = memory_dataflow_order(body, &cfg);
    let mut exits = order
        .iter()
        .copied()
        .map(|block| (block, None))
        .collect::<BTreeMap<BlockId, Option<AvailableMemoryStateV1>>>();
    let mut work = 0_usize;
    let mut converged = false;
    for _ in 0..limits.max_memory_dataflow_iterations {
        let mut changed = false;
        for block_id in &order {
            charge_memory_dataflow_work(&mut work, 1, limits.max_memory_dataflow_work)?;
            let block = find_block(body, *block_id)?;
            let Some(entry) = memory_block_entry_state(
                *block_id,
                &cfg,
                &exits,
                &mut work,
                limits.max_memory_dataflow_work,
            )?
            else {
                continue;
            };
            let next = transfer_memory_block(
                function,
                block,
                &cfg,
                &pointers,
                &value_types,
                entry,
                None,
                &mut work,
                limits,
            )?;
            if exits.get(block_id) != Some(&Some(next.clone())) {
                exits.insert(*block_id, Some(next));
                changed = true;
            }
        }
        if !changed && exits.values().all(Option::is_some) {
            converged = true;
            break;
        }
    }
    if !converged {
        return Err(
            LoopMemoryTransformErrorV1::MemoryDataflowIterationLimitExceeded {
                limit: limits.max_memory_dataflow_iterations,
            },
        );
    }

    let mut result = MemoryDataflowResultV1::default();
    for block_id in order {
        let block = find_block(body, block_id)?;
        let entry = memory_block_entry_state(
            block_id,
            &cfg,
            &exits,
            &mut work,
            limits.max_memory_dataflow_work,
        )?
        .ok_or(LoopMemoryTransformErrorV1::InputRejected)?;
        let predecessors = sorted_predecessors(&cfg, block_id);
        if predecessors.len() > 1
            && predecessors.iter().any(|predecessor| {
                cfg.is_reachable(*predecessor) && cfg.dominates(block_id, *predecessor)
            })
        {
            result.loop_carried_phis.push(LoopCarriedMemoryPhiV1 {
                function: function.id.clone(),
                block: block_id,
                predecessors,
                synchronization_epoch: entry.synchronization_epoch.clone(),
                available_producers: entry
                    .values
                    .iter()
                    .map(|value| value.producer.clone())
                    .collect(),
            });
        }
        let _ = transfer_memory_block(
            function,
            block,
            &cfg,
            &pointers,
            &value_types,
            entry,
            Some(&mut result),
            &mut work,
            limits,
        )?;
    }
    Ok(result)
}

fn find_block(
    body: &fe2o3_kernel_ir::FunctionBody,
    block: BlockId,
) -> Result<&BasicBlock, LoopMemoryTransformErrorV1> {
    body.blocks
        .iter()
        .find(|candidate| candidate.id == block)
        .ok_or(LoopMemoryTransformErrorV1::InputRejected)
}

fn memory_block_entry_state(
    block: BlockId,
    cfg: &IndexedControlFlow,
    exits: &BTreeMap<BlockId, Option<AvailableMemoryStateV1>>,
    work: &mut usize,
    work_limit: usize,
) -> Result<Option<AvailableMemoryStateV1>, LoopMemoryTransformErrorV1> {
    let predecessors = sorted_predecessors(cfg, block);
    if predecessors.is_empty() || !cfg.is_reachable(block) {
        return Ok(Some(AvailableMemoryStateV1::root()));
    }
    let incoming = predecessors
        .iter()
        .filter_map(|predecessor| exits.get(predecessor).and_then(Option::as_ref))
        .collect::<Vec<_>>();
    if incoming.is_empty() {
        return Ok(None);
    }
    let merge_work = incoming
        .iter()
        .map(|state| state.values.len().max(1))
        .sum::<usize>();
    charge_memory_dataflow_work(work, merge_work, work_limit)?;
    Ok(Some(intersect_available_memory(&incoming)))
}

#[allow(clippy::too_many_arguments)]
fn transfer_memory_block(
    function: &Function,
    block: &BasicBlock,
    cfg: &IndexedControlFlow,
    pointers: &PointerFactsV1,
    value_types: &BTreeMap<ValueId, Type>,
    mut state: AvailableMemoryStateV1,
    mut result: Option<&mut MemoryDataflowResultV1>,
    work: &mut usize,
    limits: LoopMemoryTransformLimitsV1,
) -> Result<AvailableMemoryStateV1, LoopMemoryTransformErrorV1> {
    for (operation_index, operation) in block.operations.iter().enumerate() {
        charge_memory_dataflow_work(
            work,
            state.values.len().saturating_add(1),
            limits.max_memory_dataflow_work,
        )?;
        let site = OperationCoordinateV1::new(&function.id, block.id, operation_index)?;
        match &operation.kind {
            OperationKind::Load { pointer, access } if !access.volatile => {
                let [load_result] = operation.results.as_slice() else {
                    return Err(LoopMemoryTransformErrorV1::InputRejected);
                };
                let location = pointers.location(*pointer, access.address_space);
                let eligible = state.synchronization_epoch
                    != MemorySynchronizationEpochV1::Ambiguous
                    && exact_forwardable_load(&location, &load_result.ty);
                let candidate = eligible.then(|| {
                    state.values.iter().position(|value| {
                        alias_relation(&value.location, &location) == AliasRelationV1::Must
                            && value_types.get(&value.value) == Some(&load_result.ty)
                            && memory_producer_dominates_use(cfg, &value.producer, &site)
                    })
                });
                if let Some(Some(candidate)) = candidate {
                    if let Some(result) = result.as_deref_mut() {
                        let value = &state.values[candidate];
                        let forwarded = resolve_value(value.value, &result.replacements);
                        result.replacements.insert(load_result.id, forwarded);
                        if result.removed.insert(site.clone()) {
                            result.edits.push(LoopMemoryEditV1::MemoryValueForwarded {
                                producer: value.producer.clone(),
                                load: site,
                                kind: if value.from_store {
                                    MemoryForwardKindV1::StoreToLoad
                                } else {
                                    MemoryForwardKindV1::LoadToLoad
                                },
                                synchronization_epoch: state.synchronization_epoch.clone(),
                            });
                        }
                    }
                } else {
                    for value in &mut state.values {
                        if value.from_store
                            && alias_relation(&value.location, &location) != AliasRelationV1::No
                        {
                            value.store_observed = true;
                        }
                    }
                    if eligible {
                        state.values.retain(|value| {
                            alias_relation(&value.location, &location) != AliasRelationV1::Must
                        });
                        state.values.push(AvailableMemoryValueV1 {
                            location,
                            value: load_result.id,
                            producer: site,
                            from_store: false,
                            store_observed: true,
                        });
                    }
                }
            }
            OperationKind::Store {
                pointer,
                value,
                access,
            } if !access.volatile => {
                let location = pointers.location(*pointer, access.address_space);
                if let Some(result) = result.as_deref_mut() {
                    for previous in &state.values {
                        if previous.from_store
                            && previous.location.has_exact_writable_provenance()
                            && !previous.store_observed
                            && state.synchronization_epoch
                                != MemorySynchronizationEpochV1::Ambiguous
                            && alias_relation(&previous.location, &location)
                                == AliasRelationV1::Must
                            && linear_cfg_path(
                                cfg,
                                previous.producer.block,
                                block.id,
                                limits.max_blocks,
                            )
                            && result.removed.insert(previous.producer.clone())
                        {
                            result.edits.push(LoopMemoryEditV1::DeadStoreEliminated {
                                store: previous.producer.clone(),
                                overwriting_store: site.clone(),
                                synchronization_epoch: state.synchronization_epoch.clone(),
                            });
                        }
                    }
                }
                state.values.retain(|previous| {
                    alias_relation(&previous.location, &location) == AliasRelationV1::No
                });
                if state.synchronization_epoch != MemorySynchronizationEpochV1::Ambiguous
                    && exact_forwardable_store(&location)
                {
                    let value = result
                        .as_deref()
                        .map_or(*value, |result| resolve_value(*value, &result.replacements));
                    state.values.push(AvailableMemoryValueV1 {
                        location,
                        value,
                        producer: site,
                        from_store: true,
                        store_observed: false,
                    });
                }
            }
            _ if classify_memory_event(operation).is_some() => {
                state.values.clear();
                state.synchronization_epoch = MemorySynchronizationEpochV1::Boundary(site);
            }
            _ => {}
        }
    }
    Ok(state)
}

fn exact_forwardable_load(location: &MemoryLocationV1, ty: &Type) -> bool {
    location.constant_projection.is_some()
        && (location.has_exact_writable_provenance() || location.read_only_external)
        && matches!(ty, Type::Scalar(_))
}

fn exact_forwardable_store(location: &MemoryLocationV1) -> bool {
    location.constant_projection.is_some() && location.has_exact_writable_provenance()
}

fn memory_producer_dominates_use(
    cfg: &IndexedControlFlow,
    producer: &OperationCoordinateV1,
    use_site: &OperationCoordinateV1,
) -> bool {
    if producer.block == use_site.block {
        producer.operation < use_site.operation
    } else {
        cfg.dominates(producer.block, use_site.block)
    }
}

fn same_available_fact(left: &AvailableMemoryValueV1, right: &AvailableMemoryValueV1) -> bool {
    left.location == right.location
        && left.value == right.value
        && left.producer == right.producer
        && left.from_store == right.from_store
}

fn intersect_available_memory(incoming: &[&AvailableMemoryStateV1]) -> AvailableMemoryStateV1 {
    let Some(first) = incoming.first() else {
        return AvailableMemoryStateV1::root();
    };
    let synchronization_epoch = if incoming
        .iter()
        .all(|state| state.synchronization_epoch == first.synchronization_epoch)
    {
        first.synchronization_epoch.clone()
    } else {
        return AvailableMemoryStateV1 {
            synchronization_epoch: MemorySynchronizationEpochV1::Ambiguous,
            values: Vec::new(),
        };
    };
    let mut intersection = first.values.clone();
    intersection.retain_mut(|candidate| {
        let mut observed = candidate.store_observed;
        for state in &incoming[1..] {
            let Some(matching) = state
                .values
                .iter()
                .find(|other| same_available_fact(candidate, other))
            else {
                return false;
            };
            observed |= matching.store_observed;
        }
        candidate.store_observed = observed;
        true
    });
    AvailableMemoryStateV1 {
        synchronization_epoch,
        values: intersection,
    }
}

fn sorted_predecessors(cfg: &IndexedControlFlow, block: BlockId) -> Vec<BlockId> {
    let mut predecessors = cfg
        .predecessor_blocks(block)
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    predecessors.sort_unstable();
    predecessors
}

fn sorted_successors(cfg: &IndexedControlFlow, block: BlockId) -> Vec<BlockId> {
    let mut successors = cfg
        .successor_blocks(block)
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    successors.sort_unstable();
    successors
}

fn memory_dataflow_order(
    body: &fe2o3_kernel_ir::FunctionBody,
    cfg: &IndexedControlFlow,
) -> Vec<BlockId> {
    let ids = body
        .blocks
        .iter()
        .map(|block| block.id)
        .collect::<BTreeSet<_>>();
    let mut indegree = ids
        .iter()
        .copied()
        .map(|block| {
            let incoming = sorted_predecessors(cfg, block)
                .into_iter()
                .filter(|predecessor| !cfg.dominates(block, *predecessor))
                .count();
            (block, incoming)
        })
        .collect::<BTreeMap<_, _>>();
    let mut ready = indegree
        .iter()
        .filter_map(|(block, incoming)| (*incoming == 0).then_some(*block))
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(ids.len());
    while let Some(block) = ready.iter().next().copied() {
        ready.remove(&block);
        order.push(block);
        for successor in sorted_successors(cfg, block) {
            if cfg.dominates(successor, block) {
                continue;
            }
            let Some(incoming) = indegree.get_mut(&successor) else {
                continue;
            };
            *incoming = incoming.saturating_sub(1);
            if *incoming == 0 {
                ready.insert(successor);
            }
        }
    }
    let scheduled = order.iter().copied().collect::<BTreeSet<_>>();
    order.extend(ids.difference(&scheduled).copied());
    order
}

fn linear_cfg_path(
    cfg: &IndexedControlFlow,
    source: BlockId,
    destination: BlockId,
    max_blocks: usize,
) -> bool {
    if source == destination {
        return true;
    }
    let mut current = source;
    let mut visited = BTreeSet::from([source]);
    for _ in 0..max_blocks {
        let successors = sorted_successors(cfg, current);
        let [next] = successors.as_slice() else {
            return false;
        };
        if sorted_predecessors(cfg, *next).len() != 1 || !visited.insert(*next) {
            return false;
        }
        if *next == destination {
            return true;
        }
        current = *next;
    }
    false
}

fn charge_memory_dataflow_work(
    work: &mut usize,
    units: usize,
    limit: usize,
) -> Result<(), LoopMemoryTransformErrorV1> {
    *work = work.checked_add(units).ok_or(
        LoopMemoryTransformErrorV1::MemoryDataflowWorkLimitExceeded {
            required: usize::MAX,
            limit,
        },
    )?;
    if *work > limit {
        return Err(
            LoopMemoryTransformErrorV1::MemoryDataflowWorkLimitExceeded {
                required: *work,
                limit,
            },
        );
    }
    Ok(())
}

fn function_value_types(function: &Function) -> BTreeMap<ValueId, Type> {
    let mut types = function
        .body
        .as_ref()
        .into_iter()
        .flat_map(|body| body.parameters.iter().copied())
        .zip(function.signature.parameters.iter().cloned())
        .collect::<BTreeMap<_, _>>();
    if let Some(body) = &function.body {
        for block in &body.blocks {
            types.extend(
                block
                    .parameters
                    .iter()
                    .map(|parameter| (parameter.id, parameter.ty.clone())),
            );
            types.extend(
                block
                    .operations
                    .iter()
                    .flat_map(|operation| &operation.results)
                    .map(|result| (result.id, result.ty.clone())),
            );
        }
    }
    types
}

#[cfg(test)]
mod tests {
    use fe2o3_kernel_ir::{
        Atomic, AtomicKind, Barrier, BarrierSemantics, Fence, MemoryOrdering, SynchronizationScope,
        WaveOperation, WaveOperationKind, WaveWidth,
    };

    use super::*;

    fn event_kind(kind: OperationKind) -> (MemoryVersionEventKindV1, bool) {
        let (kind, mutates_version, _, _) =
            classify_memory_event(&Operation::new(Vec::new(), kind)).expect("memory boundary");
        (kind, mutates_version)
    }

    #[test]
    fn every_explicit_memory_and_convergence_boundary_starts_a_new_version() {
        let semantics = BarrierSemantics::new(
            MemoryOrdering::AcquireRelease,
            [AddressSpace::Private, AddressSpace::Global],
        );
        let cases = [
            (
                OperationKind::Barrier(Barrier {
                    execution_scope: SynchronizationScope::Workgroup,
                    memory_scope: SynchronizationScope::Device,
                    semantics: semantics.clone(),
                }),
                MemoryVersionEventKindV1::BarrierBoundary,
            ),
            (
                OperationKind::Fence(Fence {
                    memory_scope: SynchronizationScope::Device,
                    semantics,
                }),
                MemoryVersionEventKindV1::FenceBoundary,
            ),
            (
                OperationKind::Atomic(Atomic {
                    kind: AtomicKind::Add,
                    pointer: ValueId(0),
                    value: Some(ValueId(1)),
                    compare: None,
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                    scope: SynchronizationScope::Device,
                    ordering: MemoryOrdering::AcquireRelease,
                    failure_ordering: None,
                }),
                MemoryVersionEventKindV1::AtomicBoundary,
            ),
            (
                OperationKind::Wave(WaveOperation::full(
                    WaveOperationKind::LaneId,
                    WaveWidth::Wave64,
                )),
                MemoryVersionEventKindV1::ConvergentBoundary,
            ),
            (
                OperationKind::Call {
                    callee: "opaque".into(),
                    arguments: Vec::new(),
                },
                MemoryVersionEventKindV1::UnknownCallBoundary,
            ),
        ];

        for (operation, expected) in cases {
            assert_eq!(event_kind(operation), (expected, true));
        }
    }

    #[test]
    fn volatile_accesses_are_boundaries_and_plain_loads_do_not_mutate_versions() {
        let mut volatile = MemoryAccess::new(AddressSpace::Private, 4);
        volatile.volatile = true;
        assert_eq!(
            event_kind(OperationKind::Load {
                pointer: ValueId(0),
                access: volatile,
            }),
            (MemoryVersionEventKindV1::VolatileLoadBoundary, true)
        );
        assert_eq!(
            event_kind(OperationKind::Store {
                pointer: ValueId(0),
                value: ValueId(1),
                access: volatile,
            }),
            (MemoryVersionEventKindV1::VolatileStoreBoundary, true)
        );
        assert_eq!(
            event_kind(OperationKind::Load {
                pointer: ValueId(0),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            }),
            (MemoryVersionEventKindV1::Load, false)
        );
    }
}
