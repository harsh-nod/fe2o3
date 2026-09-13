//! Bounded exact execution traces over the closed ranked PLIRON CFG.
//!
//! This is shared analysis infrastructure. It evaluates sparse index facts for
//! every invocation in a retained static launch and records only target-neutral
//! memory and synchronization events. Verifier passes consume the traces; this
//! module does not itself decide race freedom or initialization.

use std::collections::{HashMap, HashSet};

use dialect_gpu::{
    AddressSpaceAttr, BarrierOp, ExecutionDomainAttr, ExecutionLayoutOp, FenceOp, HierarchyAttr,
    MemoryOrderAttr, MemoryScopeAttr,
};
use dialect_kernel::{
    AccessKindAttr, AllocationEffectOp, AtomicOrderingAttr, AtomicScopeAttr, BranchArgsOp,
    BranchOp, IndexBinaryKindAttr, IndexBinaryOp, IndexEqualBranchArgsOp, IndexEqualBranchOp,
    IndexLessThanBranchArgsOp, IndexLessThanBranchOp, MAX_RANKED_MEMORY_RANK, MemorySpaceAttr,
    RankedAccessOp, RankedViewOp, ReturnOp, TensorLayoutOp, TrapOp,
};
use pliron::{
    basic_block::BasicBlock,
    context::{Context, Ptr},
    linked_list::ContainsLinkedList,
    op::Op,
    operation::Operation,
    value::Value,
};

use crate::production_analysis::pliron_function_inventory::BoundedPlironFunctionInventoryV1;
use crate::production_analysis::{
    ProductionAnalysisInputCensusV1, ProductionAnalysisResourceLimitV1,
    ProductionAnalysisResourceLimitsV1, ProductionAnalysisResourcePhaseV1,
    ProductionAnalysisResourceUpperBoundV1,
};
use crate::{MAX_PLIRON_RACE_INVOCATIONS_V1, SparseIndexFailureV1};

