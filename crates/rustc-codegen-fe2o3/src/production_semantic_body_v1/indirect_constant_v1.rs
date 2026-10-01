//! Typed projection of an indirect value; uninitialized live bytes never become data.

use rustc_abi::{
    BackendRepr, Endian, FieldsShape, Primitive, Scalar, Size, TagEncoding, VariantIdx, Variants,
    WrappingRange,
};
use rustc_middle::ty::layout::{HasTyCtxt, LayoutCx, LayoutOf, TyAndLayout};
use rustc_middle::ty::{TyCtxt, TyKind, TypingEnv};

use super::{ProductionSemanticBodyErrorV1, SemanticMirResourceV1, try_vec_v1, unsupported};
use crate::semantic_layout_bridge::MAX_SEMANTIC_LAYOUT_DEPTH_V1;

type Result<T> = std::result::Result<T, ProductionSemanticBodyErrorV1>;

pub(super) fn canonicalize<'tcx>(
    tcx: TyCtxt<'tcx>,
    layout: TyAndLayout<'tcx>,
    raw: &[u8],
    initialized: impl Fn(usize) -> bool,
    charge: impl FnMut(usize) -> Result<()>,
    block: Option<u32>,
    statement: Option<u32>,
) -> Result<Vec<u8>> {
    let mut bytes = Bytes {
        raw,
        initialized,
        charge,
        endian: tcx.data_layout.endian,
        block,
        statement,
    };
    let cx = LayoutCx::new(tcx, TypingEnv::fully_monomorphized());
    if layout.size.bytes_usize() != raw.len() {
        return Err(bytes.error("indirect constant size differs from its typed layout"));
    }
    visit(&cx, layout, 0, 0, &mut bytes)?;
    bytes.finish()
}

struct Bytes<'a, I, C> {
    raw: &'a [u8],
    initialized: I,
    charge: C,
    endian: Endian,
    block: Option<u32>,
    statement: Option<u32>,
}

impl<I: Fn(usize) -> bool, C: FnMut(usize) -> Result<()>> Bytes<'_, I, C> {
    fn error(&self, reason: &'static str) -> ProductionSemanticBodyErrorV1 {
        unsupported(reason, self.block, self.statement)
    }

    fn node(&mut self, depth: usize) -> Result<()> {
        (self.charge)(1)?;
        if depth > MAX_SEMANTIC_LAYOUT_DEPTH_V1 {
            return Err(self.error("indirect constant exceeds the typed layout depth bound"));
        }
        Ok(())
    }

    fn range(&self, offset: usize, length: usize) -> Result<std::ops::Range<usize>> {
        let end = offset
            .checked_add(length)
            .filter(|end| *end <= self.raw.len())
            .ok_or_else(|| {
                self.error("indirect constant field lies outside its allocation view")
            })?;
        Ok(offset..end)
    }

    fn integer(&mut self, offset: usize, size: Size) -> Result<u128> {
        let length = size.bytes_usize();
        if !(1..=16).contains(&length) {
            return Err(self.error("indirect constant scalar has an unsupported width"));
        }
        let range = self.range(offset, length)?;
        (self.charge)(length)?;
        let mut bits = 0_u128;
        for (ordinal, index) in range.enumerate() {
            if !(self.initialized)(index) {
                return Err(self.error("indirect constant has an uninitialized live field or tag"));
            }
            let shift = match self.endian {
                Endian::Little => ordinal,
                Endian::Big => length - ordinal - 1,
            } * 8;
            bits |= u128::from(self.raw[index]) << shift;
        }
        Ok(bits)
    }

    fn scalar(&mut self, cx: &LayoutCx<'_>, scalar: Scalar, offset: usize) -> Result<u128> {
        let (value, valid_range) = self.scalar_representation(scalar)?;
        self.valid_scalar(offset, value.size(cx), valid_range)
    }

    fn scalar_representation(&self, scalar: Scalar) -> Result<(Primitive, WrappingRange)> {
        let Scalar::Initialized { value, valid_range } = scalar else {
            return Err(self.error("indirect constant live scalar permits unknown initialization"));
        };
        if matches!(value, Primitive::Pointer(_)) {
            return Err(self.error("indirect constant has an unsupported pointer representation"));
        }
        Ok((value, valid_range))
    }

    fn valid_scalar(
        &mut self,
        offset: usize,
        size: Size,
        valid_range: WrappingRange,
    ) -> Result<u128> {
        let bits = self.integer(offset, size)?;
        if !valid_range.contains(bits) {
            return Err(self.error("indirect constant live scalar is outside its valid range"));
        }
        Ok(bits)
    }

    fn finish(&mut self) -> Result<Vec<u8>> {
        (self.charge)(self.raw.len())?;
        let mut result = try_vec_v1(self.raw.len(), SemanticMirResourceV1::ConstantBytes)?;
        // Only reached after every live field and tag was proved initialized.
        for (index, byte) in self.raw.iter().copied().enumerate() {
            result.push(if (self.initialized)(index) { byte } else { 0 });
        }
        Ok(result)
    }

    fn direct_variant(
        &mut self,
        bits: u128,
        variants: impl Iterator<Item = (VariantIdx, u128)>,
    ) -> Result<VariantIdx> {
        let mut selected = None;
        for (index, discriminant) in variants {
            (self.charge)(1)?;
            if discriminant == bits && selected.replace(index).is_some() {
                return Err(self.error("indirect constant enum discriminant is ambiguous"));
            }
        }
        selected.ok_or_else(|| self.error("indirect constant has an invalid enum discriminant"))
    }
}

