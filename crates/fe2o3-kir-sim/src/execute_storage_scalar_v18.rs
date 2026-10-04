//! Private storage views use the existing allocation, initialization and lifetime engine.
use super::*;
use fe2o3_kernel_ir::{StorageLayoutKindV1, StorageOperationV1, StorageProjectionV1};

fn project(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    values: &HashMap<ValueId, RuntimeValue>,
    base: ValueId,
    step: StorageProjectionV1,
    site: CompactSite,
) -> Result<StoragePointerValueV18, SimulationExecutionErrorV1> {
    let RuntimeValue::StoragePointer(storage) = runtime_value(engine, values, base, &site)? else {
        return Err(engine.at(
            site,
            SimulationExecutionErrorKindV1::RuntimeType {
                value: Some(base),
                expected: "private storage pointer",
            },
        ));
    };
    let pointer = &storage.pointer;
    let invalid = || {
        engine.at(
            site,
            SimulationExecutionErrorKindV1::InternalInvariant(
                "preflighted private record/array projection",
            ),
        )
    };
    if pointer.address_space != AddressSpace::Private || pointer.exposed_generic {
        return Err(invalid());
    }
    let row = engine
        .module
        .storage_layouts
        .get(storage.layout.0 as usize)
        .ok_or_else(invalid)?;
    let allocation = engine
        .memory
        .allocation(pointer)
        .map_err(|kind| engine.at(site, kind))?;
    let parent_bytes = usize::try_from(row.size).map_err(|_| invalid())?;
    let parent_end = pointer
        .byte_offset
        .checked_add(parent_bytes)
        .ok_or_else(|| engine.at(site, SimulationExecutionErrorKindV1::PointerOffsetOverflow))?;
    // Formation does not read bytes or grant read/write access. Even a zero-sized
    // projection must retain a live allocation and the exact parent view's bounds.
    if pointer.byte_offset < pointer.lower_bound
        || parent_end > pointer.upper_bound
        || parent_end > allocation.bytes.len()
    {
        return Err(engine.at(
            site,
            out_of_bounds_error(pointer, parent_bytes, allocation.bytes.len()),
        ));
    }
    let (child_id, offset) = match (&row.kind, step) {
        (StorageLayoutKindV1::Record(fields), StorageProjectionV1::Field(index)) => {
            let field = fields.get(index as usize).ok_or_else(invalid)?;
            (field.layout, field.offset)
        }
        (
            StorageLayoutKindV1::Array {
                element,
                length,
                stride,
            },
            StorageProjectionV1::ArrayIndex(index),
        ) => {
            let index = scalar_nonnegative_usize(
                scalar_value(engine, values, index, &site)?,
                engine.target,
            )
            .map_err(|kind| engine.at(site, kind))?;
            let index = u64::try_from(index)
                .map_err(|_| engine.at(site, SimulationExecutionErrorKindV1::IntegerOutOfRange))?;
            if index >= *length {
                return Err(engine.at(
                    site,
                    SimulationExecutionErrorKindV1::StorageArrayIndexOutOfBounds {
                        index,
                        length: *length,
                    },
                ));
            }
            (
                *element,
                index.checked_mul(*stride).ok_or_else(|| {
                    engine.at(site, SimulationExecutionErrorKindV1::PointerOffsetOverflow)
                })?,
            )
        }
        _ => return Err(invalid()),
    };
    let (_, element, bytes) = crate::storage_scalar_v18::allocation_layout(
        engine.module,
        &Type::StorageObject(child_id),
        engine.target,
    )
    .ok_or_else(invalid)?;
    let offset = usize::try_from(offset)
        .map_err(|_| engine.at(site, SimulationExecutionErrorKindV1::PointerOffsetOverflow))?;
    let byte_offset = pointer
        .byte_offset
        .checked_add(offset)
        .ok_or_else(|| engine.at(site, SimulationExecutionErrorKindV1::PointerOffsetOverflow))?;
    let end = byte_offset
        .checked_add(bytes)
        .ok_or_else(|| engine.at(site, SimulationExecutionErrorKindV1::PointerOffsetOverflow))?;
    if end > parent_end {
        return Err(invalid());
    }
    Ok(StoragePointerValueV18 {
        layout: child_id,
        pointer: PointerValue {
            byte_offset,
            element,
            lower_bound: byte_offset,
            upper_bound: end,
            ..pointer.clone()
        },
    })
}

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
        OperationKind::Storage(StorageOperationV1::Project { base, step }) => {
            Ok(SmallResults::One(RuntimeValue::StoragePointer(project(
                engine, values, *base, *step, site,
            )?)))
        }
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
