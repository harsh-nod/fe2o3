//! Necessary expanded current/suspended-frame contracts from one retained plan.
//! No initialization, step, effect-history, progress or finite-trace law follows
//! from emitting these predicates, even when every target locator exists.
use super::super::invocations::InvocationPlan;
use super::{
    Error, Resource, Result, Writer,
    expanded_execution::ExpandedExecutionBindingsV199,
    forwarding_observation::Observer,
    paired::ExpandedScalarBindingsV196,
    slots::SourceSlots,
    source_frame_plan::FramePlan,
    tile_target::{TileMicroCutsV180, TileTargetV176},
};
use fe2o3_kernel_ir::FormalIndexWidth;
use fe2o3_lower_mir_kernel::ProductionSourceSsaCarrierShapeV37 as Carrier;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticExecutionRoleV29 as Role, SemanticRustTypeKindV1 as RustType,
    SemanticTypeShapeV1 as Shape,
};
use std::{
    fmt::Write as _,
    mem::{align_of_val, size_of, size_of_val},
    ops::Range,
};

#[cfg(test)]
#[path = "original_semantic_mir_expanded_frame_contract_v281_tests.rs"]
mod tests;

macro_rules! emit {
    ($out:expr, $($arg:tt)*) => { write!($out, $($arg)*).map_err(|_| $out.error())? };
}

fn mismatch() -> Error {
    Error::Statement("expanded frame contract differs from its original demand owner or cuts")
}

fn headers() -> usize {
    Error::frame_binding_headers_v284()
        + super::forwarding_observation::headers()
        + 4 * size_of::<Range<usize>>()
        + 36 * size_of::<usize>()
        + 24 * size_of::<&()>()
        + 3 * size_of::<Result<()>>()
        + size_of::<Carrier>()
        + size_of::<Option<Role>>()
        + size_of::<fe2o3_lower_mir_kernel::ProductionSourceSsaEndpointV36<'_, '_>>()
        + size_of::<Result<fe2o3_lower_mir_kernel::ProductionSourceSsaEndpointV36<'_, '_>>>()
        + size_of::<ExpandedScalarBindingsV196<'_, '_, '_, '_>>()
        + size_of::<ExpandedExecutionBindingsV199<'_, '_, '_, '_, '_>>()
        + size_of::<super::tile_target::StaticCutsV280<'_>>()
        + size_of::<std::slice::Iter<'_, super::source_frame_plan::Cut>>()
        + size_of::<std::slice::Iter<'_, super::source_frame_plan::Frame>>()
        + size_of::<std::slice::Iter<'_, super::source_frame_plan::Call>>()
}

