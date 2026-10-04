//! Target-free pointer carriers and exact original Borrow/descriptor events.
//! This module neither manufactures external identities nor stores provenance
//! into raw bytes. Entry/call relations supply the original tagged carriers.

use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBorrowKindV1 as Borrow, SemanticMutabilityV1 as Mutability,
    SemanticPointerTypeV1 as PointerType,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) enum Event {
    Copy {
        destination: usize,
        operand: TypedOperand,
        metadata_bits: u32,
    },
    Borrow {
        destination: usize,
        access: Access,
        bits: u32,
    },
    TypedBorrow {
        destination: usize,
        access: Access,
        source_type: TypeId,
        root_type: TypeId,
    },
    SliceBorrow {
        destination: usize,
        local: usize,
        metadata_bits: u32,
        bytes: u64,
        alignment: u64,
        bits: u32,
    },
    IndexBorrow {
        destination: usize,
        local: usize,
        index: usize,
        index_bits: u32,
        metadata_bits: u32,
        bytes: u64,
        alignment: u64,
        bits: u32,
    },
    Length {
        destination: usize,
        local: usize,
        metadata_bits: u32,
        moved: bool,
    },
}

fn pointer<'a>(
    context: &'a Context<'_, '_, '_>,
    ty: TypeId,
    out: &mut Writer<'_, '_>,
) -> Result<&'a PointerType> {
    out.budget.charge_work(4)?;
    let Some(Shape::Pointer(pointer)) = context.types.get(ty.index() as usize).map(Type::shape)
    else {
        return Err(mismatch());
    };
    // The archived source ABI is 64-bit generic. Canonical address-space casts
    // preserve the allocation tag in the independent physical interpreter.
    if pointer.address_space() != 0
        || pointer.pointer_width_bits() != 64
        || context
            .slots
            .witness_class(pointer.pointee(), out)?
            .is_some()
    {
        return Err(unsupported());
    }
    Ok(pointer)
}

fn local_destination(
    context: &Context<'_, '_, '_>,
    place: &Place,
    out: &mut Writer<'_, '_>,
) -> Result<usize> {
    match context.destination(place, out)? {
        Destination::Local(local) => Ok(local),
        Destination::Memory(_) | Destination::Component(_) => Err(unsupported()),
    }
}

fn slice_base(
    context: &Context<'_, '_, '_>,
    place: &Place,
    mutable: bool,
    out: &mut Writer<'_, '_>,
) -> Result<(usize, TypeId, u32)> {
    out.budget.charge_work(5)?;
    let declaration = context
        .function
        .locals()
        .get(place.local().index() as usize)
        .ok_or_else(mismatch)?;
    let pointer = pointer(context, declaration.ty(), out)?;
    let Some(Shape::Slice { element }) = context
        .types
        .get(pointer.pointee().index() as usize)
        .map(Type::shape)
    else {
        return Err(unsupported());
    };
    if pointer.kind() != PointerKind::Reference
        || pointer.metadata() != PointerMetadata::SliceLength
        || mutable && pointer.mutability() != Mutability::Mutable
        || context.descriptor(place.local().index(), out)?.is_some()
        || !place.projections().first().is_some_and(|projection| {
            projection.kind() == Projection::Dereference
                && projection.result_type() == pointer.pointee()
        })
    {
        return Err(unsupported());
    }
    let metadata_bits =
        slice_metadata_bits_v36(&context.types[declaration.ty().index() as usize], out)?;
    Ok((
        context.local(place.local().index())?,
        *element,
        metadata_bits,
    ))
}

fn scalar_layout(
    context: &Context<'_, '_, '_>,
    ty: TypeId,
    out: &mut Writer<'_, '_>,
) -> Result<(u64, u64, u32)> {
    let scalar = context.scalar(ty, out)?;
    if scalar == ScalarV30::Unit {
        return Err(unsupported());
    }
    out.budget.charge_work(2)?;
    let layout = context
        .types
        .get(ty.index() as usize)
        .ok_or_else(mismatch)?
        .layout();
    Ok((
        layout.size_bytes().ok_or_else(unsupported)?,
        layout.alignment_bytes(),
        scalar.width(),
    ))
}

