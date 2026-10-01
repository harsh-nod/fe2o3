/// Which authenticated schema of one original endpoint is being inspected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSourceObjectSchemaV42 {
    /// The endpoint's original complete object schema.
    Root,
    /// Its exact source-projected schema at this operation.
    Projected,
}

/// Borrowed representation selection, not reference validity or access authority.
/// The encoded space comes from original Rust layout; the value space comes
/// from the same C1-selected endpoint used by original object emission.
pub struct ProductionSourceSelectedPointerNicheV42<'view, 'source> {
    owner: &'view ProductionSourceCorrespondenceV18<'source>,
    endpoint: &'view ScopedObjectEndpointV29,
    coordinate: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    ordinal: usize,
    component: ProductionSourceObjectSchemaV42,
    source_type: SemanticTypeIdV1,
    terminal_type: SemanticTypeIdV1,
    schema: fe2o3_kernel_ir::StorageLayoutIdV1,
    pointer: fe2o3_kernel_ir::StoragePointerV1,
    required: usize,
}

fn selected_pointer_niche_error_v42() -> ProductionSourceOwnedViewErrorV18 {
    ProductionSourceOwnedViewErrorV18::Binding(
        "selected pointer niche differs from its original endpoint",
    )
}

fn selected_pointer_niche_headers_v42() -> Result<usize, ArgumentResourceV1> {
    fn h<T>() -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            size_of::<T>(),
            argument_product_v1(2, size_of::<SourceOwnedResultV18<T>>())?,
        ])
    }
    argument_sum_v1(&[
        h::<ProductionSourceSelectedPointerNicheV42<'_, '_>>()?,
        h::<Option<ProductionSourceSelectedPointerNicheV42<'_, '_>>>()?,
        h::<ProductionSourceObjectEndpointV39<'_, '_>>()?,
        h::<ProductionSourceObjectSchemaV42>()?,
        h::<fe2o3_kernel_ir::StoragePointerV1>()?,
        h::<(
            SemanticTypeIdV1,
            SemanticTypeIdV1,
            fe2o3_kernel_ir::StorageLayoutIdV1,
        )>()?,
        h::<Option<(SemanticTypeIdV1, fe2o3_kernel_ir::StoragePointerV1)>>()?,
        h::<&SemanticTypeDeclV1>()?,
        h::<&fe2o3_kernel_ir::StorageLayoutV1>()?,
        h::<&[SemanticTypeDeclV1]>()?,
        h::<&[fe2o3_kernel_ir::StorageLayoutV1]>()?,
        h::<&[SemanticTypeIdV1]>()?,
        h::<&[u64]>()?,
        h::<std::slice::Iter<'_, fe2o3_mir_model::semantic_mir_v1::SemanticNichePathComponentV1>>(
        )?,
        h::<(
            Option<SemanticTypeIdV1>,
            fe2o3_kernel_ir::StorageLayoutIdV1,
            u64,
        )>()?,
        h::<(SemanticTypeIdV1, fe2o3_kernel_ir::StorageLayoutIdV1, u64)>()?,
        argument_product_v1(12, h::<usize>()?)?,
    ])
}

