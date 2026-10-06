//! Current-frame live value conjuncts at distinct original source block entries.
//! This does not replace suspended-caller, memory, effect, control or step laws.
use super::super::{
    expanded_execution::ExpandedExecutionBindingsV199, tile_target::TileTargetV176,
};
use super::*;
use fe2o3_lower_mir_kernel::{
    ProductionSourceSsaCarrierShapeV37 as Carrier, ProductionSourceSsaEndpointV36 as Endpoint,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticExecutionRoleV29 as Role, SemanticRustTypeKindV1 as RustType,
    SemanticTypeShapeV1 as Shape,
};
use std::fmt::Write as _;

#[cfg(test)]
#[path = "original_semantic_mir_expanded_live_values_v213_tests.rs"]
mod tests;

macro_rules! emit {
    ($out:expr, $($arg:tt)*) => { write!($out, $($arg)*).map_err(|_| $out.error())? };
}

fn headers() -> usize {
    size_of::<Vec<Vec<Block>>>()
        + 2 * size_of::<Result<Vec<Vec<Block>>>>()
        + size_of::<Vec<Block>>()
        + 2 * size_of::<Result<Vec<Block>>>()
        + size_of::<Endpoint<'_, '_>>()
        + 2 * size_of::<Result<Endpoint<'_, '_>>>()
        + size_of::<Option<Role>>()
        + size_of::<Carrier>()
        + size_of::<Value>()
        + size_of::<Result<()>>()
        + 24 * size_of::<usize>()
        + 12 * size_of::<&()>()
}

