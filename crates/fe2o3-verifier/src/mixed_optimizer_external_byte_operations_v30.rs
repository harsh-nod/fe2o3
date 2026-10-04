//! Checked actual pointer operations in the shared tagged-byte interpreter.
//! Derivation emits no text. The closed function dispatcher must reject every
//! unrecognized operation instead of treating it as an uninterpreted effect.
use super::byte_memory_v30::{ByteMemoryContextNamesV30, ByteMemoryStateNamesV30};
use super::{Error, Inventory, Resource, Result, Writer};
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, CastKind, FormalIndexWidth, OperationKind, ScalarType, Type,
};
use std::{fmt::Write as _, mem::size_of, ops::Range};

macro_rules! emit {
    ($out:expr, $($arg:tt)*) => { write!($out, $($arg)*).map_err(|_| $out.error())? };
}

pub(super) fn scalar_bytes(scalar: ScalarType, width: FormalIndexWidth) -> Result<usize> {
    Ok(match scalar {
        ScalarType::Bool | ScalarType::U8 | ScalarType::I8 => 1,
        ScalarType::U16 | ScalarType::I16 | ScalarType::F16 | ScalarType::Bf16 => 2,
        ScalarType::U32 | ScalarType::I32 | ScalarType::F32 => 4,
        ScalarType::U64 | ScalarType::I64 | ScalarType::F64 => 8,
        ScalarType::U128 | ScalarType::I128 => 16,
        ScalarType::Index => match width {
            FormalIndexWidth::Bits32 => 4,
            FormalIndexWidth::Bits64 => 8,
            FormalIndexWidth::Unknown => {
                return Err(Error::Statement("actual byte index width is unknown"));
            }
        },
    })
}

fn external(space: AddressSpace) -> bool {
    matches!(space, AddressSpace::Global | AddressSpace::Generic)
}

fn pointer_memory(space: AddressSpace) -> bool {
    external(space) || space == AddressSpace::Private
}

pub(super) fn pointer_space(space: AddressSpace) -> Result<usize> {
    match space {
        AddressSpace::Private => Ok(0),
        AddressSpace::Global => Ok(1),
        AddressSpace::Generic => Ok(2),
        _ => Err(Error::Statement(
            "actual byte pointer address space is not modeled",
        )),
    }
}

fn scalar(ty: &Type) -> Result<ScalarType> {
    match ty {
        Type::Scalar(scalar) => Ok(*scalar),
        _ => Err(Error::Statement(
            "byte memory requires a scalar payload; stored pointers are not modeled",
        )),
    }
}

