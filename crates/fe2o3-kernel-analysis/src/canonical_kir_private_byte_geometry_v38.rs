//! Exact-owner geometry. SSA dependencies are traversed once, without assuming
//! that the inventory's physical block order is a dominance order.
use super::*;
use fe2o3_kernel_ir::{
    AccessMode, CastKind, MemoryAccess, ScalarType, StorageCopyOverlapV1,
    StorageLayoutKindV1 as LayoutKind, StorageLayoutV1, StorageOperationV1 as Storage,
    StorageProjectionV1 as Projection,
};

fn layout<'a>(inventory: &'a Inventory<'_>, id: LayoutId) -> R<&'a StorageLayoutV1> {
    inventory
        .owner()
        .module()
        .storage_layouts
        .get(id.0 as usize)
        .ok_or_else(arithmetic)
}

fn object(ty: &Type) -> Option<Object> {
    match ty {
        Type::Scalar(scalar) => Some(Object::Scalar(*scalar)),
        Type::Vector(vector) => Some(Object::Vector(*vector)),
        Type::StorageObject(id) => Some(Object::Storage(*id)),
        _ => None,
    }
}

fn scalar_bytes(scalar: ScalarType) -> Option<u64> {
    scalar.bit_width().map(|bits| u64::from(bits.div_ceil(8)))
}

fn object_bytes(inventory: &Inventory<'_>, object: Object) -> R<Option<u64>> {
    Ok(match object {
        Object::Scalar(scalar) => scalar_bytes(scalar),
        Object::Vector(vector) => vector.byte_width().map(u64::from),
        Object::Storage(id) => Some(layout(inventory, id)?.size),
    })
}

fn payload_bytes(inventory: &Inventory<'_>, object: Object) -> R<Option<u64>> {
    let Object::Storage(id) = object else {
        return object_bytes(inventory, object);
    };
    let row = layout(inventory, id)?;
    let bytes = match row.kind {
        // INDEX representation comes from this exact admitted storage layout.
        LayoutKind::Scalar(ScalarType::Index) => Some(row.size),
        LayoutKind::Scalar(scalar) => scalar_bytes(scalar),
        LayoutKind::Vector(vector) => vector.byte_width().map(u64::from),
        LayoutKind::Pointer(pointer) => Some(u64::from(pointer.stored_bits / 8)),
        _ => None,
    };
    Ok(bytes.filter(|bytes| *bytes <= row.size))
}

fn operand(inventory: &Inventory<'_>, operation: usize, ordinal: usize) -> R<usize> {
    let row = inventory
        .operations()
        .get(operation)
        .ok_or_else(arithmetic)?;
    inventory
        .uses()
        .get(row.operands.clone())
        .and_then(|rows| rows.get(ordinal))
        .map(|row| row.definition)
        .ok_or_else(arithmetic)
}

fn alignment(base: u32, offset: u64) -> u32 {
    if offset == 0 {
        base
    } else {
        base.min(1u32 << offset.trailing_zeros().min(31))
    }
}

fn form(
    parent: ByteAddress,
    offset: u64,
    bytes: u64,
    object: Object,
    elements: u64,
    fact: &mut Fact,
) -> R<Option<ByteAddress>> {
    if offset > parent.range.length || bytes > parent.range.length - offset {
        fact.require(Obligation::Bounds);
        return Ok(None);
    }
    let start = parent
        .range
        .start
        .checked_add(offset)
        .ok_or_else(arithmetic)?;
    let address = ByteAddress {
        range: ByteRange {
            allocation: parent.range.allocation,
            start,
            length: bytes,
        },
        alignment: alignment(parent.alignment, offset),
        object,
        elements,
    };
    fact.ranges[0] = Some(address.range);
    Ok(Some(address))
}

