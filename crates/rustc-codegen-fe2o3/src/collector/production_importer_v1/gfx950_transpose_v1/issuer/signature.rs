//! Exact original issuer signature and layout predicates.
//! The parent joins these facts with body, root and canonical source replay.

use super::*;
use rustc_abi::{BackendRepr, Variants};
use rustc_middle::ty::{GenericParamDefKind, InstanceKind, TypeVisitableExt, TypingEnv};

pub(super) struct IssuerSignatureLayoutV1<'tcx> {
    pub instance: Instance<'tcx>,
    pub source_identity: SemanticFunctionIdentityV1,
    pub partition_reference: Ty<'tcx>,
    pub partition: Ty<'tcx>,
    pub tile: Ty<'tcx>,
    pub format: SemanticGfx950LdsTransposeFormatV1,
    pub execution_brand: Ty<'tcx>,
    pub subgroup_brand: Ty<'tcx>,
    pub epoch: Ty<'tcx>,
}

pub(super) fn observe_signature_layout<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    root: &AuthenticatedProductionKernelContextRootV1,
    abi: &SemanticFunctionAbiV1,
    types: &[SemanticTypeDeclV1],
) -> Result<IssuerSignatureLayoutV1<'tcx>, ProductionSemanticImportErrorV1> {
    let rejected = || {
        ProductionSemanticImportErrorV1::KernelContextBinding(
            "LDS issuer exact original signature/layout",
        )
    };
    if !matches!(instance.def, InstanceKind::Item(_))
        || instance.args.len() != 5
        || instance.args.len() != tcx.generics_of(instance.def_id()).count()
        || instance.args.consts().next().is_some()
        || !tcx.is_mir_available(instance.def_id())
        || trusted_device_items::classify(tcx, instance.def_id())
            != Some(TrustedDeviceItem::Gfx950LdsTransposeTileIssue)
    {
        return Err(rejected());
    }
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    );
    let signature = tcx
        .try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), signature)
        .map_err(|_| rejected())?;
    if signature.safety != rustc_hir::Safety::Safe
        || signature.abi != rustc_abi::ExternAbi::Rust
        || signature.c_variadic
        || signature.has_non_region_param()
        || signature.has_infer()
        || signature.has_aliases()
        || signature.has_escaping_bound_vars()
    {
        return Err(rejected());
    }
    let [reference] = signature.inputs() else {
        return Err(rejected());
    };
    let partition = rust_shared_reference_v1(*reference).ok_or_else(rejected)?;
    if !original_method(tcx, instance, partition) {
        return Err(rejected());
    }
    let arguments = rust_trusted_adt_type_arguments_v1(
        tcx,
        partition,
        TrustedDeviceItem::Gfx950SubgroupContext,
    )
    .ok_or_else(rejected)?;
    let [execution, epoch] = arguments.as_slice() else {
        return Err(rejected());
    };
    let execution_root = rust_execution_brand_v1(tcx, *execution).ok_or_else(rejected)?;
    if !rust_kernel_brand_matches_root_v1(tcx, execution_root, root)
        || !partition_layout(tcx, partition, *execution, *epoch)
    {
        return Err(rejected());
    }
    let tile = signature.output();
    let arguments =
        rust_trusted_adt_type_arguments_v1(tcx, tile, TrustedDeviceItem::Gfx950LdsTransposeTile)
            .ok_or_else(rejected)?;
    let [format, state, subgroup] = arguments.as_slice() else {
        return Err(rejected());
    };
    let subgroup_arguments = rust_exact_reviewed_adt_arguments_v1(
        tcx,
        *subgroup,
        "fe2o3_device::execution::SubgroupBrand",
    )
    .ok_or_else(rejected)?
    .types()
    .collect::<Vec<_>>();
    let [width, subgroup_execution, subgroup_epoch] = subgroup_arguments.as_slice() else {
        return Err(rejected());
    };
    if rust_subgroup_width_v1(tcx, *width) != Some(64)
        || subgroup_execution != execution
        || subgroup_epoch != epoch
        || instance.args.types().collect::<Vec<_>>() != [*execution, *epoch, *format]
        || !uninitialized(tcx, *state)
        || !tile_layout(tcx, tile, *format, *state, *subgroup)
    {
        return Err(rejected());
    }
    let format =
        if rust_is_exact_trusted_marker_v1(tcx, *format, TrustedDeviceItem::Gfx950Fp4E2M1Format) {
            SemanticGfx950LdsTransposeFormatV1::Fp4E2M1
        } else if rust_is_exact_trusted_marker_v1(
            tcx,
            *format,
            TrustedDeviceItem::Gfx950Fp8E4M3Format,
        ) {
            SemanticGfx950LdsTransposeFormatV1::Fp8E4M3
        } else {
            return Err(rejected());
        };
    require_capability_memory_terminal_abi_v1(
        tcx,
        abi,
        types,
        signature.inputs(),
        tile,
        &[SemanticSourceArgumentOwnershipV1::SharedBorrow],
    )?;
    Ok(IssuerSignatureLayoutV1 {
        instance,
        source_identity: canonical_function_identities_v1(tcx, instance).function(),
        partition_reference: *reference,
        partition,
        tile,
        format,
        execution_brand: *execution,
        subgroup_brand: *subgroup,
        epoch: *epoch,
    })
}

