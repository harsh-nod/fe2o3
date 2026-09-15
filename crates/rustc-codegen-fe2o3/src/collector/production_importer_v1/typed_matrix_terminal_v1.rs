//! Source-checked terminal ABIs, without erasing nominal capability brands.

use super::*;
use crate::production_semantic_terminal_v1::ProductionTerminalExpansionV1 as Expansion;

mod legacy_multiply;

pub(super) fn handles(expansion: Expansion) -> bool {
    matches!(
        expansion,
        Expansion::WaveLaneCurrent
            | Expansion::F32MatrixAccumulatorIntoValues
            | Expansion::MatrixMultiplyAccumulate
    )
}

pub(super) fn source_matches<'tcx>(
    tcx: TyCtxt<'tcx>,
    expansion: Expansion,
    inputs: &[Ty<'tcx>],
    output: Ty<'tcx>,
) -> bool {
    match expansion {
        Expansion::WaveLaneCurrent => {
            if !inputs.is_empty() {
                return false;
            }
            let Some(arguments) =
                rust_trusted_adt_type_arguments_v1(tcx, output, TrustedDeviceItem::WaveLane)
            else {
                return false;
            };
            matches!(arguments.as_slice(), [width, brand]
                if rust_is_exact_trusted_marker_v1(tcx, *width, TrustedDeviceItem::Wave64)
                    && rust_exact_reviewed_adt_arguments_v1(
                        tcx, *brand, "fe2o3_device::context::UnbrandedCapability",
                    ).is_some_and(|arguments| arguments.is_empty()))
        }
        Expansion::F32MatrixAccumulatorIntoValues => {
            let [fragment] = inputs else {
                return false;
            };
            let Some(arguments) = rust_trusted_adt_type_arguments_v1(
                tcx,
                *fragment,
                TrustedDeviceItem::F32AccumulatorFragment,
            ) else {
                return false;
            };
            // The reviewed method consumes Self for any Brand. This predicate
            // is only for projection, never issuance or brand compatibility.
            // operation() binds the complete Self type, including Brand.
            matches!(arguments.as_slice(), [profile, distribution, width, _brand]
                if rust_is_exact_trusted_marker_v1(tcx, *profile, TrustedDeviceItem::Bf16MfmaProfile)
                    && rust_is_exact_trusted_marker_v1(tcx, *distribution, TrustedDeviceItem::MfmaAccumulatorRowMajor)
                    && rust_is_exact_trusted_marker_v1(tcx, *width, TrustedDeviceItem::Wave64))
                && rust_f32_array_v1(tcx, output, 4)
        }
        Expansion::MatrixMultiplyAccumulate => {
            legacy_multiply::contracts(tcx, inputs, output).is_some()
        }
        _ => false,
    }
}

pub(super) fn operation<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    expansion: Expansion,
    abi: &SemanticFunctionAbiV1,
    types: &[SemanticTypeDeclV1],
) -> Result<SemanticCompilerIntrinsicOperationV1, ProductionSemanticImportErrorV1> {
    let (item, ownership): (_, &[SemanticSourceArgumentOwnershipV1]) = match expansion {
        Expansion::WaveLaneCurrent => (TrustedDeviceItem::WaveLaneCurrent, &[]),
        Expansion::F32MatrixAccumulatorIntoValues => (
            TrustedDeviceItem::F32AccumulatorFragmentIntoValues,
            &[SemanticSourceArgumentOwnershipV1::ByValue],
        ),
        Expansion::MatrixMultiplyAccumulate => (
            TrustedDeviceItem::DeviceMatrixMultiplyAccumulate,
            &[
                SemanticSourceArgumentOwnershipV1::SharedBorrow,
                SemanticSourceArgumentOwnershipV1::ByValue,
                SemanticSourceArgumentOwnershipV1::ByValue,
                SemanticSourceArgumentOwnershipV1::ByValue,
            ],
        ),
        _ => {
            return Err(body_owner_table_mismatch_v1(
                "typed matrix terminal operation",
            ));
        }
    };
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    );
    if trusted_device_items::classify(tcx, instance.def_id()) != Some(item)
        || !source_matches(tcx, expansion, signature.inputs(), signature.output())
    {
        return Err(ProductionSemanticImportErrorV1::TerminalAbiMismatch {
            terminal: expansion,
            source: tcx.def_path_str(instance.def_id()),
            signature: format!("{signature:?}"),
        });
    }
    require_capability_memory_terminal_abi_v1(
        tcx,
        abi,
        types,
        signature.inputs(),
        signature.output(),
        ownership,
    )?;
    Ok(match expansion {
        Expansion::WaveLaneCurrent => SemanticCompilerIntrinsicOperationV1::WaveLaneCurrent {
            lane: abi.source_output_type(),
            wave_width: 64,
        },
        Expansion::F32MatrixAccumulatorIntoValues => {
            SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorIntoValues {
                fragment: abi.source_input_types()[0],
                values: abi.source_output_type(),
            }
        }
        Expansion::MatrixMultiplyAccumulate => {
            let (lhs, rhs, accumulator) =
                legacy_multiply::contracts(tcx, signature.inputs(), signature.output())
                    .ok_or_else(|| body_owner_table_mismatch_v1("legacy typed MFMA contract"))?;
            let inputs = abi.source_input_types();
            SemanticCompilerIntrinsicOperationV1::MatrixMultiplyAccumulate {
                context: pointer_pointee_v1(types, inputs[0])?,
                lhs_fragment: inputs[1],
                rhs_fragment: inputs[2],
                accumulator_fragment: inputs[3],
                lhs,
                rhs,
                accumulator,
            }
        }
        _ => unreachable!("closed typed matrix terminal set"),
    })
}

pub(super) fn validate_carriage<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    mir: &AdmittedInertSemanticMirV1,
) -> Result<(), ProductionSemanticImportErrorV1> {
    for (index, terminal) in plan.terminal_producers().iter().enumerate() {
        if !handles(terminal.expansion) {
            continue;
        }
        let Some(SemanticCallableDeclV1::CompilerIntrinsic {
            binding,
            operation: retained,
            ..
        }) = mir.callables().get(plan.function_producers().len() + index)
        else {
            return Err(body_owner_table_mismatch_v1(
                "typed matrix terminal carriage",
            ));
        };
        if binding.identity() != terminal.identities.function()
            || binding.abi().identity() != terminal.abi.identity
            || *retained
                != operation(
                    tcx,
                    terminal.instance,
                    terminal.expansion,
                    binding.abi(),
                    mir.types(),
                )?
        {
            return Err(body_owner_table_mismatch_v1(
                "typed matrix terminal source substitution",
            ));
        }
    }
    Ok(())
}
