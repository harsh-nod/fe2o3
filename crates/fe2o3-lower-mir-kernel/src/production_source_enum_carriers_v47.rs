// Retain locators from the existing emitted enum binding. The discriminant is
// logical source SSA, not a tag-byte decoder or evidence that a payload is live.
include!("production_source_option_pointer_carriers_v52.rs");

fn retain_source_enum_carriers_v47<'a>(
    instances: &ExecutionInstancesV29<'_>,
    ty: SemanticTypeIdV1,
    binding: &'a SemanticValueBindingV1,
    carriers: &mut Vec<SourceSsaComponentV37>,
    pending: &mut Vec<SourceCarrierFrameV37<'a>>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<SourceSsaPhysicalV36>, ProductionSemanticKirErrorV1> {
    if matches!(binding, SemanticValueBindingV1::OptionPointer { .. }) {
        return retain_source_option_pointer_carriers_v52(instances, ty, binding, carriers, budget)
            .map(Some);
    }
    let SemanticValueBindingV1::Enum {
        discriminant,
        discriminant_ty,
        semantic_type,
        variant,
        payloads,
    } = binding
    else {
        return Ok(None);
    };
    budget.charge_work(9)?;
    let types = instances.owner().source_semantic().types();
    let declaration = types
        .get(ty.index() as usize)
        .ok_or_else(source_typed_endpoint_error_v36)?;
    let SemanticTypeShapeV1::Enum {
        discriminant: source_discriminant,
        variants,
    } = declaration.shape()
    else {
        return Err(source_typed_endpoint_error_v36());
    };
    if *semantic_type != ty
        || variant.is_some_and(|index| index as usize >= variants.len())
        || lower_scalar_type(types, *source_discriminant)? != *discriminant_ty
    {
        return Err(source_typed_endpoint_error_v36());
    }
    let Some(tag_type @ SourceSsaCarrierTypeV36::Scalar(_)) =
        SourceSsaCarrierTypeV36::from_type(discriminant_ty)
    else {
        return Err(source_typed_endpoint_error_v36());
    };
    let tag = carriers.len();
    emission_push_v1(
        carriers,
        SourceSsaComponentV37 {
            ty: *source_discriminant,
            physical: SourceSsaPhysicalV36::Value {
                value: *discriminant,
                ty: tag_type,
                loan: None,
            },
        },
        budget,
    )?;
    let start = carriers.len();
    for (index, fields) in payloads {
        budget.charge_work(5)?;
        let original = variants
            .get(*index as usize)
            .ok_or_else(source_typed_endpoint_error_v36)?;
        if original.fields().fields().len() != fields.len() {
            return Err(source_typed_endpoint_error_v36());
        }
        emission_push_v1(
            carriers,
            SourceSsaComponentV37 {
                ty,
                physical: SourceSsaPhysicalV36::EnumVariant {
                    variant: *index,
                    start: 0,
                    length: fields.len(),
                },
            },
            budget,
        )?;
    }
    // Immediate variant rows are contiguous and ordered by original ordinal.
    // Their field slots are separate, so absent and empty payloads stay distinct.
    for (ordinal, (index, fields)) in payloads.iter().enumerate() {
        budget.charge_work(5)?;
        let row = start
            .checked_add(ordinal)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        let field_start = carriers.len();
        let original = variants
            .get(*index as usize)
            .ok_or_else(source_typed_endpoint_error_v36)?;
        for field in original.fields().fields() {
            budget.charge_work(2)?;
            emission_push_v1(
                carriers,
                SourceSsaComponentV37 {
                    ty: *field,
                    physical: SourceSsaPhysicalV36::Unmodeled,
                },
                budget,
            )?;
        }
        carriers
            .get_mut(row)
            .ok_or_else(source_typed_endpoint_error_v36)?
            .physical = SourceSsaPhysicalV36::EnumVariant {
            variant: *index,
            start: field_start,
            length: fields.len(),
        };
        for (field, binding) in fields.iter().enumerate().rev() {
            budget.charge_work(3)?;
            let destination = field_start
                .checked_add(field)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            let ty = carriers
                .get(destination)
                .ok_or_else(source_typed_endpoint_error_v36)?
                .ty;
            emission_push_v1(
                pending,
                SourceCarrierFrameV37 {
                    binding,
                    ty,
                    destination: Some(destination),
                },
                budget,
            )?;
        }
    }
    Ok(Some(SourceSsaPhysicalV36::Enum {
        discriminant: tag,
        start,
        length: payloads.len(),
        known_variant: *variant,
        presence: None,
    }))
}

