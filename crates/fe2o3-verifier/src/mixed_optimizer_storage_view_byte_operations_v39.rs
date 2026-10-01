//! Actual storage projections and discriminants. Cached rows come from the
//! exact canonical owner; source validity and paired observations are separate.
use super::super::byte_memory_v30::{
    ByteMemoryContextNamesV30 as Context, ByteMemoryStateNamesV30 as State,
};
use super::super::target_view_contracts_v38::TargetByteTagClassV38 as Class;
use super::{ByteInterpretationContextV39, Error, Inventory, Result, Writer};
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, MemoryAccess, OperationKind, PointerType, ScalarType,
    StorageLayoutIdV1 as Id, StorageLayoutKindV1 as Kind, StorageLayoutV1 as Layout,
    StorageOperationV1 as Storage, StorageProjectionV1 as Projection,
    StorageVariantEncodingV1 as Encoding, Type,
};
use std::{fmt::Write as _, mem::size_of};

macro_rules! emit {
    ($out:expr, $($arg:tt)*) => { write!($out, $($arg)*).map_err(|_| $out.error())? };
}

#[derive(Clone, Copy)]
pub(super) enum Action {
    Project {
        result: usize,
        relative: u64,
        extent: u64,
        index: Option<(usize, u64, u64)>,
        construction: bool,
    },
    TagRead {
        result: usize,
        payload: Option<(u32, u64)>,
        offset: u64,
        bytes: u64,
        alignment: u32,
    },
    TagWrite {
        offset: u64,
        bytes: u64,
        alignment: u32,
        bits: u128,
    },
    UntaggedNoop,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ViewGuard {
    Form,
    Construction,
    Variant,
    Discriminant,
    TagWrite,
    UntaggedNoop,
}

#[derive(Clone, Copy)]
pub(super) struct StorageViewByteOperationV39 {
    address: usize,
    space: usize,
    index_bytes: usize,
    object_bytes: u64,
    row: u32,
    action: Action,
}

fn unsupported() -> Error {
    Error::Statement("actual storage view operation is not modeled")
}

fn holder(ty: &Type) -> Result<(&PointerType, Id)> {
    let Type::Pointer(pointer) = ty else {
        return Err(unsupported());
    };
    let Type::StorageObject(id) = pointer.pointee.as_ref() else {
        return Err(unsupported());
    };
    Ok((pointer, *id))
}

fn check_access(pointer: &PointerType, access: MemoryAccess, writing: bool) -> Result<()> {
    if access.volatile
        || !access.alignment.is_power_of_two()
        || access.address_space != pointer.address_space
        || writing
            && (pointer.access == AccessMode::ReadOnly
                || pointer.address_space == AddressSpace::Constant)
        || !writing && pointer.access == AccessMode::WriteOnly
    {
        return Err(unsupported());
    }
    Ok(())
}

fn check_result(pointer: &PointerType, ty: &Type, child: Id, construction: bool) -> Result<()> {
    let (result, actual) = holder(ty)?;
    if actual != child
        || result.address_space != pointer.address_space
        || !(pointer.access == AccessMode::ReadWrite || pointer.access == result.access)
        || construction && result.access != AccessMode::WriteOnly
    {
        return Err(unsupported());
    }
    Ok(())
}

fn row(rows: &[Layout], id: Id) -> Result<&Layout> {
    rows.get(id.0 as usize).ok_or_else(unsupported)
}

impl StorageViewByteOperationV39 {
    pub(super) fn derive(
        input: &Inventory<'_>,
        operation: usize,
        interpretation: ByteInterpretationContextV39<'_, '_>,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        out.budget.charge_work(32)?;
        let actual = input.operations().get(operation).ok_or_else(unsupported)?;
        let OperationKind::Storage(storage) = actual.operation.kind else {
            return Err(unsupported());
        };
        let uses = input
            .uses()
            .get(actual.operands.clone())
            .ok_or_else(unsupported)?;
        let operand = |ordinal: usize| -> Result<(usize, &Type)> {
            let id = uses.get(ordinal).ok_or_else(unsupported)?.definition;
            Ok((id, input.definitions().get(id).ok_or_else(unsupported)?.ty))
        };
        let (address, ty) = operand(0)?;
        let (pointer, id) = holder(ty)?;
        let rows = &input.owner().module().storage_layouts;
        let layout = row(rows, id)?;
        let result = || -> Result<(usize, &Type)> {
            if actual.results.len() != 1 {
                return Err(unsupported());
            }
            Ok((
                actual.results.start,
                input
                    .definitions()
                    .get(actual.results.start)
                    .ok_or_else(unsupported)?
                    .ty,
            ))
        };
        let tag = |out: &mut Writer<'_, '_>| -> Result<(Encoding, u64, u64)> {
            if !matches!(
                interpretation.tag_class(input.owner(), id, out)?,
                Class::DirectScalar | Class::ScalarNiche | Class::PointerNullNiche
            ) {
                return Err(unsupported());
            }
            let Kind::Variants { encoding, .. } = layout.kind else {
                return Err(unsupported());
            };
            let field = encoding.tag();
            Ok((encoding, field.offset, row(rows, field.layout)?.size))
        };
        let action = match storage {
            Storage::Project { step, .. } => {
                let (result, result_type) = result()?;
                let (child, relative, index, construction, variant) = match (&layout.kind, step) {
                    (Kind::Record(fields) | Kind::Union(fields), Projection::Field(index)) => {
                        let field = fields.get(index as usize).ok_or_else(unsupported)?;
                        (field.layout, field.offset, None, false, None)
                    }
                    (Kind::Slice { data, length, .. }, Projection::Field(index)) => {
                        let field = match index {
                            0 => data,
                            1 => length,
                            _ => return Err(unsupported()),
                        };
                        (field.layout, field.offset, None, false, None)
                    }
                    (
                        Kind::Array {
                            element,
                            length,
                            stride,
                        },
                        Projection::ArrayIndex(_),
                    ) => {
                        let (index, ty) = operand(1)?;
                        if uses.len() != 2 || ty != &Type::INDEX || *length == 0 {
                            return Err(unsupported());
                        }
                        (*element, 0, Some((index, *length, *stride)), false, None)
                    }
                    (Kind::Variants { variants, .. }, Projection::Variant { index, access }) => {
                        check_access(pointer, access, false)?;
                        let entry = variants
                            .get(index as usize)
                            .filter(|v| !v.uninhabited)
                            .ok_or_else(unsupported)?;
                        let (_, offset, bytes) = tag(out)?;
                        (
                            entry.layout,
                            0,
                            None,
                            false,
                            Some((index, offset, bytes, access.alignment)),
                        )
                    }
                    (Kind::Variants { variants, .. }, Projection::VariantForWrite { index }) => {
                        if pointer.access == AccessMode::ReadOnly
                            || pointer.address_space == AddressSpace::Constant
                        {
                            return Err(unsupported());
                        }
                        let entry = variants
                            .get(index as usize)
                            .filter(|v| !v.uninhabited)
                            .ok_or_else(unsupported)?;
                        tag(out)?;
                        (entry.layout, 0, None, true, None)
                    }
                    _ => return Err(unsupported()),
                };
                if uses.len() != if index.is_some() { 2 } else { 1 } {
                    return Err(unsupported());
                }
                check_result(pointer, result_type, child, construction)?;
                let extent = row(rows, child)?.size;
                match variant {
                    Some((variant, offset, bytes, alignment)) => Action::TagRead {
                        result,
                        payload: Some((variant, extent)),
                        offset,
                        bytes,
                        alignment,
                    },
                    None => Action::Project {
                        result,
                        relative,
                        extent,
                        index,
                        construction,
                    },
                }
            }
            Storage::ReadDiscriminant { access, .. } => {
                check_access(pointer, access, false)?;
                let (result, ty) = result()?;
                if uses.len() != 1 || ty != &Type::Scalar(ScalarType::U128) {
                    return Err(unsupported());
                }
                let (_, offset, bytes) = tag(out)?;
                Action::TagRead {
                    result,
                    payload: None,
                    offset,
                    bytes,
                    alignment: access.alignment,
                }
            }
            Storage::SetDiscriminant {
                access, variant, ..
            } => {
                check_access(pointer, access, true)?;
                if uses.len() != 1 || !actual.results.is_empty() {
                    return Err(unsupported());
                }
                let (encoding, offset, bytes) = tag(out)?;
                let Kind::Variants { variants, .. } = &layout.kind else {
                    return Err(unsupported());
                };
                let entry = variants
                    .get(variant as usize)
                    .filter(|v| !v.uninhabited)
                    .ok_or_else(unsupported)?;
                let bits = match encoding {
                    Encoding::Direct { .. } => Some(entry.direct_tag_bits.ok_or_else(unsupported)?),
                    Encoding::Niche {
                        untagged_variant,
                        first_niche_variant,
                        last_niche_variant,
                        niche_start,
                        ..
                    } => {
                        if variant == untagged_variant {
                            None
                        } else if first_niche_variant <= variant && variant <= last_niche_variant {
                            let sum =
                                niche_start.wrapping_add(u128::from(variant - first_niche_variant));
                            Some(if bytes == 16 {
                                sum
                            } else {
                                sum & ((1u128 << (bytes * 8)) - 1)
                            })
                        } else {
                            return Err(unsupported());
                        }
                    }
                };
                match bits {
                    None => Action::UntaggedNoop,
                    Some(bits) => Action::TagWrite {
                        offset,
                        bytes,
                        alignment: access.alignment,
                        bits,
                    },
                }
            }
            _ => return Err(unsupported()),
        };
        Ok(Self {
            address,
            space: super::pointer_space(pointer.address_space)?,
            index_bytes: super::scalar_bytes(ScalarType::Index, interpretation.width)?,
            object_bytes: layout.size,
            row: id.0,
            action,
        })
    }