fn selected_pointer_niche_records_v42(
    types: &[SemanticTypeDeclV1],
    rows: &[fe2o3_kernel_ir::StorageLayoutV1],
    source_type: SemanticTypeIdV1,
    schema: fe2o3_kernel_ir::StorageLayoutIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Option<(SemanticTypeIdV1, fe2o3_kernel_ir::StoragePointerV1)>> {
    use fe2o3_kernel_ir::{StorageLayoutKindV1 as Kind, StorageVariantEncodingV1 as Encoding};
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticNichePathComponentV1 as Part, SemanticRustcVariantsV1 as Variants,
    };
    budget.charge_work(12)?;
    let source = types
        .get(source_type.index() as usize)
        .ok_or_else(selected_pointer_niche_error_v42)?;
    let row = rows
        .get(schema.0 as usize)
        .ok_or_else(selected_pointer_niche_error_v42)?;
    let (SemanticTypeShapeV1::Enum { variants, .. }, Variants::Multiple(layout)) =
        (source.shape(), source.layout().variants())
    else {
        return Ok(None);
    };
    let fe2o3_mir_model::semantic_mir_v1::SemanticEnumEncodingV1::Niche(niche) = layout.encoding()
    else {
        return Ok(None);
    };
    let fe2o3_mir_model::semantic_mir_v1::SemanticBackendPrimitiveV1::Pointer {
        address_space,
        size_bytes,
        alignment_bytes,
    } = niche.tag().primitive()
    else {
        return Ok(None);
    };
    let Kind::Variants {
        encoding,
        variants: actual,
    } = &row.kind
    else {
        return Err(selected_pointer_niche_error_v42());
    };
    let Encoding::Niche {
        tag,
        untagged_variant,
        first_niche_variant,
        last_niche_variant,
        niche_start,
    } = *encoding
    else {
        return Err(selected_pointer_niche_error_v42());
    };
    if source.layout().size_bytes() != Some(row.size)
        || source.layout().alignment_bytes() != u64::from(row.alignment)
        || actual.len() != variants.len()
        || actual.len() != layout.variants().len()
        || untagged_variant != niche.untagged_variant()
        || (first_niche_variant, last_niche_variant) != niche.niche_variant_range()
        || niche_start != niche.niche_start()
        || tag.offset != niche.source().expected_offset_bytes()
    {
        return Err(selected_pointer_niche_error_v42());
    }
    let source_variant = variants
        .get(untagged_variant as usize)
        .ok_or_else(selected_pointer_niche_error_v42)?;
    let source_payload = layout
        .variants()
        .get(untagged_variant as usize)
        .ok_or_else(selected_pointer_niche_error_v42)?;
    let mut schema = actual
        .get(untagged_variant as usize)
        .ok_or_else(selected_pointer_niche_error_v42)?
        .layout;
    let payload = rows
        .get(schema.0 as usize)
        .ok_or_else(selected_pointer_niche_error_v42)?;
    if payload.size != source_payload.rustc_size_bytes()
        || u64::from(payload.alignment) != source_payload.alignment_bytes()
    {
        return Err(selected_pointer_niche_error_v42());
    }
    let mut terminal: Option<SemanticTypeIdV1> = None;
    let mut offset = 0_u64;
    for part in niche.source().path() {
        budget.charge_work(8)?;
        let row = rows
            .get(schema.0 as usize)
            .ok_or_else(selected_pointer_niche_error_v42)?;
        let (next_type, next_schema, relative) = match part {
            Part::Field(index) => {
                let (fields, offsets) = match terminal {
                    None => (
                        source_variant.fields().fields(),
                        source_payload.aggregate().field_offsets(),
                    ),
                    Some(ty) => {
                        let declaration = types
                            .get(ty.index() as usize)
                            .ok_or_else(selected_pointer_niche_error_v42)?;
                        let (
                            SemanticTypeShapeV1::Tuple(fields)
                            | SemanticTypeShapeV1::Aggregate(fields),
                            SemanticTypeLayoutDetailsV1::Aggregate(layout),
                        ) = (declaration.shape(), declaration.layout().details())
                        else {
                            return Err(selected_pointer_niche_error_v42());
                        };
                        (fields.fields(), layout.field_offsets())
                    }
                };
                let next = *fields
                    .get(*index as usize)
                    .ok_or_else(selected_pointer_niche_error_v42)?;
                let expected = *offsets
                    .get(*index as usize)
                    .ok_or_else(selected_pointer_niche_error_v42)?;
                let mut physical = 0usize;
                for (ordinal, &field) in fields.iter().enumerate().take(*index as usize + 1) {
                    budget.charge_work(3)?;
                    let nominal = execution_cfg_nominal_kind_v29(types, field)
                        .map_err(|error| source_attachment_error_v18(error.into()))?
                        .is_some();
                    if ordinal == *index as usize {
                        if nominal {
                            return Err(selected_pointer_niche_error_v42());
                        }
                    } else if !nominal {
                        physical = physical
                            .checked_add(1)
                            .ok_or(ArgumentResourceV1::Arithmetic)?;
                    }
                }
                let Kind::Record(fields) = &row.kind else {
                    return Err(selected_pointer_niche_error_v42());
                };
                let field = fields
                    .get(physical)
                    .ok_or_else(selected_pointer_niche_error_v42)?;
                if field.offset != expected {
                    return Err(selected_pointer_niche_error_v42());
                }
                (next, field.layout, expected)
            }
            Part::ArrayElement(index) => {
                let ty = terminal.ok_or_else(selected_pointer_niche_error_v42)?;
                let declaration = types
                    .get(ty.index() as usize)
                    .ok_or_else(selected_pointer_niche_error_v42)?;
                let (
                    SemanticTypeShapeV1::Array { element, length },
                    fe2o3_mir_model::semantic_mir_v1::SemanticFieldsShapeV1::Array {
                        stride_bytes,
                        count,
                    },
                    Kind::Array {
                        element: actual,
                        length: actual_length,
                        stride,
                    },
                ) = (
                    declaration.shape(),
                    declaration.layout().fields(),
                    &row.kind,
                )
                else {
                    return Err(selected_pointer_niche_error_v42());
                };
                if index >= length
                    || count != length
                    || actual_length != length
                    || stride != stride_bytes
                {
                    return Err(selected_pointer_niche_error_v42());
                }
                (
                    *element,
                    *actual,
                    index
                        .checked_mul(*stride_bytes)
                        .ok_or(ArgumentResourceV1::Arithmetic)?,
                )
            }
        };
        let declaration = types
            .get(next_type.index() as usize)
            .ok_or_else(selected_pointer_niche_error_v42)?;
        let row = rows
            .get(next_schema.0 as usize)
            .ok_or_else(selected_pointer_niche_error_v42)?;
        if declaration.layout().size_bytes() != Some(row.size)
            || declaration.layout().alignment_bytes() != u64::from(row.alignment)
        {
            return Err(selected_pointer_niche_error_v42());
        }
        offset = offset
            .checked_add(relative)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        terminal = Some(next_type);
        schema = next_schema;
    }
    budget.charge_work(12)?;
    let terminal = terminal.ok_or_else(selected_pointer_niche_error_v42)?;
    let declaration = types
        .get(terminal.index() as usize)
        .ok_or_else(selected_pointer_niche_error_v42)?;
    let SemanticTypeShapeV1::Pointer(source_pointer) = declaration.shape() else {
        return Err(selected_pointer_niche_error_v42());
    };
    let row = rows
        .get(schema.0 as usize)
        .ok_or_else(selected_pointer_niche_error_v42)?;
    let Kind::Pointer(pointer) = row.kind else {
        return Err(selected_pointer_niche_error_v42());
    };
    let expected_space = match address_space {
        0 => AddressSpace::Generic,
        1 => AddressSpace::Global,
        3 => AddressSpace::Workgroup,
        4 => AddressSpace::Constant,
        5 => AddressSpace::Private,
        _ => return Err(selected_pointer_niche_error_v42()),
    };
    let expected_access = match source_pointer.mutability() {
        SemanticMutabilityV1::Immutable => AccessMode::ReadOnly,
        SemanticMutabilityV1::Mutable => AccessMode::ReadWrite,
    };
    let tag_row = rows
        .get(tag.layout.0 as usize)
        .ok_or_else(selected_pointer_niche_error_v42)?;
    if offset != tag.offset
        || source_pointer.metadata() != SemanticPointerMetadataV1::None
        || source_pointer.address_space() != address_space
        || source_pointer.pointer_width_bits() != pointer.stored_bits
        || size_bytes.checked_mul(8) != Some(u64::from(pointer.stored_bits))
        || row.size != size_bytes
        || u64::from(row.alignment) != alignment_bytes
        || pointer.encoded_space != expected_space
        || pointer.access != expected_access
        || (expected_space != AddressSpace::Generic && pointer.value_space != expected_space)
        || (pointer.value_space == AddressSpace::Constant && pointer.access != AccessMode::ReadOnly)
        || tag_row != row
    {
        return Err(selected_pointer_niche_error_v42());
    }
    Ok(Some((terminal, pointer)))
}

