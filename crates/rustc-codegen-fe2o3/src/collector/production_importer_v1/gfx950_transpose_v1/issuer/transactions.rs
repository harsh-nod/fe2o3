//! Reconstructs all owned transitions from the reviewed original source ABI.
use super::*;
use crate::production_semantic_terminal_v1::ProductionExecutionTerminalV1 as Terminal;
use SemanticGfx950TransposeOperationV1 as T;
use rustc_middle::ty::{InstanceKind, TypeVisitableExt, TypingEnv};

fn rejected() -> ProductionSemanticImportErrorV1 {
    ProductionSemanticImportErrorV1::KernelContextBinding(
        "gfx950 transpose exact source transaction",
    )
}

fn rejected_at(detail: &'static str) -> ProductionSemanticImportErrorV1 {
    ProductionSemanticImportErrorV1::KernelContextBinding(detail)
}

#[derive(Clone, Copy)]
struct Tile<'tcx> {
    format: Ty<'tcx>,
    brand: Ty<'tcx>,
    execution: Ty<'tcx>,
    epoch: Ty<'tcx>,
}

fn tile<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>, state: &'static str) -> Option<Tile<'tcx>> {
    let args =
        rust_trusted_adt_type_arguments_v1(tcx, ty, TrustedDeviceItem::Gfx950LdsTransposeTile)?;
    let [format, state_ty, brand] = args.as_slice() else {
        return None;
    };
    let state_args = rust_exact_reviewed_adt_arguments_v1(tcx, *state_ty, state)?;
    if !state_args.is_empty()
        || !matches!(state_ty.kind(), TyKind::Adt(def, _) if def.is_enum()
        && def.variants().is_empty())
        || !signature::tile_layout(tcx, ty, *format, *state_ty, *brand)
    {
        return None;
    }
    let subgroup = rust_exact_reviewed_adt_arguments_v1(
        tcx,
        *brand,
        "fe2o3_device::execution::SubgroupBrand",
    )?
    .types()
    .collect::<Vec<_>>();
    let [width, execution, epoch] = subgroup.as_slice() else {
        return None;
    };
    (rust_subgroup_width_v1(tcx, *width) == Some(64)).then_some(Tile {
        format: *format,
        brand: *brand,
        execution: *execution,
        epoch: *epoch,
    })
}

fn canonical_field(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    index: usize,
) -> Result<SemanticTypeIdV1, ProductionSemanticImportErrorV1> {
    let Some(SemanticTypeShapeV1::Aggregate(fields)) =
        types.get(ty.index() as usize).map(|ty| ty.shape())
    else {
        return Err(rejected());
    };
    fields.fields().get(index).copied().ok_or_else(rejected)
}

fn canonical_publish_fields(
    types: &[SemanticTypeDeclV1],
    transition: SemanticTypeIdV1,
) -> Result<[SemanticTypeIdV1; 2], ProductionSemanticImportErrorV1> {
    let Some(SemanticTypeShapeV1::Tuple(tuple)) =
        types.get(transition.index() as usize).map(SemanticTypeDeclV1::shape)
    else { return Err(rejected()) };
    let [workgroup, tile] = tuple.fields() else { return Err(rejected()) };
    Ok([*workgroup, *tile])
}

fn exact<'tcx>(
    tcx: TyCtxt<'tcx>,
    types: &[SemanticTypeDeclV1],
    id: SemanticTypeIdV1,
    ty: Ty<'tcx>,
) -> Result<SemanticTypeIdV1, ProductionSemanticImportErrorV1> {
    if types
        .get(id.index() as usize)
        .is_none_or(|decl| decl.identity() != rustc_type_identity_v1(tcx, ty))
    {
        return Err(rejected());
    }
    Ok(id)
}

fn field<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>, index: usize) -> Option<Ty<'tcx>> {
    let TyKind::Adt(definition, arguments) = *ty.kind() else {
        return None;
    };
    if !definition.is_struct() {
        return None;
    }
    let field = definition.non_enum_variant().fields.iter().nth(index)?;
    tcx.try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), field.ty(tcx, arguments))
        .ok()
}

