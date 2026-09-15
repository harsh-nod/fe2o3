//! Exact branded projection and policy-bound FP4/FP8 MFMA source ABIs.
//! Retaining this context is not numerical or target legalization.

use super::*;
use crate::production_semantic_terminal_v1::ProductionTerminalExpansionV1 as Expansion;

mod layout;
mod profiles;
pub(in crate::collector::production_importer_v1) mod accumulator_zero;

use profiles::MfmaFormats;

#[test]
fn gfx950_matrix_abi_scope_excludes_issuance_and_other_terminals() {
    for expansion in [
        Expansion::Gfx950Fp4AccumulatorIntoValues,
        Expansion::Gfx950Fp8AccumulatorIntoValues,
        Expansion::Gfx950Fp4Fp8MultiplyAccumulate,
        Expansion::Gfx950Fp4MultiplyAccumulate,
        Expansion::Gfx950Fp8MultiplyAccumulate,
    ] {
        assert!(handles(expansion));
        assert!(marker(expansion).is_some());
    }
    for expansion in [
        Expansion::Gfx950Fp4AccumulatorZero,
        Expansion::Gfx950Fp8AccumulatorZero,
        Expansion::Gfx950LdsTransposeTileCurrent,
        Expansion::Gfx950MatrixContextCurrent,
        Expansion::F32MatrixAccumulatorIntoValues,
        Expansion::MatrixMultiplyAccumulate,
    ] {
        assert!(!handles(expansion));
        assert!(marker(expansion).is_none());
    }
}

pub(in crate::collector::production_importer_v1) fn handles(expansion: Expansion) -> bool {
    matches!(
        expansion,
        Expansion::Gfx950Fp4AccumulatorIntoValues
            | Expansion::Gfx950Fp8AccumulatorIntoValues
            | Expansion::Gfx950Fp4Fp8MultiplyAccumulate
            | Expansion::Gfx950Fp4MultiplyAccumulate
            | Expansion::Gfx950Fp8MultiplyAccumulate
    )
}

fn marker(expansion: Expansion) -> Option<TrustedDeviceItem> {
    Some(match expansion {
        Expansion::Gfx950Fp4AccumulatorIntoValues => {
            TrustedDeviceItem::Gfx950Fp4F32AccumulatorFragmentIntoValues
        }
        Expansion::Gfx950Fp8AccumulatorIntoValues => {
            TrustedDeviceItem::Gfx950F32AccumulatorFragmentIntoValues
        }
        Expansion::Gfx950Fp4Fp8MultiplyAccumulate => {
            TrustedDeviceItem::Gfx950MatrixMultiplyAccumulateFp4Fp8
        }
        Expansion::Gfx950Fp4MultiplyAccumulate => {
            TrustedDeviceItem::Gfx950MatrixMultiplyAccumulateFp4
        }
        Expansion::Gfx950Fp8MultiplyAccumulate => {
            TrustedDeviceItem::Gfx950MatrixMultiplyAccumulateFp8
        }
        _ => return None,
    })
}

struct Facts<'tcx> {
    generic_arguments: Vec<Ty<'tcx>>,
    context: Option<Ty<'tcx>>,
}

fn accumulator<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
    format: TrustedDeviceItem,
) -> Option<(Ty<'tcx>, Ty<'tcx>)> {
    let args = rust_trusted_adt_type_arguments_v1(
        tcx,
        ty,
        TrustedDeviceItem::Gfx950F32AccumulatorFragment,
    )?;
    let [actual_format, brand] = args.as_slice() else {
        return None;
    };
    if !rust_is_exact_trusted_marker_v1(tcx, *actual_format, format) {
        return None;
    }
    let values = layout::accumulator(tcx, ty, *actual_format, *brand)?;
    Some((*brand, values))
}