fn visit<'tcx, I: Fn(usize) -> bool, C: FnMut(usize) -> Result<()>>(
    cx: &LayoutCx<'tcx>,
    layout: TyAndLayout<'tcx>,
    base: usize,
    depth: usize,
    bytes: &mut Bytes<'_, I, C>,
) -> Result<()> {
    bytes.node(depth)?;
    bytes.range(base, layout.size.bytes_usize())?;
    if layout.is_uninhabited() || layout.backend_repr.is_unsized() {
        return Err(bytes.error("indirect constant has an uninhabited or unsized layout"));
    }
    match layout.ty.kind() {
        TyKind::Bool | TyKind::Char | TyKind::Int(_) | TyKind::Uint(_) | TyKind::Float(_) => {
            let BackendRepr::Scalar(scalar) = layout.backend_repr else {
                return Err(bytes.error("indirect constant primitive lacks its scalar layout"));
            };
            if scalar.size(cx) != layout.size {
                return Err(bytes.error("indirect constant primitive layout size disagrees"));
            }
            let bits = bytes.scalar(cx, scalar, base)?;
            if matches!(layout.ty.kind(), TyKind::Char)
                && u32::try_from(bits).ok().and_then(char::from_u32).is_none()
            {
                return Err(bytes.error("indirect constant contains an invalid char"));
            }
            Ok(())
        }
        TyKind::Tuple(fields) => fields_visit(cx, layout, fields.len(), base, depth, bytes, true),
        TyKind::Array(_, length) => {
            let FieldsShape::Array { count, .. } = layout.fields else {
                return Err(bytes.error("indirect constant array lacks its exact field layout"));
            };
            if length
                .try_to_target_usize(cx.tcx())
                .is_none_or(|length| length != count)
            {
                return Err(bytes.error("indirect constant array length differs from its layout"));
            }
            let count = usize::try_from(count)
                .map_err(|_| bytes.error("indirect constant array count exceeds host usize"))?;
            fields_visit(cx, layout, count, base, depth, bytes, false)
        }
        TyKind::Adt(definition, _) if definition.is_struct() => fields_visit(
            cx,
            layout,
            definition.non_enum_variant().fields.len(),
            base,
            depth,
            bytes,
            true,
        ),
        TyKind::Adt(definition, _) if definition.is_enum() => {
            let variant = match &layout.variants {
                Variants::Empty => return Err(bytes.error("indirect constant enum is uninhabited")),
                Variants::Single { index } => *index,
                Variants::Multiple {
                    tag,
                    tag_encoding,
                    tag_field,
                    variants,
                } => {
                    if !matches!(tag.primitive(), Primitive::Int(..))
                        || tag_field.as_usize() >= layout.fields.count()
                    {
                        return Err(
                            bytes.error("indirect constant enum has an unsupported tag layout")
                        );
                    }
                    let offset = base
                        .checked_add(layout.fields.offset(tag_field.as_usize()).bytes_usize())
                        .ok_or_else(|| {
                            bytes.error("indirect constant enum tag offset overflows")
                        })?;
                    let bits = bytes.scalar(cx, *tag, offset)?;
                    let variant = match tag_encoding {
                        TagEncoding::Direct => {
                            (bytes.charge)(1)?;
                            let discr = cx.layout_of(layout.ty.discriminant_ty(cx.tcx())).map_err(
                                |_| {
                                    bytes
                                        .error("indirect constant enum discriminant lacks a layout")
                                },
                            )?;
                            let bits = direct_discriminant(bits, tag.primitive(), discr.size)
                                .ok_or_else(|| {
                                    bytes.error(
                                        "indirect constant enum discriminant width is unsupported",
                                    )
                                })?;
                            bytes.direct_variant(
                                bits,
                                definition
                                    .discriminants(cx.tcx())
                                    .map(|(index, value)| (index, value.val)),
                            )?
                        }
                        TagEncoding::Niche {
                            untagged_variant,
                            niche_variants,
                            niche_start,
                        } => {
                            let index = niche_variant(
                                bits,
                                tag.size(cx),
                                *niche_start,
                                niche_variants.start().as_u32(),
                                niche_variants.end().as_u32(),
                                untagged_variant.as_u32(),
                            )
                            .ok_or_else(|| {
                                bytes.error("indirect constant enum niche layout is invalid")
                            })?;
                            VariantIdx::from_u32(index)
                        }
                    };
                    if variants.get(variant).is_none() {
                        return Err(
                            bytes.error("indirect constant selected enum variant lacks a layout")
                        );
                    }
                    variant
                }
            };
            let variant_definition = definition
                .variants()
                .get(variant)
                .ok_or_else(|| bytes.error("indirect constant selected enum variant is absent"))?;
            let selected = layout.for_variant(cx, variant);
            if selected.is_uninhabited() {
                return Err(bytes.error("indirect constant selected enum variant is uninhabited"));
            }
            // Variant ABI scalar pairs may describe inactive storage. Visit only
            // the selected fields; the outer tag was checked separately above.
            fields_visit(
                cx,
                selected,
                variant_definition.fields.len(),
                base,
                depth,
                bytes,
                false,
            )
        }
        _ => Err(bytes.error("indirect constant has an unsupported typed representation")),
    }
}

