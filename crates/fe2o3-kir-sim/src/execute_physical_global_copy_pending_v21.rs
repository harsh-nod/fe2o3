//! Pending values have no public/debug scalar representation before exact waits.
use super::*;
use fe2o3_kernel_ir::Gfx942PhysicalGlobalCopyOpcodeV1 as Opcode;
use physical_entry_state_v20::{Half, Value};
#[derive(Clone, Copy)]
pub(super) enum Profile {
    CopyV21,
    LdsV22,
}
fn memory_step(operation: &Operation, profile: Profile) -> Option<(Opcode, u32)> {
    match (profile, &operation.kind) {
        (Profile::CopyV21, OperationKind::Gfx942PhysicalGlobalCopyStep(step)) => {
            Some((step.instruction.opcode, step.instruction.immediate))
        }
        (Profile::LdsV22, OperationKind::Gfx942PhysicalLdsExchangeStep(step)) => {
            use fe2o3_kernel_ir::Gfx942PhysicalLdsExchangeOpcodeV1 as L;
            let opcode = match step.instruction.opcode {
                L::LoadKernargPair => Opcode::LoadKernargPair,
                L::GlobalLoadDword => Opcode::GlobalLoadDword,
                L::GlobalStoreDword => Opcode::GlobalStoreDword,
                _ => return None,
            };
            Some((opcode, step.instruction.immediate))
        }
        _ => None,
    }
}

