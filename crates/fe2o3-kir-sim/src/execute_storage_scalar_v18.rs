//! Execute the closed scalar-cell profile through the existing memory/lifetime engine.
use super::*;
use fe2o3_kernel_ir::StorageOperationV1;

fn storage_pointer<'a>(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    values: &'a HashMap<ValueId, RuntimeValue>,
    address: ValueId,
    site: CompactSite,
) -> Result<&'a PointerValue, SimulationExecutionErrorV1> {
    let RuntimeValue::StoragePointer(storage) = runtime_value(engine, values, address, &site)?
    else {
        return Err(engine.at(
            site,
            SimulationExecutionErrorKindV1::RuntimeType {
                value: Some(address),
                expected: "private scalar storage pointer",
            },
        ));
    };
    if storage.pointer.address_space != AddressSpace::Private
        || crate::storage_scalar_v18::scalar_layout(
            engine.module,
            &Type::StorageObject(storage.layout),
            engine.target,
        ) != Some((storage.layout, storage.pointer.element))
    {
        return Err(engine.at(
            site,
            SimulationExecutionErrorKindV1::InternalInvariant(
                "preflighted exact scalar storage row",
            ),
        ));
    }
    Ok(&storage.pointer)
}

#[inline(never)]
pub(super) fn execute(
    engine: &mut Engine<'_, impl SimulationEventSinkV1>,
    values: &HashMap<ValueId, RuntimeValue>,
    operation: &Operation,
    site: CompactSite,
) -> Result<SmallResults<RuntimeValue>, SimulationExecutionErrorV1> {
    match &operation.kind {
        OperationKind::Storage(StorageOperationV1::ReadValue { address, access }) => {
            let pointer = storage_pointer(engine, values, *address, site)?;
            let value = execute_pointer_load(engine, pointer, *access, &site)?;
            Ok(SmallResults::One(RuntimeValue::Scalar(value)))
        }
        OperationKind::Storage(StorageOperationV1::WriteValue {
            address,
            value,
            access,
        }) => {
            let pointer = storage_pointer(engine, values, *address, site)?;
            let value = scalar_value(engine, values, *value, &site)?;
            execute_pointer_store(engine, pointer, value, *access, &site)?;
            Ok(SmallResults::None)
        }
        _ => Err(engine.at(
            site,
            SimulationExecutionErrorKindV1::InternalInvariant(
                "preflighted scalar storage operation",
            ),
        )),
    }
}
