//! Typed storage operations use the current owner-qualified physical layout.
//! These steps describe execution; independent source/physical admission is
//! still required before a paired refinement can use them.
use super::byte_memory_v30::{
    ByteMemoryContextNamesV30 as Context, ByteMemoryStateNamesV30 as State,
};
use super::pointer_byte_operations_v30::{
    PointerByteEffectV30 as Effect, pointer_space, scalar_bytes,
};
use super::{Error, Inventory, Result, Writer};
use fe2o3_kernel_ir::{
    AccessMode, FormalIndexWidth, MemoryAccess, OperationKind, ScalarType, StorageCopyOverlapV1,
    StorageLayoutKindV1 as Kind, StorageOperationV1 as Storage, Type, VectorLayoutV12,
};
use std::{fmt::Write as _, mem::size_of};

macro_rules! emit {
    ($out:expr, $($arg:tt)*) => { write!($out, $($arg)*).map_err(|_| $out.error())? };
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Payload {
    Scalar {
        boolean: bool,
    },
    Pointer {
        space: usize,
    },
    Vector {
        lane_bytes: usize,
        lanes: u16,
        factor: u16,
    },
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Action {
    Value {
        address: usize,
        value: usize,
        object_bytes: u64,
        bytes: usize,
        holder_space: usize,
        index_bytes: usize,
        alignment: u32,
        payload: Payload,
        writing: bool,
    },
    Copy {
        source: usize,
        destination: usize,
        bytes: usize,
        source_space: usize,
        destination_space: usize,
        index_bytes: usize,
        source_alignment: u32,
        destination_alignment: u32,
        nonoverlapping: bool,
    },
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) struct StorageByteOperationV37 {
    action: Action,
}

fn unsupported() -> Error {
    Error::Statement("actual typed storage payload or operation is not modeled")
}

fn access(
    ty: &Type,
    access: MemoryAccess,
    writing: bool,
) -> Result<(fe2o3_kernel_ir::StorageLayoutIdV1, usize)> {
    let Type::Pointer(pointer) = ty else {
        return Err(unsupported());
    };
    let Type::StorageObject(layout) = pointer.pointee.as_ref() else {
        return Err(unsupported());
    };
    if pointer.address_space != access.address_space
        || access.volatile
        || !access.alignment.is_power_of_two()
        || writing && pointer.access == AccessMode::ReadOnly
        || !writing && pointer.access == AccessMode::WriteOnly
    {
        return Err(Error::Statement(
            "actual typed storage access contract differs",
        ));
    }
    Ok((*layout, pointer_space(pointer.address_space)?))
}

impl StorageByteOperationV37 {
    pub(super) fn derive(
        input: &Inventory<'_>,
        operation: usize,
        width: FormalIndexWidth,
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<Self>> {
        out.budget.charge_work(6)?;
        let row = input.operations().get(operation).ok_or_else(unsupported)?;
        let OperationKind::Storage(storage) = row.operation.kind else {
            return Ok(None);
        };
        let uses = input
            .uses()
            .get(row.operands.clone())
            .ok_or_else(unsupported)?;
        let operand = |ordinal: usize| -> Result<(usize, &Type)> {
            let definition = uses.get(ordinal).ok_or_else(unsupported)?.definition;
            Ok((
                definition,
                input
                    .definitions()
                    .get(definition)
                    .ok_or_else(unsupported)?
                    .ty,
            ))
        };
        let layout = |id: fe2o3_kernel_ir::StorageLayoutIdV1| {
            input
                .owner()
                .module()
                .storage_layouts
                .get(id.0 as usize)
                .ok_or_else(unsupported)
        };
        out.budget.charge_work(32)?;
        let action = match storage {
            Storage::ReadValue { access: memory, .. }
            | Storage::WriteValue { access: memory, .. } => {
                let writing = matches!(storage, Storage::WriteValue { .. });
                let (address, ty) = operand(0)?;
                let (id, holder_space) = access(ty, memory, writing)?;
                let layout = layout(id)?;
                let (value, value_type) = if writing {
                    if uses.len() != 2 || !row.results.is_empty() {
                        return Err(unsupported());
                    }
                    operand(1)?
                } else {
                    if uses.len() != 1 || row.results.len() != 1 {
                        return Err(unsupported());
                    }
                    let value = row.results.start;
                    (
                        value,
                        input.definitions().get(value).ok_or_else(unsupported)?.ty,
                    )
                };
                let (payload, bytes) = match &layout.kind {
                    Kind::Scalar(scalar) if value_type == &Type::Scalar(*scalar) => {
                        let bytes = scalar_bytes(*scalar, width)?;
                        if layout.size != bytes as u64 {
                            return Err(unsupported());
                        }
                        (
                            Payload::Scalar {
                                boolean: *scalar == ScalarType::Bool,
                            },
                            bytes,
                        )
                    }
                    Kind::Pointer(stored) => {
                        let Type::Pointer(value) = value_type else {
                            return Err(unsupported());
                        };
                        if value.pointee.as_ref() != &Type::StorageObject(stored.pointee)
                            || value.address_space != stored.value_space
                            || value.access != stored.access
                            || !matches!(stored.stored_bits, 8 | 16 | 32 | 64 | 128)
                            || layout.size != u64::from(stored.stored_bits / 8)
                        {
                            return Err(unsupported());
                        }
                        // Stored representation width comes from the exact layout,
                        // never from the canonical INDEX width or holder space.
                        (
                            Payload::Pointer {
                                space: pointer_space(stored.value_space)?,
                            },
                            usize::from(stored.stored_bits / 8),
                        )
                    }
                    Kind::Vector(vector) if value_type == &Type::Vector(*vector) => {
                        vector.validate().map_err(|_| unsupported())?;
                        let lane_bytes = scalar_bytes(vector.element, width)?;
                        let bytes = lane_bytes
                            .checked_mul(usize::from(vector.lanes))
                            .ok_or(super::Resource::Arithmetic)?;
                        if bytes as u64 > layout.size {
                            return Err(unsupported());
                        }
                        let factor = match vector.layout {
                            VectorLayoutV12::Contiguous => 1,
                            VectorLayoutV12::Interleaved { factor } => factor,
                        };
                        (
                            Payload::Vector {
                                lane_bytes,
                                lanes: vector.lanes,
                                factor,
                            },
                            bytes,
                        )
                    }
                    _ => return Err(unsupported()),
                };
                Action::Value {
                    address,
                    value,
                    object_bytes: layout.size,
                    bytes,
                    holder_space,
                    index_bytes: scalar_bytes(ScalarType::Index, width)?,
                    alignment: memory.alignment,
                    payload,
                    writing,
                }
            }
            Storage::CopyObject {
                source_access,
                destination_access,
                overlap,
                ..
            } => {
                if uses.len() != 2 || !row.results.is_empty() {
                    return Err(unsupported());
                }
                let (source, source_type) = operand(0)?;
                let (destination, destination_type) = operand(1)?;
                let (source_layout, source_space) = access(source_type, source_access, false)?;
                let (destination_layout, destination_space) =
                    access(destination_type, destination_access, true)?;
                if source_layout != destination_layout {
                    return Err(unsupported());
                }
                Action::Copy {
                    source,
                    destination,
                    bytes: usize::try_from(layout(source_layout)?.size)
                        .map_err(|_| super::Resource::Arithmetic)?,
                    source_space,
                    destination_space,
                    index_bytes: scalar_bytes(ScalarType::Index, width)?,
                    source_alignment: source_access.alignment,
                    destination_alignment: destination_access.alignment,
                    nonoverlapping: overlap == StorageCopyOverlapV1::NonOverlapping,
                }
            }
            _ => return Ok(None),
        };
        Ok(Some(Self { action }))
    }

    pub(super) fn effect(&self) -> Effect {
        match self.action {
            Action::Value {
                address,
                value,
                bytes,
                alignment,
                writing: false,
                ..
            } => Effect::Read {
                pointer: address,
                bytes,
                alignment,
                result: value,
            },
            Action::Value {
                address,
                value,
                bytes,
                alignment,
                writing: true,
                ..
            } => Effect::Write {
                pointer: address,
                bytes,
                alignment,
                value,
            },
            Action::Copy {
                source,
                destination,
                bytes,
                source_alignment,
                destination_alignment,
                nonoverlapping,
                ..
            } => Effect::Copy {
                source,
                destination,
                bytes,
                source_alignment,
                destination_alignment,
                nonoverlapping,
            },
        }
    }

    pub(super) fn emit_step(
        &self,
        before: State<'_>,
        after: State<'_>,
        context: Context<'_>,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        out.budget.charge_work(1)?;
        let values = before.values;
        let memory = before.memory;
        let valid = before.valid;
        let endian = context.little_endian;
        match self.action {
            Action::Value {
                address,
                value,
                object_bytes,
                bytes,
                holder_space,
                index_bytes,
                alignment,
                payload,
                writing,
            } => {
                emit!(
                    out,
                    " let {} = {valid} && match {values}[{address}] {{ MemoryValueV30::Pointer(p) => byte_pointer_type_v30(p, {holder_space}, {index_bytes}) && byte_range_aligned_v30({memory}, p, {object_bytes}, {alignment}) && ",
                    after.valid
                );
                if writing {
                    match payload {
                        Payload::Scalar { boolean } => {
                            emit!(
                                out,
                                "match {values}[{value}] {{ MemoryValueV30::Scalar(v) => 0 <= v < "
                            );
                            if boolean {
                                emit!(out, "2");
                            } else {
                                emit!(out, "memory_value_modulus_v30({bytes})");
                            }
                            emit!(out, ", _ => false }}");
                        }
                        Payload::Pointer { space } => emit!(
                            out,
                            "match {values}[{value}] {{ MemoryValueV30::Pointer(v) => byte_pointer_type_v30(v, {space}, {bytes}), _ => false }}"
                        ),
                        Payload::Vector {
                            lane_bytes, lanes, ..
                        } => emit!(
                            out,
                            "match {values}[{value}] {{ MemoryValueV30::Vector(v) => v.len() == {lanes} && (forall|lane: int| 0 <= lane < v.len() ==> 0 <= v[lane] < memory_value_modulus_v30({lane_bytes})), _ => false }}"
                        ),
                    }
                } else {
                    match payload {
                        Payload::Pointer { space } => emit!(
                            out,
                            "byte_pointer_load_valid_v37({memory}, p, {bytes}, {endian}) && byte_pointer_type_v30(byte_pointer_load_v37({memory}, p, {bytes}, {endian}), {space}, {bytes})"
                        ),
                        Payload::Scalar { boolean } => {
                            emit!(
                                out,
                                "byte_scalar_range_initialized_v37({memory}, p, {bytes})"
                            );
                            if boolean {
                                emit!(out, " && byte_load_v30({memory}, p, {bytes}, {endian}) < 2");
                            }
                        }
                        Payload::Vector { .. } => emit!(
                            out,
                            "byte_scalar_range_initialized_v37({memory}, p, {bytes})"
                        ),
                    }
                }
                emit!(out, ", _ => false }};\n");
                if writing {
                    emit!(
                        out,
                        " let {} = if {} {{ match ({values}[{address}], {values}[{value}]) {{ ",
                        after.memory,
                        after.valid
                    );
                    match payload {
                        Payload::Scalar { .. } => emit!(
                            out,
                            "(MemoryValueV30::Pointer(p), MemoryValueV30::Scalar(v)) => byte_store_v30({memory}, p, {bytes}, v, {endian})"
                        ),
                        Payload::Pointer { .. } => emit!(
                            out,
                            "(MemoryValueV30::Pointer(p), MemoryValueV30::Pointer(v)) => byte_pointer_store_v37({memory}, p, {bytes}, v, {endian})"
                        ),
                        Payload::Vector {
                            lane_bytes, factor, ..
                        } => emit!(
                            out,
                            "(MemoryValueV30::Pointer(p), MemoryValueV30::Vector(v)) => byte_vector_store_v30({memory}, p, {lane_bytes}, v, {factor}, {endian})"
                        ),
                    }
                    emit!(
                        out,
                        ", _ => {memory} }} }} else {{ {memory} }};\n let {} = {values};\n",
                        after.values
                    );
                } else {
                    emit!(
                        out,
                        " let {} = {values}.update({value}, if {} {{ match {values}[{address}] {{ MemoryValueV30::Pointer(p) => ",
                        after.values,
                        after.valid
                    );
                    match payload {
                        Payload::Scalar { .. } => emit!(
                            out,
                            "MemoryValueV30::Scalar(byte_load_v30({memory}, p, {bytes}, {endian}))"
                        ),
                        Payload::Pointer { .. } => emit!(
                            out,
                            "MemoryValueV30::Pointer(byte_pointer_load_v37({memory}, p, {bytes}, {endian}))"
                        ),
                        Payload::Vector {
                            lane_bytes,
                            lanes,
                            factor,
                        } => emit!(
                            out,
                            "MemoryValueV30::Vector(byte_vector_load_v30({memory}, p, {lane_bytes}, {lanes}, {factor}, {endian}))"
                        ),
                    }
                    emit!(
                        out,
                        ", _ => MemoryValueV30::Undefined }} }} else {{ MemoryValueV30::Undefined }});\n let {} = {memory};\n",
                        after.memory
                    );
                }
            }
            Action::Copy {
                source,
                destination,
                bytes,
                source_space,
                destination_space,
                index_bytes,
                source_alignment,
                destination_alignment,
                nonoverlapping,
            } => {
                emit!(
                    out,
                    " let {} = {valid} && match ({values}[{source}], {values}[{destination}]) {{ (MemoryValueV30::Pointer(source), MemoryValueV30::Pointer(destination)) => byte_pointer_type_v30(source, {source_space}, {index_bytes}) && byte_pointer_type_v30(destination, {destination_space}, {index_bytes}) && byte_copy_object_valid_v37({memory}, source, destination, {bytes}, {source_alignment}, {destination_alignment}, {nonoverlapping}), _ => false }};\n",
                    after.valid
                );
                emit!(
                    out,
                    " let {} = if {} {{ match ({values}[{source}], {values}[{destination}]) {{ (MemoryValueV30::Pointer(source), MemoryValueV30::Pointer(destination)) => byte_copy_object_v37({memory}, source, destination, {bytes}), _ => {memory} }} }} else {{ {memory} }};\n let {} = {values};\n",
                    after.memory,
                    after.valid,
                    after.values
                );
            }
        }
        emit!(
            out,
            " let {} = {};\n let {} = {};\n",
            after.generations,
            before.generations,
            after.frames,
            before.frames
        );
        Ok(())
    }
}

pub(super) fn headers() -> usize {
    2 * size_of::<StorageByteOperationV37>()
        + 2 * size_of::<Result<Option<StorageByteOperationV37>>>()
        + 2 * size_of::<State<'_>>()
        + size_of::<Context<'_>>()
        + size_of::<Effect>()
        + size_of::<([usize; 20], [&Type; 8], [Result<()>; 4])>()
}