fn fragment<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
    format: TrustedDeviceItem,
    role: TrustedDeviceItem,
    brand: Ty<'tcx>,
) -> bool {
    let Some(args) =
        rust_trusted_adt_type_arguments_v1(tcx, ty, TrustedDeviceItem::Gfx950MfmaFragment)
    else {
        return false;
    };
    let [actual_format, actual_role, actual_brand] = args.as_slice() else {
        return false;
    };
    *actual_brand == brand
        && rust_is_exact_trusted_marker_v1(tcx, *actual_format, format)
        && rust_is_exact_trusted_marker_v1(tcx, *actual_role, role)
        && layout::fragment(tcx, ty, *actual_format, *actual_role, brand)
}

fn facts<'tcx>(
    tcx: TyCtxt<'tcx>,
    expansion: Expansion,
    inputs: &[Ty<'tcx>],
    output: Ty<'tcx>,
) -> Option<Facts<'tcx>> {
    match expansion {
        Expansion::Gfx950Fp4AccumulatorIntoValues | Expansion::Gfx950Fp8AccumulatorIntoValues => {
            let [input] = inputs else { return None };
            let format = if expansion == Expansion::Gfx950Fp4AccumulatorIntoValues {
                TrustedDeviceItem::Gfx950Fp4E2M1Format
            } else {
                TrustedDeviceItem::Gfx950Fp8E4M3Format
            };
            let (brand, values) = accumulator(tcx, *input, format)?;
            // Projection accepts any exact source Brand; it never issues one.
            (values == output && rust_f32_array_v1(tcx, output, 4)).then_some(Facts {
                generic_arguments: vec![brand],
                context: None,
            })
        }
        Expansion::Gfx950Fp4Fp8MultiplyAccumulate
        | Expansion::Gfx950Fp4MultiplyAccumulate
        | Expansion::Gfx950Fp8MultiplyAccumulate => {
            let formats = MfmaFormats::for_expansion(expansion)?;
            let [reference, lhs, rhs, acc] = inputs else {
                return None;
            };
            let context = rust_shared_reference_v1(*reference)?;
            let args = rust_trusted_adt_type_arguments_v1(
                tcx,
                context,
                TrustedDeviceItem::PolicyGfx950MatrixCapability,
            )?;
            let [matrix, root, policy] = args.as_slice() else {
                return None;
            };
            let fields = source::fields(tcx, context).ok()?;
            let [bound_reference, private] = fields.as_slice() else {
                return None;
            };
            if !layout::private_marker(tcx, *private) {
                return None;
            }
            let bound = rust_shared_reference_v1(*bound_reference)?;
            let pair = source::pair(tcx, bound).ok()?;
            if pair.identity.matrix_brand != *matrix
                || pair.identity.root.ty != *root
                || pair.identity.policy != *policy
                || *acc != output
                || !layout::context(tcx, context)
                || accumulator(tcx, *acc, formats.accumulator().marker())?.0 != *matrix
                || !fragment(
                    tcx,
                    *lhs,
                    formats.lhs.marker(),
                    TrustedDeviceItem::Gfx950MfmaOperandA,
                    *matrix,
                )
                || !fragment(
                    tcx,
                    *rhs,
                    formats.rhs.marker(),
                    TrustedDeviceItem::Gfx950MfmaOperandB,
                    *matrix,
                )
            {
                return None;
            }
            Some(Facts {
                generic_arguments: args,
                context: Some(context),
            })
        }
        _ => None,
    }
}

pub(in crate::collector::production_importer_v1) fn source_matches<'tcx>(
    tcx: TyCtxt<'tcx>,
    expansion: Expansion,
    inputs: &[Ty<'tcx>],
    output: Ty<'tcx>,
) -> bool {
    facts(tcx, expansion, inputs, output).is_some()
}

