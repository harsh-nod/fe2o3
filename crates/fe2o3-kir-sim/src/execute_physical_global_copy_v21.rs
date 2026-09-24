//! Executes the admitted physical primitive at its actual canonical SSA site.
//! The enclosing engine owns CFG, scheduling, memory validation/events and limits.
use super::*;
use fe2o3_kernel_ir::{
    Gfx942PhysicalEntryRegisterV20 as Register, Gfx942PhysicalGlobalCopyOpcodeV1 as Opcode,
    Gfx942PhysicalGlobalCopyStepV1 as Step,
};
use physical_entry_state_v20::{Half, Results, Value};

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
fn operand_id(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    step: &Step,
    index: usize,
    site: &CompactSite,
) -> Result<ValueId, SimulationExecutionErrorV1> {
    step.operands
        .get(index)
        .copied()
        .flatten()
        .ok_or_else(|| failure(engine, site, "physical operand prefix"))
}
fn operand<'a>(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    values: &'a HashMap<ValueId, RuntimeValue>,
    step: &Step,
    index: usize,
    site: &CompactSite,
) -> Result<&'a RuntimeValue, SimulationExecutionErrorV1> {
    runtime_value(engine, values, operand_id(engine, step, index, site)?, site)
}
fn scalar(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    values: &HashMap<ValueId, RuntimeValue>,
    step: &Step,
    index: usize,
    ty: ScalarType,
    site: &CompactSite,
) -> Result<ScalarBitsV1, SimulationExecutionErrorV1> {
    let value = scalar_value(engine, values, operand_id(engine, step, index, site)?, site)?;
    if value.ty() != ty {
        return Err(failure(engine, site, "physical numerical scalar type"));
    }
    Ok(value)
}
fn u64_scalar(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    value: u64,
    site: &CompactSite,
) -> Result<ScalarBitsV1, SimulationExecutionErrorV1> {
    ScalarBitsV1::new(ScalarType::U64, u128::from(value), engine.target)
        .map_err(|_| failure(engine, site, "physical U64 scalar"))
}
fn pair(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    values: &HashMap<ValueId, RuntimeValue>,
    step: &Step,
    index: usize,
    site: &CompactSite,
) -> Result<u64, SimulationExecutionErrorV1> {
    let low = scalar(engine, values, step, index, ScalarType::U32, site)?.bits() as u64;
    let high = scalar(engine, values, step, index + 1, ScalarType::U32, site)?.bits() as u64;
    Ok(low | (high << 32))
}
fn push(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    results: &mut Results,
    value: RuntimeValue,
    site: &CompactSite,
) -> Result<(), SimulationExecutionErrorV1> {
    results.push(value).map_err(|kind| engine.at(*site, kind))
}
fn push_scalar(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    results: &mut Results,
    value: ScalarBitsV1,
    site: &CompactSite,
) -> Result<(), SimulationExecutionErrorV1> {
    push(engine, results, RuntimeValue::Scalar(value), site)
}
fn binary(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    opcode: BinaryOp,
    left: ScalarBitsV1,
    right: ScalarBitsV1,
    site: &CompactSite,
) -> Result<ScalarBitsV1, SimulationExecutionErrorV1> {
    match (
        opcode,
        execute_binary(opcode, left, right, engine.target)
            .map_err(|kind| engine.at(*site, kind))?,
    ) {
        (BinaryOp::Checked(_), SmallResults::Two(value, overflow))
            if overflow.ty() == ScalarType::Bool =>
        {
            Ok(value)
        }
        (_, SmallResults::One(value)) => Ok(value),
        _ => Err(failure(engine, site, "physical scalar engine result")),
    }
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
        .position(|register| *register == Some(Register::Exec))
        .ok_or_else(|| failure(engine, site, "physical vector EXEC operand"))?;
    if scalar(engine, values, step, index, ScalarType::U64, site)?.bits() != u128::from(u64::MAX) {
        return Err(failure(
            engine,
            site,
            "physical VALU requires full entry EXEC",
        ));
    }
    Ok(())
}

