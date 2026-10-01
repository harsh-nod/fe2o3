//! Original enum tags and partial payloads join exact SSA or compiler spill sites.
use super::super::slots::{EnumFieldV47, Spill};
use super::*;
use fe2o3_kernel_ir::{AccessMode, AddressSpace, OperationKind, Type as PhysicalType};
use fe2o3_lower_mir_kernel::{
    ProductionSourceSsaCarrierShapeV37 as Carrier, ProductionSourceSsaEndpointV36 as Endpoint,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticFunctionIdV1 as FunctionId, SemanticTypeIdV1 as TypeId, SemanticTypeShapeV1 as Shape,
};

#[derive(Clone, Copy)]
pub(super) enum FieldCarrier {
    Missing,
    Unit,
    Value(usize),
    Spill {
        row: Spill,
        width: u64,
        alignment: u32,
    },
}

pub(super) struct EnumVariantBinding {
    pub(super) ordinal: u32,
    pub(super) discriminant: u128,
    pub(super) fields: Vec<FieldCarrier>,
}

pub(super) struct EnumBinding {
    pub(super) local: usize,
    pub(super) source_type: TypeId,
    pub(super) tag: usize,
    pub(super) known_variant: Option<u32>,
    pub(super) variants: Vec<EnumVariantBinding>,
}

fn refused() -> Error {
    Error::Statement("original enum payload needs its exact SSA or compiler spill correspondence")
}

fn field_type_matches(
    types: &[super::super::super::Type],
    ty: TypeId,
    kind: EnumFieldV47,
    physical: &PhysicalType,
    width: FormalIndexWidth,
) -> bool {
    let Some(original) = types.get(ty.index() as usize) else {
        return false;
    };
    match kind {
        EnumFieldV47::Scalar(scalar) => {
            aggregate_bindings::scalar_matches(scalar, original.rust_type_kind(), physical, width)
        }
        EnumFieldV47::Reference {
            scalar, mutable, ..
        } => {
            let (Shape::Pointer(source), PhysicalType::Pointer(actual)) =
                (original.shape(), physical)
            else {
                return false;
            };
            let Some(pointee) = types.get(source.pointee().index() as usize) else {
                return false;
            };
            actual.access
                == if mutable {
                    AccessMode::ReadWrite
                } else {
                    AccessMode::ReadOnly
                }
                && matches!(
                    actual.address_space,
                    AddressSpace::Private | AddressSpace::Global | AddressSpace::Generic
                )
                && aggregate_bindings::scalar_matches(
                    scalar,
                    pointee.rust_type_kind(),
                    &actual.pointee,
                    width,
                )
        }
    }
}

fn exact_endpoint(
    endpoint: &Endpoint<'_, '_>,
    function: FunctionId,
    local: usize,
    ty: TypeId,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    out.budget.charge_work(3)?;
    if endpoint.source_function(out.budget)? != function
        || endpoint.source_local(out.budget)?.index() as usize != local
        || endpoint.source_type(out.budget)? != ty
    {
        return Err(refused());
    }
    Ok(())
}