    pub(super) fn guard(&self) -> ViewGuard {
        match self.action {
            Action::Project {
                construction: false,
                ..
            } => ViewGuard::Form,
            Action::Project {
                construction: true, ..
            } => ViewGuard::Construction,
            Action::TagRead {
                payload: Some(_), ..
            } => ViewGuard::Variant,
            Action::TagRead { payload: None, .. } => ViewGuard::Discriminant,
            Action::TagWrite { .. } => ViewGuard::TagWrite,
            Action::UntaggedNoop => ViewGuard::UntaggedNoop,
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
        let (values, memory, valid, endian) = (
            before.values,
            before.memory,
            before.valid,
            context.little_endian,
        );
        let (address, space, index_bytes, object_bytes, row) = (
            self.address,
            self.space,
            self.index_bytes,
            self.object_bytes,
            self.row,
        );
        match self.action {
            Action::Project {
                result,
                relative,
                extent,
                index,
                ..
            } => {
                emit!(
                    out,
                    " let projection = match {values}[{address}] {{ MemoryValueV30::Pointer(p) => if !byte_pointer_type_v30(p, {space}, {index_bytes}) || !byte_range_live_v30({memory}, p, {object_bytes}) {{ None }} else {{ "
                );
                if let Some((index, length, stride)) = index {
                    emit!(
                        out,
                        "match {values}[{index}] {{ MemoryValueV30::Scalar(index) => if 0 <= index < {length} && index < memory_value_modulus_v30({index_bytes}) {{ byte_project_view_v38({memory}, p, index * {stride}, {extent}) }} else {{ None }}, _ => None }}"
                    );
                } else {
                    emit!(
                        out,
                        "byte_project_view_v38({memory}, p, {relative}, {extent})"
                    );
                }
                emit!(
                    out,
                    " }}, _ => None }};\n let {} = {valid} && match projection {{ Some(pointer) => byte_pointer_type_v30(pointer, {space}, {index_bytes}), None => false }};\n let {} = {values}.update({result}, if {} {{ match projection {{ Some(pointer) => MemoryValueV30::Pointer(pointer), None => MemoryValueV30::Undefined }} }} else {{ MemoryValueV30::Undefined }});\n let {} = {memory};\n",
                    after.valid,
                    after.values,
                    after.valid,
                    after.memory
                );
            }
            Action::TagRead {
                result,
                payload,
                alignment,
                ..
            } => {
                emit!(
                    out,
                    " let observed = match {values}[{address}] {{ MemoryValueV30::Pointer(p) => if byte_pointer_type_v30(p, {space}, {index_bytes}) {{ byte_read_tag_v39({memory}, p, {row}, {alignment}) }} else {{ None }}, _ => None }};\n let selected = match observed {{ Some(tag) => byte_tag_selected_variant_v39({memory}.view_contracts.rows[{row}], tag), None => None }};\n"
                );
                if let Some((variant, extent)) = payload {
                    emit!(
                        out,
                        " let projection = match {values}[{address}] {{ MemoryValueV30::Pointer(p) => if selected == Some({variant}int) {{ byte_variant_projection_v39({memory}, p, {row}, {variant}, {extent}, {alignment}) }} else {{ None }}, _ => None }};\n let {} = {valid} && match projection {{ Some(pointer) => byte_pointer_type_v30(pointer, {space}, {index_bytes}), None => false }};\n let {} = {values}.update({result}, if {} {{ match projection {{ Some(pointer) => MemoryValueV30::Pointer(pointer), None => MemoryValueV30::Undefined }} }} else {{ MemoryValueV30::Undefined }});\n",
                        after.valid,
                        after.values,
                        after.valid
                    );
                } else {
                    emit!(
                        out,
                        " let {} = {valid} && selected.is_some();\n let {} = {values}.update({result}, if {} {{ match selected {{ Some(variant) => MemoryValueV30::Scalar({memory}.view_contracts.rows[{row}].discriminants[variant]), None => MemoryValueV30::Undefined }} }} else {{ MemoryValueV30::Undefined }});\n",
                        after.valid,
                        after.values,
                        after.valid
                    );
                }
                emit!(out, " let {} = {memory};\n", after.memory);
            }
            Action::TagWrite {
                offset,
                bytes,
                alignment,
                bits,
            } => {
                emit!(
                    out,
                    " let {} = {valid} && match {values}[{address}] {{ MemoryValueV30::Pointer(p) => byte_pointer_type_v30(p, {space}, {index_bytes}) && byte_range_live_v30({memory}, p, {object_bytes}) && byte_range_aligned_v30({memory}, MemoryPointerV30 {{ byte_offset: p.byte_offset + {offset}, ..p }}, {bytes}, {alignment}), _ => false }};\n let {} = if {} {{ match {values}[{address}] {{ MemoryValueV30::Pointer(p) => byte_store_v30({memory}, MemoryPointerV30 {{ byte_offset: p.byte_offset + {offset}, ..p }}, {bytes}, {bits}, {endian}), _ => {memory} }} }} else {{ {memory} }};\n let {} = {values};\n",
                    after.valid,
                    after.memory,
                    after.valid,
                    after.values
                );
            }
            // Untagged niche selection is a physical no-op, including when its
            // unused address is stale. It cannot issue or refresh a view guard.
            Action::UntaggedNoop => emit!(
                out,
                " let {} = {valid};\n let {} = {values};\n let {} = {memory};\n",
                after.valid,
                after.values,
                after.memory
            ),
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

    pub(super) fn emit_effect(&self, values: &str, out: &mut Writer<'_, '_>) -> Result<()> {
        out.budget.charge_work(1)?;
        let address = self.address;
        match self.action {
            Action::Project { .. } | Action::UntaggedNoop => {
                emit!(out, "MemoryOperationEffectV30::Pure")
            }
            Action::TagRead {
                payload,
                offset,
                bytes,
                alignment,
                ..
            } => {
                let purpose = if payload.is_some() {
                    "VariantValidation"
                } else {
                    "DiscriminantRead"
                };
                emit!(
                    out,
                    "MemoryOperationEffectV30::TagRead {{ address: match {values}[{address}] {{ MemoryValueV30::Pointer(p) => MemoryValueV30::Pointer(MemoryPointerV30 {{ byte_offset: p.byte_offset + {offset}, ..p }}), _ => MemoryValueV30::Undefined }}, width: {bytes}, alignment: {alignment}, purpose: MemoryTagReadPurposeV39::{purpose}, observation: observed }}"
                );
            }
            Action::TagWrite {
                offset,
                bytes,
                alignment,
                bits,
            } => emit!(
                out,
                "MemoryOperationEffectV30::Write {{ address: match {values}[{address}] {{ MemoryValueV30::Pointer(p) => MemoryValueV30::Pointer(MemoryPointerV30 {{ byte_offset: p.byte_offset + {offset}, ..p }}), _ => MemoryValueV30::Undefined }}, width: {bytes}, alignment: {alignment}, value: MemoryValueV30::Scalar({bits}) }}"
            ),
        }
        Ok(())
    }
}

pub(super) fn headers() -> usize {
    size_of::<StorageViewByteOperationV39>()
        + 2 * size_of::<Result<StorageViewByteOperationV39>>()
        + size_of::<Action>()
        + size_of::<ViewGuard>()
        + size_of::<(
            [&Inventory<'_>; 2],
            &mut Writer<'_, '_>,
            [&Type; 3],
            &PointerType,
            &[Layout],
            [&Layout; 3],
            [usize; 12],
            [u64; 6],
            [u128; 2],
            Option<(usize, u64, u64)>,
            Option<(u32, u64, u64, u32)>,
            Encoding,
            Projection,
            MemoryAccess,
            [Result<()>; 3],
            State<'static>,
            State<'static>,
            Context<'static>,
            [&str; 8],
        )>()
}