pub const MAX_PLIRON_TRACE_TOTAL_STEPS_V1: usize = 1_048_576;
const MAX_PLIRON_TRACE_VALUE_VISITS_PER_QUERY_V1: usize = 65;
const TRACE_VALUE_STACK_FRAMES_PER_VISIT_V1: usize = 2;
const TRACE_VALUE_STACK_FIXED_FRAMES_V1: usize = 1;
const TRACE_VALUE_STACK_WORK_PER_VISIT_V1: usize = 8;
const TRACE_VALUE_FIXED_WORK_PER_VISIT_V1: usize = 16;
const TRACE_VALUE_QUERY_SETUP_WORK_V1: usize = 3;
const TRACE_VALUE_STACK_STORAGE_PER_FRAME_V1: usize = 8;
const TRACE_VALUE_CACHE_STORAGE_PER_VISIT_V1: usize = 8;
const TRACE_VALUE_ACTIVE_STORAGE_PER_VISIT_V1: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProductionInvocationTraceResourceAdmissionV1 {
    upper_bound: ProductionAnalysisResourceUpperBoundV1,
    invocation_count: usize,
    launch_rank: usize,
    event_upper_bound: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProductionInvocationTraceResourcePreflightV1 {
    attempt_upper_bound: ProductionAnalysisResourceUpperBoundV1,
    exact_admission: Option<ProductionInvocationTraceResourceAdmissionV1>,
}

impl ProductionInvocationTraceResourcePreflightV1 {
    pub(crate) const fn attempt_upper_bound(self) -> ProductionAnalysisResourceUpperBoundV1 {
        self.attempt_upper_bound
    }

    /// Authenticates the cardinalities exposed to downstream analyses against
    /// the trace cache that was materialized under `attempt_upper_bound`.
    /// Failed trace attempts have no exact downstream admission.
    pub(crate) fn exact_admission(
        self,
        traces: Result<&[PlironInvocationTraceV1], PlironTraceFailureV1>,
    ) -> Result<
        Option<ProductionInvocationTraceResourceAdmissionV1>,
        ProductionAnalysisResourceLimitV1,
    > {
        let Ok(traces) = traces else {
            return Ok(None);
        };
        let Some(attempt_admission) = self.exact_admission else {
            return Err(trace_authenticated_cardinality_error_v1());
        };
        let event_count = traces.iter().try_fold(0_usize, |total, trace| {
            total
                .checked_add(trace.events.len())
                .ok_or_else(trace_authenticated_cardinality_error_v1)
        })?;
        if traces.len() > attempt_admission.invocation_count
            || event_count > attempt_admission.event_upper_bound
            || traces
                .iter()
                .any(|trace| trace.invocation.len() > attempt_admission.launch_rank)
        {
            return Err(trace_authenticated_cardinality_error_v1());
        }
        Ok(Some(ProductionInvocationTraceResourceAdmissionV1 {
            upper_bound: attempt_admission.upper_bound,
            invocation_count: traces.len(),
            launch_rank: attempt_admission.launch_rank,
            event_upper_bound: event_count,
        }))
    }
}

impl ProductionInvocationTraceResourceAdmissionV1 {
    pub(crate) const fn upper_bound(self) -> ProductionAnalysisResourceUpperBoundV1 {
        self.upper_bound
    }

    pub(crate) const fn invocation_count(self) -> usize {
        self.invocation_count
    }

    pub(crate) const fn launch_rank(self) -> usize {
        self.launch_rank
    }

    pub(crate) const fn event_upper_bound(self) -> usize {
        self.event_upper_bound
    }

    #[cfg(test)]
    pub(crate) const fn from_counts_for_test_v1(
        invocation_count: usize,
        launch_rank: usize,
        event_upper_bound: usize,
    ) -> Self {
        Self {
            upper_bound: ProductionAnalysisResourceUpperBoundV1::zero(),
            invocation_count,
            launch_rank,
            event_upper_bound,
        }
    }
}

fn trace_resource_overflow_v1() -> ProductionAnalysisResourceLimitV1 {
    ProductionAnalysisResourceLimitV1 {
        phase: ProductionAnalysisResourcePhaseV1::InvocationTrace,
        resource: "invocation trace resource upper bound",
    }
}

fn trace_authenticated_cardinality_error_v1() -> ProductionAnalysisResourceLimitV1 {
    ProductionAnalysisResourceLimitV1 {
        phase: ProductionAnalysisResourcePhaseV1::InvocationTrace,
        resource: "authenticated trace exceeds admitted attempt upper bound",
    }
}

fn checked_trace_sum_v1(items: &[usize]) -> Result<usize, ProductionAnalysisResourceLimitV1> {
    items.iter().try_fold(0_usize, |total, item| {
        total
            .checked_add(*item)
            .ok_or_else(trace_resource_overflow_v1)
    })
}

fn checked_trace_product_v1(
    lhs: usize,
    rhs: usize,
) -> Result<usize, ProductionAnalysisResourceLimitV1> {
    lhs.checked_mul(rhs).ok_or_else(trace_resource_overflow_v1)
}

include!("pliron_invocation_trace/resources_v1.rs");

fn charge_trace_work_v1(total: &mut usize, amount: usize) -> Result<(), PlironTraceFailureV1> {
    *total = total
        .checked_add(amount)
        .ok_or(PlironTraceFailureV1::ResourceLimit)?;
    if *total > MAX_PLIRON_TRACE_TOTAL_STEPS_V1 {
        return Err(PlironTraceFailureV1::ResourceLimit);
    }
    Ok(())
}

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
        memory_scope: MemoryScopeAttr,
        address_space: AddressSpaceAttr,
        order: MemoryOrderAttr,
    },
    Fence {
        location: PlironTraceLocationV1,
        memory_scope: MemoryScopeAttr,
        address_space: AddressSpaceAttr,
        order: MemoryOrderAttr,
    },
    TensorInstruction {
        location: PlironTraceLocationV1,
        subgroup_width: u16,
        claimed_active_lanes: u32,
    },
    Trap {
        location: PlironTraceLocationV1,
    },
    Memory {
        location: PlironTraceLocationV1,
        view: Value,
        memory_space: MemorySpaceAttr,
        access: AccessKindAttr,
        atomic_ordering: Option<AtomicOrderingAttr>,
        atomic_scope: Option<AtomicScopeAttr>,
        indices: Vec<Option<u64>>,
        allocation_origin: u64,
        noalias_class: u64,
        view_signature: (u32, Vec<u64>),
    },
    CollectiveAllocation {
        location: PlironTraceLocationV1,
        access: AccessKindAttr,
        memory_space: MemorySpaceAttr,
        allocation_origin: u64,
        noalias_class: u64,
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
    pub(crate) execution_domain: ExecutionDomainAttr,
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

pub(crate) fn pliron_execution_layout_with_inventory_v1(
    context: &Context,
    inventory: &BoundedPlironFunctionInventoryV1,
) -> Result<Option<PlironExecutionLayoutV1>, PlironTraceFailureV1> {
    let mut layout = None;
    for site in inventory.operations() {
        let operation = Operation::get_op_dyn(site.pointer(), context);
        let Some(candidate) = operation.downcast_ref::<ExecutionLayoutOp>() else {
            continue;
        };
        if site.block() != 0 || layout.is_some() {
            return Err(PlironTraceFailureV1::InvalidExecutionLayout);
        }
        let (
            Some(grid),
            Some(global_extents),
            Some(workgroup_extents),
            Some(subgroup_size),
            execution_domain,
        ) = (
            candidate.grid_identity(context),
            candidate.global_extents(context),
            candidate.workgroup_extents(context),
            candidate.subgroup_size(context),
            candidate.execution_domain(context),
        )
        else {
            return Err(PlironTraceFailureV1::InvalidExecutionLayout);
        };
        let workgroup_size = workgroup_extents
            .into_iter()
            .try_fold(1_u64, u64::checked_mul);
        if workgroup_extents.contains(&0)
            || workgroup_size.is_none()
            || subgroup_size == 0
            || (execution_domain == ExecutionDomainAttr::FullPhysicalWorkgroups
                && global_extents
                    .iter()
                    .zip(workgroup_extents)
                    .any(|(global, workgroup)| *global != 0 && !global.is_multiple_of(workgroup)))
        {
            return Err(PlironTraceFailureV1::InvalidExecutionLayout);
        }
        layout = Some(PlironExecutionLayoutV1 {
            grid,
            global_extents,
            workgroup_extents,
            subgroup_size,
            execution_domain,
        });
    }
    Ok(layout)
}

pub(crate) fn trace_pliron_invocations_with_inputs_v1(
    context: &Context,
    inventory: &BoundedPlironFunctionInventoryV1,
    sparse: &crate::SparseIndexAnalysisV1,
    layout: Option<PlironExecutionLayoutV1>,
) -> Result<Vec<PlironInvocationTraceV1>, PlironTraceFailureV1> {
    let needs_scoped_layout = inventory.operations().iter().any(|site| {
        let operation = Operation::get_op_dyn(site.pointer(), context);
        operation.downcast_ref::<BarrierOp>().is_some()
            || operation
                .downcast_ref::<AllocationEffectOp>()
                .is_some_and(|effect| {
                    effect.memory_space(context) == Some(MemorySpaceAttr::Workgroup)
                })
            || operation
                .downcast_ref::<RankedViewOp>()
                .is_some_and(|view| view.memory_space(context) == Some(MemorySpaceAttr::Workgroup))
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
            let has_scope = inventory.operations().iter().any(|site| {
                Operation::get_op_dyn(site.pointer(), context)
                    .downcast_ref::<BarrierOp>()
                    .is_some_and(|barrier| barrier.execution_scope(context) == Some(scope))
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

    let blocks = inventory.blocks();
    let block_indices = blocks
        .iter()
        .enumerate()
        .map(|(index, block)| (*block, index))
        .collect::<HashMap<Ptr<BasicBlock>, usize>>();
    let mut traces = Vec::with_capacity(invocation_count as usize);
    let mut total_steps = 0_usize;
    let mut total_evaluation_visits = 0_usize;
    for linear in 0..invocation_count {
        let invocation = decode_invocation(linear, &launch_extents);
        let mut events = Vec::new();
        let mut block_index = 0_usize;
        let mut environment = HashMap::<Value, u64>::new();
        let mut visited = HashSet::<(usize, Vec<Option<u64>>)>::new();
        loop {
            charge_trace_work_v1(&mut total_steps, 1)?;
            let block = blocks
                .get(block_index)
                .copied()
                .ok_or(PlironTraceFailureV1::UnsupportedTerminator { block: block_index })?;
            let block_state = (0..block.deref(context).get_num_arguments())
                .map(|argument| {
                    environment
                        .get(&block.deref(context).get_argument(argument))
                        .copied()
                })
                .collect::<Vec<_>>();
            if !visited.insert((block_index, block_state)) {
                return Err(PlironTraceFailureV1::CyclicControlFlow { block: block_index });
            }
            let terminator = block
                .deref(context)
                .get_terminator(context)
                .ok_or(PlironTraceFailureV1::UnsupportedTerminator { block: block_index })?;
            let mut terminator_index = None;
            for site in inventory.block_operations(block_index) {
                let operation_index = site.operation();
                let operation = site.pointer();
                // Charge the scan itself, including pure definitions and the
                // terminator. Event count is therefore bounded by this same
                // budget instead of only by the number of visited blocks.
                charge_trace_work_v1(&mut total_steps, 1)?;
                if operation == terminator {
                    terminator_index = Some(operation_index);
                    continue;
                }
                let operation = Operation::get_op_dyn(operation, context);
                if let Some(barrier) = operation.downcast_ref::<BarrierOp>() {
                    let (
                        Some(execution_scope),
                        Some(memory_scope),
                        Some(address_space),
                        Some(order),
                    ) = (
                        barrier.execution_scope(context),
                        barrier.memory_scope(context),
                        barrier.address_space(context),
                        barrier.order(context),
                    )
                    else {
                        return Err(PlironTraceFailureV1::UnsupportedTerminator {
                            block: block_index,
                        });
                    };
                    if execution_scope == HierarchyAttr::Grid {
                        return Err(PlironTraceFailureV1::UnsupportedGridSynchronization {
                            block: block_index,
                            operation: operation_index,
                        });
                    }
                    events.push(PlironTraceEventV1::Barrier {
                        location: PlironTraceLocationV1 {
                            block: block_index,
                            operation: operation_index,
                        },
                        execution_scope,
                        memory_scope,
                        address_space,
                        order,
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
                } else if let Some(tensor) = operation.downcast_ref::<TensorLayoutOp>() {
                    let contract = tensor.contract(context).map_err(|_| {
                        PlironTraceFailureV1::UnsupportedTerminator { block: block_index }
                    })?;
                    let claimed_active_lanes = tensor.active_lanes(context).ok_or(
                        PlironTraceFailureV1::UnsupportedTerminator { block: block_index },
                    )?;
                    events.push(PlironTraceEventV1::TensorInstruction {
                        location: PlironTraceLocationV1 {
                            block: block_index,
                            operation: operation_index,
                        },
                        subgroup_width: contract.subgroup_width,
                        claimed_active_lanes,
                    });
                } else if let Some(effect) = operation.downcast_ref::<AllocationEffectOp>() {
                    let (
                        Some(access),
                        Some(memory_space),
                        Some(allocation_origin),
                        Some(noalias_class),
                    ) = (
                        effect.kind(context),
                        effect.memory_space(context),
                        effect.allocation_origin(context),
                        effect.noalias_class(context),
                    )
                    else {
                        return Err(PlironTraceFailureV1::UnsupportedTerminator {
                            block: block_index,
                        });
                    };
                    if memory_space == MemorySpaceAttr::Workgroup {
                        events.push(PlironTraceEventV1::CollectiveAllocation {
                            location: PlironTraceLocationV1 {
                                block: block_index,
                                operation: operation_index,
                            },
                            access,
                            memory_space,
                            allocation_origin,
                            noalias_class,
                        });
                    }
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
                        .map(|index| {
                            evaluate_trace_value_v1(
                                context,
                                sparse,
                                &invocation,
                                &environment,
                                &mut total_evaluation_visits,
                                index,
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    events.push(PlironTraceEventV1::Memory {
                        location: PlironTraceLocationV1 {
                            block: block_index,
                            operation: operation_index,
                        },
                        view,
                        memory_space,
                        access: access_kind,
                        atomic_ordering: access.atomic_ordering(context),
                        atomic_scope: access.atomic_scope(context),
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
            if terminator.downcast_ref::<TrapOp>().is_some() {
                let operation = terminator_index
                    .ok_or(PlironTraceFailureV1::UnsupportedTerminator { block: block_index })?;
                events.push(PlironTraceEventV1::Trap {
                    location: PlironTraceLocationV1 {
                        block: block_index,
                        operation,
                    },
                });
                break;
            }
            let raw = terminator.get_operation().deref(context);
            if terminator
                .downcast_ref::<dialect_gpu::optimization_v1::CondBranchOp>()
                .is_some()
            {
                // A data Boolean is not an index expression. This concrete
                // trace service has no Boolean evaluator or path-fork state.
                return Err(PlironTraceFailureV1::UnresolvedBranch { block: block_index });
            }
            let (successor, edge_arguments) = if terminator.downcast_ref::<BranchOp>().is_some() {
                (raw.successors().next(), Vec::new())
            } else if let Some(branch) = terminator.downcast_ref::<BranchArgsOp>() {
                (raw.successors().next(), branch.arguments(context))
            } else if let Some(branch) = terminator.downcast_ref::<IndexLessThanBranchOp>() {
                let lhs = evaluate_trace_value_v1(
                    context,
                    sparse,
                    &invocation,
                    &environment,
                    &mut total_evaluation_visits,
                    branch.lhs(context),
                )?
                .ok_or(PlironTraceFailureV1::UnresolvedBranch { block: block_index })?;
                let rhs = evaluate_trace_value_v1(
                    context,
                    sparse,
                    &invocation,
                    &environment,
                    &mut total_evaluation_visits,
                    branch.rhs(context),
                )?
                .ok_or(PlironTraceFailureV1::UnresolvedBranch { block: block_index })?;
                (raw.successors().nth(usize::from(lhs >= rhs)), Vec::new())
            } else if let Some(branch) = terminator.downcast_ref::<IndexLessThanBranchArgsOp>() {
                let lhs = evaluate_trace_value_v1(
                    context,
                    sparse,
                    &invocation,
                    &environment,
                    &mut total_evaluation_visits,
                    branch.lhs(context),
                )?;
                let rhs = evaluate_trace_value_v1(
                    context,
                    sparse,
                    &invocation,
                    &environment,
                    &mut total_evaluation_visits,
                    branch.rhs(context),
                )?;
                if let (Some(lhs), Some(rhs)) = (lhs, rhs) {
                    let successor_index = usize::from(lhs >= rhs);
                    let arguments = if successor_index == 0 {
                        branch.true_arguments(context)
                    } else {
                        branch.false_arguments(context)
                    };
                    (raw.successors().nth(successor_index), arguments)
                } else if let Some(exit) = summarize_trace_silent_finite_loop_v1(
                    context,
                    sparse,
                    &invocation,
                    &environment,
                    &mut total_evaluation_visits,
                    blocks[block_index],
                    branch,
                )? {
                    (Some(exit), branch.false_arguments(context))
                } else {
                    return Err(PlironTraceFailureV1::UnresolvedBranch { block: block_index });
                }
            } else if let Some(branch) = terminator.downcast_ref::<IndexEqualBranchOp>() {
                let lhs = evaluate_trace_value_v1(
                    context,
                    sparse,
                    &invocation,
                    &environment,
                    &mut total_evaluation_visits,
                    branch.lhs(context),
                )?
                .ok_or(PlironTraceFailureV1::UnresolvedBranch { block: block_index })?;
                let rhs = evaluate_trace_value_v1(
                    context,
                    sparse,
                    &invocation,
                    &environment,
                    &mut total_evaluation_visits,
                    branch.rhs(context),
                )?
                .ok_or(PlironTraceFailureV1::UnresolvedBranch { block: block_index })?;
                (raw.successors().nth(usize::from(lhs != rhs)), Vec::new())
            } else if let Some(branch) = terminator.downcast_ref::<IndexEqualBranchArgsOp>() {
                let lhs = evaluate_trace_value_v1(
                    context,
                    sparse,
                    &invocation,
                    &environment,
                    &mut total_evaluation_visits,
                    branch.lhs(context),
                )?
                .ok_or(PlironTraceFailureV1::UnresolvedBranch { block: block_index })?;
                let rhs = evaluate_trace_value_v1(
                    context,
                    sparse,
                    &invocation,
                    &environment,
                    &mut total_evaluation_visits,
                    branch.rhs(context),
                )?
                .ok_or(PlironTraceFailureV1::UnresolvedBranch { block: block_index })?;
                let successor_index = usize::from(lhs != rhs);
                let arguments = if successor_index == 0 {
                    branch.true_arguments(context)
                } else {
                    branch.false_arguments(context)
                };
                (raw.successors().nth(successor_index), arguments)
            } else {
                return Err(PlironTraceFailureV1::UnsupportedTerminator { block: block_index });
            };
            let successor = successor
                .ok_or(PlironTraceFailureV1::UnsupportedTerminator { block: block_index })?;
            let next_block = block_indices
                .get(&successor)
                .copied()
                .ok_or(PlironTraceFailureV1::UnsupportedTerminator { block: block_index })?;
            bind_edge_arguments_v1(
                context,
                sparse,
                &invocation,
                &mut environment,
                &mut total_evaluation_visits,
                successor,
                &edge_arguments,
                block_index,
            )?;
            block_index = next_block;
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

/// Summarize only a canonical machine-finite loop whose body cannot emit a
/// trace event. This lets the event analyses cross a dynamic induction-only
/// loop without inventing a value for its external bound. Any extra operation,
/// edge, carried value, or non-unit transition keeps the branch unresolved.
fn summarize_trace_silent_finite_loop_v1(
    context: &Context,
    sparse: &crate::SparseIndexAnalysisV1,
    invocation: &[u64],
    environment: &HashMap<Value, u64>,
    total_evaluation_visits: &mut usize,
    header: Ptr<BasicBlock>,
    branch: &IndexLessThanBranchArgsOp,
) -> Result<Option<Ptr<BasicBlock>>, PlironTraceFailureV1> {
    let header_block = header.deref(context);
    if header_block.get_num_arguments() != 1
        || branch.lhs(context) != header_block.get_argument(0)
        || branch.true_arguments(context).as_slice() != [branch.lhs(context)]
        || !branch.false_arguments(context).is_empty()
    {
        return Ok(None);
    }

    let raw = branch.get_operation().deref(context);
    let Some(body) = raw.successors().next() else {
        return Ok(None);
    };
    let Some(exit) = raw.successors().nth(1) else {
        return Ok(None);
    };
    let body_block = body.deref(context);
    if body_block.get_num_arguments() != 1 {
        return Ok(None);
    }
    let operations = body_block.iter(context).collect::<Vec<_>>();
    if operations.len() != 2 || body_block.get_terminator(context) != Some(operations[1]) {
        return Ok(None);
    }

    let increment = Operation::get_op_dyn(operations[0], context);
    let Some(increment) = increment.downcast_ref::<IndexBinaryOp>() else {
        return Ok(None);
    };
    if increment.kind(context) != Some(IndexBinaryKindAttr::Add)
        || increment.lhs(context) != body_block.get_argument(0)
        || evaluate_trace_value_v1(
            context,
            sparse,
            invocation,
            environment,
            total_evaluation_visits,
            increment.rhs(context),
        )? != Some(1)
    {
        return Ok(None);
    }

    let backedge = Operation::get_op_dyn(operations[1], context);
    let Some(backedge) = backedge.downcast_ref::<BranchArgsOp>() else {
        return Ok(None);
    };
    let arguments = backedge.arguments(context);
    if arguments.as_slice() != [increment.result(context)]
        || backedge.get_operation().deref(context).successors().next() != Some(header)
    {
        return Ok(None);
    }
    Ok(Some(exit))
}

fn evaluate_trace_value_v1(
    context: &Context,
    sparse: &crate::SparseIndexAnalysisV1,
    invocation: &[u64],
    environment: &HashMap<Value, u64>,
    total_evaluation_visits: &mut usize,
    value: Value,
) -> Result<Option<u64>, PlironTraceFailureV1> {
    evaluate_trace_value_with_origin_v1(
        context,
        sparse,
        invocation,
        environment,
        total_evaluation_visits,
        value,
    )
    .map(|result| result.map(|(value, _)| value))
}

#[derive(Clone, Copy)]
enum TraceValueEvaluationFrameV1 {
    Evaluate(Value),
    FinishBinary {
        value: Value,
        lhs: Value,
        rhs: Value,
        kind: Option<IndexBinaryKindAttr>,
    },
}

fn charge_trace_value_visit_v1(
    query_visits: &mut usize,
    total_evaluation_visits: &mut usize,
) -> Result<(), PlironTraceFailureV1> {
    let next_query = query_visits
        .checked_add(1)
        .ok_or(PlironTraceFailureV1::ResourceLimit)?;
    let next_total = total_evaluation_visits
        .checked_add(1)
        .ok_or(PlironTraceFailureV1::ResourceLimit)?;
    *query_visits = next_query;
    *total_evaluation_visits = next_total;
    if next_query > MAX_PLIRON_TRACE_VALUE_VISITS_PER_QUERY_V1
        || next_total > MAX_PLIRON_TRACE_TOTAL_STEPS_V1
    {
        return Err(PlironTraceFailureV1::ResourceLimit);
    }
    Ok(())
}

fn push_trace_value_frame_v1(
    stack: &mut Vec<TraceValueEvaluationFrameV1>,
    frame: TraceValueEvaluationFrameV1,
    frame_limit: usize,
) -> Result<(), PlironTraceFailureV1> {
    if stack.len() == frame_limit {
        return Err(PlironTraceFailureV1::ResourceLimit);
    }
    stack
        .try_reserve(1)
        .map_err(|_| PlironTraceFailureV1::ResourceLimit)?;
    stack.push(frame);
    Ok(())
}

fn cache_trace_value_v1(
    cache: &mut HashMap<Value, Option<(u64, bool)>>,
    value: Value,
    result: Option<(u64, bool)>,
) -> Result<(), PlironTraceFailureV1> {
    cache
        .try_reserve(1)
        .map_err(|_| PlironTraceFailureV1::ResourceLimit)?;
    cache.insert(value, result);
    Ok(())
}

fn evaluate_trace_value_with_origin_v1(
    context: &Context,
    sparse: &crate::SparseIndexAnalysisV1,
    invocation: &[u64],
    environment: &HashMap<Value, u64>,
    total_evaluation_visits: &mut usize,
    value: Value,
) -> Result<Option<(u64, bool)>, PlironTraceFailureV1> {
    let frame_limit = MAX_PLIRON_TRACE_VALUE_VISITS_PER_QUERY_V1
        .checked_mul(TRACE_VALUE_STACK_FRAMES_PER_VISIT_V1)
        .and_then(|frames| frames.checked_add(TRACE_VALUE_STACK_FIXED_FRAMES_V1))
        .ok_or(PlironTraceFailureV1::ResourceLimit)?;
    let mut stack = Vec::new();
    let mut cache = HashMap::<Value, Option<(u64, bool)>>::new();
    let mut active = HashSet::<Value>::new();
    let mut query_visits = 0_usize;
    push_trace_value_frame_v1(
        &mut stack,
        TraceValueEvaluationFrameV1::Evaluate(value),
        frame_limit,
    )?;

    while let Some(frame) = stack.pop() {
        match frame {
            TraceValueEvaluationFrameV1::Evaluate(value) => {
                if cache.contains_key(&value) {
                    continue;
                }
                charge_trace_value_visit_v1(&mut query_visits, total_evaluation_visits)?;
                if let Some(result) = environment.get(&value).copied() {
                    cache_trace_value_v1(&mut cache, value, Some((result, true)))?;
                    continue;
                }
                if let Some(result) = sparse.fact(value).evaluate(invocation) {
                    cache_trace_value_v1(&mut cache, value, Some((result, false)))?;
                    continue;
                }
                if active.contains(&value) {
                    return Ok(None);
                }
                active
                    .try_reserve(1)
                    .map_err(|_| PlironTraceFailureV1::ResourceLimit)?;
                active.insert(value);
                let Some(definition) = value.defining_op() else {
                    active.remove(&value);
                    cache_trace_value_v1(&mut cache, value, None)?;
                    continue;
                };
                let operation = Operation::get_op_dyn(definition, context);
                let Some(binary) = operation.downcast_ref::<IndexBinaryOp>() else {
                    active.remove(&value);
                    cache_trace_value_v1(&mut cache, value, None)?;
                    continue;
                };
                let lhs = binary.lhs(context);
                let rhs = binary.rhs(context);
                push_trace_value_frame_v1(
                    &mut stack,
                    TraceValueEvaluationFrameV1::FinishBinary {
                        value,
                        lhs,
                        rhs,
                        kind: binary.kind(context),
                    },
                    frame_limit,
                )?;
                push_trace_value_frame_v1(
                    &mut stack,
                    TraceValueEvaluationFrameV1::Evaluate(rhs),
                    frame_limit,
                )?;
                push_trace_value_frame_v1(
                    &mut stack,
                    TraceValueEvaluationFrameV1::Evaluate(lhs),
                    frame_limit,
                )?;
            }
            TraceValueEvaluationFrameV1::FinishBinary {
                value,
                lhs,
                rhs,
                kind,
            } => {
                let lhs = cache.get(&lhs).copied().flatten();
                let rhs = cache.get(&rhs).copied().flatten();
                let result = match (lhs, rhs, kind) {
                    (
                        Some((lhs, lhs_from_environment)),
                        Some((rhs, rhs_from_environment)),
                        kind,
                    ) if lhs_from_environment || rhs_from_environment => match kind {
                        Some(IndexBinaryKindAttr::Add) => lhs.checked_add(rhs),
                        Some(IndexBinaryKindAttr::Multiply) => lhs.checked_mul(rhs),
                        Some(IndexBinaryKindAttr::Remainder) if rhs != 0 => Some(lhs % rhs),
                        Some(IndexBinaryKindAttr::Divide) if rhs != 0 => Some(lhs / rhs),
                        Some(IndexBinaryKindAttr::Remainder | IndexBinaryKindAttr::Divide)
                        | None => None,
                    }
                    .map(|result| (result, true)),
                    _ => None,
                };
                active.remove(&value);
                cache_trace_value_v1(&mut cache, value, result)?;
            }
        }
    }
    Ok(cache.get(&value).copied().flatten())
}

#[allow(clippy::too_many_arguments)]
fn bind_edge_arguments_v1(
    context: &Context,
    sparse: &crate::SparseIndexAnalysisV1,
    invocation: &[u64],
    environment: &mut HashMap<Value, u64>,
    total_evaluation_visits: &mut usize,
    successor: Ptr<BasicBlock>,
    edge_arguments: &[Value],
    source_block: usize,
) -> Result<(), PlironTraceFailureV1> {
    let successor = successor.deref(context);
    if successor.get_num_arguments() != edge_arguments.len() {
        return Err(PlironTraceFailureV1::UnsupportedTerminator {
            block: source_block,
        });
    }
    let values = edge_arguments
        .iter()
        .map(|argument| {
            evaluate_trace_value_v1(
                context,
                sparse,
                invocation,
                environment,
                total_evaluation_visits,
                *argument,
            )?
            .ok_or(PlironTraceFailureV1::UnresolvedBranch {
                block: source_block,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    for (index, value) in values.into_iter().enumerate() {
        environment.insert(successor.get_argument(index), value);
    }
    Ok(())
}

fn decode_invocation(mut linear: u64, extents: &[u64]) -> Vec<u64> {
    let mut invocation = Vec::with_capacity(extents.len());
    for extent in extents {
        invocation.push(linear % extent);
        linear /= extent;
    }
    invocation
}

include!("pliron_invocation_trace/resource_tests.rs");
