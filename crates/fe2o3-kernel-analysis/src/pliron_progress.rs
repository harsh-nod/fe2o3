//! Workload-neutral termination and progress checks over ranked PLIRON CFGs.
//!
//! The analysis proves only a closed canonical induction form. Other cyclic
//! control flow is rejected when nontermination is structural, or reported as
//! incomplete when a ranking function would require a stronger solver.

use std::{collections::HashMap, fmt};

use dialect_kernel::{
    AnalysisSplitOp, BranchArgsOp, BranchOp, IndexBinaryKindAttr, IndexBinaryOp, IndexConstantOp,
    IndexEqualBranchArgsOp, IndexLessThanBranchArgsOp,
};
use pliron::{
    basic_block::BasicBlock,
    builtin::{op_interfaces::OneRegionInterface, ops::FuncOp},
    common_traits::Named,
    context::{Context, Ptr},
    linked_list::ContainsLinkedList,
    op::Op,
    operation::Operation,
};

use crate::KernelCheckStatusV1;

pub const MAX_PLIRON_PROGRESS_BLOCKS_V1: usize = 4_096;
pub const MAX_PLIRON_PROGRESS_EDGES_V1: usize = 16_384;
/// Maximum number of boolean lattice cells examined or initialized while
/// solving dominators. This independently bounds adversarial fixed-point
/// convergence within the structural block and edge limits while allowing
/// several complete maximum-size lattice sweeps.
pub const MAX_PLIRON_DOMINATOR_LATTICE_WORK_V1: usize = 268_435_456;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlironProgressCertificateV1 {
    header: usize,
    body: usize,
    exit: usize,
    induction: String,
    bound: String,
    step: u64,
}