fn fields_visit<'tcx, I: Fn(usize) -> bool, C: FnMut(usize) -> Result<()>>(
    cx: &LayoutCx<'tcx>,
    layout: TyAndLayout<'tcx>,
    count: usize,
    base: usize,
    depth: usize,
    bytes: &mut Bytes<'_, I, C>,
    check_abi: bool,
) -> Result<()> {
    if matches!(
        layout.fields,
        FieldsShape::Primitive | FieldsShape::Union(_)
    ) || layout.fields.count() != count
    {
        return Err(bytes.error("indirect constant fields differ from their typed layout"));
    }
    if check_abi {
        match layout.backend_repr {
            BackendRepr::Scalar(scalar) => {
                aggregate_scalar(cx, scalar, base, bytes)?;
            }
            BackendRepr::ScalarPair(first, second) => {
                aggregate_scalar(cx, first, base, bytes)?;
                let offset = first.size(cx).align_to(second.align(cx).abi).bytes_usize();
                let at = base
                    .checked_add(offset)
                    .ok_or_else(|| bytes.error("indirect constant scalar-pair offset overflows"))?;
                aggregate_scalar(cx, second, at, bytes)?;
            }
            BackendRepr::Memory { sized: true } => {}
            _ => return Err(bytes.error("indirect constant aggregate ABI is unsupported")),
        }
    }
    let end = base
        .checked_add(layout.size.bytes_usize())
        .ok_or_else(|| bytes.error("indirect constant aggregate range overflows"))?;
    for index in 0..count {
        (bytes.charge)(1)?;
        let field = layout.field(cx, index);
        let offset = base
            .checked_add(layout.fields.offset(index).bytes_usize())
            .ok_or_else(|| bytes.error("indirect constant field offset overflows"))?;
        if offset
            .checked_add(field.size.bytes_usize())
            .is_none_or(|field_end| field_end > end)
        {
            return Err(bytes.error("indirect constant field exceeds its containing layout"));
        }
        visit(cx, field, offset, depth + 1, bytes)?;
    }
    Ok(())
}

fn aggregate_scalar<I: Fn(usize) -> bool, C: FnMut(usize) -> Result<()>>(
    cx: &LayoutCx<'_>,
    scalar: Scalar,
    base: usize,
    bytes: &mut Bytes<'_, I, C>,
) -> Result<()> {
    // A containing aggregate can expose a union ABI slot for an inner enum's
    // inactive payload. Its actual live fields are still checked recursively.
    if matches!(scalar, Scalar::Initialized { .. }) {
        bytes.scalar(cx, scalar, base)?;
    }
    Ok(())
}

fn direct_discriminant(bits: u128, primitive: Primitive, output: Size) -> Option<u128> {
    let Primitive::Int(integer, signed) = primitive else {
        return None;
    };
    if output.bytes() == 0 || output.bytes() > 16 {
        return None;
    }
    Some(output.truncate(if signed {
        integer.size().sign_extend(bits) as u128
    } else {
        bits
    }))
}

fn niche_variant(
    bits: u128,
    size: Size,
    start: u128,
    first: u32,
    last: u32,
    untagged: u32,
) -> Option<u32> {
    if !(1..=16).contains(&size.bytes()) || first > last {
        return None;
    }
    let relative = size.truncate(bits.wrapping_sub(start));
    if relative <= u128::from(last - first) {
        // The reserved range may span the untagged variant. Its encoded slot
        // is a dead tag, not an alternative encoding of the live payload.
        first
            .checked_add(u32::try_from(relative).ok()?)
            .filter(|variant| *variant != untagged)
    } else {
        Some(untagged)
    }
}

#[cfg(test)]
#[path = "indirect_constant_v1_tests.rs"]
mod tests;
