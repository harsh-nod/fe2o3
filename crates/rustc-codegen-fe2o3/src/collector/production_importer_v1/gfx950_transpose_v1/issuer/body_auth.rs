//! Exact reviewed issuer-stub authentication.
//! This observes source shape. It neither issues a tile nor proves LDS allocation.

use rustc_abi::ExternAbi;
use rustc_hir::{Safety, def::DefKind};
use rustc_middle::mir::{Body, Const, ConstValue, Operand, START_BLOCK, TerminatorKind};
use rustc_middle::ty::{
    EarlyBinder, GenericParamDefKind, Instance, InstanceKind, Ty, TyCtxt, TyKind, TypeVisitableExt,
    TypingEnv,
};

use super::trusted_device_items::{self, TrustedDeviceItem};

#[path = "body.rs"]
mod shape;

const MESSAGE: &[u8] = b"internal error: entered unreachable code: gfx950 LDS transpose issuance requires authenticated lowering";

pub(super) fn observe_body<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    receiver: Ty<'tcx>,
    output: Ty<'tcx>,
) -> bool {
    if !matches!(instance.def, InstanceKind::Item(_))
        || trusted_device_items::classify(tcx, instance.def_id())
            != Some(TrustedDeviceItem::Gfx950LdsTransposeTileIssue)
        || body.source.instance != instance.def
        || body.local_decls.len() != 4
        || body.basic_blocks.len() != 2
        || body.arg_count != 1
        || body.basic_blocks.iter().any(|block| {
            block.is_cleanup || block.terminator.is_none() || !block.statements.is_empty()
        })
    {
        return false;
    }
    let TerminatorKind::Call { func, args, .. } = &body.basic_blocks[START_BLOCK].terminator().kind
    else {
        return false;
    };
    let Some(format) = exact_callee(tcx, instance, func) else {
        return false;
    };
    let Some(arguments) = format_arguments_constructor(tcx, format) else {
        return false;
    };
    let [argument] = args.as_ref() else {
        return false;
    };
    let Operand::Constant(message) = &argument.node else {
        return false;
    };
    let Const::Val(value, message_ty) = message.const_ else {
        return false;
    };
    if !matches!(message_ty.kind(), TyKind::Ref(_, ty, rustc_hir::Mutability::Not)
        if *ty == tcx.types.str_)
        || value.try_get_slice_bytes_for_diagnostics(tcx) != Some(MESSAGE)
    {
        return false;
    }
    let TerminatorKind::Call { func, .. } = &body.basic_blocks
        [rustc_middle::mir::BasicBlock::from_usize(1)]
    .terminator()
    .kind
    else {
        return false;
    };
    let Some(panic) = exact_callee(tcx, instance, func) else {
        return false;
    };
    if !panic_format(tcx, panic, arguments) {
        return false;
    }
    // The supplied identities/message are now checked observations, not authority inputs.
    shape::issuer_stub(
        tcx, instance, body, receiver, output, format, panic, message,
    )
}

fn exact_callee<'tcx>(
    tcx: TyCtxt<'tcx>,
    caller: Instance<'tcx>,
    operand: &Operand<'tcx>,
) -> Option<Instance<'tcx>> {
    let Operand::Constant(function) = operand else {
        return None;
    };
    let Const::Val(ConstValue::ZeroSized, ty) = function.const_ else {
        return None;
    };
    let ty = caller
        .try_instantiate_mir_and_normalize_erasing_regions(
            tcx,
            TypingEnv::fully_monomorphized(),
            EarlyBinder::bind(ty),
        )
        .ok()?;
    let TyKind::FnDef(definition, arguments) = *ty.kind() else {
        return None;
    };
    let result = Instance::try_resolve(
        tcx,
        TypingEnv::fully_monomorphized(),
        definition,
        tcx.erase_and_anonymize_regions(arguments),
    )
    .ok()??;
    (matches!(result.def, InstanceKind::Item(_)) && result.def_id() == definition).then_some(result)
}

fn format_arguments_constructor<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Option<Ty<'tcx>> {
    let core = tcx.lang_items().sized_trait()?.krate;
    let arguments = tcx.lang_items().format_arguments()?;
    let definition = instance.def_id();
    let implementation = tcx.impl_of_assoc(definition)?;
    let associated = tcx.opt_associated_item(definition)?;
    if definition.krate != core
        || arguments.krate != core
        || implementation.krate != core
        || tcx.crate_name(core).as_str() != "core"
        || tcx.def_kind(definition) != DefKind::AssocFn
        || tcx.item_name(definition).as_str() != "from_str_nonconst"
        || !associated.is_fn()
        || associated.trait_item_def_id().is_some()
        || tcx.impl_is_of_trait(implementation)
        || !tcx.inherent_impls(arguments).contains(&implementation)
        || instance.args.len() != 1
        || instance
            .args
            .iter()
            .any(|argument| argument.as_region().is_none())
    {
        return None;
    }
    let parent = tcx.generics_of(implementation);
    let method = tcx.generics_of(definition);
    let [lifetime] = parent.own_params.as_slice() else {
        return None;
    };
    if parent.parent.is_some()
        || parent.parent_count != 0
        || parent.has_self
        || lifetime.index != 0
        || lifetime.pure_wrt_drop
        || !matches!(lifetime.kind, GenericParamDefKind::Lifetime)
        || tcx.parent(lifetime.def_id) != implementation
        || method.parent != Some(implementation)
        || method.parent_count != 1
        || method.has_self
        || !method.own_params.is_empty()
    {
        return None;
    }
    let signature = signature(tcx, instance)?;
    let [input] = signature.inputs() else {
        return None;
    };
    let output = signature.output();
    let TyKind::Adt(output_definition, output_arguments) = *output.kind() else {
        return None;
    };
    if !matches!(input.kind(), TyKind::Ref(_, ty, rustc_hir::Mutability::Not)
        if *ty == tcx.types.str_)
        || output_definition.did() != arguments
        || output_arguments.len() != 1
        || output_arguments
            .iter()
            .any(|argument| argument.as_region().is_none())
        || tcx
            .try_normalize_erasing_regions(
                TypingEnv::fully_monomorphized(),
                tcx.type_of(implementation).instantiate(tcx, instance.args),
            )
            .ok()?
            != output
    {
        return None;
    }
    Some(output)
}

fn panic_format<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>, arguments: Ty<'tcx>) -> bool {
    let Some(core) = tcx.lang_items().sized_trait() else {
        return false;
    };
    if instance.def_id().krate != core.krate
        || Some(instance.def_id()) != tcx.lang_items().panic_fmt()
        || !instance.args.is_empty()
        || tcx.generics_of(instance.def_id()).count() != 0
        || tcx.def_kind(instance.def_id()) != DefKind::Fn
    {
        return false;
    }
    signature(tcx, instance).is_some_and(|signature| {
        signature.inputs() == [arguments] && signature.output() == tcx.types.never
    })
}

fn signature<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Option<rustc_middle::ty::FnSig<'tcx>> {
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    );
    let signature = tcx
        .try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), signature)
        .ok()?;
    (signature.safety == Safety::Safe
        && signature.abi == ExternAbi::Rust
        && !signature.c_variadic
        && !signature.has_non_region_param()
        && !signature.has_aliases()
        && !signature.has_infer()
        && !signature.has_escaping_bound_vars())
    .then_some(signature)
}