impl PlironProgressCertificateV1 {
    pub const fn header(&self) -> usize {
        self.header
    }
    pub const fn body(&self) -> usize {
        self.body
    }
    pub const fn exit(&self) -> usize {
        self.exit
    }
    pub fn induction(&self) -> &str {
        &self.induction
    }
    pub fn bound(&self) -> &str {
        &self.bound
    }
    pub const fn step(&self) -> u64 {
        self.step
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlironProgressFindingV1 {
    ResourceLimitExceeded {
        resource: &'static str,
        actual: usize,
        limit: usize,
    },
    NonTerminatingCycle {
        blocks: Vec<usize>,
        reason: &'static str,
        counterexample: String,
    },
    ProgressIncomplete {
        blocks: Vec<usize>,
        reason: &'static str,
    },
}

impl PlironProgressFindingV1 {
    pub const fn status(&self) -> KernelCheckStatusV1 {
        match self {
            Self::NonTerminatingCycle { .. } => KernelCheckStatusV1::Rejected,
            Self::ResourceLimitExceeded { .. } | Self::ProgressIncomplete { .. } => {
                KernelCheckStatusV1::Incomplete
            }
        }
    }
}

impl fmt::Display for PlironProgressFindingV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ResourceLimitExceeded {
                resource,
                actual,
                limit,
            } => write!(
                formatter,
                "error[FE2O3-PROGRESS-003]: progress analysis has {actual} {resource}, exceeding limit {limit}; help: split the kernel or reduce its control-flow graph"
            ),
            Self::NonTerminatingCycle {
                blocks,
                reason,
                counterexample,
            } => write!(
                formatter,
                "error[FE2O3-PROGRESS-001]: control-flow cycle {blocks:?} does not terminate: {reason}; counterexample: {counterexample}; help: add an exit controlled by a finite induction variable and advance it on every backedge"
            ),
            Self::ProgressIncomplete { blocks, reason } => write!(
                formatter,
                "error[FE2O3-PROGRESS-002]: termination proof for control-flow cycle {blocks:?} is incomplete: {reason}; help: express the loop as `i < bound` with a positive constant backedge step and a statically proved no-wrap update, or provide a future supported ranking-function contract"
            ),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlironProgressReportV1 {
    findings: Vec<PlironProgressFindingV1>,
    certificates: Vec<PlironProgressCertificateV1>,
}

impl PlironProgressReportV1 {
    pub fn status(&self) -> KernelCheckStatusV1 {
        self.findings
            .iter()
            .fold(KernelCheckStatusV1::Clean, |status, finding| {
                status.join(finding.status())
            })
    }
    pub fn is_clean(&self) -> bool {
        self.status() == KernelCheckStatusV1::Clean
    }
    pub fn findings(&self) -> &[PlironProgressFindingV1] {
        &self.findings
    }
    pub fn certificates(&self) -> &[PlironProgressCertificateV1] {
        &self.certificates
    }
    pub const fn grants_launch_or_liveness_authority(&self) -> bool {
        false
    }
    pub(crate) fn clean() -> Self {
        Self {
            findings: Vec::new(),
            certificates: Vec::new(),
        }
    }
}

pub fn run_pliron_progress_check_v1(
    context: &Context,
    function: &FuncOp,
) -> PlironProgressReportV1 {
    let blocks = function
        .get_region(context)
        .deref(context)
        .iter(context)
        .collect::<Vec<_>>();
    if blocks.len() > MAX_PLIRON_PROGRESS_BLOCKS_V1 {
        return report(PlironProgressFindingV1::ResourceLimitExceeded {
            resource: "basic blocks",
            actual: blocks.len(),
            limit: MAX_PLIRON_PROGRESS_BLOCKS_V1,
        });
    }
    let block_indices = blocks
        .iter()
        .enumerate()
        .map(|(index, block)| (*block, index))
        .collect::<HashMap<_, _>>();
    let mut operation_blocks = HashMap::new();
    let mut edges = vec![Vec::new(); blocks.len()];
    let mut edge_count = 0_usize;
    for (block_index, block) in blocks.iter().copied().enumerate() {
        for operation in block.deref(context).iter(context) {
            operation_blocks.insert(operation, block_index);
        }
        let Some(terminator) = block.deref(context).get_terminator(context) else {
            return report(PlironProgressFindingV1::ProgressIncomplete {
                blocks: vec![block_index],
                reason: "the block has no terminator",
            });
        };
        for successor in terminator.deref(context).successors() {
            let Some(successor) = block_indices.get(&successor).copied() else {
                return report(PlironProgressFindingV1::ProgressIncomplete {
                    blocks: vec![block_index],
                    reason: "a successor is outside the kernel function",
                });
            };
            edge_count += 1;
            if edge_count > MAX_PLIRON_PROGRESS_EDGES_V1 {
                return report(PlironProgressFindingV1::ResourceLimitExceeded {
                    resource: "CFG edges",
                    actual: edge_count,
                    limit: MAX_PLIRON_PROGRESS_EDGES_V1,
                });
            }
            edges[block_index].push(successor);
        }
    }

    let reachable = reachable_blocks(&edges);
    let definitely_reachable = definitely_reachable_blocks(context, &blocks, &block_indices);
    let predecessors = predecessor_graph(&edges);
    let dominators = match dominators(&predecessors, &reachable) {
        Ok(dominators) => dominators,
        Err(actual) => {
            return report(PlironProgressFindingV1::ResourceLimitExceeded {
                resource: "dominator lattice work units",
                actual,
                limit: MAX_PLIRON_DOMINATOR_LATTICE_WORK_V1,
            });
        }
    };
    let mut findings = Vec::new();
    let mut certificates = Vec::new();
    for mut component in strongly_connected_components(&edges) {
        component.sort_unstable();
        if !component.iter().any(|block| reachable[*block]) || !is_cycle(&component, &edges) {
            continue;
        }
        let has_exit = component.iter().any(|block| {
            edges[*block]
                .iter()
                .any(|successor| !component.contains(successor))
        });
        if !has_exit {
            if component.iter().any(|block| definitely_reachable[*block]) {
                findings.push(PlironProgressFindingV1::NonTerminatingCycle {
                    blocks: component,
                    reason: "the strongly connected component has no exit edge",
                    counterexample: "an unconditional path from entry reaches the cycle, and every successor remains in it".to_owned(),
                });
            } else {
                findings.push(PlironProgressFindingV1::ProgressIncomplete {
                    blocks: component,
                    reason: "the exit-free cycle is only conditionally reachable and no feasible incoming witness was reconstructed",
                });
            }
            continue;
        }
        match canonical_positive_induction_loop(
            context,
            &blocks,
            &block_indices,
            &operation_blocks,
            &edges,
            &predecessors,
            &dominators,
            &component,
        ) {
            CanonicalLoopResultV1::Proved(mut component_certificates) => {
                certificates.append(&mut component_certificates);
            }
            CanonicalLoopResultV1::Inactive => {}
            CanonicalLoopResultV1::Rejected {
                reason,
                counterexample,
            } => {
                findings.push(PlironProgressFindingV1::NonTerminatingCycle {
                    blocks: component,
                    reason,
                    counterexample,
                });
            }
            CanonicalLoopResultV1::Incomplete(reason) => {
                findings.push(PlironProgressFindingV1::ProgressIncomplete {
                    blocks: component,
                    reason,
                });
            }
        }
    }
    PlironProgressReportV1 {
        findings,
        certificates,
    }
}

enum CanonicalLoopResultV1 {
    Proved(Vec<PlironProgressCertificateV1>),
    Inactive,
    Rejected {
        reason: &'static str,
        counterexample: String,
    },
    Incomplete(&'static str),
}

fn canonical_positive_induction_loop(
    context: &Context,
    blocks: &[Ptr<BasicBlock>],
    block_indices: &HashMap<Ptr<BasicBlock>, usize>,
    operation_blocks: &HashMap<Ptr<Operation>, usize>,
    edges: &[Vec<usize>],
    predecessors: &[Vec<usize>],
    dominators: &[Vec<bool>],
    component: &[usize],
) -> CanonicalLoopResultV1 {
    let mut component_members = vec![false; blocks.len()];
    for block in component {
        component_members[*block] = true;
    }
    let mut backedges_by_header = HashMap::<usize, Vec<usize>>::new();
    for source in component {
        for target in &edges[*source] {
            if component_members[*target]
                && dominators
                    .get(*source)
                    .and_then(|row| row.get(*target))
                    .copied()
                    .unwrap_or(false)
            {
                backedges_by_header
                    .entry(*target)
                    .or_default()
                    .push(*source);
            }
        }
    }
    if backedges_by_header.is_empty() {
        return CanonicalLoopResultV1::Incomplete(
            "the loop body has a predecessor other than its guarded header, so the cycle has no dominance-backed induction backedge",
        );
    }

    let mut headers = backedges_by_header.keys().copied().collect::<Vec<_>>();
    headers.sort_unstable();
    let mut certificates = Vec::with_capacity(headers.len());
    let mut certified_backedges = Vec::with_capacity(headers.len());
    for header_index in headers {
        let latches = &backedges_by_header[&header_index];
        if latches.len() != 1 {
            return CanonicalLoopResultV1::Incomplete(
                "a natural induction loop has more than one dominated backedge",
            );
        }
        let latch_index = latches[0];
        let natural_loop =
            natural_loop_members(header_index, latch_index, predecessors, &component_members);
        match certify_natural_induction_loop(
            context,
            blocks,
            block_indices,
            operation_blocks,
            edges,
            predecessors,
            &natural_loop,
            header_index,
            latch_index,
        ) {
            CanonicalLoopResultV1::Proved(mut loop_certificates) => {
                certificates.append(&mut loop_certificates);
            }
            CanonicalLoopResultV1::Inactive => {}
            other => return other,
        }
        certified_backedges.push((latch_index, header_index));
    }
    if has_residual_cycle(component, edges, &certified_backedges) {
        return CanonicalLoopResultV1::Incomplete(
            "the cycle retains a residual backedge after certified induction recurrences are removed",
        );
    }
    if certificates.is_empty() {
        CanonicalLoopResultV1::Inactive
    } else {
        CanonicalLoopResultV1::Proved(certificates)
    }
}

#[allow(clippy::too_many_arguments)]
fn certify_natural_induction_loop(
    context: &Context,
    blocks: &[Ptr<BasicBlock>],
    block_indices: &HashMap<Ptr<BasicBlock>, usize>,
    operation_blocks: &HashMap<Ptr<Operation>, usize>,
    edges: &[Vec<usize>],
    predecessors: &[Vec<usize>],
    natural_loop: &[bool],
    header_index: usize,
    latch_index: usize,
) -> CanonicalLoopResultV1 {
    let header = blocks[header_index];
    let header_block = header.deref(context);
    let Some(terminator) = header_block.get_terminator(context) else {
        return CanonicalLoopResultV1::Incomplete("the natural loop header has no terminator");
    };
    let terminator = Operation::get_op_dyn(terminator, context);
    let Some(branch) = terminator.downcast_ref::<IndexLessThanBranchArgsOp>() else {
        return CanonicalLoopResultV1::Incomplete(
            "a dominated backedge does not target an `i < bound` induction header",
        );
    };
    let Some(induction_slot) = (0..header_block.get_num_arguments())
        .find(|slot| header_block.get_argument(*slot) == branch.lhs(context))
    else {
        return CanonicalLoopResultV1::Incomplete(
            "the induction comparison does not use one header block argument",
        );
    };
    let tuple_width = header_block.get_num_arguments();
    let raw = branch.get_operation().deref(context);
    let (Some(body), Some(exit)) = (raw.successors().next(), raw.successors().nth(1)) else {
        return CanonicalLoopResultV1::Incomplete(
            "the induction header does not have exact body and exit successors",
        );
    };
    let (Some(body_index), Some(exit_index)) = (
        block_indices.get(&body).copied(),
        block_indices.get(&exit).copied(),
    ) else {
        return CanonicalLoopResultV1::Incomplete(
            "an induction header successor is outside the kernel function",
        );
    };
    if !natural_loop.get(body_index).copied().unwrap_or(false)
        || natural_loop.get(exit_index).copied().unwrap_or(false)
    {
        return CanonicalLoopResultV1::Incomplete(
            "the induction true edge does not enter the natural loop or its false edge does not exit it",
        );
    }
    let header_arguments = (0..tuple_width)
        .map(|slot| header_block.get_argument(slot))
        .collect::<Vec<_>>();
    if branch.true_arguments(context) != header_arguments {
        return CanonicalLoopResultV1::Incomplete(
            "the induction header does not forward its exact live tuple to the body",
        );
    }
    let bound = branch.rhs(context);
    let bound_operation_is_internal = bound
        .defining_op()
        .and_then(|definition| operation_blocks.get(&definition))
        .is_some_and(|block| natural_loop.get(*block).copied().unwrap_or(false));
    let bound_block_is_internal = bound
        .defining_block()
        .and_then(|block| block_indices.get(&block))
        .is_some_and(|block| natural_loop.get(*block).copied().unwrap_or(false));
    if bound_operation_is_internal || bound_block_is_internal {
        return CanonicalLoopResultV1::Incomplete(
            "the loop bound depends on a value defined inside the cycle",
        );
    }

    let mut preheaders = Vec::new();
    for (block, inside) in natural_loop.iter().copied().enumerate() {
        if !inside {
            continue;
        }
        for predecessor in &predecessors[block] {
            if natural_loop.get(*predecessor).copied().unwrap_or(false) {
                continue;
            }
            if block != header_index {
                return CanonicalLoopResultV1::Incomplete(
                    "a non-header edge enters the natural induction loop",
                );
            }
            preheaders.push(*predecessor);
        }
    }
    preheaders.sort_unstable();
    preheaders.dedup();
    if preheaders.len() != 1 {
        return CanonicalLoopResultV1::Incomplete(
            "the natural induction loop does not have exactly one preheader",
        );
    }

    let latch = blocks[latch_index];
    let latch_block = latch.deref(context);
    if latch_block.get_num_arguments() < tuple_width {
        return CanonicalLoopResultV1::Incomplete(
            "the induction latch lost part of the live loop tuple",
        );
    }
    let Some(backedge) = latch_block.get_terminator(context) else {
        return CanonicalLoopResultV1::Incomplete("the induction latch has no backedge");
    };
    let backedge = Operation::get_op_dyn(backedge, context);
    let Some(backedge) = backedge.downcast_ref::<BranchArgsOp>() else {
        return CanonicalLoopResultV1::Incomplete(
            "the induction recurrence is not an exact argument-carrying backedge",
        );
    };
    if backedge.get_operation().deref(context).successors().next() != Some(header) {
        return CanonicalLoopResultV1::Incomplete(
            "the induction recurrence does not return to its header",
        );
    }
    let arguments = backedge.arguments(context);
    if arguments.len() != tuple_width {
        return CanonicalLoopResultV1::Incomplete(
            "the induction backedge changed the live tuple width",
        );
    }
    for (slot, argument) in arguments.iter().copied().enumerate() {
        if slot != induction_slot
            && !is_local_slot_evolution(
                context,
                operation_blocks,
                latch_index,
                latch_block.get_argument(slot),
                argument,
            )
        {
            return CanonicalLoopResultV1::Incomplete(
                "the induction backedge mutated a non-induction live tuple slot",
            );
        }
    }
    let next = arguments[induction_slot];
    if next == latch_block.get_argument(induction_slot) {
        return zero_step_result(
            context,
            blocks,
            natural_loop,
            header,
            induction_slot,
            branch.rhs(context),
            "the induction variable is unchanged on the backedge",
        );
    }
    let Some(increment_definition) = next.defining_op() else {
        return CanonicalLoopResultV1::Incomplete(
            "the backedge value is not a locally reconstructed induction update",
        );
    };
    if operation_blocks.get(&increment_definition).copied() != Some(latch_index) {
        return CanonicalLoopResultV1::Incomplete(
            "the induction update is not defined in its unique latch",
        );
    }
    let increment = Operation::get_op_dyn(increment_definition, context);
    let Some(increment) = increment.downcast_ref::<IndexBinaryOp>() else {
        return CanonicalLoopResultV1::Incomplete(
            "the induction update is not target-neutral index addition",
        );
    };
    if increment.kind(context) != Some(IndexBinaryKindAttr::Add)
        || increment.lhs(context) != latch_block.get_argument(induction_slot)
    {
        return CanonicalLoopResultV1::Incomplete("the induction update is not `i + constant`");
    }
    let Some(step_definition) = increment.rhs(context).defining_op() else {
        return CanonicalLoopResultV1::Incomplete("the induction step is not constant");
    };
    let step = Operation::get_op_dyn(step_definition, context);
    let Some(step) = step.downcast_ref::<IndexConstantOp>() else {
        return CanonicalLoopResultV1::Incomplete("the induction step is not constant");
    };
    let step = match step.value(context) {
        Some(0) => {
            return zero_step_result(
                context,
                blocks,
                natural_loop,
                header,
                induction_slot,
                branch.rhs(context),
                "the induction step is zero",
            );
        }
        Some(step) => step,
        None => return CanonicalLoopResultV1::Incomplete("the induction step is malformed"),
    };
    if step > 1 {
        let Some(bound) = index_constant(context, branch.rhs(context)) else {
            return CanonicalLoopResultV1::Incomplete(
                "a symbolic bound with a non-unit step needs a no-wrap range proof",
            );
        };
        if bound != 0 && (bound - 1).checked_add(step).is_none() {
            return CanonicalLoopResultV1::Incomplete(
                "the largest guarded induction value plus the step can overflow u64",
            );
        }
    }

    for (source, inside) in natural_loop.iter().copied().enumerate() {
        if !inside {
            continue;
        }
        let source_block = blocks[source].deref(context);
        if source_block.get_num_arguments() < tuple_width {
            return CanonicalLoopResultV1::Incomplete(
                "a natural-loop block lost part of the live induction tuple",
            );
        }
        let Some(terminator) = source_block.get_terminator(context) else {
            return CanonicalLoopResultV1::Incomplete("a natural-loop block has no terminator");
        };
        for (successor_ordinal, target) in edges[source].iter().copied().enumerate() {
            if !natural_loop.get(target).copied().unwrap_or(false) {
                continue;
            }
            if source == latch_index && target == header_index {
                continue;
            }
            let Some(edge_arguments) = successor_arguments(context, terminator, successor_ordinal)
            else {
                return CanonicalLoopResultV1::Incomplete(
                    "an internal natural-loop edge does not carry its exact live tuple",
                );
            };
            if edge_arguments.len() < tuple_width
                || blocks[target].deref(context).get_num_arguments() < tuple_width
            {
                return CanonicalLoopResultV1::Incomplete(
                    "an internal natural-loop edge changed the live tuple width",
                );
            }
            for (slot, argument) in edge_arguments.iter().copied().take(tuple_width).enumerate() {
                let source_argument = source_block.get_argument(slot);
                if slot == induction_slot && argument != source_argument {
                    return CanonicalLoopResultV1::Incomplete(
                        "a non-latch loop edge mutated the tracked induction value",
                    );
                }
                if slot != induction_slot
                    && !is_local_slot_evolution(
                        context,
                        operation_blocks,
                        source,
                        source_argument,
                        argument,
                    )
                {
                    return CanonicalLoopResultV1::Incomplete(
                        "an internal natural-loop edge substituted a non-induction live tuple slot without a local recurrence",
                    );
                }
            }
        }
    }

    CanonicalLoopResultV1::Proved(vec![PlironProgressCertificateV1 {
        header: header_index,
        body: body_index,
        exit: exit_index,
        induction: branch.lhs(context).unique_name(context).to_string(),
        bound: branch.rhs(context).unique_name(context).to_string(),
        step,
    }])
}

fn is_local_slot_evolution(
    context: &Context,
    operation_blocks: &HashMap<Ptr<Operation>, usize>,
    source_block: usize,
    source_argument: pliron::value::Value,
    edge_argument: pliron::value::Value,
) -> bool {
    if edge_argument == source_argument {
        return true;
    }
    let Some(definition) = edge_argument.defining_op() else {
        return false;
    };
    operation_blocks.get(&definition).copied() == Some(source_block)
        && definition
            .deref(context)
            .operands()
            .any(|operand| operand == source_argument)
}

fn successor_arguments(
    context: &Context,
    terminator: Ptr<Operation>,
    successor_ordinal: usize,
) -> Option<Vec<pliron::value::Value>> {
    let operation = Operation::get_op_dyn(terminator, context);
    if let Some(branch) = operation.downcast_ref::<BranchArgsOp>() {
        return (successor_ordinal == 0).then(|| branch.arguments(context));
    }
    if let Some(branch) = operation.downcast_ref::<IndexLessThanBranchArgsOp>() {
        return match successor_ordinal {
            0 => Some(branch.true_arguments(context)),
            1 => Some(branch.false_arguments(context)),
            _ => None,
        };
    }
    if let Some(branch) = operation.downcast_ref::<IndexEqualBranchArgsOp>() {
        return match successor_ordinal {
            0 => Some(branch.true_arguments(context)),
            1 => Some(branch.false_arguments(context)),
            _ => None,
        };
    }
    if let Some(branch) = operation.downcast_ref::<AnalysisSplitOp>() {
        return match successor_ordinal {
            0 => Some(branch.first_arguments(context)),
            1 => Some(branch.second_arguments(context)),
            _ => None,
        };
    }
    None
}

fn natural_loop_members(
    header: usize,
    latch: usize,
    predecessors: &[Vec<usize>],
    component: &[bool],
) -> Vec<bool> {
    let mut members = vec![false; predecessors.len()];
    members[header] = true;
    members[latch] = true;
    let mut pending = vec![latch];
    while let Some(block) = pending.pop() {
        if block == header {
            continue;
        }
        for predecessor in &predecessors[block] {
            if component.get(*predecessor).copied().unwrap_or(false) && !members[*predecessor] {
                members[*predecessor] = true;
                pending.push(*predecessor);
            }
        }
    }
    members
}

fn has_residual_cycle(
    component: &[usize],
    edges: &[Vec<usize>],
    removed_edges: &[(usize, usize)],
) -> bool {
    let mut members = vec![false; edges.len()];
    for block in component {
        members[*block] = true;
    }
    let mut indegree = vec![0_usize; edges.len()];
    for source in component {
        for target in &edges[*source] {
            if members[*target] && !removed_edges.contains(&(*source, *target)) {
                indegree[*target] += 1;
            }
        }
    }
    let mut pending = component
        .iter()
        .copied()
        .filter(|block| indegree[*block] == 0)
        .collect::<Vec<_>>();
    let mut removed = 0_usize;
    while let Some(source) = pending.pop() {
        removed += 1;
        for target in &edges[source] {
            if !members[*target] || removed_edges.contains(&(source, *target)) {
                continue;
            }
            indegree[*target] -= 1;
            if indegree[*target] == 0 {
                pending.push(*target);
            }
        }
    }
    removed != component.len()
}

fn zero_step_result(
    context: &Context,
    blocks: &[Ptr<BasicBlock>],
    natural_loop: &[bool],
    header: Ptr<BasicBlock>,
    induction_slot: usize,
    bound: pliron::value::Value,
    reason: &'static str,
) -> CanonicalLoopResultV1 {
    let Some(bound) = index_constant(context, bound) else {
        return CanonicalLoopResultV1::Incomplete(
            "the zero-step cycle needs a feasible true-edge witness, but its bound is symbolic",
        );
    };
    let mut saw_predecessor = false;
    let mut saw_unknown = false;
    for (predecessor_index, predecessor) in blocks.iter().copied().enumerate() {
        if natural_loop
            .get(predecessor_index)
            .copied()
            .unwrap_or(false)
        {
            continue;
        }
        let Some(terminator) = predecessor.deref(context).get_terminator(context) else {
            continue;
        };
        if !terminator
            .deref(context)
            .successors()
            .any(|successor| successor == header)
        {
            continue;
        }
        saw_predecessor = true;
        let operation = Operation::get_op_dyn(terminator, context);
        let Some(branch) = operation.downcast_ref::<BranchArgsOp>() else {
            saw_unknown = true;
            continue;
        };
        let arguments = branch.arguments(context);
        let Some(initial) = arguments
            .get(induction_slot)
            .and_then(|value| index_constant(context, *value))
        else {
            saw_unknown = true;
            continue;
        };
        if initial < bound {
            return CanonicalLoopResultV1::Rejected {
                reason,
                counterexample: format!(
                    "the live incoming edge carries i = {initial} and bound = {bound}, so the true edge repeats forever"
                ),
            };
        }
    }
    if saw_predecessor && !saw_unknown {
        CanonicalLoopResultV1::Inactive
    } else {
        CanonicalLoopResultV1::Incomplete(
            "the zero-step cycle has no reconstructed feasible incoming value",
        )
    }
}

fn predecessor_graph(edges: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let mut predecessors = vec![Vec::new(); edges.len()];
    for (source, successors) in edges.iter().enumerate() {
        for target in successors {
            predecessors[*target].push(source);
        }
    }
    predecessors
}

fn dominators(predecessors: &[Vec<usize>], reachable: &[bool]) -> Result<Vec<Vec<bool>>, usize> {
    dominators_with_work_limit(
        predecessors,
        reachable,
        MAX_PLIRON_DOMINATOR_LATTICE_WORK_V1,
    )
}

fn dominators_with_work_limit(
    predecessors: &[Vec<usize>],
    reachable: &[bool],
    work_limit: usize,
) -> Result<Vec<Vec<bool>>, usize> {
    let mut work = 0_usize;
    let mut charge = |amount: usize| {
        work = work.checked_add(amount).unwrap_or(usize::MAX);
        if work > work_limit { Err(work) } else { Ok(()) }
    };
    let mut result = vec![vec![false; predecessors.len()]; predecessors.len()];
    if predecessors.is_empty() {
        return Ok(result);
    }
    result[0][0] = true;
    for block in 1..predecessors.len() {
        if reachable.get(block).copied().unwrap_or(false) {
            charge(reachable.len())?;
            result[block].clone_from_slice(reachable);
        }
    }
    loop {
        let mut changed = false;
        for block in 1..predecessors.len() {
            if !reachable.get(block).copied().unwrap_or(false) {
                continue;
            }
            charge(reachable.len())?;
            let mut updated = reachable.to_vec();
            let mut saw_predecessor = false;
            for predecessor in &predecessors[block] {
                if !reachable.get(*predecessor).copied().unwrap_or(false) {
                    continue;
                }
                if !saw_predecessor {
                    charge(reachable.len())?;
                    updated.clone_from_slice(&result[*predecessor]);
                    saw_predecessor = true;
                } else {
                    charge(reachable.len())?;
                    for (value, predecessor_value) in updated.iter_mut().zip(&result[*predecessor])
                    {
                        *value &= *predecessor_value;
                    }
                }
            }
            if !saw_predecessor {
                updated.fill(false);
            }
            updated[block] = true;
            charge(reachable.len())?;
            if updated != result[block] {
                result[block] = updated;
                changed = true;
            }
        }
        if !changed {
            return Ok(result);
        }
    }
}

#[cfg(test)]
mod dominator_work_tests {
    use super::dominators_with_work_limit;

    #[test]
    fn reverse_ordered_chain_exhausts_the_explicit_work_budget() {
        // Entry reaches the highest-numbered block and the chain then descends.
        // The ascending solver order can refine only one additional row per pass.
        let mut predecessors = vec![Vec::new(); 8];
        predecessors[7].push(0);
        for block in 1..7 {
            predecessors[block].push(block + 1);
        }
        let reachable = vec![true; predecessors.len()];

        let actual = dominators_with_work_limit(&predecessors, &reachable, 200).unwrap_err();
        assert!(actual > 200);
    }

    #[test]
    fn bounded_solver_returns_only_a_complete_fixed_point() {
        let predecessors = vec![vec![], vec![0], vec![1], vec![1, 2]];
        let reachable = vec![true; predecessors.len()];
        let dominators = dominators_with_work_limit(&predecessors, &reachable, 1_000).unwrap();

        assert_eq!(dominators[0], [true, false, false, false]);
        assert_eq!(dominators[1], [true, true, false, false]);
        assert_eq!(dominators[2], [true, true, true, false]);
        assert_eq!(dominators[3], [true, true, false, true]);
    }
}

fn index_constant(context: &Context, value: pliron::value::Value) -> Option<u64> {
    let definition = value.defining_op()?;
    Operation::get_op_dyn(definition, context)
        .downcast_ref::<IndexConstantOp>()?
        .value(context)
}

fn reachable_blocks(edges: &[Vec<usize>]) -> Vec<bool> {
    let mut reachable = vec![false; edges.len()];
    if edges.is_empty() {
        return reachable;
    }
    let mut stack = vec![0];
    while let Some(block) = stack.pop() {
        if reachable[block] {
            continue;
        }
        reachable[block] = true;
        stack.extend(edges[block].iter().copied());
    }
    reachable
}

fn definitely_reachable_blocks(
    context: &Context,
    blocks: &[Ptr<BasicBlock>],
    block_indices: &HashMap<Ptr<BasicBlock>, usize>,
) -> Vec<bool> {
    let mut reachable = vec![false; blocks.len()];
    if blocks.is_empty() {
        return reachable;
    }
    let mut stack = vec![0];
    while let Some(block_index) = stack.pop() {
        if reachable[block_index] {
            continue;
        }
        reachable[block_index] = true;
        let Some(terminator) = blocks[block_index].deref(context).get_terminator(context) else {
            continue;
        };
        let operation = Operation::get_op_dyn(terminator, context);
        if operation.downcast_ref::<BranchOp>().is_none()
            && operation.downcast_ref::<BranchArgsOp>().is_none()
        {
            continue;
        }
        for successor in operation.get_operation().deref(context).successors() {
            if let Some(successor) = block_indices.get(&successor) {
                stack.push(*successor);
            }
        }
    }
    reachable
}

fn strongly_connected_components(edges: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let mut reverse = vec![Vec::new(); edges.len()];
    for (from, successors) in edges.iter().enumerate() {
        for successor in successors {
            reverse[*successor].push(from);
        }
    }
    let mut visited = vec![false; edges.len()];
    let mut order = Vec::with_capacity(edges.len());
    for root in 0..edges.len() {
        if visited[root] {
            continue;
        }
        visited[root] = true;
        let mut stack = vec![(root, 0_usize)];
        while let Some((block, next)) = stack.pop() {
            if let Some(successor) = edges[block].get(next).copied() {
                stack.push((block, next + 1));
                if !visited[successor] {
                    visited[successor] = true;
                    stack.push((successor, 0));
                }
            } else {
                order.push(block);
            }
        }
    }
    visited.fill(false);
    let mut components = Vec::new();
    for root in order.into_iter().rev() {
        if visited[root] {
            continue;
        }
        visited[root] = true;
        let mut component = Vec::new();
        let mut stack = vec![root];
        while let Some(block) = stack.pop() {
            component.push(block);
            for predecessor in &reverse[block] {
                if !visited[*predecessor] {
                    visited[*predecessor] = true;
                    stack.push(*predecessor);
                }
            }
        }
        components.push(component);
    }
    components
}

fn is_cycle(component: &[usize], edges: &[Vec<usize>]) -> bool {
    component.len() > 1
        || component
            .first()
            .is_some_and(|block| edges[*block].contains(block))
}

fn report(finding: PlironProgressFindingV1) -> PlironProgressReportV1 {
    PlironProgressReportV1 {
        findings: vec![finding],
        certificates: Vec::new(),
    }
}