pub(super) fn derive(
    context: &Context<'_, '_, '_>,
    statement: &Statement,
    out: &mut Writer<'_, '_>,
) -> Result<Option<Event>> {
    out.budget.charge_work(2)?;
    let Statement::Assign(assignment) = statement else {
        return Ok(None);
    };
    let ty = assignment.destination().ty();
    if ty != assignment.value().result_type() {
        return Err(mismatch());
    }
    match assignment.value().kind() {
        Rvalue::Use(operand) => {
            let metadata_bits = if matches!(
                context.types.get(ty.index() as usize).map(Type::shape),
                Some(Shape::Pointer(_))
            ) {
                let target = pointer(context, ty, out)?;
                match target.metadata() {
                    PointerMetadata::None => 0,
                    PointerMetadata::SliceLength => {
                        slice_metadata_bits_v36(&context.types[ty.index() as usize], out)?
                    }
                    _ => return Err(unsupported()),
                }
            } else if let Some(bits) = context.slots.descriptor_slice_bits(ty, out)? {
                bits
            } else {
                return Ok(None);
            };
            if operand.ty() != ty {
                return Err(mismatch());
            }
            Ok(Some(Event::Copy {
                destination: local_destination(context, assignment.destination(), out)?,
                operand: context.typed_operand(operand, out)?,
                metadata_bits,
            }))
        }
        Rvalue::Length(place)
            if matches!(
                context
                    .types
                    .get(place.ty().index() as usize)
                    .map(Type::shape),
                Some(Shape::Slice { .. })
            ) =>
        {
            let (local, _, metadata_bits) = slice_base(context, place, false, out)?;
            if place.projections().len() != 1
                || context.scalar(ty, out)?
                    != (ScalarV30::Integer {
                        signed: false,
                        width: 64,
                    })
                || metadata_bits != 64
            {
                return Err(unsupported());
            }
            Ok(Some(Event::Length {
                destination: local_destination(context, assignment.destination(), out)?,
                local,
                metadata_bits,
                moved: false,
            }))
        }
        Rvalue::Unary {
            operation: fe2o3_mir_model::semantic_mir_v1::SemanticUnaryOpV1::PointerMetadata,
            operand,
        } => {
            let original = pointer(context, operand.ty(), out)?;
            if original.kind() != PointerKind::Reference
                || original.metadata() != PointerMetadata::SliceLength
                || !matches!(
                    context
                        .types
                        .get(original.pointee().index() as usize)
                        .map(Type::shape),
                    Some(Shape::Slice { .. })
                )
                || context.scalar(ty, out)?
                    != (ScalarV30::Integer {
                        signed: false,
                        width: 64,
                    })
            {
                return Err(unsupported());
            }
            let TypedOperand {
                kind:
                    OperandKind::Slice {
                        local,
                        moved,
                        metadata_bits,
                    },
                ..
            } = context.typed_operand(operand, out)?
            else {
                return Err(unsupported());
            };
            if metadata_bits != 64 {
                return Err(unsupported());
            }
            Ok(Some(Event::Length {
                destination: local_destination(context, assignment.destination(), out)?,
                local,
                metadata_bits,
                moved,
            }))
        }
        Rvalue::Borrow { kind, place } => {
            let target = pointer(context, ty, out)?;
            let mutable = match (kind, target.mutability()) {
                (Borrow::Shared, Mutability::Immutable) => false,
                (Borrow::Mutable, Mutability::Mutable) => true,
                _ => return Err(unsupported()),
            };
            if target.kind() != PointerKind::Reference || target.pointee() != place.ty() {
                return Err(mismatch());
            }
            let destination = local_destination(context, assignment.destination(), out)?;
            if target.metadata() == PointerMetadata::SliceLength {
                let (local, element, metadata_bits) = slice_base(context, place, mutable, out)?;
                if place.projections().len() != 1
                    || context.function.locals()[place.local().index() as usize].ty() != ty
                {
                    return Err(unsupported());
                }
                let (bytes, alignment, bits) = scalar_layout(context, element, out)?;
                return Ok(Some(Event::SliceBorrow {
                    destination,
                    local,
                    metadata_bits,
                    bytes,
                    alignment,
                    bits,
                }));
            }
            if target.metadata() != PointerMetadata::None {
                return Err(unsupported());
            }
            if matches!(
                context
                    .types
                    .get(place.ty().index() as usize)
                    .map(Type::shape),
                Some(
                    Shape::Tuple(_) | Shape::Aggregate(_) | Shape::Array { .. } | Shape::Pointer(_)
                )
            ) {
                if place
                    .projections()
                    .first()
                    .is_some_and(|p| p.kind() == Projection::Dereference)
                {
                    let base_type = context
                        .function
                        .locals()
                        .get(place.local().index() as usize)
                        .ok_or_else(mismatch)?
                        .ty();
                    if mutable
                        && pointer(context, base_type, out)?.mutability() != Mutability::Mutable
                    {
                        return Err(unsupported());
                    }
                }
                let Some((bytes, alignment, _)) =
                    context.slots.original_memory_layout_v51(place.ty(), out)?
                else {
                    return Err(unsupported());
                };
                let access = context.access(place, out)?;
                if access.ty != place.ty() || access.bytes != bytes || access.alignment != alignment
                {
                    return Err(mismatch());
                }
                return Ok(Some(Event::TypedBorrow {
                    destination,
                    access,
                    source_type: place.ty(),
                    root_type: context
                        .function
                        .locals()
                        .get(place.local().index() as usize)
                        .ok_or_else(mismatch)?
                        .ty(),
                }));
            }
            let (bytes, alignment, bits) = scalar_layout(context, place.ty(), out)?;
            if let [first, second] = place.projections()
                && first.kind() == Projection::Dereference
                && let Projection::Index(index) = second.kind()
            {
                let (local, element, metadata_bits) = slice_base(context, place, mutable, out)?;
                let index_type = context
                    .function
                    .locals()
                    .get(index.index() as usize)
                    .ok_or_else(mismatch)?
                    .ty();
                let ScalarV30::Integer {
                    signed: false,
                    width: index_bits,
                } = context.scalar(index_type, out)?
                else {
                    return Err(unsupported());
                };
                if element != place.ty()
                    || second.result_type() != element
                    || context.descriptor(index.index(), out)?.is_some()
                {
                    return Err(unsupported());
                }
                return Ok(Some(Event::IndexBorrow {
                    destination,
                    local,
                    index: context.local(index.index())?,
                    index_bits,
                    metadata_bits,
                    bytes,
                    alignment,
                    bits,
                }));
            }
            if place
                .projections()
                .first()
                .is_some_and(|projection| projection.kind() == Projection::Dereference)
            {
                let base_type = context
                    .function
                    .locals()
                    .get(place.local().index() as usize)
                    .ok_or_else(mismatch)?
                    .ty();
                if mutable && pointer(context, base_type, out)?.mutability() != Mutability::Mutable
                {
                    return Err(unsupported());
                }
            }
            Ok(Some(Event::Borrow {
                destination,
                access: context.access(place, out)?,
                bits,
            }))
        }
        _ => Ok(None),
    }
}

