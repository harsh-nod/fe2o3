//! Typed V22 adapter over the existing scalar/memory engine and WG scheduler.
use super::*;
use fe2o3_kernel_ir::{
    Gfx942PhysicalEntryRegisterV20 as Register, Gfx942PhysicalLdsExchangeOpcodeV1 as Opcode,
    Gfx942PhysicalLdsExchangeStepV1 as Step,
};
use physical_entry_state_v20::{Results, Value};
use physical_global_copy_pending_v21::Profile;
fn failure(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    site: &CompactSite,
    message: &'static str,
) -> SimulationExecutionErrorV1 {
    engine.at(
        *site,
        SimulationExecutionErrorKindV1::InternalInvariant(message),
    )
}
pub(super) fn is_operation(operation: &OperationKind) -> bool {
    matches!(
        operation,
        OperationKind::Gfx942PhysicalLdsExchangeDeclaration(_)
            | OperationKind::Gfx942PhysicalLdsExchangeStep(_)
    )
}
pub(super) fn is_collective(operation: &OperationKind) -> bool {
    matches!(operation, OperationKind::Gfx942PhysicalLdsExchangeStep(step) if step.instruction.opcode == Opcode::VectorCompareGtU64)
}
pub(super) fn is_barrier(operation: &OperationKind) -> bool {
    matches!(operation, OperationKind::Gfx942PhysicalLdsExchangeStep(step) if step.instruction.opcode == Opcode::WorkgroupPublishBarrier)
}
fn scalar(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    values: &HashMap<ValueId, RuntimeValue>,
    step: &Step,
    index: usize,
    ty: ScalarType,
    site: &CompactSite,
) -> Result<ScalarBitsV1, SimulationExecutionErrorV1> {
    let id = step
        .operands
        .get(index)
        .copied()
        .flatten()
        .ok_or_else(|| failure(engine, site, "V22 operand prefix"))?;
    let value = scalar_value(engine, values, id, site)?;
    if value.ty() != ty {
        return Err(failure(engine, site, "V22 operand scalar type"));
    }
    Ok(value)
}
fn full_exec(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    values: &HashMap<ValueId, RuntimeValue>,
    step: &Step,
    site: &CompactSite,
) -> Result<(), SimulationExecutionErrorV1> {
    let index = step
        .instruction
        .operand_registers()
        .iter()
        .position(|r| *r == Some(Register::Exec))
        .ok_or_else(|| failure(engine, site, "V22 full EXEC operand"))?;
    if scalar(engine, values, step, index, ScalarType::U64, site)?.bits() != u128::from(u64::MAX) {
        return Err(failure(engine, site, "V22 LDS/VALU requires full EXEC"));
    }
    Ok(())
}
#[inline(never)]
pub(super) fn execute_and_bind(
    engine: &mut Engine<'_, impl SimulationEventSinkV1>,
    values: &mut HashMap<ValueId, RuntimeValue>,
    operation: &Operation,
    site: &CompactSite,
) -> Result<(), SimulationExecutionErrorV1> {
    if let OperationKind::Gfx942PhysicalLdsExchangeDeclaration(declaration) = &operation.kind {
        if declaration.validate_shape().is_err() || operation.results.len() != 5 {
            return Err(failure(engine, site, "V22 declaration shape"));
        }
        let results = physical_global_copy_v21::declaration_results(
            engine,
            values,
            declaration.parameters,
            site,
        )?;
        physical_lds_context_v22::frame_pointer(engine, *site)?;
        return results.bind(engine, values, &operation.results, site);
    }
    let OperationKind::Gfx942PhysicalLdsExchangeStep(step) = &operation.kind else {
        return Err(failure(engine, site, "V22 typed operation"));
    };
    if step.validate_shape().is_err() {
        return Err(failure(engine, site, "V22 step shape"));
    }
    match step.instruction.opcode {
        Opcode::WaitLgkm0 | Opcode::WaitVm0 if !operation.results.is_empty() => {
            return Err(failure(engine, site, "V22 wait cannot redefine SSA"));
        }
        Opcode::WaitLgkm0 => return complete_lgkm(engine, values, *site),
        Opcode::WaitVm0 => {
            return physical_global_copy_pending_v21::complete_vm_for(
                engine,
                values,
                *site,
                Profile::LdsV22,
            );
        }
        _ => {}
    }
    if let Some(view) = physical_global_copy_v21::Step::lds(step) {
        return physical_global_copy_v21::execute_step(engine, values, operation, site, &view)?
            .bind(engine, values, &operation.results, site);
    }
    full_exec(engine, values, step, site)?;
    let mut results = Results::empty();
    match step.instruction.opcode {
        Opcode::VectorLshlrev32 | Opcode::VectorXor32 => {
            let input = scalar(engine, values, step, 0, ScalarType::U32, site)?;
            let opcode = if step.instruction.opcode == Opcode::VectorLshlrev32 {
                BinaryOp::ShiftLeft
            } else {
                BinaryOp::BitXor
            };
            let SmallResults::One(bits) = execute_binary(
                opcode,
                input,
                ScalarBitsV1::u32(step.instruction.immediate),
                engine.target,
            )
            .map_err(|kind| engine.at(*site, kind))?
            else {
                return Err(failure(engine, site, "V22 scalar engine arity"));
            };
            results
                .push(RuntimeValue::Scalar(bits))
                .map_err(|kind| engine.at(*site, kind))?;
        }
        Opcode::LdsWriteB32 => {
            let offset = scalar(engine, values, step, 0, ScalarType::U32, site)?.bits() as u32;
            let bits = scalar(engine, values, step, 1, ScalarType::U32, site)?;
            physical_lds_context_v22::issue_write(engine, *site, offset, bits)?;
        }
        Opcode::LdsReadB32 => {
            let offset = scalar(engine, values, step, 0, ScalarType::U32, site)?.bits() as u32;
            let bits = physical_lds_context_v22::read(engine, *site, offset)?;
            let result = operation
                .results
                .first()
                .ok_or_else(|| failure(engine, site, "V22 LDS result"))?
                .id;
            results
                .push(RuntimeValue::PhysicalEntry(Value::LdsPendingRead {
                    generation: *site,
                    result,
                    bits,
                    epoch: 1,
                }))
                .map_err(|kind| engine.at(*site, kind))?;
        }
        _ => {
            return Err(failure(
                engine,
                site,
                "V22 barrier belongs to workgroup scheduler",
            ));
        }
    }
    results.bind(engine, values, &operation.results, site)
}
fn complete_lgkm(
    engine: &mut Engine<'_, impl SimulationEventSinkV1>,
    values: &mut HashMap<ValueId, RuntimeValue>,
    site: CompactSite,
) -> Result<(), SimulationExecutionErrorV1> {
    if site.operation == Some(5) {
        return physical_global_copy_pending_v21::complete_initial_lgkm(
            engine,
            values,
            site,
            Profile::LdsV22,
        );
    }
    let prior = site
        .operation
        .and_then(|o| o.checked_sub(1))
        .ok_or_else(|| failure(engine, &site, "V22 wait predecessor"))?;
    let (operation, source) =
        physical_global_copy_pending_v21::prior_operation(engine, site, prior)?;
    let OperationKind::Gfx942PhysicalLdsExchangeStep(step) = &operation.kind else {
        return Err(failure(engine, &site, "V22 LGKM actual predecessor"));
    };
    match step.instruction.opcode {
        Opcode::LdsWriteB32 if operation.results.is_empty() => {
            physical_lds_context_v22::complete_write(engine, site, source)
        }
        Opcode::LdsReadB32 if operation.results.len() == 1 => {
            physical_lds_context_v22::require_published(engine, site)?;
            let id = operation.results[0].id;
            let value = values
                .get_mut(&id)
                .ok_or_else(|| failure(engine, &site, "V22 pending LDS read ID"))?;
            let RuntimeValue::PhysicalEntry(Value::LdsPendingRead {
                generation,
                result,
                bits,
                epoch,
            }) = value
            else {
                return Err(failure(engine, &site, "V22 LGKM pending LDS read"));
            };
            if *generation != source || *result != id || *epoch != 1 || bits.ty() != ScalarType::U32
            {
                return Err(failure(
                    engine,
                    &site,
                    "V22 LGKM read generation/result/epoch",
                ));
            }
            *value = RuntimeValue::Scalar(*bits);
            Ok(())
        }
        _ => Err(failure(engine, &site, "V22 LGKM exact predecessor family")),
    }
}
pub(super) fn comparison_input(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    values: &HashMap<ValueId, RuntimeValue>,
    operation: &OperationKind,
    site: &CompactSite,
) -> Result<bool, SimulationExecutionErrorV1> {
    let OperationKind::Gfx942PhysicalLdsExchangeStep(step) = operation else {
        return Err(failure(engine, site, "V22 collective dispatch"));
    };
    if step.validate_shape().is_err() || step.instruction.opcode != Opcode::VectorCompareGtU64 {
        return Err(failure(engine, site, "V22 collective shape"));
    }
    let view = physical_global_copy_v21::Step::lds(step)
        .ok_or_else(|| failure(engine, site, "V22 scalar view"))?;
    physical_global_copy_v21::comparison_step(engine, values, &view, site)
}

#[cfg(test)]
#[path = "execute_physical_lds_exchange_v22_tests.rs"]
mod tests;
