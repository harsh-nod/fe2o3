//! Side-effect-free policy audit for every raw call in a checked core helper.
//! Does not traverse a callee body, whitelist panic, or mutate the device graph.
use rustc_abi::{CanonAbi, ExternAbi};
use rustc_hir::Safety;
use rustc_middle::{
    mir::Operand,
    ty::{self, EarlyBinder, Instance, InstanceKind, TyCtxt, TyKind, TypingEnv},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RawCallRefusalV1 {
    Operand,
    Provider,
    Core,
    Resolve,
    Signature,
    Abi,
    Ffi,
}

pub(crate) fn audit_raw_call_v1<'tcx, E>(
    tcx: TyCtxt<'tcx>,
    caller: Instance<'tcx>,
    operand: &Operand<'tcx>,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<Result<Instance<'tcx>, RawCallRefusalV1>, E> {
    use RawCallRefusalV1 as R;
    charge(1)?;
    let Operand::Constant(constant) = operand else {
        return Ok(Err(R::Operand));
    };
    let TyKind::FnDef(original, args) = constant.const_.ty().kind() else {
        return Ok(Err(R::Operand));
    };
    let Some(core) = tcx.lang_items().sized_trait().map(|id| id.krate) else {
        return Ok(Err(R::Core));
    };
    charge(1)?;
    if original.krate != core || tcx.crate_name(core).as_str() != "core" {
        return Ok(Err(R::Core));
    }
    if crate::trusted_device_items::rejected_provider(tcx, *original).is_some() {
        return Ok(Err(R::Provider));
    }
    charge(1)?;
    let Ok(resolved) = crate::closure_profile_v1::resolve_direct_call(tcx, caller, operand) else {
        return Ok(Err(R::Resolve));
    };
    if !matches!(resolved.def, InstanceKind::Item(_))
        || !resolved.args.is_empty()
        || resolved.def_id().krate != core
    {
        return Ok(Err(R::Core));
    }
    if crate::trusted_device_items::rejected_provider(tcx, resolved.def_id()).is_some() {
        return Ok(Err(R::Provider));
    }
    charge(1)?;
    let Ok(args) = caller.try_instantiate_mir_and_normalize_erasing_regions(
        tcx,
        TypingEnv::fully_monomorphized(),
        EarlyBinder::bind(*args),
    ) else {
        return Ok(Err(R::Resolve));
    };
    let original_signature =
        tcx.instantiate_bound_regions_with_erased(tcx.fn_sig(*original).instantiate(tcx, args));
    charge(1)?;
    let Ok(original_signature) =
        tcx.try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), original_signature)
    else {
        return Ok(Err(R::Signature));
    };
    let Ok(signature) = crate::rustc_semantic_plan_v1::source_signature_v1(tcx, resolved) else {
        return Ok(Err(R::Signature));
    };
    if signature != original_signature
        || signature.safety != Safety::Safe
        || signature.abi != ExternAbi::Rust
        || signature.c_variadic
    {
        return Ok(Err(R::Signature));
    }
    for def in [*original, resolved.def_id()] {
        charge(1)?;
        if !matches!(
            crate::device_ffi::contract_assertion_for_def(tcx, def),
            Ok(None)
        ) {
            return Ok(Err(R::Ffi));
        }
    }
    charge(1)?;
    let Ok(abi) = tcx.fn_abi_of_instance(
        TypingEnv::fully_monomorphized().as_query_input((resolved, ty::List::empty())),
    ) else {
        return Ok(Err(R::Abi));
    };
    charge(1)?;
    let caller_location = if resolved.def.requires_caller_location(tcx) {
        charge(1)?;
        let Ok(layout) = tcx
            .layout_of(TypingEnv::fully_monomorphized().as_query_input(tcx.caller_location_ty()))
        else {
            return Ok(Err(R::Abi));
        };
        Some(layout.ty)
    } else {
        None
    };
    let Some(adjusted_count) = signature
        .inputs()
        .len()
        .checked_add(usize::from(caller_location.is_some()))
    else {
        return Ok(Err(R::Abi));
    };
    if abi.conv != CanonAbi::Rust
        || abi.c_variadic
        || abi.args.len() != adjusted_count
        || usize::try_from(abi.fixed_count).ok() != Some(signature.inputs().len())
        || abi.ret.layout.ty != signature.output()
    {
        return Ok(Err(R::Abi));
    }
    for (actual, expected) in abi
        .args
        .iter()
        .zip(signature.inputs().iter().copied().chain(caller_location))
    {
        charge(1)?;
        if actual.layout.ty != expected {
            return Ok(Err(R::Abi));
        }
    }
    Ok(Ok(resolved))
}