pub(super) fn emit(event: Event, out: &mut Writer<'_, '_>) -> Result<()> {
    out.budget.charge_work(1)?;
    match event {
        Event::Copy { destination, operand, metadata_bits } => {
            write!(out, "InvocationSourcePointerEventV36::Copy {{ destination: {destination}int, operand: ").map_err(|_| out.error())?;
            operand.emit(out)?;
            write!(out, ", metadata_bits: {metadata_bits}int }}").map_err(|_| out.error())
        }
        Event::Borrow { destination, access, bits } => {
            write!(out, "InvocationSourcePointerEventV36::Borrow {{ destination: {destination}int, access: ").map_err(|_| out.error())?;
            emit_access(access, out)?;
            write!(out, ", bits: {bits}int }}").map_err(|_| out.error())
        }
        Event::TypedBorrow { destination, access, source_type, root_type } => {
            write!(out, "InvocationSourcePointerEventV36::TypedBorrow {{ destination: {destination}int, access: ").map_err(|_| out.error())?;
            emit_access(access, out)?;
            write!(out, ", source_type: {}int, root_type: {}int }}", source_type.index(), root_type.index()).map_err(|_| out.error())
        }
        Event::SliceBorrow { destination, local, metadata_bits, bytes, alignment, bits } => write!(out,
            "InvocationSourcePointerEventV36::SliceBorrow {{ destination: {destination}int, local: {local}int, metadata_bits: {metadata_bits}int, width: {bytes}int, alignment: {alignment}int, bits: {bits}int }}").map_err(|_| out.error()),
        Event::IndexBorrow { destination, local, index, index_bits, metadata_bits, bytes, alignment, bits } => write!(out,
            "InvocationSourcePointerEventV36::IndexBorrow {{ destination: {destination}int, local: {local}int, index: {index}int, index_bits: {index_bits}int, metadata_bits: {metadata_bits}int, width: {bytes}int, alignment: {alignment}int, bits: {bits}int }}").map_err(|_| out.error()),
        Event::Length { destination, local, metadata_bits, moved } => write!(out,
            "InvocationSourcePointerEventV36::Length {{ destination: {destination}int, local: {local}int, metadata_bits: {metadata_bits}int, moved: {moved} }}").map_err(|_| out.error()),
    }
}