pub(in crate::collector::production_importer_v1) fn operation<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    expansion: Expansion,
    abi: &SemanticFunctionAbiV1,
    types: &[SemanticTypeDeclV1],
) -> Result<SemanticCompilerIntrinsicOperationV1, ProductionSemanticImportErrorV1> {
    if marker(expansion).is_none()
        || trusted_device_items::classify(tcx, instance.def_id()) != marker(expansion)
    {
        return Err(rejected("gfx950 matrix terminal exact reviewed source"));
    }
    let signature = source::signature(tcx, instance)?;
    let facts = facts(tcx, expansion, signature.inputs(), signature.output())
        .ok_or_else(|| rejected("gfx950 matrix terminal branded types, policy or layout"))?;
    if instance.args.types().collect::<Vec<_>>() != facts.generic_arguments {
        return Err(rejected(
            "gfx950 matrix terminal original generic arguments",
        ));
    }
    use SemanticSourceArgumentOwnershipV1::{ByValue, SharedBorrow};
    let ownership: &[_] = if facts.context.is_some() {
        &[SharedBorrow, ByValue, ByValue, ByValue]
    } else {
        &[ByValue]
    };
    require_capability_memory_terminal_abi_v1(
        tcx,
        abi,
        types,
        signature.inputs(),
        signature.output(),
        ownership,
    )?;
    let inputs = abi.source_input_types();
    let output = abi.source_output_type();
    Ok(match expansion {
        Expansion::Gfx950Fp4AccumulatorIntoValues | Expansion::Gfx950Fp8AccumulatorIntoValues => {
            SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorIntoValues {
                fragment: inputs[0],
                values: output,
            }
        }
        Expansion::Gfx950Fp4Fp8MultiplyAccumulate
        | Expansion::Gfx950Fp4MultiplyAccumulate
        | Expansion::Gfx950Fp8MultiplyAccumulate => {
            let formats = MfmaFormats::for_expansion(expansion)
                .ok_or_else(|| rejected("gfx950 matrix closed operand format family"))?;
            // Keep the actual PolicyGfx950Matrix pointee, not an ambient context.
            SemanticCompilerIntrinsicOperationV1::MatrixMultiplyAccumulate {
                context: pointer_pointee_v1(types, inputs[0])?,
                lhs_fragment: inputs[1],
                rhs_fragment: inputs[2],
                accumulator_fragment: inputs[3],
                lhs: SemanticMfmaOperandContractV1 {
                    role: SemanticMfmaOperandRoleV1::A,
                    profile: formats.lhs.profile(),
                    register_distribution: SemanticMfmaRegisterDistributionV1::Gfx950M16N16K128,
                    wave_width: 64,
                },
                rhs: SemanticMfmaOperandContractV1 {
                    role: SemanticMfmaOperandRoleV1::B,
                    profile: formats.rhs.profile(),
                    register_distribution: SemanticMfmaRegisterDistributionV1::Gfx950M16N16K128,
                    wave_width: 64,
                },
                accumulator: SemanticMfmaAccumulatorContractV1 {
                    profile: formats.accumulator().profile(),
                    distribution: SemanticMfmaAccumulatorDistributionV1::RowMajor,
                    wave_width: 64,
                },
            }
        }
        _ => return Err(rejected("unsupported gfx950 matrix ABI terminal")),
    })
}

pub(super) fn validate_carriage<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
) -> Result<(), ProductionSemanticImportErrorV1> {
    accumulator_zero::validate_carriage(tcx, plan, types, callables)?;
    for (index, terminal) in plan.terminal_producers().iter().enumerate() {
        if !handles(terminal.expansion) {
            continue;
        }
        let Some(SemanticCallableDeclV1::CompilerIntrinsic {
            binding,
            operation: retained,
            ..
        }) = callables.get(plan.function_producers().len() + index)
        else {
            return Err(rejected("gfx950 matrix terminal canonical callable"));
        };
        if binding.identity() != terminal.identities.function()
            || binding.abi().identity() != terminal.abi.identity
            || *retained
                != operation(
                    tcx,
                    terminal.instance,
                    terminal.expansion,
                    binding.abi(),
                    types,
                )?
        {
            return Err(rejected(
                "gfx950 matrix terminal original source/ABI carriage",
            ));
        }
    }
    Ok(())
}
