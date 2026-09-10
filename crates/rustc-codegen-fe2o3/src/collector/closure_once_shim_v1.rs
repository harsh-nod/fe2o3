//! Authenticate rustc's by-value closure-to-FnMut adapter without making it a terminal.

use rustc_abi::ExternAbi;
use rustc_hir::{Safety, def::DefKind, def_id::DefId};
use rustc_middle::mir::{
    BasicBlock, Body, BorrowKind, Local, MutBorrowKind, Operand, Place, Rvalue, StatementKind,
    TerminatorKind, UnwindAction, UnwindTerminateReason,
};
use rustc_middle::ty::{
    ClosureKind, EarlyBinder, FnSig, Instance, InstanceKind, Ty, TyCtxt, TyKind, TypeVisitableExt,
    TypingEnv,
};
use rustc_target::spec::PanicStrategy;

const MAX_TUPLE_FIELDS: usize = 16;

struct ShimContract<'tcx> {
    environment: Ty<'tcx>,
    tuple: Ty<'tcx>,
    output: Ty<'tcx>,
    closure: Instance<'tcx>,
    call_mut: DefId,
}

/// This authenticates generated MIR, not a declaration body or a source-safety
/// waiver for the closure. Normal recursive collection must still visit it.
pub(crate) fn authenticate_closure_once_shim_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> bool {
    authenticated_contract(tcx, instance).is_some()
}

/// The semantic producer independently checks the supplied body, not only the
/// body cached by rustc, before inserting an explicit shared reborrow.
pub(crate) fn authenticate_closure_once_shim_body_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
) -> bool {
    shim_identity(tcx, instance)
        .is_some_and(|contract| reviewed_body(tcx, instance, body, &contract))
}

/// Only the exact by-value receiver of an authenticated generated adapter is
/// an invocation receiver. Other same-typed locals retain ordinary custody rules.
pub(crate) fn authenticated_closure_once_receiver_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    local: Local,
    ty: Ty<'tcx>,
) -> bool {
    local.as_usize() == 1
        && authenticated_contract(tcx, instance).is_some_and(|contract| contract.environment == ty)
}

fn authenticated_contract<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Option<ShimContract<'tcx>> {
    let contract = shim_identity(tcx, instance)?;
    reviewed_body(tcx, instance, tcx.instance_mir(instance.def), &contract).then_some(contract)
}

fn trait_method(tcx: TyCtxt<'_>, trait_id: DefId, name: &str) -> Option<DefId> {
    if tcx.lang_items().sized_trait()?.krate != trait_id.krate {
        return None;
    }
    let mut methods = tcx
        .associated_items(trait_id)
        .in_definition_order()
        .filter(|item| item.is_fn());
    let method = methods.next()?;
    (methods.next().is_none()
        && tcx.trait_of_assoc(method.def_id) == Some(trait_id)
        && tcx.item_name(method.def_id).as_str() == name)
        .then_some(method.def_id)
}

fn shim_identity<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> Option<ShimContract<'tcx>> {
    let InstanceKind::ClosureOnceShim {
        call_once,
        track_caller: false,
    } = instance.def
    else {
        return None;
    };
    if call_once != trait_method(tcx, tcx.lang_items().fn_once_trait()?, "call_once")?
        || instance.args.len() != 2
        || instance.args.has_non_region_param()
        || instance.args.has_escaping_bound_vars()
    {
        return None;
    }
    let environment = instance.args[0].as_type()?;
    let tuple = instance.args[1].as_type()?;
    let TyKind::Closure(definition, arguments) = *environment.kind() else {
        return None;
    };
    let TyKind::Tuple(fields) = tuple.kind() else {
        return None;
    };
    let typing_env = TypingEnv::fully_monomorphized();
    if tcx.def_kind(definition) != DefKind::Closure
        || !tcx.is_mir_available(definition)
        || fields.len() > MAX_TUPLE_FIELDS
        || environment.needs_drop(tcx, typing_env)
        || !matches!(
            arguments.as_closure().kind_ty().to_opt_closure_kind(),
            Some(ClosureKind::Fn | ClosureKind::FnMut)
        )
    {
        return None;
    }
    let closure_signature = tcx
        .try_normalize_erasing_regions(
            typing_env,
            tcx.instantiate_bound_regions_with_erased(arguments.as_closure().sig()),
        )
        .ok()?;
    if closure_signature.safety != Safety::Safe
        || closure_signature.abi != ExternAbi::RustCall
        || closure_signature.c_variadic
        || closure_signature.inputs() != [tuple]
    {
        return None;
    }
    let contract = ShimContract {
        environment,
        tuple,
        output: closure_signature.output(),
        closure: Instance::new_raw(definition, arguments),
        call_mut: trait_method(tcx, tcx.lang_items().fn_mut_trait()?, "call_mut")?,
    };
    let signature = tcx
        .try_normalize_erasing_regions(
            typing_env,
            tcx.instantiate_bound_regions_with_erased(
                tcx.fn_sig(call_once).instantiate(tcx, instance.args),
            ),
        )
        .ok()?;
    if !signature_matches(signature, &contract)
        || Instance::try_resolve(tcx, typing_env, call_once, instance.args).ok()?? != instance
    {
        return None;
    }
    Some(contract)
}

