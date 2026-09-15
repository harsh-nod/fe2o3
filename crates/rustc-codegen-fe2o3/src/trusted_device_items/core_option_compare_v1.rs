//! Source observation only for core Option equality and its default negation.
//! Retain both bodies and recursively check the exact resolved payload `eq`.
//! This does not authenticate arbitrary PartialEq implementations or their laws.

use rustc_abi::ExternAbi;
use rustc_hir::{Safety, def_id::DefId};
use rustc_middle::{
    mir::{
        BasicBlock, BinOp, Body, BorrowKind, Const, ConstValue, Local, MentionedItem, Operand,
        Place, ProjectionElem, Rvalue, StatementKind, SwitchTargets, TerminatorKind, UnOp,
        UnwindAction,
    },
    ty::{EarlyBinder, Instance, InstanceKind, Ty, TyCtxt, TyKind, TypeVisitableExt, TypingEnv},
};
use rustc_span::Symbol;

#[path = "core_option_compare_v1/encoded.rs"]
mod encoded;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Operation {
    Ne,
    Eq,
}

struct Contract<'tcx> {
    operation: Operation,
    option: Ty<'tcx>,
    payload: Ty<'tcx>,
    some: rustc_abi::VariantIdx,
    none_discriminant: u128,
    some_discriminant: u128,
    eq_item: DefId,
    call_ty: Ty<'tcx>,
    callee: Instance<'tcx>,
}

/// No terminalization: the caller must preserve the original call graph.
pub(crate) fn authenticate_reviewed_safe_core_option_compare_helper_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> bool {
    let Some(contract) = contract(tcx, instance) else {
        return false;
    };
    reviewed_body(tcx, instance, tcx.instance_mir(instance.def), &contract)
}

fn normalized_ty<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    ty: Ty<'tcx>,
) -> Option<Ty<'tcx>> {
    let ty = instance
        .try_instantiate_mir_and_normalize_erasing_regions(
            tcx,
            TypingEnv::fully_monomorphized(),
            EarlyBinder::bind(ty),
        )
        .ok()?;
    (!ty.has_non_region_param()
        && !ty.has_infer()
        && !ty.has_aliases()
        && !ty.has_escaping_bound_vars())
    .then_some(ty)
}

fn shared<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> Ty<'tcx> {
    Ty::new_imm_ref(tcx, tcx.lifetimes.re_erased, ty)
}

fn safe_signature<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>, input: Ty<'tcx>) -> bool {
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    );
    let Ok(signature) =
        tcx.try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), signature)
    else {
        return false;
    };
    signature.safety == Safety::Safe
        && signature.abi == ExternAbi::Rust
        && !signature.c_variadic
        && !signature.has_non_region_param()
        && !signature.has_infer()
        && !signature.has_aliases()
        && !signature.has_escaping_bound_vars()
        && signature.inputs() == [shared(tcx, input), shared(tcx, input)]
        && signature.output() == tcx.types.bool
}

fn resolve_eq<'tcx>(tcx: TyCtxt<'tcx>, eq: DefId, ty: Ty<'tcx>) -> Option<Instance<'tcx>> {
    let instance = Instance::try_resolve(
        tcx,
        TypingEnv::fully_monomorphized(),
        eq,
        tcx.mk_args(&[ty.into(), ty.into()]),
    )
    .ok()??;
    (matches!(instance.def, InstanceKind::Item(_))
        && tcx.is_mir_available(instance.def_id())
        && safe_signature(tcx, instance, ty))
    .then_some(instance)
}

