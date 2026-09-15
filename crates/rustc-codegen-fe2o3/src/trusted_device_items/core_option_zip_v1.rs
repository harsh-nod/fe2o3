//! Source observation only for the exact core Option::zip implementation.
//! Original MIR and concrete drop glue remain recursively admitted by callers.

use rustc_abi::{ExternAbi, VariantIdx};
use rustc_hir::{Safety, def::DefKind, def_id::DefId};
use rustc_middle::mir::{Body, Operand};
use rustc_middle::ty::{
    EarlyBinder, GenericParamDefKind, Instance, InstanceKind, Ty, TyCtxt, TyKind, TypeVisitableExt,
    TypingEnv,
};

#[path = "core_option_zip_v1/body.rs"]
mod body;

const MAX_LOCALS: usize = 32;
const MAX_BLOCKS: usize = 32;
const MAX_STATEMENTS: usize = 128;
const MAX_WORK: usize = 2048;

struct Contract<'tcx> {
    instance: Instance<'tcx>,
    option: DefId,
    some: VariantIdx,
    none: VariantIdx,
    discriminants: [u128; 2],
    parameters: [Ty<'tcx>; 2],
    inputs: [Ty<'tcx>; 2],
    input_pair: Ty<'tcx>,
    payload_pair: Ty<'tcx>,
    output: Ty<'tcx>,
}

pub(crate) fn authenticate_reviewed_safe_core_option_zip_helper_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> bool {
    let Some(contract) = contract(tcx, instance) else {
        return false;
    };
    body::reviewed(tcx, instance, tcx.instance_mir(instance.def), &contract)
}

fn option_ty<'tcx>(tcx: TyCtxt<'tcx>, option: DefId, payload: Ty<'tcx>) -> Ty<'tcx> {
    Ty::new_adt(tcx, tcx.adt_def(option), tcx.mk_args(&[payload.into()]))
}

fn normalized<'tcx, T>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>, value: T) -> Option<T>
where
    T: rustc_middle::ty::TypeFoldable<TyCtxt<'tcx>>,
{
    instance
        .try_instantiate_mir_and_normalize_erasing_regions(
            tcx,
            TypingEnv::fully_monomorphized(),
            EarlyBinder::bind(value),
        )
        .ok()
}

fn contract<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> Option<Contract<'tcx>> {
    let core = tcx.lang_items().sized_trait()?.krate;
    let option = tcx.lang_items().option_type()?;
    let some_id = tcx.lang_items().option_some_variant()?;
    let none_id = tcx.lang_items().option_none_variant()?;
    let definition = instance.def_id();
    if !matches!(instance.def, InstanceKind::Item(_))
        || definition.krate != core
        || option.krate != core
        || tcx.crate_name(core).as_str() != "core"
        || tcx.parent(some_id) != option
        || tcx.parent(none_id) != option
        || tcx.def_kind(definition) != DefKind::AssocFn
        || tcx.item_name(definition).as_str() != "zip"
        || instance.args.len() != 2
        || instance.args.iter().any(|a| a.as_type().is_none())
        || !tcx.is_mir_available(definition)
    {
        return None;
    }
    let item = tcx.opt_associated_item(definition)?;
    let implementation = tcx.impl_of_assoc(definition)?;
    if !item.is_fn()
        || item.trait_item_def_id().is_some()
        || implementation.krate != core
        || tcx.impl_is_of_trait(implementation)
        || !tcx.inherent_impls(option).contains(&implementation)
    {
        return None;
    }
    let parent = tcx.generics_of(implementation);
    let method = tcx.generics_of(definition);
    if parent.parent.is_some()
        || parent.parent_count != 0
        || parent.has_self
        || parent.own_params.len() != 1
        || method.parent != Some(implementation)
        || method.parent_count != 1
        || method.has_self
        || method.own_params.len() != 1
    {
        return None;
    }
    let mut parameters = [tcx.types.unit; 2];
    for (i, parameter) in parent
        .own_params
        .iter()
        .chain(&method.own_params)
        .enumerate()
    {
        if parameter.index as usize != i
            || parameter.pure_wrt_drop
            || tcx.parent(parameter.def_id) != if i == 0 { implementation } else { definition }
            || !matches!(
                parameter.kind,
                GenericParamDefKind::Type {
                    has_default: false,
                    synthetic: false
                }
            )
        {
            return None;
        }
        parameters[i] = Ty::new_param(tcx, parameter.index, parameter.name);
        let concrete = normalized(tcx, instance, parameters[i])?;
        if concrete.has_non_region_param()
            || concrete.has_infer()
            || concrete.has_aliases()
            || concrete.has_escaping_bound_vars()
        {
            return None;
        }
    }
    let inputs = parameters.map(|p| option_ty(tcx, option, p));
    let input_pair = Ty::new_tup(tcx, &inputs);
    let payload_pair = Ty::new_tup(tcx, &parameters);
    let output = option_ty(tcx, option, payload_pair);
    let signature =
        tcx.instantiate_bound_regions_with_erased(tcx.fn_sig(definition).instantiate_identity());
    if signature.safety != Safety::Safe
        || signature.abi != ExternAbi::Rust
        || signature.c_variadic
        || signature.inputs() != inputs
        || signature.output() != output
        || tcx.type_of(implementation).instantiate_identity() != inputs[0]
    {
        return None;
    }
    let concrete = normalized(tcx, instance, signature)?;
    if concrete.safety != Safety::Safe
        || concrete.abi != ExternAbi::Rust
        || concrete.c_variadic
        || concrete.inputs()
            != [
                normalized(tcx, instance, inputs[0])?,
                normalized(tcx, instance, inputs[1])?,
            ]
        || concrete.output() != normalized(tcx, instance, output)?
    {
        return None;
    }
    let adt = tcx.adt_def(option);
    if !adt.is_enum()
        || adt.variants().len() != 2
        || tcx.generics_of(option).count() != 1
        || adt.destructor(tcx).is_some()
    {
        return None;
    }
    let (some, variant) = adt
        .variants()
        .iter_enumerated()
        .find(|(_, v)| v.def_id == some_id)?;
    let [field] = variant.fields.raw.as_slice() else {
        return None;
    };
    if tcx
        .type_of(field.did)
        .instantiate(tcx, tcx.mk_args(&[parameters[0].into()]))
        != parameters[0]
    {
        return None;
    }
    let (none, variant) = adt
        .variants()
        .iter_enumerated()
        .find(|(_, v)| v.def_id == none_id)?;
    if !variant.fields.is_empty() || some == none {
        return None;
    }
    Some(Contract {
        instance,
        option,
        some,
        none,
        discriminants: [
            adt.discriminant_for_variant(tcx, none).val,
            adt.discriminant_for_variant(tcx, some).val,
        ],
        parameters,
        inputs,
        input_pair,
        payload_pair,
        output,
    })
}

#[cfg(test)]
#[path = "core_option_zip_v1/tests.rs"]
mod tests;
