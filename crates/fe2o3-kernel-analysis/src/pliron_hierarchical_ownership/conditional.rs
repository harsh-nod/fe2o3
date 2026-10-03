//! Closed live-graph coverage for `if global_x < extent { output[global_x] = ... }`.

use std::collections::{HashMap, VecDeque};

use dialect_gpu::ExecutionLayoutOp;
use dialect_kernel::{
    AccessKindAttr, BranchOp, DYNAMIC_EXTENT, DimensionOp, IndexConstantOp, IndexLessThanBranchOp,
    IndexUnsignedCastOp, InvocationIndexOp, MemorySpaceAttr, OwnershipContractOp,
    OwnershipCoverageAttr, OwnershipPartitionAttr, RankedAccessOp, RankedViewOp, ReturnOp,
    SemanticBinaryOp, SemanticConstantOp, SemanticExpressionCommitmentOp, SemanticSymbolOp,
    SemanticTypedBinaryOp, SemanticTypedCastOp, SemanticTypedCompareOp, SemanticTypedConstantOp,
    SemanticTypedExpressionRootOp, SemanticTypedSelectOp, SemanticTypedSymbolOp,
    SemanticTypedUnaryOp,
};
use dialect_proof::{EvidenceRefOp, ObligationOp, RequireEffectRefinementOp, RequireRefinementOp};
use pliron::{context::Context, op::Op, operation::Operation};

use super::{ContractV1, HierarchicalOwnershipLocationV1};
use crate::pliron_function_inventory::{BoundedPlironFunctionInventoryV1, PlironOperationSiteV1};
use crate::pliron_invocation_trace::PlironExecutionLayoutV1;

/// A structural, conditional coverage result retained in the exact owner-held
/// pipeline report. The write domain is `[0, min(global_x_extent, output_extent))`,
/// with each coordinate written once and both guard paths completing normally.
///
/// Total coverage additionally requires the actual output extent to be no greater
/// than the actual launch extent. The extent argument is a *ranked* argument, not
/// a source/packed ABI ordinal. This record proves neither that binding nor the
/// stored value, target code, or hardware execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuardedIdentityCoverageV1 {
    view_name: String,
    view_definition: HierarchicalOwnershipLocationV1,
    contract: HierarchicalOwnershipLocationV1,
    guard: HierarchicalOwnershipLocationV1,
    write: HierarchicalOwnershipLocationV1,
    ranked_extent_argument: usize,
    allocation_origin: u64,
    noalias_class: u64,
    element_width: u32,
    layout: PlironExecutionLayoutV1,
}