fn contract<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> Option<Contract<'tcx>> {
    let core = tcx.lang_items().sized_trait()?.krate;
    let partial_eq = tcx.lang_items().eq_trait()?;
    let eq_item = tcx.get_diagnostic_item(Symbol::intern("cmp_partialeq_eq"))?;
    let ne_item = tcx.get_diagnostic_item(Symbol::intern("cmp_partialeq_ne"))?;
    let option_id = tcx.lang_items().option_type()?;
    let definition = instance.def_id();
    if !matches!(instance.def, InstanceKind::Item(_))
        || definition.krate != core
        || tcx.crate_name(core).as_str() != "core"
        || !tcx.is_mir_available(definition)
        || [partial_eq, eq_item, ne_item, option_id]
            .iter()
            .any(|id| id.krate != core)
        || tcx.get_diagnostic_item(Symbol::intern("PartialEq")) != Some(partial_eq)
        || tcx.trait_of_assoc(eq_item) != Some(partial_eq)
        || tcx.trait_of_assoc(ne_item) != Some(partial_eq)
        || !tcx.opt_associated_item(definition)?.is_fn()
    {
        return None;
    }
    let operation = if definition == ne_item {
        Operation::Ne
    } else {
        Operation::Eq
    };
    let arity = if operation == Operation::Ne { 2 } else { 1 };
    if instance.args.len() != arity
        || tcx.generics_of(definition).count() != arity
        || instance.args.iter().any(|arg| arg.as_type().is_none())
    {
        return None;
    }
    let first = normalized_ty(tcx, instance, instance.args.type_at(0))?;
    let option = if operation == Operation::Ne {
        if normalized_ty(tcx, instance, instance.args.type_at(1))? != first {
            return None;
        }
        first
    } else {
        Ty::new_adt(tcx, tcx.adt_def(option_id), tcx.mk_args(&[first.into()]))
    };
    let TyKind::Adt(adt, args) = option.kind() else {
        return None;
    };
    if adt.did() != option_id
        || !adt.is_enum()
        || adt.variants().len() != 2
        || args.len() != 1
        || args[0].as_type().is_none()
    {
        return None;
    }
    let payload = args.type_at(0);
    let some_id = tcx.lang_items().option_some_variant()?;
    let none_id = tcx.lang_items().option_none_variant()?;
    if tcx.parent(some_id) != option_id || tcx.parent(none_id) != option_id {
        return None;
    }
    let some = adt
        .variants()
        .iter_enumerated()
        .find(|(_, v)| v.def_id == some_id && v.fields.len() == 1)?
        .0;
    let none = adt
        .variants()
        .iter_enumerated()
        .find(|(_, v)| v.def_id == none_id && v.fields.is_empty())?
        .0;
    let field = tcx
        .try_normalize_erasing_regions(
            TypingEnv::fully_monomorphized(),
            adt.variant(some).fields[rustc_abi::FieldIdx::from_usize(0)].ty(tcx, args),
        )
        .ok()?;
    if field != payload || !safe_signature(tcx, instance, option) {
        return None;
    }
    // Even default `ne` is restricted to the exact core Option implementation.
    let option_eq = resolve_eq(tcx, eq_item, option)?;
    let implementation = tcx.impl_of_assoc(option_eq.def_id())?;
    if option_eq.def_id().krate != core
        || implementation.krate != core
        || tcx
            .opt_associated_item(option_eq.def_id())?
            .trait_item_def_id()
            != Some(eq_item)
        || !tcx.impl_is_of_trait(implementation)
        || tcx.generics_of(implementation).count() != 1
        || option_eq.args.len() != 1
        || option_eq.args[0].as_type() != Some(payload)
    {
        return None;
    }
    let trait_ref = tcx
        .try_normalize_erasing_regions(
            TypingEnv::fully_monomorphized(),
            tcx.impl_trait_ref(implementation)
                .instantiate(tcx, option_eq.args),
        )
        .ok()?;
    if trait_ref.def_id != partial_eq
        || trait_ref.args != tcx.mk_args(&[option.into(), option.into()])
    {
        return None;
    }
    let (call_ty, callee) = if operation == Operation::Ne {
        let resolved = Instance::try_resolve(
            tcx,
            TypingEnv::fully_monomorphized(),
            ne_item,
            tcx.mk_args(&[option.into(), option.into()]),
        )
        .ok()??;
        if resolved != instance {
            return None;
        }
        (option, option_eq)
    } else {
        if option_eq != instance {
            return None;
        }
        (payload, resolve_eq(tcx, eq_item, payload)?)
    };
    let none_discriminant = adt.discriminant_for_variant(tcx, none).val;
    let some_discriminant = adt.discriminant_for_variant(tcx, some).val;
    if none_discriminant == some_discriminant {
        return None;
    }
    Some(Contract {
        operation,
        option,
        payload,
        some,
        none_discriminant,
        some_discriminant,
        eq_item,
        call_ty,
        callee,
    })
}

fn plain(place: Place<'_>, local: usize) -> bool {
    place.local == Local::from_usize(local) && place.projection.is_empty()
}

