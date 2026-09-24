//! Whole-root compiler MIR census, not an interpreter for authored body words.
use super::production_complete_body_call_vnext::{
    ActualCompleteBodyCallVNext, observe_complete_body_call, valid_root_signature,
};
use rustc_middle::mir::{
    Body, Operand, Rvalue, START_BLOCK, StatementKind, TerminatorKind, UnwindAction,
};
use rustc_middle::ty::{Instance, TyCtxt};
#[path = "production_complete_body_transport_vnext.rs"]
mod transport;
use transport::ArgumentTransport;

pub(crate) const MAX_SOURCE_BLOCKS: usize = 4096;
pub(crate) const MAX_SOURCE_LOCALS: usize = transport::MAX_LOCALS;
pub(crate) const MAX_SOURCE_ITEMS: usize = 65_536;

/// Counts a literal finite body census before hashing or traversal.
/// These bounds do not cover rustc query allocation/RSS.
pub(crate) fn require_body_bounds(body: &Body<'_>) -> Result<usize, &'static str> {
    if body.basic_blocks.is_empty()
        || body.basic_blocks.len() > MAX_SOURCE_BLOCKS
        || body.local_decls.len() > transport::MAX_LOCALS
    {
        return Err("complete body source body bound exceeded");
    }
    let mut items = body.local_decls.len();
    for block in body.basic_blocks.iter() {
        items = items
            .checked_add(block.statements.len())
            .and_then(|n| n.checked_add(1))
            .ok_or("complete body source item overflow")?;
        if items > MAX_SOURCE_ITEMS {
            return Err("complete body source item bound exceeded");
        }
    }
    Ok(items)
}

/// Requires the actual root to be only pure argument transport, one exact
/// trusted marker, and unit return. All MIR blocks must belong to its single
/// acyclic chain. No helper, hidden branch, memory effect or second marker.
pub(crate) fn observe_root<'tcx>(
    tcx: TyCtxt<'tcx>,
    root: Instance<'tcx>,
    body: &Body<'tcx>,
    expected_block: u32,
) -> Result<ActualCompleteBodyCallVNext<'tcx>, &'static str> {
    require_body_bounds(body)?;
    if body.arg_count != 5 || expected_block as usize >= body.basic_blocks.len() {
        return Err("complete body root arguments or selected block differ");
    }
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(root.def_id()).instantiate(tcx, root.args),
    );
    if !valid_root_signature(tcx, &signature) || !body.return_ty().is_unit() {
        return Err("complete body root signature differs");
    }
    for (index, ty) in signature.inputs().iter().enumerate() {
        if body
            .local_decls
            .get(rustc_middle::mir::Local::from_usize(index + 1))
            .is_none_or(|declaration| declaration.ty != *ty)
        {
            return Err("complete body root MIR argument type differs");
        }
    }
    let mut state = ArgumentTransport::new(body.local_decls.len())?;
    let mut visited = [false; MAX_SOURCE_BLOCKS];
    let mut next = START_BLOCK;
    let mut observed = None;
    loop {
        let index = next.index();
        let block = body
            .basic_blocks
            .get(next)
            .ok_or("complete body source edge leaves current body")?;
        if visited[index] || block.is_cleanup {
            return Err("complete body source chain cycles or enters cleanup");
        }
        visited[index] = true;
        for statement in &block.statements {
            match &statement.kind {
                StatementKind::Nop => {}
                StatementKind::StorageLive(local) | StatementKind::StorageDead(local) => {
                    state.storage(local.index())?;
                }
                StatementKind::Assign(assignment) => {
                    let (destination, value) = &**assignment;
                    if !destination.projection.is_empty() {
                        return Err("complete body source writes through a projection");
                    }
                    let Rvalue::Use(operand) = value else {
                        return Err("complete body source computes outside the authored body");
                    };
                    if destination.local.as_u32() == 0 {
                        if !matches!(operand, Operand::Constant(value) if value.const_.ty().is_unit())
                        {
                            return Err("complete body return assignment is not a unit constant");
                        }
                        continue;
                    }
                    let (source, moved) = match operand {
                        Operand::Copy(place) => (place, false),
                        Operand::Move(place) => (place, true),
                        _ => {
                            return Err(
                                "complete body source assignment is not argument transport",
                            );
                        }
                    };
                    if !source.projection.is_empty()
                        || destination.ty(&body.local_decls, tcx).ty
                            != source.ty(&body.local_decls, tcx).ty
                    {
                        return Err("complete body source transport changes type or projects");
                    }
                    state.assign(destination.local.index(), source.local.index(), moved)?;
                }
                _ => return Err("complete body source has an unadmitted statement"),
            }
        }
        next = match &block.terminator().kind {
            TerminatorKind::Goto { target } => *target,
            TerminatorKind::Call {
                func,
                args,
                destination,
                target: Some(target),
                unwind: UnwindAction::Continue | UnwindAction::Unreachable,
                ..
            } if next.as_u32() == expected_block && observed.is_none() => {
                if args.len() != 10
                    || !destination.projection.is_empty()
                    || !destination.ty(&body.local_decls, tcx).ty.is_unit()
                {
                    return Err("complete body marker call shape differs");
                }
                let actual = observe_complete_body_call(
                    tcx,
                    root,
                    body,
                    func,
                    [
                        &args[0].node,
                        &args[1].node,
                        &args[2].node,
                        &args[3].node,
                        &args[4].node,
                        &args[5].node,
                        &args[6].node,
                        &args[7].node,
                        &args[8].node,
                        &args[9].node,
                    ],
                )?;
                state.consume_marker(actual.argument_locals(), actual.moved_arguments())?;
                observed = Some(actual);
                *target
            }
            TerminatorKind::Return => {
                state.require_marker()?;
                if visited[..body.basic_blocks.len()]
                    .iter()
                    .any(|value| !value)
                {
                    return Err("complete body source contains blocks outside the accepted chain");
                }
                return observed.ok_or("complete body current marker observation missing");
            }
            _ => return Err("complete body source has another call, branch or effect"),
        };
    }
}
