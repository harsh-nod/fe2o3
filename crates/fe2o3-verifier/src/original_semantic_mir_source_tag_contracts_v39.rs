//! Original type validity is independent of the target's physical tag decoder.
//! Only a paid classification index is retained; layouts stay in the source owner.

use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBackendPrimitiveV1 as Primitive, SemanticBackendReprV1 as Backend,
    SemanticEnumEncodingV1 as Encoding, SemanticEnumLayoutV1 as EnumLayout,
    SemanticEnumVariantV1 as Variant, SemanticFieldsShapeV1 as Fields,
    SemanticNicheEnumEncodingV1 as Niche, SemanticNichePathComponentV1 as Path,
    SemanticPointerKindV1 as PointerKind, SemanticPointerMetadataV1 as Metadata,
    SemanticRustcVariantsV1 as Variants, SemanticScalarTypeV1 as Scalar,
    SemanticScalarValidityRangeV1 as Validity, SemanticTypeDeclV1 as Declaration,
    SemanticTypeIdV1 as TypeId, SemanticTypeLayoutDetailsV1 as Details,
    SemanticTypeShapeV1 as Shape,
};
use std::fmt::Write as _;

macro_rules! emit {
    ($out:expr, $($arg:tt)*) => { write!($out, $($arg)*).map_err(|_| $out.error())? };
}

#[path = "original_semantic_mir_source_tag_pairs_v40.rs"]
mod pairs;
pub(in super::super) use pairs::SourceTagPairsV40;
#[cfg(test)]
pub(in super::super) use pairs::tests::{
    Fixture as SourceTagFixtureV40, transform as source_tag_fixture_v40,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) enum SourceTagClassV39 {
    NotTagged,
    DirectScalar,
    ScalarNiche { terminal: TypeId },
    PointerNullReference { terminal: TypeId },
    Unsupported,
}

pub(super) struct SourceTagIndexV39 {
    classes: Vec<SourceTagClassV39>,
}

pub(in super::super) struct SourceTagRecipeV39<'a, 'view, 'source> {
    slots: &'a SourceSlots<'view, 'source>,
    ty: TypeId,
}

fn error() -> Error {
    Error::Statement("original source tag contract differs from retained type")
}

fn mask(bits: u16) -> Result<u128> {
    match bits {
        8 | 16 | 32 | 64 => Ok((1u128 << bits) - 1),
        128 => Ok(u128::MAX),
        _ => Err(error()),
    }
}

fn terminal(
    types: &[Declaration],
    variants: &[Variant],
    layout: &EnumLayout,
    niche: &Niche,
    out: &mut Writer<'_, '_>,
) -> Result<TypeId> {
    let variant = variants
        .get(niche.untagged_variant() as usize)
        .ok_or_else(error)?;
    let variant_layout = layout
        .variants()
        .get(niche.untagged_variant() as usize)
        .ok_or_else(error)?;
    let mut current: Option<TypeId> = None;
    let mut offset = 0u64;
    for component in niche.source().path() {
        out.budget.charge_work(6)?;
        let (next, relative) = match (component, current) {
            (Path::Field(index), None) => (
                *variant
                    .fields()
                    .fields()
                    .get(*index as usize)
                    .ok_or_else(error)?,
                *variant_layout
                    .aggregate()
                    .field_offsets()
                    .get(*index as usize)
                    .ok_or_else(error)?,
            ),
            (Path::Field(index), Some(id)) => {
                let row: &Declaration = types.get(id.index() as usize).ok_or_else(error)?;
                let (Shape::Tuple(fields) | Shape::Aggregate(fields), Details::Aggregate(layout)) =
                    (row.shape(), row.layout().details())
                else {
                    return Err(error());
                };
                (
                    *fields.fields().get(*index as usize).ok_or_else(error)?,
                    *layout
                        .field_offsets()
                        .get(*index as usize)
                        .ok_or_else(error)?,
                )
            }
            (Path::ArrayElement(index), Some(id)) => {
                let row: &Declaration = types.get(id.index() as usize).ok_or_else(error)?;
                let (
                    Shape::Array { element, length },
                    Fields::Array {
                        stride_bytes,
                        count,
                    },
                ) = (row.shape(), row.layout().fields())
                else {
                    return Err(error());
                };
                if length != count || index >= length {
                    return Err(error());
                }
                (
                    *element,
                    stride_bytes
                        .checked_mul(*index)
                        .ok_or(Resource::Arithmetic)?,
                )
            }
            (Path::ArrayElement(_), None) => return Err(error()),
        };
        offset = offset.checked_add(relative).ok_or(Resource::Arithmetic)?;
        current = Some(next);
    }
    out.budget.charge_work(3)?;
    if offset != niche.source().expected_offset_bytes()
        || offset != niche.source_niche().offset_bytes()
    {
        return Err(error());
    }
    current.ok_or_else(error)
}

