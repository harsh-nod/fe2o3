//! Exact source-helper authentication for core `Result::map_err`.
//! The original MIR, callback and drop glue remain recursively collected.
//! This is neither a terminal operation nor source trust for an arbitrary FnOnce.

use rustc_abi::{ExternAbi, VariantIdx};
use rustc_hir::{Safety, def::DefKind, def_id::DefId};
use rustc_middle::mir::{Body, Const, ConstValue, Operand};
use rustc_middle::ty::{
    EarlyBinder, FnSig, GenericParamDefKind, Instance, InstanceKind, Ty, TyCtxt, TyKind,
    TypeVisitableExt, TypingEnv,
};

#[path = "core_result_map_v1/body.rs"]
mod body;
use body::reviewed_body;

const MAX_LOCALS: usize = 32;
const MAX_BLOCKS: usize = 32;
const MAX_STATEMENTS: usize = 96;
const MAX_SCOPES: usize = 8;
const MAX_DEBUG_INFO: usize = 32;
const MAX_MENTIONED_ITEMS: usize = 3;
const MAX_WORK: usize = 1024;

struct Contract<'tcx> {
    instance: Instance<'tcx>,
    result: DefId,
    variants: [VariantIdx; 2],
    discriminants: [u128; 2],
    // Raw parameters must stay distinct even when concrete T, E and F coincide.
    parameters: [Ty<'tcx>; 4],
    concrete: [Ty<'tcx>; 4],
    input: Ty<'tcx>,
    output: Ty<'tcx>,
    tuple: Ty<'tcx>,
    callback: Ty<'tcx>,
    callee: Instance<'tcx>,
}

/// Discharges only this exact core wrapper's unavailable source observation.
/// Callers must retain its generic MIR and recursively admit the resolved call
/// and drops. No body is cloned, hashed, specialized, pruned or replaced here.
pub(crate) fn authenticate_reviewed_safe_core_result_map_helper_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> bool {
    let Some(contract) = contract(tcx, instance) else {
        return false;
    };
    reviewed_body(tcx, instance, tcx.instance_mir(instance.def), &contract)
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

fn result_ty<'tcx>(tcx: TyCtxt<'tcx>, result: DefId, t: Ty<'tcx>, e: Ty<'tcx>) -> Ty<'tcx> {
    Ty::new_adt(tcx, tcx.adt_def(result), tcx.mk_args(&[t.into(), e.into()]))
}

fn signature_matches<'tcx>(
    signature: FnSig<'tcx>,
    input: Ty<'tcx>,
    callback: Ty<'tcx>,
    output: Ty<'tcx>,
    abi: ExternAbi,
) -> bool {
    signature.safety == Safety::Safe
        && signature.abi == abi
        && !signature.c_variadic
        && signature.inputs() == [input, callback]
        && signature.output() == output
}

