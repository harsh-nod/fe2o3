//! Authenticate rustc's owned-receiver adapter, never a name-based synthetic call.

use super::*;
use rustc_middle::mir::{BasicBlock, BorrowKind, MutBorrowKind};

pub(crate) fn authenticate_once_shim_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Result<Option<Instance<'tcx>>, ClosureProfileErrorV1> {
    let InstanceKind::ClosureOnceShim { track_caller, .. } = instance.def else {
        return Ok(None);
    };
    if track_caller || instance.args.len() != 2 {
        return Err(ClosureProfileErrorV1::new(
            "unsupported closure once shim ABI",
        ));
    }
    let Some(self_ty) = instance.args[0].as_type() else {
        return Err(ClosureProfileErrorV1::new(
            "closure once shim has no concrete Self",
        ));
    };
    let TyKind::Closure(definition, args) = self_ty.kind() else {
        return Err(ClosureProfileErrorV1::new(
            "closure once shim Self is not a closure",
        ));
    };
    if !matches!(
        args.as_closure().kind_ty().to_opt_closure_kind(),
        Some(ClosureKind::Fn | ClosureKind::FnMut)
    ) || Instance::resolve_closure(tcx, *definition, args, ClosureKind::FnOnce) != instance
    {
        return Err(ClosureProfileErrorV1::new(
            "closure once shim identity changed",
        ));
    }
    Ok(Some(Instance::new_raw(*definition, args)))
}

/// The shim's receiver is executing its own environment, not forwarding a new
/// callable value. Check its exact FnMut dispatch and borrow of owned `_1`.
pub(crate) fn is_shim_receiver_call_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    caller: Instance<'tcx>,
    block: BasicBlock,
    work: &mut SourceClosureWorkV1,
) -> Result<bool, ClosureProfileErrorV1> {
    charge_work(work, 1)?;
    let Some(closure_body) = authenticate_once_shim_v1(tcx, caller)? else {
        return Ok(false);
    };
    let body = tcx.instance_mir(caller.def);
    let data = body
        .basic_blocks
        .get(block)
        .ok_or_else(|| ClosureProfileErrorV1::new("closure shim call block is absent"))?;
    let TerminatorKind::Call { func, args, .. } = &data.terminator().kind else {
        return Ok(false);
    };
    if args.len() != 2
        || declared_call_kind(tcx, func)? != ClosureCallKindV1::FnMut
        || resolve_direct_call(tcx, caller, func)? != closure_body
    {
        return Ok(false);
    }
    let receiver = match &args[0].node {
        Operand::Copy(place) | Operand::Move(place) => place.as_local(),
        Operand::Constant(_) | Operand::RuntimeChecks(_) => None,
    };
    let Some(receiver) = receiver else {
        return Ok(false);
    };
    let receiver_ty = normalized_ty(tcx, caller, args[0].node.ty(body, tcx), "shim receiver")?;
    let tuple_ty = normalized_ty(tcx, caller, args[1].node.ty(body, tcx), "shim tuple")?;
    if !matches!(receiver_ty.kind(), TyKind::Ref(_, pointee, Mutability::Mut)
        if *pointee == caller.args.type_at(0))
        || tuple_ty != caller.args.type_at(1)
    {
        return Ok(false);
    }
    let mut borrow = false;
    for statement in &data.statements {
        charge_work(work, 1)?;
        if let Some((destination, value)) = statement.kind.as_assign()
            && destination.local == receiver
        {
            if borrow
                || destination.as_local() != Some(receiver)
                || !matches!(value, Rvalue::Ref(_, BorrowKind::Mut { kind: MutBorrowKind::Default }, place)
                    if place.as_local() == Some(Local::from_usize(1)))
            {
                return Ok(false);
            }
            borrow = true;
        }
    }
    Ok(borrow)
}