fn project(
    inventory: &Inventory<'_>,
    operation: usize,
    parent: ByteAddress,
    constants: &[Option<u64>],
    fact: &mut Fact,
    budget: &mut Budget<'_>,
) -> R<Option<ByteAddress>> {
    charge(budget, 16)?;
    let row = &inventory.operations()[operation];
    match &row.operation.kind {
        OperationKind::Cast {
            kind: CastKind::RestrictPointerAccess,
            ..
        } => Ok(Some(parent)),
        OperationKind::GetElementPointer { .. } => {
            let Some(index) = constants[operand(inventory, operation, 1)?] else {
                fact.require(Obligation::Bounds);
                return Ok(None);
            };
            let Some(stride) = object_bytes(inventory, parent.object)? else {
                fact.require(Obligation::Operation);
                return Ok(None);
            };
            if index > parent.elements {
                fact.require(Obligation::Bounds);
                return Ok(None);
            }
            let Some(offset) = index.checked_mul(stride) else {
                fact.require(Obligation::Bounds);
                return Ok(None);
            };
            let Some(bytes) = parent.range.length.checked_sub(offset) else {
                fact.require(Obligation::Bounds);
                return Ok(None);
            };
            form(
                parent,
                offset,
                bytes,
                parent.object,
                parent.elements - index,
                fact,
            )
        }
        OperationKind::Storage(Storage::Project { step, .. }) => {
            let Object::Storage(id) = parent.object else {
                fact.require(Obligation::Operation);
                return Ok(None);
            };
            let row = layout(inventory, id)?;
            let field = match (&row.kind, step) {
                (
                    LayoutKind::Record(fields) | LayoutKind::Union(fields),
                    Projection::Field(index),
                ) => fields
                    .get(*index as usize)
                    .copied()
                    .ok_or_else(arithmetic)?,
                (LayoutKind::Slice { data, length, .. }, Projection::Field(index)) => match index {
                    0 => *data,
                    1 => *length,
                    _ => return Err(arithmetic()),
                },
                (
                    LayoutKind::Array {
                        element,
                        length,
                        stride,
                    },
                    Projection::ArrayIndex(_),
                ) => {
                    let Some(index) = constants[operand(inventory, operation, 1)?] else {
                        fact.require(Obligation::Bounds);
                        return Ok(None);
                    };
                    if index >= *length {
                        fact.require(Obligation::Bounds);
                        return Ok(None);
                    }
                    let Some(offset) = index.checked_mul(*stride) else {
                        fact.require(Obligation::Bounds);
                        return Ok(None);
                    };
                    fe2o3_kernel_ir::StorageFieldV1 {
                        offset,
                        layout: *element,
                    }
                }
                (_, Projection::Variant { .. } | Projection::VariantForWrite { .. }) => {
                    fact.require(Obligation::ActiveView);
                    fact.require(Obligation::TagContract);
                    return Ok(None);
                }
                _ => {
                    fact.require(Obligation::Operation);
                    return Ok(None);
                }
            };
            let child = layout(inventory, field.layout)?;
            if row.size > parent.range.length {
                fact.require(Obligation::Bounds);
                return Ok(None);
            }
            form(
                parent,
                field.offset,
                child.size,
                Object::Storage(field.layout),
                1,
                fact,
            )
        }
        _ => Err(arithmetic()),
    }
}

fn access(
    inventory: &Inventory<'_>,
    operation: usize,
    argument: usize,
    addresses: &[Option<ByteAddress>],
    access: MemoryAccess,
    writing: bool,
    whole: bool,
    fact: &mut Fact,
    budget: &mut Budget<'_>,
) -> R<Option<ByteRange>> {
    charge(budget, 15)?;
    let definition = operand(inventory, operation, argument)?;
    let Type::Pointer(pointer) = inventory.definitions()[definition].ty else {
        return Err(arithmetic());
    };
    if pointer.address_space != access.address_space
        || access.volatile
        || writing && pointer.access == AccessMode::ReadOnly
        || !writing && pointer.access == AccessMode::WriteOnly
    {
        fact.require(Obligation::Operation);
    }
    let Some(address) = addresses[definition] else {
        fact.require(if pointer.address_space == AddressSpace::Private {
            Obligation::Address
        } else {
            Obligation::ExternalMemory
        });
        fact.require(Obligation::Currentness);
        fact.require(Obligation::Bounds);
        fact.require(Obligation::Alignment);
        return Ok(None);
    };
    let bytes = if whole {
        object_bytes(inventory, address.object)?
    } else {
        payload_bytes(inventory, address.object)?
    };
    let Some(bytes) = bytes else {
        fact.require(Obligation::Operation);
        return Ok(None);
    };
    // Full object formation, not merely payload access, remains bounded.
    if object_bytes(inventory, address.object)?.is_none_or(|size| size > address.range.length) {
        fact.require(Obligation::Bounds);
    }
    if !access.alignment.is_power_of_two() || access.alignment > address.alignment {
        fact.require(Obligation::Alignment);
    }
    if !fact.is_proven() {
        return Ok(None);
    }
    Ok(Some(ByteRange {
        length: bytes,
        ..address.range
    }))
}