fn original_method<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>, receiver: Ty<'tcx>) -> bool {
    let definition = instance.def_id();
    let Some(implementation) = tcx.impl_of_assoc(definition) else {
        return false;
    };
    let Some(associated) = tcx.opt_associated_item(definition) else {
        return false;
    };
    let TyKind::Adt(receiver_definition, _) = receiver.kind() else {
        return false;
    };
    if tcx.def_kind(definition) != rustc_hir::def::DefKind::AssocFn
        || tcx.item_name(definition).as_str() != "transpose_tile"
        || !associated.is_fn()
        || associated.trait_item_def_id().is_some()
        || tcx.impl_is_of_trait(implementation)
        || !tcx
            .inherent_impls(receiver_definition.did())
            .contains(&implementation)
    {
        return false;
    }
    let parent = tcx.generics_of(implementation);
    let method = tcx.generics_of(definition);
    if parent.parent.is_some()
        || parent.parent_count != 0
        || parent.has_self
        || parent.own_params.len() != 4
        || method.parent != Some(implementation)
        || method.parent_count != 4
        || method.has_self
        || method.own_params.len() != 1
    {
        return false;
    }
    for (index, parameter) in parent
        .own_params
        .iter()
        .chain(&method.own_params)
        .enumerate()
    {
        if parameter.index as usize != index
            || parameter.pure_wrt_drop
            || tcx.parent(parameter.def_id)
                != if index < 4 {
                    implementation
                } else {
                    definition
                }
            || if index < 2 {
                !matches!(parameter.kind, GenericParamDefKind::Lifetime)
                    || instance.args[index].as_region().is_none()
            } else {
                !matches!(
                    parameter.kind,
                    GenericParamDefKind::Type {
                        has_default: false,
                        synthetic: false
                    }
                ) || instance.args[index].as_type().is_none()
            }
        {
            return false;
        }
    }
    tcx.try_normalize_erasing_regions(
        TypingEnv::fully_monomorphized(),
        tcx.type_of(implementation).instantiate(tcx, instance.args),
    )
    .ok()
        == Some(receiver)
}

fn partition_layout<'tcx>(
    tcx: TyCtxt<'tcx>,
    partition: Ty<'tcx>,
    execution: Ty<'tcx>,
    epoch: Ty<'tcx>,
) -> bool {
    let Some(fields) = two_fields(tcx, partition) else {
        return false;
    };
    let Some(contract) = phantom(tcx, fields[0]) else {
        return false;
    };
    let TyKind::FnPtr(signature, _) = contract.kind() else {
        return false;
    };
    let signature_types = signature.skip_binder().inputs_and_output;
    let [input, output] = signature_types.as_slice() else {
        return false;
    };
    let TyKind::Tuple(pair) = input.kind() else {
        return false;
    };
    let [subgroup_reference, epoch_reference] = pair.as_slice() else {
        return false;
    };
    let Some(subgroup) =
        rust_shared_reference_v1(*subgroup_reference).and_then(|ty| rust_subgroup_v1(tcx, ty))
    else {
        return false;
    };
    let Some(epoch_value) =
        rust_shared_reference_v1(*epoch_reference).and_then(|ty| rust_workgroup_epoch_v1(tcx, ty))
    else {
        return false;
    };
    input == output
        && invariant(contract, *input)
        && subgroup.width == 64
        && subgroup.kernel_brand.ty == execution
        && subgroup.epoch == epoch
        && epoch_value.kernel_brand.ty == execution
        && epoch_value.epoch == epoch
        && phantom(tcx, fields[1]).is_some_and(|ty| {
            matches!(ty.kind(),
            TyKind::RawPtr(unit, rustc_hir::Mutability::Mut) if *unit == tcx.types.unit)
        })
        && zst_two_fields(tcx, partition)
}

