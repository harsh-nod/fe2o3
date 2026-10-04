//! Concrete core From identity. No item path or method-name matching.
use super::*;

pub(crate) fn primitive_from_candidate_v1<'tcx, E>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<Option<(Ty<'tcx>, Ty<'tcx>)>, E> {
    if !matches!(instance.def, InstanceKind::Item(_)) || !instance.args.is_empty() {
        return Ok(None);
    }
    charge(5)?;
    let Some(core) = tcx.lang_items().sized_trait().map(|id| id.krate) else {
        return Ok(None);
    };
    let Some(from) = tcx.get_diagnostic_item(Symbol::intern("From")) else {
        return Ok(None);
    };
    let Some(from_fn) = tcx.get_diagnostic_item(Symbol::intern("from_fn")) else {
        return Ok(None);
    };
    let Some(associated) = tcx.opt_associated_item(instance.def_id()) else {
        return Ok(None);
    };
    let Some(implementation) = tcx.impl_of_assoc(instance.def_id()) else {
        return Ok(None);
    };
    charge(12)?;
    if [
        instance.def_id().krate,
        implementation.krate,
        from.krate,
        from_fn.krate,
    ]
    .iter()
    .any(|id| *id != core)
        || tcx.crate_name(core).as_str() != "core"
        || !associated.is_fn()
        || associated.is_method()
        || associated.impl_container(tcx) != Some(implementation)
        || associated.trait_item_def_id() != Some(from_fn)
        || tcx.parent(from_fn) != from
        || !tcx.impl_is_of_trait(implementation)
        || tcx.impl_trait_id(implementation) != from
        || tcx.generics_of(instance.def_id()).count() != 0
        || !tcx.is_mir_available(instance.def_id())
    {
        return Ok(None);
    }
    charge(2)?;
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    );
    if signature.safety != Safety::Safe
        || signature.abi != ExternAbi::Rust
        || signature.c_variadic
        || signature.inputs().len() != 1
    {
        return Ok(None);
    }
    let input = signature.inputs()[0];
    let output = signature.output();
    if !crate::production_primitive_from_v1::primitive_widening_types_v1(input, output) {
        return Ok(None);
    }
    charge(2)?;
    let trait_ref = tcx
        .impl_trait_ref(implementation)
        .instantiate(tcx, instance.args);
    Ok((trait_ref.args.len() == 2
        && trait_ref.self_ty() == output
        && trait_ref.args.type_at(1) == input)
        .then_some((input, output)))
}
