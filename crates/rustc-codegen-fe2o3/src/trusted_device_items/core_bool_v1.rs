//! Exact source-safety authentication of core `bool::then_some`, not callee
//! admission. Preserve its conditional moves and Drop edge in collected MIR.
//! A payload destructor still needs independent admission, even if it is empty.

use rustc_abi::{ExternAbi, VariantIdx};
use rustc_hir::{Safety, def_id::DefId};
use rustc_middle::{
    mir::{
        AggregateKind, BasicBlock, Body, Const, Local, MentionedItem, Operand, Rvalue,
        StatementKind, TerminatorKind, UnwindAction,
    },
    ty::{EarlyBinder, Instance, InstanceKind, Ty, TyCtxt, TypeVisitableExt, TypingEnv},
};
use rustc_span::Symbol;

// At most six blocks, five locals, one scope, and six statements. Exactly
// two source paths, no recursion or path enumeration. Normalization and glue
// resolution use rustc queries; the MIR check itself has fixed work.
struct Contract<'tcx> {
    payload: Ty<'tcx>,
    output: Ty<'tcx>,
    option: DefId,
    some: VariantIdx,
    none: VariantIdx,
    glue: Instance<'tcx>,
}

pub(crate) fn authenticate_reviewed_safe_core_bool_helper_v1<'tcx>(
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

fn contract<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> Option<Contract<'tcx>> {
    let core = tcx.lang_items().sized_trait()?.krate;
    let definition = instance.def_id();
    if !matches!(instance.def, InstanceKind::Item(_))
        || definition.krate != core
        || tcx.crate_name(core).as_str() != "core"
        || !tcx.is_mir_available(definition)
        || instance.args.len() != 1
        || instance.args[0].as_type().is_none()
    {
        return None;
    }
    // `then_some` has no diagnostic item on the pinned compiler. Its nominal
    // owner is the primitive-bool inherent impl containing diagnostic bool_then.
    let anchor = tcx.get_diagnostic_item(Symbol::intern("bool_then"))?;
    let implementation = tcx.impl_of_assoc(anchor)?;
    let item = tcx.opt_associated_item(definition)?;
    if anchor.krate != core
        || implementation.krate != core
        || tcx.impl_is_of_trait(implementation)
        || tcx.impl_of_assoc(definition) != Some(implementation)
        || !item.is_fn()
        || item.trait_item_def_id().is_some()
        || tcx.generics_of(implementation).count() != 0
        || tcx.generics_of(definition).count() != 1
        || tcx
            .associated_items(implementation)
            .in_definition_order()
            .find(|item| item.is_fn() && item.name().as_str() == "then_some")?
            .def_id
            != definition
        || normalized_ty(
            tcx,
            instance,
            tcx.type_of(implementation).instantiate(tcx, instance.args),
        )? != tcx.types.bool
    {
        return None;
    }
    let payload = normalized_ty(tcx, instance, instance.args.type_at(0))?;
    let option = tcx.lang_items().option_type()?;
    let adt = tcx.adt_def(option);
    if option.krate != core || !adt.is_enum() || adt.variants().len() != 2 {
        return None;
    }
    let some_id = tcx.lang_items().option_some_variant()?;
    let none_id = tcx.lang_items().option_none_variant()?;
    if tcx.parent(some_id) != option || tcx.parent(none_id) != option {
        return None;
    }
    let some = adt
        .variants()
        .iter_enumerated()
        .find(|(_, variant)| variant.def_id == some_id && variant.fields.len() == 1)?
        .0;
    let none = adt
        .variants()
        .iter_enumerated()
        .find(|(_, variant)| variant.def_id == none_id && variant.fields.is_empty())?
        .0;
    let output = Ty::new_adt(tcx, adt, tcx.mk_args(&[payload.into()]));
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(definition).instantiate(tcx, instance.args),
    );
    let signature = tcx
        .try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), signature)
        .ok()?;
    if signature.safety != Safety::Safe
        || signature.abi != ExternAbi::Rust
        || signature.c_variadic
        || signature.has_non_region_param()
        || signature.has_infer()
        || signature.has_aliases()
        || signature.has_escaping_bound_vars()
        || signature.inputs() != [tcx.types.bool, payload]
        || signature.output() != output
    {
        return None;
    }
    let drop_in_place = tcx.lang_items().drop_in_place_fn()?;
    if drop_in_place.krate != core {
        return None;
    }
    let glue = Instance::try_resolve(
        tcx,
        TypingEnv::fully_monomorphized(),
        drop_in_place,
        tcx.mk_args(&[payload.into()]),
    )
    .ok()??;
    if !matches!(glue.def, InstanceKind::DropGlue(id, ty)
        if id == drop_in_place && ty.is_none_or(|ty| ty == payload))
        || glue.args.len() != 1
        || glue.args[0].as_type() != Some(payload)
    {
        return None;
    }
    Some(Contract {
        payload,
        output,
        option,
        some,
        none,
        glue,
    })
}