fn inherent<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>, receiver: Ty<'tcx>) -> bool {
    let Some(implementation) = tcx.impl_of_assoc(instance.def_id()) else {
        return false;
    };
    let Some(item) = tcx.opt_associated_item(instance.def_id()) else {
        return false;
    };
    let TyKind::Adt(definition, _) = receiver.kind() else {
        return false;
    };
    item.is_fn()
        && item.trait_item_def_id().is_none()
        && !tcx.impl_is_of_trait(implementation)
        && tcx
            .inherent_impls(definition.did())
            .contains(&implementation)
        && tcx
            .try_normalize_erasing_regions(
                TypingEnv::fully_monomorphized(),
                tcx.type_of(implementation).instantiate(tcx, instance.args),
            )
            .ok()
            == Some(receiver)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn operation<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    terminal: Terminal,
    abi: &SemanticFunctionAbiV1,
    types: &[SemanticTypeDeclV1],
    root: &AuthenticatedProductionKernelContextRootV1,
    source: SemanticFunctionIdentityV1,
    contexts: &AuthenticatedProductionKernelContextsV1,
) -> Result<SemanticCompilerIntrinsicOperationV1, ProductionSemanticImportErrorV1> {
    if !matches!(instance.def, InstanceKind::Item(_))
        || !tcx.is_mir_available(instance.def_id())
        || instance.args.len() != tcx.generics_of(instance.def_id()).count()
        || instance.args.consts().next().is_some()
        || canonical_function_identities_v1(tcx, instance).function() != source
        || trusted_device_items::classify(tcx, instance.def_id())
            != Some(terminal.trusted_device_item())
        || !trusted_device_items::authenticate_reviewed_safe_external_helper_v1(
            tcx,
            instance.def_id(),
        )
        .map_err(|_| rejected())?
    {
        return Err(rejected_at("gfx950 transpose exact source transaction: original trusted item/instance identity"));
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
        return Err(rejected_at("gfx950 transpose exact source transaction: normalized safe monomorphic Rust signature"));
    }
    let inputs = signature.inputs();
    if inputs.len() != terminal.source_argument_count()
        || abi.source_input_types().len() != inputs.len()
        || !inherent(tcx, instance, inputs[0])
    {
        return Err(rejected_at("gfx950 transpose exact source transaction: source arity or inherent receiver"));
    }
    let output = signature.output();
    let ids = abi.source_input_types();
    let result = abi.source_output_type();
    let before_state = match terminal {
        Terminal::Gfx950TransposeStageB4 | Terminal::Gfx950TransposeStageB8 => {
            "fe2o3_device::gfx950::Gfx950TransposeUninitialized"
        }
        Terminal::Gfx950TransposePublish => "fe2o3_device::gfx950::Gfx950TransposeStaged",
        Terminal::Gfx950TransposeReadB4 | Terminal::Gfx950TransposeReadB8 => {
            "fe2o3_device::gfx950::Gfx950TransposePublished"
        }
        _ => return Err(rejected()),
    };
    let before = tile(tcx, inputs[0], before_state).ok_or_else(|| rejected_at("gfx950 transpose exact source transaction: input tile state/brand/layout"))?;
    let execution = rust_execution_brand_v1(tcx, before.execution).ok_or_else(|| rejected_at("gfx950 transpose exact source transaction: execution brand"))?;
    if !rust_kernel_brand_matches_root_v1(tcx, execution, root) {
        return Err(rejected_at("gfx950 transpose exact source transaction: execution root"));
    }
    let format = if rust_is_exact_trusted_marker_v1(
        tcx,
        before.format,
        TrustedDeviceItem::Gfx950Fp4E2M1Format,
    ) {
        SemanticGfx950LdsTransposeFormatV1::Fp4E2M1
    } else if rust_is_exact_trusted_marker_v1(
        tcx,
        before.format,
        TrustedDeviceItem::Gfx950Fp8E4M3Format,
    ) {
        SemanticGfx950LdsTransposeFormatV1::Fp8E4M3
    } else {
        return Err(rejected());
    };
    if matches!(
        terminal,
        Terminal::Gfx950TransposeStageB4 | Terminal::Gfx950TransposeReadB4
    ) && format != SemanticGfx950LdsTransposeFormatV1::Fp4E2M1
        || matches!(
            terminal,
            Terminal::Gfx950TransposeStageB8 | Terminal::Gfx950TransposeReadB8
        ) && format != SemanticGfx950LdsTransposeFormatV1::Fp8E4M3
    {
        return Err(rejected());
    }
    let mut after_epoch = None;
    let mut after_brand = None;
    use SemanticSourceArgumentOwnershipV1 as Own;
    let (operation, ownership): (_, &[Own]) = match terminal {
        Terminal::Gfx950TransposeStageB4 | Terminal::Gfx950TransposeStageB8 => {
            let after = tile(tcx, output, "fe2o3_device::gfx950::Gfx950TransposeStaged")
                .ok_or_else(|| rejected_at("gfx950 transpose exact source transaction: Stage output tile state/brand/layout"))?;
            let view = rust_shared_reference_v1(inputs[1]).ok_or_else(|| rejected_at("gfx950 transpose exact source transaction: Stage shared view reference"))?;
            let args = rust_trusted_adt_type_arguments_v1(
                tcx,
                view,
                TrustedDeviceItem::Gfx950MfmaGlobalMatrixView,
            )
            .ok_or_else(|| rejected_at("gfx950 transpose exact source transaction: Stage trusted view type"))?;
            let [view_format, role, matrix_brand, global_brand] = args.as_slice() else {
                return Err(rejected_at("gfx950 transpose exact source transaction: Stage view generic arity"));
            };
            let global_root = rust_kernel_brand_v1(tcx, *global_brand).ok_or_else(|| rejected_at("gfx950 transpose exact source transaction: Stage Global brand"))?;
            if inputs[2..] != [tcx.types.usize; 2] {
                return Err(rejected_at("gfx950 transpose exact source transaction: Stage index types"));
            }
            if before.format != after.format {
                return Err(rejected_at("gfx950 transpose exact source transaction: Stage output format"));
            }
            if before.brand != after.brand {
                return Err(rejected_at("gfx950 transpose exact source transaction: Stage output subgroup brand"));
            }
            if before.execution != after.execution {
                return Err(rejected_at("gfx950 transpose exact source transaction: Stage output execution brand"));
            }
            if before.epoch != after.epoch {
                return Err(rejected_at("gfx950 transpose exact source transaction: Stage output epoch"));
            }
            if *view_format != before.format {
                return Err(rejected_at("gfx950 transpose exact source transaction: Stage view format"));
            }
            if *matrix_brand != before.brand {
                return Err(rejected_at("gfx950 transpose exact source transaction: Stage view subgroup brand"));
            }
            if !rust_is_exact_trusted_marker_v1(tcx, *role, TrustedDeviceItem::Gfx950MfmaOperandA) {
                return Err(rejected_at("gfx950 transpose exact source transaction: Stage view operand role"));
            }
            if !rust_kernel_brand_matches_root_v1(tcx, global_root, root) {
                return Err(rejected_at("gfx950 transpose exact source transaction: Stage Global root"));
            }
            if numerical_policy_v1::transpose_global_root_v1(tcx, before.brand) != Some(global_root.ty) {
                return Err(rejected_at("gfx950 transpose exact source transaction: Stage sealed Matrix-to-Global root"));
            }
            if instance.args.types().collect::<Vec<_>>() != [before.brand, *global_brand] {
                return Err(rejected_at("gfx950 transpose exact source transaction: Stage instance type arguments"));
            }
            let global_reference = field(tcx, view, 0).ok_or_else(|| rejected_at("gfx950 transpose exact source transaction: Stage original view field"))?;
            let global = rust_shared_reference_v1(global_reference).ok_or_else(|| rejected_at("gfx950 transpose exact source transaction: Stage original shared Global field"))?;
            let memory = rust_capability_memory_view_v1(tcx, global).ok_or_else(|| rejected_at("gfx950 transpose exact source transaction: Stage original Global memory type"))?;
            if memory.element != tcx.types.u8 {
                return Err(rejected_at("gfx950 transpose exact source transaction: Stage Global element"));
            }
            if memory.role != RustCapabilityMemoryRoleV1::ReadOnly {
                return Err(rejected_at("gfx950 transpose exact source transaction: Stage Global memory role"));
            }
            if memory.brand.ty != global_root.ty {
                return Err(rejected_at("gfx950 transpose exact source transaction: Stage Global memory brand"));
            }
            if !numerical_policy_v1::defined_body_v1::matrix::transpose_view_layout_v1(tcx, view, global_reference, before.format, *role, before.brand) {
                return Err(rejected_at("gfx950 transpose exact source transaction: Stage original view layout"));
            }
            let view_id = exact(tcx, types, pointer_pointee_v1(types, ids[1])?, view)
                .map_err(|_| rejected_at("gfx950 transpose exact source transaction: Stage canonical view identity"))?;
            let global_ref_id = exact(
                tcx,
                types,
                canonical_field(types, view_id, 0)?,
                global_reference,
            ).map_err(|_| rejected_at("gfx950 transpose exact source transaction: Stage canonical Global reference identity"))?;
            let global_id = exact(
                tcx,
                types,
                pointer_pointee_v1(types, global_ref_id)?,
                global,
            ).map_err(|_| rejected_at("gfx950 transpose exact source transaction: Stage canonical Global identity"))?;
            (
                T::Stage {
                    input_tile: ids[0],
                    output_tile: result,
                    view_reference: ids[1],
                    view: view_id,
                    index: ids[2],
                    global_reference: global_ref_id,
                    global: global_id,
                },
                &[Own::ByValue, Own::SharedBorrow, Own::ByValue, Own::ByValue],
            )
        }
        Terminal::Gfx950TransposePublish => {
            let workgroup = rust_workgroup_capability_v1(tcx, inputs[1]).ok_or_else(rejected)?;
            let TyKind::Tuple(fields) = output.kind() else {
                return Err(rejected());
            };
            let [next_workgroup, next_tile] = fields.as_slice() else {
                return Err(rejected());
            };
            let after = tile(
                tcx,
                *next_tile,
                "fe2o3_device::gfx950::Gfx950TransposePublished",
            )
            .ok_or_else(rejected)?;
            let next = rust_workgroup_capability_v1(tcx, *next_workgroup).ok_or_else(rejected)?;
            if workgroup.kernel_brand.ty != before.execution
                || workgroup.epoch != before.epoch
                || next.kernel_brand.ty != before.execution
                || next.epoch != after.epoch
                || before.execution != after.execution
                || before.format != after.format
                || !rust_execution_epoch_transition_v1(tcx, before.epoch, after.epoch)
                || instance.args.types().collect::<Vec<_>>()
                    != [before.format, before.execution, before.epoch]
            {
                return Err(rejected());
            }
            after_epoch = Some(rustc_type_identity_v1(tcx, after.epoch));
            after_brand = Some(rustc_type_identity_v1(tcx, after.brand));
            let [workgroup_id, tile_id] = canonical_publish_fields(types, result)?;
            (
                T::Publish {
                    input_tile: ids[0],
                    input_workgroup: ids[1],
                    transition: result,
                    output_workgroup: exact(
                        tcx,
                        types,
                        workgroup_id,
                        *next_workgroup,
                    )?,
                    output_tile: exact(tcx, types, tile_id, *next_tile)?,
                },
                &[Own::ByValue, Own::ByValue],
            )
        }
        Terminal::Gfx950TransposeReadB4 | Terminal::Gfx950TransposeReadB8 => {
            let lane = rust_shared_reference_v1(inputs[1]).ok_or_else(rejected)?;
            let lane_args =
                rust_trusted_adt_type_arguments_v1(tcx, lane, TrustedDeviceItem::WaveLane)
                    .ok_or_else(rejected)?;
            let fragment = rust_trusted_adt_type_arguments_v1(
                tcx,
                output,
                TrustedDeviceItem::Gfx950MfmaFragment,
            )
            .ok_or_else(rejected)?;
            let [format_ty, role, brand] = fragment.as_slice() else {
                return Err(rejected());
            };
            let previous_epoch = rust_next_epoch_v1(tcx, before.epoch).ok_or_else(rejected)?;
            if !matches!(lane_args.as_slice(), [width, brand] if rust_is_exact_trusted_marker_v1(tcx, *width, TrustedDeviceItem::Wave64) && *brand == before.brand)
                || *format_ty != before.format
                || *brand != before.brand
                || !rust_is_exact_trusted_marker_v1(
                    tcx,
                    *role,
                    TrustedDeviceItem::Gfx950MfmaOperandB,
                )
                || instance.args.types().collect::<Vec<_>>() != [before.execution, previous_epoch]
            {
                return Err(rejected());
            }
            let registers =
                numerical_policy_v1::defined_body_v1::matrix::transpose_fragment_layout_v1(
                    tcx,
                    output,
                    before.format,
                    *role,
                    before.brand,
                )
                .ok_or_else(rejected)?;
            let registers_id = exact(tcx, types, canonical_field(types, result, 0)?, registers)?;
            let Some(SemanticTypeShapeV1::Array { element, length: 8 }) = types
                .get(registers_id.index() as usize)
                .map(|ty| ty.shape())
            else {
                return Err(rejected());
            };
            (
                T::Read {
                    tile: ids[0],
                    lane_reference: ids[1],
                    lane: exact(tcx, types, pointer_pointee_v1(types, ids[1])?, lane)?,
                    fragment: result,
                    registers: registers_id,
                    word: exact(tcx, types, *element, tcx.types.u32)?,
                },
                &[Own::ByValue, Own::SharedBorrow],
            )
        }
        _ => return Err(rejected()),
    };
    require_capability_memory_terminal_abi_v1(tcx, abi, types, inputs, output, ownership)?;
    let transpose = SemanticGfx950TransposeContractV1::new(
        operation,
        format,
        rustc_type_identity_v1(tcx, before.brand),
        after_brand,
    )
    .map_err(ProductionSemanticImportErrorV1::SemanticSchema)?;
    let contract = SemanticExecutionCapabilityContractV1::new(
        SemanticExecutionCapabilityOperationV1::Gfx950Transpose(transpose),
        SemanticExecutionCapabilitySignatureV1::new(ids, result)
            .map_err(ProductionSemanticImportErrorV1::SemanticSchema)?,
        capability_memory_provenance_v1(root, contexts)?,
        rustc_type_identity_v1(tcx, before.execution),
        rustc_type_identity_v1(tcx, before.epoch),
        after_epoch,
        source,
    )
    .map_err(ProductionSemanticImportErrorV1::SemanticSchema)?;
    Ok(SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract })
}

#[cfg(test)]
#[path = "transactions/publish_tuple_tests.rs"]
mod publish_tuple_tests;
