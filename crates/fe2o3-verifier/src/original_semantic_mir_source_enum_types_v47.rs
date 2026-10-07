//! Logical enum declarations borrowed from the original owner, not tag layouts.
use super::super::super::{ScalarV30, Shape, Type, TypeId};
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticMutabilityV1, SemanticPointerKindV1, SemanticPointerMetadataV1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) enum EnumFieldV47 {
    Scalar(ScalarV30),
    Reference {
        scalar: ScalarV30,
        bytes: u64,
        alignment: u64,
        mutable: bool,
    },
}

fn scalar(types: &[Type], ty: TypeId) -> Option<ScalarV30> {
    let declaration = types.get(ty.index() as usize)?;
    let scalar = ScalarV30::from_source(types, ty).ok()?;
    let bytes = match scalar {
        ScalarV30::Unit => 0,
        ScalarV30::Bool => 1,
        ScalarV30::Integer { width, .. } | ScalarV30::Float { width } => u64::from(width / 8),
    };
    (!declaration.layout().is_uninhabited() && declaration.layout().size_bytes() == Some(bytes))
        .then_some(scalar)
}

fn discriminant_fits(value: u128, width: u32) -> bool {
    match width {
        128 => true,
        1..=127 => value < (1u128 << width),
        _ => false,
    }
}

fn field(types: &[Type], ty: TypeId, out: &mut Writer<'_, '_>) -> Result<Option<EnumFieldV47>> {
    out.budget.charge_work(8)?;
    let declaration = types.get(ty.index() as usize).ok_or_else(mismatch)?;
    if let Some(scalar) = scalar(types, ty) {
        return Ok(Some(EnumFieldV47::Scalar(scalar)));
    }
    let Shape::Pointer(pointer) = declaration.shape() else {
        return Ok(None);
    };
    if pointer.kind() != SemanticPointerKindV1::Reference
        || pointer.metadata() != SemanticPointerMetadataV1::None
        || pointer.address_space() != 0
        || pointer.pointer_width_bits() != 64
        || declaration.layout().size_bytes() != Some(8)
        || declaration.layout().is_uninhabited()
    {
        return Ok(None);
    }
    let Some(scalar) = scalar(types, pointer.pointee()) else {
        return Ok(None);
    };
    let referent = types
        .get(pointer.pointee().index() as usize)
        .ok_or_else(mismatch)?;
    Ok(Some(EnumFieldV47::Reference {
        scalar,
        bytes: referent.layout().size_bytes().ok_or_else(mismatch)?,
        alignment: referent.layout().alignment_bytes(),
        mutable: pointer.mutability() == SemanticMutabilityV1::Mutable,
    }))
}

fn variant<'a>(
    types: &'a [Type],
    ty: TypeId,
    selected: u32,
    out: &mut Writer<'_, '_>,
) -> Result<Option<(u128, &'a [TypeId])>> {
    out.budget.charge_work(6)?;
    let declaration = types.get(ty.index() as usize).ok_or_else(mismatch)?;
    let Shape::Enum {
        discriminant,
        variants,
    } = declaration.shape()
    else {
        return Ok(None);
    };
    let Some(ScalarV30::Integer { width, .. }) = scalar(types, *discriminant) else {
        return Ok(None);
    };
    let row = variants.get(selected as usize).ok_or_else(mismatch)?;
    if declaration.layout().is_uninhabited()
        || row.is_uninhabited()
        || !discriminant_fits(row.discriminant(), width)
    {
        return Ok(None);
    }
    Ok(Some((row.discriminant(), row.fields().fields())))
}

// A product transports an existing complete enum snapshot. Its opaque atom
// must use the same admitted variants and field model as that snapshot.
pub(super) fn product_atom_v282(
    types: &[Type],
    ty: TypeId,
    out: &mut Writer<'_, '_>,
) -> Result<Option<bool>> {
    out.budget.charge_work(2)?;
    let Shape::Enum { variants, .. } = types.get(ty.index() as usize).ok_or_else(mismatch)?.shape()
    else {
        return Ok(None);
    };
    if variants.is_empty() {
        return Ok(None);
    }
    let mut copyable = true;
    for selected in 0..variants.len() {
        out.budget.charge_work(1)?;
        let Some((_, fields)) = variant(
            types,
            ty,
            u32::try_from(selected).map_err(|_| Resource::Arithmetic)?,
            out,
        )?
        else {
            return Ok(None);
        };
        for ty in fields {
            match field(types, *ty, out)? {
                Some(EnumFieldV47::Reference { mutable: true, .. }) => copyable = false,
                Some(_) => (),
                None => return Ok(None),
            }
        }
    }
    Ok(Some(copyable))
}