pub(super) fn headers() -> usize {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    h::<Event>()
        + h::<Option<Event>>()
        + h::<&PointerType>()
        + h::<(usize, TypeId, u32)>()
        + h::<(u64, u64, u32)>()
        + h::<TypedOperand>()
        + h::<Option<(u64, u64, bool)>>()
        + 18 * size_of::<usize>()
        + 10 * size_of::<&()>()
}

pub(in super::super) const SOURCE_POINTERS_V36: &str = r#"
enum InvocationSourcePointerEventV36 {
    Copy { destination: int, operand: InvocationSourceOperandV36, metadata_bits: int },
    Borrow { destination: int, access: InvocationSourceByteAccessV36, bits: int },
    TypedBorrow { destination: int, access: InvocationSourceByteAccessV36, source_type: int, root_type: int },
    SliceBorrow { destination: int, local: int, metadata_bits: int, width: int, alignment: int, bits: int },
    IndexBorrow { destination: int, local: int, index: int, index_bits: int, metadata_bits: int, width: int, alignment: int, bits: int },
    Length { destination: int, local: int, metadata_bits: int, moved: bool },
}

spec fn invocation_source_pointer_carrier_v36(value: MemoryValueV30, metadata_bits: int) -> bool {
    match value {
        MemoryValueV30::Pointer(pointer) => metadata_bits == 0 && byte_pointer_type_v30(pointer, 2, 8),
        MemoryValueV30::Slice(slice) =>
            (metadata_bits == 8 || metadata_bits == 16 || metadata_bits == 32 || metadata_bits == 64)
            && 0 <= slice.length < memory_value_modulus_v30(metadata_bits / 8)
            && byte_pointer_type_v30(slice.pointer, 2, 8),
        _ => false,
    }
}