impl<'a, 'source> ProductionSourceSsaEndpointV36<'a, 'source> {
    fn enum_source_shape_v47(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<(SemanticTypeIdV1, usize)> {
        budget.charge_work(4)?;
        let types = self
            .owner
            .source
            .owner
            .inner
            .source
            .owner
            .source_semantic()
            .types();
        let declaration = types.get(self.source_type.index() as usize).ok_or(
            ProductionSourceOwnedViewErrorV18::Binding("original enum source type is absent"),
        )?;
        let SemanticTypeShapeV1::Enum {
            discriminant,
            variants,
        } = declaration.shape()
        else {
            return self
                .owner
                .source
                .missing("original enum carrier source shape differs");
        };
        Ok((*discriminant, variants.len()))
    }

    fn enum_carrier_endpoint_v47(
        &self,
        at: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionSourceSsaEndpointV36<'a, 'source>> {
        budget.charge_work(2)?;
        let row = self
            .carriers
            .get(at)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "original enum carrier component is absent",
            ))?;
        let definition = source_carrier_definition_v37(
            self.owner,
            self.coordinate,
            &row.physical,
            self.carriers,
            budget,
        )?;
        Ok(ProductionSourceSsaEndpointV36 {
            owner: self.owner,
            row: self.row,
            function: self.function,
            coordinate: self.coordinate,
            source_type: row.ty,
            physical: &row.physical,
            carriers: self.carriers,
            definition,
        })
    }

