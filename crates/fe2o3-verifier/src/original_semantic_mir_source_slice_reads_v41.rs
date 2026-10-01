//! Original slice projection recipes for scalar reads, not pointer formation.

use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Slice {
    pub(super) local: usize,
    pub(super) index: usize,
    pub(super) index_bits: u32,
    pub(super) metadata_bits: u32,
    pub(super) stride: u64,
    pub(super) alignment: u64,
}

pub(super) fn derive(
    context: &Context<'_, '_, '_>,
    place: &Place,
    out: &mut Writer<'_, '_>,
) -> Result<Option<(Slice, TypeId)>> {
    out.budget.charge_work(5)?;
    let declaration = context
        .function
        .locals()
        .get(place.local().index() as usize)
        .ok_or_else(mismatch)?;
    let original = context
        .types
        .get(declaration.ty().index() as usize)
        .ok_or_else(mismatch)?;
    let Shape::Pointer(pointer) = original.shape() else {
        return Ok(None);
    };
    if pointer.metadata() != PointerMetadata::SliceLength {
        return Ok(None);
    }
    if pointer.kind() != PointerKind::Reference
        || pointer.address_space() != 0
        || pointer.pointer_width_bits() != 64
        || context.descriptor(place.local().index(), out)?.is_some()
        || context.slots.has_original_object(
            context.root,
            context.instance,
            place.local().index(),
            out,
        )?
    {
        return Err(unsupported());
    }
    let [dereference, index, ..] = place.projections() else {
        return Err(unsupported());
    };
    let parent = context
        .types
        .get(pointer.pointee().index() as usize)
        .ok_or_else(mismatch)?;
    let Shape::Slice { element } = parent.shape() else {
        return Err(mismatch());
    };
    if dereference.kind() != Projection::Dereference
        || dereference.result_type() != pointer.pointee()
        || index.result_type() != *element
    {
        return Err(mismatch());
    }
    out.budget.charge_work(4)?;
    let child = context
        .types
        .get(element.index() as usize)
        .ok_or_else(mismatch)?;
    let Fields::Array {
        stride_bytes,
        count: 0,
    } = parent.layout().fields()
    else {
        return Err(unsupported());
    };
    let stride = *stride_bytes;
    let alignment = child.layout().alignment_bytes();
    if child.layout().size_bytes() != Some(stride)
        || parent.layout().size_bytes().is_some()
        || parent.layout().alignment_bytes() != alignment
    {
        return Err(mismatch());
    }
    let (index, index_bits) = match index.kind() {
        Projection::Index(local) => {
            out.budget.charge_work(3)?;
            let ty = context
                .function
                .locals()
                .get(local.index() as usize)
                .ok_or_else(mismatch)?
                .ty();
            let ScalarV30::Integer {
                signed: false,
                width: bits,
            } = context.scalar(ty, out)?
            else {
                return Err(unsupported());
            };
            if context.descriptor(local.index(), out)?.is_some()
                || context.slots.has_original_object(
                    context.root,
                    context.instance,
                    local.index(),
                    out,
                )?
            {
                return Err(unsupported());
            }
            (context.local(local.index())?, bits)
        }
        _ => return Err(unsupported()),
    };
    Ok(Some((
        Slice {
            local: context.local(place.local().index())?,
            index,
            index_bits,
            metadata_bits: slice_metadata_bits_v36(original, out)?,
            stride,
            alignment,
        },
        *element,
    )))
}

pub(super) fn emit(source: Slice, out: &mut Writer<'_, '_>) -> Result<()> {
    out.budget.charge_work(1)?;
    write!(out, "InvocationSourceByteBaseV36::SliceElement(InvocationSourceSliceReadV41 {{ local: {}int, index: {}int, index_bits: {}int, metadata_bits: {}int, stride: {}int, alignment: {}int }})",
        source.local, source.index, source.index_bits, source.metadata_bits, source.stride, source.alignment).map_err(|_| out.error())
}

pub(super) fn headers() -> usize {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    h::<Slice>()
        + h::<(usize, u32)>()
        + h::<Option<(Slice, TypeId)>>()
        + 12 * size_of::<usize>()
        + 8 * size_of::<&()>()
}