spec fn invocation_source_borrow_enabled_v36(
    source: InvocationSourceByteStateV36, pointer: MemoryPointerV30,
    width: int, alignment: int, bits: int, little_endian: bool,
) -> bool {
    source.machine.valid && invocation_source_byte_state_well_formed_v36(source)
        && byte_pointer_type_v30(pointer, 2, 8)
        && (bits == 1 && width == 1 || bits == width * 8)
        && invocation_source_read_enabled_v36(source.machine, pointer, width, alignment)
        && invocation_source_byte_value_typed_v36(MemoryValueV30::Scalar(
            byte_load_v30(source.machine.memory, pointer, width, little_endian)), bits)
}

spec fn invocation_source_pointer_step_v36(
    source: InvocationSourceByteStateV36, event: InvocationSourcePointerEventV36,
    root: int, instance: int, little_endian: bool,
) -> InvocationSourceByteStateV36 {
    if !source.machine.valid || !invocation_source_byte_state_well_formed_v36(source) {
        invocation_source_byte_refused_v36(source)
    } else { match event {
        InvocationSourcePointerEventV36::Copy { destination, operand, metadata_bits } => {
            let evaluated = invocation_source_operand_evaluate_v36(source, operand, root, instance, little_endian);
            if evaluated.source.machine.valid && invocation_source_pointer_carrier_v36(evaluated.value, metadata_bits) {
                invocation_source_byte_put_local_v36(evaluated.source, destination, evaluated.value)
            } else { invocation_source_byte_refused_v36(evaluated.source) }
        }
        InvocationSourcePointerEventV36::Borrow { destination, access, bits } => {
            match invocation_source_byte_address_v36(source, access, root, instance) {
                Some(pointer) => if invocation_source_borrow_enabled_v36(source, pointer, access.width, access.alignment, bits, little_endian) {
                    invocation_source_byte_put_local_v36(source, destination, MemoryValueV30::Pointer(pointer))
                } else { invocation_source_byte_refused_v36(source) },
                None => invocation_source_byte_refused_v36(source),
            }
        }
        InvocationSourcePointerEventV36::TypedBorrow { destination, access, source_type, root_type } =>
            invocation_source_typed_borrow_v51(source, destination, access, source_type, root_type,
                root, instance, little_endian),
        InvocationSourcePointerEventV36::Length { destination, local, metadata_bits, moved } => {
            // Metadata comes from the current original carrier. The scalar
            // normalizer's descriptor name is never a runtime input or premise.
            let evaluated = invocation_source_carrier_evaluate_v36(source, local, moved, metadata_bits);
            if evaluated.source.machine.valid
                && invocation_source_pointer_carrier_v36(evaluated.value, metadata_bits) {
                match evaluated.value {
                    MemoryValueV30::Slice(slice) => invocation_source_byte_put_local_v36(evaluated.source, destination, MemoryValueV30::Scalar(slice.length)),
                    _ => invocation_source_byte_refused_v36(evaluated.source),
                }
            } else { invocation_source_byte_refused_v36(evaluated.source) }
        }
        InvocationSourcePointerEventV36::SliceBorrow { destination, local, metadata_bits, width, alignment, bits } => {
            if 0 <= local < source.machine.values.len() && width > 0
                && invocation_source_pointer_carrier_v36(source.machine.values[local], metadata_bits) {
                match source.machine.values[local] {
                    MemoryValueV30::Slice(slice) => {
                        if byte_range_aligned_v30(source.machine.memory, slice.pointer, slice.length * width, alignment)
                            && slice.pointer.byte_offset + slice.length * width < memory_value_modulus_v30(8)
                            && forall|index: int| 0 <= index < slice.length ==>
                                #[trigger] invocation_source_borrow_enabled_v36(source,
                                    MemoryPointerV30 { byte_offset: slice.pointer.byte_offset + index * width, ..slice.pointer },
                                    width, alignment, bits, little_endian) {
                            invocation_source_byte_put_local_v36(source, destination, MemoryValueV30::Slice(slice))
                        } else { invocation_source_byte_refused_v36(source) }
                    }
                    _ => invocation_source_byte_refused_v36(source),
                }
            } else { invocation_source_byte_refused_v36(source) }
        }
        InvocationSourcePointerEventV36::IndexBorrow { destination, local, index, index_bits, metadata_bits, width, alignment, bits } => {
            if 0 <= local < source.machine.values.len() && 0 <= index < source.machine.values.len()
                && width > 0 && invocation_source_pointer_carrier_v36(source.machine.values[local], metadata_bits)
                && invocation_source_byte_value_typed_v36(source.machine.values[index], index_bits) {
                match (source.machine.values[local], source.machine.values[index]) {
                    (MemoryValueV30::Slice(slice), MemoryValueV30::Scalar(index)) => {
                        let pointer = MemoryPointerV30 {
                            byte_offset: slice.pointer.byte_offset + index * width, ..slice.pointer };
                        // Formation checks occur here, not at a future load or guard.
                        if 0 <= index < slice.length
                            && byte_range_live_v30(source.machine.memory, slice.pointer, slice.length * width)
                            && slice.pointer.byte_offset + slice.length * width < memory_value_modulus_v30(8)
                            && invocation_source_borrow_enabled_v36(source, pointer, width, alignment, bits, little_endian) {
                            invocation_source_byte_put_local_v36(source, destination, MemoryValueV30::Pointer(pointer))
                        } else { invocation_source_byte_refused_v36(source) }
                    }
                    _ => invocation_source_byte_refused_v36(source),
                }
            } else { invocation_source_byte_refused_v36(source) }
        }
    } }
}

