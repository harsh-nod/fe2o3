//! Whole-function bounds verification for target-neutral `kernel.*` Pliron IR.
//!
//! Local operation/type invariants remain in `dialect-kernel`. This module is
//! the fixed `MemoryBounds` analysis stage: it intersects facts from every CFG
//! predecessor and accepts an access only when each dimension is statically
//! in range or protected by an `index < extent` fact on every incoming path.

use std::{
    collections::{HashMap, VecDeque},
    fmt,
};

use dialect_gpu::{BarrierOp, ExecutionLayoutOp, FenceOp};
use dialect_kernel::{
    AccessKindAttr, AnalysisSplitOp, BranchArgsOp, BranchOp, CheckedTiledIndex2DOp,
    DeterministicJoinOp, DimensionOp, IndexBinaryOp, IndexConstantOp, IndexEqualBranchArgsOp,
    IndexEqualBranchOp, IndexLessThanBranchArgsOp, IndexLessThanBranchOp, InvocationIndexOp,
    MAX_RANKED_MEMORY_RANK, RankedAccessOp, RankedViewOp, RankedViewType, RequireEquivalentOp,
    ReturnOp, SemanticBinaryOp, SemanticConstantOp, SemanticSymbolOp, TensorLayoutOp,
    ranked_view_type,
};
use pliron::{
    builtin::{op_interfaces::OneRegionInterface, ops::FuncOp},
    common_traits::Named,
    context::Context,
    linked_list::ContainsLinkedList,
    op::Op,
    operation::{Operation, verify_operation},
    r#type::TypedHandle,
    value::Value,
};

use crate::pliron_analysis_manager::PlironAnalysisManagerV1;
use crate::{KernelCheckPassKindV1, SparseIndexAnalysisV1, SparseIndexFailureV1};

pub const MAX_RANKED_BOUNDS_BLOCKS: usize = 1_024;
pub const MAX_RANKED_BOUNDS_OPERATIONS: usize = 65_536;
pub const MAX_RANKED_BOUNDS_EDGES: usize = MAX_RANKED_BOUNDS_BLOCKS * 2;
pub const MAX_RANKED_BOUNDS_FACTS: usize = MAX_RANKED_BOUNDS_BLOCKS;
pub const MAX_RANKED_BOUNDS_OPERATION_ITEMS: usize =
    MAX_RANKED_BOUNDS_OPERATIONS * (MAX_RANKED_MEMORY_RANK + 4);