fn move_local(operand: &Operand<'_>, local: usize) -> bool {
    matches!(operand, Operand::Move(place) if plain(*place, local))
}

fn copy_local(operand: &Operand<'_>, local: usize) -> bool {
    matches!(operand, Operand::Copy(place) if plain(*place, local))
}

fn assignment<'a, 'tcx>(
    statement: &'a StatementKind<'tcx>,
    local: usize,
) -> Option<&'a Rvalue<'tcx>> {
    match statement {
        StatementKind::Assign(a) if plain(a.0, local) => Some(&a.1),
        _ => None,
    }
}

fn exact_callee_ty<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    ty: Ty<'tcx>,
    contract: &Contract<'tcx>,
) -> bool {
    let Some(ty) = normalized_ty(tcx, instance, ty) else {
        return false;
    };
    let TyKind::FnDef(definition, args) = ty.kind() else {
        return false;
    };
    *definition == contract.eq_item
        && *args == tcx.mk_args(&[contract.call_ty.into(), contract.call_ty.into()])
        && resolve_eq(tcx, *definition, contract.call_ty) == Some(contract.callee)
}

fn call_target<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    terminator: &TerminatorKind<'tcx>,
    contract: &Contract<'tcx>,
    destination_local: usize,
    left: usize,
    right: usize,
) -> Option<BasicBlock> {
    call_target_with_operands(
        tcx,
        instance,
        terminator,
        contract,
        destination_local,
        left,
        right,
        move_local,
    )
}

fn call_target_with_operands<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    terminator: &TerminatorKind<'tcx>,
    contract: &Contract<'tcx>,
    destination_local: usize,
    left: usize,
    right: usize,
    argument_matches: fn(&Operand<'tcx>, usize) -> bool,
) -> Option<BasicBlock> {
    let TerminatorKind::Call {
        func: Operand::Constant(callee),
        args,
        destination,
        target: Some(target),
        unwind,
        ..
    } = terminator
    else {
        return None;
    };
    if callee.user_ty.is_some()
        || !matches!(callee.const_, Const::Val(ConstValue::ZeroSized, _))
        || !exact_callee_ty(tcx, instance, callee.const_.ty(), contract)
        || !plain(*destination, destination_local)
        || args.len() != 2
        || !argument_matches(&args[0].node, left)
        || !argument_matches(&args[1].node, right)
        || !match unwind {
            UnwindAction::Continue => true,
            UnwindAction::Unreachable => !tcx.sess.panic_strategy().unwinds(),
            _ => false,
        }
    {
        return None;
    }
    Some(*target)
}