proof fn invocation_source_slice_borrow_invalid_element_refuses_v74(
    source: InvocationSourceByteStateV36, slice: MemorySliceV30,
    destination: int, local: int, metadata_bits: int, width: int, alignment: int, bits: int,
    root: int, instance: int, index: int, little_endian: bool,
)
    requires 0 <= local < source.machine.values.len(),
        source.machine.values[local] == MemoryValueV30::Slice(slice),
        0 <= index < slice.length,
        !invocation_source_borrow_enabled_v36(source,
            MemoryPointerV30 { byte_offset: slice.pointer.byte_offset + index * width, ..slice.pointer },
            width, alignment, bits, little_endian),
    ensures !invocation_source_pointer_step_v36(source,
        InvocationSourcePointerEventV36::SliceBorrow {
            destination, local, metadata_bits, width, alignment, bits },
        root, instance, little_endian).machine.valid,
{}

proof fn invocation_source_slice_borrow_valid_elements_preserve_carrier_v74(
    source: InvocationSourceByteStateV36, slice: MemorySliceV30,
    destination: int, local: int, metadata_bits: int, width: int, alignment: int, bits: int,
    root: int, instance: int, little_endian: bool,
)
    requires source.machine.valid, invocation_source_byte_state_well_formed_v36(source),
        0 <= destination < source.machine.values.len(),
        0 <= local < source.machine.values.len(), width > 0,
        source.machine.values[local] == MemoryValueV30::Slice(slice),
        invocation_source_pointer_carrier_v36(MemoryValueV30::Slice(slice), metadata_bits),
        byte_range_aligned_v30(source.machine.memory, slice.pointer, slice.length * width, alignment),
        slice.pointer.byte_offset + slice.length * width < memory_value_modulus_v30(8),
        forall|index: int| 0 <= index < slice.length ==>
            #[trigger] invocation_source_borrow_enabled_v36(source,
                MemoryPointerV30 { byte_offset: slice.pointer.byte_offset + index * width, ..slice.pointer },
                width, alignment, bits, little_endian),
    ensures invocation_source_pointer_step_v36(source,
        InvocationSourcePointerEventV36::SliceBorrow {
            destination, local, metadata_bits, width, alignment, bits },
        root, instance, little_endian)
        == invocation_source_byte_put_local_v36(source, destination, MemoryValueV30::Slice(slice)),
{}
"#;

#[cfg(test)]
#[path = "original_semantic_mir_invocation_source_pointers_v36_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "original_semantic_mir_source_metadata_v40_tests.rs"]
mod metadata_tests;
