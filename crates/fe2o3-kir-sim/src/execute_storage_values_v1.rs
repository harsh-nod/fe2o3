//! Symbolic pointer payloads preserve provenance, never guessed address bits.

use super::*;
use fe2o3_kernel_ir::{
    FixedVectorTypeV12, StorageLayoutIdV1, StorageLayoutKindV1, StoragePointerV1,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum StoragePointerPayloadV1 {
    Scalar(PointerValue),
    Object(StorageAddressV1),
}

impl StoragePointerPayloadV1 {
    pub(super) fn pointer(&self) -> &PointerValue {
        match self {
            Self::Scalar(pointer) => pointer,
            Self::Object(address) => &address.pointer,
        }
    }

    pub(super) fn guard(&self) -> Option<usize> {
        self.pointer().storage_guard
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct StorageRelocationV1 {
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) representation: StoragePointerV1,
    pub(super) value: StoragePointerPayloadV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct StorageSliceV1 {
    pub(super) address: StorageAddressV1,
    pub(super) elements: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct StorageVectorValueV1 {
    pub(super) index: usize,
    pub(super) ty: FixedVectorTypeV12,
}

pub(super) struct StorageVectorV1 {
    pub(super) ty: FixedVectorTypeV12,
    pub(super) lanes: Vec<ScalarBitsV1>,
}

enum StoragePointerScopeV29 {
    Runtime(SimulationInvocationV1),
    OriginalInput,
}

fn storage_pointee_type_v1(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    mut layout: StorageLayoutIdV1,
    mut ty: &Type,
    site: &CompactSite,
) -> Result<bool, SimulationExecutionErrorV1> {
    loop {
        let row = storage_row_v1(engine, layout, site)?;
        match (ty, &row.kind) {
            (Type::StorageObject(actual), _) => return Ok(*actual == layout),
            (Type::Scalar(actual), StorageLayoutKindV1::Scalar(expected)) => {
                return Ok(actual == expected);
            }
            (Type::Vector(actual), StorageLayoutKindV1::Vector(expected)) => {
                return Ok(actual == expected);
            }
            (Type::Pointer(actual), StorageLayoutKindV1::Pointer(expected)) => {
                if actual.address_space != expected.value_space || actual.access != expected.access
                {
                    return Ok(false);
                }
                layout = expected.pointee;
                ty = &actual.pointee;
            }
            (
                Type::Slice(actual),
                StorageLayoutKindV1::Slice {
                    element,
                    value_space,
                    access,
                    ..
                },
            ) => {
                if actual.address_space != *value_space || actual.access != *access {
                    return Ok(false);
                }
                layout = *element;
                ty = &actual.element;
            }
            _ => return Ok(false),
        }
    }
}

pub(super) fn storage_runtime_type_matches_v1(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    value: &RuntimeValue,
    ty: &Type,
    site: &CompactSite,
) -> Result<bool, SimulationExecutionErrorV1> {
    Ok(match (value, ty) {
        (RuntimeValue::Scalar(value), Type::Scalar(expected)) => value.ty() == *expected,
        (RuntimeValue::Pointer(value), Type::Pointer(expected)) => {
            expected.address_space == value.visible_address_space()
                && expected.access == value.access
                && expected.pointee.as_ref() == &Type::Scalar(value.element)
        }
        (RuntimeValue::Slice(value), Type::Slice(expected)) => {
            expected.address_space == value.visible_address_space()
                && expected.access == value.access
                && expected.element.as_ref() == &Type::Scalar(value.element)
        }
        (RuntimeValue::StoragePointer(value), Type::Pointer(expected)) => {
            expected.address_space == value.pointer.visible_address_space()
                && expected.access == value.pointer.access
                && storage_pointee_type_v1(engine, value.layout, &expected.pointee, site)?
        }
        (RuntimeValue::StorageSlice(value), Type::Slice(expected)) => {
            expected.address_space == value.address.pointer.visible_address_space()
                && expected.access == value.address.pointer.access
                && storage_pointee_type_v1(engine, value.address.layout, &expected.element, site)?
        }
        (RuntimeValue::StorageVector(value), Type::Vector(expected)) => {
            *expected == value.ty
                && engine
                    .memory
                    .storage_vectors
                    .get(value.index)
                    .is_some_and(|payload| payload.ty == value.ty)
        }
        _ => false,
    })
}

impl Memory {
    fn storage_scalar_v1(
        &self,
        address: &StorageAddressV1,
        scalar: ScalarType,
        access: MemoryAccess,
        target: SimulationTargetV1,
        invocation: SimulationInvocationV1,
    ) -> Result<ScalarBitsV1, SimulationExecutionErrorKindV1> {
        let width = target
            .scalar_bytes(scalar)
            .ok_or(storage_violation_v1("unsupported storage scalar"))?;
        self.storage_value_ready_v1(address, access, width, invocation)?;
        let allocation = self.allocation(&address.pointer)?;
        let start = address.pointer.byte_offset;
        let end = start + width;
        allocation
            .storage
            .raw_read(start, end, &self.storage_accounting)?;
        self.storage_accounting.charge(width)?;
        let mut bits = [0_u8; 16];
        bits[..width].copy_from_slice(&allocation.bytes[start..end]);
        ScalarBitsV1::new(scalar, u128::from_le_bytes(bits), target)
            .map_err(|_| storage_violation_v1("stored scalar bits violate their exact type"))
    }

    pub(super) fn storage_relocation_v1(
        &self,
        address: &StorageAddressV1,
        representation: StoragePointerV1,
        invocation: SimulationInvocationV1,
    ) -> Result<StoragePointerPayloadV1, SimulationExecutionErrorKindV1> {
        let allocation = self.allocation(&address.pointer)?;
        let width = usize::from(representation.stored_bits / 8);
        let end = address
            .pointer
            .byte_offset
            .checked_add(width)
            .ok_or(SimulationExecutionErrorKindV1::PointerOffsetOverflow)?;
        self.storage_accounting
            .charge(allocation.storage.relocations.len())?;
        let relocation = allocation
            .storage
            .relocations
            .iter()
            .find(|relocation| {
                relocation.start == address.pointer.byte_offset && relocation.end == end
            })
            .ok_or(storage_violation_v1(
                "pointer representation lacks a complete relocation",
            ))?;
        if relocation.representation != representation {
            return Err(storage_violation_v1(
                "pointer relocation representation does not match its row",
            ));
        }
        self.storage_validate_pointer_v1(&relocation.value, representation, invocation)?;
        Ok(relocation.value.clone())
    }

    pub(super) fn storage_validate_pointer_v1(
        &self,
        value: &StoragePointerPayloadV1,
        representation: StoragePointerV1,
        invocation: SimulationInvocationV1,
    ) -> Result<(), SimulationExecutionErrorKindV1> {
        self.storage_validate_pointer_scope_v29(value, representation, StoragePointerScopeV29::Runtime(invocation))
    }

    pub(super) fn storage_validate_input_pointer_v29(
        &self,
        value: &StoragePointerPayloadV1,
        representation: StoragePointerV1,
    ) -> Result<(), SimulationExecutionErrorKindV1> {
        self.storage_validate_pointer_scope_v29(value, representation, StoragePointerScopeV29::OriginalInput)
    }

    fn storage_validate_pointer_scope_v29(
        &self,
        value: &StoragePointerPayloadV1,
        representation: StoragePointerV1,
        scope: StoragePointerScopeV29,
    ) -> Result<(), SimulationExecutionErrorKindV1> {
        self.storage_accounting.charge(1)?;
        let pointer = value.pointer();
        if pointer.visible_address_space() != representation.value_space
            || pointer.access != representation.access
        {
            return Err(storage_violation_v1(
                "pointer value changes provenance space or rights",
            ));
        }
        let allocation = self.allocation(pointer)?;
        if allocation.address_space != pointer.address_space
            || (allocation.access != AccessMode::ReadWrite && allocation.access != pointer.access)
            || (pointer.address_space == AddressSpace::Constant
                && pointer.access != AccessMode::ReadOnly)
        {
            return Err(storage_violation_v1(
                "pointer value exceeds its backing space or rights",
            ));
        }
        match scope {
            StoragePointerScopeV29::Runtime(invocation) => allocation.storage.scope.validate(invocation)?,
            StoragePointerScopeV29::OriginalInput => {
                if allocation.input.is_none() || allocation.storage.scope != StorageScopeV1::Unscoped
                    || !matches!(allocation.address_space, AddressSpace::Global | AddressSpace::Constant)
                { return Err(storage_violation_v1("external pointer lacks an original unscoped input allocation")); }
            }
        }
        allocation
            .storage
            .guard(value.guard(), &self.storage_accounting)?;
        if pointer.byte_offset < pointer.lower_bound
            // A value may denote an empty view or one-past endpoint. Actual
            // reads, writes and child projections separately check their width.
            || pointer.byte_offset > pointer.upper_bound
            || pointer.upper_bound > allocation.bytes.len()
        {
            return Err(storage_violation_v1(
                "pointer relocation is outside its live referent",
            ));
        }
        if let StoragePointerPayloadV1::Object(address) = value {
            if address.layout != representation.pointee {
                return Err(storage_violation_v1(
                    "pointer relocation changes local layout ID",
                ));
            }
        }
        Ok(())
    }
}

fn storage_pointer_result_v1(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    value: StoragePointerPayloadV1,
    representation: StoragePointerV1,
    result: &Type,
    site: &CompactSite,
) -> Result<RuntimeValue, SimulationExecutionErrorV1> {
    let Type::Pointer(result) = result else {
        return Err(engine.at(
            *site,
            storage_violation_v1("pointer read has a non-pointer result"),
        ));
    };
    let mut pointer = value.pointer().clone();
    if result.address_space != representation.value_space
        || result.access != representation.access
        || !storage_pointee_type_v1(engine, representation.pointee, &result.pointee, site)?
    {
        return Err(engine.at(
            *site,
            storage_violation_v1("pointer read result changes its representation"),
        ));
    }
    if let Type::Scalar(scalar) = result.pointee.as_ref() {
        pointer.element = *scalar;
        Ok(RuntimeValue::Pointer(pointer))
    } else {
        Ok(RuntimeValue::StoragePointer(StorageAddressV1 {
            pointer,
            layout: representation.pointee,
        }))
    }
}

fn vector_physical_lane_v1(vector: FixedVectorTypeV12, logical: usize) -> usize {
    match vector.layout {
        fe2o3_kernel_ir::VectorLayoutV12::Contiguous => logical,
        fe2o3_kernel_ir::VectorLayoutV12::Interleaved { factor } => {
            let factor = usize::from(factor);
            (logical % factor) * (usize::from(vector.lanes) / factor) + logical / factor
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn storage_read_value_v1(
    engine: &mut Engine<'_, impl SimulationEventSinkV1>,
    address: &StorageAddressV1,
    row: &fe2o3_kernel_ir::StorageLayoutV1,
    result: &Type,
    access: MemoryAccess,
    invocation: SimulationInvocationV1,
    site: &CompactSite,
) -> Result<RuntimeValue, SimulationExecutionErrorV1> {
    let width = storage_extent_v1(row).map_err(|kind| engine.at(*site, kind))?;
    engine
        .memory
        .storage_validate_v1(address, access, width, false, invocation)
        .map_err(|kind| engine.at(*site, kind))?;
    let value = match &row.kind {
        StorageLayoutKindV1::Scalar(scalar) => {
            if engine.target.scalar_bytes(*scalar) != Some(width)
                || result != &Type::Scalar(*scalar)
            {
                return Err(engine.at(
                    *site,
                    storage_violation_v1("storage scalar layout differs from target/result"),
                ));
            }
            RuntimeValue::Scalar(
                engine
                    .memory
                    .storage_scalar_v1(address, *scalar, access, engine.target, invocation)
                    .map_err(|kind| engine.at(*site, kind))?,
            )
        }
        StorageLayoutKindV1::Pointer(representation) => {
            engine
                .memory
                .storage_value_ready_v1(address, access, width, invocation)
                .map_err(|kind| engine.at(*site, kind))?;
            let pointer = engine
                .memory
                .storage_relocation_v1(address, *representation, invocation)
                .map_err(|kind| engine.at(*site, kind))?;
            storage_pointer_result_v1(engine, pointer, *representation, result, site)?
        }
        StorageLayoutKindV1::Vector(vector) => {
            let represented = vector
                .byte_width()
                .and_then(|value| usize::try_from(value).ok())
                .filter(|represented| *represented <= width)
                .ok_or_else(|| {
                    engine.at(
                        *site,
                        storage_violation_v1("storage vector exceeds its row extent"),
                    )
                })?;
            if result != &Type::Vector(*vector) {
                return Err(engine.at(
                    *site,
                    storage_violation_v1("storage vector layout differs from target/result"),
                ));
            }
            engine
                .memory
                .storage_value_ready_v1(address, access, represented, invocation)
                .map_err(|kind| engine.at(*site, kind))?;
            let headers = storage_vector_headers_v1();
            engine
                .memory
                .storage_accounting
                .hold(headers)
                .map_err(|kind| engine.at(*site, kind))?;
            let mut lanes = Vec::new();
            if let Err(kind) = storage_reserve_v1(
                &mut lanes,
                usize::from(vector.lanes),
                &engine.memory.storage_accounting,
            ) {
                drop(lanes);
                engine.memory.storage_accounting.release(headers);
                return Err(engine.at(*site, kind));
            }
            let decoded = (|| {
                let scalar_width = engine.target.scalar_bytes(vector.element).ok_or_else(|| {
                    engine.at(
                        *site,
                        storage_violation_v1("vector element width unavailable"),
                    )
                })?;
                for logical in 0..usize::from(vector.lanes) {
                    let mut lane = address.clone();
                    lane.pointer.byte_offset +=
                        vector_physical_lane_v1(*vector, logical) * scalar_width;
                    lanes.push(
                        engine
                            .memory
                            .storage_scalar_v1(
                                &lane,
                                vector.element,
                                MemoryAccess {
                                    alignment: 1,
                                    ..access
                                },
                                engine.target,
                                invocation,
                            )
                            .map_err(|kind| engine.at(*site, kind))?,
                    );
                }
                let index = engine.memory.storage_vectors.len();
                let count = index.checked_add(1).ok_or_else(|| {
                    engine.at(*site, storage_violation_v1("vector value ID exhausted"))
                })?;
                storage_reserve_v1(
                    &mut engine.memory.storage_vectors,
                    count,
                    &engine.memory.storage_accounting,
                )
                .map_err(|kind| engine.at(*site, kind))?;
                Ok(index)
            })();
            match decoded {
                Ok(index) => {
                    engine
                        .memory
                        .storage_vectors
                        .push(StorageVectorV1 { ty: *vector, lanes });
                    engine.memory.storage_accounting.release(headers);
                    RuntimeValue::StorageVector(StorageVectorValueV1 { index, ty: *vector })
                }
                Err(error) => {
                    let bytes = lanes.capacity() * size_of::<ScalarBitsV1>();
                    drop(lanes);
                    engine.memory.storage_accounting.release(bytes + headers);
                    return Err(error);
                }
            }
        }
        StorageLayoutKindV1::Slice {
            element,
            value_space,
            access: rights,
            data,
            length,
        } => {
            let Type::Slice(result) = result else {
                return Err(engine.at(
                    *site,
                    storage_violation_v1("slice read has a non-slice result"),
                ));
            };
            if result.address_space != *value_space
                || result.access != *rights
                || !storage_pointee_type_v1(engine, *element, &result.element, site)?
            {
                return Err(engine.at(
                    *site,
                    storage_violation_v1("slice result changes its represented type"),
                ));
            }
            let data_row = storage_row_v1(engine, data.layout, site)?;
            let length_row = storage_row_v1(engine, length.layout, site)?;
            let StorageLayoutKindV1::Pointer(representation) = data_row.kind else {
                return Err(engine.at(
                    *site,
                    storage_violation_v1("slice data field is not a pointer"),
                ));
            };
            let child_access = MemoryAccess {
                alignment: 1,
                ..access
            };
            let data_address = engine
                .memory
                .storage_child_v1(
                    address,
                    data.offset,
                    data.layout,
                    data_row,
                    address.pointer.access,
                )
                .map_err(|kind| engine.at(*site, kind))?;
            engine
                .memory
                .storage_value_ready_v1(
                    &data_address,
                    child_access,
                    storage_extent_v1(data_row).map_err(|kind| engine.at(*site, kind))?,
                    invocation,
                )
                .map_err(|kind| engine.at(*site, kind))?;
            let pointer = engine
                .memory
                .storage_relocation_v1(&data_address, representation, invocation)
                .map_err(|kind| engine.at(*site, kind))?;
            let length_address = engine
                .memory
                .storage_child_v1(
                    address,
                    length.offset,
                    length.layout,
                    length_row,
                    address.pointer.access,
                )
                .map_err(|kind| engine.at(*site, kind))?;
            if !matches!(
                length_row.kind,
                StorageLayoutKindV1::Scalar(ScalarType::Index)
            ) || engine.target.scalar_bytes(ScalarType::Index) != Some(length_row.size as usize)
            {
                return Err(engine.at(
                    *site,
                    storage_violation_v1("slice length layout differs from target"),
                ));
            }
            let elements = scalar_nonnegative_usize(
                engine
                    .memory
                    .storage_scalar_v1(
                        &length_address,
                        ScalarType::Index,
                        child_access,
                        engine.target,
                        invocation,
                    )
                    .map_err(|kind| engine.at(*site, kind))?,
                engine.target,
            )
            .map_err(|kind| engine.at(*site, kind))?;
            let element_row = storage_row_v1(engine, *element, site)?;
            let byte_len = elements
                .checked_mul(storage_extent_v1(element_row).map_err(|kind| engine.at(*site, kind))?)
                .ok_or_else(|| {
                    engine.at(*site, SimulationExecutionErrorKindV1::PointerOffsetOverflow)
                })?;
            let mut pointer = pointer.pointer().clone();
            let end = pointer.byte_offset.checked_add(byte_len).ok_or_else(|| {
                engine.at(*site, SimulationExecutionErrorKindV1::PointerOffsetOverflow)
            })?;
            if end > pointer.upper_bound {
                return Err(engine.at(
                    *site,
                    storage_violation_v1("slice extent exceeds its bounded referent"),
                ));
            }
            pointer.upper_bound = end;
            if let Type::Scalar(scalar) = result.element.as_ref() {
                RuntimeValue::Slice(SliceValue {
                    allocation: pointer.allocation,
                    elements,
                    element: *scalar,
                    address_space: pointer.address_space,
                    access: pointer.access,
                    byte_offset: pointer.byte_offset,
                    byte_len,
                    abi_argument_ordinal: pointer.abi_argument_ordinal,
                    storage_guard: pointer.storage_guard,
                    generic_exposed: pointer.generic_exposed,
                })
            } else {
                RuntimeValue::StorageSlice(StorageSliceV1 {
                    address: StorageAddressV1 {
                        pointer,
                        layout: *element,
                    },
                    elements,
                })
            }
        }
        _ => {
            return Err(engine.at(
                *site,
                storage_violation_v1("aggregate storage has no direct SSA read value"),
            ));
        }
    };
    storage_observe_v1(engine, address, width, false, site)?;
    Ok(value)
}

fn storage_pointer_input_v1(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    value: &RuntimeValue,
    representation: StoragePointerV1,
    invocation: SimulationInvocationV1,
    site: &CompactSite,
) -> Result<StoragePointerPayloadV1, SimulationExecutionErrorV1> {
    let payload = match value {
        RuntimeValue::Pointer(pointer) => {
            let row = storage_row_v1(engine, representation.pointee, site)?;
            if !matches!(row.kind, StorageLayoutKindV1::Scalar(scalar) if scalar == pointer.element)
            {
                return Err(engine.at(
                    *site,
                    storage_violation_v1("scalar pointer changes its referent layout"),
                ));
            }
            StoragePointerPayloadV1::Scalar(pointer.clone())
        }
        RuntimeValue::StoragePointer(address) if address.layout == representation.pointee => {
            StoragePointerPayloadV1::Object(address.clone())
        }
        _ => {
            return Err(engine.at(
                *site,
                storage_violation_v1("storage pointer write requires its exact pointer value"),
            ));
        }
    };
    engine
        .memory
        .storage_validate_pointer_v1(&payload, representation, invocation)
        .map_err(|kind| engine.at(*site, kind))?;
    Ok(payload)
}

fn snapshot_scalar_v1(
    snapshot: &mut StorageSnapshotV1,
    offset: usize,
    value: ScalarBitsV1,
    target: SimulationTargetV1,
    accounting: &StorageAccountingV1,
) -> Result<(), SimulationExecutionErrorKindV1> {
    let width = target
        .scalar_bytes(value.ty())
        .ok_or(storage_violation_v1("storage scalar width unavailable"))?;
    let end = offset
        .checked_add(width)
        .filter(|end| *end <= snapshot.width())
        .ok_or(SimulationExecutionErrorKindV1::PointerOffsetOverflow)?;
    accounting.charge(
        width
            .checked_mul(2)
            .ok_or(storage_violation_v1("scalar encoding work overflow"))?,
    )?;
    snapshot.bytes[offset..end].copy_from_slice(&value.bits().to_le_bytes()[..width]);
    snapshot.initialized[offset..end].fill(true);
    Ok(())
}

fn snapshot_pointer_v1(
    snapshot: &mut StorageSnapshotV1,
    offset: usize,
    representation: StoragePointerV1,
    value: StoragePointerPayloadV1,
    accounting: &StorageAccountingV1,
) -> Result<(), SimulationExecutionErrorKindV1> {
    let width = usize::from(representation.stored_bits / 8);
    let end = offset
        .checked_add(width)
        .filter(|end| *end <= snapshot.width())
        .ok_or(SimulationExecutionErrorKindV1::PointerOffsetOverflow)?;
    storage_reserve_v1(&mut snapshot.relocations, 1, accounting)?;
    accounting.charge(width)?;
    snapshot.initialized[offset..end].fill(true);
    snapshot.relocations.push(StorageRelocationV1 {
        start: offset,
        end,
        representation,
        value,
    });
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn storage_write_value_v1(
    engine: &mut Engine<'_, impl SimulationEventSinkV1>,
    address: &StorageAddressV1,
    row: &fe2o3_kernel_ir::StorageLayoutV1,
    value: &RuntimeValue,
    access: MemoryAccess,
    invocation: SimulationInvocationV1,
    site: &CompactSite,
) -> Result<(), SimulationExecutionErrorV1> {
    let width = storage_extent_v1(row).map_err(|kind| engine.at(*site, kind))?;
    engine
        .memory
        .storage_validate_v1(address, access, width, true, invocation)
        .map_err(|kind| engine.at(*site, kind))?;
    let mut snapshot = StorageSnapshotV1::new(&engine.memory.storage_accounting)
        .map_err(|kind| engine.at(*site, kind))?;
    let prepared = (|| {
        storage_reserve_v1(
            &mut snapshot.bytes,
            width,
            &engine.memory.storage_accounting,
        )
        .map_err(|kind| engine.at(*site, kind))?;
        storage_reserve_v1(
            &mut snapshot.initialized,
            width,
            &engine.memory.storage_accounting,
        )
        .map_err(|kind| engine.at(*site, kind))?;
        engine.charge_steps(
            site,
            width.checked_mul(2).ok_or_else(|| {
                engine.at(
                    *site,
                    storage_violation_v1("storage value initialization work overflow"),
                )
            })?,
        )?;
        snapshot.bytes.resize(width, 0);
        snapshot.initialized.resize(width, false);
        match (&row.kind, value) {
            (StorageLayoutKindV1::Scalar(expected), RuntimeValue::Scalar(actual)) => {
                if actual.ty() != *expected || engine.target.scalar_bytes(*expected) != Some(width)
                {
                    return Err(engine.at(
                        *site,
                        storage_violation_v1("storage scalar write changes exact target type"),
                    ));
                }
                snapshot_scalar_v1(
                    &mut snapshot,
                    0,
                    *actual,
                    engine.target,
                    &engine.memory.storage_accounting,
                )
                .map_err(|kind| engine.at(*site, kind))?;
            }
            (StorageLayoutKindV1::Pointer(representation), _) => {
                let value =
                    storage_pointer_input_v1(engine, value, *representation, invocation, site)?;
                snapshot_pointer_v1(
                    &mut snapshot,
                    0,
                    *representation,
                    value,
                    &engine.memory.storage_accounting,
                )
                .map_err(|kind| engine.at(*site, kind))?;
            }
            (StorageLayoutKindV1::Vector(expected), RuntimeValue::StorageVector(actual)) => {
                let payload = engine
                    .memory
                    .storage_vectors
                    .get(actual.index)
                    .filter(|payload| payload.ty == *expected && actual.ty == *expected)
                    .ok_or_else(|| {
                        engine.at(
                            *site,
                            storage_violation_v1("vector write uses a foreign or mistyped value"),
                        )
                    })?;
                if payload.lanes.len() != usize::from(expected.lanes)
                    || !expected
                        .byte_width()
                        .and_then(|value| usize::try_from(value).ok())
                        .is_some_and(|represented| represented <= width)
                {
                    return Err(
                        engine.at(*site, storage_violation_v1("vector write extent mismatch"))
                    );
                }
                let lane_width = engine
                    .target
                    .scalar_bytes(expected.element)
                    .ok_or_else(|| {
                        engine.at(
                            *site,
                            storage_violation_v1("vector scalar width unavailable"),
                        )
                    })?;
                for (logical, lane) in payload.lanes.iter().enumerate() {
                    snapshot_scalar_v1(
                        &mut snapshot,
                        vector_physical_lane_v1(*expected, logical) * lane_width,
                        *lane,
                        engine.target,
                        &engine.memory.storage_accounting,
                    )
                    .map_err(|kind| engine.at(*site, kind))?;
                }
            }
            (
                StorageLayoutKindV1::Slice {
                    element,
                    data,
                    length,
                    ..
                },
                _,
            ) => {
                let data_row = storage_row_v1(engine, data.layout, site)?;
                let length_row = storage_row_v1(engine, length.layout, site)?;
                let StorageLayoutKindV1::Pointer(representation) = data_row.kind else {
                    return Err(engine.at(
                        *site,
                        storage_violation_v1("slice data field is not a pointer"),
                    ));
                };
                if !matches!(
                    length_row.kind,
                    StorageLayoutKindV1::Scalar(ScalarType::Index)
                ) || engine.target.scalar_bytes(ScalarType::Index)
                    != Some(length_row.size as usize)
                {
                    return Err(engine.at(
                        *site,
                        storage_violation_v1("slice length layout differs from target"),
                    ));
                }
                let (pointer, elements) = match value {
                    RuntimeValue::Slice(slice) => {
                        let upper_bound = slice
                            .byte_offset
                            .checked_add(slice.byte_len)
                            .ok_or_else(|| {
                                engine.at(
                                    *site,
                                    SimulationExecutionErrorKindV1::PointerOffsetOverflow,
                                )
                            })?;
                        (
                            RuntimeValue::Pointer(PointerValue {
                                allocation: slice.allocation,
                                byte_offset: slice.byte_offset,
                                element: slice.element,
                                address_space: slice.address_space,
                                access: slice.access,
                                lower_bound: slice.byte_offset,
                                upper_bound,
                                abi_argument_ordinal: slice.abi_argument_ordinal,
                                storage_guard: slice.storage_guard,
                                generic_exposed: slice.generic_exposed,
                            }),
                            slice.elements,
                        )
                    }
                    RuntimeValue::StorageSlice(slice) if slice.address.layout == *element => (
                        RuntimeValue::StoragePointer(slice.address.clone()),
                        slice.elements,
                    ),
                    _ => {
                        return Err(engine.at(
                            *site,
                            storage_violation_v1("slice write requires its exact slice value"),
                        ));
                    }
                };
                let value =
                    storage_pointer_input_v1(engine, &pointer, representation, invocation, site)?;
                let element_row = storage_row_v1(engine, *element, site)?;
                let bytes = elements
                    .checked_mul(
                        storage_extent_v1(element_row).map_err(|kind| engine.at(*site, kind))?,
                    )
                    .ok_or_else(|| {
                        engine.at(*site, SimulationExecutionErrorKindV1::PointerOffsetOverflow)
                    })?;
                let pointer_end =
                    value
                        .pointer()
                        .byte_offset
                        .checked_add(bytes)
                        .ok_or_else(|| {
                            engine.at(*site, SimulationExecutionErrorKindV1::PointerOffsetOverflow)
                        })?;
                if pointer_end > value.pointer().upper_bound {
                    return Err(engine.at(
                        *site,
                        storage_violation_v1("slice value exceeds its bounded referent"),
                    ));
                }
                let length_value = u64::try_from(elements)
                    .ok()
                    .and_then(|length| ScalarBitsV1::index(length, engine.target).ok())
                    .ok_or_else(|| {
                        engine.at(
                            *site,
                            storage_violation_v1("slice length exceeds target index width"),
                        )
                    })?;
                snapshot_pointer_v1(
                    &mut snapshot,
                    usize::try_from(data.offset).map_err(|_| {
                        engine.at(*site, SimulationExecutionErrorKindV1::PointerOffsetOverflow)
                    })?,
                    representation,
                    value,
                    &engine.memory.storage_accounting,
                )
                .map_err(|kind| engine.at(*site, kind))?;
                snapshot_scalar_v1(
                    &mut snapshot,
                    usize::try_from(length.offset).map_err(|_| {
                        engine.at(*site, SimulationExecutionErrorKindV1::PointerOffsetOverflow)
                    })?,
                    length_value,
                    engine.target,
                    &engine.memory.storage_accounting,
                )
                .map_err(|kind| engine.at(*site, kind))?;
            }
            _ => {
                return Err(engine.at(
                    *site,
                    storage_violation_v1("storage write has no matching represented value"),
                ));
            }
        }
        engine
            .memory
            .storage_prepare_copy_v1(address, &snapshot)
            .map_err(|kind| engine.at(*site, kind))?;
        let writer = if address.pointer.address_space == AddressSpace::Workgroup {
            Some(
                invocation_local_ordinal(invocation)
                    .and_then(|value| value.checked_add(1))
                    .ok_or_else(|| {
                        engine.at(
                            *site,
                            storage_violation_v1("storage workgroup writer ordinal overflow"),
                        )
                    })?,
            )
        } else {
            None
        };
        storage_observe_v1(engine, address, width, true, site)?;
        engine
            .memory
            .storage_commit_copy_v1(address, &snapshot, writer);
        Ok(())
    })();
    snapshot.release(&engine.memory.storage_accounting);
    prepared
}