fn signature_matches<'tcx>(signature: FnSig<'tcx>, contract: &ShimContract<'tcx>) -> bool {
    signature.safety == Safety::Safe
        && signature.abi == ExternAbi::RustCall
        && !signature.c_variadic
        && signature.inputs() == [contract.environment, contract.tuple]
        && signature.output() == contract.output
}

fn normalized_ty<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    ty: Ty<'tcx>,
) -> Option<Ty<'tcx>> {
    instance
        .try_instantiate_mir_and_normalize_erasing_regions(
            tcx,
            TypingEnv::fully_monomorphized(),
            EarlyBinder::bind(ty),
        )
        .ok()
}

fn local(index: usize) -> Local {
    Local::from_usize(index)
}

fn block(index: usize) -> BasicBlock {
    BasicBlock::from_usize(index)
}

fn moves_local(operand: &Operand<'_>, index: usize) -> bool {
    matches!(operand, Operand::Move(place) if *place == Place::from(local(index)))
}

fn trivial_receiver_drop(kind: &TerminatorKind<'_>, target: usize, unwind: UnwindAction) -> bool {
    matches!(kind, TerminatorKind::Drop {
        place, target: actual_target, unwind: actual_unwind,
        replace: false, drop: None, async_fut: None,
    } if *place == Place::from(local(1)) && *actual_target == block(target) && *actual_unwind == unwind)
}

fn reviewed_body<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    contract: &ShimContract<'tcx>,
) -> bool {
    // rustc emits exactly one borrow and one forwarding call, with a normal
    // trivial drop and, under unwinding, a cleanup trivial drop plus resume.
    // Checking every block also rejects hidden effects in unreachable blocks.
    let abort = tcx.sess.panic_strategy() == PanicStrategy::Abort;
    if body.source.instance != instance.def
        || body.source.promoted.is_some()
        || body.arg_count != 2
        || body.spread_arg != Some(local(2))
        || body.local_decls.len() != 4
        || body.basic_blocks.len() != if abort { 3 } else { 5 }
        || body.source_scopes.len() > 4
        || body.coroutine.is_some()
    {
        return false;
    }
    let receiver_ref = Ty::new_mut_ref(tcx, tcx.lifetimes.re_erased, contract.environment);
    for (index, expected) in [
        contract.output,
        contract.environment,
        contract.tuple,
        receiver_ref,
    ]
    .into_iter()
    .enumerate()
    {
        if normalized_ty(tcx, instance, body.local_decls[local(index)].ty) != Some(expected) {
            return false;
        }
    }
    for (index, data) in body.basic_blocks.iter_enumerated() {
        if data.is_cleanup != (index.as_usize() >= 3)
            || data.terminator.is_none()
            || (index.as_usize() != 0 && !data.statements.is_empty())
        {
            return false;
        }
    }
    let entry = &body.basic_blocks[block(0)];
    let [statement] = entry.statements.as_slice() else {
        return false;
    };
    if !matches!(&statement.kind, StatementKind::Assign(assignment)
        if assignment.0 == Place::from(local(3))
        && matches!(assignment.1, Rvalue::Ref(_, BorrowKind::Mut { kind: MutBorrowKind::Default }, place)
            if place == Place::from(local(1))))
    {
        return false;
    }
    let TerminatorKind::Call {
        func,
        args,
        destination,
        target,
        unwind,
        ..
    } = &entry.terminator().kind
    else {
        return false;
    };
    if args.len() != 2
        || !moves_local(&args[0].node, 3)
        || !moves_local(&args[1].node, 2)
        || *destination != Place::from(local(0))
        || *target != Some(block(1))
        || *unwind
            != if abort {
                UnwindAction::Unreachable
            } else {
                UnwindAction::Cleanup(block(3))
            }
    {
        return false;
    }
    let Operand::Constant(callee) = func else {
        return false;
    };
    let Some(callee_ty) = normalized_ty(tcx, instance, callee.const_.ty()) else {
        return false;
    };
    let TyKind::FnDef(definition, arguments) = *callee_ty.kind() else {
        return false;
    };
    if definition != contract.call_mut
        || arguments != instance.args
        || Instance::try_resolve(tcx, TypingEnv::fully_monomorphized(), definition, arguments)
            .ok()
            .flatten()
            != Some(contract.closure)
    {
        return false;
    }
    let normal_unwind = if abort {
        UnwindAction::Unreachable
    } else {
        UnwindAction::Continue
    };
    trivial_receiver_drop(
        &body.basic_blocks[block(1)].terminator().kind,
        2,
        normal_unwind,
    ) && matches!(
        body.basic_blocks[block(2)].terminator().kind,
        TerminatorKind::Return
    ) && (abort
        || (trivial_receiver_drop(
            &body.basic_blocks[block(3)].terminator().kind,
            4,
            UnwindAction::Terminate(UnwindTerminateReason::InCleanup),
        ) && matches!(
            body.basic_blocks[block(4)].terminator().kind,
            TerminatorKind::UnwindResume
        )))
}

#[cfg(test)]
#[path = "closure_once_shim_v1/tests.rs"]
pub(crate) mod tests;
