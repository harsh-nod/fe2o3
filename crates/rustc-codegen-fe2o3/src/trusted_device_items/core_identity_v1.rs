//! Source-safety authentication for core's exact reflexive `From<T> for T`.
//! The original generic MIR and concrete call/ABI remain recursively collected.
//! No expansion, panic exemption, fingerprint authority or Copy bound is added.

use rustc_abi::ExternAbi;
use rustc_hir::{Safety, def::DefKind};
use rustc_middle::mir::{
    Body, Local, MirPhase, Operand, RETURN_PLACE, RuntimePhase, Rvalue, START_BLOCK, StatementKind,
    TerminatorKind,
};
use rustc_middle::ty::{
    EarlyBinder, FnSig, GenericParamDefKind, Instance, InstanceKind, Ty, TyCtxt, TyKind,
    TypeVisitableExt, TypingEnv,
};
use rustc_span::Symbol;

const MAX_DEBUG_INFO: usize = 32;

/// Discharges only the unavailable core HIR observation for this exact item.
pub(crate) fn authenticate_reviewed_safe_core_identity_helper_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> bool {
    let Some(parameter) = identity(tcx, instance) else {
        return false;
    };
    reviewed_body(tcx, instance, tcx.instance_mir(instance.def), parameter)
}

fn identity<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> Option<Ty<'tcx>> {
    let core = tcx.lang_items().sized_trait()?.krate;
    let definition = instance.def_id();
    if !matches!(instance.def, InstanceKind::Item(_))
        || definition.krate != core
        || tcx.crate_name(core).as_str() != "core"
        || tcx.def_kind(definition) != DefKind::AssocFn
        || tcx.item_name(definition).as_str() != "from"
        || !matches!(instance.args.as_slice(), [argument] if argument.as_type().is_some())
        || !tcx.is_mir_available(definition)
    {
        return None;
    }
    let from = tcx.get_diagnostic_item(Symbol::intern("From"))?;
    let item = tcx.opt_associated_item(definition)?;
    let trait_item = item.trait_item_def_id()?;
    let implementation = tcx.impl_of_assoc(definition)?;
    if from.krate != core
        || !item.is_fn()
        || trait_item.krate != core
        || tcx.trait_of_assoc(trait_item) != Some(from)
        || tcx.item_name(trait_item).as_str() != "from"
        || implementation.krate != core
        || !tcx.impl_is_of_trait(implementation)
    {
        return None;
    }
    let generics = tcx.generics_of(implementation);
    let [generic] = generics.own_params.as_slice() else {
        return None;
    };
    let method_generics = tcx.generics_of(definition);
    if generics.parent.is_some()
        || generics.parent_count != 0
        || generics.has_self
        || generic.index != 0
        || generic.pure_wrt_drop
        || tcx.parent(generic.def_id) != implementation
        || !matches!(
            generic.kind,
            GenericParamDefKind::Type {
                has_default: false,
                synthetic: false
            }
        )
        || method_generics.parent != Some(implementation)
        || method_generics.parent_count != 1
        || !method_generics.own_params.is_empty()
    {
        return None;
    }
    let parameter = tcx.type_of(implementation).instantiate_identity();
    let TyKind::Param(param) = parameter.kind() else {
        return None;
    };
    let trait_ref = tcx.impl_trait_ref(implementation).instantiate_identity();
    if param.index != generic.index
        || param.name != generic.name
        || trait_ref.def_id != from
        || trait_ref.args.as_slice()
            != tcx
                .mk_args(&[parameter.into(), parameter.into()])
                .as_slice()
    {
        return None;
    }
    let signature =
        tcx.instantiate_bound_regions_with_erased(tcx.fn_sig(definition).instantiate_identity());
    signature_matches(signature, parameter).then_some(parameter)
}

fn signature_matches<'tcx>(signature: FnSig<'tcx>, value: Ty<'tcx>) -> bool {
    signature.safety == Safety::Safe
        && signature.abi == ExternAbi::Rust
        && !signature.c_variadic
        && signature.inputs() == [value]
        && signature.output() == value
}

fn reviewed_body<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    parameter: Ty<'tcx>,
) -> bool {
    // Check cardinalities before inspecting metadata or normalizing types.
    // Production borrows this body throughout: it neither hashes nor clones it.
    if body.source.instance != instance.def
        || body.source.promoted.is_some()
        || body.phase != MirPhase::Runtime(RuntimePhase::Optimized)
        || body.injection_phase.is_some()
        || body.tainted_by_errors.is_some()
        || !body.is_polymorphic
        || body.coroutine.is_some()
        || body.spread_arg.is_some()
        || body.arg_count != 1
        || body.local_decls.len() != 2
        || body.basic_blocks.len() != 1
        || body.source_scopes.len() != 1
        || body.var_debug_info.len() > MAX_DEBUG_INFO
        || !body.user_type_annotations.is_empty()
        || body
            .required_consts
            .as_ref()
            .is_none_or(|items| !items.is_empty())
        || body
            .mentioned_items
            .as_ref()
            .is_some_and(|items| !items.is_empty())
        || body.coverage_info_hi.is_some()
        || body.function_coverage_info.is_some()
    {
        return false;
    }
    let scope = &body.source_scopes.raw[0];
    let block = &body.basic_blocks[START_BLOCK];
    if scope.parent_scope.is_some()
        || scope.inlined.is_some()
        || scope.inlined_parent_scope.is_some()
        || block.is_cleanup
        || block.statements.len() != 1
        || body
            .local_decls
            .iter()
            .any(|local| local.ty != parameter || local.source_info.scope.as_usize() != 0)
    {
        return false;
    }
    let statement = &block.statements[0];
    let Some(terminator) = &block.terminator else {
        return false;
    };
    if statement.source_info.scope.as_usize() != 0
        || terminator.source_info.scope.as_usize() != 0
        || !matches!(terminator.kind, TerminatorKind::Return)
        || !matches!(&statement.kind, StatementKind::Assign(assignment)
            if assignment.0 == RETURN_PLACE.into()
                && matches!(&assignment.1, Rvalue::Use(Operand::Move(source))
                    if *source == Local::from_usize(1).into()))
    {
        return false;
    }
    let environment = TypingEnv::fully_monomorphized();
    let Ok(value) = instance.try_instantiate_mir_and_normalize_erasing_regions(
        tcx,
        environment,
        EarlyBinder::bind(parameter),
    ) else {
        return false;
    };
    if value.has_non_region_param()
        || value.has_infer()
        || value.has_aliases()
        || value.has_escaping_bound_vars()
        || value.references_error()
        || !value.is_sized(tcx, environment)
    {
        return false;
    }
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    );
    tcx.try_normalize_erasing_regions(environment, signature)
        .is_ok_and(|signature| signature_matches(signature, value))
}

#[cfg(test)]
#[path = "core_identity_v1/tests.rs"]
mod tests;