// Fixed MIR work: host ne has 2 blocks/4 locals/3 statements; host eq has 7/8/7.
// Encoded eq has 9/14/14; encoded ne has 2/4/1. No body normalization is performed.
// Every block is assigned one role, including the invalid-discriminant sink.
// Type normalization and trait resolution remain rustc queries, not MIR walks.
fn reviewed_body<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    contract: &Contract<'tcx>,
) -> bool {
    let encoded_eq = contract.operation == Operation::Eq && body.basic_blocks.len() == 9;
    let encoded_ne = contract.operation == Operation::Ne
        && body
            .basic_blocks
            .iter()
            .next()
            .is_some_and(|block| block.statements.is_empty());
    let (blocks, locals, scopes, statements) = if encoded_ne {
        (2, 4, 1, 1)
    } else if contract.operation == Operation::Ne {
        (2, 4, 1, 3)
    } else if encoded_eq {
        (9, 14, 2, 14)
    } else {
        (7, 8, 2, 7)
    };
    if body.arg_count != 2
        || body.basic_blocks.len() != blocks
        || body.local_decls.len() != locals
        || body.source_scopes.len() != scopes
        || body.source.instance != instance.def
        || body.source.promoted.is_some()
        || body.coroutine.is_some()
        || body.spread_arg.is_some()
        || body.tainted_by_errors.is_some()
        || !body.user_type_annotations.is_empty()
        || body
            .required_consts
            .as_ref()
            .is_some_and(|items| !items.is_empty())
        || body.source_scopes.iter_enumerated().any(|(i, s)| {
            s.inlined.is_some()
                || s.inlined_parent_scope.is_some()
                || s.parent_scope
                    != (i.as_usize() != 0).then_some(rustc_middle::mir::SourceScope::from_usize(0))
        })
        || body
            .basic_blocks
            .iter()
            .any(|b| b.is_cleanup || b.terminator.is_none() || b.statements.len() > statements)
    {
        return false;
    }
    if body
        .basic_blocks
        .iter()
        .map(|b| b.statements.len())
        .sum::<usize>()
        != statements
    {
        return false;
    }
    let reference = shared(tcx, contract.option);
    let expected = if contract.operation == Operation::Ne {
        vec![tcx.types.bool, reference, reference, tcx.types.bool]
    } else if encoded_eq {
        encoded::eq_local_types(tcx, contract)
    } else {
        vec![
            tcx.types.bool,
            reference,
            reference,
            tcx.types.isize,
            tcx.types.isize,
            tcx.types.isize,
            shared(tcx, contract.payload),
            shared(tcx, contract.payload),
        ]
    };
    if body
        .local_decls
        .iter()
        .zip(expected)
        .any(|(decl, expected)| {
            normalized_ty(tcx, instance, decl.ty) != Some(expected)
                || decl.user_ty.is_some()
                || decl.source_info.scope.as_usize() >= scopes
        })
        || body.basic_blocks.iter().any(|b| {
            b.terminator().source_info.scope.as_usize() >= scopes
                || b.statements
                    .iter()
                    .any(|s| s.source_info.scope.as_usize() >= scopes)
        })
    {
        return false;
    }
    let Some([mention]) = body.mentioned_items.as_deref() else {
        return false;
    };
    if !matches!(mention.node, MentionedItem::Fn(ty) if exact_callee_ty(tcx, instance, ty, contract))
    {
        return false;
    }
    if encoded_ne {
        encoded::reviewed_ne(tcx, instance, body, contract)
    } else if contract.operation == Operation::Ne {
        reviewed_ne(tcx, instance, body, contract)
    } else if encoded_eq {
        encoded::reviewed_eq(tcx, instance, body, contract)
    } else {
        reviewed_eq(tcx, instance, body, contract)
    }
}

fn reviewed_ne<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    contract: &Contract<'tcx>,
) -> bool {
    let entry = &body.basic_blocks[BasicBlock::from_usize(0)];
    let done = &body.basic_blocks[BasicBlock::from_usize(1)];
    matches!(entry.statements.as_slice(), [live] if matches!(live.kind, StatementKind::StorageLive(l) if l.as_usize() == 3))
        && call_target(tcx, instance, &entry.terminator().kind, contract, 3, 1, 2)
            == Some(BasicBlock::from_usize(1))
        && matches!(done.statements.as_slice(), [negate, dead]
            if matches!(assignment(&negate.kind, 0), Some(Rvalue::UnaryOp(UnOp::Not, operand)) if move_local(operand, 3))
            && matches!(dead.kind, StatementKind::StorageDead(l) if l.as_usize() == 3))
        && matches!(done.terminator().kind, TerminatorKind::Return)
}

fn discriminant(statement: &StatementKind<'_>, destination: usize, input: usize) -> bool {
    matches!(assignment(statement, destination), Some(Rvalue::Discriminant(place))
        if place.local.as_usize() == input && matches!(place.projection.as_ref(), [ProjectionElem::Deref]))
}

fn switch<'a>(
    terminator: &'a TerminatorKind<'_>,
    local: usize,
    contract: &Contract<'_>,
) -> Option<&'a SwitchTargets> {
    let TerminatorKind::SwitchInt { discr, targets } = terminator else {
        return None;
    };
    (move_local(discr, local)
        && targets.all_values() == [contract.none_discriminant, contract.some_discriminant]
        && targets.all_targets().len() == 3
        && targets.all_targets().iter().all(|b| b.as_usize() < 7))
    .then_some(targets)
}

fn constant<'tcx>(tcx: TyCtxt<'tcx>, operand: &Operand<'tcx>, ty: Ty<'tcx>, value: u128) -> bool {
    let Operand::Constant(c) = operand else {
        return false;
    };
    if c.user_ty.is_some() || !matches!(c.const_, Const::Val(_, t) if t == ty) {
        return false;
    }
    if ty == tcx.types.bool {
        c.const_.try_to_bool().map(u128::from) == Some(value)
    } else {
        c.const_
            .try_to_scalar_int()
            .and_then(|v| v.try_to_bits(tcx.data_layout.pointer_size()).ok())
            == Some(value)
    }
}

