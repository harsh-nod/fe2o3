//! Checked, non-executable ranked analysis of one exact final canonical function.
//!
//! No source-ranked evidence, launch assumptions, or alias separation is imported.
//! Owners preflight `tree_work`, materialize in their canonical bridge context,
//! use the closed batch builder to append all functions before their common
//! mutation-attempt seal, and revalidate before and after analysis. Single-view
//! materialization seals a detached function immediately; attaching it later
//! invalidates that seal. A materialization error requires owner-session
//! poisoning; it publishes no view.
//! Writes currently reject: plain canonical V13 supplies no exact reference
//! effect/ownership contracts. Structural write projection is not a proof of
//! those obligations and must not make their absence vacuously successful.
//!
//! Decode is bounded by canonical V13's module-byte limit. Planning uses sparse
//! SSA maps in O(source operations * log(source values) + CFG edges) work and
//! O(source values + source operations + blocks) storage. Each guarded write
//! adds exactly two bounded synthetic blocks, never a path enumeration.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    error::Error,
    fmt,
    panic::{AssertUnwindSafe, catch_unwind},
};

use dialect_kernel::{
    AccessKindAttr, BranchOp, DYNAMIC_EXTENT, DimensionOp, IndexBinaryKindAttr, IndexBinaryOp,
    IndexConstantOp, IndexLessThanBranchOp, IndexType, InvocationIndexOp, MemorySpaceAttr,
    RankedAccessOp, RankedViewOp, RankedViewType, ReturnOp, SemanticOverflowAttr,
    SemanticScalarKindAttr, SemanticTypedBinaryKindAttr, SemanticTypedBinaryOp,
    SemanticTypedConstantOp, SemanticTypedScalarV1, SemanticTypedSymbolOp,
};
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, Axis, BinaryOp, BlockId, ComparePredicate, Constant, Function,
    FunctionId, GlobalCapabilityRoleV1, GlobalDisjointIndexSpaceV1, IndexKind, IntrinsicKind,
    MemoryAccess, OperationKind, Terminator, Type, ValueId, VerifiedCanonicalKernelIrIdentityV13,
    VerifiedCanonicalKernelIrV13, decode_module_v13,
};
use fe2o3_pliron_owner_core::{ContextIdentity, require_context_identity};
use pliron::{
    basic_block::BasicBlock,
    builtin::{
        attributes::IdentifierAttr,
        op_interfaces::{ATTR_KEY_SYM_NAME, OneRegionInterface},
        ops::{FuncOp, ModuleOp},
        types::FunctionType,
    },
    context::{Context, IrMutationAttemptEpoch, Ptr},
    linked_list::ContainsLinkedList,
    op::Op,
    operation::{Operation, verify_operation},
    r#type::TypeHandle,
    value::Value,
};

use crate::pliron_ir_identity::{
    PlironIrIdentityErrorV1, PlironIrStructuralIdentityV1, derive_pliron_ir_structural_identity_v1,
};

mod launch;
mod preflight;
pub use preflight::{CanonicalRankedViewPreflightV1, preflight_canonical_ranked_view_v1};

pub const MAX_CANONICAL_RANKED_SOURCE_OPERATIONS_V1: usize = 16_384;
pub const MAX_CANONICAL_RANKED_SOURCE_VALUES_V1: usize = 32_768;
pub const MAX_CANONICAL_RANKED_SOURCE_BLOCKS_V1: usize = 256;
pub const MAX_CANONICAL_RANKED_SYNTHETIC_BLOCKS_V1: usize = 512;
pub const MAX_CANONICAL_RANKED_PARAMETERS_V1: usize = 64;
pub const MAX_CANONICAL_RANKED_VIEWS_V1: usize = crate::MAX_PRODUCTION_W4_FUNCTIONS_V1;
pub const MAX_CANONICAL_RANKED_ROOT_SYMBOL_BYTES_V1: usize = 1_024;

#[derive(Debug)]
pub enum CanonicalRankedViewErrorV1 {
    CanonicalDecode,
    MissingFunction,
    Unsupported {
        block: Option<BlockId>,
        operation: Option<usize>,
        reason: &'static str,
    },
    UnsupportedIntrinsic {
        block: BlockId,
        operation: usize,
        intrinsic: IntrinsicKind,
    },
    Limit {
        resource: &'static str,
        limit: usize,
        actual: usize,
    },
    MissingContextIdentity,
    ContextChanged,
    CanonicalSubjectChanged,
    AnalysisGraphChanged,
    MutationEpochUnavailable,
    MutationEpochChanged,
    InvalidBatch(&'static str),
    InvalidBatchRoot,
    MissingWriteContracts {
        block: BlockId,
        operation: usize,
    },
    StructuralIdentity(PlironIrIdentityErrorV1),
    Materialization(&'static str),
}

impl fmt::Display for CanonicalRankedViewErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedIntrinsic {
                block,
                operation,
                intrinsic,
            } => write!(
                f,
                "canonical ranked view: unsupported intrinsic {intrinsic:?} at {block} operation {operation}: unavailable hierarchy/launch facts"
            ),
            Self::MissingWriteContracts { block, operation } => write!(
                f,
                "canonical ranked view: write at {block:?} operation {operation} requires exact canonical ownership and reference-effect contracts"
            ),
            _ => write!(f, "canonical ranked view: {self:?}"),
        }
    }
}
impl Error for CanonicalRankedViewErrorV1 {}