fn move_local(operand: &Operand<'_>, local: usize) -> bool {
    matches!(operand, Operand::Move(place)
        if place.local == Local::from_usize(local) && place.projection.is_empty())
}

fn variant_assignment<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    statement: &StatementKind<'tcx>,
    contract: &Contract<'tcx>,
    payload_local: Option<usize>,
) -> bool {
    let StatementKind::Assign(assignment) = statement else {
        return false;
    };
    if assignment.0.local != Local::from_usize(0) || !assignment.0.projection.is_empty() {
        return false;
    }
    let Rvalue::Aggregate(kind, operands) = &assignment.1 else {
        return false;
    };
    let AggregateKind::Adt(definition, variant, args, None, None) = &**kind else {
        return false;
    };
    if *definition != contract.option
        || args.len() != 1
        || args[0].as_type().is_none()
        || normalized_ty(tcx, instance, args.type_at(0)) != Some(contract.payload)
    {
        return false;
    }
    match payload_local {
        Some(local) => {
            *variant == contract.some && operands.len() == 1 && move_local(&operands.raw[0], local)
        }
        None => *variant == contract.none && operands.is_empty(),
    }
}

fn reviewed_body<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    contract: &Contract<'tcx>,
) -> bool {
    if body.arg_count != 2
        || !(3..=5).contains(&body.local_decls.len())
        || ![4, 6].contains(&body.basic_blocks.len())
        || body.source_scopes.len() != 1
        || body.source_scopes.iter().any(|scope| {
            scope.parent_scope.is_some()
                || scope.inlined.is_some()
                || scope.inlined_parent_scope.is_some()
        })
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
        || body
            .basic_blocks
            .iter()
            .any(|block| block.is_cleanup || block.terminator.is_none())
    {
        return false;
    }
    let expected = [
        contract.output,
        tcx.types.bool,
        contract.payload,
        contract.payload,
        tcx.types.bool,
    ];
    if body
        .local_decls
        .iter()
        .zip(expected)
        .any(|(decl, expected)| normalized_ty(tcx, instance, decl.ty) != Some(expected))
    {
        return false;
    }
    let Some([mention]) = body.mentioned_items.as_deref() else {
        return false;
    };
    if !matches!(&mention.node, MentionedItem::Drop(ty)
        if normalized_ty(tcx, instance, *ty) == Some(contract.payload))
    {
        return false;
    }
    if body.basic_blocks.len() == 6 {
        return reviewed_drop_flag_body(tcx, instance, body, contract);
    }
    let entry = BasicBlock::from_usize(0);
    let start = &body.basic_blocks[entry];
    let TerminatorKind::SwitchInt {
        discr: Operand::Copy(place),
        targets,
    } = &start.terminator().kind
    else {
        return false;
    };
    if !start.statements.is_empty()
        || place.local != Local::from_usize(1)
        || !place.projection.is_empty()
        || targets.all_values() != [0]
        || targets.all_targets().len() != 2
        || targets
            .all_targets()
            .iter()
            .any(|target| target.as_usize() >= 4)
    {
        return false;
    }
    let yes = targets.target_for_value(1);
    let no = targets.target_for_value(0);
    let yes_block = &body.basic_blocks[yes];
    let no_block = &body.basic_blocks[no];
    let TerminatorKind::Goto { target: done } = &yes_block.terminator().kind else {
        return false;
    };
    let TerminatorKind::Drop {
        place,
        target,
        unwind,
        replace: false,
        drop: None,
        async_fut: None,
    } = &no_block.terminator().kind
    else {
        return false;
    };
    let mut visited = [
        entry.as_usize(),
        yes.as_usize(),
        no.as_usize(),
        done.as_usize(),
    ];
    visited.sort_unstable();
    if visited != [0, 1, 2, 3]
        || target != done
        || place.local != Local::from_usize(2)
        || !place.projection.is_empty()
        || !match unwind {
            UnwindAction::Continue => true,
            UnwindAction::Unreachable => !tcx.sess.panic_strategy().unwinds(),
            _ => false,
        }
    {
        return false;
    }
    let done_block = &body.basic_blocks[*done];
    if !done_block.statements.is_empty()
        || !matches!(done_block.terminator().kind, TerminatorKind::Return)
        || no_block.statements.len() != 1
        || !variant_assignment(tcx, instance, &no_block.statements[0].kind, contract, None)
    {
        return false;
    }
    // The explicit move-only forms are valid for arbitrary non-Copy T. Neither
    // equal types nor a no-drop answer can replace the original payload value.
    let moves_payload = match yes_block.statements.as_slice() {
        [some] if body.local_decls.len() == 3 => {
            variant_assignment(tcx, instance, &some.kind, contract, Some(2))
        }
        [live, take, some, dead] if body.local_decls.len() == 4 => {
            matches!(live.kind, StatementKind::StorageLive(local) if local == Local::from_usize(3))
                && matches!(&take.kind, StatementKind::Assign(assignment)
                    if assignment.0.local == Local::from_usize(3) && assignment.0.projection.is_empty()
                    && matches!(&assignment.1, Rvalue::Use(operand) if move_local(operand, 2)))
                && variant_assignment(tcx, instance, &some.kind, contract, Some(3))
                && matches!(dead.kind, StatementKind::StorageDead(local) if local == Local::from_usize(3))
        }
        _ => false,
    };
    let Some(drop_in_place) = tcx.lang_items().drop_in_place_fn() else {
        return false;
    };
    moves_payload
        && matches!(Instance::try_resolve(
        tcx, TypingEnv::fully_monomorphized(), drop_in_place, tcx.mk_args(&[contract.payload.into()]),
    ), Ok(Some(glue)) if glue == contract.glue)
}