pub const MAX_RANKED_BOUNDS_FINDINGS: usize = 4_096;
pub const MAX_RANKED_BOUNDS_STORAGE_ITEMS: usize = 131_072;
pub const MAX_RANKED_BOUNDS_WORK_UNITS: usize = MAX_RANKED_BOUNDS_OPERATIONS * 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RankedBoundsStatusV1 {
    Clean,
    Rejected,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RankedBoundsFindingV1 {
    StructuralVerificationFailed,
    ResourceLimitExceeded {
        resource: &'static str,
        limit: usize,
        actual: usize,
    },
    UnreachableBlock {
        block: usize,
    },
    UnsupportedTerminator {
        block: usize,
        operation: String,
    },
    UnsupportedOperation {
        block: usize,
        operation: usize,
        kind: String,
    },
    SparseIndexAnalysisFailed {
        detail: String,
    },
    StaticOutOfBounds {
        block: usize,
        operation: usize,
        access: AccessKindAttr,
        view: String,
        dimension: usize,
        index: u64,
        extent: u64,
    },
    UnprovedBound {
        block: usize,
        operation: usize,
        access: AccessKindAttr,
        view: String,
        dimension: usize,
        index: String,
        extent: String,
    },
}

impl fmt::Display for RankedBoundsFindingV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StructuralVerificationFailed => formatter.write_str(
                "error[FE2O3-BOUNDS-000]: Pliron structural verification failed before bounds analysis",
            ),
            Self::ResourceLimitExceeded {
                resource,
                limit,
                actual,
            } => write!(
                formatter,
                "error[FE2O3-BOUNDS-003]: {resource} count {actual} exceeds analysis limit {limit}",
            ),
            Self::UnreachableBlock { block } => write!(
                formatter,
                "error[FE2O3-BOUNDS-003]: block {block} is unreachable in the closed kernel CFG",
            ),
            Self::UnsupportedTerminator { block, operation } => write!(
                formatter,
                "error[FE2O3-BOUNDS-003]: block {block} uses unsupported terminator {operation}",
            ),
            Self::UnsupportedOperation {
                block,
                operation,
                kind,
            } => write!(
                formatter,
                "error[FE2O3-BOUNDS-003]: block {block} op {operation} uses unsupported operation {kind}",
            ),
            Self::SparseIndexAnalysisFailed { detail } => write!(
                formatter,
                "error[FE2O3-BOUNDS-003]: sparse index analysis failed before bounds verification: {detail}",
            ),
            Self::StaticOutOfBounds {
                block,
                operation,
                access,
                view,
                dimension,
                index,
                extent,
            } => write!(
                formatter,
                "error[FE2O3-BOUNDS-001]: statically out-of-bounds {access:?} at block {block} op {operation}; access: {view} dimension {dimension}; required: {index} < {extent}",
            ),
            Self::UnprovedBound {
                block,
                operation,
                access,
                view,
                dimension,
                index,
                extent,
            } => write!(
                formatter,
                "error[FE2O3-BOUNDS-002]: cannot prove {access:?} is in bounds at block {block} op {operation}; access: {view} dimension {dimension}; unproven bound: {index} < {extent}; help: guard every path to the access or use an explicitly checked access",
            ),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RankedBoundsReportV1 {
    findings: Vec<RankedBoundsFindingV1>,
}

impl RankedBoundsReportV1 {
    pub const fn pass(&self) -> KernelCheckPassKindV1 {
        KernelCheckPassKindV1::MemoryBounds
    }

    pub fn status(&self) -> RankedBoundsStatusV1 {
        if self.findings.is_empty() {
            RankedBoundsStatusV1::Clean
        } else {
            RankedBoundsStatusV1::Rejected
        }
    }

    pub fn findings(&self) -> &[RankedBoundsFindingV1] {
        &self.findings
    }

    pub fn is_clean(&self) -> bool {
        self.findings.is_empty()
    }

    pub const fn grants_compiler_refinement_authority(&self) -> bool {
        false
    }

    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

/// Terminal compile-time failure from the ranked-memory bounds stage.
///
/// This error is the lowering-facing API: callers must not lower the function
/// when it is returned. It retains every stable finding so the frontend can
/// render all unsafe dimensions in one diagnostic batch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RankedBoundsCheckErrorV1 {
    report: RankedBoundsReportV1,
}

impl RankedBoundsCheckErrorV1 {
    pub fn report(&self) -> &RankedBoundsReportV1 {
        &self.report
    }
}

impl fmt::Display for RankedBoundsCheckErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, finding) in self.report.findings().iter().enumerate() {
            if index != 0 {
                formatter.write_str("\n")?;
            }
            finding.fmt(formatter)?;
        }
        Ok(())
    }
}

impl std::error::Error for RankedBoundsCheckErrorV1 {}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum IndexExpr {
    Constant(u64),
    Dimension { view: Value, dimension: usize },
    Value(Value),
}