fn integer_value(bits: u16, signed: bool, value: u128) -> Result<(bool, u128)> {
    let mask = mask(bits)?;
    if value > mask {
        return Err(error());
    }
    if signed && value & (1u128 << (bits - 1)) != 0 {
        Ok((true, ((!value) & mask).wrapping_add(1)))
    } else {
        Ok((false, value))
    }
}

fn direct_tag(logical: Scalar, primitive: Primitive, discriminant: u128) -> Result<u128> {
    let (
        Scalar::Integer { signed, bits },
        Primitive::Integer {
            signed: tag_signed,
            bits: tag_bits,
            ..
        },
    ) = (logical, primitive)
    else {
        return Err(error());
    };
    let (negative, magnitude) = integer_value(bits, signed, discriminant)?;
    let encoded = if negative {
        0u128.wrapping_sub(magnitude)
    } else {
        magnitude
    } & mask(tag_bits)?;
    if integer_value(tag_bits, tag_signed, encoded)? != (negative, magnitude) {
        return Err(error());
    }
    Ok(encoded)
}

fn classify(
    types: &[Declaration],
    row: &Declaration,
    out: &mut Writer<'_, '_>,
) -> Result<SourceTagClassV39> {
    use SourceTagClassV39 as Class;
    out.budget.charge_work(5)?;
    let Variants::Multiple(layout) = row.layout().variants() else {
        return Ok(Class::NotTagged);
    };
    let Shape::Enum {
        discriminant,
        variants,
    } = row.shape()
    else {
        return Err(error());
    };
    if variants.len() != layout.variants().len() || row.layout().size_bytes().is_none() {
        return Err(error());
    }
    match layout.encoding() {
        Encoding::Direct(direct) => {
            let Shape::Scalar(logical) = types
                .get(discriminant.index() as usize)
                .ok_or_else(error)?
                .shape()
            else {
                return Ok(Class::Unsupported);
            };
            for variant in variants {
                out.budget.charge_work(2)?;
                if direct_tag(*logical, direct.tag().primitive(), variant.discriminant()).is_err() {
                    return Ok(Class::Unsupported);
                }
            }
            Ok(Class::DirectScalar)
        }
        Encoding::Niche(niche) => {
            let terminal = terminal(types, variants, layout, niche, out)?;
            let leaf = types.get(terminal.index() as usize).ok_or_else(error)?;
            let Backend::Scalar(scalar) = leaf.layout().backend_repr() else {
                return Ok(Class::Unsupported);
            };
            if scalar.primitive() != niche.source_niche().primitive()
                || scalar.valid_range() != Some(niche.source_niche().valid_range())
            {
                return Err(error());
            }
            match (scalar.primitive(), leaf.shape()) {
                (
                    Primitive::Integer { bits, .. },
                    Shape::Scalar(Scalar::Bool | Scalar::Char | Scalar::Integer { .. })
                    | Shape::ValidityScalar(_),
                ) => {
                    mask(bits)?;
                    Ok(Class::ScalarNiche { terminal })
                }
                (
                    Primitive::Pointer {
                        address_space,
                        size_bytes,
                        ..
                    },
                    Shape::Pointer(pointer),
                ) => {
                    let bits = pointer.pointer_width_bits();
                    let range = niche.source_niche().valid_range();
                    let (first, last) = niche.niche_variant_range();
                    if pointer.kind() == PointerKind::Reference
                        && pointer.metadata() == Metadata::None
                        && pointer.address_space() == address_space
                        && size_bytes.checked_mul(8) == Some(u64::from(bits))
                        && matches!(bits, 8 | 16 | 32 | 64 | 128)
                        && range == Validity::new(1, mask(bits)?)
                        && first == last
                        && niche.niche_start() == 0
                    {
                        Ok(Class::PointerNullReference { terminal })
                    } else {
                        Ok(Class::Unsupported)
                    }
                }
                _ => Ok(Class::Unsupported),
            }
        }
    }
}

impl SourceTagIndexV39 {
    pub(super) fn derive(types: &[Declaration], out: &mut Writer<'_, '_>) -> Result<Self> {
        out.budget.reserve_storage(headers())?;
        let mut classes = vector(types.len(), out)?;
        for row in types {
            out.budget.charge_work(1)?;
            classes.push(classify(types, row, out)?);
        }
        Ok(Self { classes })
    }
}

impl<'view, 'source> SourceSlots<'view, 'source> {
    fn check_tag_query(&self, out: &mut Writer<'_, '_>) -> Result<()> {
        let result = (|| {
            self.check_source(self.relation, out)?;
            out.budget.charge_work(1)?;
            Ok(())
        })();
        match result {
            Err(Error::Resource(resource)) => Err(self
                .relation
                .retain_query_resource_error_v18(resource)
                .into()),
            result => result,
        }
    }