    fn enum_variant_row_v47(
        &self,
        selected: u32,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<(usize, usize)>> {
        budget.charge_work(5)?;
        let SourceSsaPhysicalV36::Enum { start, length, .. } = *self.physical else {
            return self
                .owner
                .source
                .missing("SSA carrier is not an original enum");
        };
        let (_, variants) = self.enum_source_shape_v47(budget)?;
        if selected as usize >= variants {
            return self
                .owner
                .source
                .missing("original enum variant is out of range");
        }
        let end = start
            .checked_add(length)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        let rows =
            self.carriers
                .get(start..end)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "original enum variant roster differs",
                ))?;
        let mut lo = 0;
        let mut hi = rows.len();
        while lo < hi {
            budget.charge_work(5)?;
            let middle = lo + (hi - lo) / 2;
            let row = &rows[middle];
            let SourceSsaPhysicalV36::EnumVariant {
                variant,
                start,
                length,
            } = row.physical
            else {
                return self
                    .owner
                    .source
                    .missing("original enum variant row differs");
            };
            if row.ty != self.source_type {
                return self
                    .owner
                    .source
                    .missing("original enum variant nominal type differs");
            }
            match variant.cmp(&selected) {
                std::cmp::Ordering::Less => lo = middle + 1,
                std::cmp::Ordering::Greater => hi = middle,
                std::cmp::Ordering::Equal => {
                    let end = start
                        .checked_add(length)
                        .ok_or(ArgumentResourceV1::Arithmetic)?;
                    if self.carriers.get(start..end).is_none() {
                        return self
                            .owner
                            .source
                            .missing("original enum field roster differs");
                    }
                    return Ok(Some((start, length)));
                }
            }
        }
        Ok(None)
    }

    /// Borrows the exact logical discriminant definition of this original enum.
    /// This is not the storage tag encoding and does not validate an active variant.
    pub fn enum_discriminant_v47(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionSourceSsaEndpointV36<'a, 'source>> {
        self.owner.retain_query((|| {
            self.owner.query(budget)?;
            budget.charge_work(1)?;
            let SourceSsaPhysicalV36::Enum {
                discriminant,
                presence: None,
                ..
            } = *self.physical
            else {
                return self
                    .owner
                    .source
                    .missing("SSA carrier has no enum discriminant");
            };
            let (expected, _) = self.enum_source_shape_v47(budget)?;
            if self.carriers.get(discriminant).map(|row| row.ty) != Some(expected) {
                return self
                    .owner
                    .source
                    .missing("original enum discriminant type differs");
            }
            self.enum_carrier_endpoint_v47(discriminant, budget)
        })())
    }

    /// Returns the number of retained fields for an original variant ordinal.
    /// `None` means no physical payload roster, not that this variant is impossible.
    pub fn enum_variant_fields_v47(
        &self,
        variant: u32,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<usize>> {
        self.owner.retain_query((|| {
            self.owner.query(budget)?;
            Ok(self
                .enum_variant_row_v47(variant, budget)?
                .map(|(_, length)| length))
        })())
    }

    /// Borrows one field from the exact original variant roster. Inactive or
    /// undefined source payloads require separate checks by the semantic consumer.
    pub fn enum_field_v47(
        &self,
        variant: u32,
        field: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionSourceSsaEndpointV36<'a, 'source>> {
        self.owner.retain_query((|| {
            self.owner.query(budget)?;
            let Some((start, length)) = self.enum_variant_row_v47(variant, budget)? else {
                return self
                    .owner
                    .source
                    .missing("original enum payload is not retained");
            };
            if field >= length {
                return self
                    .owner
                    .source
                    .missing("original enum field is out of range");
            }
            let at = start
                .checked_add(field)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            self.enum_carrier_endpoint_v47(at, budget)
        })())
    }

    /// Borrows a retained field carrier when the immutable emitted binding has
    /// one. `None` means an unmodeled physical carrier, never source undefinedness
    /// or permission to omit an independently defined source payload.
    pub fn enum_field_carrier_v49(
        &self,
        variant: u32,
        field: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<ProductionSourceSsaEndpointV36<'a, 'source>>> {
        self.owner.retain_query((|| {
            self.owner.query(budget)?;
            let Some((start, length)) = self.enum_variant_row_v47(variant, budget)? else {
                return self
                    .owner
                    .source
                    .missing("original enum payload is not retained");
            };
            if field >= length {
                return self
                    .owner
                    .source
                    .missing("original enum field is out of range");
            }
            let at = start
                .checked_add(field)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            budget.charge_work(1)?;
            let row = self
                .carriers
                .get(at)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "original enum carrier component is absent",
                ))?;
            if matches!(row.physical, SourceSsaPhysicalV36::Unmodeled) {
                Ok(None)
            } else {
                self.enum_carrier_endpoint_v47(at, budget).map(Some)
            }
        })())
    }
}

fn source_enum_carrier_headers_v47() -> Result<usize, ArgumentResourceV1> {
    fn h<T>() -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            size_of::<T>(),
            argument_product_v1(2, size_of::<Result<T, ProductionSemanticKirErrorV1>>())?,
            argument_product_v1(2, size_of::<SourceOwnedResultV18<T>>())?,
        ])
    }
    argument_sum_v1(&[
        h::<Option<SourceSsaPhysicalV36>>()?,
        h::<Option<(usize, usize)>>()?,
        h::<Option<SemanticOptionAvailabilityV1>>()?,
        h::<(
            &ExecutionInstancesV29<'_>,
            SemanticTypeIdV1,
            &SemanticValueBindingV1,
            &mut Vec<SourceSsaComponentV37>,
            &mut ArgumentBudgetV1<'_>,
        )>()?,
        h::<Option<ProductionSourceSsaEndpointV36<'_, '_>>>()?,
        h::<(SemanticTypeIdV1, usize)>()?,
        h::<std::cmp::Ordering>()?,
        h::<std::collections::btree_map::Iter<'_, u32, Vec<SemanticValueBindingV1>>>()?,
        h::<
            std::iter::Enumerate<
                std::collections::btree_map::Iter<'_, u32, Vec<SemanticValueBindingV1>>,
            >,
        >()?,
        h::<std::slice::Iter<'_, SemanticTypeIdV1>>()?,
        h::<(
            &ExecutionInstancesV29<'_>,
            SemanticTypeIdV1,
            &SemanticValueBindingV1,
            &mut Vec<SourceSsaComponentV37>,
            &mut Vec<SourceCarrierFrameV37<'_>>,
            &mut ArgumentBudgetV1<'_>,
        )>()?,
        argument_product_v1(16, h::<usize>()?)?,
    ])
}