pub(super) fn derive(inventory: &Inventory<'_>, budget: &mut Budget<'_>) -> R<Census> {
    let definitions = inventory.definitions().len();
    let operations = inventory.operations().len();
    let mut census = Census {
        allocations: scratch(operations, budget)?,
        addresses: filled(definitions, None, budget)?,
        facts: filled(operations, Fact::NONE, budget)?,
        events: filled(operations, Event::None, budget)?,
    };
    let mut constants = filled(definitions, None, budget)?;
    let mut heads = filled(definitions, None, budget)?;
    let mut next = filled(operations, None, budget)?;
    let mut pending = scratch::<usize>(definitions, budget)?;
    for row in inventory.operations() {
        charge(budget, 3)?;
        if let OperationKind::Constant(value) = &row.operation.kind {
            if row.results.len() == 1 {
                constants[row.results.start] = typed::count_literal(value);
            }
        }
    }
    for (ordinal, row) in inventory.operations().iter().enumerate() {
        charge(budget, 10)?;
        let fact = &mut census.facts[ordinal];
        match &row.operation.kind {
            OperationKind::Alloca {
                element,
                count,
                address_space,
                alignment,
            } => {
                fact.kind = OperationKindV38::Allocate;
                if *address_space != AddressSpace::Private {
                    fact.require(Obligation::ExternalMemory);
                    continue;
                }
                let Some(object) = object(element) else {
                    fact.require(Obligation::Operation);
                    continue;
                };
                let Some(bytes) = object_bytes(inventory, object)? else {
                    fact.require(Obligation::Operation);
                    continue;
                };
                let count = if count.is_some() {
                    constants[operand(inventory, ordinal, 0)?]
                } else {
                    Some(1)
                };
                let Some(count) = count else {
                    fact.require(Obligation::Bounds);
                    continue;
                };
                let Some(extent) = count.checked_mul(bytes) else {
                    fact.require(Obligation::Bounds);
                    continue;
                };
                if !alignment.is_power_of_two() {
                    fact.require(Obligation::Alignment);
                    continue;
                }
                let address = ByteAddress {
                    range: ByteRange {
                        allocation: ordinal,
                        start: 0,
                        length: extent,
                    },
                    alignment: *alignment,
                    object,
                    elements: count,
                };
                census.allocations.push(Allocation {
                    operation: ordinal,
                    extent,
                });
                census.addresses[row.results.start] = Some(address);
                fact.ranges[0] = Some(address.range);
                census.events[ordinal] = Event::Reset(ordinal);
                pending.push(row.results.start);
            }
            OperationKind::GetElementPointer { .. }
            | OperationKind::Cast {
                kind: CastKind::RestrictPointerAccess,
                ..
            }
            | OperationKind::Storage(Storage::Project { .. }) => {
                // Restricting access preserves a pointer without dereferencing
                // it. Keep known geometry, but do not invent a liveness test.
                fact.kind = if matches!(row.operation.kind, OperationKind::Cast { .. }) {
                    OperationKindV38::None
                } else {
                    OperationKindV38::Project
                };
                if matches!(
                    row.operation.kind,
                    OperationKind::Storage(Storage::Project {
                        step: Projection::Variant { .. } | Projection::VariantForWrite { .. },
                        ..
                    })
                ) {
                    fact.require(Obligation::ActiveView);
                    fact.require(Obligation::TagContract);
                }
                let base = operand(inventory, ordinal, 0)?;
                next[ordinal] = heads[base];
                heads[base] = Some(ordinal);
            }
            _ => {}
        }
    }
    let mut cursor = 0;
    while cursor < pending.len() {
        charge(budget, 5)?;
        let definition = pending[cursor];
        cursor += 1;
        let parent = census.addresses[definition].ok_or_else(arithmetic)?;
        let mut dependent = heads[definition];
        while let Some(ordinal) = dependent {
            charge(budget, 5)?;
            dependent = next[ordinal];
            let result = inventory.operations()[ordinal].results.start;
            if let Some(address) = project(
                inventory,
                ordinal,
                parent,
                &constants,
                &mut census.facts[ordinal],
                budget,
            )? {
                if census.addresses[result].is_some() || pending.len() == pending.capacity() {
                    return Err(arithmetic());
                }
                census.addresses[result] = Some(address);
                pending.push(result);
            }
        }
    }
    for (ordinal, row) in inventory.operations().iter().enumerate() {
        charge(budget, 10)?;
        let fact = &mut census.facts[ordinal];
        let action = match row.operation.kind {
            OperationKind::Load { access, .. } => Some((false, access)),
            OperationKind::Store { access, .. } => Some((true, access)),
            OperationKind::Storage(Storage::ReadValue { access, .. }) => Some((false, access)),
            OperationKind::Storage(Storage::WriteValue { access, .. }) => Some((true, access)),
            _ => None,
        };
        if let Some((writing, memory)) = action {
            fact.kind = if writing {
                OperationKindV38::Write
            } else {
                OperationKindV38::Read
            };
            let range = access(
                inventory,
                ordinal,
                0,
                &census.addresses,
                memory,
                writing,
                false,
                fact,
                budget,
            )?;
            fact.ranges[0] = range;
            census.events[ordinal] = match (writing, range) {
                (false, Some(range)) => Event::Read(range),
                (true, Some(range)) => Event::Write(range),
                (true, None) => Event::UnknownWrite,
                (false, None) => {
                    fact.require(Obligation::Initialization);
                    Event::None
                }
            };
            continue;
        }
        match row.operation.kind {
            OperationKind::Storage(Storage::CopyObject {
                source_access,
                destination_access,
                overlap,
                ..
            }) => {
                fact.kind = OperationKindV38::Copy;
                let source = access(
                    inventory,
                    ordinal,
                    0,
                    &census.addresses,
                    source_access,
                    false,
                    true,
                    fact,
                    budget,
                )?;
                let destination = access(
                    inventory,
                    ordinal,
                    1,
                    &census.addresses,
                    destination_access,
                    true,
                    true,
                    fact,
                    budget,
                )?;
                fact.ranges = [source, destination];
                if let (Some(source), Some(destination)) = (source, destination) {
                    if source.length != destination.length {
                        return Err(arithmetic());
                    }
                    if overlap == StorageCopyOverlapV1::NonOverlapping
                        && source.allocation == destination.allocation
                        && source.start < destination.end()?
                        && destination.start < source.end()?
                        && source.length != 0
                    {
                        fact.require(Obligation::Operation);
                    }
                    census.events[ordinal] = if fact.is_proven() {
                        Event::Copy {
                            source,
                            destination,
                        }
                    } else {
                        Event::UnknownWrite
                    };
                } else {
                    census.events[ordinal] = Event::UnknownWrite;
                }
            }
            _ if fact.kind == OperationKindV38::Project => {
                if census.addresses[row.results.start].is_none() {
                    fact.require(Obligation::Address);
                    fact.require(Obligation::Currentness);
                    // A failed predecessor never silently discharges formation.
                    fact.require(Obligation::Bounds);
                    fact.require(Obligation::Alignment);
                }
            }
            _ if fact.kind == OperationKindV38::Allocate => {}
            _ if !row.effects.is_empty()
                || matches!(row.operation.kind, OperationKind::Call { .. }) =>
            {
                fact.kind = OperationKindV38::Unmodeled;
                fact.require(Obligation::Operation);
                if matches!(
                    row.operation.kind,
                    OperationKind::Storage(
                        Storage::ReadDiscriminant { .. } | Storage::SetDiscriminant { .. }
                    )
                ) {
                    fact.require(Obligation::TagContract);
                    fact.require(Obligation::ActiveView);
                }
                // Unknown aliases and effects cannot preserve a universal claim.
                census.events[ordinal] = Event::UnknownWrite;
            }
            _ => {}
        }
    }
    Ok(census)
}

