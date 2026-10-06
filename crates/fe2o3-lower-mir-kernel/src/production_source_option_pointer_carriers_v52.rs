// A checked optional pointer has a Boolean presence carrier, not an ordinary
// source discriminant. Its archived Some-edge condition remains part of replay.
fn retain_source_option_pointer_carriers_v52(
    instances: &ExecutionInstancesV29<'_>,
    ty: SemanticTypeIdV1,
    binding: &SemanticValueBindingV1,
    carriers: &mut Vec<SourceSsaComponentV37>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<SourceSsaPhysicalV36, ProductionSemanticKirErrorV1> {
    budget.charge_work(20)?;
    let SemanticValueBindingV1::OptionPointer {
        present,
        pointer,
        pointer_ty,
        availability,
    } = binding
    else {
        return Err(source_typed_endpoint_error_v36());
    };
    let types = instances.owner().source_semantic().types();
    let declaration = types
        .get(ty.index() as usize)
        .ok_or_else(source_typed_endpoint_error_v36)?;
    let SemanticTypeShapeV1::Enum { discriminant, .. } = declaration.shape() else {
        return Err(source_typed_endpoint_error_v36());
    };
    let payload = option_payload_type_v1(types, ty).ok_or_else(source_typed_endpoint_error_v36)?;
    let SemanticTypeShapeV1::Pointer(original) = types
        .get(payload.index() as usize)
        .ok_or_else(source_typed_endpoint_error_v36)?
        .shape()
    else {
        return Err(source_typed_endpoint_error_v36());
    };
    let Type::Pointer(actual) = pointer_ty else {
        return Err(source_typed_endpoint_error_v36());
    };
    // This bounded locator covers the existing global mutable scalar-reference
    // issuer. Neither its shape nor the presence bit creates reference validity.
    if original.kind() != SemanticPointerKindV1::Reference
        || original.metadata() != SemanticPointerMetadataV1::None
        || original.mutability() != SemanticMutabilityV1::Mutable
        || original.address_space() != 0
        || original.pointer_width_bits() != 64
        || actual.address_space != AddressSpace::Global
        || actual.access != AccessMode::ReadWrite
        || actual.pointee.as_ref() != &lower_scalar_type(types, original.pointee())?
    {
        return Err(source_typed_endpoint_error_v36());
    }
    let Some(pointer_type @ SourceSsaCarrierTypeV36::Pointer { .. }) =
        SourceSsaCarrierTypeV36::from_type(pointer_ty)
    else {
        return Err(source_typed_endpoint_error_v36());
    };
    let tag = carriers.len();
    emission_push_v1(
        carriers,
        SourceSsaComponentV37 {
            ty: *discriminant,
            physical: SourceSsaPhysicalV36::Value {
                value: *present,
                ty: SourceSsaCarrierTypeV36::Scalar(ScalarType::Bool),
                loan: None,
            },
        },
        budget,
    )?;
    let start = carriers.len();
    let field = start.checked_add(2).ok_or(ArgumentResourceV1::Arithmetic)?;
    for (variant, length) in [(0, 0), (1, 1)] {
        budget.charge_work(2)?;
        emission_push_v1(
            carriers,
            SourceSsaComponentV37 {
                ty,
                physical: SourceSsaPhysicalV36::EnumVariant {
                    variant,
                    start: field,
                    length,
                },
            },
            budget,
        )?;
    }
    emission_push_v1(
        carriers,
        SourceSsaComponentV37 {
            ty: payload,
            physical: SourceSsaPhysicalV36::Value {
                value: *pointer,
                ty: pointer_type,
                loan: None,
            },
        },
        budget,
    )?;
    Ok(SourceSsaPhysicalV36::Enum {
        discriminant: tag,
        start,
        length: 2,
        known_variant: None,
        presence: Some(*availability),
    })
}

impl<'a, 'source> ProductionSourceSsaEndpointV36<'a, 'source> {
    /// Borrows the exact Boolean presence carrier of an archived checked
    /// optional pointer. `None` is an ordinary enum, not an absent payload.
    /// The original variants must remain exactly None=0 and Some=1; this locator
    /// does not establish the Some edge, pointee validity or dereference rights.
    pub fn enum_pointer_presence_v52(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<ProductionSourceSsaEndpointV36<'a, 'source>>> {
        self.owner.retain_query((|| {
            self.owner.query(budget)?;
            budget.charge_work(3)?;
            let SourceSsaPhysicalV36::Enum {
                discriminant,
                presence,
                known_variant,
                length,
                ..
            } = *self.physical
            else {
                return self
                    .owner
                    .source
                    .missing("SSA carrier is not an original enum");
            };
            if presence.is_none() {
                return Ok(None);
            }
            let (expected, variants) = self.enum_source_shape_v47(budget)?;
            if known_variant.is_some()
                || variants != 2
                || length != 2
                || !matches!(self.carriers.get(discriminant), Some(SourceSsaComponentV37 {
                    ty,
                    physical: SourceSsaPhysicalV36::Value {
                        ty: SourceSsaCarrierTypeV36::Scalar(ScalarType::Bool),
                        loan: None,
                        ..
                    },
                }) if *ty == expected)
            {
                return self
                    .owner
                    .source
                    .missing("checked optional pointer presence differs");
            }
            self.enum_carrier_endpoint_v47(discriminant, budget)
                .map(Some)
        })())
    }
}
