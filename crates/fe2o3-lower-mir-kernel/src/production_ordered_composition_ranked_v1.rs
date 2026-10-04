//! Conservative safety projection of the actual root and composed value graph.
//! Memory rows come from the same-owner complete formal extraction. Their entire
//! launch envelope is checked; additional runtime buffer bounds remain explicit.
//! This is never executable KIR and does not prove a user functional algorithm.
use super::*;
use fe2o3_kernel_ir::{
    AddressSpace, ByteExpression, FormalMemoryAccessKind, FormalMemoryObligations,
    OrderedProgramCallKeyV1, VerifiedOrderedProgramCompositionV1,
};
use fe2o3_pliron::{
    ProductionRankedBlockV1 as Block, ProductionRankedKernelV1 as Recipe,
    ProductionRankedOperationV1 as OpR, ProductionRankedTerminatorV1 as EndR,
    ProductionRankedValueIdV1 as IdR, ProductionRankedValueV1 as ValueR,
};
#[path = "production_ordered_composition_dependencies_v1.rs"]
mod dependencies;
use dependencies::Dependencies;

pub(super) const MAX_NODES: usize = 4096;
pub(super) const MAX_DEPENDENCIES: usize = 16;
pub(super) const PROJECTION_STORAGE: usize = 4 * 1024 * 1024;
const WORK: usize = 131_072;
type E = OrderedCompositionCheckAuxV1;

/// An extra unresolved runtime requirement introduced by conservative widening.
/// This is NOT a claim that a source predicate establishes the runtime extent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrderedCompositionLaunchEnvelopeRequirementV1 {
    parameter: u32,
    minimum_bytes: u64,
    initialized_read: bool,
    write_permission: bool,
}
impl OrderedCompositionLaunchEnvelopeRequirementV1 {
    /// Returns the original logical allocation parameter index.
    pub const fn parameter_index(self) -> u32 {
        self.parameter
    }
    /// Returns the full-launch minimum buffer length, still a host obligation.
    pub const fn minimum_byte_len(self) -> u64 {
        self.minimum_bytes
    }
    /// Whether the full-launch range must be initialized before reading.
    pub const fn requires_initialized_read(self) -> bool {
        self.initialized_read
    }
    /// Whether the full-launch range requires write permission.
    pub const fn requires_write_permission(self) -> bool {
        self.write_permission
    }
}

/// Exact dependency attribution in a safety-only projection. A helper called
/// twice has distinct incoming call keys but the same original function/body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrderedCompositionRankedDependencyV1 {
    function: u32,
    call: Option<OrderedProgramCallKeyV1>,
    block: fe2o3_kernel_ir::BlockId,
    operation: Option<u32>,
    result: ValueId,
    descriptor: Option<u8>,
    ranked: IdR,
}
impl OrderedCompositionRankedDependencyV1 {
    /// Returns the exact canonical function ordinal.
    pub const fn function_ordinal(self) -> u32 {
        self.function
    }
    /// Returns the static helper call identity, or None for the root.
    pub const fn incoming_call(self) -> Option<OrderedProgramCallKeyV1> {
        self.call
    }
    /// Returns the actual canonical block identity.
    pub const fn block(self) -> fe2o3_kernel_ir::BlockId {
        self.block
    }
    /// Returns the operation ordinal; None denotes a block parameter.
    pub const fn operation_ordinal(self) -> Option<u32> {
        self.operation
    }
    /// Returns the actual SSA value attributed by this row.
    pub const fn result(self) -> ValueId {
        self.result
    }
    /// Returns the authored instruction ordinal, if this is an instruction row.
    pub const fn descriptor_ordinal(self) -> Option<u8> {
        self.descriptor
    }
    /// Returns the analysis-only value; it is not a physical register.
    pub const fn ranked_value(self) -> IdR {
        self.ranked
    }
}
pub(super) struct Projection {
    pub recipe: Recipe,
    pub dependencies: Vec<OrderedCompositionRankedDependencyV1>,
    pub bounds: Vec<OrderedCompositionLaunchEnvelopeRequirementV1>,
}