impl<'view, 'source> ProductionSourceObjectRecipeV39<'view, 'source> {
    /// Inspects the selected pointer niche of this exact endpoint. Unsupported
    /// non-pointer encodings return None; mismatched paths/roles poison the owner.
    pub fn selected_pointer_niche_v42(
        &self,
        ordinal: usize,
        component: ProductionSourceObjectSchemaV42,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<ProductionSourceSelectedPointerNicheV42<'view, 'source>>> {
        self.owner.retain_query((|| {
            let endpoint = self.endpoint(ordinal, budget)?;
            budget.reserve_storage(selected_pointer_niche_headers_v42()?)?;
            let (source_type, schema) = match component {
                ProductionSourceObjectSchemaV42::Root => {
                    (endpoint.endpoint.root_type, endpoint.endpoint.root_schema)
                }
                ProductionSourceObjectSchemaV42::Projected => (
                    endpoint.endpoint.projected_type,
                    endpoint.endpoint.projected_schema,
                ),
            };
            let semantic = self.owner.source.source_semantic(budget)?;
            let rows = &self.owner.source.owner.pending_module().storage_layouts;
            let Some((terminal_type, pointer)) = selected_pointer_niche_records_v42(
                semantic.types(),
                rows,
                source_type,
                schema,
                budget,
            )?
            else {
                return Ok(None);
            };
            Ok(Some(ProductionSourceSelectedPointerNicheV42 {
                owner: self.owner,
                endpoint: endpoint.endpoint,
                coordinate: self.coordinate,
                ordinal,
                component,
                source_type,
                terminal_type,
                schema,
                pointer,
                required: budget.storage(),
            }))
        })())
    }
}

impl ProductionSourceSelectedPointerNicheV42<'_, '_> {
    /// Requires the exact recipe, endpoint and schema role, not an equal row.
    pub fn check_binding(
        &self,
        recipe: &ProductionSourceObjectRecipeV39<'_, '_>,
        ordinal: usize,
        component: ProductionSourceObjectSchemaV42,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        self.owner.retain_query((|| {
            source_object_endpoint_check_v39(self.owner, self.required, budget)?;
            if !std::ptr::eq(self.owner, recipe.owner)
                || self.coordinate != recipe.coordinate
                || self.ordinal != ordinal
                || self.component != component
            {
                return Err(selected_pointer_niche_error_v42());
            }
            let endpoint = recipe.endpoint(ordinal, budget)?;
            if !std::ptr::eq(self.endpoint, endpoint.endpoint) {
                return Err(selected_pointer_niche_error_v42());
            }
            Ok(())
        })())
    }

    /// Original enum/terminal types and the authenticated selected enum schema.
    pub fn types_and_schema(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<(
        SemanticTypeIdV1,
        SemanticTypeIdV1,
        fe2o3_kernel_ir::StorageLayoutIdV1,
    )> {
        source_object_endpoint_check_v39(self.owner, self.required, budget)?;
        Ok((self.source_type, self.terminal_type, self.schema))
    }

    /// Exact selected pointer representation, without dereference authority.
    pub fn pointer(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<fe2o3_kernel_ir::StoragePointerV1> {
        source_object_endpoint_check_v39(self.owner, self.required, budget)?;
        Ok(self.pointer)
    }
}
