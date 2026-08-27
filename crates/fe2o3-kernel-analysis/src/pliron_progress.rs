//! Workload-neutral termination and progress checks over ranked PLIRON CFGs.
//!
//! The analysis proves only a closed canonical induction form. Other cyclic
//! control flow is rejected when nontermination is structural, or reported as
//! incomplete when a ranking function would require a stronger solver.

use std::{collections::HashMap, fmt};

use dialect_kernel::{
    BranchArgsOp, BranchOp, IndexBinaryKindAttr, IndexBinaryOp, IndexConstantOp,
    IndexLessThanBranchArgsOp,
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

    let mut predecessors = vec![Vec::new(); blocks.len()];
    for (source, successors) in edges.iter().enumerate() {
        for successor in successors {
            predecessors[*successor].push(source);
        }
    }
    for incoming in &mut predecessors {
        incoming.sort_unstable();
        incoming.dedup();
    }
    let reachable = reachable_blocks(&edges);
    let definitely_reachable = definitely_reachable_blocks(context, &blocks, &block_indices);
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
            &predecessors,
            &component,
        ) {
            CanonicalLoopResultV1::Proved(certificate) => certificates.push(certificate),
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
    Proved(PlironProgressCertificateV1),
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
    predecessors: &[Vec<usize>],
    component: &[usize],
) -> CanonicalLoopResultV1 {
    if component.len() < 2 {
        return CanonicalLoopResultV1::Incomplete(
            "a canonical induction loop needs a distinct guarded header and body",
        );
    }
    for header_index in component {
        let header = blocks[*header_index];
        let header_block = header.deref(context);
        let Some(terminator) = header_block.get_terminator(context) else {
            continue;
        };
        let terminator = Operation::get_op_dyn(terminator, context);
        let Some(branch) = terminator.downcast_ref::<IndexLessThanBranchArgsOp>() else {
            continue;
        };
        let Some(header_induction_slot) = (0..header_block.get_num_arguments())
            .find(|slot| header_block.get_argument(*slot) == branch.lhs(context))
        else {
            continue;
        };
        let raw = branch.get_operation().deref(context);
        let Some(body) = raw.successors().next() else {
            continue;
        };
        let Some(exit) = raw.successors().nth(1) else {
            continue;
        };
        let (Some(body_index), Some(exit_index)) = (
            block_indices.get(&body).copied(),
            block_indices.get(&exit).copied(),
        ) else {
            continue;
        };
        if !component.contains(&body_index) || component.contains(&exit_index) {
            continue;
        }
        let body_induction_slots = branch
            .true_arguments(context)
            .iter()
            .enumerate()
            .filter_map(|(slot, value)| (*value == branch.lhs(context)).then_some(slot))
            .collect::<Vec<_>>();
        if body_induction_slots.len() != 1 {
            return CanonicalLoopResultV1::Incomplete(
                "the guarded edge does not carry one unambiguous induction value",
            );
        }
        if predecessors[body_index].as_slice() != [*header_index] {
            return CanonicalLoopResultV1::Incomplete(
                "the loop body has a predecessor other than its guarded header",
            );
        }
        let internal_header_predecessors = predecessors[*header_index]
            .iter()
            .copied()
            .filter(|block| component.contains(block))
            .collect::<Vec<_>>();
        if internal_header_predecessors.len() != 1
            || !predecessors[*header_index]
                .iter()
                .any(|block| !component.contains(block))
        {
            return CanonicalLoopResultV1::Incomplete(
                "the loop header does not have exactly one recurrence and a canonical external entry",
            );
        }
        let bound = branch.rhs(context);
        let bound_operation_is_internal = bound
            .defining_op()
            .and_then(|definition| operation_blocks.get(&definition))
            .is_some_and(|block| component.contains(block));
        let bound_block_is_internal = bound
            .defining_block()
            .and_then(|block| block_indices.get(&block))
            .is_some_and(|block| component.contains(block));
        if bound_operation_is_internal || bound_block_is_internal {
            return CanonicalLoopResultV1::Incomplete(
                "the loop bound depends on a value defined inside the cycle",
            );
        }
        let mut visited = vec![false; blocks.len()];
        let mut visited_count = 0_usize;
        let mut previous_index = *header_index;
        let mut current_index = body_index;
        let mut current_induction_slot = body_induction_slots[0];
        let (latch_index, latch_argument, next) = loop {
            if current_index == *header_index || visited[current_index] {
                return CanonicalLoopResultV1::Incomplete(
                    "the guarded loop body contains a cycle that bypasses its header",
                );
            }
            if !component.contains(&current_index) {
                return CanonicalLoopResultV1::Incomplete(
                    "the guarded loop body leaves the cycle before its recurrence",
                );
            }
            visited[current_index] = true;
            visited_count += 1;
            let current = blocks[current_index];
            let current_block = current.deref(context);
            if current_induction_slot >= current_block.get_num_arguments() {
                return CanonicalLoopResultV1::Incomplete(
                    "a canonical loop edge does not target its tracked induction slot",
                );
            }
            if predecessors[current_index].as_slice() != [previous_index] {
                return CanonicalLoopResultV1::Incomplete(
                    "the canonical loop body has a side entry, join, or alternate recurrence",
                );
            }
            let Some(terminator) = current_block.get_terminator(context) else {
                return CanonicalLoopResultV1::Incomplete(
                    "a canonical loop body block has no terminator",
                );
            };
            let terminator = Operation::get_op_dyn(terminator, context);
            let Some(forward) = terminator.downcast_ref::<BranchArgsOp>() else {
                return CanonicalLoopResultV1::Incomplete(
                    "a canonical loop body must be a single-recurrence ring",
                );
            };
            let raw_forward = forward.get_operation().deref(context);
            if raw_forward.successors().count() != 1 {
                return CanonicalLoopResultV1::Incomplete(
                    "a canonical loop body branch must have exactly one successor",
                );
            }
            let successor = raw_forward
                .successors()
                .next()
                .expect("one successor checked");
            let Some(successor_index) = block_indices.get(&successor).copied() else {
                return CanonicalLoopResultV1::Incomplete(
                    "a canonical loop body successor is outside the kernel function",
                );
            };
            let arguments = forward.arguments(context);
            let current_argument = current_block.get_argument(current_induction_slot);
            if successor_index == *header_index {
                if current_index != internal_header_predecessors[0]
                    || visited_count != component.len() - 1
                {
                    return CanonicalLoopResultV1::Incomplete(
                        "the canonical recurrence does not cover every block in the cycle exactly once",
                    );
                }
                let Some(next) = arguments.get(header_induction_slot).copied() else {
                    return CanonicalLoopResultV1::Incomplete(
                        "the recurrence does not carry the guarded header induction slot",
                    );
                };
                break (current_index, current_argument, next);
            }
            let induction_slots = arguments
                .iter()
                .enumerate()
                .filter_map(|(slot, value)| (*value == current_argument).then_some(slot))
                .collect::<Vec<_>>();
            if induction_slots.len() != 1 {
                return CanonicalLoopResultV1::Incomplete(
                    "a non-latch loop edge does not forward one unambiguous induction value unchanged",
                );
            }
            previous_index = current_index;
            current_index = successor_index;
            current_induction_slot = induction_slots[0];
        };
        if next == latch_argument {
            return zero_step_result(
                context,
                blocks,
                component,
                header,
                header_induction_slot,
                branch.rhs(context),
                "the induction variable is unchanged on the backedge",
            );
        }
        let Some(increment_definition) = next.defining_op() else {
            return CanonicalLoopResultV1::Incomplete(
                "the backedge value is not a locally reconstructed induction update",
            );
        };
        if operation_blocks.get(&increment_definition) != Some(&latch_index) {
            return CanonicalLoopResultV1::Incomplete(
                "the induction update is not defined in the unique loop latch",
            );
        }
        let increment = Operation::get_op_dyn(increment_definition, context);
        let Some(increment) = increment.downcast_ref::<IndexBinaryOp>() else {
            return CanonicalLoopResultV1::Incomplete(
                "the induction update is not target-neutral index addition",
            );
        };
        if increment.kind(context) != Some(IndexBinaryKindAttr::Add)
            || increment.lhs(context) != latch_argument
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
                    component,
                    header,
                    header_induction_slot,
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
        return CanonicalLoopResultV1::Proved(PlironProgressCertificateV1 {
            header: *header_index,
            body: body_index,
            exit: exit_index,
            induction: branch.lhs(context).unique_name(context).to_string(),
            bound: branch.rhs(context).unique_name(context).to_string(),
            step,
        });
    }
    CanonicalLoopResultV1::Incomplete(
        "the cycle has no supported `i < bound; i := i + positive_constant` header and backedge",
    )
}

fn zero_step_result(
    context: &Context,
    blocks: &[Ptr<BasicBlock>],
    component: &[usize],
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
        if component.contains(&predecessor_index) {
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
