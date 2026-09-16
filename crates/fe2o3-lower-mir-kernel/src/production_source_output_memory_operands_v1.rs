#[derive(Clone, Copy, Debug)]
enum SourceOutputMemoryOperandsV1 {
    OmittedUnreachable,
    Retained {
        operation: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
        pointer: fe2o3_kernel_analysis::CanonicalKirOutputUseV1,
        value: Option<fe2o3_kernel_analysis::CanonicalKirOutputUseV1>,
        executable: bool,
    },
}

// Exact operand transport shared with source-rooted Store-value correspondence.
// The legacy Global adapter retains this body's charge and refusal sequence.
fn source_output_memory_operands_v1(
    control: &fe2o3_kernel_analysis::CheckedCanonicalKirControlIndexV1<'_, '_, '_>,
    original: &fe2o3_kernel_analysis::CanonicalKirOperationRefV1<'_>,
    value: Option<ValueId>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<SourceOutputMemoryOperandsV1, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    use fe2o3_kernel_ir::CanonicalKirUseCoordinateV1::OperationOperand;
    let state = control
        .block(original.coordinate.block, budget)
        .map_err(Error::Transition)?;
    let pointer_use = control
        .operand(
            OperationOperand {
                operation: original.coordinate,
                operand: 0,
            },
            budget,
        )
        .map_err(Error::Transition)?;
    budget.charge_work(1).map_err(Error::Resource)?;
    let value_use = if value.is_some() {
        control
            .operand(
                OperationOperand {
                    operation: original.coordinate,
                    operand: 1,
                },
                budget,
            )
            .map_err(Error::Transition)?
    } else {
        None
    };
    budget.charge_work(3).map_err(Error::Resource)?;
    let Some(pointer_use) = pointer_use else {
        if value_use.is_none() && !state.reachable {
            return Ok(SourceOutputMemoryOperandsV1::OmittedUnreachable);
        }
        return Err(Error::Invalid("global reachable access operand omitted"));
    };
    let OperationOperand {
        operation: output_coordinate,
        operand: 0,
    } = pointer_use.coordinate
    else {
        return Err(Error::Invalid("global pointer occurrence changed"));
    };
    if value.is_some() != value_use.is_some()
        || value_use.is_some_and(|used| {
            used.coordinate
                != OperationOperand {
                    operation: output_coordinate,
                    operand: 1,
                }
        })
    {
        return Err(Error::Invalid("global Store value occurrence changed"));
    }
    let output_operation =
        source_output_operation_v1(control.output().owner(), output_coordinate, budget)?;
    budget.charge_work(4).map_err(Error::Resource)?;
    let actual_pointer = match (&original.operation.kind, &output_operation.kind) {
        (OperationKind::Load { access: a, .. }, OperationKind::Load { pointer, access: b })
            if a == b
                && original.operation.results.len() == 1
                && output_operation.results.len() == 1 =>
        {
            *pointer
        }
        (
            OperationKind::Store { access: a, .. },
            OperationKind::Store {
                pointer,
                value,
                access: b,
            },
        ) if a == b
            && original.operation.results.is_empty()
            && output_operation.results.is_empty() =>
        {
            let (actual, _) = source_output_definition_v1(
                control.output().owner(),
                value_use.unwrap().definition,
                budget,
            )?;
            if actual != *value {
                return Err(Error::Invalid("global Store value binding changed"));
            }
            *pointer
        }
        _ => return Err(Error::Invalid("global memory operation payload changed")),
    };
    let (actual, _) =
        source_output_definition_v1(control.output().owner(), pointer_use.definition, budget)?;
    budget.charge_work(1).map_err(Error::Resource)?;
    if actual != actual_pointer {
        return Err(Error::Invalid("global pointer binding changed"));
    }
    Ok(SourceOutputMemoryOperandsV1::Retained {
        operation: output_coordinate,
        pointer: pointer_use,
        value: value_use,
        executable: state.reachable,
    })
}