fn two_fields<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> Option<[Ty<'tcx>; 2]> {
    let TyKind::Adt(definition, arguments) = *ty.kind() else {
        return None;
    };
    if !definition.is_struct() || definition.non_enum_variant().fields.len() != 2 {
        return None;
    }
    let fields = definition
        .non_enum_variant()
        .fields
        .iter()
        .map(|field| {
            tcx.try_normalize_erasing_regions(
                TypingEnv::fully_monomorphized(),
                field.ty(tcx, arguments),
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    fields.try_into().ok()
}

fn zst_two_fields<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> bool {
    let Ok(layout) = tcx.layout_of(TypingEnv::fully_monomorphized().as_query_input(ty)) else {
        return false;
    };
    layout.size.bytes() == 0
        && layout.align.abi.bytes() == 1
        && layout.backend_repr == (BackendRepr::Memory { sized: true })
        && !layout.uninhabited
        && matches!(layout.variants, Variants::Single { index } if index.as_u32() == 0)
        && layout.fields.count() == 2
        && layout.fields.offset(0).bytes() == 0
        && layout.fields.offset(1).bytes() == 0
}

fn uninitialized<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> bool {
    let Some(arguments) = rust_exact_reviewed_adt_arguments_v1(
        tcx,
        ty,
        "fe2o3_device::gfx950::Gfx950TransposeUninitialized",
    ) else {
        return false;
    };
    matches!(ty.kind(), TyKind::Adt(definition, _)
        if arguments.is_empty() && definition.is_enum() && definition.variants().is_empty())
}

fn phantom<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> Option<Ty<'tcx>> {
    let TyKind::Adt(definition, arguments) = *ty.kind() else {
        return None;
    };
    if Some(definition.did()) != tcx.lang_items().phantom_data() || arguments.len() != 1 {
        return None;
    }
    arguments[0].as_type()
}

fn invariant<'tcx>(ty: Ty<'tcx>, marker: Ty<'tcx>) -> bool {
    matches!(ty.kind(), TyKind::FnPtr(signature, header)
        if signature.bound_vars().is_empty()
        && header.safety == rustc_hir::Safety::Safe
        && header.abi == rustc_abi::ExternAbi::Rust
        && !header.c_variadic
        && signature.skip_binder().inputs_and_output.as_slice() == [marker, marker])
}

pub(super) fn tile_layout<'tcx>(
    tcx: TyCtxt<'tcx>,
    tile: Ty<'tcx>,
    format: Ty<'tcx>,
    state: Ty<'tcx>,
    brand: Ty<'tcx>,
) -> bool {
    let Some(fields) = two_fields(tcx, tile) else {
        return false;
    };
    let Some(contract) = phantom(tcx, fields[0]) else {
        return false;
    };
    let TyKind::Tuple(contract) = contract.kind() else {
        return false;
    };
    let [lifetime, association] = contract.as_slice() else {
        return false;
    };
    let TyKind::FnPtr(signature, _) = lifetime.kind() else {
        return false;
    };
    let signature_types = signature.skip_binder().inputs_and_output;
    let [input, output] = signature_types.as_slice() else {
        return false;
    };
    if input != output
        || !matches!(input.kind(), TyKind::Ref(_, unit, rustc_hir::Mutability::Mut) if *unit == tcx.types.unit)
        || !invariant(*lifetime, *input)
        || !invariant(
            *association,
            Ty::new_tup(tcx, &[Ty::new_tup(tcx, &[format, state]), brand]),
        )
        || !phantom(tcx, fields[1]).is_some_and(|ty| {
            matches!(ty.kind(),
            TyKind::RawPtr(unit, rustc_hir::Mutability::Mut) if *unit == tcx.types.unit)
        })
    {
        return false;
    }
    zst_two_fields(tcx, tile)
}
