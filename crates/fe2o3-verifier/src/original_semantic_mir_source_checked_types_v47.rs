//! Original checked-result storage geometry, independent of promoted leaves.
use super::super::super::{ScalarV30, Shape, Type, TypeId};
use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) struct CheckedObjectTypeV47 {
    pub(in super::super) scalar: ScalarV30,
    pub(in super::super) bytes: u64,
    pub(in super::super) alignment: u64,
    pub(in super::super) offsets: [u64; 2],
}

pub(super) fn classify(
    types: &[Type],
    ty: TypeId,
    out: &mut Writer<'_, '_>,
) -> Result<Option<CheckedObjectTypeV47>> {
    out.budget.charge_work(12)?;
    let declaration = types.get(ty.index() as usize).ok_or_else(mismatch)?;
    let Shape::Tuple(tuple) = declaration.shape() else {
        return Ok(None);
    };
    let [integer, boolean] = tuple.fields() else {
        return Ok(None);
    };
    let Ok(scalar @ ScalarV30::Integer { width, .. }) = ScalarV30::from_source(types, *integer)
    else {
        return Ok(None);
    };
    if !matches!(ScalarV30::from_source(types, *boolean), Ok(ScalarV30::Bool)) {
        return Ok(None);
    }
    let value_type = types.get(integer.index() as usize).ok_or_else(mismatch)?;
    let overflow_type = types.get(boolean.index() as usize).ok_or_else(mismatch)?;
    let layout = declaration.layout();
    let Some(bytes) = layout.size_bytes() else {
        return Ok(None);
    };
    let Some([value, overflow]) = layout.fields().source_order_offsets_bytes() else {
        return Ok(None);
    };
    let width = u64::from(width / 8);
    let value_end = value.checked_add(width).ok_or(Resource::Arithmetic)?;
    let overflow_end = overflow.checked_add(1).ok_or(Resource::Arithmetic)?;
    if layout.is_uninhabited()
        || value_type.layout().is_uninhabited()
        || overflow_type.layout().is_uninhabited()
        || value_type.layout().size_bytes() != Some(width)
        || overflow_type.layout().size_bytes() != Some(1)
        || value_end > bytes
        || overflow_end > bytes
        || !(value_end <= *overflow || overflow_end <= *value)
    {
        return Ok(None);
    }
    Ok(Some(CheckedObjectTypeV47 {
        scalar,
        bytes,
        alignment: layout.alignment_bytes(),
        offsets: [*value, *overflow],
    }))
}

impl SourceSlots<'_, '_> {
    pub(in super::super) fn checked_object_type_v47(
        &self,
        ty: TypeId,
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<CheckedObjectTypeV47>> {
        self.with_source_query_v42(out, |out| {
            let source = self.relation.source(out.budget)?;
            classify(source.source_semantic(out.budget)?.types(), ty, out)
        })
    }

    pub(super) fn emit_checked_object_types_v47(&self, out: &mut Writer<'_, '_>) -> Result<()> {
        self.with_source_query_v42(out, |out| {
            let source = self.relation.source(out.budget)?;
            let original = source.source_semantic(out.budget)?;
            write!(out, "open spec fn invocation_source_checked_object_type_v47(ty: int, bits: int, signed: bool, bytes: int, alignment: int, value_offset: int, overflow_offset: int) -> bool {{ false").map_err(|_| out.error())?;
            for (index, _) in original.types().iter().enumerate() {
                out.budget.charge_work(1)?;
                let ty = TypeId::from_index(u32::try_from(index).map_err(|_| Resource::Arithmetic)?);
                let Some(row) = classify(original.types(), ty, out)? else { continue };
                let ScalarV30::Integer { width, signed } = row.scalar else { return Err(mismatch()) };
                write!(out, "\n    || (ty == {index}int && bits == {width}int && signed == {signed} && bytes == {}int && alignment == {}int && value_offset == {}int && overflow_offset == {}int)", row.bytes, row.alignment, row.offsets[0], row.offsets[1]).map_err(|_| out.error())?;
            }
            write!(out, "\n}}\n").map_err(|_| out.error())
        })
    }
}

pub(super) fn headers() -> usize {
    size_of::<CheckedObjectTypeV47>()
        + 2 * size_of::<Result<Option<CheckedObjectTypeV47>>>()
        + size_of::<Option<CheckedObjectTypeV47>>()
        + size_of::<std::iter::Enumerate<std::slice::Iter<'_, Type>>>()
        + 4 * size_of::<Result<ScalarV30>>()
        + 12 * size_of::<u64>()
        + 10 * size_of::<&()>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_checked_object_type_headers_cover_the_independent_geometry_frame() {
        type Fields = (ScalarV30, u64, u64, [u64; 2]);
        assert_eq!(size_of::<CheckedObjectTypeV47>(), size_of::<Fields>());
        assert_eq!(
            headers(),
            size_of::<Fields>()
                + 2 * size_of::<Result<Option<Fields>>>()
                + size_of::<Option<Fields>>()
                + size_of::<std::iter::Enumerate<std::slice::Iter<'_, Type>>>()
                + 4 * size_of::<Result<ScalarV30>>()
                + 12 * size_of::<u64>()
                + 10 * size_of::<&()>()
        );
    }
}