#[derive(Clone, Copy)]
pub(super) enum PointerByteEffectV30 {
    None,
    Copy {
        source: usize,
        destination: usize,
        bytes: usize,
        source_alignment: u32,
        destination_alignment: u32,
        nonoverlapping: bool,
    },
    Read {
        pointer: usize,
        bytes: usize,
        alignment: u32,
        result: usize,
    },
    Write {
        pointer: usize,
        bytes: usize,
        alignment: u32,
        value: usize,
    },
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Action {
    Slice {
        input: usize,
        output: usize,
        index_bytes: usize,
        space: usize,
        data: bool,
    },
    Gep {
        base: usize,
        index: usize,
        output: usize,
        bytes: usize,
        index_bytes: usize,
        space: usize,
    },
    Cast {
        input: usize,
        output: usize,
        slice_index_bytes: Option<usize>,
        index_bytes: usize,
        space: usize,
    },
    Memory {
        pointer: usize,
        value: usize,
        bytes: usize,
        index_bytes: usize,
        space: usize,
        alignment: u32,
        boolean: bool,
        writing: bool,
    },
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) struct PointerByteOperationV30 {
    action: Action,
}

impl PointerByteOperationV30 {
    pub(super) fn derive(
        inventory: &Inventory<'_>,
        operation: usize,
        width: FormalIndexWidth,
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<Self>> {
        out.budget.charge_work(2)?;
        let row = inventory
            .operations()
            .get(operation)
            .ok_or(Error::Statement("byte operation coordinate"))?;
        let uses = inventory
            .uses()
            .get(row.operands.clone())
            .ok_or(Error::Statement("byte operand range"))?;
        out.budget.charge_work(
            uses.len()
                .checked_mul(8)
                .and_then(|work| work.checked_add(24))
                .ok_or(Resource::Arithmetic)?,
        )?;
        let input = |ordinal: usize| -> Result<usize> {
            uses.get(ordinal)
                .map(|row| row.definition)
                .ok_or(Error::Statement("byte operand occurrence"))
        };
        let ty = |definition: usize| -> Result<&Type> {
            inventory
                .definitions()
                .get(definition)
                .map(|row| row.ty)
                .ok_or(Error::Statement("byte definition occurrence"))
        };
        let result = || -> Result<usize> {
            if row.results.len() != 1 {
                return Err(Error::Statement("byte result arity"));
            }
            Ok(row.results.start)
        };
        let action = match &row.operation.kind {
            OperationKind::SliceData { .. } | OperationKind::SliceLength { .. } => {
                let input = input(0)?;
                let Type::Slice(slice) = ty(input)? else {
                    return Err(Error::Statement("byte slice operand type"));
                };
                if !external(slice.address_space) {
                    return Ok(None);
                }
                if uses.len() != 1 {
                    return Err(Error::Statement("byte slice operand arity"));
                }
                let output = result()?;
                let data = matches!(row.operation.kind, OperationKind::SliceData { .. });
                if data {
                    if !matches!(ty(output)?, Type::Pointer(pointer)
                        if pointer.pointee.as_ref() == slice.element.as_ref()
                            && pointer.address_space == slice.address_space && pointer.access == slice.access)
                    {
                        return Err(Error::Statement("byte SliceData exact result type"));
                    }
                } else if ty(output)? != &Type::INDEX {
                    return Err(Error::Statement("byte SliceLength exact result type"));
                }
                Action::Slice {
                    input,
                    output,
                    index_bytes: scalar_bytes(ScalarType::Index, width)?,
                    space: pointer_space(slice.address_space)?,
                    data,
                }
            }
            OperationKind::GetElementPointer { .. } => {
                let base = input(0)?;
                let index = input(1)?;
                let output = result()?;
                let Type::Pointer(pointer) = ty(base)? else {
                    return Err(Error::Statement("byte GEP pointer type"));
                };
                if !pointer_memory(pointer.address_space) {
                    return Ok(None);
                }
                if uses.len() != 2 || ty(index)? != &Type::INDEX || ty(output)? != ty(base)? {
                    return Err(Error::Statement("byte GEP exact operand/result types"));
                }
                Action::Gep {
                    base,
                    index,
                    output,
                    bytes: scalar_bytes(scalar(pointer.pointee.as_ref())?, width)?,
                    index_bytes: scalar_bytes(ScalarType::Index, width)?,
                    space: pointer_space(pointer.address_space)?,
                }
            }
            OperationKind::Cast {
                kind: CastKind::PointerToGeneric | CastKind::RestrictPointerAccess,
                ..
            } => {
                let input = input(0)?;
                let output = result()?;
                let (Type::Pointer(from), Type::Pointer(to)) = (ty(input)?, ty(output)?) else {
                    return Err(Error::Statement("byte pointer cast exact types"));
                };
                if !pointer_memory(from.address_space) {
                    return Ok(None);
                }
                let exact = match row.operation.kind {
                    OperationKind::Cast {
                        kind: CastKind::PointerToGeneric,
                        ..
                    } => {
                        matches!(
                            from.address_space,
                            AddressSpace::Private | AddressSpace::Global
                        ) && to.address_space == AddressSpace::Generic
                            && from.access == to.access
                    }
                    OperationKind::Cast {
                        kind: CastKind::RestrictPointerAccess,
                        ..
                    } => {
                        from.address_space == to.address_space
                            && from.access == AccessMode::ReadWrite
                            && to.access == AccessMode::ReadOnly
                    }
                    _ => false,
                };
                if !exact || uses.len() != 1 || from.pointee != to.pointee {
                    return Err(Error::Statement(
                        "byte pointer cast widens permission or changes representation",
                    ));
                }
                Action::Cast {
                    input,
                    output,
                    slice_index_bytes: None,
                    index_bytes: scalar_bytes(ScalarType::Index, width)?,
                    space: pointer_space(from.address_space)?,
                }
            }
            OperationKind::Cast {
                kind: CastKind::SliceToGeneric,
                ..
            } => {
                let input = input(0)?;
                let output = result()?;
                let (Type::Slice(from), Type::Slice(to)) = (ty(input)?, ty(output)?) else {
                    return Err(Error::Statement("byte slice cast exact types"));
                };
                if !external(from.address_space) {
                    return Ok(None);
                }
                if uses.len() != 1
                    || from.address_space != AddressSpace::Global
                    || to.address_space != AddressSpace::Generic
                    || from.access != to.access
                    || from.element != to.element
                {
                    return Err(Error::Statement(
                        "byte slice cast changes payload or permission",
                    ));
                }
                Action::Cast {
                    input,
                    output,
                    slice_index_bytes: Some(scalar_bytes(ScalarType::Index, width)?),
                    index_bytes: scalar_bytes(ScalarType::Index, width)?,
                    space: pointer_space(from.address_space)?,
                }
            }
            OperationKind::Load { access, .. } | OperationKind::Store { access, .. } => {
                if !pointer_memory(access.address_space) {
                    return Ok(None);
                }
                if access.volatile {
                    return Err(Error::Statement("byte volatile operation is not modeled"));
                }
                let writing = matches!(row.operation.kind, OperationKind::Store { .. });
                let pointer = input(0)?;
                let Type::Pointer(pointer_type) = ty(pointer)? else {
                    return Err(Error::Statement("byte memory pointer type"));
                };
                let scalar = scalar(pointer_type.pointee.as_ref())?;
                if pointer_type.address_space != access.address_space
                    || writing && pointer_type.access == AccessMode::ReadOnly
                    || !writing && pointer_type.access == AccessMode::WriteOnly
                {
                    return Err(Error::Statement("byte memory permission or address space"));
                }
                let value = if writing {
                    let value = input(1)?;
                    if uses.len() != 2
                        || !row.results.is_empty()
                        || ty(value)? != pointer_type.pointee.as_ref()
                    {
                        return Err(Error::Statement("byte Store exact payload"));
                    }
                    value
                } else {
                    let output = result()?;
                    if uses.len() != 1 || ty(output)? != pointer_type.pointee.as_ref() {
                        return Err(Error::Statement("byte Load exact payload"));
                    }
                    output
                };
                Action::Memory {
                    pointer,
                    value,
                    bytes: scalar_bytes(scalar, width)?,
                    index_bytes: scalar_bytes(ScalarType::Index, width)?,
                    space: pointer_space(pointer_type.address_space)?,
                    alignment: access.alignment,
                    boolean: scalar == ScalarType::Bool,
                    writing,
                }
            }
            _ => return Ok(None),
        };
        Ok(Some(Self { action }))
    }

    pub(super) fn effect(&self) -> PointerByteEffectV30 {
        match self.action {
            Action::Memory {
                pointer,
                value,
                bytes,
                alignment,
                writing: false,
                ..
            } => PointerByteEffectV30::Read {
                pointer,
                bytes,
                alignment,
                result: value,
            },
            Action::Memory {
                pointer,
                value,
                bytes,
                alignment,
                writing: true,
                ..
            } => PointerByteEffectV30::Write {
                pointer,
                bytes,
                alignment,
                value,
            },
            _ => PointerByteEffectV30::None,
        }
    }

    pub(super) fn emit_step(
        &self,
        before: ByteMemoryStateNamesV30<'_>,
        after: ByteMemoryStateNamesV30<'_>,
        context: ByteMemoryContextNamesV30<'_>,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        out.budget.charge_work(1)?;
        let values = before.values;
        let memory = before.memory;
        let valid = before.valid;
        let endian = context.little_endian;
        match self.action {
            Action::Slice {
                input,
                output,
                index_bytes,
                space,
                data,
            } => {
                emit!(
                    out,
                    " let {} = {valid} && match {values}[{input}] {{ MemoryValueV30::Slice(s) => 0 <= s.length < memory_value_modulus_v30({index_bytes}) && byte_pointer_type_v30(s.pointer, {space}, {index_bytes}), _ => false }};\n",
                    after.valid
                );
                let payload = if data {
                    "MemoryValueV30::Pointer(s.pointer)"
                } else {
                    "MemoryValueV30::Scalar(s.length)"
                };
                emit!(
                    out,
                    " let {} = {values}.update({output}, if {} {{ match {values}[{input}] {{ MemoryValueV30::Slice(s) => {payload}, _ => MemoryValueV30::Undefined }} }} else {{ MemoryValueV30::Undefined }});\n let {} = {memory};\n",
                    after.values,
                    after.valid,
                    after.memory
                );
            }
            Action::Gep {
                base,
                index,
                output,
                bytes,
                index_bytes,
                space,
            } => {
                // Formation is checked here, never against a later access guard.
                emit!(
                    out,
                    " let {} = {valid} && match ({values}[{base}], {values}[{index}]) {{ (MemoryValueV30::Pointer(p), MemoryValueV30::Scalar(i)) => 0 <= i < memory_value_modulus_v30({index_bytes}) && byte_pointer_type_v30(p, {space}, {index_bytes}) && byte_range_live_v30({memory}, p, 0) && byte_range_live_v30({memory}, MemoryPointerV30 {{ allocation: p.allocation, byte_offset: p.byte_offset + i * {bytes}, view: p.view }}, 0) && p.byte_offset + i * {bytes} < memory_value_modulus_v30({index_bytes}), _ => false }};\n",
                    after.valid
                );
                emit!(
                    out,
                    " let {} = {values}.update({output}, if {} {{ match ({values}[{base}], {values}[{index}]) {{ (MemoryValueV30::Pointer(p), MemoryValueV30::Scalar(i)) => MemoryValueV30::Pointer(MemoryPointerV30 {{ allocation: p.allocation, byte_offset: p.byte_offset + i * {bytes}, view: p.view }}), _ => MemoryValueV30::Undefined }} }} else {{ MemoryValueV30::Undefined }});\n let {} = {memory};\n",
                    after.values,
                    after.valid,
                    after.memory
                );
            }
            Action::Cast {
                input,
                output,
                slice_index_bytes,
                index_bytes,
                space,
            } => {
                emit!(out, " let {} = {valid} && ", after.valid);
                if let Some(bytes) = slice_index_bytes {
                    emit!(
                        out,
                        "match {values}[{input}] {{ MemoryValueV30::Slice(s) => 0 <= s.length < memory_value_modulus_v30({bytes}) && byte_pointer_type_v30(s.pointer, {space}, {index_bytes}), _ => false }}"
                    );
                } else {
                    emit!(
                        out,
                        "match {values}[{input}] {{ MemoryValueV30::Pointer(p) => byte_pointer_type_v30(p, {space}, {index_bytes}), _ => false }}"
                    );
                }
                emit!(
                    out,
                    ";\n let {} = {values}.update({output}, {values}[{input}]);\n let {} = {memory};\n",
                    after.values,
                    after.memory
                );
            }
            Action::Memory {
                pointer,
                value,
                bytes,
                index_bytes,
                space,
                alignment,
                boolean,
                writing: true,
            } => {
                emit!(
                    out,
                    " let {} = {valid} && match ({values}[{pointer}], {values}[{value}]) {{ (MemoryValueV30::Pointer(p), MemoryValueV30::Scalar(v)) => byte_pointer_type_v30(p, {space}, {index_bytes}) && byte_range_aligned_v30({memory}, p, {bytes}, {alignment}) && 0 <= v < ",
                    after.valid
                );
                if boolean {
                    emit!(out, "2");
                } else {
                    emit!(out, "memory_value_modulus_v30({bytes})");
                }
                emit!(out, ", _ => false }};\n");
                emit!(
                    out,
                    " let {} = if {} {{ match ({values}[{pointer}], {values}[{value}]) {{ (MemoryValueV30::Pointer(p), MemoryValueV30::Scalar(v)) => byte_store_v30({memory}, p, {bytes}, v, {endian}), _ => {memory} }} }} else {{ {memory} }};\n let {} = {values};\n",
                    after.memory,
                    after.valid,
                    after.values
                );
            }
            Action::Memory {
                pointer,
                value: output,
                bytes,
                index_bytes,
                space,
                alignment,
                boolean,
                writing: false,
            } => {
                emit!(
                    out,
                    " let {} = {valid} && match {values}[{pointer}] {{ MemoryValueV30::Pointer(p) => byte_pointer_type_v30(p, {space}, {index_bytes}) && byte_range_aligned_v30({memory}, p, {bytes}, {alignment}) && byte_scalar_range_initialized_v37({memory}, p, {bytes})",
                    after.valid
                );
                if boolean {
                    emit!(out, " && byte_load_v30({memory}, p, {bytes}, {endian}) < 2");
                }
                emit!(
                    out,
                    ", _ => false }};\n let {} = {values}.update({output}, if {} {{ match {values}[{pointer}] {{ MemoryValueV30::Pointer(p) => MemoryValueV30::Scalar(byte_load_v30({memory}, p, {bytes}, {endian})), _ => MemoryValueV30::Undefined }} }} else {{ MemoryValueV30::Undefined }});\n let {} = {memory};\n",
                    after.values,
                    after.valid,
                    after.memory
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
    2 * size_of::<PointerByteOperationV30>()
        + 2 * size_of::<Result<Option<PointerByteOperationV30>>>()
        + 2 * size_of::<ByteMemoryStateNamesV30<'_>>()
        + size_of::<ByteMemoryContextNamesV30<'_>>()
        + size_of::<PointerByteEffectV30>()
        + size_of::<Range<usize>>()
        + size_of::<FormalIndexWidth>()
        + size_of::<(
            Option<usize>,
            [usize; 18],
            [bool; 4],
            [&(); 18],
            [Result<usize>; 5],
        )>()
}
