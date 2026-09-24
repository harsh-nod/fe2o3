//! Unchanged scalar memory semantics outlined from the ordinary dispatcher.
//! Debug-build callers reserve only the selected operation family's frame.
use super::*;

#[inline(never)]
pub(super) fn execute(
    engine: &mut Engine<'_, impl SimulationEventSinkV1>,
    values: &HashMap<ValueId, RuntimeValue>,
    operation: &Operation,
    site: CompactSite,
) -> Result<SmallResults<RuntimeValue>, SimulationExecutionErrorV1> {
    let one = |value| Ok(SmallResults::One(value));
    match &operation.kind {
        OperationKind::Load { pointer, access } => one(RuntimeValue::Scalar(execute_scalar_load(
            engine, values, *pointer, *access, &site,
        )?)),
        OperationKind::GuardedLoad {
            pointer,
            predicate,
            fallback,
            access,
        } => {
            let predicate = scalar_value(engine, values, *predicate, &site)?
                .as_bool()
                .ok_or_else(|| {
                    engine.at(
                        site,
                        SimulationExecutionErrorKindV1::RuntimeType {
                            value: Some(*predicate),
                            expected: "boolean guarded-load predicate",
                        },
                    )
                })?;
            let value = if predicate {
                execute_scalar_load(engine, values, *pointer, *access, &site)?
            } else {
                scalar_value(engine, values, *fallback, &site)?
            };
            one(RuntimeValue::Scalar(value))
        }
        OperationKind::Store {
            pointer,
            value,
            access,
        } => {
            let RuntimeValue::Pointer(pointer_value) =
                runtime_value(engine, values, *pointer, &site)?
            else {
                return Err(engine.at(
                    site,
                    SimulationExecutionErrorKindV1::RuntimeType {
                        value: Some(*pointer),
                        expected: "pointer",
                    },
                ));
            };
            let stored = scalar_value(engine, values, *value, &site)?;
            let bytes = engine
                .memory
                .validate_store(pointer_value, *access, stored, engine.target)
                .map_err(|kind| engine.memory_error_at(site, pointer_value, kind))?;
            if pointer_value.address_space == AddressSpace::Global {
                engine.record_access(
                    &site,
                    pointer_value.allocation,
                    pointer_value.byte_offset,
                    bytes,
                    true,
                    false,
                )?;
            }
            engine.observe_and_commit_store(&site, pointer_value, stored, bytes)?;
            Ok(SmallResults::None)
        }
        OperationKind::GuardedStore {
            pointer,
            value,
            predicate,
            access,
        } => {
            let predicate = scalar_value(engine, values, *predicate, &site)?
                .as_bool()
                .ok_or_else(|| {
                    engine.at(
                        site,
                        SimulationExecutionErrorKindV1::RuntimeType {
                            value: Some(*predicate),
                            expected: "boolean guarded-store predicate",
                        },
                    )
                })?;
            if !predicate {
                return Ok(SmallResults::None);
            }
            let RuntimeValue::Pointer(pointer_value) =
                runtime_value(engine, values, *pointer, &site)?
            else {
                return Err(engine.at(
                    site,
                    SimulationExecutionErrorKindV1::RuntimeType {
                        value: Some(*pointer),
                        expected: "pointer",
                    },
                ));
            };
            let stored = scalar_value(engine, values, *value, &site)?;
            let bytes = engine
                .memory
                .validate_store(pointer_value, *access, stored, engine.target)
                .map_err(|kind| engine.at(site, kind))?;
            if pointer_value.address_space == AddressSpace::Global {
                engine.record_access(
                    &site,
                    pointer_value.allocation,
                    pointer_value.byte_offset,
                    bytes,
                    true,
                    false,
                )?;
            }
            engine.observe_and_commit_store(&site, pointer_value, stored, bytes)?;
            Ok(SmallResults::None)
        }
        _ => Err(engine.at(
            site,
            SimulationExecutionErrorKindV1::InternalInvariant(
                "outlined scalar memory operation dispatch",
            ),
        )),
    }
}
