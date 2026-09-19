//! Precharge the supplied, polymorphic once-shim payload before its stable hash.
//!
//! Pinned rustc's `build_call_shim` has one uninlined scope, boring locals, no
//! debug/user/coverage/coroutine payloads, and shallow `Self`/`Args` types. Reject
//! other payloads without walking them. This is not a general MIR/type hasher or
//! a bound on rustc queries: the exact Instance remains under existing custody.

use rustc_middle::mir::{
    Body, ClearCrossCrate, Const, ConstValue, LocalInfo, MentionedItem, OUTERMOST_SOURCE_SCOPE,
    Operand, Place, Rvalue, SourceInfo, StatementKind, TerminatorKind,
};
use rustc_middle::ty::{self, GenericArgsRef, InstanceKind, Ty, TyCtxt, TyKind};

use super::ReceiverReborrowErrorV1;
use ReceiverReborrowErrorV1::{Resource, Unsupported};

pub(in crate::production_semantic_body_v1) fn charge_once_shim_fingerprint_v1<'tcx, E>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    mut charge: impl FnMut(usize) -> Result<(), E>,
) -> Result<(), ReceiverReborrowErrorV1<E>> {
    charge(1).map_err(Resource)?;
    if !matches!(body.source.instance, InstanceKind::ClosureOnceShim { .. })
        || body.source.promoted.is_some()
        || body.coroutine.is_some()
        || !body.user_type_annotations.is_empty()
        || !body.var_debug_info.is_empty()
        || body
            .required_consts
            .as_ref()
            .is_some_and(|items| !items.is_empty())
        || body.injection_phase.is_some()
        || body.tainted_by_errors.is_some()
        || body.coverage_info_hi.is_some()
        || body.function_coverage_info.is_some()
    {
        return Err(Unsupported("closure once shim fingerprint metadata"));
    }
    charge(body.source_scopes.len()).map_err(Resource)?;
    if body.source_scopes.len() != 1 {
        return Err(Unsupported("closure once shim fingerprint source scopes"));
    }
    let scope = &body.source_scopes[OUTERMOST_SOURCE_SCOPE];
    if scope.parent_scope.is_some()
        || scope.inlined.is_some()
        || scope.inlined_parent_scope.is_some()
        || !matches!(scope.local_data, ClearCrossCrate::Clear)
    {
        return Err(Unsupported(
            "closure once shim fingerprint source scope metadata",
        ));
    }
    // Spans hash scalar positions, DefPathHash and ExpnHash, not expansion data.
    charge(2).map_err(Resource)?; // body.span and scope.span
    charge(body.local_decls.len()).map_err(Resource)?;
    for local in &body.local_decls {
        if local.user_ty.is_some()
            || !match &local.local_info {
                ClearCrossCrate::Clear => true,
                ClearCrossCrate::Set(info) => matches!(info.as_ref(), LocalInfo::Boring),
            }
        {
            return Err(Unsupported("closure once shim fingerprint local metadata"));
        }
        source_info(local.source_info, &mut charge)?;
        shallow_type(tcx, local.ty, &mut charge)?;
    }
    if let Some(items) = &body.mentioned_items {
        charge(items.len()).map_err(Resource)?;
        for item in items {
            charge(1).map_err(Resource)?; // item.span
            let ty = match item.node {
                MentionedItem::Fn(ty) | MentionedItem::Drop(ty) => ty,
                _ => return Err(Unsupported("closure once shim fingerprint mentioned item")),
            };
            shallow_type(tcx, ty, &mut charge)?;
        }
    }
    charge(body.basic_blocks.len()).map_err(Resource)?;
    for block in body.basic_blocks.iter() {
        if !block.after_last_stmt_debuginfos.is_empty() {
            return Err(Unsupported(
                "closure once shim fingerprint statement debug metadata",
            ));
        }
        charge(block.statements.len()).map_err(Resource)?;
        for statement in &block.statements {
            if !statement.debuginfos.is_empty() {
                return Err(Unsupported(
                    "closure once shim fingerprint statement debug metadata",
                ));
            }
            source_info(statement.source_info, &mut charge)?;
            match &statement.kind {
                StatementKind::Assign(assignment) => {
                    plain_place(assignment.0, body, &mut charge)?;
                    let Rvalue::Ref(region, _, place) = &assignment.1 else {
                        return Err(Unsupported("closure once shim fingerprint rvalue"));
                    };
                    if *region != tcx.lifetimes.re_erased {
                        return Err(Unsupported("closure once shim fingerprint borrow region"));
                    }
                    plain_place(*place, body, &mut charge)?;
                }
                StatementKind::StorageLive(local) | StatementKind::StorageDead(local) => {
                    plain_place(Place::from(*local), body, &mut charge)?;
                }
                StatementKind::Nop => {}
                _ => return Err(Unsupported("closure once shim fingerprint statement")),
            }
        }
        charge(1).map_err(Resource)?;
        let terminator = block.terminator.as_ref().ok_or(Unsupported(
            "closure once shim fingerprint missing terminator",
        ))?;
        source_info(terminator.source_info, &mut charge)?;
        match &terminator.kind {
            TerminatorKind::Call {
                func,
                args,
                destination,
                fn_span: _,
                ..
            } => {
                charge(1).map_err(Resource)?; // fn_span
                operand(tcx, func, body, &mut charge)?;
                plain_place(*destination, body, &mut charge)?;
                charge(args.len()).map_err(Resource)?;
                for arg in args.iter() {
                    charge(1).map_err(Resource)?; // arg.span
                    operand(tcx, &arg.node, body, &mut charge)?;
                }
            }
            TerminatorKind::Drop {
                place,
                drop,
                async_fut,
                ..
            } => {
                if drop.is_some() || async_fut.is_some() {
                    return Err(Unsupported("closure once shim fingerprint async drop"));
                }
                plain_place(*place, body, &mut charge)?;
            }
            TerminatorKind::Goto { .. }
            | TerminatorKind::Return
            | TerminatorKind::Unreachable
            | TerminatorKind::UnwindResume
            | TerminatorKind::UnwindTerminate(_) => {}
            _ => return Err(Unsupported("closure once shim fingerprint terminator")),
        }
    }
    Ok(())
}