impl IndexExpr {
    fn describe(self, context: &Context) -> String {
        match self {
            Self::Constant(value) => value.to_string(),
            Self::Dimension { view, dimension } => {
                format!("{}.dim<{dimension}>()", view.unique_name(context))
            }
            Self::Value(value) => value.unique_name(context).to_string(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct LessThanFact {
    lhs: IndexExpr,
    rhs: IndexExpr,
}

#[derive(Clone, Copy)]
struct PredecessorEdge {
    block: usize,
    guard_fact: Option<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RankedOperationKind {
    RankedView,
    IndexConstant,
    InvocationIndex,
    IndexBinary,
    DeterministicJoin,
    CheckedTiledIndex2D,
    Dimension,
    RankedAccess,
    IndexLessThanBranch,
    IndexLessThanBranchArgs,
    IndexEqualBranch,
    IndexEqualBranchArgs,
    AnalysisSplit,
    Branch,
    BranchArgs,
    Return,
    Barrier,
    ExecutionLayout,
    Fence,
    SemanticSymbol,
    SemanticConstant,
    SemanticBinary,
    RequireEquivalent,
    TensorLayout,
}

impl RankedOperationKind {
    const fn is_terminator(self) -> bool {
        matches!(
            self,
            Self::IndexLessThanBranch
                | Self::IndexLessThanBranchArgs
                | Self::IndexEqualBranch
                | Self::IndexEqualBranchArgs
                | Self::AnalysisSplit
                | Self::Branch
                | Self::BranchArgs
                | Self::Return
        )
    }
}

fn ranked_operation_kind(operation: &dyn Op) -> Option<RankedOperationKind> {
    if operation.downcast_ref::<RankedViewOp>().is_some() {
        Some(RankedOperationKind::RankedView)
    } else if operation.downcast_ref::<IndexConstantOp>().is_some() {
        Some(RankedOperationKind::IndexConstant)
    } else if operation.downcast_ref::<InvocationIndexOp>().is_some() {
        Some(RankedOperationKind::InvocationIndex)
    } else if operation.downcast_ref::<IndexBinaryOp>().is_some() {
        Some(RankedOperationKind::IndexBinary)
    } else if operation.downcast_ref::<DeterministicJoinOp>().is_some() {
        Some(RankedOperationKind::DeterministicJoin)
    } else if operation.downcast_ref::<CheckedTiledIndex2DOp>().is_some() {
        Some(RankedOperationKind::CheckedTiledIndex2D)
    } else if operation.downcast_ref::<DimensionOp>().is_some() {
        Some(RankedOperationKind::Dimension)
    } else if operation.downcast_ref::<RankedAccessOp>().is_some() {
        Some(RankedOperationKind::RankedAccess)
    } else if operation.downcast_ref::<IndexLessThanBranchOp>().is_some() {
        Some(RankedOperationKind::IndexLessThanBranch)
    } else if operation
        .downcast_ref::<IndexLessThanBranchArgsOp>()
        .is_some()
    {
        Some(RankedOperationKind::IndexLessThanBranchArgs)
    } else if operation.downcast_ref::<IndexEqualBranchOp>().is_some() {
        Some(RankedOperationKind::IndexEqualBranch)
    } else if operation.downcast_ref::<IndexEqualBranchArgsOp>().is_some() {
        Some(RankedOperationKind::IndexEqualBranchArgs)
    } else if operation.downcast_ref::<AnalysisSplitOp>().is_some() {
        Some(RankedOperationKind::AnalysisSplit)
    } else if operation.downcast_ref::<BranchOp>().is_some() {
        Some(RankedOperationKind::Branch)
    } else if operation.downcast_ref::<BranchArgsOp>().is_some() {
        Some(RankedOperationKind::BranchArgs)
    } else if operation.downcast_ref::<ReturnOp>().is_some() {
        Some(RankedOperationKind::Return)
    } else if operation.downcast_ref::<BarrierOp>().is_some() {
        Some(RankedOperationKind::Barrier)
    } else if operation.downcast_ref::<ExecutionLayoutOp>().is_some() {
        Some(RankedOperationKind::ExecutionLayout)
    } else if operation.downcast_ref::<FenceOp>().is_some() {
        Some(RankedOperationKind::Fence)
    } else if operation.downcast_ref::<SemanticSymbolOp>().is_some() {
        Some(RankedOperationKind::SemanticSymbol)
    } else if operation.downcast_ref::<SemanticConstantOp>().is_some() {
        Some(RankedOperationKind::SemanticConstant)
    } else if operation.downcast_ref::<SemanticBinaryOp>().is_some() {
        Some(RankedOperationKind::SemanticBinary)
    } else if operation.downcast_ref::<RequireEquivalentOp>().is_some() {
        Some(RankedOperationKind::RequireEquivalent)
    } else if operation.downcast_ref::<TensorLayoutOp>().is_some() {
        Some(RankedOperationKind::TensorLayout)
    } else {
        None
    }
}

#[derive(Clone, Copy)]
enum RankedBoundsResource {
    Blocks,
    Operations,
    Edges,
    Facts,
    OperationItems,
    Findings,
    StorageItems,
    WorkUnits,
}

impl RankedBoundsResource {
    const fn description(self) -> &'static str {
        match self {
            Self::Blocks => "basic block",
            Self::Operations => "operation",
            Self::Edges => "CFG edge",
            Self::Facts => "guard fact",
            Self::OperationItems => "operation component",
            Self::Findings => "finding",
            Self::StorageItems => "analysis storage item",
            Self::WorkUnits => "analysis work unit",
        }
    }

    const fn limit(self) -> usize {
        match self {
            Self::Blocks => MAX_RANKED_BOUNDS_BLOCKS,
            Self::Operations => MAX_RANKED_BOUNDS_OPERATIONS,
            Self::Edges => MAX_RANKED_BOUNDS_EDGES,
            Self::Facts => MAX_RANKED_BOUNDS_FACTS,
            Self::OperationItems => MAX_RANKED_BOUNDS_OPERATION_ITEMS,
            Self::Findings => MAX_RANKED_BOUNDS_FINDINGS,
            Self::StorageItems => MAX_RANKED_BOUNDS_STORAGE_ITEMS,
            Self::WorkUnits => MAX_RANKED_BOUNDS_WORK_UNITS,
        }
    }
}

#[derive(Default)]
struct RankedBoundsBudget {
    blocks: usize,
    operations: usize,
    edges: usize,
    facts: usize,
    operation_items: usize,
    findings: usize,
    storage_items: usize,
    work_units: usize,
}

impl RankedBoundsBudget {
    fn reserve(
        &mut self,
        resource: RankedBoundsResource,
        amount: usize,
    ) -> Result<(), RankedBoundsFindingV1> {
        let current = match resource {
            RankedBoundsResource::Blocks => self.blocks,
            RankedBoundsResource::Operations => self.operations,
            RankedBoundsResource::Edges => self.edges,
            RankedBoundsResource::Facts => self.facts,
            RankedBoundsResource::OperationItems => self.operation_items,
            RankedBoundsResource::Findings => self.findings,
            RankedBoundsResource::StorageItems => self.storage_items,
            RankedBoundsResource::WorkUnits => self.work_units,
        };
        let actual = current.saturating_add(amount);
        if actual > resource.limit() {
            return Err(RankedBoundsFindingV1::ResourceLimitExceeded {
                resource: resource.description(),
                limit: resource.limit(),
                actual,
            });
        }
        match resource {
            RankedBoundsResource::Blocks => self.blocks = actual,
            RankedBoundsResource::Operations => self.operations = actual,
            RankedBoundsResource::Edges => self.edges = actual,
            RankedBoundsResource::Facts => self.facts = actual,
            RankedBoundsResource::OperationItems => self.operation_items = actual,
            RankedBoundsResource::Findings => self.findings = actual,
            RankedBoundsResource::StorageItems => self.storage_items = actual,
            RankedBoundsResource::WorkUnits => self.work_units = actual,
        }
        Ok(())
    }

    fn storage(&mut self, amount: usize) -> Result<(), RankedBoundsFindingV1> {
        self.reserve(RankedBoundsResource::StorageItems, amount)
    }

    fn work(&mut self, amount: usize) -> Result<(), RankedBoundsFindingV1> {
        self.reserve(RankedBoundsResource::WorkUnits, amount)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FactSet {
    words: Vec<u64>,
}

impl FactSet {
    fn empty(fact_count: usize) -> Self {
        Self {
            words: vec![0; fact_count.div_ceil(u64::BITS as usize)],
        }
    }

    fn full(fact_count: usize) -> Self {
        let mut words = vec![u64::MAX; fact_count.div_ceil(u64::BITS as usize)];
        if let Some(last) = words.last_mut() {
            let used = fact_count % u64::BITS as usize;
            if used != 0 {
                *last = (1_u64 << used) - 1;
            }
        }
        Self { words }
    }

    fn insert(&mut self, fact: usize) {
        self.words[fact / u64::BITS as usize] |= 1_u64 << (fact % u64::BITS as usize);
    }

    fn contains(&self, fact: usize) -> bool {
        self.words[fact / u64::BITS as usize] & (1_u64 << (fact % u64::BITS as usize)) != 0
    }

    fn intersect_edge(&mut self, source: &Self, guard_fact: Option<usize>) {
        for (word_index, word) in self.words.iter_mut().enumerate() {
            let mut source_word = source.words[word_index];
            if let Some(fact) = guard_fact
                && fact / u64::BITS as usize == word_index
            {
                source_word |= 1_u64 << (fact % u64::BITS as usize);
            }
            *word &= source_word;
        }
    }
}

/// Runs the target-neutral ranked-memory bounds stage for one Pliron function.
///
/// The function must already contain `kernel.ranked_view`, `kernel.dim`,
/// `kernel.access`, and the closed kernel CFG terminators. No GEMM operation,
/// schedule, tile, target, or device profile participates in this analysis.
pub fn run_pliron_ranked_bounds_check_v1(
    context: &Context,
    function: &FuncOp,
) -> RankedBoundsReportV1 {
    let mut analyses = PlironAnalysisManagerV1::new(function);
    run_pliron_ranked_bounds_check_with_analyses_v1(context, function, &mut analyses)
}

pub(crate) fn run_pliron_ranked_bounds_check_with_analyses_v1(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
) -> RankedBoundsReportV1 {
    let mut budget = RankedBoundsBudget::default();
    let region = function.get_region(context);
    let mut blocks = Vec::new();
    for block in region.deref(context).iter(context) {
        if let Err(finding) = budget.reserve(RankedBoundsResource::Blocks, 1) {
            return finding_failure(finding);
        }
        if let Err(finding) = budget.storage(1) {
            return finding_failure(finding);
        }
        blocks.push(block);
    }
    if blocks.is_empty() {
        return structural_failure();
    }

    // Close the accepted language before recursive Pliron verification. Every
    // admitted body operation is regionless, so an unknown operation cannot
    // hide an unmetered nested graph from this analysis.
    for (block_index, block) in blocks.iter().enumerate() {
        let terminator = block.deref(context).get_terminator(context);
        for (operation_index, operation_pointer) in block.deref(context).iter(context).enumerate() {
            if let Err(finding) = budget.reserve(RankedBoundsResource::Operations, 1) {
                return finding_failure(finding);
            }
            let operation = Operation::get_op_dyn(operation_pointer, context);
            let Some(kind) = ranked_operation_kind(operation.as_ref()) else {
                let finding = if terminator == Some(operation_pointer) {
                    RankedBoundsFindingV1::UnsupportedTerminator {
                        block: block_index,
                        operation: operation.get_opid().to_string(),
                    }
                } else {
                    RankedBoundsFindingV1::UnsupportedOperation {
                        block: block_index,
                        operation: operation_index,
                        kind: operation.get_opid().to_string(),
                    }
                };
                return finding_failure(finding);
            };
            if kind.is_terminator() != (terminator == Some(operation_pointer)) {
                return structural_failure();
            }

            let raw = operation_pointer.deref(context);
            if raw.num_regions() != 0 {
                return structural_failure();
            }
            let Some(operation_items) = raw
                .get_num_operands()
                .checked_add(raw.get_num_results())
                .and_then(|total| total.checked_add(raw.get_num_successors()))
                .and_then(|total| total.checked_add(raw.attributes.0.len()))
            else {
                return resource_failure(
                    RankedBoundsResource::OperationItems.description(),
                    RankedBoundsResource::OperationItems.limit(),
                    usize::MAX,
                );
            };
            if let Err(finding) =
                budget.reserve(RankedBoundsResource::OperationItems, operation_items)
            {
                return finding_failure(finding);
            }
            if let Err(finding) =
                budget.reserve(RankedBoundsResource::Edges, raw.get_num_successors())
            {
                return finding_failure(finding);
            }
            if let Err(finding) = budget.work(operation_items.saturating_add(1)) {
                return finding_failure(finding);
            }
        }
    }

    if verify_operation(function.get_operation(), context).is_err() {
        return structural_failure();
    }

    analyses.prepare_sparse_indices(context, function);
    let sparse_indices = match analyses.sparse_indices() {
        Ok(analysis) => analysis,
        Err(failure) => {
            return finding_failure(sparse_index_failure(failure));
        }
    };

    if let Err(finding) = budget.storage(blocks.len().saturating_mul(3)) {
        return finding_failure(finding);
    }
    if let Err(finding) = budget.work(blocks.len()) {
        return finding_failure(finding);
    }
    let indices = blocks
        .iter()
        .enumerate()
        .map(|(index, block)| (*block, index))
        .collect::<HashMap<_, _>>();
    let mut successors = vec![Vec::new(); blocks.len()];
    let mut predecessors = vec![Vec::new(); blocks.len()];
    let mut findings = Vec::new();
    let mut fact_indices = HashMap::new();

    for (block_index, block) in blocks.iter().enumerate() {
        let Some(terminator) = block.deref(context).get_terminator(context) else {
            return structural_failure();
        };
        let operation = Operation::get_op_dyn(terminator, context);
        let operands = operation
            .downcast_ref::<IndexLessThanBranchOp>()
            .map(|branch| (branch.lhs(context), branch.rhs(context)))
            .or_else(|| {
                operation
                    .downcast_ref::<IndexLessThanBranchArgsOp>()
                    .map(|branch| (branch.lhs(context), branch.rhs(context)))
            });
        let guard_fact = if let Some((lhs, rhs)) = operands {
            let fact = LessThanFact {
                lhs: canonical_index_expr(lhs, context),
                rhs: canonical_index_expr(rhs, context),
            };
            if let Some(index) = fact_indices.get(&fact).copied() {
                Some(index)
            } else {
                if let Err(finding) = budget.reserve(RankedBoundsResource::Facts, 1) {
                    return finding_failure(finding);
                }
                if let Err(finding) = budget.storage(1) {
                    return finding_failure(finding);
                }
                let next = fact_indices.len();
                fact_indices.insert(fact, next);
                Some(next)
            }
        } else {
            None
        };

        let raw = terminator.deref(context);
        for (successor_index, successor) in raw.successors().enumerate() {
            if let Err(finding) = budget.work(1) {
                return finding_failure(finding);
            }
            let Some(target) = indices.get(&successor).copied() else {
                if let Err(finding) = push_finding(
                    &mut findings,
                    &mut budget,
                    RankedBoundsFindingV1::UnsupportedTerminator {
                        block: block_index,
                        operation: "successor outside function region".to_owned(),
                    },
                ) {
                    return finding_failure(finding);
                }
                continue;
            };
            successors[block_index].push(target);
            predecessors[target].push(PredecessorEdge {
                block: block_index,
                guard_fact: (successor_index == 0).then_some(guard_fact).flatten(),
            });
        }
    }

    if let Err(finding) = budget.storage(budget.edges.saturating_mul(2)) {
        return finding_failure(finding);
    }
    if let Err(finding) = budget.storage(blocks.len().saturating_mul(2)) {
        return finding_failure(finding);
    }
    let reachable = match reachable_blocks(&successors, &mut budget) {
        Ok(reachable) => reachable,
        Err(finding) => return finding_failure(finding),
    };
    for (block, is_reachable) in reachable.iter().copied().enumerate() {
        if !is_reachable
            && let Err(finding) = push_finding(
                &mut findings,
                &mut budget,
                RankedBoundsFindingV1::UnreachableBlock { block },
            )
        {
            return finding_failure(finding);
        }
    }

    let fact_count = fact_indices.len();
    let fact_words = fact_count.div_ceil(u64::BITS as usize);
    let Some(input_words) = blocks.len().checked_mul(fact_words) else {
        return resource_failure(
            RankedBoundsResource::StorageItems.description(),
            RankedBoundsResource::StorageItems.limit(),
            usize::MAX,
        );
    };
    let Some(dataflow_storage) = blocks
        .len()
        .checked_mul(3)
        .and_then(|outer| outer.checked_add(input_words))
    else {
        return resource_failure(
            RankedBoundsResource::StorageItems.description(),
            RankedBoundsResource::StorageItems.limit(),
            usize::MAX,
        );
    };
    if let Err(finding) = budget.storage(dataflow_storage) {
        return finding_failure(finding);
    }
    let mut inputs = (0..blocks.len())
        .map(|block| {
            if block == 0 {
                FactSet::empty(fact_count)
            } else {
                FactSet::full(fact_count)
            }
        })
        .collect::<Vec<_>>();
    let mut pending = (0..blocks.len())
        .filter(|block| reachable[*block])
        .collect::<VecDeque<_>>();
    let mut queued = reachable.clone();
    while let Some(block) = pending.pop_front() {
        queued[block] = false;
        let next = if block == 0 {
            FactSet::empty(fact_count)
        } else {
            match intersect_predecessor_facts(
                block,
                &predecessors,
                &inputs,
                fact_count,
                &mut budget,
            ) {
                Ok(next) => next,
                Err(finding) => return finding_failure(finding),
            }
        };
        if let Err(finding) = budget.work(1) {
            return finding_failure(finding);
        }
        if next != inputs[block] {
            inputs[block] = next;
            for successor in &successors[block] {
                if let Err(finding) = budget.work(1) {
                    return finding_failure(finding);
                }
                if reachable[*successor] && !queued[*successor] {
                    queued[*successor] = true;
                    pending.push_back(*successor);
                }
            }
        }
    }

    for (block_index, block) in blocks.iter().enumerate() {
        if !reachable[block_index] {
            continue;
        }
        for (operation_index, operation) in block.deref(context).iter(context).enumerate() {
            if let Err(finding) = budget.work(1) {
                return finding_failure(finding);
            }
            let operation = Operation::get_op_dyn(operation, context);
            if let Some(access) = operation.downcast_ref::<RankedAccessOp>()
                && let Err(finding) = verify_access(
                    access,
                    block_index,
                    operation_index,
                    &mut AccessCheck {
                        facts: &inputs[block_index],
                        fact_indices: &fact_indices,
                        context,
                        sparse_indices,
                        findings: &mut findings,
                        budget: &mut budget,
                    },
                )
            {
                return finding_failure(finding);
            }
        }
    }

    RankedBoundsReportV1 { findings }
}

/// Enforces the ranked-memory stage as a compile-time pre-lowering gate.
///
/// A clean result is still descriptive and grants no authority. Any malformed,
/// static-out-of-bounds, or unproved access returns a terminal error; callers
/// must not fall back to unchecked lowering.
pub fn require_pliron_ranked_bounds_before_lowering_v1(
    context: &Context,
    function: &FuncOp,
) -> Result<RankedBoundsReportV1, RankedBoundsCheckErrorV1> {
    let report = run_pliron_ranked_bounds_check_v1(context, function);
    if report.is_clean() {
        Ok(report)
    } else {
        Err(RankedBoundsCheckErrorV1 { report })
    }
}

pub(crate) fn require_pliron_ranked_bounds_with_analyses_v1(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
) -> Result<RankedBoundsReportV1, RankedBoundsCheckErrorV1> {
    let report = run_pliron_ranked_bounds_check_with_analyses_v1(context, function, analyses);
    if report.is_clean() {
        Ok(report)
    } else {
        Err(RankedBoundsCheckErrorV1 { report })
    }
}

fn resource_failure(resource: &'static str, limit: usize, actual: usize) -> RankedBoundsReportV1 {
    finding_failure(RankedBoundsFindingV1::ResourceLimitExceeded {
        resource,
        limit,
        actual,
    })
}

fn structural_failure() -> RankedBoundsReportV1 {
    finding_failure(RankedBoundsFindingV1::StructuralVerificationFailed)
}

fn finding_failure(finding: RankedBoundsFindingV1) -> RankedBoundsReportV1 {
    RankedBoundsReportV1 {
        findings: vec![finding],
    }
}

fn push_finding(
    findings: &mut Vec<RankedBoundsFindingV1>,
    budget: &mut RankedBoundsBudget,
    finding: RankedBoundsFindingV1,
) -> Result<(), RankedBoundsFindingV1> {
    budget.reserve(RankedBoundsResource::Findings, 1)?;
    budget.storage(1)?;
    findings.push(finding);
    Ok(())
}

fn reachable_blocks(
    successors: &[Vec<usize>],
    budget: &mut RankedBoundsBudget,
) -> Result<Vec<bool>, RankedBoundsFindingV1> {
    let mut reachable = vec![false; successors.len()];
    let mut pending = vec![0];
    reachable[0] = true;
    while let Some(block) = pending.pop() {
        budget.work(1)?;
        for successor in &successors[block] {
            budget.work(1)?;
            if !reachable[*successor] {
                reachable[*successor] = true;
                pending.push(*successor);
            }
        }
    }
    Ok(reachable)
}

fn intersect_predecessor_facts(
    block: usize,
    predecessors: &[Vec<PredecessorEdge>],
    inputs: &[FactSet],
    fact_count: usize,
    budget: &mut RankedBoundsBudget,
) -> Result<FactSet, RankedBoundsFindingV1> {
    let mut edges = predecessors[block].iter();
    let Some(first) = edges.next() else {
        budget.work(1)?;
        return Ok(FactSet::empty(fact_count));
    };
    budget.work(inputs[first.block].words.len().saturating_add(1))?;
    let mut result = inputs[first.block].clone();
    if let Some(fact) = first.guard_fact {
        result.insert(fact);
    }
    for edge in edges {
        budget.work(result.words.len().saturating_add(1))?;
        result.intersect_edge(&inputs[edge.block], edge.guard_fact);
    }
    Ok(result)
}

struct AccessCheck<'a> {
    facts: &'a FactSet,
    fact_indices: &'a HashMap<LessThanFact, usize>,
    context: &'a Context,
    sparse_indices: &'a SparseIndexAnalysisV1,
    findings: &'a mut Vec<RankedBoundsFindingV1>,
    budget: &'a mut RankedBoundsBudget,
}

fn verify_access(
    access: &RankedAccessOp,
    block: usize,
    operation: usize,
    check: &mut AccessCheck<'_>,
) -> Result<(), RankedBoundsFindingV1> {
    let view = access.view(check.context);
    let Some(view_type) = ranked_view_type(view, check.context) else {
        return push_finding(
            check.findings,
            check.budget,
            RankedBoundsFindingV1::StructuralVerificationFailed,
        );
    };
    let view_type = view_type.deref(check.context);
    let Some(access_kind) = access.kind(check.context) else {
        return push_finding(
            check.findings,
            check.budget,
            RankedBoundsFindingV1::StructuralVerificationFailed,
        );
    };
    let view_name = view.unique_name(check.context).to_string();
    for (dimension, index) in access.indices(check.context).into_iter().enumerate() {
        check.budget.work(1)?;
        let index_expr = canonical_index_expr(index, check.context);
        let extent_expr = extent_expr(view, &view_type, dimension, check.context);
        if bound_is_proven(index_expr, extent_expr, check.facts, check.fact_indices)
            || sparse_bound_is_proven(index, extent_expr, check.sparse_indices)
        {
            continue;
        }
        match (index_expr, extent_expr) {
            (IndexExpr::Constant(index), IndexExpr::Constant(extent)) => {
                push_finding(
                    check.findings,
                    check.budget,
                    RankedBoundsFindingV1::StaticOutOfBounds {
                        block,
                        operation,
                        access: access_kind,
                        view: view_name.clone(),
                        dimension,
                        index,
                        extent,
                    },
                )?;
            }
            _ => push_finding(
                check.findings,
                check.budget,
                RankedBoundsFindingV1::UnprovedBound {
                    block,
                    operation,
                    access: access_kind,
                    view: view_name.clone(),
                    dimension,
                    index: index_expr.describe(check.context),
                    extent: extent_expr.describe(check.context),
                },
            )?,
        }
    }
    Ok(())
}

fn sparse_bound_is_proven(
    index: Value,
    extent: IndexExpr,
    sparse_indices: &SparseIndexAnalysisV1,
) -> bool {
    let Some(index_maximum) = sparse_indices
        .fact(index)
        .maximum(sparse_indices.launch_extents())
    else {
        return false;
    };
    let extent = match extent {
        IndexExpr::Constant(extent) => Some(extent),
        IndexExpr::Value(value) => sparse_indices.fact(value).constant_value(),
        IndexExpr::Dimension { .. } => None,
    };
    extent.is_some_and(|extent| index_maximum < extent)
}

fn sparse_index_failure(failure: SparseIndexFailureV1) -> RankedBoundsFindingV1 {
    let detail = match failure {
        SparseIndexFailureV1::ResourceLimit {
            resource,
            limit,
            actual,
        } => format!("{resource} count {actual} exceeds {limit}"),
        SparseIndexFailureV1::InconsistentLaunchExtent {
            dimension,
            first,
            second,
        } => format!(
            "invocation dimension {dimension} has inconsistent launch extents {first} and {second}"
        ),
        SparseIndexFailureV1::MalformedControlFlow { detail } => detail.to_owned(),
    };
    RankedBoundsFindingV1::SparseIndexAnalysisFailed { detail }
}

fn bound_is_proven(
    index: IndexExpr,
    extent: IndexExpr,
    facts: &FactSet,
    fact_indices: &HashMap<LessThanFact, usize>,
) -> bool {
    match (index, extent) {
        (IndexExpr::Constant(index), IndexExpr::Constant(extent)) => index < extent,
        _ => fact_indices
            .get(&LessThanFact {
                lhs: index,
                rhs: extent,
            })
            .is_some_and(|fact| facts.contains(*fact)),
    }
}

fn extent_expr(
    view: Value,
    view_type: &RankedViewType,
    dimension: usize,
    context: &Context,
) -> IndexExpr {
    let extent = view_type.shape()[dimension];
    if extent == dialect_kernel::DYNAMIC_EXTENT {
        if let Some(definition) = view.defining_op() {
            let definition = Operation::get_op_dyn(definition, context);
            if let Some(view_op) = definition.downcast_ref::<RankedViewOp>()
                && let Some(runtime_extent) = view_op.dynamic_extent(context, dimension)
            {
                return canonical_runtime_extent(runtime_extent, context);
            }
        }
        IndexExpr::Dimension { view, dimension }
    } else {
        IndexExpr::Constant(extent)
    }
}

fn canonical_runtime_extent(value: Value, context: &Context) -> IndexExpr {
    if let Some(operation) = value.defining_op() {
        let operation = Operation::get_op_dyn(operation, context);
        if let Some(constant) = operation.downcast_ref::<IndexConstantOp>()
            && let Some(value) = constant.value(context)
        {
            return IndexExpr::Constant(value);
        }
    }
    IndexExpr::Value(value)
}

fn canonical_index_expr(value: Value, context: &Context) -> IndexExpr {
    let Some(operation) = value.defining_op() else {
        return IndexExpr::Value(value);
    };
    let operation = Operation::get_op_dyn(operation, context);
    if let Some(constant) = operation.downcast_ref::<IndexConstantOp>()
        && let Some(value) = constant.value(context)
    {
        return IndexExpr::Constant(value);
    }
    if let Some(dimension) = operation.downcast_ref::<DimensionOp>()
        && let Some(dimension_index) = dimension.dimension(context)
        && let Ok(dimension_index) = usize::try_from(dimension_index)
    {
        let view = dimension.view(context);
        if let Some(view_type) = ranked_view_type(view, context) {
            let view_type: TypedHandle<RankedViewType> = view_type;
            let view_type = view_type.deref(context);
            if view_type.shape().get(dimension_index).is_some() {
                return extent_expr(view, &view_type, dimension_index, context);
            }
        }
    }
    IndexExpr::Value(value)
}
