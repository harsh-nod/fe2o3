//! Source-safety authentication of the exact core `mem_drop` wrapper only.
//!
//! Keep the original body and its Drop edge. This does not authenticate the
//! resolved drop glue, any destructor, or any other function with no-drop types.
//! The collector must independently check that edge or reject unsupported glue.

use rustc_abi::ExternAbi;
use rustc_hir::{Safety, def::DefKind};
use rustc_middle::{
    mir::{BasicBlock, Body, Local, MentionedItem, TerminatorKind, UnwindAction},
    ty::{EarlyBinder, Instance, InstanceKind, Ty, TyCtxt, TypeVisitableExt, TypingEnv},
};
use rustc_span::Symbol;

// Two locals, two blocks, one scope, no statements, and one exact Drop edge.
// MIR work is constant; rustc owns normalization and drop-resolution limits.
struct Contract<'tcx> {
    input: Ty<'tcx>,
    glue: Instance<'tcx>,
}

/// Grants source observation for the wrapper, never terminal/callee admission.
pub(crate) fn authenticate_reviewed_safe_core_mem_drop_helper_v1<'tcx>(
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
    let definition = tcx.get_diagnostic_item(Symbol::intern("mem_drop"))?;
    if instance.def != InstanceKind::Item(definition)
        || definition.krate != core
        || tcx.crate_name(core).as_str() != "core"
        || tcx.def_kind(definition) != DefKind::Fn
        || tcx.opt_associated_item(definition).is_some()
        || tcx.generics_of(definition).count() != 1
        || !tcx.is_mir_available(definition)
        || instance.args.len() != 1
        || instance.args[0].as_type().is_none()
    {
        return None;
    }
    let input = normalized_ty(tcx, instance, instance.args.type_at(0))?;
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
        || signature.inputs() != [input]
        || signature.output() != tcx.types.unit
    {
        return None;
    }
    let drop_in_place = tcx.lang_items().drop_in_place_fn()?;
    if drop_in_place.krate != core || tcx.def_kind(drop_in_place) != DefKind::Fn {
        return None;
    }
    let glue = Instance::try_resolve(
        tcx,
        TypingEnv::fully_monomorphized(),
        drop_in_place,
        tcx.mk_args(&[input.into()]),
    )
    .ok()??;
    match glue.def {
        InstanceKind::DropGlue(definition, ty)
            if definition == drop_in_place
                && ty.is_none_or(|ty| ty == input)
                && glue.args.len() == 1
                && glue.args[0].as_type() == Some(input) => {}
        _ => return None,
    }
    Some(Contract { input, glue })
}

fn reviewed_body<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    contract: &Contract<'tcx>,
) -> bool {
    if body.arg_count != 1
        || body.local_decls.len() != 2
        || body.basic_blocks.len() != 2
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
            .any(|block| block.is_cleanup || !block.statements.is_empty())
        || normalized_ty(tcx, instance, body.local_decls[Local::from_usize(0)].ty)
            != Some(tcx.types.unit)
        || normalized_ty(tcx, instance, body.local_decls[Local::from_usize(1)].ty)
            != Some(contract.input)
    {
        return false;
    }
    let Some([mention]) = body.mentioned_items.as_deref() else {
        return false;
    };
    if !matches!(&mention.node, MentionedItem::Drop(ty)
        if normalized_ty(tcx, instance, *ty) == Some(contract.input))
    {
        return false;
    }
    let first = BasicBlock::from_usize(0);
    let last = BasicBlock::from_usize(1);
    let Some(terminator) = body.basic_blocks[first].terminator.as_ref() else {
        return false;
    };
    let TerminatorKind::Drop {
        place,
        target,
        unwind,
        replace: false,
        drop: None,
        async_fut: None,
    } = &terminator.kind
    else {
        return false;
    };
    if place.local != Local::from_usize(1)
        || !place.projection.is_empty()
        || *target != last
        || !match unwind {
            UnwindAction::Continue => true,
            UnwindAction::Unreachable => !tcx.sess.panic_strategy().unwinds(),
            _ => false,
        }
        || !matches!(
            body.basic_blocks[last].terminator.as_ref().map(|t| &t.kind),
            Some(TerminatorKind::Return)
        )
    {
        return false;
    }
    // Type-normalized Drop(_1) must name the same compiler-resolved glue. The
    // glue's body remains an independent collector obligation, including when
    // rustc resolves a user's destructor inside it.
    let Some(drop_in_place) = tcx.lang_items().drop_in_place_fn() else {
        return false;
    };
    matches!(
        Instance::try_resolve(
            tcx,
            TypingEnv::fully_monomorphized(),
            drop_in_place,
            tcx.mk_args(&[contract.input.into()]),
        ),
        Ok(Some(glue)) if glue == contract.glue
    )
}

#[cfg(test)]
#[path = "core_mem_drop_v1/tests.rs"]
mod tests;