fn borrow_payload<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    statement: &StatementKind<'tcx>,
    contract: &Contract<'tcx>,
    destination: usize,
    input: usize,
) -> bool {
    let Some(Rvalue::Ref(_, BorrowKind::Shared, place)) = assignment(statement, destination) else {
        return false;
    };
    place.local.as_usize() == input
        && matches!(place.projection.as_ref(),
        [ProjectionElem::Deref, ProjectionElem::Downcast(_, variant), ProjectionElem::Field(field, ty)]
        if *variant == contract.some && field.as_usize() == 0 && normalized_ty(tcx, instance, *ty) == Some(contract.payload))
}

fn reviewed_eq<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    contract: &Contract<'tcx>,
) -> bool {
    let entry = BasicBlock::from_usize(0);
    let start = &body.basic_blocks[entry];
    if !matches!(start.statements.as_slice(), [s] if discriminant(&s.kind, 5, 1)) {
        return false;
    }
    let Some(first) = switch(&start.terminator().kind, 5, contract) else {
        return false;
    };
    let left_some = first.target_for_value(contract.some_discriminant);
    let left_none = first.target_for_value(contract.none_discriminant);
    let invalid = first.otherwise();
    let some_block = &body.basic_blocks[left_some];
    if !matches!(some_block.statements.as_slice(), [s] if discriminant(&s.kind, 3, 2)) {
        return false;
    }
    let Some(second) = switch(&some_block.terminator().kind, 3, contract) else {
        return false;
    };
    if second.otherwise() != invalid {
        return false;
    }
    let both_some = second.target_for_value(contract.some_discriminant);
    let mixed = second.target_for_value(contract.none_discriminant);
    let both_block = &body.basic_blocks[both_some];
    let Some(done) = call_target(
        tcx,
        instance,
        &both_block.terminator().kind,
        contract,
        0,
        6,
        7,
    ) else {
        return false;
    };
    let mut roles = [
        entry.as_usize(),
        left_some.as_usize(),
        left_none.as_usize(),
        invalid.as_usize(),
        both_some.as_usize(),
        mixed.as_usize(),
        done.as_usize(),
    ];
    roles.sort_unstable();
    if roles != [0, 1, 2, 3, 4, 5, 6] {
        return false;
    }
    let invalid_block = &body.basic_blocks[invalid];
    let none_block = &body.basic_blocks[left_none];
    let mixed_block = &body.basic_blocks[mixed];
    let done_block = &body.basic_blocks[done];
    invalid_block.statements.is_empty()
        && matches!(invalid_block.terminator().kind, TerminatorKind::Unreachable)
        && matches!(none_block.statements.as_slice(), [tag, result]
            if discriminant(&tag.kind, 4, 2)
            && matches!(assignment(&result.kind, 0), Some(Rvalue::BinaryOp(BinOp::Eq, operands))
                if matches!(operands.0, Operand::Copy(place) if plain(place, 4))
                && constant(tcx, &operands.1, tcx.types.isize, contract.none_discriminant)))
        && matches!(none_block.terminator().kind, TerminatorKind::Goto { target } if target == done)
        && matches!(mixed_block.statements.as_slice(), [s]
            if matches!(assignment(&s.kind, 0), Some(Rvalue::Use(operand)) if constant(tcx, operand, tcx.types.bool, 0)))
        && matches!(mixed_block.terminator().kind, TerminatorKind::Goto { target } if target == done)
        && matches!(both_block.statements.as_slice(), [left, right]
            if borrow_payload(tcx, instance, &left.kind, contract, 6, 1)
            && borrow_payload(tcx, instance, &right.kind, contract, 7, 2))
        && done_block.statements.is_empty()
        && matches!(done_block.terminator().kind, TerminatorKind::Return)
}

#[cfg(test)]
#[path = "core_option_compare_v1/tests.rs"]
mod tests;

#[path = "core_option_compare_v1/promoted.rs"]
mod promoted;
pub(crate) use promoted::{ReviewedPromotedOptionCompareV1, prove_promoted_option_comparisons_v1};