    pub(in super::super) fn tag_contract(
        &self,
        ty: TypeId,
        out: &mut Writer<'_, '_>,
    ) -> Result<SourceTagRecipeV39<'_, 'view, 'source>> {
        self.check_tag_query(out)?;
        self.tags
            .classes
            .get(ty.index() as usize)
            .ok_or_else(error)?;
        Ok(SourceTagRecipeV39 { slots: self, ty })
    }

    pub(in super::super) fn emit_source_tag_contracts(
        &self,
        namespace: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        let result = self.emit_source_tag_contracts_inner(namespace, out);
        match result {
            Err(Error::Resource(resource)) => Err(self
                .relation
                .retain_query_resource_error_v18(resource)
                .into()),
            result => result,
        }
    }

    fn emit_source_tag_contracts_inner(
        &self,
        namespace: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.check_source(self.relation, out)?;
        let semantic = self
            .relation
            .source(out.budget)?
            .source_semantic(out.budget)?;
        if semantic.types().len() != self.tags.classes.len() {
            return Err(error());
        }
        emit!(
            out,
            "spec fn invocation_source_view_contracts_{namespace}_v39(little_endian: bool) -> MemoryViewContractsV38 {{\n MemoryViewContractsV38 {{ owner: {namespace}, rows: Map::empty()"
        );
        for (ty, (row, class)) in semantic.types().iter().zip(&self.tags.classes).enumerate() {
            out.budget.charge_work(2)?;
            if matches!(
                class,
                SourceTagClassV39::NotTagged | SourceTagClassV39::Unsupported
            ) {
                continue;
            }
            let (
                Shape::Enum {
                    discriminant,
                    variants,
                },
                Variants::Multiple(layout),
            ) = (row.shape(), row.layout().variants())
            else {
                return Err(error());
            };
            let (offset, primitive) = match layout.encoding() {
                Encoding::Direct(tag) => (tag.tag_offset_bytes(), tag.tag().primitive()),
                Encoding::Niche(tag) => {
                    (tag.source().expected_offset_bytes(), tag.tag().primitive())
                }
            };
            emit!(
                out,
                "\n .insert({ty}, MemoryTagContractV38 {{ object_bytes: {}, tag_offset: {offset}, tag_width: {}, little_endian, inhabited: seq![",
                row.layout().size_bytes().ok_or_else(error)?,
                primitive.size_bytes().ok_or_else(error)?
            );
            for variant in variants {
                out.budget.charge_work(1)?;
                emit!(out, "{},", !variant.is_uninhabited());
            }
            emit!(out, "], discriminants: seq![");
            for variant in variants {
                out.budget.charge_work(1)?;
                emit!(out, "{}int,", variant.discriminant());
            }
            emit!(out, "], untagged_valid_bits: seq![");
            if let SourceTagClassV39::ScalarNiche { terminal } = class {
                let leaf = semantic
                    .types()
                    .get(terminal.index() as usize)
                    .ok_or_else(error)?;
                let Encoding::Niche(niche) = layout.encoding() else {
                    return Err(error());
                };
                let Primitive::Integer { bits, .. } = primitive else {
                    return Err(error());
                };
                emit_validity(
                    leaf.shape(),
                    niche.source_niche().valid_range(),
                    mask(bits)?,
                    out,
                )?;
            }
            emit!(out, "], encoding: ");
            match (class, layout.encoding()) {
                (SourceTagClassV39::DirectScalar, Encoding::Direct(tag)) => {
                    let Shape::Scalar(logical) = semantic
                        .types()
                        .get(discriminant.index() as usize)
                        .ok_or_else(error)?
                        .shape()
                    else {
                        return Err(error());
                    };
                    emit!(out, "MemoryTagEncodingV38::Direct {{ tags: seq![");
                    for variant in variants {
                        out.budget.charge_work(2)?;
                        emit!(
                            out,
                            "{}int,",
                            direct_tag(*logical, tag.tag().primitive(), variant.discriminant())?
                        );
                    }
                    emit!(out, "] }}");
                }
                (SourceTagClassV39::ScalarNiche { .. }, Encoding::Niche(niche)) => {
                    let (first, last) = niche.niche_variant_range();
                    emit!(
                        out,
                        "MemoryTagEncodingV38::Niche {{ untagged: {}, first: {first}, last: {last}, start: {}int }}",
                        niche.untagged_variant(),
                        niche.niche_start()
                    );
                }
                (SourceTagClassV39::PointerNullReference { .. }, Encoding::Niche(niche)) => {
                    emit!(
                        out,
                        "MemoryTagEncodingV38::PointerNullNiche {{ nonnull: {}, null: {} }}",
                        niche.untagged_variant(),
                        niche.niche_variant_range().0
                    );
                }
                _ => return Err(error()),
            }
            emit!(out, " }})");
        }
        emit!(
            out,
            "\n }}\n}}\nspec fn invocation_source_view_contracts_match_{namespace}_v39(memory: ByteMemoryV30, little_endian: bool) -> bool {{ memory.view_contracts == invocation_source_view_contracts_{namespace}_v39(little_endian) }}\n"
        );
        Ok(())
    }
}

