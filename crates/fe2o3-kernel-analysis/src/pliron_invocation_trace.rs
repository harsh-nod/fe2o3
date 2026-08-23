//! Bounded exact execution traces over the closed ranked PLIRON CFG.
//!
//! This is shared analysis infrastructure. It evaluates sparse index facts for
//! every invocation in a retained static launch and records only target-neutral
//! memory and synchronization events. Verifier passes consume the traces; this
//! module does not itself decide race freedom or initialization.

use std::collections::{HashMap, HashSet};

use dialect_gpu::{
    AddressSpaceAttr, BarrierOp, ExecutionLayoutOp, FenceOp, HierarchyAttr, MemoryOrderAttr,
    MemoryScopeAttr,
};
use dialect_kernel::{
    AccessKindAttr, BranchArgsOp, BranchOp, IndexLessThanBranchArgsOp, IndexLessThanBranchOp,
    MemorySpaceAttr, RankedAccessOp, RankedViewOp, ReturnOp, TensorLayoutOp,
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
        execution_scope: HierarchyAttr,
        address_space: AddressSpaceAttr,
    },
    Fence {
        location: PlironTraceLocationV1,
        memory_scope: MemoryScopeAttr,
        address_space: AddressSpaceAttr,
        order: MemoryOrderAttr,
    },
    TensorInstruction {
        location: PlironTraceLocationV1,
    },
    Memory {
        location: PlironTraceLocationV1,
        view: Value,
        memory_space: MemorySpaceAttr,
        access: AccessKindAttr,
        indices: Vec<Option<u64>>,
        allocation_origin: u64,
        noalias_class: u64,
        view_signature: (u32, Vec<u64>),
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PlironInvocationTraceV1 {
    pub(crate) invocation: Vec<u64>,
    pub(crate) grid: u64,
    pub(crate) workgroup: u64,
    pub(crate) subgroup: u64,
    pub(crate) lane: u64,
    pub(crate) events: Vec<PlironTraceEventV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PlironTraceFailureV1 {
    Sparse(SparseIndexFailureV1),
    DynamicLaunch {
        dimension: usize,
    },
    LaunchTooLarge {
        invocations: u64,
    },
    UnresolvedBranch {
        block: usize,
    },
    ForeignView {
        block: usize,
        operation: usize,
    },
    UnsupportedTerminator {
        block: usize,
    },
    CyclicControlFlow {
        block: usize,
    },
    MissingExecutionLayout,
    InvalidExecutionLayout,
    UnsupportedGridSynchronization {
        block: usize,
        operation: usize,
    },
    PartialBarrierParticipants {
        scope: HierarchyAttr,
        dimension: usize,
        global_extent: u64,
        workgroup_extent: u64,
    },
    ResourceLimit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PlironExecutionLayoutV1 {
    pub(crate) grid: u64,
    pub(crate) global_extents: [u64; 3],
    pub(crate) workgroup_extents: [u64; 3],
    pub(crate) subgroup_size: u64,
}

impl PlironExecutionLayoutV1 {
    pub(crate) fn scoped_identity(self, invocation: &[u64]) -> Option<(u64, u64, u64)> {
        let invocation: [u64; 3] = invocation.try_into().ok()?;
        let mut workgroup = [0_u64; 3];
        let mut local = [0_u64; 3];
        let mut workgroup_counts = [0_u64; 3];
        for dimension in 0..3 {
            let workgroup_extent = self.workgroup_extents[dimension];
            if workgroup_extent == 0 || self.global_extents[dimension] == 0 {
                return None;
            }
            workgroup[dimension] = invocation[dimension] / workgroup_extent;
            local[dimension] = invocation[dimension] % workgroup_extent;
            workgroup_counts[dimension] = self.global_extents[dimension].div_ceil(workgroup_extent);
        }
        let workgroup = workgroup[0].checked_add(workgroup_counts[0].checked_mul(
            workgroup[1].checked_add(workgroup_counts[1].checked_mul(workgroup[2])?)?,
        )?)?;
        let local = local[0].checked_add(self.workgroup_extents[0].checked_mul(
            local[1].checked_add(self.workgroup_extents[1].checked_mul(local[2])?)?,
        )?)?;
        Some((
            workgroup,
            local / self.subgroup_size,
            local % self.subgroup_size,
        ))
    }
}

pub(crate) fn pliron_execution_layout_v1(
    context: &Context,
    function: &FuncOp,
) -> Result<Option<PlironExecutionLayoutV1>, PlironTraceFailureV1> {
    let mut layout = None;
    for (block_index, block) in function
        .get_region(context)
        .deref(context)
        .iter(context)
        .enumerate()
    {
        for operation in block.deref(context).iter(context) {
            let operation = Operation::get_op_dyn(operation, context);
            let Some(candidate) = operation.downcast_ref::<ExecutionLayoutOp>() else {
                continue;
            };
            if block_index != 0 || layout.is_some() {
                return Err(PlironTraceFailureV1::InvalidExecutionLayout);
            }
            let (Some(grid), Some(global_extents), Some(workgroup_extents), Some(subgroup_size)) = (
                candidate.grid_identity(context),
                candidate.global_extents(context),
                candidate.workgroup_extents(context),
                candidate.subgroup_size(context),
            ) else {
                return Err(PlironTraceFailureV1::InvalidExecutionLayout);
            };
            let workgroup_size = workgroup_extents
                .into_iter()
                .try_fold(1_u64, u64::checked_mul);
            if workgroup_extents.contains(&0)
                || workgroup_size.is_none()
                || subgroup_size == 0
                || workgroup_size.is_some_and(|size| subgroup_size > size)
                || workgroup_size.is_some_and(|size| !size.is_multiple_of(subgroup_size))
            {
                return Err(PlironTraceFailureV1::InvalidExecutionLayout);
            }
            layout = Some(PlironExecutionLayoutV1 {
                grid,
                global_extents,
                workgroup_extents,
                subgroup_size,
            });
        }
    }
    Ok(layout)
}

pub(crate) fn trace_pliron_invocations_v1(
    context: &Context,
    function: &FuncOp,
) -> Result<Vec<PlironInvocationTraceV1>, PlironTraceFailureV1> {
    let sparse = analyze_pliron_sparse_indices_v1(context, function)
        .map_err(PlironTraceFailureV1::Sparse)?;
    let layout = pliron_execution_layout_v1(context, function)?;
    let needs_scoped_layout = function
        .get_region(context)
        .deref(context)
        .iter(context)
        .any(|block| {
            block.deref(context).iter(context).any(|operation| {
                let operation = Operation::get_op_dyn(operation, context);
                operation.downcast_ref::<BarrierOp>().is_some()
                    || operation
                        .downcast_ref::<RankedViewOp>()
                        .is_some_and(|view| {
                            view.memory_space(context) == Some(MemorySpaceAttr::Workgroup)
                        })
            })
        });
    if needs_scoped_layout && layout.is_none() {
        return Err(PlironTraceFailureV1::MissingExecutionLayout);
    }
    let launch_extents = if let Some(layout) = layout {
        for dimension in 0..sparse.launch_extents().len().max(3) {
            if let Some(declared) = sparse.declared_launch_extent(dimension) {
                let Some(layout_extent) = layout.global_extents.get(dimension).copied() else {
                    return Err(PlironTraceFailureV1::InvalidExecutionLayout);
                };
                if declared != 0 && layout_extent != declared {
                    return Err(PlironTraceFailureV1::InvalidExecutionLayout);
                }
            }
        }
        layout.global_extents.to_vec()
    } else {
        sparse.launch_extents().to_vec()
    };
    if let Some(dimension) = launch_extents.iter().position(|extent| *extent == 0) {
        return Err(PlironTraceFailureV1::DynamicLaunch { dimension });
    }
    let invocation_count = launch_extents
        .iter()
        .try_fold(1_u64, |total, extent| total.checked_mul(*extent))
        .ok_or(PlironTraceFailureV1::LaunchTooLarge {
            invocations: u64::MAX,
        })?;
    if invocation_count > MAX_PLIRON_RACE_INVOCATIONS_V1 {
        return Err(PlironTraceFailureV1::LaunchTooLarge {
            invocations: invocation_count,
        });
    }
    if let Some(layout) = layout {
        for scope in [HierarchyAttr::Workgroup, HierarchyAttr::Subgroup] {
            let has_scope = function
                .get_region(context)
                .deref(context)
                .iter(context)
                .any(|block| {
                    block.deref(context).iter(context).any(|operation| {
                        Operation::get_op_dyn(operation, context)
                            .downcast_ref::<BarrierOp>()
                            .is_some_and(|barrier| barrier.execution_scope(context) == Some(scope))
                    })
                });
            if has_scope {
                for (dimension, (global_extent, workgroup_extent)) in launch_extents
                    .iter()
                    .zip(layout.workgroup_extents)
                    .enumerate()
                {
                    if !global_extent.is_multiple_of(workgroup_extent) {
                        return Err(PlironTraceFailureV1::PartialBarrierParticipants {
                            scope,
                            dimension,
                            global_extent: *global_extent,
                            workgroup_extent,
                        });
                    }
                }
            }
        }
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
        let invocation = decode_invocation(linear, &launch_extents);
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
                    let execution_scope = barrier.execution_scope(context).ok_or(
                        PlironTraceFailureV1::UnsupportedTerminator { block: block_index },
                    )?;
                    if execution_scope == HierarchyAttr::Grid {
                        return Err(PlironTraceFailureV1::UnsupportedGridSynchronization {
                            block: block_index,
                            operation: operation_index,
                        });
                    }
                    let address_space = barrier.address_space(context).ok_or(
                        PlironTraceFailureV1::UnsupportedTerminator { block: block_index },
                    )?;
                    events.push(PlironTraceEventV1::Barrier {
                        location: PlironTraceLocationV1 {
                            block: block_index,
                            operation: operation_index,
                        },
                        execution_scope,
                        address_space,
                    });
                } else if let Some(fence) = operation.downcast_ref::<FenceOp>() {
                    let (Some(memory_scope), Some(address_space), Some(order)) = (
                        fence.memory_scope(context),
                        fence.address_space(context),
                        fence.order(context),
                    ) else {
                        return Err(PlironTraceFailureV1::UnsupportedTerminator {
                            block: block_index,
                        });
                    };
                    events.push(PlironTraceEventV1::Fence {
                        location: PlironTraceLocationV1 {
                            block: block_index,
                            operation: operation_index,
                        },
                        memory_scope,
                        address_space,
                        order,
                    });
                } else if operation.downcast_ref::<TensorLayoutOp>().is_some() {
                    events.push(PlironTraceEventV1::TensorInstruction {
                        location: PlironTraceLocationV1 {
                            block: block_index,
                            operation: operation_index,
                        },
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
                        allocation_origin: view_op.allocation_origin(context).unwrap_or(0),
                        noalias_class: view_op.noalias_class(context).unwrap_or(0),
                        view_signature: view_op
                            .view_type(context)
                            .map(|ty| {
                                let ty = ty.deref(context);
                                (ty.element_width(), ty.shape().to_vec())
                            })
                            .unwrap_or_default(),
                    });
                }
            }

            let terminator = Operation::get_op_dyn(terminator, context);
            if terminator.downcast_ref::<ReturnOp>().is_some() {
                break;
            }
            let raw = terminator.get_operation().deref(context);
            let successor = if terminator.downcast_ref::<BranchOp>().is_some()
                || terminator.downcast_ref::<BranchArgsOp>().is_some()
            {
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
            } else if let Some(branch) = terminator.downcast_ref::<IndexLessThanBranchArgsOp>() {
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
        let (grid, workgroup, subgroup, lane) = if let Some(layout) = layout {
            let (workgroup, subgroup, lane) = layout
                .scoped_identity(&invocation)
                .ok_or(PlironTraceFailureV1::InvalidExecutionLayout)?;
            (layout.grid, workgroup, subgroup, lane)
        } else {
            (0, 0, 0, linear)
        };
        traces.push(PlironInvocationTraceV1 {
            invocation,
            grid,
            workgroup,
            subgroup,
            lane,
            events,
        });
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