impl PairedInvocations<'_, '_, '_> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn enum_binding(
        &mut self,
        root: usize,
        instance: usize,
        local: usize,
        function: FunctionId,
        source_type: TypeId,
        endpoint: &Endpoint<'_, '_>,
        local_start: usize,
        frame: usize,
        physical: &Range<usize>,
        out: &mut Writer<'_, '_>,
    ) -> Result<Binding> {
        let slots = self.slots;
        let relation = slots.correspondence(out)?;
        let semantic = relation.source(out.budget)?.source_semantic(out.budget)?;
        let inventory = relation.inventory(out.budget)?;
        exact_endpoint(endpoint, function, local, source_type, out)?;
        if slots.has_original_object(
            root,
            instance,
            u32::try_from(local).map_err(|_| Resource::Arithmetic)?,
            out,
        )? {
            return Err(refused());
        }
        let Shape::Enum {
            discriminant,
            variants,
        } = semantic
            .types()
            .get(source_type.index() as usize)
            .ok_or_else(refused)?
            .shape()
        else {
            return Err(refused());
        };
        let Carrier::Enum { known_variant, .. } = endpoint.carrier_shape(out.budget)? else {
            return Err(refused());
        };
        let presence = endpoint.enum_pointer_presence_v52(out.budget)?;
        let presence_encoded = presence.is_some();
        let tag = match presence {
            Some(tag) => tag,
            None => endpoint.enum_discriminant_v47(out.budget)?,
        };
        exact_endpoint(&tag, function, local, *discriminant, out)?;
        let Some(tag_definition) = tag.original_definition(out.budget)? else {
            return Err(refused());
        };
        let Some(tag_type) = tag.physical_type(out.budget)? else {
            return Err(refused());
        };
        let source_tag = ScalarV30::from_source(semantic.types(), *discriminant)?;
        out.budget.charge_work(5)?;
        if !physical.contains(&tag_definition)
            || tag.carrier_shape(out.budget)? != Carrier::Value
            || inventory
                .definitions()
                .get(tag_definition)
                .map(|row| row.ty)
                != Some(tag_type)
            || if presence_encoded {
                *tag_type != PhysicalType::BOOL
                    || variants.len() != 2
                    || variants[0].discriminant() != 0
                    || variants[1].discriminant() != 1
                    || !variants[0].fields().fields().is_empty()
                    || variants[1].fields().fields().len() != 1
                    || known_variant.is_some()
            } else {
                !aggregate_bindings::scalar_matches(
                    source_tag,
                    semantic.types()[discriminant.index() as usize].rust_type_kind(),
                    tag_type,
                    self.width,
                )
            }
            || known_variant.is_some_and(|variant| variant as usize >= variants.len())
        {
            return Err(refused());
        }
        let mut bindings = vector(variants.len(), out)?;
        for (ordinal, declaration) in variants.iter().enumerate() {
            out.budget.charge_work(1)?;
            let ordinal = u32::try_from(ordinal).map_err(|_| Resource::Arithmetic)?;
            if declaration.is_uninhabited() {
                continue;
            }
            if known_variant.is_some_and(|known| known != ordinal) {
                continue;
            }
            let (discriminant, count) = slots
                .logical_enum_variant_v47(source_type, ordinal, out)?
                .ok_or_else(refused)?;
            let retained = endpoint.enum_variant_fields_v47(ordinal, out.budget)?;
            if retained.is_some_and(|length| length != count) {
                return Err(refused());
            }
            let mut fields = vector(count, out)?;
            for field in 0..count {
                let (ty, kind) = slots
                    .logical_enum_field_v47(source_type, ordinal, field, out)?
                    .ok_or_else(refused)?;
                let selected = if retained.is_some() {
                    let child = endpoint.enum_field_carrier_v49(ordinal, field, out.budget)?;
                    if let Some(child) = &child {
                        exact_endpoint(child, function, local, ty, out)?;
                    }
                    child
                } else {
                    None
                };
                let carrier = if matches!(kind, EnumFieldV47::Scalar(ScalarV30::Unit)) {
                    if let Some(child) = selected {
                        if child.carrier_shape(out.budget)? != Carrier::Unit
                            || child.original_definition(out.budget)?.is_some()
                            || child.physical_type(out.budget)?.is_some()
                        {
                            return Err(refused());
                        }
                    }
                    FieldCarrier::Unit
                } else if let Some(child) = selected {
                    let Some(definition) = child.original_definition(out.budget)? else {
                        return Err(refused());
                    };
                    let Some(actual) = child.physical_type(out.budget)? else {
                        return Err(refused());
                    };
                    out.budget.charge_work(4)?;
                    if child.carrier_shape(out.budget)? != Carrier::Value
                        || !physical.contains(&definition)
                        || inventory.definitions().get(definition).map(|row| row.ty) != Some(actual)
                        || !field_type_matches(semantic.types(), ty, kind, actual, self.width)
                    {
                        return Err(refused());
                    }
                    FieldCarrier::Value(definition)
                } else {
                    let Some(spill) = slots
                        .compiler_spill_field(
                            [root, instance, local, ordinal as usize, field, 0],
                            out,
                        )?
                        .copied()
                    else {
                        fields.push(FieldCarrier::Missing);
                        continue;
                    };
                    let (mut lo, mut hi) = (0, inventory.operations().len());
                    while lo < hi {
                        out.budget.charge_work(1)?;
                        let middle = lo + (hi - lo) / 2;
                        if inventory.operations()[middle].coordinate < spill.operation {
                            lo = middle + 1;
                        } else {
                            hi = middle;
                        }
                    }
                    let operation = inventory
                        .operations()
                        .get(lo)
                        .filter(|row| row.coordinate == spill.operation)
                        .ok_or_else(refused)?;
                    let OperationKind::Alloca {
                        element,
                        count: None,
                        alignment,
                        ..
                    } = &operation.operation.kind
                    else {
                        return Err(refused());
                    };
                    let bytes = match kind {
                        EnumFieldV47::Scalar(ScalarV30::Bool) => 1,
                        EnumFieldV47::Scalar(scalar) => u64::from(scalar.width() / 8),
                        EnumFieldV47::Reference { .. } => 8,
                    };
                    out.budget.charge_work(6)?;
                    if spill.origin.source_type != source_type
                        || spill.origin.field_type != ty
                        || !physical.contains(&spill.definition)
                        || !field_type_matches(semantic.types(), ty, kind, element, self.width)
                        || bytes == 0
                        || *alignment == 0
                    {
                        return Err(refused());
                    }
                    FieldCarrier::Spill {
                        row: spill,
                        width: bytes,
                        alignment: *alignment,
                    }
                };
                fields.push(carrier);
            }
            bindings.push(EnumVariantBinding {
                ordinal,
                discriminant,
                fields,
            });
        }
        let index = self.enums.len();
        push_enum(
            &mut self.enums,
            EnumBinding {
                local: add(local_start, local)?,
                source_type,
                tag: tag_definition,
                known_variant,
                variants: bindings,
            },
            out,
        )?;
        Ok(Binding {
            source: SourceValue::Enum(index),
            logical: LogicalBinding::Plain,
            definition: None,
            frame,
        })
    }
}