pub(super) fn is_operation(operation: &OperationKind) -> bool {
    matches!(
        operation,
        OperationKind::Gfx942PhysicalGlobalCopyDeclaration(_)
            | OperationKind::Gfx942PhysicalGlobalCopyStep(_)
    )
}
pub(super) fn is_collective(operation: &OperationKind) -> bool {
    matches!(operation,OperationKind::Gfx942PhysicalGlobalCopyStep(step)if step.instruction.opcode==Opcode::VectorCompareGtU64)
}

/// Keep the physical five-result temporary and its move/bind frame out of every
/// legacy operation path. The same adapter, SSA binder and charges are retained.
#[inline(never)]
pub(super) fn execute_and_bind(
    engine: &mut Engine<'_, impl SimulationEventSinkV1>,
    values: &mut HashMap<ValueId, RuntimeValue>,
    operation: &Operation,
    site: &CompactSite,
) -> Result<(), SimulationExecutionErrorV1> {
    if let OperationKind::Gfx942PhysicalGlobalCopyStep(step) = &operation.kind {
        if step.validate_shape().is_err() {
            return Err(failure(engine, site, "global-copy wait/step shape"));
        }
        if matches!(step.instruction.opcode, Opcode::WaitLgkm0 | Opcode::WaitVm0)
            && !operation.results.is_empty()
        {
            return Err(failure(
                engine,
                site,
                "global-copy wait cannot redefine SSA",
            ));
        }
        match step.instruction.opcode {
            Opcode::WaitLgkm0 => {
                return physical_global_copy_pending_v21::complete_lgkm(engine, values, *site);
            }
            Opcode::WaitVm0 => {
                return physical_global_copy_pending_v21::complete_vm(engine, values, *site);
            }
            _ => {}
        }
    }
    let results = execute(engine, values, operation, site)?;
    results.bind(engine, values, &operation.results, site)
}