/// This predicate selects an analysis representation, never a successful check.
/// Unknown operations/control select the projector, which must reject them.
pub fn needs_ranked_projection(function: &Function) -> bool {
    let Some(body) = &function.body else {
        return true;
    };
    body.blocks.iter().any(|block| {
        block.operations.iter().any(|operation| {
            !matches!(
                operation.kind,
                OperationKind::KernelContextIssue(_)
                    | OperationKind::Intrinsic(_)
                    | OperationKind::MemoryIntrinsic(_)
                    | OperationKind::Alloca { .. }
                    | OperationKind::GuardedLoad { .. }
                    | OperationKind::GuardedStore { .. }
                    | OperationKind::Barrier(_)
                    | OperationKind::Atomic(_)
                    | OperationKind::Fence(_)
                    | OperationKind::WorkgroupBarrier(_)
                    | OperationKind::WorkgroupMemory(_)
                    | OperationKind::Matrix(_)
                    | OperationKind::Gfx950LdsTranspose(_)
                    | OperationKind::Wave(_)
                    | OperationKind::InlineAssembly(_)
            )
        }) || !matches!(&block.terminator,
            Some(Terminator::Return { values }) if values.is_empty()
        ) && !matches!(
            &block.terminator,
            Some(
                Terminator::Switch { .. }
                    | Terminator::IntegerSwitch { .. }
                    | Terminator::Unreachable
            )
        )
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Node(usize);

#[derive(Clone, Debug)]
enum PlannedOp {
    View {
        result: Node,
        extent: Node,
        parameter: usize,
        access: AccessMode,
    },
    Index {
        result: Node,
        bits: u64,
    },
    Invocation {
        result: Node,
        axis: u32,
        extent: u64,
    },
    IndexBinary {
        result: Node,
        kind: IndexBinaryKindAttr,
        lhs: Node,
        rhs: Node,
    },
    F32 {
        result: Node,
        bits: u32,
    },
    F32Parameter {
        result: Node,
        parameter: u32,
    },
    F32Binary {
        result: Node,
        kind: SemanticTypedBinaryKindAttr,
        lhs: Node,
        rhs: Node,
    },
    Length {
        result: Node,
        view: Node,
    },
    Write {
        view: Node,
        index: Node,
        rhs: Node,
        effect: usize,
    },
}

#[derive(Clone, Debug)]
enum PlannedTerminator {
    LessThan {
        lhs: Node,
        rhs: Node,
        yes: usize,
        no: usize,
    },
    Branch(usize),
    Return,
}

#[derive(Default, Debug)]
struct PlannedBlock {
    operations: Vec<PlannedOp>,
    terminator: Option<PlannedTerminator>,
}

#[derive(Clone, Copy, Debug)]
enum Fact {
    Index {
        node: Node,
        constant: Option<u64>,
    },
    F32(Node),
    Slice(usize),
    Context,
    Capability {
        slice: usize,
        role: GlobalCapabilityRoleV1,
    },
    Predicate {
        lhs: Node,
        rhs: Node,
    },
    Select {
        predicate: ValueId,
        index: Node,
    },
    Pointer {
        slice: usize,
        offset: Option<ValueId>,
    },
}

#[derive(Debug)]
struct SliceOrigin {
    parameter: usize,
    view: Node,
    access: AccessMode,
}

/// Source-to-analysis write correspondence, privately constructed and sealed
/// with the analysis graph. RankedAccessOp has no RHS operand: this record
/// retains the exact source RHS and the materialized typed SSA node separately.
#[derive(Debug)]
pub struct CanonicalRankedWriteBindingV1 {
    source_block: BlockId,
    source_operation: usize,
    parameter: usize,
    pointer: ValueId,
    index: ValueId,
    predicate: Option<ValueId>,
    rhs: ValueId,
    access: MemoryAccess,
    analysis_block: usize,
    analysis_operation: usize,
    rhs_node: Node,
}

impl CanonicalRankedWriteBindingV1 {
    pub const fn source_block(&self) -> BlockId {
        self.source_block
    }
    pub const fn source_operation(&self) -> usize {
        self.source_operation
    }
    pub const fn parameter_ordinal(&self) -> usize {
        self.parameter
    }
    pub const fn pointer(&self) -> ValueId {
        self.pointer
    }
    pub const fn index(&self) -> ValueId {
        self.index
    }
    pub const fn predicate(&self) -> Option<ValueId> {
        self.predicate
    }
    pub const fn rhs(&self) -> ValueId {
        self.rhs
    }
    pub const fn access(&self) -> MemoryAccess {
        self.access
    }
    pub const fn element_stride_bytes(&self) -> u32 {
        4
    }
}

/// No public constructor or mutable recipe access. Only complete projection
/// of a VerifiedCanonicalKernelIrV13 can create a blueprint.
#[derive(Debug)]
pub struct CanonicalRankedViewBlueprintV1 {
    canonical: VerifiedCanonicalKernelIrIdentityV13,
    epoch: u64,
    function: FunctionId,
    function_ordinal: usize,
    arguments: Vec<Node>,
    blocks: Vec<PlannedBlock>,
    emission_order: Vec<usize>,
    nodes: usize,
    covered_operations: usize,
    writes: Vec<CanonicalRankedWriteBindingV1>,
    tree_work: usize,
}

/// This is an analysis-view receipt, not executable lowering or artifact authority.
pub struct CheckedCanonicalRankedViewV1 {
    graph: UnsealedCanonicalRankedViewV1,
    mutation_epoch: IrMutationAttemptEpoch,
}

// This private construction state can only escape after a single final seal.
struct UnsealedCanonicalRankedViewV1 {
    canonical: VerifiedCanonicalKernelIrIdentityV13,
    epoch: u64,
    function: FunctionId,
    context: ContextIdentity,
    pliron: FuncOp,
    structural: PlironIrStructuralIdentityV1,
    live_blocks: Box<[Ptr<BasicBlock>]>,
    live_operations: Box<[Ptr<Operation>]>,
    writes: Box<[CanonicalRankedWriteBindingV1]>,
    // Every node's location is retained, so write RHS associations are not
    // detached formulas and cannot be rebound to a caller-provided SSA value.
    nodes: Box<[Value]>,
    covered_operations: usize,
}

impl CheckedCanonicalRankedViewV1 {
    pub fn function_id(&self) -> &FunctionId {
        &self.graph.function
    }
    pub const fn pliron(&self) -> &FuncOp {
        &self.graph.pliron
    }
    pub const fn canonical_identity(&self) -> &VerifiedCanonicalKernelIrIdentityV13 {
        &self.graph.canonical
    }
    pub const fn final_epoch(&self) -> u64 {
        self.graph.epoch
    }
    pub fn writes(&self) -> &[CanonicalRankedWriteBindingV1] {
        &self.graph.writes
    }
    /// Exact typed SSA producer retained for the corresponding source store.
    /// Like `pliron`, this handle is meaningful only in the revalidated context.
    pub fn write_rhs(&self, write_ordinal: usize) -> Option<Value> {
        let binding = self.graph.writes.get(write_ordinal)?;
        self.graph.nodes.get(binding.rhs_node.0).copied()
    }
    pub const fn covered_source_operations(&self) -> usize {
        self.graph.covered_operations
    }

    pub fn revalidate(
        &self,
        context: &Context,
        canonical: &VerifiedCanonicalKernelIrV13,
        epoch: u64,
    ) -> Result<(), CanonicalRankedViewErrorV1> {
        let identity = require_context_identity(context)
            .map_err(|_| CanonicalRankedViewErrorV1::MissingContextIdentity)?;
        if identity != self.graph.context {
            return Err(CanonicalRankedViewErrorV1::ContextChanged);
        }
        if canonical.identity() != &self.graph.canonical || epoch != self.graph.epoch {
            return Err(CanonicalRankedViewErrorV1::CanonicalSubjectChanged);
        }
        self.require_mutation_epoch(context)?;
        require_write_contracts(&self.graph.writes)?;
        let actual = derive_pliron_ir_structural_identity_v1(context, &self.graph.pliron)
            .map_err(|_| CanonicalRankedViewErrorV1::AnalysisGraphChanged)?;
        if !actual.exactly_matches(&self.graph.structural) {
            return Err(CanonicalRankedViewErrorV1::AnalysisGraphChanged);
        }
        // Ensure retained source bindings still address the same live SSA
        // producers, even after an isomorphic replacement of graph nodes.
        let region = self.graph.pliron.get_region(context);
        let blocks = region.deref(context).iter(context).collect::<Vec<_>>();
        let operations = blocks
            .iter()
            .flat_map(|block| block.deref(context).iter(context).collect::<Vec<_>>())
            .collect::<Vec<_>>();
        if blocks.as_slice() != self.graph.live_blocks.as_ref()
            || operations.as_slice() != self.graph.live_operations.as_ref()
        {
            return Err(CanonicalRankedViewErrorV1::AnalysisGraphChanged);
        }
        self.require_mutation_epoch(context)
    }

    fn require_mutation_epoch(&self, context: &Context) -> Result<(), CanonicalRankedViewErrorV1> {
        if mutation_epoch(context)? != self.mutation_epoch {
            return Err(CanonicalRankedViewErrorV1::MutationEpochChanged);
        }
        Ok(())
    }
}

fn mutation_epoch(context: &Context) -> Result<IrMutationAttemptEpoch, CanonicalRankedViewErrorV1> {
    context
        .ir_mutation_attempt_epoch()
        .map_err(|_| CanonicalRankedViewErrorV1::MutationEpochUnavailable)
}

pub fn compile_canonical_ranked_view_v1(
    context: &mut Context,
    canonical: &VerifiedCanonicalKernelIrV13,
    final_epoch: u64,
    function_id: &FunctionId,
) -> Result<CheckedCanonicalRankedViewV1, CanonicalRankedViewErrorV1> {
    prepare_canonical_ranked_view_v1(canonical, final_epoch, function_id)?.materialize(context)
}

/// Builds and appends every selected view before stamping one final epoch.
/// The owner must supply an empty module from this context and preflight the
/// sum of `tree_work()` before calling. A post-allocation error poisons its
/// session. No caller callback or already-checked view participates in sealing.
/// The root admits only its bounded `sym_name` identifier attribute and no
/// body-block attributes; unknown metadata is rejected before verification.
pub fn materialize_canonical_ranked_views_v1(
    context: &mut Context,
    root: &ModuleOp,
    plans: Vec<Option<CanonicalRankedViewBlueprintV1>>,
) -> Result<Vec<Option<CheckedCanonicalRankedViewV1>>, CanonicalRankedViewErrorV1> {
    limit(
        "batch plan slots",
        plans.len(),
        MAX_CANONICAL_RANKED_VIEWS_V1,
    )?;
    require_context_identity(context)
        .map_err(|_| CanonicalRankedViewErrorV1::MissingContextIdentity)?;
    mutation_epoch(context)?;
    let mut subject = None;
    let mut functions = BTreeSet::new();
    for plan in plans.iter().flatten() {
        require_write_contracts(&plan.writes)?;
        let binding = (plan.canonical, plan.epoch);
        if subject.is_some_and(|expected| expected != binding) {
            return Err(CanonicalRankedViewErrorV1::InvalidBatch(
                "mixed canonical identities or final epochs",
            ));
        }
        subject = Some(binding);
        if !functions.insert(plan.function.clone()) {
            return Err(CanonicalRankedViewErrorV1::InvalidBatch(
                "duplicate function",
            ));
        }
    }
    let block = checked_empty_batch_root(context, root)?;
    let mut graphs = Vec::with_capacity(plans.len());
    for plan in plans {
        let graph = if let Some(plan) = plan {
            let graph = plan.materialize_unsealed(context)?;
            graph.pliron.get_operation().insert_at_back(block, context);
            Some(graph)
        } else {
            None
        };
        graphs.push(graph);
    }
    let final_mutation_epoch = mutation_epoch(context)?;
    Ok(graphs
        .into_iter()
        .map(|graph| {
            graph.map(|graph| CheckedCanonicalRankedViewV1 {
                graph,
                mutation_epoch: final_mutation_epoch,
            })
        })
        .collect())
}

fn checked_empty_batch_root(
    context: &Context,
    root: &ModuleOp,
) -> Result<Ptr<BasicBlock>, CanonicalRankedViewErrorV1> {
    catch_unwind(AssertUnwindSafe(|| {
        let pointer = root.get_operation();
        if !Operation::is_op::<ModuleOp>(pointer, context) {
            return Err(CanonicalRankedViewErrorV1::InvalidBatchRoot);
        }
        let operation = pointer.deref(context);
        if operation.num_regions() != 1
            || operation.get_num_operands() != 0
            || operation.get_num_results() != 0
            || operation.get_num_successors() != 0
        {
            return Err(CanonicalRankedViewErrorV1::InvalidBatchRoot);
        }
        // Keep verifier attribute enumeration closed and bounded. Read the
        // one permitted payload directly, without printing or cloning it.
        if operation.attributes.0.len() != 1 {
            return Err(CanonicalRankedViewErrorV1::InvalidBatchRoot);
        }
        let name = operation
            .attributes
            .get::<IdentifierAttr>(&ATTR_KEY_SYM_NAME)
            .ok_or(CanonicalRankedViewErrorV1::InvalidBatchRoot)?;
        let name: &pliron::identifier::Identifier = name.as_ref();
        let name: &str = name.as_ref();
        limit(
            "batch root symbol bytes",
            name.len(),
            MAX_CANONICAL_RANKED_ROOT_SYMBOL_BYTES_V1,
        )?;
        let blocks = root
            .get_region(context)
            .deref(context)
            .iter(context)
            .take(2)
            .collect::<Vec<_>>();
        let [block] = blocks.as_slice() else {
            return Err(CanonicalRankedViewErrorV1::InvalidBatchRoot);
        };
        if block.deref(context).get_num_arguments() != 0
            || !block.deref(context).attributes.0.is_empty()
            || block.deref(context).iter(context).next().is_some()
        {
            return Err(CanonicalRankedViewErrorV1::InvalidBatchRoot);
        }
        verify_operation(pointer, context)
            .map_err(|_| CanonicalRankedViewErrorV1::InvalidBatchRoot)?;
        Ok(*block)
    }))
    .unwrap_or(Err(CanonicalRankedViewErrorV1::InvalidBatchRoot))
}

fn limit(
    resource: &'static str,
    actual: usize,
    maximum: usize,
) -> Result<(), CanonicalRankedViewErrorV1> {
    if actual > maximum {
        Err(CanonicalRankedViewErrorV1::Limit {
            resource,
            limit: maximum,
            actual,
        })
    } else {
        Ok(())
    }
}

fn unsupported(
    block: Option<BlockId>,
    operation: Option<usize>,
    reason: &'static str,
) -> CanonicalRankedViewErrorV1 {
    CanonicalRankedViewErrorV1::Unsupported {
        block,
        operation,
        reason,
    }
}

pub fn prepare_canonical_ranked_view_v1(
    canonical: &VerifiedCanonicalKernelIrV13,
    final_epoch: u64,
    function_id: &FunctionId,
) -> Result<CanonicalRankedViewBlueprintV1, CanonicalRankedViewErrorV1> {
    let plan = plan_canonical_ranked_view(canonical, final_epoch, function_id)?;
    require_write_contracts(&plan.writes)?;
    Ok(plan)
}

fn plan_canonical_ranked_view(
    canonical: &VerifiedCanonicalKernelIrV13,
    final_epoch: u64,
    function_id: &FunctionId,
) -> Result<CanonicalRankedViewBlueprintV1, CanonicalRankedViewErrorV1> {
    // The input's private constructor already established semantic validity.
    // Decode its exact bytes, never a caller-supplied parallel Module.
    let module = decode_module_v13(canonical.canonical_bytes())
        .map_err(|_| CanonicalRankedViewErrorV1::CanonicalDecode)?;
    let (ordinal, function) = module
        .functions
        .iter()
        .enumerate()
        .find(|(_, f)| &f.id == function_id)
        .ok_or(CanonicalRankedViewErrorV1::MissingFunction)?;
    let mut planner = Planner::new(*canonical.identity(), final_epoch, function)?;
    planner.launch = launch::CanonicalLaunch::for_function(&module, function_id);
    planner.plan.function_ordinal = ordinal;
    planner.finish(function)
}

fn require_write_contracts(
    writes: &[CanonicalRankedWriteBindingV1],
) -> Result<(), CanonicalRankedViewErrorV1> {
    if let Some(write) = writes.first() {
        return Err(CanonicalRankedViewErrorV1::MissingWriteContracts {
            block: write.source_block,
            operation: write.source_operation,
        });
    }
    Ok(())
}

struct Planner {
    plan: CanonicalRankedViewBlueprintV1,
    facts: BTreeMap<ValueId, Fact>,
    slices: Vec<SliceOrigin>,
    source_entries: BTreeMap<BlockId, usize>,
    source_order: Vec<usize>,
    synthetic_blocks: usize,
    launch: Option<launch::CanonicalLaunch>,
}

impl Planner {
    fn new(
        canonical: VerifiedCanonicalKernelIrIdentityV13,
        epoch: u64,
        function: &Function,
    ) -> Result<Self, CanonicalRankedViewErrorV1> {
        let body = function
            .body
            .as_ref()
            .ok_or_else(|| unsupported(None, None, "external function"))?;
        if body.blocks.is_empty() || !function.signature.results.is_empty() {
            return Err(unsupported(None, None, "empty body or nonvoid result"));
        }
        limit(
            "parameters",
            body.parameters.len(),
            MAX_CANONICAL_RANKED_PARAMETERS_V1,
        )?;
        limit(
            "source blocks",
            body.blocks.len(),
            MAX_CANONICAL_RANKED_SOURCE_BLOCKS_V1,
        )?;
        let operations = body
            .blocks
            .iter()
            .try_fold(0usize, |n, b| n.checked_add(b.operations.len()))
            .ok_or_else(|| unsupported(None, None, "operation count overflow"))?;
        limit(
            "source operations",
            operations,
            MAX_CANONICAL_RANKED_SOURCE_OPERATIONS_V1,
        )?;
        let values = body
            .blocks
            .iter()
            .try_fold(body.parameters.len(), |n, b| {
                b.operations
                    .iter()
                    .try_fold(n.checked_add(b.parameters.len())?, |n, o| {
                        n.checked_add(o.results.len())
                    })
            })
            .ok_or_else(|| unsupported(None, None, "value count overflow"))?;
        limit(
            "source values",
            values,
            MAX_CANONICAL_RANKED_SOURCE_VALUES_V1,
        )?;
        let source_entries = body
            .blocks
            .iter()
            .enumerate()
            .map(|(i, b)| (b.id, i))
            .collect();
        let mut planner = Self {
            plan: CanonicalRankedViewBlueprintV1 {
                canonical,
                epoch,
                function: function.id.clone(),
                function_ordinal: 0,
                arguments: vec![],
                blocks: (0..body.blocks.len())
                    .map(|_| PlannedBlock::default())
                    .collect(),
                emission_order: vec![],
                nodes: 0,
                covered_operations: 0,
                writes: vec![],
                tree_work: 0,
            },
            facts: BTreeMap::new(),
            slices: vec![],
            source_entries,
            source_order: vec![],
            synthetic_blocks: 0,
            launch: None,
        };
        planner.source_order = planner.cfg_order(function)?;
        for (parameter, (&id, ty)) in body
            .parameters
            .iter()
            .zip(&function.signature.parameters)
            .enumerate()
        {
            let fact = match ty {
                Type::Slice(slice)
                    if slice.address_space == AddressSpace::Global
                        && *slice.element == Type::F32 =>
                {
                    let extent = planner.node();
                    planner.plan.arguments.push(extent);
                    let view = planner.node();
                    planner.emit(
                        0,
                        PlannedOp::View {
                            result: view,
                            extent,
                            parameter,
                            access: slice.access,
                        },
                    );
                    let index = planner.slices.len();
                    planner.slices.push(SliceOrigin {
                        parameter,
                        view,
                        access: slice.access,
                    });
                    Fact::Slice(index)
                }
                ty if *ty == Type::INDEX => {
                    let node = planner.node();
                    planner.plan.arguments.push(node);
                    Fact::Index {
                        node,
                        constant: None,
                    }
                }
                ty if *ty == Type::F32 => {
                    let node = planner.node();
                    planner.emit(
                        0,
                        PlannedOp::F32Parameter {
                            result: node,
                            parameter: parameter as u32,
                        },
                    );
                    Fact::F32(node)
                }
                _ => {
                    return Err(unsupported(
                        None,
                        None,
                        "parameter is not an Index, f32, or global f32 slice",
                    ));
                }
            };
            planner.facts.insert(id, fact);
        }
        Ok(planner)
    }

    fn node(&mut self) -> Node {
        let node = Node(self.plan.nodes);
        self.plan.nodes += 1;
        node
    }
    fn emit(&mut self, block: usize, operation: PlannedOp) {
        self.plan.blocks[block].operations.push(operation);
    }
    fn fact(&self, id: ValueId) -> Option<Fact> {
        self.facts.get(&id).copied()
    }
    fn index(&self, id: ValueId) -> Option<Node> {
        match self.fact(id)? {
            Fact::Index { node, .. } => Some(node),
            _ => None,
        }
    }
    fn slice(&self, id: ValueId) -> Option<usize> {
        match self.fact(id)? {
            Fact::Slice(i) | Fact::Capability { slice: i, .. } => Some(i),
            _ => None,
        }
    }
    fn new_block(&mut self) -> Result<usize, CanonicalRankedViewErrorV1> {
        self.synthetic_blocks += 1;
        limit(
            "synthetic blocks",
            self.synthetic_blocks,
            MAX_CANONICAL_RANKED_SYNTHETIC_BLOCKS_V1,
        )?;
        let index = self.plan.blocks.len();
        self.plan.blocks.push(PlannedBlock::default());
        Ok(index)
    }

    fn cfg_order(&self, function: &Function) -> Result<Vec<usize>, CanonicalRankedViewErrorV1> {
        let body = function.body.as_ref().unwrap();
        let mut edges = vec![vec![]; body.blocks.len()];
        let mut incoming = vec![0usize; body.blocks.len()];
        for (i, block) in body.blocks.iter().enumerate() {
            let error = || {
                unsupported(
                    Some(block.id),
                    None,
                    "unsupported CFG, block arguments, or return values",
                )
            };
            if !block.parameters.is_empty() {
                return Err(error());
            }
            let targets = match &block.terminator {
                Some(Terminator::Return { values }) if values.is_empty() => vec![],
                Some(Terminator::Branch { target, arguments }) if arguments.is_empty() => {
                    vec![*target]
                }
                Some(Terminator::ConditionalBranch {
                    then_target,
                    else_target,
                    then_arguments,
                    else_arguments,
                    ..
                }) if then_arguments.is_empty() && else_arguments.is_empty() => {
                    vec![*then_target, *else_target]
                }
                _ => return Err(error()),
            };
            for target in targets {
                let next = *self.source_entries.get(&target).ok_or_else(error)?;
                edges[i].push(next);
                incoming[next] += 1;
            }
        }
        // Only the entry may start a component; every source block is audited.
        if incoming[0] != 0 || incoming.iter().skip(1).any(|n| *n == 0) {
            return Err(unsupported(None, None, "unreachable block or cyclic entry"));
        }
        let mut pending = VecDeque::from([0]);
        let mut order = vec![];
        while let Some(block) = pending.pop_front() {
            order.push(block);
            for &next in &edges[block] {
                incoming[next] -= 1;
                if incoming[next] == 0 {
                    pending.push_back(next);
                }
            }
        }
        if order.len() != body.blocks.len() {
            return Err(unsupported(None, None, "cyclic or unreachable CFG"));
        }
        Ok(order)
    }

    fn finish(
        mut self,
        function: &Function,
    ) -> Result<CanonicalRankedViewBlueprintV1, CanonicalRankedViewErrorV1> {
        let body = function.body.as_ref().unwrap();
        for source in self.source_order.clone() {
            let block = &body.blocks[source];
            let mut current = source;
            for (ordinal, operation) in block.operations.iter().enumerate() {
                let failure = |reason| unsupported(Some(block.id), Some(ordinal), reason);
                let single = || match operation.results.as_slice() {
                    [result] => Ok(result),
                    _ => Err(failure("operation result arity")),
                };
                let fact = match &operation.kind {
                    OperationKind::KernelContextIssue(_) => {
                        single()?;
                        Some(Fact::Context)
                    }
                    OperationKind::GlobalCapabilityBind(bind) => {
                        let result = single()?;
                        let Type::GlobalCapability(capability) = &result.ty else {
                            return Err(failure("capability result type"));
                        };
                        let Some(Fact::Slice(slice)) = self.fact(bind.physical) else {
                            return Err(failure(
                                "capability physical origin is not an exact parameter",
                            ));
                        };
                        if !matches!(self.fact(bind.context), Some(Fact::Context))
                            || capability.element() != &Type::F32
                        {
                            return Err(failure("capability context or element"));
                        }
                        Some(Fact::Capability {
                            slice,
                            role: capability.role(),
                        })
                    }
                    OperationKind::GlobalCapabilityIndex(index) => {
                        single()?;
                        let Some(Fact::Capability { role, .. }) = self.fact(index.capability)
                        else {
                            return Err(failure("index capability origin"));
                        };
                        let exact = match role {
                            GlobalCapabilityRoleV1::ReadOnly
                            | GlobalCapabilityRoleV1::ExclusiveReadWrite => {
                                index.index_space.is_none()
                            }
                            GlobalCapabilityRoleV1::DisjointWrite(contract) => {
                                index.index_space == Some(contract)
                                    && contract.mapping() == GlobalDisjointIndexSpaceV1::Index1d
                            }
                        };
                        if !exact || self.index(index.index).is_none() {
                            return Err(failure("index mapping is not an exact identity alias"));
                        }
                        self.fact(index.index)
                    }
                    OperationKind::Intrinsic(intrinsic) => {
                        single()?;
                        Some(self.project_intrinsic(current, block.id, ordinal, intrinsic.kind)?)
                    }
                    OperationKind::Constant(Constant::Index(bits)) => {
                        single()?;
                        let node = self.node();
                        self.emit(
                            current,
                            PlannedOp::Index {
                                result: node,
                                bits: *bits,
                            },
                        );
                        Some(Fact::Index {
                            node,
                            constant: Some(*bits),
                        })
                    }
                    OperationKind::Constant(Constant::F32Bits(bits)) => {
                        single()?;
                        let node = self.node();
                        self.emit(
                            current,
                            PlannedOp::F32 {
                                result: node,
                                bits: *bits,
                            },
                        );
                        Some(Fact::F32(node))
                    }
                    OperationKind::Binary { op, lhs, rhs } => {
                        if single()?.ty != Type::F32 {
                            return Err(failure("binary scalar type is not f32"));
                        }
                        let (Some(Fact::F32(lhs)), Some(Fact::F32(rhs))) =
                            (self.fact(*lhs), self.fact(*rhs))
                        else {
                            return Err(failure("binary operands lack exact f32 producers"));
                        };
                        let kind = match op {
                            BinaryOp::Add => SemanticTypedBinaryKindAttr::Add,
                            BinaryOp::Subtract => SemanticTypedBinaryKindAttr::Subtract,
                            BinaryOp::Multiply => SemanticTypedBinaryKindAttr::Multiply,
                            BinaryOp::Divide => SemanticTypedBinaryKindAttr::Divide,
                            BinaryOp::Remainder => SemanticTypedBinaryKindAttr::Remainder,
                            _ => return Err(failure("unsupported f32 binary operation")),
                        };
                        let node = self.node();
                        self.emit(
                            current,
                            PlannedOp::F32Binary {
                                result: node,
                                kind,
                                lhs,
                                rhs,
                            },
                        );
                        Some(Fact::F32(node))
                    }
                    OperationKind::SliceLength { slice } => {
                        single()?;
                        let origin = self
                            .slice(*slice)
                            .ok_or_else(|| failure("slice length origin"))?;
                        let view = self.slices[origin].view;
                        let node = self.node();
                        self.emit(current, PlannedOp::Length { result: node, view });
                        Some(Fact::Index {
                            node,
                            constant: None,
                        })
                    }
                    OperationKind::Compare {
                        predicate: ComparePredicate::LessThan,
                        lhs,
                        rhs,
                    } => {
                        single()?;
                        Some(Fact::Predicate {
                            lhs: self
                                .index(*lhs)
                                .ok_or_else(|| failure("comparison lhs is not an exact index"))?,
                            rhs: self
                                .index(*rhs)
                                .ok_or_else(|| failure("comparison rhs is not an exact index"))?,
                        })
                    }
                    OperationKind::Select {
                        condition,
                        true_value,
                        false_value,
                    } => {
                        single()?;
                        if !matches!(self.fact(*condition), Some(Fact::Predicate { .. }))
                            || !matches!(
                                self.fact(*false_value),
                                Some(Fact::Index {
                                    constant: Some(0),
                                    ..
                                })
                            )
                        {
                            return Err(failure(
                                "select requires an exact predicate and zero false arm",
                            ));
                        }
                        Some(Fact::Select {
                            predicate: *condition,
                            index: self
                                .index(*true_value)
                                .ok_or_else(|| failure("select true arm is not an index"))?,
                        })
                    }
                    OperationKind::SliceData { slice } => {
                        single()?;
                        Some(Fact::Pointer {
                            slice: self
                                .slice(*slice)
                                .ok_or_else(|| failure("slice data origin"))?,
                            offset: None,
                        })
                    }
                    OperationKind::GetElementPointer { base, offset } => {
                        single()?;
                        let Some(Fact::Pointer {
                            slice,
                            offset: None,
                        }) = self.fact(*base)
                        else {
                            return Err(failure(
                                "GEP must start at exact slice data; chained offsets unsupported",
                            ));
                        };
                        if !matches!(
                            self.fact(*offset),
                            Some(Fact::Index { .. } | Fact::Select { .. })
                        ) {
                            return Err(failure(
                                "GEP offset is not an exact index or guarded select",
                            ));
                        }
                        Some(Fact::Pointer {
                            slice,
                            offset: Some(*offset),
                        })
                    }
                    OperationKind::GuardedStore {
                        pointer,
                        predicate,
                        value,
                        access,
                    } => {
                        if !operation.results.is_empty() {
                            return Err(failure("store result arity"));
                        }
                        current = self.write(
                            current,
                            block.id,
                            ordinal,
                            *pointer,
                            Some(*predicate),
                            *value,
                            *access,
                        )?;
                        None
                    }
                    OperationKind::Store {
                        pointer,
                        value,
                        access,
                    } => {
                        if !operation.results.is_empty() {
                            return Err(failure("store result arity"));
                        }
                        current = self
                            .write(current, block.id, ordinal, *pointer, None, *value, *access)?;
                        None
                    }
                    _ => {
                        return Err(failure(
                            "operation is outside the closed scalar/global-store projection",
                        ));
                    }
                };
                if let Some(fact) = fact {
                    self.facts.insert(single()?.id, fact);
                }
                self.plan.covered_operations += 1;
            }
            let terminator = match block.terminator.as_ref().unwrap() {
                Terminator::Return { .. } => PlannedTerminator::Return,
                Terminator::Branch { target, .. } => {
                    PlannedTerminator::Branch(self.source_entries[target])
                }
                Terminator::ConditionalBranch {
                    condition,
                    then_target,
                    else_target,
                    ..
                } => {
                    let Some(Fact::Predicate { lhs, rhs }) = self.fact(*condition) else {
                        return Err(unsupported(
                            Some(block.id),
                            None,
                            "branch condition is not an exact index comparison",
                        ));
                    };
                    PlannedTerminator::LessThan {
                        lhs,
                        rhs,
                        yes: self.source_entries[then_target],
                        no: self.source_entries[else_target],
                    }
                }
                _ => unreachable!("CFG preflight"),
            };
            self.plan.blocks[current].terminator = Some(terminator);
            self.plan.emission_order.push(current);
        }
        let operations = self
            .plan
            .blocks
            .iter()
            .map(|b| b.operations.len() + 1)
            .sum::<usize>();
        limit(
            "analysis operations",
            operations,
            crate::MAX_RANKED_BOUNDS_OPERATIONS,
        )?;
        limit(
            "analysis blocks",
            self.plan.blocks.len(),
            crate::MAX_RANKED_BOUNDS_BLOCKS,
        )?;
        // Owner traversal charges every child once at its parent and once when
        // visited. Include the extra child charge when this function is attached.
        self.plan.tree_work = 3 + self.plan.blocks.len() + 2 * operations;
        Ok(self.plan)
    }

    #[allow(clippy::too_many_arguments)]
    fn write(
        &mut self,
        current: usize,
        block: BlockId,
        ordinal: usize,
        pointer: ValueId,
        predicate: Option<ValueId>,
        rhs: ValueId,
        access: MemoryAccess,
    ) -> Result<usize, CanonicalRankedViewErrorV1> {
        let failure = |reason| unsupported(Some(block), Some(ordinal), reason);
        let Some(Fact::Pointer {
            slice,
            offset: Some(offset),
        }) = self.fact(pointer)
        else {
            return Err(failure("store address lacks one exact slice GEP"));
        };
        let Some(Fact::F32(rhs_node)) = self.fact(rhs) else {
            return Err(failure("store RHS is not an exact f32 value"));
        };
        if access.address_space != AddressSpace::Global
            || access.alignment != 4
            || self.slices[slice].access == AccessMode::ReadOnly
        {
            return Err(failure("store access/element stride contract"));
        }
        let index = match self.fact(offset) {
            Some(Fact::Index { node, .. }) => node,
            Some(Fact::Select {
                predicate: selected,
                index,
            }) if predicate == Some(selected) => index,
            _ => {
                return Err(failure(
                    "selected GEP index is not guarded by its exact predicate",
                ));
            }
        };
        let (write_block, continuation) = if let Some(predicate) = predicate {
            let Some(Fact::Predicate { lhs, rhs }) = self.fact(predicate) else {
                return Err(failure("store predicate is not an exact index comparison"));
            };
            let write = self.new_block()?;
            let continuation = self.new_block()?;
            self.plan.blocks[current].terminator = Some(PlannedTerminator::LessThan {
                lhs,
                rhs,
                yes: write,
                no: continuation,
            });
            self.plan.emission_order.push(current);
            self.plan.blocks[write].terminator = Some(PlannedTerminator::Branch(continuation));
            self.plan.emission_order.push(write);
            (write, continuation)
        } else {
            (current, current)
        };
        let origin = &self.slices[slice];
        let view = origin.view;
        let effect = self.plan.writes.len();
        self.plan.writes.push(CanonicalRankedWriteBindingV1 {
            source_block: block,
            source_operation: ordinal,
            parameter: origin.parameter,
            pointer,
            index: offset,
            predicate,
            rhs,
            access,
            analysis_block: write_block,
            analysis_operation: self.plan.blocks[write_block].operations.len(),
            rhs_node,
        });
        self.emit(
            write_block,
            PlannedOp::Write {
                view,
                index,
                rhs: rhs_node,
                effect,
            },
        );
        Ok(continuation)
    }
}

impl CanonicalRankedViewBlueprintV1 {
    /// Upper bound on owner tree-work added by attaching this detached function
    /// to an existing module, including the parent's child-operation charge.
    pub const fn tree_work(&self) -> usize {
        self.tree_work
    }
    pub fn block_count(&self) -> usize {
        self.blocks.len()
    }
    pub const fn covered_source_operations(&self) -> usize {
        self.covered_operations
    }

    /// Materializes and immediately seals one detached function. Use the batch
    /// builder when functions must be attached before their shared final seal.
    pub fn materialize(
        self,
        context: &mut Context,
    ) -> Result<CheckedCanonicalRankedViewV1, CanonicalRankedViewErrorV1> {
        let graph = self.materialize_unsealed(context)?;
        Ok(CheckedCanonicalRankedViewV1 {
            graph,
            mutation_epoch: mutation_epoch(context)?,
        })
    }

    fn materialize_unsealed(
        self,
        context: &mut Context,
    ) -> Result<UnsealedCanonicalRankedViewV1, CanonicalRankedViewErrorV1> {
        require_write_contracts(&self.writes)?;
        let context_identity = require_context_identity(context)
            .map_err(|_| CanonicalRankedViewErrorV1::MissingContextIdentity)?;
        mutation_epoch(context)?;
        let index_type: TypeHandle = IndexType::get(context).into();
        let function_type =
            FunctionType::get(context, vec![index_type; self.arguments.len()], vec![]);
        let name = format!("canonical_ranked_view_f{}", self.function_ordinal);
        let function = FuncOp::new(
            context,
            name.try_into()
                .map_err(|_| CanonicalRankedViewErrorV1::Materialization("function name"))?,
            function_type,
        );
        let mut blocks = vec![function.get_entry_block(context)];
        for _ in 1..self.blocks.len() {
            let block = BasicBlock::new(context, None, vec![]);
            block.insert_at_back(function.get_region(context), context);
            blocks.push(block);
        }
        let mut nodes = vec![None; self.nodes];
        for (node, argument) in self
            .arguments
            .iter()
            .zip(blocks[0].deref(context).arguments())
        {
            nodes[node.0] = Some(argument);
        }
        let f32_type = SemanticTypedScalarV1::new(SemanticScalarKindAttr::Float, 32).unwrap();
        for &block_index in &self.emission_order {
            let block = blocks[block_index];
            for (operation_index, operation) in
                self.blocks[block_index].operations.iter().enumerate()
            {
                let (op, result): (Ptr<Operation>, Option<(Node, Value)>) = match operation {
                    PlannedOp::View {
                        result,
                        extent,
                        parameter,
                        access,
                    } => {
                        let ty = RankedViewType::new(
                            context,
                            32,
                            *access != AccessMode::ReadOnly,
                            vec![DYNAMIC_EXTENT],
                        )
                        .map_err(|_| CanonicalRankedViewErrorV1::Materialization("view type"))?;
                        let op = RankedViewOp::new_in_space_with_allocation_contract(
                            context,
                            ty,
                            vec![get(&nodes, *extent)?],
                            MemorySpaceAttr::Global,
                            *parameter as u64 + 1,
                            0,
                        )
                        .map_err(|_| CanonicalRankedViewErrorV1::Materialization("view origin"))?;
                        (op.get_operation(), Some((*result, op.result(context))))
                    }
                    PlannedOp::Index { result, bits } => {
                        let op = IndexConstantOp::new(context, *bits);
                        (op.get_operation(), Some((*result, op.result(context))))
                    }
                    PlannedOp::Invocation {
                        result,
                        axis,
                        extent,
                    } => {
                        let op = InvocationIndexOp::new(context, *axis, *extent);
                        (op.get_operation(), Some((*result, op.result(context))))
                    }
                    PlannedOp::IndexBinary {
                        result,
                        kind,
                        lhs,
                        rhs,
                    } => {
                        let op = IndexBinaryOp::new(
                            context,
                            *kind,
                            get(&nodes, *lhs)?,
                            get(&nodes, *rhs)?,
                        );
                        (op.get_operation(), Some((*result, op.result(context))))
                    }
                    PlannedOp::F32 { result, bits } => {
                        let op = SemanticTypedConstantOp::new(context, u64::from(*bits), f32_type);
                        (op.get_operation(), Some((*result, op.result(context))))
                    }
                    PlannedOp::F32Parameter { result, parameter } => {
                        let op = SemanticTypedSymbolOp::new(context, *parameter, f32_type);
                        (op.get_operation(), Some((*result, op.result(context))))
                    }
                    PlannedOp::F32Binary {
                        result,
                        kind,
                        lhs,
                        rhs,
                    } => {
                        let op = SemanticTypedBinaryOp::new(
                            context,
                            *kind,
                            SemanticOverflowAttr::Wrapping,
                            f32_type,
                            get(&nodes, *lhs)?,
                            get(&nodes, *rhs)?,
                        );
                        (op.get_operation(), Some((*result, op.result(context))))
                    }
                    PlannedOp::Length { result, view } => {
                        let op =
                            DimensionOp::new(context, get(&nodes, *view)?, 0).map_err(|_| {
                                CanonicalRankedViewErrorV1::Materialization("slice length")
                            })?;
                        (op.get_operation(), Some((*result, op.result(context))))
                    }
                    PlannedOp::Write {
                        view,
                        index,
                        rhs,
                        effect,
                    } => {
                        let _rhs = get(&nodes, *rhs)?;
                        let binding = &self.writes[*effect];
                        if binding.rhs_node != *rhs
                            || binding.analysis_block != block_index
                            || binding.analysis_operation != operation_index
                        {
                            return Err(CanonicalRankedViewErrorV1::Materialization(
                                "write correspondence",
                            ));
                        }
                        let op = RankedAccessOp::new(
                            context,
                            AccessKindAttr::Write,
                            get(&nodes, *view)?,
                            vec![get(&nodes, *index)?],
                        )
                        .map_err(|_| CanonicalRankedViewErrorV1::Materialization("write"))?;
                        (op.get_operation(), None)
                    }
                };
                op.insert_at_back(block, context);
                if let Some((node, value)) = result {
                    nodes[node.0] = Some(value);
                }
            }
            let op = match self.blocks[block_index]
                .terminator
                .as_ref()
                .ok_or(CanonicalRankedViewErrorV1::Materialization("terminator"))?
            {
                PlannedTerminator::LessThan { lhs, rhs, yes, no } => IndexLessThanBranchOp::new(
                    context,
                    get(&nodes, *lhs)?,
                    get(&nodes, *rhs)?,
                    blocks[*yes],
                    blocks[*no],
                )
                .get_operation(),
                PlannedTerminator::Branch(target) => {
                    BranchOp::new(context, blocks[*target]).get_operation()
                }
                PlannedTerminator::Return => ReturnOp::new(context).get_operation(),
            };
            op.insert_at_back(block, context);
        }
        let structural = derive_pliron_ir_structural_identity_v1(context, &function)
            .map_err(CanonicalRankedViewErrorV1::StructuralIdentity)?;
        let nodes = nodes.into_iter().collect::<Option<Vec<_>>>().ok_or(
            CanonicalRankedViewErrorV1::Materialization("unmaterialized node"),
        )?;
        let live_operations = blocks
            .iter()
            .flat_map(|block| block.deref(context).iter(context).collect::<Vec<_>>())
            .collect::<Vec<_>>();
        Ok(UnsealedCanonicalRankedViewV1 {
            canonical: self.canonical,
            epoch: self.epoch,
            function: self.function,
            context: context_identity,
            pliron: function,
            structural,
            live_blocks: blocks.into_boxed_slice(),
            live_operations: live_operations.into_boxed_slice(),
            writes: self.writes.into_boxed_slice(),
            nodes: nodes.into_boxed_slice(),
            covered_operations: self.covered_operations,
        })
    }
}

fn get(nodes: &[Option<Value>], node: Node) -> Result<Value, CanonicalRankedViewErrorV1> {
    nodes
        .get(node.0)
        .copied()
        .flatten()
        .ok_or(CanonicalRankedViewErrorV1::Materialization(
            "SSA dependency",
        ))
}

#[cfg(test)]
#[path = "canonical_ranked_view_v1/tests.rs"]
mod tests;
