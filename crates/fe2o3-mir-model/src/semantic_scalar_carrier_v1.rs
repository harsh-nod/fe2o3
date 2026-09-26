//! One exact transparent-carrier rule, shared by paid and compatibility readers.
use crate::semantic_mir_v1::*;

pub(crate) trait ScalarCarrierMeterV1 {
    type Error;
    fn work(&mut self, amount: usize) -> Result<(), Self::Error>;
    fn reserve_storage(&mut self, bytes: usize) -> Result<(), Self::Error>;
}
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum ScalarCarrierErrorV1<E> {
    Meter(E),
    Arithmetic,
    Allocation,
}

pub(crate) struct UnmeteredCarrierCompatibilityV1;
impl ScalarCarrierMeterV1 for UnmeteredCarrierCompatibilityV1 {
    type Error = std::convert::Infallible;
    fn work(&mut self, _: usize) -> Result<(), Self::Error> {
        Ok(())
    }
    fn reserve_storage(&mut self, _: usize) -> Result<(), Self::Error> {
        Ok(())
    }
}

fn exact_inert_zero_sized_marker_v1<M: ScalarCarrierMeterV1>(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    visiting: &mut [bool],
    meter: &mut M,
) -> Result<bool, ScalarCarrierErrorV1<M::Error>> {
    meter.work(1).map_err(ScalarCarrierErrorV1::Meter)?;
    let Some(slot) = visiting.get_mut(ty.index() as usize) else {
        return Ok(false);
    };
    if *slot {
        return Ok(false);
    }
    *slot = true;
    let Some(marker) = types.get(ty.index() as usize) else {
        visiting[ty.index() as usize] = false;
        return Ok(false);
    };
    let layout = marker.layout();
    let properties = marker.abi_properties();
    let exact_layout = layout.size_bytes() == Some(0)
        && layout.rustc_size_bytes() == 0
        && layout.alignment_bytes() == 1
        && layout.unadjusted_abi_alignment_bytes() == 1
        && layout.max_repr_alignment_bytes().is_none()
        && layout.largest_niche().is_none()
        && !layout.is_uninhabited()
        && matches!(
            layout.variants(),
            SemanticRustcVariantsV1::Single { index: 0 }
        )
        && matches!(
            layout.backend_repr(),
            SemanticBackendReprV1::Memory { sized: true }
        )
        && !properties.pass_indirectly_in_non_rustic_abis()
        && !properties.has_unsized_foreign_tail()
        && properties.rustc_layout_is_noundef()
        && properties.first_pointee().is_none()
        && properties.second_pointee().is_none()
        && marker.rust_type_kind() == SemanticRustTypeKindV1::Ordinary;
    let exact_shape = match (marker.shape(), layout.fields(), layout.details()) {
        (
            SemanticTypeShapeV1::Aggregate(fields) | SemanticTypeShapeV1::Tuple(fields),
            SemanticFieldsShapeV1::Arbitrary {
                source_order_offsets_bytes,
                memory_order_source_indices,
            },
            SemanticTypeLayoutDetailsV1::Aggregate(details),
        ) => {
            meter
                .work(
                    source_order_offsets_bytes
                        .len()
                        .checked_add(details.field_offsets().len())
                        .ok_or(ScalarCarrierErrorV1::Arithmetic)?,
                )
                .map_err(ScalarCarrierErrorV1::Meter)?;
            let exact_fields = source_order_offsets_bytes.len() == fields.fields().len()
                && memory_order_source_indices.len() == fields.fields().len()
                && source_order_offsets_bytes.iter().all(|offset| *offset == 0)
                && details.field_offsets().len() == fields.fields().len()
                && details.field_offsets().iter().all(|offset| *offset == 0)
                && details.padding().is_empty();
            let mut exact_children = exact_fields;
            if exact_children {
                for &field in fields.fields() {
                    if !exact_inert_zero_sized_marker_v1(types, field, visiting, meter)? {
                        exact_children = false;
                        break;
                    }
                }
            }
            exact_children
        }
        _ => false,
    };
    visiting[ty.index() as usize] = false;
    Ok(exact_layout && exact_shape)
}

