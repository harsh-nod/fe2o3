// A physical view of an exact original source path, never a currentness proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SourceSelectedComponentKindV29 {
    Field {
        original: u32,
        physical: usize,
        byte_offset: u64,
    },
    Variant {
        original: u32,
        physical: bool,
    },
    Index {
        length: u64,
        stride: u64,
    },
    OmittedNominal {
        original: u32,
    },
}

#[derive(Clone, Copy, Debug)]
pub(super) struct SourceSelectedComponentV29<'path> {
    pub(super) projection: &'path SemanticProjectionV1,
    pub(super) source_type: SemanticTypeIdV1,
    pub(super) source_schema: StorageLayoutIdV1,
    pub(super) result_type: SemanticTypeIdV1,
    pub(super) result_schema: Option<StorageLayoutIdV1>,
    pub(super) kind: SourceSelectedComponentKindV29,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SourceSelectedProjectionV29 {
    Physical {
        ty: SemanticTypeIdV1,
        schema: StorageLayoutIdV1,
    },
    Payload {
        ty: SemanticTypeIdV1,
        schema: StorageLayoutIdV1,
        variant: u32,
    },
    OmittedNominal {
        ty: SemanticTypeIdV1,
        position: usize,
    },
}

fn source_selected_projection_headers_v29() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        argument_product_v1(2, size_of::<SourceSelectedProjectionV29>())?,
        argument_product_v1(2, size_of::<Result<SourceSelectedProjectionV29, Error>>())?,
        size_of::<SourceSelectedComponentV29<'_>>(),
        size_of::<Result<(), Error>>(),
    ])
}

fn source_selected_strict_headers_v29() -> Result<usize, ArgumentResourceV1> {
    // The old tuple result and its caller envelope already existed. Only the
    // additional shared-walk result and transient callback values overlap here.
    argument_sum_v1(&[
        size_of::<SourceSelectedProjectionV29>(),
        size_of::<Result<SourceSelectedProjectionV29, Error>>(),
        size_of::<SourceSelectedComponentV29<'_>>(),
        size_of::<Result<(), Error>>(),
        size_of::<std::thread::Result<Result<(SemanticTypeIdV1, StorageLayoutIdV1), Error>>>(),
        size_of::<Result<(), Error>>(),
    ])
}