fn source_info<E>(
    info: SourceInfo,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<(), ReceiverReborrowErrorV1<E>> {
    charge(1).map_err(Resource)?;
    if info.scope != OUTERMOST_SOURCE_SCOPE {
        return Err(Unsupported(
            "closure once shim fingerprint source scope index",
        ));
    }
    Ok(())
}

fn plain_place<E>(
    place: Place<'_>,
    body: &Body<'_>,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<(), ReceiverReborrowErrorV1<E>> {
    charge(1).map_err(Resource)?;
    if !place.projection.is_empty() || place.local.index() >= body.local_decls.len() {
        return Err(Unsupported("closure once shim fingerprint place"));
    }
    Ok(())
}

fn operand<'tcx, E>(
    tcx: TyCtxt<'tcx>,
    operand: &Operand<'tcx>,
    body: &Body<'tcx>,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<(), ReceiverReborrowErrorV1<E>> {
    charge(1).map_err(Resource)?;
    match operand {
        Operand::Copy(place) | Operand::Move(place) => plain_place(*place, body, charge),
        Operand::Constant(constant) => {
            if constant.user_ty.is_some() {
                return Err(Unsupported(
                    "closure once shim fingerprint constant user type",
                ));
            }
            let Const::Val(ConstValue::ZeroSized, ty) = constant.const_ else {
                return Err(Unsupported("closure once shim fingerprint constant"));
            };
            if !matches!(ty.kind(), TyKind::FnDef(..)) {
                return Err(Unsupported(
                    "closure once shim fingerprint function constant",
                ));
            }
            charge(1).map_err(Resource)?; // constant.span
            shallow_type(tcx, ty, charge)
        }
        _ => Err(Unsupported("closure once shim fingerprint operand")),
    }
}

fn shallow_type<'tcx, E>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<(), ReceiverReborrowErrorV1<E>> {
    charge(1).map_err(Resource)?;
    match ty.kind() {
        TyKind::Param(_) => parameter(ty, charge),
        TyKind::Ref(region, pointee, _) if *region == tcx.lifetimes.re_erased => {
            parameter(*pointee, charge)
        }
        TyKind::FnDef(_, args) => parameters(args, charge),
        TyKind::Alias(ty::Projection, alias) => parameters(alias.args, charge),
        _ => Err(Unsupported("closure once shim fingerprint nested type")),
    }
}

fn parameters<E>(
    args: GenericArgsRef<'_>,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<(), ReceiverReborrowErrorV1<E>> {
    charge(args.len()).map_err(Resource)?;
    if args.len() != 2 {
        return Err(Unsupported(
            "closure once shim fingerprint generic arguments",
        ));
    }
    for arg in args.iter() {
        let ty = arg.as_type().ok_or(Unsupported(
            "closure once shim fingerprint non-type argument",
        ))?;
        parameter(ty, charge)?;
    }
    Ok(())
}

fn parameter<E>(
    ty: Ty<'_>,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<(), ReceiverReborrowErrorV1<E>> {
    charge(1).map_err(Resource)?;
    let TyKind::Param(param) = ty.kind() else {
        return Err(Unsupported(
            "closure once shim fingerprint nested parameter",
        ));
    };
    if param.index > 1 {
        return Err(Unsupported("closure once shim fingerprint parameter index"));
    }
    charge(param.name.as_str().len()).map_err(Resource)
}