pub(crate) fn exact_transparent_scalar_carrier_field_metered_v1<M: ScalarCarrierMeterV1>(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    meter: &mut M,
) -> Result<Option<SemanticTypeIdV1>, ScalarCarrierErrorV1<M::Error>> {
    meter.work(1).map_err(ScalarCarrierErrorV1::Meter)?;
    let Some(carrier) = types.get(ty.index() as usize) else {
        return Ok(None);
    };
    let SemanticTypeShapeV1::Aggregate(fields) = carrier.shape() else {
        return Ok(None);
    };
    let [field, markers @ ..] = fields.fields() else {
        return Ok(None);
    };
    let Some(field_decl) = types.get(field.index() as usize) else {
        return Ok(None);
    };
    if !matches!(field_decl.shape(), SemanticTypeShapeV1::Scalar(_)) {
        return Ok(None);
    }
    let carrier_layout = carrier.layout();
    let field_layout = field_decl.layout();
    let exact_scalar_repr = matches!(
        carrier_layout.backend_repr(),
        SemanticBackendReprV1::Scalar(SemanticBackendScalarV1::Initialized { .. })
    ) && carrier_layout.backend_repr() == field_layout.backend_repr();
    if let (
        SemanticFieldsShapeV1::Arbitrary {
            source_order_offsets_bytes,
            ..
        },
        SemanticTypeLayoutDetailsV1::Aggregate(layout),
    ) = (carrier_layout.fields(), carrier_layout.details())
    {
        meter
            .work(
                source_order_offsets_bytes
                    .len()
                    .checked_add(layout.field_offsets().len())
                    .ok_or(ScalarCarrierErrorV1::Arithmetic)?,
            )
            .map_err(ScalarCarrierErrorV1::Meter)?;
    }
    let exact_fields = matches!(
        (carrier_layout.fields(), carrier_layout.details()),
        (
            SemanticFieldsShapeV1::Arbitrary {
                source_order_offsets_bytes,
                memory_order_source_indices,
            },
            SemanticTypeLayoutDetailsV1::Aggregate(layout),
        ) if source_order_offsets_bytes.len() == fields.fields().len()
            && memory_order_source_indices.len() == fields.fields().len()
            && source_order_offsets_bytes.first() == Some(&0)
            && layout.field_offsets() == source_order_offsets_bytes.as_ref()
            && layout.padding().is_empty()
    ) && matches!(field_layout.fields(), SemanticFieldsShapeV1::Primitive);
    let exact_details = matches!(field_layout.details(), SemanticTypeLayoutDetailsV1::None);
    let exact_variants = matches!(
        carrier_layout.variants(),
        SemanticRustcVariantsV1::Single { index: 0 }
    ) && matches!(
        field_layout.variants(),
        SemanticRustcVariantsV1::Single { index: 0 }
    );
    let exact_layout = carrier_layout.size_bytes() == field_layout.size_bytes()
        && carrier_layout.rustc_size_bytes() == field_layout.rustc_size_bytes()
        && carrier_layout.alignment_bytes() == field_layout.alignment_bytes()
        && carrier_layout.unadjusted_abi_alignment_bytes()
            == field_layout.unadjusted_abi_alignment_bytes()
        && carrier_layout.max_repr_alignment_bytes() == field_layout.max_repr_alignment_bytes()
        && carrier_layout.largest_niche().is_none()
        && field_layout.largest_niche().is_none()
        && !carrier_layout.is_uninhabited()
        && !field_layout.is_uninhabited();
    let exact_abi_properties = [carrier, field_decl].into_iter().all(|ty| {
        let properties = ty.abi_properties();
        !properties.pass_indirectly_in_non_rustic_abis()
            && !properties.has_unsized_foreign_tail()
            && properties.rustc_layout_is_noundef()
            && properties.first_pointee().is_none()
            && properties.second_pointee().is_none()
            && ty.rust_type_kind() == SemanticRustTypeKindV1::Ordinary
    });
    let mut visiting = Vec::new();
    if !markers.is_empty() {
        meter
            .reserve_storage(types.len())
            .map_err(ScalarCarrierErrorV1::Meter)?;
        visiting
            .try_reserve_exact(types.len())
            .map_err(|_| ScalarCarrierErrorV1::Allocation)?;
        meter
            .reserve_storage(
                visiting
                    .capacity()
                    .checked_sub(types.len())
                    .ok_or(ScalarCarrierErrorV1::Arithmetic)?,
            )
            .map_err(ScalarCarrierErrorV1::Meter)?;
        meter
            .work(types.len())
            .map_err(ScalarCarrierErrorV1::Meter)?;
        visiting.resize(types.len(), false);
    }
    let mut exact_markers = true;
    for &marker in markers {
        if !exact_inert_zero_sized_marker_v1(types, marker, &mut visiting, meter)? {
            exact_markers = false;
            break;
        }
    }
    Ok((exact_scalar_repr
        && exact_fields
        && exact_details
        && exact_variants
        && exact_layout
        && exact_abi_properties
        && exact_markers)
        .then_some(*field))
}