fn push_enum(
    rows: &mut Vec<EnumBinding>,
    row: EnumBinding,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    out.budget.charge_work(1)?;
    if rows.len() == rows.capacity() {
        let old = rows.capacity();
        let desired = old.checked_mul(2).ok_or(Resource::Arithmetic)?.max(4);
        out.budget.reserve_storage(
            desired
                .checked_mul(size_of::<EnumBinding>())
                .ok_or(Resource::Arithmetic)?,
        )?;
        out.budget.charge_work(rows.len())?;
        rows.try_reserve_exact(desired - rows.len())
            .map_err(|_| Resource::Allocation)?;
        if rows.capacity() > desired {
            out.budget.reserve_storage(
                (rows.capacity() - desired)
                    .checked_mul(size_of::<EnumBinding>())
                    .ok_or(Resource::Arithmetic)?,
            )?;
        }
        out.budget.release_storage(
            old.checked_mul(size_of::<EnumBinding>())
                .ok_or(Resource::Arithmetic)?,
        )?;
    }
    rows.push(row);
    Ok(())
}

pub(super) fn headers() -> usize {
    size_of::<EnumBinding>()
        + 2 * size_of::<Result<EnumBinding>>()
        + size_of::<EnumVariantBinding>()
        + size_of::<FieldCarrier>()
        + size_of::<Vec<EnumBinding>>()
        + size_of::<Vec<EnumVariantBinding>>()
        + size_of::<Vec<FieldCarrier>>()
        + size_of::<Option<Endpoint<'_, '_>>>()
        + 24 * size_of::<usize>()
        + 16 * size_of::<&()>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_enum_cut_headers_have_independent_nominal_and_carrier_fields() {
        #[allow(dead_code)]
        enum Field {
            Missing,
            Unit,
            Value(usize),
            Spill {
                row: Spill,
                width: u64,
                alignment: u32,
            },
        }
        type Variant = (u32, u128, Vec<Field>);
        type Enum = (usize, TypeId, usize, Option<u32>, Vec<Variant>);
        assert_eq!(size_of::<FieldCarrier>(), size_of::<Field>());
        assert_eq!(size_of::<EnumVariantBinding>(), size_of::<Variant>());
        assert_eq!(size_of::<EnumBinding>(), size_of::<Enum>());
        assert_eq!(
            headers(),
            size_of::<Enum>()
                + 2 * size_of::<Result<Enum>>()
                + size_of::<Variant>()
                + size_of::<Field>()
                + size_of::<Vec<Enum>>()
                + size_of::<Vec<Variant>>()
                + size_of::<Vec<Field>>()
                + size_of::<Option<Endpoint<'_, '_>>>()
                + 24 * size_of::<usize>()
                + 16 * size_of::<&()>()
        );
    }

    #[test]
    fn original_enum_cut_runtime_never_uses_heap_bits_as_an_allocation_identity() {
        let runtime = include_str!("original_semantic_mir_enum_bindings_v49.vrs");
        assert!(runtime.contains("match target.values[definition]"));
        assert!(
            runtime.contains("actual_owner == owner && invocation == 0 && actual_site == site")
        );
        assert!(runtime.contains("address.byte_offset != 0 || address.view.is_some()"));
        assert!(
            runtime.contains(
                "byte_pointer_load_valid_v37(target.memory, address, width, little_endian)"
            )
        );
        assert!(
            runtime.contains("byte_scalar_range_initialized_v37(target.memory, address, width)")
        );
        assert!(!runtime.contains("assume("));
        let generate = include_str!("original_semantic_mir_invocation_paired_generate_v36.rs");
        assert!(generate.contains("FieldCarrier::Missing => emit!(out, \"false\")"));
        assert!(generate.contains("!value.fields.contains_key({field})"));
        assert!(generate.contains("InvocationSourceValueV42::Enum(value) => value.source_type"));
        assert!(generate.contains("invocation_source_enum_snapshot_current_v50(source, value"));
        assert!(generate.contains("enum_payload_binding(model, index, out)?"));
    }
}