fn contract<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> Option<Contract<'tcx>> {
    let core = tcx.lang_items().sized_trait()?.krate;
    let definition = instance.def_id();
    let ok = tcx.lang_items().result_ok_variant()?;
    let err = tcx.lang_items().result_err_variant()?;
    let result = tcx.parent(ok);
    if !matches!(instance.def, InstanceKind::Item(_))
        || definition.krate != core
        || result.krate != core
        || tcx.parent(err) != result
        || tcx.crate_name(core).as_str() != "core"
        || tcx.def_kind(definition) != DefKind::AssocFn
        || tcx.item_name(definition).as_str() != "map_err"
        || instance.args.len() != 4
        || instance.args.iter().any(|arg| arg.as_type().is_none())
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
        || !tcx.inherent_impls(result).contains(&implementation)
    {
        return None;
    }
    let parent = tcx.generics_of(implementation);
    let method = tcx.generics_of(definition);
    if parent.parent.is_some()
        || parent.parent_count != 0
        || parent.has_self
        || parent.own_params.len() != 2
        || method.parent != Some(implementation)
        || method.parent_count != 2
        || method.has_self
        || method.own_params.len() != 2
    {
        return None;
    }
    let mut parameters = [tcx.types.unit; 4];
    for (index, parameter) in parent
        .own_params
        .iter()
        .chain(&method.own_params)
        .enumerate()
    {
        if parameter.index as usize != index
            || parameter.pure_wrt_drop
            || tcx.parent(parameter.def_id)
                != if index < 2 {
                    implementation
                } else {
                    definition
                }
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
        parameters[index] = Ty::new_param(tcx, parameter.index, parameter.name);
    }
    let [t, e, f, o] = parameters;
    let input = result_ty(tcx, result, t, e);
    let output = result_ty(tcx, result, t, f);
    let signature =
        tcx.instantiate_bound_regions_with_erased(tcx.fn_sig(definition).instantiate_identity());
    if tcx.type_of(implementation).instantiate_identity() != input
        || !signature_matches(signature, input, o, output, ExternAbi::Rust)
    {
        return None;
    }
    let adt = tcx.adt_def(result);
    if !adt.is_enum()
        || adt.variants().len() != 2
        || tcx.generics_of(result).count() != 2
        || adt.destructor(tcx).is_some()
    {
        return None;
    }
    let mut variants = [VariantIdx::from_usize(0); 2];
    let mut discriminants = [0; 2];
    for (role, (id, field_ty)) in [(ok, t), (err, e)].into_iter().enumerate() {
        let (index, variant) = adt
            .variants()
            .iter_enumerated()
            .find(|(_, variant)| variant.def_id == id)?;
        let [field] = variant.fields.raw.as_slice() else {
            return None;
        };
        if tcx
            .type_of(field.did)
            .instantiate(tcx, tcx.mk_args(&[t.into(), e.into()]))
            != field_ty
        {
            return None;
        }
        variants[role] = index;
        discriminants[role] = adt.discriminant_for_variant(tcx, index).val;
    }
    if variants[0] == variants[1] || discriminants[0] == discriminants[1] {
        return None;
    }
    let mut concrete = [tcx.types.unit; 4];
    for (index, parameter) in parameters.into_iter().enumerate() {
        let ty = normalized(tcx, instance, parameter)?;
        if ty.has_non_region_param()
            || ty.has_infer()
            || ty.has_aliases()
            || ty.has_escaping_bound_vars()
            || ty.references_error()
            || !ty.is_sized(tcx, TypingEnv::fully_monomorphized())
        {
            return None;
        }
        concrete[index] = ty;
    }
    if !signature_matches(
        normalized(tcx, instance, signature)?,
        normalized(tcx, instance, input)?,
        concrete[3],
        normalized(tcx, instance, output)?,
        ExternAbi::Rust,
    ) {
        return None;
    }
    let fn_once = tcx.lang_items().fn_once_trait()?;
    if fn_once.krate != core {
        return None;
    }
    let call_once = tcx
        .associated_items(fn_once)
        .in_definition_order()
        .find(|item| item.is_fn() && item.name().as_str() == "call_once")?
        .def_id;
    let tuple = Ty::new_tup(tcx, &[e]);
    let args = tcx.mk_args(&[o.into(), tuple.into()]);
    let callback = Ty::new_fn_def(tcx, call_once, args);
    let concrete_args = normalized(tcx, instance, args)?;
    let callback_signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(call_once).instantiate(tcx, concrete_args),
    );
    let callback_signature = tcx
        .try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), callback_signature)
        .ok()?;
    if !signature_matches(
        callback_signature,
        concrete[3],
        Ty::new_tup(tcx, &[concrete[1]]),
        concrete[2],
        ExternAbi::RustCall,
    ) {
        return None;
    }
    let callee = Instance::try_resolve(
        tcx,
        TypingEnv::fully_monomorphized(),
        call_once,
        concrete_args,
    )
    .ok()??;
    if callee.args.has_non_region_param()
        || callee.args.has_infer()
        || callee.args.has_aliases()
        || callee.args.has_escaping_bound_vars()
    {
        return None;
    }
    Some(Contract {
        instance,
        result,
        variants,
        discriminants,
        parameters,
        concrete,
        input,
        output,
        tuple,
        callback,
        callee,
    })
}

fn exact_callback<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    operand: &Operand<'tcx>,
    contract: &Contract<'tcx>,
) -> bool {
    let Operand::Constant(callee) = operand else {
        return false;
    };
    if callee.user_ty.is_some()
        || !matches!(callee.const_, Const::Val(ConstValue::ZeroSized, ty) if ty == contract.callback)
    {
        return false;
    }
    let TyKind::FnDef(definition, args) = *contract.callback.kind() else {
        return false;
    };
    let Some(args) = normalized(tcx, instance, args) else {
        return false;
    };
    Instance::try_resolve(tcx, TypingEnv::fully_monomorphized(), definition, args)
        .is_ok_and(|resolved| resolved == Some(contract.callee))
}

#[cfg(test)]
#[path = "core_result_map_v1/tests.rs"]
mod tests;