impl GuardedIdentityCoverageV1 {
    pub fn view_name(&self) -> &str {
        &self.view_name
    }
    pub const fn view_definition(&self) -> HierarchicalOwnershipLocationV1 {
        self.view_definition
    }
    pub const fn contract_location(&self) -> HierarchicalOwnershipLocationV1 {
        self.contract
    }
    pub const fn guard_location(&self) -> HierarchicalOwnershipLocationV1 {
        self.guard
    }
    pub const fn write_location(&self) -> HierarchicalOwnershipLocationV1 {
        self.write
    }
    pub const fn ranked_extent_argument(&self) -> usize {
        self.ranked_extent_argument
    }
    pub const fn allocation_origin(&self) -> u64 {
        self.allocation_origin
    }
    pub const fn noalias_class(&self) -> u64 {
        self.noalias_class
    }
    /// Ranked element width in bits, not the host buffer byte stride.
    pub const fn element_width(&self) -> u32 {
        self.element_width
    }
    pub const fn grid_identity(&self) -> u64 {
        self.layout.grid
    }
    /// `None` denotes a symbolic launch extent, never a proved empty launch.
    pub const fn static_global_x_extent(&self) -> Option<u64> {
        if self.layout.global_extents[0] == 0 {
            None
        } else {
            Some(self.layout.global_extents[0])
        }
    }
    pub const fn workgroup_extents(&self) -> [u64; 3] {
        self.layout.workgroup_extents
    }
    pub const fn subgroup_size(&self) -> u64 {
        self.layout.subgroup_size
    }
    pub const fn requires_full_physical_workgroups(&self) -> bool {
        matches!(
            self.layout.execution_domain,
            dialect_gpu::ExecutionDomainAttr::FullPhysicalWorkgroups
        )
    }
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

fn location(site: PlironOperationSiteV1) -> HierarchicalOwnershipLocationV1 {
    HierarchicalOwnershipLocationV1 {
        block: site.block(),
        operation: site.operation(),
    }
}

#[derive(Clone, Copy)]
enum Exit {
    Branch(usize),
    Guard(usize, usize),
    Return,
}

impl Exit {
    fn successors(self) -> impl Iterator<Item = usize> {
        match self {
            Self::Branch(next) => [Some(next), None],
            Self::Guard(yes, no) => [Some(yes), Some(no)],
            Self::Return => [None, None],
        }
        .into_iter()
        .flatten()
    }
}

/// Optional recognition never changes generic ExactEffectDomain acceptance.
/// A conditional consumer must require this record explicitly.
pub(super) fn classify_guarded_identity_v1(
    context: &Context,
    inventory: &BoundedPlironFunctionInventoryV1,
    layout: PlironExecutionLayoutV1,
    contract: &ContractV1,
) -> Option<GuardedIdentityCoverageV1> {
    if contract.coverage != OwnershipCoverageAttr::ExactEffectDomain
        || contract.partition != OwnershipPartitionAttr::ExactSets
        || layout.global_extents[1..] != [1, 1]
    {
        return None;
    }
    let entry = *inventory.blocks().first()?;
    let view = &contract.view_op;
    let ty = view.view_type(context)?;
    let ty = ty.deref(context);
    if ty.shape() != [DYNAMIC_EXTENT]
        || !ty.writable()
        || view.memory_space(context) != Some(MemorySpaceAttr::Global)
    {
        return None;
    }
    let extent = view.dynamic_extent(context, 0)?;
    if extent.defining_block() != Some(entry) {
        return None;
    }
    let ranked_extent_argument = extent.try_find_index(context).ok()?;
    let allocation_origin = view.allocation_origin(context)?;
    let noalias_class = view.noalias_class(context)?;
    if allocation_origin == 0 || noalias_class == 0 {
        return None;
    }

    let mut view_definition = None;
    let mut invocation = None;
    let mut guard = None;
    let mut write = None;
    let mut ownership = None;
    let mut execution = None;
    for site in inventory.operations().iter().copied() {
        let raw = site.pointer().deref(context);
        if raw.num_regions() != 0 {
            return None;
        }
        let op = Operation::get_op_dyn(site.pointer(), context);
        if op.verify(context).is_err() {
            return None;
        }
        let is_terminator = op.downcast_ref::<BranchOp>().is_some()
            || op.downcast_ref::<IndexLessThanBranchOp>().is_some()
            || op.downcast_ref::<ReturnOp>().is_some();
        if is_terminator {
            if inventory.block_operations(site.block()).last()?.pointer() != site.pointer() {
                return None;
            }
            if op.downcast_ref::<IndexLessThanBranchOp>().is_some() && guard.replace(site).is_some()
            {
                return None;
            }
        } else {
            if raw.get_num_successors() != 0 {
                return None;
            }
            if let Some(actual) = op.downcast_ref::<RankedViewOp>() {
                if site.block() != 0
                    || actual.result(context) != contract.view
                    || view_definition.replace(site).is_some()
                {
                    return None;
                }
            } else if let Some(actual) = op.downcast_ref::<InvocationIndexOp>() {
                if site.block() != 0
                    || actual.dimension(context) != Some(0)
                    || actual.launch_extent(context) != Some(layout.global_extents[0])
                    || invocation.replace(actual.result(context)).is_some()
                {
                    return None;
                }
            } else if let Some(actual) = op.downcast_ref::<DimensionOp>() {
                if site.block() != 0
                    || actual.view(context) != contract.view
                    || actual.dimension(context) != Some(0)
                {
                    return None;
                }
            } else if let Some(actual) = op.downcast_ref::<RankedAccessOp>() {
                if actual.kind(context) != Some(AccessKindAttr::Write)
                    || actual.view(context) != contract.view
                    || actual.checked_success(context).is_some()
                    || actual.atomic_ordering(context).is_some()
                    || actual.atomic_scope(context).is_some()
                    || write.replace(site).is_some()
                {
                    return None;
                }
            } else if op.downcast_ref::<OwnershipContractOp>().is_some() {
                if site.block() != 0
                    || location(site) != contract.location
                    || ownership.replace(site).is_some()
                {
                    return None;
                }
            } else if op.downcast_ref::<ExecutionLayoutOp>().is_some() {
                if site.block() != 0 || execution.replace(site).is_some() {
                    return None;
                }
            } else if !is_effect_free_metadata(op.as_ref()) {
                return None;
            }
        }
    }
    ownership?;
    execution?;
    let invocation = invocation?;
    let write = write?;
    let access = Operation::get_op::<RankedAccessOp>(write.pointer(), context)?;
    if access.indices(context) != [invocation] {
        return None;
    }
    let guard = guard?;
    let branch = Operation::get_op::<IndexLessThanBranchOp>(guard.pointer(), context)?;
    let rhs = branch.rhs(context);
    if branch.lhs(context) != invocation {
        return None;
    }
    if rhs != extent {
        let dim = Operation::get_op::<DimensionOp>(rhs.defining_op()?, context)?;
        if dim.view(context) != contract.view || dim.dimension(context) != Some(0) {
            return None;
        }
    }

    let blocks = inventory.blocks();
    let block_ids = blocks
        .iter()
        .copied()
        .enumerate()
        .map(|(i, b)| (b, i))
        .collect::<HashMap<_, _>>();
    let mut exits = Vec::with_capacity(blocks.len());
    let mut indegrees = vec![0_usize; blocks.len()];
    for (index, block) in blocks.iter().enumerate() {
        if index != 0 && block.deref(context).get_num_arguments() != 0 {
            return None;
        }
        let last = inventory.block_operations(index).last()?;
        let raw = last.pointer().deref(context);
        let op = Operation::get_op_dyn(last.pointer(), context);
        let exit = if op.downcast_ref::<ReturnOp>().is_some() {
            Exit::Return
        } else if op.downcast_ref::<BranchOp>().is_some() {
            Exit::Branch(*block_ids.get(&raw.get_successor(0))?)
        } else if last.pointer() == guard.pointer() {
            Exit::Guard(
                *block_ids.get(&raw.get_successor(0))?,
                *block_ids.get(&raw.get_successor(1))?,
            )
        } else {
            return None;
        };
        for successor in exit.successors() {
            indegrees[successor] += 1;
        }
        exits.push(exit);
    }

    // Kahn traversal rejects cycles and unreachable blocks. Four bit states
    // keep work linear even when both guard paths merge through many diamonds.
    const BEFORE: u8 = 1;
    const YES_BEFORE_WRITE: u8 = 2;
    const YES_AFTER_WRITE: u8 = 4;
    const NO_WRITE: u8 = 8;
    if indegrees[0] != 0 {
        return None;
    }
    let mut pending = VecDeque::from([0]);
    let mut states = vec![0_u8; blocks.len()];
    states[0] = BEFORE;
    let mut visited = 0;
    while let Some(block) = pending.pop_front() {
        visited += 1;
        let mut state = states[block];
        if block == write.block() {
            if state != YES_BEFORE_WRITE {
                return None;
            }
            state = YES_AFTER_WRITE;
        }
        match exits[block] {
            Exit::Return => {
                if state == 0 || state & !(YES_AFTER_WRITE | NO_WRITE) != 0 {
                    return None;
                }
            }
            Exit::Branch(next) => states[next] |= state,
            Exit::Guard(yes, no) => {
                if state != BEFORE {
                    return None;
                }
                states[yes] |= YES_BEFORE_WRITE;
                states[no] |= NO_WRITE;
            }
        }
        for next in exits[block].successors() {
            indegrees[next] -= 1;
            if indegrees[next] == 0 {
                pending.push_back(next);
            }
        }
    }
    if visited != blocks.len() {
        return None;
    }
    Some(GuardedIdentityCoverageV1 {
        view_name: contract.view_name.clone(),
        view_definition: location(view_definition?),
        contract: contract.location,
        guard: location(guard),
        write: location(write),
        ranked_extent_argument,
        allocation_origin,
        noalias_class,
        element_width: ty.element_width(),
        layout,
    })
}

fn is_effect_free_metadata(op: &dyn Op) -> bool {
    macro_rules! known {
        ($($kind:ty),+ $(,)?) => { $(op.downcast_ref::<$kind>().is_some())||+ };
    }
    known!(
        IndexConstantOp,
        IndexUnsignedCastOp,
        SemanticSymbolOp,
        SemanticConstantOp,
        SemanticBinaryOp,
        SemanticExpressionCommitmentOp,
        SemanticTypedSymbolOp,
        SemanticTypedConstantOp,
        SemanticTypedUnaryOp,
        SemanticTypedBinaryOp,
        SemanticTypedCompareOp,
        SemanticTypedSelectOp,
        SemanticTypedCastOp,
        SemanticTypedExpressionRootOp,
        ObligationOp,
        EvidenceRefOp,
        RequireEffectRefinementOp,
        RequireRefinementOp
    )
}