fn flag_assignment<'tcx>(tcx: TyCtxt<'tcx>, statement: &StatementKind<'tcx>, value: bool) -> bool {
    matches!(statement, StatementKind::Assign(a)
        if a.0.local.as_usize() == 4 && a.0.projection.is_empty()
        && matches!(&a.1, Rvalue::Use(Operand::Constant(c))
            if c.user_ty.is_none() && matches!(c.const_, Const::Val(_, ty) if ty == tcx.types.bool)
            && c.const_.try_to_bool() == Some(value)))
}

fn flag_switch(terminator: &TerminatorKind<'_>, local: usize) -> Option<(BasicBlock, BasicBlock)> {
    let TerminatorKind::SwitchInt {
        discr: Operand::Copy(place),
        targets,
    } = terminator
    else {
        return None;
    };
    (place.local.as_usize() == local
        && place.projection.is_empty()
        && targets.all_values() == [0]
        && targets.all_targets().len() == 2
        && targets.all_targets().iter().all(|bb| bb.as_usize() < 6))
    .then(|| (targets.target_for_value(0), targets.target_for_value(1)))
}

fn reviewed_drop_flag_body<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    contract: &Contract<'tcx>,
) -> bool {
    if body.local_decls.len() != 5 {
        return false;
    }
    let entry = BasicBlock::from_usize(0);
    let start = &body.basic_blocks[entry];
    if !matches!(start.statements.as_slice(), [clear, live]
        if flag_assignment(tcx, &clear.kind, false) && flag_assignment(tcx, &live.kind, true))
    {
        return false;
    }
    let Some((no, yes)) = flag_switch(&start.terminator().kind, 1) else {
        return false;
    };
    let yes_block = &body.basic_blocks[yes];
    let no_block = &body.basic_blocks[no];
    let TerminatorKind::Goto { target: join } = yes_block.terminator().kind else {
        return false;
    };
    if join.as_usize() >= 6
        || !matches!(no_block.terminator().kind, TerminatorKind::Goto { target } if target == join)
    {
        return false;
    }
    let join_block = &body.basic_blocks[join];
    let Some((done, drop)) = flag_switch(&join_block.terminator().kind, 4) else {
        return false;
    };
    let mut roles = [
        entry.as_usize(),
        yes.as_usize(),
        no.as_usize(),
        join.as_usize(),
        done.as_usize(),
        drop.as_usize(),
    ];
    roles.sort_unstable();
    if roles != [0, 1, 2, 3, 4, 5] {
        return false;
    }
    let done_block = &body.basic_blocks[done];
    let drop_block = &body.basic_blocks[drop];
    // `_4` means `_2` still owns the payload. On true it is cleared before
    // the sole move; on false it remains true until the sole Drop. The complete
    // six-role match proves both routes, without ignoring the flag as dead code.
    matches!(yes_block.statements.as_slice(), [clear, take, some]
        if flag_assignment(tcx, &clear.kind, false)
        && matches!(&take.kind, StatementKind::Assign(a)
            if a.0.local.as_usize() == 3 && a.0.projection.is_empty()
            && matches!(&a.1, Rvalue::Use(operand) if move_local(operand, 2)))
        && variant_assignment(tcx, instance, &some.kind, contract, Some(3)))
        && matches!(no_block.statements.as_slice(), [none]
            if variant_assignment(tcx, instance, &none.kind, contract, None))
        && join_block.statements.is_empty()
        && done_block.statements.is_empty()
        && matches!(done_block.terminator().kind, TerminatorKind::Return)
        && drop_block.statements.is_empty()
        && matches!(drop_block.terminator().kind, TerminatorKind::Drop {
            place, target, unwind, replace: false, drop: None, async_fut: None,
        } if place.local.as_usize() == 2 && place.projection.is_empty() && target == done
            && match unwind {
                UnwindAction::Continue => true,
                UnwindAction::Unreachable => !tcx.sess.panic_strategy().unwinds(),
                _ => false,
            })
}

#[cfg(test)]
#[path = "core_bool_v1/tests.rs"]
mod tests;