impl SourceStorageLayoutsV29<'_> {
    fn selected_niche_work(
        &self,
        components: &mut usize,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        self.lease.work(1, budget)?;
        *components = components
            .checked_add(1)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        if *components > MAX_SSA_VALUE_COMPONENTS_V1 {
            return Err(error("selected niche exceeds the source component policy"));
        }
        Ok(())
    }

    fn selected_niche_descend(
        &self,
        physical: &SourceStoragePhysicalV29,
        parent: Option<StorageLayoutIdV1>,
        child: StorageLayoutIdV1,
        depth: &mut usize,
        components: &mut usize,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        self.selected_niche_work(components, budget)?;
        *depth = depth.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
        let child_height = *physical
            .depths
            .get(child.0 as usize)
            .ok_or_else(|| error("selected niche child has no closed containment metric"))?;
        let parent_height = match parent {
            Some(parent) => *physical
                .depths
                .get(parent.0 as usize)
                .ok_or_else(|| error("selected niche parent has no closed containment metric"))?,
            None => self.limits.containment_depth,
        };
        // Strictly decreasing closed heights rule out repeated physical nodes
        // and therefore repeated source/schema pairs without a second census.
        if child_height == 0
            || child_height >= parent_height
            || *depth > self.limits.containment_depth
        {
            return Err(error(
                "selected niche containment is cyclic or exceeds its depth policy",
            ));
        }
        Ok(())
    }

    fn selected_niche_pointer_row(
        &self,
        mut ty: SemanticTypeIdV1,
        selected: &[(u32, StorageLayoutIdV1)],
        budget: &mut Budget<'_>,
    ) -> Result<StorageLayoutV1, Error> {
        self.lease.check(budget)?;
        let physical = self
            .physical
            .try_borrow()
            .map_err(|_| error("selected niche projection has an active schema builder"))?;
        let mut parent: Option<StorageLayoutIdV1> = None;
        let mut depth = 1_usize;
        let mut components = 0_usize;
        let mut original_primitive = None;
        loop {
            self.selected_niche_work(&mut components, budget)?;
            let (declaration, variants) = self.enum_parts(ty)?;
            let SemanticRustcVariantsV1::Multiple(enumeration) = declaration.layout().variants()
            else {
                return Err(error(
                    "selected niche has no original multiple-variant encoding",
                ));
            };
            let SemanticEnumEncodingV1::Niche(niche) = enumeration.encoding() else {
                return Err(error("selected niche changes its original enum encoding"));
            };
            let primitive = niche.tag().primitive();
            if !matches!(primitive, SemanticBackendPrimitiveV1::Pointer { .. })
                || original_primitive.is_some_and(|original| original != primitive)
                || niche.source_niche().primitive() != primitive
            {
                return Err(error(
                    "selected niche changes its original encoded primitive",
                ));
            }
            original_primitive = Some(primitive);
            let variant = niche.untagged_variant();
            let original = variants
                .get(variant as usize)
                .ok_or_else(|| error("selected niche has no original untagged variant"))?;
            let original_payload = self.enum_variant_layout(ty, variant)?;
            let mut id = if let Some(parent) = parent {
                let row = physical
                    .rows
                    .get(parent.0 as usize)
                    .ok_or_else(|| error("selected nested niche row is absent"))?;
                let StorageLayoutKindV1::Variants {
                    encoding,
                    variants: actual,
                } = &row.kind
                else {
                    return Err(error("selected nested niche has no physical variant table"));
                };
                let (first, last) = niche.niche_variant_range();
                if !matches!(*encoding, StorageVariantEncodingV1::Niche {
                    tag, untagged_variant, first_niche_variant, last_niche_variant, niche_start
                } if tag.offset == niche.source().expected_offset_bytes()
                    && untagged_variant == variant && first_niche_variant == first
                    && last_niche_variant == last && niche_start == niche.niche_start())
                    || actual.len() != variants.len()
                    || row.size
                        != declaration
                            .layout()
                            .size_bytes()
                            .ok_or(ArgumentResourceV1::Arithmetic)?
                    || row.alignment != physical_alignment(declaration.layout().alignment_bytes())?
                {
                    return Err(error(
                        "selected nested niche changes original variant geometry",
                    ));
                }
                actual
                    .get(variant as usize)
                    .ok_or_else(|| error("selected nested niche payload is absent"))?
                    .layout
            } else {
                let &(index, id) = selected.get(variant as usize).ok_or_else(|| {
                    error("selected niche payload is absent from the original roster")
                })?;
                if selected.len() != variants.len() || index != variant {
                    return Err(error("selected niche reorders the original payload roster"));
                }
                id
            };
            self.selected_child(id, ty, RowRole::Payload(variant), budget)?;
            self.selected_niche_descend(
                &physical,
                parent,
                id,
                &mut depth,
                &mut components,
                budget,
            )?;
            let payload = physical
                .rows
                .get(id.0 as usize)
                .ok_or_else(|| error("selected niche payload row is absent"))?;
            if payload.size != original_payload.rustc_size_bytes()
                || payload.alignment != physical_alignment(original_payload.alignment_bytes())?
            {
                return Err(error("selected niche payload changes original geometry"));
            }
            let mut current = None;
            let mut offset = 0_u64;
            for part in niche.source().path() {
                self.selected_niche_work(&mut components, budget)?;
                let row = physical
                    .rows
                    .get(id.0 as usize)
                    .ok_or_else(|| error("selected niche path row is absent"))?;
                let (next_ty, next_id, relative) = match part {
                    SemanticNichePathComponentV1::Field(index) => {
                        let (fields, offsets) = match current {
                            None => (
                                original.fields().fields(),
                                original_payload.aggregate().field_offsets(),
                            ),
                            Some(ty) => {
                                let declaration = self.declaration(ty)?;
                                let (
                                    SemanticTypeShapeV1::Tuple(fields)
                                    | SemanticTypeShapeV1::Aggregate(fields),
                                    SemanticTypeLayoutDetailsV1::Aggregate(layout),
                                ) = (declaration.shape(), declaration.layout().details())
                                else {
                                    return Err(error(
                                        "selected niche field changes the original aggregate",
                                    ));
                                };
                                (fields.fields(), layout.field_offsets())
                            }
                        };
                        let next_ty = *fields.get(*index as usize).ok_or_else(|| {
                            error("selected niche field is outside the original roster")
                        })?;
                        let expected = *offsets.get(*index as usize).ok_or_else(|| {
                            error("selected niche field lacks an original offset")
                        })?;
                        let mut ordinal = 0;
                        for (at, &field) in fields.iter().enumerate().take(*index as usize + 1) {
                            self.selected_niche_work(&mut components, budget)?;
                            let nominal = execution_cfg_nominal_kind_v29(
                                self.owner.source_semantic().types(),
                                field,
                            )?
                            .is_some();
                            if at == *index as usize {
                                if nominal {
                                    return Err(error(
                                        "selected niche cannot project a source-nominal field",
                                    ));
                                }
                            } else if !nominal {
                                ordinal += 1;
                            }
                        }
                        let StorageLayoutKindV1::Record(actual) = &row.kind else {
                            return Err(error("selected niche field lacks its physical record"));
                        };
                        let field = actual.get(ordinal).ok_or_else(|| {
                            error("selected niche field lacks its physical component")
                        })?;
                        if field.offset != expected {
                            return Err(error("selected niche field changes its original offset"));
                        }
                        (next_ty, field.layout, expected)
                    }
                    SemanticNichePathComponentV1::ArrayElement(index) => {
                        let declaration = self.declaration(current.ok_or_else(|| {
                            error("selected niche path lacks its original field root")
                        })?)?;
                        let (
                            SemanticTypeShapeV1::Array { element, length },
                            SemanticFieldsShapeV1::Array {
                                stride_bytes,
                                count,
                            },
                            StorageLayoutKindV1::Array {
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
                            return Err(error("selected niche index changes its original array"));
                        };
                        if index >= length
                            || count != length
                            || actual_length != length
                            || stride != stride_bytes
                        {
                            return Err(error(
                                "selected niche index changes its original bounds or stride",
                            ));
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
                self.selected_child(next_id, next_ty, RowRole::Type, budget)?;
                self.selected_niche_descend(
                    &physical,
                    Some(id),
                    next_id,
                    &mut depth,
                    &mut components,
                    budget,
                )?;
                let child = physical
                    .rows
                    .get(next_id.0 as usize)
                    .ok_or_else(|| error("selected niche projected row is absent"))?;
                let original_child = self.declaration(next_ty)?;
                if child.size
                    != original_child
                        .layout()
                        .size_bytes()
                        .ok_or(ArgumentResourceV1::Arithmetic)?
                    || child.alignment
                        != physical_alignment(original_child.layout().alignment_bytes())?
                {
                    return Err(error(
                        "selected niche projection changes its original child geometry",
                    ));
                }
                offset = offset
                    .checked_add(relative)
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
                current = Some(next_ty);
                id = next_id;
            }
            let terminal =
                current.ok_or_else(|| error("selected niche has an empty original path"))?;
            let relative = niche
                .source()
                .expected_offset_bytes()
                .checked_sub(offset)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            let declaration = self.declaration(terminal)?;
            match declaration.shape() {
                SemanticTypeShapeV1::Pointer(pointer) if relative == 0 => {
                    let row = physical
                        .rows
                        .get(id.0 as usize)
                        .ok_or_else(|| error("selected niche terminal row is absent"))?;
                    let (row, descriptor) = match pointer.metadata() {
                        SemanticPointerMetadataV1::None => (row, false),
                        SemanticPointerMetadataV1::SliceLength => {
                            let StorageLayoutKindV1::Slice {
                                element,
                                value_space,
                                access,
                                data,
                                length,
                            } = row.kind
                            else {
                                return Err(error(
                                    "selected niche slice lacks its actual descriptor",
                                ));
                            };
                            let (_, _, length_offset) = self.descriptor_parts(terminal)?;
                            if data.offset != 0 || length.offset != length_offset {
                                return Err(error(
                                    "selected niche slice changes original component offsets",
                                ));
                            }
                            self.selected_child(
                                data.layout,
                                terminal,
                                RowRole::DescriptorData,
                                budget,
                            )?;
                            self.selected_niche_descend(
                                &physical,
                                Some(id),
                                data.layout,
                                &mut depth,
                                &mut components,
                                budget,
                            )?;
                            let row = physical
                                .rows
                                .get(data.layout.0 as usize)
                                .ok_or_else(|| error("selected niche data row is absent"))?;
                            if !matches!(row.kind, StorageLayoutKindV1::Pointer(actual)
                                if actual.pointee == element && actual.value_space == value_space && actual.access == access)
                            {
                                return Err(error(
                                    "selected niche slice and data representations disagree",
                                ));
                            }
                            (row, true)
                        }
                        _ => {
                            return Err(error(
                                "selected pointer niche lacks an admitted metadata contract",
                            ));
                        }
                    };
                    let StorageLayoutKindV1::Pointer(actual) = row.kind else {
                        return Err(error(
                            "selected niche terminal is not its actual pointer row",
                        ));
                    };
                    if !matches!(primitive, SemanticBackendPrimitiveV1::Pointer { address_space, .. }
                        if address_space == pointer.address_space())
                        || row.size
                            != primitive
                                .size_bytes()
                                .ok_or(ArgumentResourceV1::Arithmetic)?
                        || row.alignment != physical_alignment(primitive.alignment_bytes())?
                    {
                        return Err(error(
                            "selected niche pointer changes its original encoded geometry",
                        ));
                    }
                    let expected = self.selected_pointer(
                        terminal,
                        actual.pointee,
                        actual.value_space,
                        actual.access,
                        descriptor,
                        budget,
                    )?;
                    if row != &expected {
                        return Err(error(
                            "selected niche pointer differs from its exact source representation",
                        ));
                    }
                    return Ok(expected);
                }
                SemanticTypeShapeV1::Enum { .. } => {
                    let nested = declaration.layout().largest_niche().ok_or_else(|| {
                        error("selected nested enum lacks its original source niche")
                    })?;
                    if nested.offset_bytes() != relative || nested.primitive() != primitive {
                        return Err(error(
                            "selected nested enum changes its original niche source",
                        ));
                    }
                    ty = terminal;
                    parent = Some(id);
                }
                _ => {
                    return Err(error(
                        "selected niche is not grounded in its original pointer field",
                    ));
                }
            }
        }
    }

    pub(super) fn project_selected_schema(
        &self,
        owner: &ProductionSemanticSsaOwnerV1,
        ty: SemanticTypeIdV1,
        id: StorageLayoutIdV1,
        projections: &[SemanticProjectionV1],
        budget: &mut Budget<'_>,
    ) -> Result<(SemanticTypeIdV1, StorageLayoutIdV1), Error> {
        self.check_selected_schema(owner, ty, id, budget)?;
        self.with_selected_strict_headers(budget, |budget| {
            match self.walk_selected_components_checked(
                owner,
                ty,
                id,
                projections,
                true,
                budget,
                &mut |_, _| Ok(()),
            )? {
                SourceSelectedProjectionV29::Physical { ty, schema } => Ok((ty, schema)),
                SourceSelectedProjectionV29::Payload { .. }
                | SourceSelectedProjectionV29::OmittedNominal { .. } => {
                    Err(ArgumentResourceV1::Accounting.into())
                }
            }
        })
    }

    fn with_selected_strict_headers(
        &self,
        budget: &mut Budget<'_>,
        body: impl FnOnce(&mut Budget<'_>) -> Result<(SemanticTypeIdV1, StorageLayoutIdV1), Error>,
    ) -> Result<(SemanticTypeIdV1, StorageLayoutIdV1), Error> {
        let headers = source_selected_strict_headers_v29()?;
        self.lease.reserve(headers, budget)?;
        // The closed walk result and callback temporaries die inside this scope;
        // only the pre-existing tuple result escapes to the strict caller.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body(budget)));
        // Lease custody is checked even after a sticky failure. Foreign ledger
        // or slot substitution cannot turn these headers into a refund permit.
        let cleanup = self.lease.refund(headers, budget);
        match result {
            Ok(Ok(value)) => cleanup.map(|()| value),
            Ok(Err(error)) => Err(error),
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    pub(super) fn visit_selected_components<'path>(
        &self,
        owner: &ProductionSemanticSsaOwnerV1,
        ty: SemanticTypeIdV1,
        id: StorageLayoutIdV1,
        projections: &'path [SemanticProjectionV1],
        budget: &mut Budget<'_>,
        mut consume: impl FnMut(SourceSelectedComponentV29<'path>, &mut Budget<'_>) -> Result<(), Error>,
    ) -> Result<SourceSelectedProjectionV29, Error> {
        self.check_owner(owner, budget)?;
        budget
            .reserve_storage(source_selected_projection_headers_v29()?)
            .map_err(Error::from)
            .inspect_err(|error| self.lease.record(error))?;
        // Validate the entire borrowed path before exposing any component.
        // Replay uses the same original owner/table and pays the same traversal.
        let validated = self.walk_selected_components(
            owner,
            ty,
            id,
            projections,
            false,
            budget,
            &mut |_, _| Ok(()),
        )?;
        let actual =
            self.walk_selected_components(owner, ty, id, projections, false, budget, &mut consume)?;
        self.lease.work(1, budget)?;
        if actual != validated {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        Ok(actual)
    }

    fn walk_selected_components<'path>(
        &self,
        owner: &ProductionSemanticSsaOwnerV1,
        ty: SemanticTypeIdV1,
        id: StorageLayoutIdV1,
        projections: &'path [SemanticProjectionV1],
        strict_pointer: bool,
        budget: &mut Budget<'_>,
        consume: &mut impl FnMut(
            SourceSelectedComponentV29<'path>,
            &mut Budget<'_>,
        ) -> Result<(), Error>,
    ) -> Result<SourceSelectedProjectionV29, Error> {
        self.check_selected_schema(owner, ty, id, budget)?;
        self.walk_selected_components_checked(
            owner,
            ty,
            id,
            projections,
            strict_pointer,
            budget,
            consume,
        )
    }

    fn walk_selected_components_checked<'path>(
        &self,
        owner: &ProductionSemanticSsaOwnerV1,
        mut ty: SemanticTypeIdV1,
        mut id: StorageLayoutIdV1,
        projections: &'path [SemanticProjectionV1],
        strict_pointer: bool,
        budget: &mut Budget<'_>,
        consume: &mut impl FnMut(
            SourceSelectedComponentV29<'path>,
            &mut Budget<'_>,
        ) -> Result<(), Error>,
    ) -> Result<SourceSelectedProjectionV29, Error> {
        let physical = self
            .physical
            .try_borrow()
            .map_err(|_| error("selected source projection has an active schema builder"))?;
        let mut variant = None;
        for (position, projection) in projections.iter().enumerate() {
            self.lease.work(3, budget)?;
            let source_type = ty;
            let source_schema = id;
            let declaration = self.declaration(ty)?;
            let row = physical
                .rows
                .get(id.0 as usize)
                .ok_or_else(|| error("selected source projection has a missing row"))?;
            let kind = match projection.kind() {
                SemanticProjectionKindV1::Downcast(index) => {
                    let SemanticTypeShapeV1::Enum { variants, .. } = declaration.shape() else {
                        return Err(error("selected downcast changes original enum type"));
                    };
                    if variant.is_some()
                        || variants.get(index as usize).is_none()
                        || projection.result_type() != ty
                    {
                        return Err(error("selected downcast changes original variant"));
                    }
                    let physical = match declaration.layout().variants() {
                        SemanticRustcVariantsV1::Single { index: original }
                            if *original == index =>
                        {
                            false
                        }
                        SemanticRustcVariantsV1::Multiple(_) => {
                            let StorageLayoutKindV1::Variants { variants, .. } = &row.kind else {
                                return Err(error(
                                    "selected downcast has no physical variant mapping",
                                ));
                            };
                            id = variants
                                .get(index as usize)
                                .ok_or_else(|| {
                                    error("selected downcast has no original payload row")
                                })?
                                .layout;
                            self.selected_child(id, ty, RowRole::Payload(index), budget)?;
                            true
                        }
                        _ => return Err(error("selected downcast changes original encoding")),
                    };
                    variant = Some(index);
                    SourceSelectedComponentKindV29::Variant {
                        original: index,
                        physical,
                    }
                }
                SemanticProjectionKindV1::Field(index) => {
                    let fields = match (declaration.shape(), variant) {
                        (
                            SemanticTypeShapeV1::Tuple(fields)
                            | SemanticTypeShapeV1::Aggregate(fields)
                            | SemanticTypeShapeV1::Union(fields),
                            None,
                        ) => fields.fields(),
                        (SemanticTypeShapeV1::Enum { variants, .. }, Some(variant)) => variants
                            .get(variant as usize)
                            .ok_or_else(|| error("selected field has no original variant"))?
                            .fields()
                            .fields(),
                        _ => return Err(error("selected field changes original aggregate type")),
                    };
                    let field_ty = *fields
                        .get(index as usize)
                        .ok_or_else(|| error("selected field is outside original source roster"))?;
                    let mut ordinal = 0;
                    for (source, &field) in fields.iter().enumerate().take(index as usize + 1) {
                        self.lease.work(1, budget)?;
                        let nominal =
                            execution_cfg_nominal_kind_v29(owner.source_semantic().types(), field)?
                                .is_some();
                        if source == index as usize {
                            if nominal {
                                if strict_pointer {
                                    return Err(error(
                                        "source-nominal field has no physical pointer view",
                                    ));
                                }
                                if position + 1 != projections.len()
                                    || projection.result_type() != field_ty
                                {
                                    return Err(error(
                                        "nominal source projection has an invalid terminal path",
                                    ));
                                }
                                consume(
                                    SourceSelectedComponentV29 {
                                        projection,
                                        source_type,
                                        source_schema,
                                        result_type: field_ty,
                                        result_schema: None,
                                        kind: SourceSelectedComponentKindV29::OmittedNominal {
                                            original: index,
                                        },
                                    },
                                    budget,
                                )?;
                                return Ok(SourceSelectedProjectionV29::OmittedNominal {
                                    ty: field_ty,
                                    position,
                                });
                            }
                            break;
                        }
                        if !nominal {
                            ordinal += 1;
                        }
                    }
                    let fields = match &row.kind {
                        StorageLayoutKindV1::Record(fields)
                        | StorageLayoutKindV1::Union(fields) => fields,
                        _ => return Err(error("selected field has no physical source mapping")),
                    };
                    let field = fields
                        .get(ordinal)
                        .ok_or_else(|| error("selected field has no physical component"))?;
                    id = field.layout;
                    ty = field_ty;
                    variant = None;
                    if projection.result_type() != ty {
                        return Err(error(
                            "selected field result differs from original source type",
                        ));
                    }
                    self.selected_child(id, ty, RowRole::Type, budget)?;
                    SourceSelectedComponentKindV29::Field {
                        original: index,
                        physical: ordinal,
                        byte_offset: field.offset,
                    }
                }
                SemanticProjectionKindV1::Index(_)
                | SemanticProjectionKindV1::ConstantIndex { .. } => {
                    let SemanticTypeShapeV1::Array { element, .. } = declaration.shape() else {
                        return Err(error("selected index changes original array type"));
                    };
                    let StorageLayoutKindV1::Array {
                        element: physical,
                        length,
                        stride,
                    } = &row.kind
                    else {
                        return Err(error("selected index has no physical array mapping"));
                    };
                    if variant.is_some() || projection.result_type() != *element {
                        return Err(error("selected index changes original element type"));
                    }
                    ty = *element;
                    id = *physical;
                    self.selected_child(id, ty, RowRole::Type, budget)?;
                    SourceSelectedComponentKindV29::Index {
                        length: *length,
                        stride: *stride,
                    }
                }
                _ => {
                    return Err(error(
                        "selected source path has no admitted physical projection",
                    ));
                }
            };
            consume(
                SourceSelectedComponentV29 {
                    projection,
                    source_type,
                    source_schema,
                    result_type: ty,
                    result_schema: Some(id),
                    kind,
                },
                budget,
            )?;
        }
        // A downcast is an inert path component, never a whole enum payload
        // pointer ABI. Callers must select its exact original field afterward.
        if let Some(variant) = variant {
            return if strict_pointer {
                Err(error(
                    "selected source path ends at an untyped enum payload",
                ))
            } else {
                Ok(SourceSelectedProjectionV29::Payload {
                    ty,
                    schema: id,
                    variant,
                })
            };
        }
        Ok(SourceSelectedProjectionV29::Physical { ty, schema: id })
    }
}