pub(super) fn emit_carry(
    frame: &super::source_frame_plan::Frame,
    call: &super::source_frame_plan::Call,
    out: &mut Writer<'_, '_>,
    emit_demands: impl FnOnce(&mut Writer<'_, '_>) -> Result<()>,
) -> Result<()> {
    let headers = (2 * size_of_val(&emit_demands))
        .checked_add(align_of_val(&emit_demands))
        .and_then(|n| n.checked_add(4 * size_of::<&()>() + 3 * size_of::<usize>()))
        .ok_or(Resource::Arithmetic)?;
    out.budget.reserve_storage(headers)?;
    out.budget.charge_work(5)?;
    if (call.root, call.caller) != (frame.root, frame.instance)
        || (call.child_active && call.child.is_none())
        || (!call.child_active && !call.demands.is_empty())
    {
        return Err(mismatch());
    }
    emit!(
        out,
        "spec fn invocation_expanded_carry_{}_{}_{}_v281(source: InvocationSourceByteStateV36, target: MemoryStateV30, map: InvocationByteMapV36, execution_map: Map<MemoryExecutionReferenceV178, InvocationExecutionOriginV205>) -> bool {{ ",
        call.root,
        call.caller,
        call.block
    );
    if frame.active && call.reachable && call.child_active {
        emit!(out, "source.machine.valid && target.valid");
        emit_demands(out)?;
    } else {
        emit!(out, "false");
    }
    emit!(out, " }}\n");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn demands(
    frames: &FramePlan<'_, '_, '_, '_>,
    plan: &InvocationPlan<'_, '_>,
    slots: &SourceSlots<'_, '_>,
    scalar: &ExpandedScalarBindingsV196<'_, '_, '_, '_>,
    execution: &ExpandedExecutionBindingsV199<'_, '_, '_, '_, '_>,
    frame: usize,
    range: Range<usize>,
    width: FormalIndexWidth,
    out: &mut Writer<'_, '_>,
    observe: &mut Observer<'_>,
) -> Result<()> {
    let owner = frames.frames.get(frame).ok_or_else(mismatch)?;
    let relation = slots.correspondence(out)?;
    let semantic = relation.source(out.budget)?.source_semantic(out.budget)?;
    let function = semantic
        .functions()
        .get(owner.function.index() as usize)
        .ok_or_else(mismatch)?;
    for index in range {
        out.budget.charge_work(6)?;
        let demand = frames.demands.get(index).ok_or_else(mismatch)?;
        if demand.frame != frame {
            return Err(mismatch());
        }
        let demand = &demand.source;
        let site = [
            owner.root,
            owner.instance,
            owner.function.index() as usize,
            demand.local,
        ];
        let endpoint = relation.ssa_typed_endpoint_v36(
            owner.root,
            owner.instance,
            demand.value,
            out.budget,
        )?;
        let ty = endpoint.source_type(out.budget)?;
        if endpoint.source_function(out.budget)? != owner.function
            || endpoint.source_local(out.budget)?.index() as usize != demand.local
            || function.locals().get(demand.local).map(|row| row.ty()) != Some(ty)
        {
            return Err(mismatch());
        }
        if slots.is_product_v282(ty, out)? {
            let count = slots
                .product_component_count_v282(ty, out)?
                .ok_or_else(mismatch)?;
            if count == 0 || !slots.product_type_supported_v282(ty, out)? {
                return Err(mismatch());
            }
            for atom in 0..count {
                out.budget.charge_work(1)?;
                if frames.leaf_required(frame, index, atom, out)? {
                    scalar
                        .emit_source_product_conjunct_observed_v288(
                            plan,
                            owner.root,
                            owner.instance,
                            demand.value,
                            atom,
                            width,
                            out,
                            observe,
                        )
                        .map_err(|error| {
                            error.at_frame_binding_v284(
                                site,
                                demand.value,
                                Some(atom),
                                "product",
                                "source-component",
                            )
                        })?;
                }
            }
            continue;
        }
        let nominal = semantic
            .types()
            .get(ty.index() as usize)
            .ok_or_else(mismatch)?;
        let role = match nominal.rust_type_kind() {
            RustType::Execution(role) => Some(role),
            _ => match nominal.shape() {
                Shape::Pointer(pointer) => match semantic
                    .types()
                    .get(pointer.pointee().index() as usize)
                    .ok_or_else(mismatch)?
                    .rust_type_kind()
                {
                    RustType::Execution(role) => Some(role),
                    _ => None,
                },
                _ => None,
            },
        };
        if matches!(role, Some(Role::KernelContext | Role::Workgroup)) {
            emit!(out, " && ");
            execution
                .emit_mapped_conjunct_v205(owner.root, owner.instance, demand.value, out)
                .map_err(|error| {
                    error.at_frame_binding_v284(site, demand.value, None, "execution", "source-map")
                })?;
            continue;
        }
        let payload = matches!(
            role,
            Some(Role::MaskedTileU32 { .. } | Role::LaneFragmentU32 { .. })
        );
        let carrier = endpoint.carrier_shape(out.budget)?;
        if payload || matches!(carrier, Carrier::Aggregate { .. }) {
            if payload {
                emit!(out, " && ");
                execution
                    .emit_payload_lease_conjunct_v209(owner.root, owner.instance, demand.value, out)
                    .map_err(|error| {
                        error.at_frame_binding_v284(
                            site,
                            demand.value,
                            None,
                            "execution-payload",
                            "source-lease",
                        )
                    })?;
            }
            let count = slots.aggregate_leaf_count(ty, out)?.ok_or_else(mismatch)?;
            for leaf in 0..count {
                out.budget.charge_work(1)?;
                if frames.leaf_required(frame, index, leaf, out)? {
                    scalar
                        .emit_source_leaf_conjunct(
                            plan,
                            owner.root,
                            owner.instance,
                            demand.value,
                            leaf,
                            width,
                            out,
                        )
                        .map_err(|error| {
                            error.at_frame_binding_v284(
                                site,
                                demand.value,
                                Some(leaf),
                                "aggregate",
                                "source-leaf",
                            )
                        })?;
                }
            }
        } else if matches!(carrier, Carrier::Unit | Carrier::Value) {
            scalar
                .emit_source_conjunct(plan, owner.root, owner.instance, demand.value, width, out)
                .map_err(|error| {
                    error.at_frame_binding_v284(
                        site,
                        demand.value,
                        None,
                        "scalar",
                        "source-endpoint",
                    )
                })?;
        } else {
            return Err(Error::Statement(
                "expanded frame contract requires a complete enum component relation",
            ));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn emit(
    frames: &FramePlan<'_, '_, '_, '_>,
    plan: &InvocationPlan<'_, '_>,
    slots: &SourceSlots<'_, '_>,
    target: &TileTargetV176<'_, '_, '_>,
    cuts: &TileMicroCutsV180<'_, '_, '_, '_>,
    width: FormalIndexWidth,
    out: &mut Writer<'_, '_>,
    observe: &mut Observer<'_>,
) -> Result<()> {
    let emit = |out: &mut Writer<'_, '_>| {
        frames.check(plan, slots, out)?;
        cuts.check_target_v280(target, out)?;
        out.budget.charge_work(2)?;
        if width == FormalIndexWidth::Unknown || !std::ptr::eq(slots, target.source_slots(out)?) {
            return Err(mismatch());
        }
        let rows = cuts.static_rows_v280(out)?;
        if rows.cuts.len() != frames.cuts.len() || rows.roots.len() != frames.roots.len() {
            return Err(mismatch());
        }
        let scalar = ExpandedScalarBindingsV196::derive(slots, target, out)?;
        let execution = ExpandedExecutionBindingsV199::derive(plan, slots, target, out)?;
        for (index, frame) in frames.frames.iter().enumerate() {
            for at in frame.calls.clone() {
                let call = &frames.calls[at];
                emit_carry(frame, call, out, |out| {
                    demands(
                        frames,
                        plan,
                        slots,
                        &scalar,
                        &execution,
                        index,
                        call.demands.clone(),
                        width,
                        out,
                        observe,
                    )
                })?;
            }
            // One reference to the parent's contract shares every ancestor carry.
            emit!(
                out,
                "spec fn invocation_expanded_ancestors_{}_{}_v281(source: InvocationSourceByteStateV36, target: MemoryStateV30, map: InvocationByteMapV36, execution_map: Map<MemoryExecutionReferenceV178, InvocationExecutionOriginV205>) -> bool {{ ",
                frame.root,
                frame.instance
            );
            if frame.active {
                emit!(
                    out,
                    "{} < source.machine.frames.active.len() && source.machine.frames.active[{}].owner == {}",
                    frame.depth,
                    frame.depth,
                    frame.function.index()
                );
                if let Some(at) = frame.parent_call {
                    let call = &frames.calls[at];
                    emit!(
                        out,
                        " && invocation_expanded_ancestors_{}_{}_v281(source, target, map, execution_map) && invocation_expanded_carry_{}_{}_{}_v281(source, target, map, execution_map)",
                        call.root,
                        call.caller,
                        call.root,
                        call.caller,
                        call.block
                    );
                }
            } else {
                emit!(out, "false");
            }
            emit!(out, " }}\n");
            for at in frame.cuts.clone() {
                let cut = &frames.cuts[at];
                let actual = &rows.cuts[at];
                out.budget.charge_work(4)?;
                if !rows.roots[cut.root].contains(&at)
                    || (actual.instance, actual.block.index() as usize) != (cut.instance, cut.block)
                    || cut.pc != at
                {
                    return Err(mismatch());
                }
                emit!(
                    out,
                    "spec fn invocation_expanded_current_{}_{}_{}_v281(source: InvocationSourceByteStateV36, target: MemoryStateV30, map: InvocationByteMapV36, execution_map: Map<MemoryExecutionReferenceV178, InvocationExecutionOriginV205>) -> bool {{ ",
                    cut.root,
                    cut.instance,
                    cut.block
                );
                if cut.reachable {
                    emit!(out, "source.machine.valid && target.valid");
                    demands(
                        frames,
                        plan,
                        slots,
                        &scalar,
                        &execution,
                        index,
                        cut.demands.clone(),
                        width,
                        out,
                        observe,
                    )?;
                } else {
                    emit!(out, "false");
                }
                emit!(out, " }}\n");
            }
        }
        let locals = frames.frames.last().map_or(0, |frame| frame.locals.end);
        let definitions = target.inventory(out)?.definitions().len();
        for (root, range) in frames.roots.iter().enumerate() {
            emit!(
                out,
                "spec fn invocation_expanded_frame_contract_{root}_v281(source: InvocationSourceByteStateV36, micro: MemoryMicroStateV30, map: InvocationByteMapV36, execution_map: Map<MemoryExecutionReferenceV178, InvocationExecutionOriginV205>, target_prefix: Seq<MemoryOperationObservationV30>) -> bool {{ source.machine.valid && micro.state.valid && invocation_source_byte_state_well_formed_v36(source) && source.machine.values.len() == {locals} && micro.state.values.len() == {definitions} && micro.observations == target_prefix && invocation_tile_cursor_{root}_v180(source.machine.pc, micro) && invocation_execution_map_current_v205(source, micro.state, execution_map) && (match source.machine.frames.execution {{ Some(execution) => invocation_runtime_execution_{root}_v37(execution), None => false }}) && ({{ let target = micro.state;\n"
            );
            for frame in &frames.frames[range.clone()] {
                for cut in &frames.cuts[frame.cuts.clone()] {
                    out.budget.charge_work(1)?;
                    let depth = frame.depth.checked_add(1).ok_or(Resource::Arithmetic)?;
                    emit!(
                        out,
                        " if source.machine.pc == {} {{ source.machine.frames.active.len() == {depth} && invocation_expanded_ancestors_{root}_{}_v281(source, target, map, execution_map) && invocation_expanded_current_{root}_{}_{}_v281(source, target, map, execution_map) }} else\n",
                        cut.pc,
                        frame.instance,
                        frame.instance,
                        cut.block
                    );
                }
            }
            emit!(out, " false }}) }}\n");
        }
        scalar.check_owner(slots, target, out)?;
        execution.check_owner(plan, slots, target, out)?;
        frames.check(plan, slots, out)
    };
    let headers = headers()
        .checked_add(2 * size_of_val(&emit))
        .and_then(|n| n.checked_add(align_of_val(&emit)))
        .ok_or(Resource::Arithmetic)?;
    out.budget.reserve_storage(headers)?;
    slots.with_source_query_v42(out, emit)
}