pub(super) fn kernarg_parameters(low: &RuntimeValue, high: &RuntimeValue) -> Option<[ValueId; 2]> {
    match (low, high) {
        (
            RuntimeValue::PhysicalEntry(Value::GlobalCopyKernarg {
                half: Half::Low,
                declaration: a,
                parameters: p,
            }),
            RuntimeValue::PhysicalEntry(Value::GlobalCopyKernarg {
                half: Half::High,
                declaration: b,
                parameters: q,
            }),
        ) if a == b && p == q => Some(*p),
        _ => None,
    }
}
fn failure(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    site: CompactSite,
    text: &'static str,
) -> SimulationExecutionErrorV1 {
    engine.at(
        site,
        SimulationExecutionErrorKindV1::InternalInvariant(text),
    )
}
pub(super) fn prior_operation<'a>(
    engine: &'a Engine<'_, impl SimulationEventSinkV1>,
    site: CompactSite,
    ordinal: u32,
) -> Result<(&'a Operation, CompactSite), SimulationExecutionErrorV1> {
    let module_index = *engine
        .function_module_indices
        .get(site.function)
        .ok_or_else(|| failure(engine, site, "global-copy function index"))?;
    let body = engine
        .module
        .functions
        .get(module_index)
        .and_then(|f| f.body.as_ref())
        .ok_or_else(|| failure(engine, site, "global-copy function body"))?;
    let block_index = engine
        .block_indices
        .get(site.function)
        .and_then(|m| m.get(&site.block))
        .ok_or_else(|| failure(engine, site, "global-copy block index"))?;
    let operation = body
        .blocks
        .get(*block_index)
        .and_then(|b| b.operations.get(ordinal as usize))
        .ok_or_else(|| failure(engine, site, "global-copy predecessor operation"))?;
    Ok((
        operation,
        CompactSite {
            operation: Some(ordinal),
            ..site
        },
    ))
}
fn ready_kernarg(
    value: &RuntimeValue,
    expected: CompactSite,
    half: Half,
    slot: u32,
) -> Option<RuntimeValue> {
    match value {
        RuntimeValue::PhysicalEntry(Value::GlobalCopyPendingPointer {
            half: h,
            generation,
            pointer,
        }) if *h == half
            && *generation == expected
            && slot.is_multiple_of(2)
            && pointer.abi_argument_ordinal == slot / 2
            && pointer.access
                == if slot == 0 {
                    AccessMode::ReadOnly
                } else {
                    AccessMode::ReadWrite
                } =>
        {
            Some(RuntimeValue::PhysicalEntry(Value::Output {
                half,
                generation: *generation,
                pointer: pointer.clone(),
            }))
        }
        RuntimeValue::PhysicalEntry(Value::GlobalCopyPendingLength {
            half: h,
            generation,
            bits,
        }) if *h == half
            && *generation == expected
            && !slot.is_multiple_of(2)
            && bits.ty() == ScalarType::U32 =>
        {
            Some(RuntimeValue::Scalar(*bits))
        }
        _ => None,
    }
}
/// At most four actual predecessors/eight actual result IDs; no table scan,
/// transaction counter, new SSA definition or replacement executable graph.
pub(super) fn complete_lgkm(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    values: &mut HashMap<ValueId, RuntimeValue>,
    site: CompactSite,
) -> Result<(), SimulationExecutionErrorV1> {
    complete_initial_lgkm(engine, values, site, Profile::CopyV21)
}
pub(super) fn complete_initial_lgkm(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    values: &mut HashMap<ValueId, RuntimeValue>,
    site: CompactSite,
    profile: Profile,
) -> Result<(), SimulationExecutionErrorV1> {
    let ordinal = site
        .operation
        .filter(|n| *n == 5)
        .ok_or_else(|| failure(engine, site, "global-copy initial LGKM wait site"))?;
    let mut ids = [ValueId(0); 8];
    let mut sources = [site; 8];
    let mut slots = [0u32; 8];
    for (pair, previous) in (ordinal - 4..ordinal).enumerate() {
        let (operation, source) = prior_operation(engine, site, previous)?;
        let Some((Opcode::LoadKernargPair, immediate)) = memory_step(operation, profile) else {
            return Err(failure(engine, site, "global-copy LGKM predecessor roster"));
        };
        if operation.results.len() != 2 {
            return Err(failure(engine, site, "global-copy LGKM result roster"));
        }
        let slot = immediate / 8;
        for (part, half) in [Half::Low, Half::High].into_iter().enumerate() {
            let id = operation.results[part].id;
            let value = values
                .get(&id)
                .ok_or_else(|| failure(engine, site, "global-copy pending kernarg ID"))?;
            if ready_kernarg(value, source, half, slot).is_none() {
                return Err(failure(
                    engine,
                    site,
                    "global-copy pending kernarg generation",
                ));
            }
            ids[pair * 2 + part] = id;
            sources[pair * 2 + part] = source;
            slots[pair * 2 + part] = slot;
        }
    }
    for (index, id) in ids.into_iter().enumerate() {
        let value = values
            .get_mut(&id)
            .ok_or_else(|| failure(engine, site, "global-copy retained kernarg ID"))?;
        *value = ready_kernarg(
            value,
            sources[index],
            if index.is_multiple_of(2) {
                Half::Low
            } else {
                Half::High
            },
            slots[index],
        )
        .ok_or_else(|| failure(engine, site, "global-copy retained kernarg generation"))?;
    }
    Ok(())
}
pub(super) fn complete_vm(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    values: &mut HashMap<ValueId, RuntimeValue>,
    site: CompactSite,
) -> Result<(), SimulationExecutionErrorV1> {
    complete_vm_for(engine, values, site, Profile::CopyV21)
}
pub(super) fn complete_vm_for(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    values: &mut HashMap<ValueId, RuntimeValue>,
    site: CompactSite,
    profile: Profile,
) -> Result<(), SimulationExecutionErrorV1> {
    let ordinal = site
        .operation
        .and_then(|n| n.checked_sub(1))
        .ok_or_else(|| failure(engine, site, "global-copy VM wait predecessor"))?;
    let (operation, source) = prior_operation(engine, site, ordinal)?;
    let Some((opcode, _)) = memory_step(operation, profile) else {
        return Err(failure(engine, site, "global-copy VM predecessor kind"));
    };
    match opcode {
        Opcode::GlobalLoadDword if operation.results.len() == 1 => {
            let id = operation.results[0].id;
            let value = values
                .get_mut(&id)
                .ok_or_else(|| failure(engine, site, "global-copy pending read ID"))?;
            let RuntimeValue::PhysicalEntry(Value::GlobalCopyPendingRead {
                generation,
                result,
                bits,
            }) = value
            else {
                return Err(failure(
                    engine,
                    site,
                    "global-copy VM result is not pending",
                ));
            };
            if *generation != source || *result != id || bits.ty() != ScalarType::U32 {
                return Err(failure(
                    engine,
                    site,
                    "global-copy VM read generation/result join",
                ));
            }
            *value = RuntimeValue::Scalar(*bits);
            Ok(())
        }
        Opcode::GlobalStoreDword if operation.results.is_empty() => Ok(()),
        _ => Err(failure(
            engine,
            site,
            "global-copy VM wait without exact memory predecessor",
        )),
    }
}