// A fixed reusable slot bundle covers capture and its deepest access/project
// helpers. Vector backing and owned vector headers are paid by scratch itself.
pub(super) fn headers() -> R<usize> {
    header_sum(&[
        h::<&Inventory<'_>>()?,
        h::<&Type>()?,
        h::<&StorageLayoutV1>()?,
        h::<Option<&StorageLayoutV1>>()?,
        h::<&LayoutKind>()?,
        h::<&Projection>()?,
        h::<&crate::CanonicalKirOperationRefV1<'_>>()?,
        h::<&crate::CanonicalKirDefinitionRefV1<'_>>()?,
        h::<std::iter::Enumerate<std::slice::Iter<'_, crate::CanonicalKirOperationRefV1<'_>>>>()?,
        h::<std::slice::Iter<'_, crate::CanonicalKirOperationRefV1<'_>>>()?,
        h::<&[Option<ByteAddress>]>()?,
        h::<&[Option<u64>]>()?,
        h::<&mut Fact>()?,
        h::<Option<Object>>()?,
        h::<Option<ByteAddress>>()?,
        h::<Option<ByteRange>>()?,
        h::<Option<(bool, MemoryAccess)>>()?,
        h::<(bool, MemoryAccess)>()?,
        h::<MemoryAccess>()?,
        h::<fe2o3_kernel_ir::StorageFieldV1>()?,
        h::<&fe2o3_kernel_ir::PointerType>()?,
        header_copies::<usize>(20)?,
        header_copies::<Option<usize>>(6)?,
        header_copies::<u64>(10)?,
        header_copies::<Option<u64>>(4)?,
        header_copies::<u32>(3)?,
        header_copies::<bool>(4)?,
        h::<(ByteAddress, u64, u64, Object, u64, &mut Fact)>()?,
        h::<(
            &Inventory<'_>,
            usize,
            ByteAddress,
            &[Option<u64>],
            &mut Fact,
            &mut Budget<'_>,
        )>()?,
        h::<(
            &Inventory<'_>,
            usize,
            usize,
            &[Option<ByteAddress>],
            MemoryAccess,
            bool,
            bool,
            &mut Fact,
            &mut Budget<'_>,
        )>()?,
    ])
}