#[inline(never)]
pub(super) fn execute(
    engine: &mut Engine<'_, impl SimulationEventSinkV1>,
    values: &HashMap<ValueId, RuntimeValue>,
    operation: &Operation,
    site: &CompactSite,
) -> Result<Results, SimulationExecutionErrorV1> {
    let mut results = Results::empty();
    let step = match &operation.kind {
        OperationKind::Gfx942PhysicalGlobalCopyDeclaration(declaration) => {
            if declaration.validate_shape().is_err() || operation.results.len() != 5 {
                return Err(failure(
                    engine,
                    site,
                    "physical declaration shape after admission",
                ));
            }
            // Bind actual argument values. These identities are not numerical addresses.
            let input = slice_value(engine, values, declaration.parameters[0], site)?;
            let output = slice_value(engine, values, declaration.parameters[1], site)?;
            if input.element != ScalarType::U32
                || input.address_space != AddressSpace::Global
                || input.access != AccessMode::ReadOnly
                || input.abi_argument_ordinal != 0
                || output.element != ScalarType::U32
                || output.address_space != AddressSpace::Global
                || output.access != AccessMode::ReadWrite
                || output.abi_argument_ordinal != 1
                || input.allocation == output.allocation
            {
                return Err(failure(
                    engine,
                    site,
                    "global copy two separate ABI allocations/permissions",
                ));
            }
            for half in [Half::Low, Half::High] {
                push(
                    engine,
                    &mut results,
                    RuntimeValue::PhysicalEntry(Value::GlobalCopyKernarg {
                        half,
                        declaration: *site,
                        parameters: declaration.parameters,
                    }),
                    site,
                )?;
            }
            let invocation = engine
                .invocation
                .ok_or_else(|| failure(engine, site, "physical invocation"))?;
            let group = u32::try_from(invocation.workgroup[0])
                .map_err(|_| failure(engine, site, "physical workgroup index"))?;
            push_scalar(engine, &mut results, ScalarBitsV1::u32(group), site)?;
            push_scalar(
                engine,
                &mut results,
                ScalarBitsV1::u32(invocation.local[0]),
                site,
            )?;
            push_scalar(
                engine,
                &mut results,
                u64_scalar(engine, u64::MAX, site)?,
                site,
            )?;
            return Ok(results);
        }
        OperationKind::Gfx942PhysicalGlobalCopyStep(step) if step.validate_shape().is_ok() => step,
        _ => return Err(failure(engine, site, "physical step shape after admission")),
    };
    if step.instruction.opcode.is_vector_definition() {
        full_exec(engine, values, step, site)?;
    }
    match step.instruction.opcode {
        Opcode::LoadKernargPair => {
            let parameters = physical_global_copy_pending_v21::kernarg_parameters(
                operand(engine, values, step, 0, site)?,
                operand(engine, values, step, 1, site)?,
            )
            .ok_or_else(|| failure(engine, site, "global-copy kernarg half provenance"))?;
            let slot = step.instruction.immediate / 8;
            let root = (slot / 2) as usize;
            let slice = slice_value(engine, values, parameters[root], site)?;
            if slice.abi_argument_ordinal != root as u32 {
                return Err(failure(engine, site, "global-copy kernarg ABI root"));
            }
            if slot.is_multiple_of(2) {
                let pointer = PointerValue {
                    allocation: slice.allocation,
                    byte_offset: slice.byte_offset,
                    element: slice.element,
                    address_space: slice.address_space,
                    access: slice.access,
                    lower_bound: slice.byte_offset,
                    upper_bound: slice.byte_offset.checked_add(slice.byte_len).ok_or_else(
                        || engine.at(*site, SimulationExecutionErrorKindV1::PointerOffsetOverflow),
                    )?,
                    abi_argument_ordinal: slice.abi_argument_ordinal,
                };
                for half in [Half::Low, Half::High] {
                    push(
                        engine,
                        &mut results,
                        RuntimeValue::PhysicalEntry(Value::GlobalCopyPendingPointer {
                            half,
                            generation: *site,
                            pointer: pointer.clone(),
                        }),
                        site,
                    )?;
                }
            } else {
                let length = u64::try_from(slice.elements)
                    .map_err(|_| failure(engine, site, "global-copy length width"))?;
                for (half, bits) in [
                    (Half::Low, length as u32),
                    (Half::High, (length >> 32) as u32),
                ] {
                    push(
                        engine,
                        &mut results,
                        RuntimeValue::PhysicalEntry(Value::GlobalCopyPendingLength {
                            half,
                            generation: *site,
                            bits: ScalarBitsV1::u32(bits),
                        }),
                        site,
                    )?;
                }
            }
        }
        Opcode::WaitLgkm0 | Opcode::WaitVm0 => {
            return Err(failure(
                engine,
                site,
                "global-copy wait requires same-SSA mutable completion",
            ));
        }
        Opcode::ScalarLshl32 => {
            let value = binary(
                engine,
                BinaryOp::ShiftLeft,
                scalar(engine, values, step, 0, ScalarType::U32, site)?,
                ScalarBitsV1::u32(6),
                site,
            )?;
            push_scalar(engine, &mut results, value, site)?;
            push_scalar(
                engine,
                &mut results,
                ScalarBitsV1::boolean(value.bits() != 0),
                site,
            )?;
        }
        Opcode::VectorAddU32 => {
            let left = scalar(engine, values, step, 0, ScalarType::U32, site)?;
            let right = scalar(engine, values, step, 1, ScalarType::U32, site)?;
            push_scalar(
                engine,
                &mut results,
                binary(
                    engine,
                    BinaryOp::Checked(CheckedBinaryOperator::Add),
                    left,
                    right,
                    site,
                )?,
                site,
            )?;
        }
        Opcode::VectorMove32 => {
            let value = if step.instruction.source0 == 255 {
                RuntimeValue::Scalar(ScalarBitsV1::u32(0))
            } else {
                match operand(engine, values, step, 0, site)? {
                    RuntimeValue::Scalar(value) if value.ty() == ScalarType::U32 => {
                        RuntimeValue::Scalar(*value)
                    }
                    value @ RuntimeValue::PhysicalEntry(Value::Output {
                        half: Half::High, ..
                    }) => value.clone(),
                    _ => {
                        return Err(failure(
                            engine,
                            site,
                            "physical move cannot expose symbolic bits",
                        ));
                    }
                }
            };
            push(engine, &mut results, value, site)?;
        }
        Opcode::VectorLshlrev64 => {
            let value = u64_scalar(engine, pair(engine, values, step, 0, site)?, site)?;
            let shifted = binary(
                engine,
                BinaryOp::ShiftLeft,
                value,
                u64_scalar(engine, 2, site)?,
                site,
            )?
            .bits() as u64;
            for half in [Half::Low, Half::High] {
                push(
                    engine,
                    &mut results,
                    RuntimeValue::PhysicalEntry(Value::ScaledOffset {
                        half,
                        generation: *site,
                        displacement: shifted,
                    }),
                    site,
                )?;
            }
        }
        Opcode::VectorAddCarry => {
            let chain = physical_entry_state_v20::low_chain(
                operand(engine, values, step, 0, site)?,
                operand(engine, values, step, 1, site)?,
                *site,
            )
            .ok_or_else(|| failure(engine, site, "physical low pointer chain"))?;
            push(
                engine,
                &mut results,
                RuntimeValue::PhysicalEntry(Value::AddressLow(chain.clone())),
                site,
            )?;
            push(
                engine,
                &mut results,
                RuntimeValue::PhysicalEntry(Value::CarryLow(chain)),
                site,
            )?;
        }
        Opcode::VectorAddCarryIn => {
            let chain = physical_entry_state_v20::high_chain(
                operand(engine, values, step, 0, site)?,
                operand(engine, values, step, 1, site)?,
                operand(engine, values, step, 3, site)?,
            )
            .ok_or_else(|| failure(engine, site, "physical high pointer carry lineage"))?;
            push(
                engine,
                &mut results,
                RuntimeValue::PhysicalEntry(Value::AddressHigh {
                    chain: chain.clone(),
                    high_add: *site,
                }),
                site,
            )?;
            push(
                engine,
                &mut results,
                RuntimeValue::PhysicalEntry(Value::CarryHigh {
                    chain,
                    high_add: *site,
                }),
                site,
            )?;
        }
        Opcode::SaveAndMaskExec => {
            let vcc = scalar(engine, values, step, 0, ScalarType::U64, site)?;
            let exec = scalar(engine, values, step, 1, ScalarType::U64, site)?;
            let masked = binary(engine, BinaryOp::BitAnd, vcc, exec, site)?;
            push_scalar(
                engine,
                &mut results,
                ScalarBitsV1::u32(exec.bits() as u32),
                site,
            )?;
            push_scalar(
                engine,
                &mut results,
                ScalarBitsV1::u32((exec.bits() >> 32) as u32),
                site,
            )?;
            push_scalar(engine, &mut results, masked, site)?;
            push_scalar(
                engine,
                &mut results,
                ScalarBitsV1::boolean(masked.bits() != 0),
                site,
            )?;
        }
        Opcode::GlobalLoadDword => {
            let chain = physical_entry_state_v20::store_chain(
                operand(engine, values, step, 0, site)?,
                operand(engine, values, step, 1, site)?,
            )
            .ok_or_else(|| failure(engine, site, "global-copy input address halves"))?;
            if chain.pointer.access != AccessMode::ReadOnly
                || chain.pointer.abi_argument_ordinal != 0
            {
                return Err(failure(
                    engine,
                    site,
                    "global-copy readonly input provenance",
                ));
            }
            let displacement = usize::try_from(chain.displacement).map_err(|_| {
                engine.at(*site, SimulationExecutionErrorKindV1::PointerOffsetOverflow)
            })?;
            let pointer =
                pointer_at_byte(engine, &chain.pointer, displacement, ScalarType::U32, site)?;
            // The ordinary engine checks permission, bounds, alignment and initialization
            // and records the real read at issue. No scalar result is readable yet.
            let bits = execute_pointer_load(
                engine,
                &pointer,
                MemoryAccess::new(AddressSpace::Global, 4),
                site,
            )?;
            let result = operation
                .results
                .first()
                .ok_or_else(|| failure(engine, site, "global-copy load result"))?
                .id;
            push(
                engine,
                &mut results,
                RuntimeValue::PhysicalEntry(Value::GlobalCopyPendingRead {
                    generation: *site,
                    result,
                    bits,
                }),
                site,
            )?;
        }
        Opcode::GlobalStoreDword => {
            let exec = scalar(engine, values, step, 3, ScalarType::U64, site)?.bits() as u64;
            let lane = engine
                .invocation
                .ok_or_else(|| failure(engine, site, "physical store invocation"))?
                .local[0];
            if lane >= 64 {
                return Err(failure(engine, site, "physical store lane"));
            }
            // Skip before even looking up pointer/data SSA values, validating the
            // pointer, recording accesses/events, or changing initialized memory.
            if exec & (1u64 << lane) == 0 {
                return Ok(results);
            }
            let chain = physical_entry_state_v20::store_chain(
                operand(engine, values, step, 0, site)?,
                operand(engine, values, step, 1, site)?,
            )
            .ok_or_else(|| failure(engine, site, "physical store pointer halves differ"))?;
            if chain.pointer.access != AccessMode::ReadWrite
                || chain.pointer.abi_argument_ordinal != 1
            {
                return Err(failure(engine, site, "global-copy output write provenance"));
            }
            let displacement = usize::try_from(chain.displacement).map_err(|_| {
                engine.at(*site, SimulationExecutionErrorKindV1::PointerOffsetOverflow)
            })?;
            let pointer =
                pointer_at_byte(engine, &chain.pointer, displacement, ScalarType::U32, site)?;
            let data = scalar(engine, values, step, 2, ScalarType::U32, site)?;
            execute_pointer_store(
                engine,
                &pointer,
                data,
                MemoryAccess::new(AddressSpace::Global, 4),
                site,
            )?;
        }
        Opcode::RestoreExec => {
            push_scalar(
                engine,
                &mut results,
                u64_scalar(engine, pair(engine, values, step, 0, site)?, site)?,
                site,
            )?;
        }
        Opcode::VectorCompareGtU64 => {
            return Err(failure(
                engine,
                site,
                "physical comparison requires actual wave rendezvous",
            ));
        }
        Opcode::Endpgm0 => {
            return Err(failure(
                engine,
                site,
                "physical control belongs to canonical terminator",
            ));
        }
    }
    Ok(results)
}

pub(super) fn comparison_input(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    values: &HashMap<ValueId, RuntimeValue>,
    operation: &OperationKind,
    site: &CompactSite,
) -> Result<bool, SimulationExecutionErrorV1> {
    let OperationKind::Gfx942PhysicalGlobalCopyStep(step) = operation else {
        return Err(failure(engine, site, "physical collective dispatch"));
    };
    if step.validate_shape().is_err() || step.instruction.opcode != Opcode::VectorCompareGtU64 {
        return Err(failure(engine, site, "physical collective shape"));
    }
    full_exec(engine, values, step, site)?;
    let lhs = u64_scalar(engine, pair(engine, values, step, 0, site)?, site)?;
    let rhs = u64_scalar(engine, pair(engine, values, step, 2, site)?, site)?;
    execute_compare(ComparePredicate::GreaterThan, lhs, rhs, engine.target)
        .map_err(|kind| engine.at(*site, kind))
}
