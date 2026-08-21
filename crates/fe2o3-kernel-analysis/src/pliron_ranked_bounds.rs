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

use dialect_kernel::{
    AccessKindAttr, BranchOp, DimensionOp, IndexConstantOp, IndexLessThanBranchOp, RankedAccessOp,
    RankedViewOp, RankedViewType, ReturnOp, ranked_view_type,
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

use crate::KernelCheckPassKindV1;

pub const MAX_RANKED_BOUNDS_BLOCKS: usize = 1_024;
pub const MAX_RANKED_BOUNDS_OPERATIONS: usize = 65_536;

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
    let region = function.get_region(context);
    let blocks = region.deref(context).iter(context).collect::<Vec<_>>();
    if blocks.len() > MAX_RANKED_BOUNDS_BLOCKS {
        return resource_failure("basic block", MAX_RANKED_BOUNDS_BLOCKS, blocks.len());
    }
    let Some(operation_count) = blocks.iter().try_fold(0_usize, |total, block| {
        total.checked_add(block.deref(context).iter(context).count())
    }) else {
        return resource_failure("operation", MAX_RANKED_BOUNDS_OPERATIONS, usize::MAX);
    };
    if operation_count > MAX_RANKED_BOUNDS_OPERATIONS {
        return resource_failure("operation", MAX_RANKED_BOUNDS_OPERATIONS, operation_count);
    }
    if verify_operation(function.get_operation(), context).is_err() {
        return RankedBoundsReportV1 {
            findings: vec![RankedBoundsFindingV1::StructuralVerificationFailed],
        };
    }
    if blocks.is_empty() {
        return RankedBoundsReportV1 {
            findings: vec![RankedBoundsFindingV1::StructuralVerificationFailed],
        };
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
            return RankedBoundsReportV1 {
                findings: vec![RankedBoundsFindingV1::StructuralVerificationFailed],
            };
        };
        let operation = Operation::get_op_dyn(terminator, context);
        let guard_fact = if let Some(branch) = operation.downcast_ref::<IndexLessThanBranchOp>() {
            let fact = LessThanFact {
                lhs: canonical_index_expr(branch.lhs(context), context),
                rhs: canonical_index_expr(branch.rhs(context), context),
            };
            let next = fact_indices.len();
            Some(*fact_indices.entry(fact).or_insert(next))
        } else {
            None
        };

        let raw = terminator.deref(context);
        for (successor_index, successor) in raw.successors().enumerate() {
            let Some(target) = indices.get(&successor).copied() else {
                findings.push(RankedBoundsFindingV1::UnsupportedTerminator {
                    block: block_index,
                    operation: "successor outside function region".to_owned(),
                });
                continue;
            };
            successors[block_index].push(target);
            predecessors[target].push(PredecessorEdge {
                block: block_index,
                guard_fact: (successor_index == 0).then_some(guard_fact).flatten(),
            });
        }

        if operation.downcast_ref::<IndexLessThanBranchOp>().is_none()
            && operation.downcast_ref::<BranchOp>().is_none()
            && operation.downcast_ref::<ReturnOp>().is_none()
        {
            findings.push(RankedBoundsFindingV1::UnsupportedTerminator {
                block: block_index,
                operation: operation.get_opid().to_string(),
            });
        }
    }

    let reachable = reachable_blocks(&successors);
    for (block, is_reachable) in reachable.iter().copied().enumerate() {
        if !is_reachable {
            findings.push(RankedBoundsFindingV1::UnreachableBlock { block });
        }
    }

    let fact_count = fact_indices.len();
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
            intersect_predecessor_facts(block, &predecessors, &inputs, fact_count)
        };
        if next != inputs[block] {
            inputs[block] = next;
            for successor in &successors[block] {
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
            let operation = Operation::get_op_dyn(operation, context);
            let Some(access) = operation.downcast_ref::<RankedAccessOp>() else {
                continue;
            };
            verify_access(
                access,
                block_index,
                operation_index,
                &inputs[block_index],
                &fact_indices,
                context,
                &mut findings,
            );
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

fn resource_failure(resource: &'static str, limit: usize, actual: usize) -> RankedBoundsReportV1 {
    RankedBoundsReportV1 {
        findings: vec![RankedBoundsFindingV1::ResourceLimitExceeded {
            resource,
            limit,
            actual,
        }],
    }
}

fn reachable_blocks(successors: &[Vec<usize>]) -> Vec<bool> {
    let mut reachable = vec![false; successors.len()];
    let mut pending = vec![0];
    while let Some(block) = pending.pop() {
        if !reachable[block] {
            reachable[block] = true;
            pending.extend(successors[block].iter().copied());
        }
    }
    reachable
}

fn intersect_predecessor_facts(
    block: usize,
    predecessors: &[Vec<PredecessorEdge>],
    inputs: &[FactSet],
    fact_count: usize,
) -> FactSet {
    let mut edges = predecessors[block].iter();
    let Some(first) = edges.next() else {
        return FactSet::empty(fact_count);
    };
    let mut result = inputs[first.block].clone();
    if let Some(fact) = first.guard_fact {
        result.insert(fact);
    }
    for edge in edges {
        result.intersect_edge(&inputs[edge.block], edge.guard_fact);
    }
    result
}

fn verify_access(
    access: &RankedAccessOp,
    block: usize,
    operation: usize,
    facts: &FactSet,
    fact_indices: &HashMap<LessThanFact, usize>,
    context: &Context,
    findings: &mut Vec<RankedBoundsFindingV1>,
) {
    let view = access.view(context);
    let Some(view_type) = ranked_view_type(view, context) else {
        findings.push(RankedBoundsFindingV1::StructuralVerificationFailed);
        return;
    };
    let view_type = view_type.deref(context);
    let Some(access_kind) = access.kind(context) else {
        findings.push(RankedBoundsFindingV1::StructuralVerificationFailed);
        return;
    };
    let view_name = view.unique_name(context).to_string();
    for (dimension, index) in access.indices(context).into_iter().enumerate() {
        let index_expr = canonical_index_expr(index, context);
        let extent_expr = extent_expr(view, &view_type, dimension, context);
        if bound_is_proven(index_expr, extent_expr, facts, fact_indices) {
            continue;
        }
        match (index_expr, extent_expr) {
            (IndexExpr::Constant(index), IndexExpr::Constant(extent)) => {
                findings.push(RankedBoundsFindingV1::StaticOutOfBounds {
                    block,
                    operation,
                    access: access_kind,
                    view: view_name.clone(),
                    dimension,
                    index,
                    extent,
                });
            }
            _ => findings.push(RankedBoundsFindingV1::UnprovedBound {
                block,
                operation,
                access: access_kind,
                view: view_name.clone(),
                dimension,
                index: index_expr.describe(context),
                extent: extent_expr.describe(context),
            }),
        }
    }
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