fn emit_intersection(
    range: Validity,
    backend: Validity,
    maximum: u128,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    if range.start() > range.end() || range.end() > maximum {
        return Err(error());
    }
    let portions = if backend.start() <= backend.end() {
        [Some(backend), None]
    } else {
        [
            Some(Validity::new(0, backend.end())),
            Some(Validity::new(backend.start(), maximum)),
        ]
    };
    for portion in portions.into_iter().flatten() {
        out.budget.charge_work(3)?;
        let first = range.start().max(portion.start());
        let last = range.end().min(portion.end());
        if first <= last {
            emit!(out, "({first}int,{last}int),");
        }
    }
    Ok(())
}

fn emit_validity(
    shape: &Shape,
    backend: Validity,
    maximum: u128,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    match shape {
        Shape::Scalar(Scalar::Bool) => {
            emit_intersection(Validity::new(0, 1), backend, maximum, out)
        }
        Shape::Scalar(Scalar::Char) => {
            emit_intersection(Validity::new(0, 0xd7ff), backend, maximum, out)?;
            emit_intersection(Validity::new(0xe000, 0x10ffff), backend, maximum, out)
        }
        Shape::Scalar(Scalar::Integer { .. }) => {
            emit_intersection(Validity::new(0, maximum), backend, maximum, out)
        }
        Shape::ValidityScalar(validity) => {
            for range in validity.valid_ranges() {
                out.budget.charge_work(1)?;
                emit_intersection(*range, backend, maximum, out)?;
            }
            Ok(())
        }
        _ => Err(error()),
    }
}

impl SourceTagRecipeV39<'_, '_, '_> {
    pub(in super::super) fn source_type(&self, out: &mut Writer<'_, '_>) -> Result<TypeId> {
        self.slots.check_tag_query(out)?;
        Ok(self.ty)
    }

    pub(in super::super) fn declaration(&self, out: &mut Writer<'_, '_>) -> Result<&Declaration> {
        self.slots.check_tag_query(out)?;
        self.slots
            .relation
            .source(out.budget)?
            .source_semantic(out.budget)?
            .types()
            .get(self.ty.index() as usize)
            .ok_or_else(error)
    }

    pub(in super::super) fn class(&self, out: &mut Writer<'_, '_>) -> Result<SourceTagClassV39> {
        self.slots.check_tag_query(out)?;
        self.slots
            .tags
            .classes
            .get(self.ty.index() as usize)
            .copied()
            .ok_or_else(error)
    }
}

pub(super) fn headers() -> usize {
    size_of::<SourceTagIndexV39>()
        + size_of::<SourceTagRecipeV39<'_, '_, '_>>()
        + size_of::<Result<SourceTagIndexV39>>()
        + size_of::<Result<SourceTagRecipeV39<'_, '_, '_>>>()
        + 3 * size_of::<Result<SourceTagClassV39>>()
        + size_of::<Result<TypeId>>()
        + size_of::<Result<&Declaration>>()
        + size_of::<(&SourceSlots<'_, '_>, &mut Writer<'_, '_>)>()
        + 2 * size_of::<Result<()>>()
        + size_of::<Option<TypeId>>()
        + size_of::<Result<u128>>()
        + 2 * size_of::<Result<(bool, u128)>>()
        + size_of::<[Option<Validity>; 2]>()
        + size_of::<std::iter::Flatten<std::array::IntoIter<Option<Validity>, 2>>>()
        + size_of::<
            std::iter::Enumerate<
                std::iter::Zip<
                    std::slice::Iter<'static, Declaration>,
                    std::slice::Iter<'static, SourceTagClassV39>,
                >,
            >,
        >()
        + size_of::<(
            [&Declaration; 4],
            &[Declaration],
            &[Variant],
            &EnumLayout,
            &Niche,
            std::slice::Iter<'static, Declaration>,
            std::slice::Iter<'static, Variant>,
            std::slice::Iter<'static, Path>,
            [usize; 6],
            [u64; 6],
            [u128; 6],
            [u16; 4],
            [bool; 4],
            Primitive,
            Scalar,
            SourceTagClassV39,
        )>()
}

#[cfg(test)]
#[path = "original_semantic_mir_source_tag_contracts_v39_tests.rs"]
mod tests;