pub(in super::super) fn emit_source_cut_values_v213(
    plan: &InvocationPlan<'_, '_>,
    slots: &SourceSlots<'_, '_>,
    target: &TileTargetV176<'_, '_, '_>,
    width: FormalIndexWidth,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    slots.with_source_query_v42(out, |out| {
        out.budget.reserve_storage(headers())?;
        let relation = slots.correspondence(out)?;
        let source = relation.source(out.budget)?;
        out.budget.charge_work(3)?;
        if !std::ptr::eq(source, plan.source(out)?)
            || !std::ptr::eq(slots, target.source_slots(out)?)
            || width == FormalIndexWidth::Unknown
        {
            return Err(mismatch());
        }
        let scalar = ExpandedScalarBindingsV196::derive(slots, target, out)?;
        let execution = ExpandedExecutionBindingsV199::derive(plan, slots, target, out)?;
        let archive = source.source_ssa(out.budget)?;
        let semantic = source.source_semantic(out.budget)?;
        for root in 0..source.root_count(out.budget)? {
            for instance in 0..plan.root(root, out)?.instances.len() {
                let row = plan.instance(root, instance, out)?;
                out.budget.charge_work(1)?;
                if !row.active {
                    continue;
                }
                let function = semantic.functions().get(row.function.index() as usize)
                    .ok_or_else(mismatch)?;
                let ssa = archive.plan_for_function(row.function).ok_or_else(mismatch)?.plan();
                let analysis_floor = out.budget.storage();
                let mut successors = vector(function.blocks().len(), out)?;
                for declaration in function.blocks() {
                    out.budget.charge_work(1)?;
                    let mut edges = vector(declaration.terminator().kind().edge_count(), out)?;
                    declaration.terminator().kind().try_for_each_edge(|edge| {
                        out.budget.charge_work(1)?;
                        edges.push(Block::new(edge.target().index()));
                        Ok::<_, Error>(())
                    })?;
                    successors.push(edges);
                }
                let boundaries = Boundaries::derive(ssa, ControlInput {
                    entry: Block::new(function.entry().index()), successors: &successors,
                }, out)?;
                let demands = ComponentDemandsV42::derive(slots, row.function, out)?;
                let analysis_storage = out.budget.storage().checked_sub(analysis_floor)
                    .ok_or(Resource::Accounting)?;
                let (mut ancestor, mut depth) = (instance, 0usize);
                loop {
                    out.budget.charge_work(2)?;
                    match plan.instance(root, ancestor, out)?.incoming {
                        Some((parent, _)) if parent < ancestor => {
                            ancestor = parent;
                            depth = add(depth, 1)?;
                        }
                        None if ancestor == 0 => break,
                        _ => return Err(mismatch()),
                    }
                }
                for block_index in 0..function.blocks().len() {
                    out.budget.charge_work(2)?;
                    let block = block(block_index)?;
                    let Some(live) = ssa.live_in(block) else { continue; };
                    let pc = add(row.blocks.start, block_index)?;
                    emit!(out, "spec fn invocation_expanded_live_values_{root}_{instance}_{block_index}_v213(source: InvocationSourceByteStateV36, target: MemoryStateV30, map: InvocationByteMapV36, execution_map: Map<MemoryExecutionReferenceV178, InvocationExecutionOriginV205>) -> bool {{ source.machine.valid && target.valid && source.machine.pc == {pc} && source.machine.frames.active.len() == {}", add(depth, 1)?);
                    let (mut ancestor, mut frame) = (instance, depth);
                    loop {
                        let owner = plan.instance(root, ancestor, out)?;
                        out.budget.charge_work(2)?;
                        emit!(out, " && source.machine.frames.active[{frame}].owner == {}", owner.function.index());
                        match owner.incoming {
                            Some((parent, _)) if parent < ancestor && frame > 0 => {
                                ancestor = parent;
                                frame -= 1;
                            }
                            None if ancestor == 0 && frame == 0 => break,
                            _ => return Err(mismatch()),
                        }
                    }
                    for &variable in live {
                        out.budget.charge_work(4)?;
                        let value = boundaries.value(block, variable, out)?;
                        let endpoint = relation.ssa_typed_endpoint_v36(root, instance, value, out.budget)?;
                        let local = variable.get() as usize;
                        let source_type = endpoint.source_type(out.budget)?;
                        if endpoint.source_function(out.budget)? != row.function
                            || endpoint.source_local(out.budget)?.index() != variable.get()
                            || function.locals().get(local).map(|local| local.ty()) != Some(source_type)
                        {
                            return Err(mismatch());
                        }
                        let nominal = semantic.types().get(source_type.index() as usize).ok_or_else(mismatch)?;
                        let role = match nominal.rust_type_kind() {
                            RustType::Execution(role) => Some(role),
                            _ => match nominal.shape() {
                                Shape::Pointer(pointer) => {
                                    match semantic.types().get(pointer.pointee().index() as usize)
                                        .ok_or_else(mismatch)?.rust_type_kind() {
                                        RustType::Execution(role) => Some(role),
                                        _ => None,
                                    }
                                }
                                _ => None,
                            },
                        };
                        if matches!(role, Some(Role::KernelContext | Role::Workgroup)) {
                            emit!(out, " && ");
                            execution.emit_mapped_conjunct_v205(root, instance, value, out)?;
                            continue;
                        }
                        let payload = matches!(role, Some(Role::MaskedTileU32 { .. } | Role::LaneFragmentU32 { .. }));
                        let carrier = endpoint.carrier_shape(out.budget)?;
                        if payload || matches!(carrier, Carrier::Aggregate { .. }) {
                            if payload {
                                emit!(out, " && ");
                                execution.emit_payload_lease_conjunct_v209(root, instance, value, out)?;
                            }
                            let count = slots.aggregate_leaf_count(source_type, out)?.ok_or_else(mismatch)?;
                            for leaf in 0..count {
                                out.budget.charge_work(1)?;
                                if demands.leaf_required(row.function, block_index, local, leaf, out)? {
                                    scalar.emit_source_leaf_conjunct(plan, root, instance, value, leaf, width, out)?;
                                }
                            }
                        } else if matches!(carrier, Carrier::Unit | Carrier::Value) {
                            scalar.emit_source_conjunct(plan, root, instance, value, width, out)?;
                        } else {
                            return Err(Error::Statement("expanded source cut requires a complete enum component relation"));
                        }
                    }
                    emit!(out, " }}\n");
                }
                // Only analysis allocations are refunded; emitted text remains
                // charged, and the retained source/target owners stay funded.
                drop(demands);
                drop(boundaries);
                drop(successors);
                out.budget.release_storage(analysis_storage)?;
            }
        }
        scalar.check_owner(slots, target, out)?;
        execution.check_owner(plan, slots, target, out)
    })
}