pub(super) fn refusal(message: &'static str) -> E {
    E::Relation(message)
}
pub(super) fn vector<T>(count: usize) -> Result<Vec<T>, E> {
    let mut rows = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| ArgumentResourceV1::Allocation)?;
    if rows.capacity() != count {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    Ok(rows)
}
pub(super) fn copy<T: Copy>(rows: &[T]) -> Result<Vec<T>, E> {
    let mut result = vector(rows.len())?;
    result.extend_from_slice(rows);
    Ok(result)
}
pub(super) fn storage_bound() -> Result<usize, E> {
    Ok(argument_sum_v1(&[
        std::mem::size_of::<Projection>(),
        std::mem::size_of::<Dependencies<'_, '_, '_>>(),
        argument_product_v1(MAX_NODES, std::mem::size_of::<OpR>())?,
        argument_product_v1(
            MAX_NODES,
            std::mem::size_of::<OrderedCompositionRankedDependencyV1>(),
        )?,
        dependencies::memo_storage_bound()?,
        dependencies::scratch_storage_bound()?,
        argument_product_v1(MAX_NODES * MAX_DEPENDENCIES, std::mem::size_of::<ValueR>())?,
        argument_product_v1(
            8,
            std::mem::size_of::<OrderedCompositionLaunchEnvelopeRequirementV1>(),
        )?,
        argument_product_v1(64, std::mem::size_of::<ValueR>())?,
        4096,
    ])?)
}
pub(super) fn project(
    owner: &VerifiedOrderedProgramCompositionV1,
    source_launch: &crate::ProductionSourceLaunchRosterV1,
    formal: &FormalMemoryObligations,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Projection, E> {
    let floor = budget.storage();
    budget.with_prepaid_scope(floor, 1, WORK, PROJECTION_STORAGE, |budget| {
        if storage_bound()? > PROJECTION_STORAGE {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        let module = owner.canonical().module();
        let root = &module.functions[owner.root_function_ordinal() as usize];
        let [kernel] = module.kernels.as_slice() else {
            return Err(refusal("one composition kernel"));
        };
        let [launch] = source_launch.roots() else {
            return Err(refusal("one composition source launch"));
        };
        let layout = launch.layout();
        let extents = layout.global_extents();
        if launch.source_rank() != 1
            || extents[0] == 0
            || extents[1..] != [1, 1]
            || !extents[0].is_multiple_of(64)
            || layout.workgroup_extents() != [64, 1, 1]
            || layout.subgroup_size() != 64
            || !layout.full_physical_workgroups()
            || formal.kernel() != &kernel.id
            || formal.entry() != &root.id
            || formal
                .invocations()
                .is_none_or(|r| r.start() != 0 || r.end_exclusive() != extents[0])
            || !formal.inter_invocation_conflicts().is_empty()
            || formal.allocations().len() > 8
            || formal.accesses().len() > 16
        {
            return Err(refusal("composition formal/source launch relation"));
        }
        let mut state = Dependencies::new(owner, budget)?;
        state.push(OpR::ExecutionLayout {
            grid_identity: layout.grid_identity(),
            global_extents: extents,
            workgroup_extents: layout.workgroup_extents(),
            subgroup_size: 64,
            full_physical_workgroups: true,
        })?;
        // The projection is an over-approximation. Exact formal conditions and
        // these extra full-envelope bounds stay separately unresolved.
        let mut bounds = vector(8)?;
        let mut views = [None; 8];
        for (i, allocation) in formal.allocations().iter().enumerate() {
            if allocation.address_space() != AddressSpace::Global {
                return Err(refusal("composition projection global allocation only"));
            }
            let mut minimum = 0u64;
            let mut writable = false;
            let mut initialized_read = false;
            for access in formal
                .accesses()
                .iter()
                .filter(|a| a.allocation() == allocation.identity())
            {
                if access.address_space() != AddressSpace::Global
                    || access.byte_width() != 4
                    || access.alignment() < 4
                    || access.kind() == FormalMemoryAccessKind::Atomic
                    || access.byte_offset() != ByteExpression::invocation_affine(0, 4)
                    || access.invocations()
                        != formal.invocations().ok_or_else(|| refusal("launch"))?
                {
                    return Err(refusal(
                        "composition projection exact u32 invocation coordinate",
                    ));
                }
                minimum = extents[0]
                    .checked_mul(4)
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
                writable |= access.kind() == FormalMemoryAccessKind::Write;
                initialized_read |= access.kind() == FormalMemoryAccessKind::Read;
            }
            // Unused allocations are not invented as memory effects or requirements.
            if minimum == 0 {
                continue;
            }
            let id = state.next()?;
            state.push(OpR::ViewInSpace {
                result: id,
                element_width: 32,
                writable,
                shape: copy(&[extents[0]])?,
                dynamic_extents: Vec::new(),
                allocation_origin: u64::from(allocation.identity().parameter_index()) + 1,
                // Different logical parameters are NOT asserted disjoint.
                noalias_class: 0,
                memory_space: dialect_kernel::MemorySpaceAttr::Global,
            })?;
            views[i] = Some(id);
            bounds.push(OrderedCompositionLaunchEnvelopeRequirementV1 {
                parameter: allocation.identity().parameter_index(),
                minimum_bytes: minimum,
                initialized_read,
                write_permission: writable,
            });
        }
        state.root_dependencies()?;
        let index = state.next()?;
        state.push(OpR::InvocationIndex {
            result: index,
            dimension: 0,
            launch_extent: extents[0],
        })?;
        for access in formal.accesses() {
            let allocation = formal
                .allocations()
                .iter()
                .position(|a| a.identity() == access.allocation())
                .ok_or_else(|| refusal("actual formal allocation"))?;
            let view = views[allocation].ok_or_else(|| refusal("actual formal view"))?;
            // Exact location is independently joined against the immutable root;
            // no caller-authored effect roster reaches this constructor.
            state.check_access(access)?;
            state.push(OpR::Access {
                kind: match access.kind() {
                    FormalMemoryAccessKind::Read => dialect_kernel::AccessKindAttr::Read,
                    FormalMemoryAccessKind::Write => dialect_kernel::AccessKindAttr::Write,
                    FormalMemoryAccessKind::Atomic => return Err(refusal("atomic projection")),
                },
                view: ValueR::Local(view),
                indices: copy(&[ValueR::Local(index)])?,
            })?;
        }
        let (operations, dependencies) = state.finish();
        let mut blocks = vector(1)?;
        blocks.push(Block::new(operations, EndR::Return));
        let recipe = Recipe::new(root.id.as_str(), root.signature.parameters.len(), blocks)
            .map_err(E::RankedRecipe)?;
        Ok(Projection {
            recipe,
            dependencies,
            bounds,
        })
    })
}

#[cfg(test)]
pub(super) fn substituted_row_for_test(
    mut row: OrderedCompositionRankedDependencyV1,
) -> OrderedCompositionRankedDependencyV1 {
    row.result = ValueId(row.result.0.wrapping_add(1));
    row
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn composition_projection_measured_payload_fits_prepaid_envelope() {
        assert!(storage_bound().unwrap() <= PROJECTION_STORAGE);
        assert_eq!(
            vector::<ValueR>(MAX_DEPENDENCIES).unwrap().capacity(),
            MAX_DEPENDENCIES
        );
    }
}