impl SourceSlots<'_, '_> {
    pub(in super::super) fn logical_enum_variant_v47(
        &self,
        ty: TypeId,
        selected: u32,
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<(u128, usize)>> {
        self.with_source_query_v42(out, |out| {
            let source = self.relation.source(out.budget)?;
            let types = source.source_semantic(out.budget)?.types();
            Ok(variant(types, ty, selected, out)?.map(|(tag, fields)| (tag, fields.len())))
        })
    }

    pub(in super::super) fn logical_enum_field_v47(
        &self,
        ty: TypeId,
        selected: u32,
        ordinal: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<(TypeId, EnumFieldV47)>> {
        self.with_source_query_v42(out, |out| {
            let source = self.relation.source(out.budget)?;
            let types = source.source_semantic(out.budget)?.types();
            let Some((_, fields)) = variant(types, ty, selected, out)? else {
                return Ok(None);
            };
            let ty = *fields.get(ordinal).ok_or_else(mismatch)?;
            Ok(field(types, ty, out)?.map(|kind| (ty, kind)))
        })
    }

    pub(super) fn emit_logical_enum_types_v47(&self, out: &mut Writer<'_, '_>) -> Result<()> {
        self.with_source_query_v42(out, |out| {
            let source = self.relation.source(out.budget)?;
            let types = source.source_semantic(out.budget)?.types();
            for function in ["field_count", "discriminant"] {
                write!(out, "spec fn invocation_source_enum_{function}_v47(ty: int, variant: int) -> Option<int> {{\n")
                    .map_err(|_| out.error())?;
                for (index, declaration) in types.iter().enumerate() {
                    out.budget.charge_work(1)?;
                    let Shape::Enum { variants, .. } = declaration.shape() else { continue };
                    let ty = TypeId::from_index(u32::try_from(index).map_err(|_| Resource::Arithmetic)?);
                    for selected in 0..variants.len() {
                        let selected = u32::try_from(selected).map_err(|_| Resource::Arithmetic)?;
                        let Some((tag, fields)) = variant(types, ty, selected, out)? else { continue };
                        write!(out, " if ty == {index}int && variant == {selected}int {{ Some(")
                            .map_err(|_| out.error())?;
                        if function == "field_count" {
                            write!(out, "{}int", fields.len()).map_err(|_| out.error())?;
                        } else {
                            write!(out, "{tag}int").map_err(|_| out.error())?;
                        }
                        write!(out, ") }} else").map_err(|_| out.error())?;
                    }
                }
                write!(out, " {{ None }}\n}}\n").map_err(|_| out.error())?;
            }
            write!(out, "spec fn invocation_source_enum_field_type_v47(ty: int, variant: int, field: int) -> Option<InvocationSourceEnumFieldTypeV47> {{\n")
                .map_err(|_| out.error())?;
            for (index, declaration) in types.iter().enumerate() {
                out.budget.charge_work(1)?;
                let Shape::Enum { variants, .. } = declaration.shape() else { continue };
                let ty = TypeId::from_index(u32::try_from(index).map_err(|_| Resource::Arithmetic)?);
                for selected in 0..variants.len() {
                    let selected = u32::try_from(selected).map_err(|_| Resource::Arithmetic)?;
                    let Some((_, fields)) = variant(types, ty, selected, out)? else { continue };
                    for (ordinal, ty) in fields.iter().copied().enumerate() {
                        let Some(kind) = field(types, ty, out)? else { continue };
                        write!(out, " if ty == {index}int && variant == {selected}int && field == {ordinal}int {{ Some(")
                            .map_err(|_| out.error())?;
                        match kind {
                            EnumFieldV47::Scalar(scalar) => write!(out,
                                "InvocationSourceEnumFieldTypeV47::Scalar {{ bits: {}int }}", scalar.width()),
                            EnumFieldV47::Reference { scalar, bytes, alignment, mutable } => write!(out,
                                "InvocationSourceEnumFieldTypeV47::Reference {{ referent_bits: {}int, referent_width: {bytes}int, referent_alignment: {alignment}int, mutable: {mutable} }}", scalar.width()),
                        }.map_err(|_| out.error())?;
                        write!(out, ") }} else").map_err(|_| out.error())?;
                    }
                }
            }
            write!(out, " {{ None }}\n}}\n").map_err(|_| out.error())
        })
    }
}

pub(super) fn headers() -> usize {
    size_of::<EnumFieldV47>()
        + 2 * size_of::<Result<Option<EnumFieldV47>>>()
        + 2 * size_of::<Result<Option<(u128, &[TypeId])>>>()
        + size_of::<Result<Option<(u128, usize)>>>()
        + size_of::<Result<Option<(TypeId, EnumFieldV47)>>>()
        + size_of::<std::iter::Enumerate<std::slice::Iter<'_, Type>>>()
        + size_of::<std::iter::Enumerate<std::slice::Iter<'_, TypeId>>>()
        + size_of::<(u128, u32, bool)>()
        + 16 * size_of::<usize>()
        + 14 * size_of::<&()>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticLayoutIdentityV1, SemanticScalarTypeV1, SemanticTypeIdentityV1,
        SemanticTypeLayoutV1,
    };

    #[test]
    fn original_enum_discriminant_bounds_are_total_through_the_full_u128_domain() {
        assert!(discriminant_fits(u128::MAX, 128));
        assert!(discriminant_fits(1u128 << 127, 128));
        assert!(discriminant_fits(0, 128));
        for width in [1, 8, 16, 32, 64, 127] {
            let limit = 1u128 << width;
            assert!(discriminant_fits(limit - 1, width));
            assert!(!discriminant_fits(limit, width));
            assert!(!discriminant_fits(u128::MAX, width));
        }
        assert!(!discriminant_fits(0, 0));
        assert!(!discriminant_fits(0, 129));
        assert!(!discriminant_fits(0, u32::MAX));
    }

    #[test]
    fn original_enum_scalar_classifier_does_not_widen_existing_128_bit_source_admission() {
        for signed in [false, true] {
            let declaration = Type::new(
                SemanticTypeIdentityV1::from_sha256([1; 32]),
                SemanticLayoutIdentityV1::from_sha256([1; 32]),
                SemanticTypeLayoutV1::new(Some(16), 16).unwrap(),
                Shape::Scalar(SemanticScalarTypeV1::Integer { bits: 128, signed }),
            );
            let types = [declaration];
            let ty = TypeId::from_index(0);
            assert!(ScalarV30::from_source(&types, ty).is_err());
            assert!(scalar(&types, ty).is_none());
        }
    }

    #[test]
    fn original_enum_type_queries_have_an_independent_fixed_header_envelope() {
        #[allow(dead_code)]
        enum Field {
            Scalar(ScalarV30),
            Reference {
                scalar: ScalarV30,
                bytes: u64,
                alignment: u64,
                mutable: bool,
            },
        }
        assert_eq!(size_of::<EnumFieldV47>(), size_of::<Field>());
        assert_eq!(
            headers(),
            size_of::<Field>()
                + 2 * size_of::<Result<Option<Field>>>()
                + 2 * size_of::<Result<Option<(u128, &[TypeId])>>>()
                + size_of::<Result<Option<(u128, usize)>>>()
                + size_of::<Result<Option<(TypeId, Field)>>>()
                + size_of::<std::iter::Enumerate<std::slice::Iter<'_, Type>>>()
                + size_of::<std::iter::Enumerate<std::slice::Iter<'_, TypeId>>>()
                + size_of::<(u128, u32, bool)>()
                + 16 * size_of::<usize>()
                + 14 * size_of::<&()>()
        );
    }
}
