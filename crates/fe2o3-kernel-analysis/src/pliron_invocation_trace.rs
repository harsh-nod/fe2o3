//! Bounded exact execution traces over the closed ranked PLIRON CFG.
//!
//! This is shared analysis infrastructure. It evaluates sparse index facts for
//! every invocation in a retained static launch and records only target-neutral
//! memory and synchronization events. Verifier passes consume the traces; this
//! module does not itself decide race freedom or initialization.

use std::collections::{HashMap, HashSet};

use dialect_gpu::{AddressSpaceAttr, BarrierOp};
use dialect_kernel::{
    AccessKindAttr, BranchOp, IndexLessThanBranchOp, MemorySpaceAttr, RankedAccessOp, RankedViewOp,
    ReturnOp,
};
use pliron::{
    basic_block::BasicBlock,
    builtin::{op_interfaces::OneRegionInterface, ops::FuncOp},
    context::{Context, Ptr},
    linked_list::ContainsLinkedList,
    operation::Operation,
    value::Value,
};

use crate::{
    MAX_PLIRON_RACE_INVOCATIONS_V1, SparseIndexFailureV1, analyze_pliron_sparse_indices_v1,
};

pub const MAX_PLIRON_TRACE_TOTAL_STEPS_V1: usize = 1_048_576;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct PlironTraceLocationV1 {
    pub(crate) block: usize,
    pub(crate) operation: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PlironTraceEventV1 {
    Barrier {
        location: PlironTraceLocationV1,
        address_space: AddressSpaceAttr,
    },
    Memory {
        location: PlironTraceLocationV1,
        view: Value,
        memory_space: MemorySpaceAttr,
        access: AccessKindAttr,
        indices: Vec<Option<u64>>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PlironInvocationTraceV1 {
    pub(crate) invocation: Vec<u64>,
    pub(crate) events: Vec<PlironTraceEventV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PlironTraceFailureV1 {
    Sparse(SparseIndexFailureV1),
    DynamicLaunch { dimension: usize },
    LaunchTooLarge { invocations: u64 },
    UnresolvedBranch { block: usize },
    ForeignView { block: usize, operation: usize },
    UnsupportedTerminator { block: usize },
    CyclicControlFlow { block: usize },
    ResourceLimit,
}

pub(crate) fn trace_pliron_invocations_v1(
    context: &Context,
    function: &FuncOp,
) -> Result<Vec<PlironInvocationTraceV1>, PlironTraceFailureV1> {
    let sparse = analyze_pliron_sparse_indices_v1(context, function)
        .map_err(PlironTraceFailureV1::Sparse)?;
    if let Some(dimension) = sparse
        .launch_extents()
        .iter()
        .position(|extent| *extent == 0)
    {
        return Err(PlironTraceFailureV1::DynamicLaunch { dimension });
    }
    let invocation_count =
        sparse
            .invocation_count()
            .ok_or(PlironTraceFailureV1::LaunchTooLarge {
                invocations: u64::MAX,
            })?;
    if invocation_count > MAX_PLIRON_RACE_INVOCATIONS_V1 {
        return Err(PlironTraceFailureV1::LaunchTooLarge {
            invocations: invocation_count,
        });
    }

    let blocks = function
        .get_region(context)
        .deref(context)
        .iter(context)
        .collect::<Vec<_>>();
    let block_indices = blocks
        .iter()
        .enumerate()
        .map(|(index, block)| (*block, index))
        .collect::<HashMap<Ptr<BasicBlock>, usize>>();
    let mut traces = Vec::with_capacity(invocation_count as usize);
    let mut total_steps = 0_usize;
    for linear in 0..invocation_count {
        let invocation = decode_invocation(linear, sparse.launch_extents());
        let mut events = Vec::new();
        let mut block_index = 0_usize;
        let mut visited = HashSet::new();
        loop {
            total_steps = total_steps.saturating_add(1);
            if total_steps > MAX_PLIRON_TRACE_TOTAL_STEPS_V1 {
                return Err(PlironTraceFailureV1::ResourceLimit);
            }
            if !visited.insert(block_index) {
                return Err(PlironTraceFailureV1::CyclicControlFlow { block: block_index });
            }
            let block = blocks
                .get(block_index)
                .copied()
                .ok_or(PlironTraceFailureV1::UnsupportedTerminator { block: block_index })?;
            let terminator = block
                .deref(context)
                .get_terminator(context)
                .ok_or(PlironTraceFailureV1::UnsupportedTerminator { block: block_index })?;
            for (operation_index, operation) in block.deref(context).iter(context).enumerate() {
                if operation == terminator {
                    continue;
                }
                let operation = Operation::get_op_dyn(operation, context);
                if let Some(barrier) = operation.downcast_ref::<BarrierOp>() {
                    let address_space = barrier.address_space(context).ok_or(
                        PlironTraceFailureV1::UnsupportedTerminator { block: block_index },
                    )?;
                    events.push(PlironTraceEventV1::Barrier {
                        location: PlironTraceLocationV1 {
                            block: block_index,
                            operation: operation_index,
                        },
                        address_space,
                    });
                } else if let Some(access) = operation.downcast_ref::<RankedAccessOp>() {
                    let view = access.view(context);
                    let definition =
                        view.defining_op()
                            .ok_or(PlironTraceFailureV1::ForeignView {
                                block: block_index,
                                operation: operation_index,
                            })?;
                    let definition = Operation::get_op_dyn(definition, context);
                    let view_op = definition.downcast_ref::<RankedViewOp>().ok_or(
                        PlironTraceFailureV1::ForeignView {
                            block: block_index,
                            operation: operation_index,
                        },
                    )?;
                    let memory_space =
                        view_op
                            .memory_space(context)
                            .ok_or(PlironTraceFailureV1::ForeignView {
                                block: block_index,
                                operation: operation_index,
                            })?;
                    let access_kind =
                        access
                            .kind(context)
                            .ok_or(PlironTraceFailureV1::ForeignView {
                                block: block_index,
                                operation: operation_index,
                            })?;
                    let indices = access
                        .indices(context)
                        .into_iter()
                        .map(|index| sparse.fact(index).evaluate(&invocation))
                        .collect();
                    events.push(PlironTraceEventV1::Memory {
                        location: PlironTraceLocationV1 {
                            block: block_index,
                            operation: operation_index,
                        },
                        view,
                        memory_space,
                        access: access_kind,
                        indices,
                    });
                }
            }

            let terminator = Operation::get_op_dyn(terminator, context);
            if terminator.downcast_ref::<ReturnOp>().is_some() {
                break;
            }
            let raw = terminator.get_operation().deref(context);
            let successor = if terminator.downcast_ref::<BranchOp>().is_some() {
                raw.successors().next()
            } else if let Some(branch) = terminator.downcast_ref::<IndexLessThanBranchOp>() {
                let lhs = sparse
                    .fact(branch.lhs(context))
                    .evaluate(&invocation)
                    .ok_or(PlironTraceFailureV1::UnresolvedBranch { block: block_index })?;
                let rhs = sparse
                    .fact(branch.rhs(context))
                    .evaluate(&invocation)
                    .ok_or(PlironTraceFailureV1::UnresolvedBranch { block: block_index })?;
                raw.successors().nth(usize::from(lhs >= rhs))
            } else {
                return Err(PlironTraceFailureV1::UnsupportedTerminator { block: block_index });
            }
            .ok_or(PlironTraceFailureV1::UnsupportedTerminator { block: block_index })?;
            block_index = block_indices
                .get(&successor)
                .copied()
                .ok_or(PlironTraceFailureV1::UnsupportedTerminator { block: block_index })?;
        }
        traces.push(PlironInvocationTraceV1 { invocation, events });
    }
    Ok(traces)
}

fn decode_invocation(mut linear: u64, extents: &[u64]) -> Vec<u64> {
    let mut invocation = Vec::with_capacity(extents.len());
    for extent in extents {
        invocation.push(linear % extent);
        linear /= extent;
    }
    invocation
}
