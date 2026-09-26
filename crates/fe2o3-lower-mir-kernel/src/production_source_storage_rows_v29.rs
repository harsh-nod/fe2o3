impl<'source> SourceStorageLayoutsV29<'source> {
    fn source_root_subobject<'layout>(
        &'layout self,
        owner: &ProductionSemanticSsaOwnerV1,
        ty: SemanticTypeIdV1,
        budget: &mut Budget<'_>,
    ) -> Result<SourceStorageSubobjectV29<'layout, 'source>, Error> {
        self.check_owner(owner, budget)?;
        let size = self
            .declaration(ty)?
            .layout()
            .size_bytes()
            .ok_or_else(|| error("source logical object has no admitted sized geometry"))?;
        self.make_root_subobject(ty, size, budget)
    }

    fn make_root_subobject<'layout>(
        &'layout self,
        ty: SemanticTypeIdV1,
        size: u64,
        budget: &mut Budget<'_>,
    ) -> Result<SourceStorageSubobjectV29<'layout, 'source>, Error> {
        self.lease
            .reserve(size_of::<SourceStorageSubobjectV29<'_, '_>>(), budget)?;
        Ok(SourceStorageSubobjectV29 {
            layouts: self,
            root: ty,
            ty,
            variant: None,
            initialization_floor: 0,
            selected: false,
            path: Vec::new(),
            range: SourceStorageRangeV29::new(0, size, size)?,
        })
    }

    pub(super) fn root_subobject<'layout>(
        &'layout self,
        owner: &ProductionSemanticSsaOwnerV1,
        ty: SemanticTypeIdV1,
        budget: &mut Budget<'_>,
    ) -> Result<SourceStorageSubobjectV29<'layout, 'source>, Error> {
        self.row_for(owner, ty, budget)?;
        let size = self.declaration(ty)?.layout().size_bytes()
            .ok_or_else(|| error("source storage object has no admitted sized geometry"))?;
        self.make_root_subobject(ty, size, budget)
    }

    fn project_step<'layout>(
        &'layout self,
        parent: &SourceStorageSubobjectV29<'layout, 'source>,
        step: SubobjectStep,
        budget: &mut Budget<'_>,
    ) -> Result<SourceStorageSubobjectV29<'layout, 'source>, Error> {
        self.lease.work(1, budget)?;
        if !std::ptr::eq(self, parent.layouts) {
            return Err(error(
                "source subobject belongs to a different layout owner",
            ));
        }
        let declaration = self.declaration(parent.ty)?;
        let (ty, offset, size, variant) = match (step, declaration.shape()) {
            (
                SubobjectStep::Field(index),
                SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields),
            ) => {
                let SemanticTypeLayoutDetailsV1::Aggregate(layout) = declaration.layout().details()
                else {
                    return Err(error("source subobject field has no exact offset"));
                };
                let ty = *fields
                    .fields()
                    .get(index as usize)
                    .ok_or_else(|| error("source subobject field is absent"))?;
                let offset = *layout
                    .field_offsets()
                    .get(index as usize)
                    .ok_or_else(|| error("source subobject field placement is absent"))?;
                (
                    ty,
                    offset,
                    self.declaration(ty)?
                        .layout()
                        .size_bytes()
                        .ok_or_else(|| error("source subobject field is unsized"))?,
                    None,
                )
            }
            (SubobjectStep::Field(index), SemanticTypeShapeV1::Union(fields)) => {
                let ty = *fields
                    .fields()
                    .get(index as usize)
                    .ok_or_else(|| error("source union field is absent"))?;
                if !matches!(declaration.layout().fields(), SemanticFieldsShapeV1::Union { field_count }
                    if *field_count as usize == fields.fields().len())
                {
                    return Err(error("source union field roster changed"));
                }
                (
                    ty,
                    0,
                    self.declaration(ty)?
                        .layout()
                        .size_bytes()
                        .ok_or_else(|| error("source union field is unsized"))?,
                    None,
                )
            }
            (SubobjectStep::Field(index), SemanticTypeShapeV1::Enum { variants, .. }) => {
                let variant = parent
                    .variant
                    .ok_or_else(|| error("source enum field lacks its variant projection"))?;
                let fields = variants
                    .get(variant as usize)
                    .ok_or_else(|| error("source subobject variant is absent"))?
                    .fields()
                    .fields();
                let offsets = match declaration.layout().variants() {
                    SemanticRustcVariantsV1::Multiple(_) => self
                        .enum_variant_layout(parent.ty, variant)?
                        .aggregate()
                        .field_offsets(),
                    SemanticRustcVariantsV1::Single { index } if *index == variant => {
                        match declaration.layout().details() {
                            SemanticTypeLayoutDetailsV1::Aggregate(layout) => {
                                layout.field_offsets()
                            }
                            _ => return Err(error("single source variant lacks field offsets")),
                        }
                    }
                    _ => return Err(error("source variant projection differs from its layout")),
                };
                let ty = *fields
                    .get(index as usize)
                    .ok_or_else(|| error("source enum field is absent"))?;
                (
                    ty,
                    *offsets
                        .get(index as usize)
                        .ok_or_else(|| error("source enum field offset is absent"))?,
                    self.declaration(ty)?
                        .layout()
                        .size_bytes()
                        .ok_or_else(|| error("source enum field is unsized"))?,
                    None,
                )
            }
            (SubobjectStep::Element(index), SemanticTypeShapeV1::Array { element, length }) => {
                let SemanticFieldsShapeV1::Array {
                    stride_bytes,
                    count,
                } = declaration.layout().fields()
                else {
                    return Err(error("source array projection lacks exact stride"));
                };
                if count != length || index >= *length {
                    return Err(error("source array projection index is out of bounds"));
                }
                (
                    *element,
                    index
                        .checked_mul(*stride_bytes)
                        .ok_or(ArgumentResourceV1::Arithmetic)?,
                    self.declaration(*element)?
                        .layout()
                        .size_bytes()
                        .ok_or_else(|| error("source array element is unsized"))?,
                    None,
                )
            }
            (SubobjectStep::Selection(_), SemanticTypeShapeV1::Array { element, length }) => {
                let SemanticFieldsShapeV1::Array { count, .. } = declaration.layout().fields()
                else {
                    return Err(error("source selected array lacks its original stride"));
                };
                if count != length || *length == 0 {
                    return Err(error(
                        "source selected array has no successful element projection",
                    ));
                }
                (
                    *element,
                    0,
                    self.declaration(*element)?
                        .layout()
                        .size_bytes()
                        .ok_or_else(|| error("source selected array element is unsized"))?,
                    None,
                )
            }
            (SubobjectStep::Variant(index), SemanticTypeShapeV1::Enum { variants, .. }) => {
                variants
                    .get(index as usize)
                    .ok_or_else(|| error("source variant projection is absent"))?;
                let size = match declaration.layout().variants() {
                    SemanticRustcVariantsV1::Multiple(_) => self
                        .enum_variant_layout(parent.ty, index)?
                        .rustc_size_bytes(),
                    SemanticRustcVariantsV1::Single { index: actual } if *actual == index => {
                        parent.range.length()
                    }
                    _ => return Err(error("source variant projection differs from its layout")),
                };
                (parent.ty, 0, size, Some(index))
            }
            _ => {
                return Err(error(
                    "source subobject projection changes the original type shape",
                ));
            }
        };
        if offset
            .checked_add(size)
            .is_none_or(|end| end > parent.range.length())
        {
            return Err(error(
                "source subobject projection exceeds its containing extent",
            ));
        }
        let mut result = parent.copy(budget)?;
        result.selected |= matches!(step, SubobjectStep::Selection(_));
        self.lease.push(&mut result.path, step, budget)?;
        if matches!(declaration.shape(), SemanticTypeShapeV1::Union(_)) {
            result.initialization_floor = result.path.len();
        }
        result.ty = ty;
        result.variant = variant;
        result.range =
            if matches!(step, SubobjectStep::Selection(_)) || parent.has_selection(budget)? {
                // This is a conservative alias envelope, never a concrete address
                // or byte-initialization proof. The logical path keeps field identity.
                parent.range
            } else {
                SourceStorageRangeV29::new(
                    parent
                        .range
                        .start
                        .checked_add(offset)
                        .ok_or(ArgumentResourceV1::Arithmetic)?,
                    size,
                    self.declaration(parent.root)?
                        .layout()
                        .size_bytes()
                        .ok_or(ArgumentResourceV1::Arithmetic)?,
                )?
            };
        Ok(result)
    }

    pub(super) fn projected_subobject<'layout>(
        &'layout self,
        owner: &ProductionSemanticSsaOwnerV1,
        root: SemanticTypeIdV1,
        projections: &[SemanticProjectionV1],
        budget: &mut Budget<'_>,
    ) -> Result<SourceStorageSubobjectV29<'layout, 'source>, Error> {
        let place = self.root_subobject(owner, root, budget)?;
        self.project_subobject(place, projections, budget)
    }

    fn source_projected_subobject<'layout>(
        &'layout self,
        owner: &ProductionSemanticSsaOwnerV1,
        root: SemanticTypeIdV1,
        projections: &[SemanticProjectionV1],
        budget: &mut Budget<'_>,
    ) -> Result<SourceStorageSubobjectV29<'layout, 'source>, Error> {
        let place = self.source_root_subobject(owner, root, budget)?;
        self.project_subobject(place, projections, budget)
    }

    fn project_subobject<'layout>(
        &'layout self,
        mut place: SourceStorageSubobjectV29<'layout, 'source>,
        projections: &[SemanticProjectionV1],
        budget: &mut Budget<'_>,
    ) -> Result<SourceStorageSubobjectV29<'layout, 'source>, Error> {
        for projection in projections {
            self.lease.work(1, budget)?;
            let step = match projection.kind() {
                SemanticProjectionKindV1::Field(index) => SubobjectStep::Field(index),
                SemanticProjectionKindV1::Downcast(index) => SubobjectStep::Variant(index),
                SemanticProjectionKindV1::ConstantIndex {
                    offset,
                    minimum_length,
                    from_end,
                } => {
                    let SemanticTypeShapeV1::Array { length, .. } =
                        self.declaration(place.ty)?.shape()
                    else {
                        return Err(error(
                            "source constant-index subobject needs its runtime slice extent",
                        ));
                    };
                    let invalid_offset = if from_end {
                        offset == 0 || offset > minimum_length
                    } else {
                        offset >= minimum_length
                    };
                    if invalid_offset || minimum_length > *length {
                        return Err(error(
                            "source constant-index subobject exceeds its proven static extent",
                        ));
                    }
                    SubobjectStep::Element(if from_end {
                        length
                            .checked_sub(offset)
                            .ok_or(ArgumentResourceV1::Arithmetic)?
                    } else {
                        offset
                    })
                }
                SemanticProjectionKindV1::Index(_) => {
                    return Err(error(
                        "source indexed storage requires exact current SSA index and runtime extent correspondence",
                    ));
                }
                _ => {
                    return Err(error(
                        "source storage projection requires its original backing-object relation",
                    ));
                }
            };
            let next = self.project_step(&place, step, budget)?;
            if next.ty != projection.result_type() {
                return Err(error("source storage projection result type changed"));
            }
            place.discard(budget)?;
            place = next;
        }
        Ok(place)
    }
}

fn scalar_kind(scalar: SemanticScalarTypeV1) -> Result<ScalarType, Error> {
    match lower_scalar_kind(scalar)? {
        Type::Scalar(scalar) => Ok(scalar),
        _ => Err(error("source storage scalar lowering changed kind")),
    }
}

impl SourceStorageLayoutsV29<'_> {
    // Query a symbolic source data mask without materializing repeated array
    // elements or bytes. Interior array repetitions share one element query.
    fn data_covers(
        &self,
        ty: SemanticTypeIdV1,
        range: SourceStorageRangeV29,
        budget: &mut Budget<'_>,
    ) -> Result<bool, Error> {
        type Task = (SemanticTypeIdV1, SourceStorageRangeV29);
        self.lease.reserve(2 * size_of::<Vec<Task>>(), budget)?;
        let mut tasks = self.lease.vector(1, budget)?;
        let mut visited = self.lease.vector::<Task>(0, budget)?;
        self.lease.push(&mut tasks, (ty, range), budget)?;
        let mut covered = true;
        while let Some((ty, range)) = tasks.pop() {
            self.lease.work(1, budget)?;
            // A by-value type DAG can repeat the same field type many times.
            // Memoize exact relative mask queries rather than expanding that
            // DAG into its exponentially larger physical object tree.
            let key = (ty, range);
            let (mut low, mut high) = (0, visited.len());
            let mut seen = false;
            while low < high {
                self.lease.work(1, budget)?;
                let mid = low + (high - low) / 2;
                match visited[mid].cmp(&key) {
                    std::cmp::Ordering::Less => low = mid + 1,
                    std::cmp::Ordering::Greater => high = mid,
                    std::cmp::Ordering::Equal => {
                        seen = true;
                        break;
                    }
                }
            }
            if seen {
                continue;
            }
            self.lease.push(&mut visited, key, budget)?;
            self.lease.work(visited.len() - 1 - low, budget)?;
            for index in (low + 1..visited.len()).rev() {
                visited.swap(index - 1, index);
            }
            let declaration = self.declaration(ty)?;
            range.check(declaration.layout().size_bytes().ok_or_else(|| {
                error("source byte mask has no admitted sized geometry")
            })?)?;
            if range.start == range.end {
                continue;
            }
            match declaration.shape() {
                SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_) => {}
                SemanticTypeShapeV1::Pointer(pointer) => {
                    if pointer.metadata() == SemanticPointerMetadataV1::SliceLength {
                        let (data, length, length_offset) = self.descriptor_parts(ty)?;
                        let data_end = data.size_bytes().ok_or(ArgumentResourceV1::Arithmetic)?;
                        let length_end = length_offset
                            .checked_add(length.size_bytes().ok_or(ArgumentResourceV1::Arithmetic)?)
                            .ok_or(ArgumentResourceV1::Arithmetic)?;
                        if range.end > length_end
                            || (range.start < length_offset
                                && range.end > data_end
                                && data_end != length_offset)
                        {
                            covered = false;
                        }
                    }
                }
                SemanticTypeShapeV1::Opaque
                    if matches!(declaration.layout().backend_repr(), SemanticBackendReprV1::SimdVector { .. }) => {}
                SemanticTypeShapeV1::Enum { .. } => {
                    // Every valid whole enum has its encoded tag initialized,
                    // but the unknown variant proves no other payload bytes.
                    let SemanticRustcVariantsV1::Multiple(layout) = declaration.layout().variants() else {
                        covered = false;
                        break;
                    };
                    let (offset, primitive) = match layout.encoding() {
                        SemanticEnumEncodingV1::Direct(tag) =>
                            (tag.tag_offset_bytes(), tag.tag().primitive()),
                        SemanticEnumEncodingV1::Niche(tag) =>
                            (tag.source().expected_offset_bytes(), tag.tag().primitive()),
                    };
                    let tag = SourceStorageRangeV29::new(
                        offset,
                        primitive.size_bytes().ok_or(ArgumentResourceV1::Arithmetic)?,
                        declaration.layout().size_bytes().ok_or(ArgumentResourceV1::Arithmetic)?,
                    )?;
                    covered = tag.contains(range);
                }
                SemanticTypeShapeV1::Array { element, .. } => {
                    let SemanticFieldsShapeV1::Array { stride_bytes: stride, .. } = declaration.layout().fields() else {
                        return Err(error("source array mask lacks original stride"));
                    };
                    let stride = *stride;
                    if stride == 0 {
                        return Err(error("nonempty source byte range has zero array stride"));
                    }
                    let first = range.start / stride;
                    let last = (range.end - 1) / stride;
                    let first_range = SourceStorageRangeV29::new(
                        range.start % stride,
                        if first == last {
                            range.length()
                        } else {
                            stride - range.start % stride
                        },
                        stride,
                    )?;
                    self.lease
                        .push(&mut tasks, (*element, first_range), budget)?;
                    if first != last {
                        self.lease.push(
                            &mut tasks,
                            (
                                *element,
                                SourceStorageRangeV29::new(
                                    0,
                                    (range.end - 1) % stride + 1,
                                    stride,
                                )?,
                            ),
                            budget,
                        )?;
                    }
                    if last - first > 1 {
                        self.lease.push(
                            &mut tasks,
                            (*element, SourceStorageRangeV29::new(0, stride, stride)?),
                            budget,
                        )?;
                    }
                }
                SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields) => {
                    let SemanticTypeLayoutDetailsV1::Aggregate(geometry) = declaration.layout().details() else {
                        return Err(error("source record mask lacks original offsets"));
                    };
                    if fields.fields().len() != geometry.field_offsets().len() {
                        return Err(error("source record mask field roster changed"));
                    }
                    self.lease
                        .reserve(size_of::<Vec<SourceStorageRangeV29>>(), budget)?;
                    let mut pieces = self.lease.vector(fields.fields().len(), budget)?;
                    for (&field_ty, &offset) in fields.fields().iter().zip(geometry.field_offsets()) {
                        self.lease.work(1, budget)?;
                        let end = offset
                            .checked_add(self.declaration(field_ty)?.layout().size_bytes()
                                .ok_or_else(|| error("source record mask field is unsized"))?)
                            .ok_or(ArgumentResourceV1::Arithmetic)?;
                        let intersection = SourceStorageRangeV29 {
                            start: range.start.max(offset),
                            end: range.end.min(end),
                        };
                        if intersection.start < intersection.end {
                            self.lease.push(&mut pieces, intersection, budget)?;
                            self.lease.push(
                                &mut tasks,
                                (
                                    field_ty,
                                    SourceStorageRangeV29 {
                                        start: intersection.start - offset,
                                        end: intersection.end - offset,
                                    },
                                ),
                                budget,
                            )?;
                        }
                    }
                    sort_rows(&mut pieces, &self.lease, budget)?;
                    let mut end = range.start;
                    for piece in &pieces {
                        self.lease.work(1, budget)?;
                        if piece.start > end {
                            covered = false;
                            break;
                        }
                        end = end.max(piece.end);
                    }
                    covered &= end == range.end;
                    self.lease.discard_vec(pieces, budget)?;
                    self.lease
                        .refund(size_of::<Vec<SourceStorageRangeV29>>(), budget)?;
                }
                _ => covered = false,
            }
            if !covered {
                break;
            }
        }
        self.lease.discard_vec(tasks, budget)?;
        self.lease.discard_vec(visited, budget)?;
        self.lease.refund(2 * size_of::<Vec<Task>>(), budget)?;
        Ok(covered)
    }
}

fn primitive_scalar(primitive: SemanticBackendPrimitiveV1) -> Result<ScalarType, Error> {
    match primitive {
        SemanticBackendPrimitiveV1::Integer { signed, bits, .. } => {
            scalar_kind(SemanticScalarTypeV1::Integer { signed, bits })
        }
        SemanticBackendPrimitiveV1::Float { bits, .. } => {
            scalar_kind(SemanticScalarTypeV1::Float { bits })
        }
        SemanticBackendPrimitiveV1::Pointer { .. } => {
            Err(error("source storage pointer bits are not a scalar value"))
        }
    }
}

fn physical_alignment(alignment: u64) -> Result<u32, Error> {
    let alignment = u32::try_from(alignment).map_err(|_| ArgumentResourceV1::Arithmetic)?;
    if !alignment.is_power_of_two() {
        return Err(error("source storage alignment is invalid"));
    }
    Ok(alignment)
}

fn exact_box<T>(rows: Vec<T>) -> Result<Box<[T]>, Error> {
    // With equal length/capacity this conversion cannot request replacement
    // backing. Larger allocator capacity must not hide a shrinking allocation.
    if rows.len() != rows.capacity() {
        return Err(ArgumentResourceV1::Allocation.into());
    }
    Ok(rows.into_boxed_slice())
}

impl SourceStorageLayoutsV29<'_> {
    fn pointer_space(&self, address_space: u32) -> Result<AddressSpace, Error> {
        match self.owner.source_semantic().target().architecture() {
            SemanticTargetArchitectureV1::AmdGpuGfx942 => {
                lower_address_space(address_space).map_err(|_| error(
                    "source storage pointer requires an additional admitted target address-space contract",
                ))
            }
        }
    }

    fn pointer_access(pointer: &SemanticPointerTypeV1) -> AccessMode {
        // An inert representation restriction, not ownership or allocation
        // authority. The source access/loan checker still controls every use.
        match pointer.mutability() {
            SemanticMutabilityV1::Immutable => AccessMode::ReadOnly,
            SemanticMutabilityV1::Mutable => AccessMode::ReadWrite,
        }
    }

    fn slice_element(&self, pointer: &SemanticPointerTypeV1) -> Result<SemanticTypeIdV1, Error> {
        match self.declaration(pointer.pointee())?.shape() {
            SemanticTypeShapeV1::Slice { element } => Ok(*element),
            _ => Err(error(
                "source storage slice metadata has a non-slice referent",
            )),
        }
    }

    fn follow_type(
        &mut self,
        ty: SemanticTypeIdV1,
        selected: &mut [bool],
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        self.lease.work(1, budget)?;
        let declaration = self.declaration(ty)?;
        if declaration.layout().size_bytes().is_none() {
            return Err(error(
                "source storage cannot fabricate a sized object for an unsized source type",
            ));
        }
        match declaration.shape() {
            SemanticTypeShapeV1::Unit
            | SemanticTypeShapeV1::Never
            | SemanticTypeShapeV1::Scalar(_)
            | SemanticTypeShapeV1::ValidityScalar(_) => {}
            SemanticTypeShapeV1::Array { element, .. } => {
                self.enqueue(*element, selected, budget)?
            }
            SemanticTypeShapeV1::Tuple(aggregate)
            | SemanticTypeShapeV1::Aggregate(aggregate)
            | SemanticTypeShapeV1::Union(aggregate) => {
                for &field in aggregate.fields() {
                    self.enqueue(field, selected, budget)?;
                }
            }
            SemanticTypeShapeV1::Pointer(pointer) => match pointer.metadata() {
                SemanticPointerMetadataV1::None => {
                    self.enqueue(pointer.pointee(), selected, budget)?
                }
                SemanticPointerMetadataV1::SliceLength => {
                    self.enqueue(self.slice_element(pointer)?, selected, budget)?;
                    self.auxiliary(ty, RowRole::DescriptorData, budget)?;
                    self.auxiliary(ty, RowRole::DescriptorLength, budget)?;
                }
                SemanticPointerMetadataV1::VTable => {
                    return Err(error(
                        "source storage vtable metadata needs its admitted descriptor contract",
                    ));
                }
            },
            SemanticTypeShapeV1::Enum { variants, .. } => match declaration.layout().variants() {
                SemanticRustcVariantsV1::Multiple(_) => {
                    self.auxiliary(ty, RowRole::Tag, budget)?;
                    for (index, variant) in variants.iter().enumerate() {
                        self.auxiliary(
                            ty,
                            RowRole::Payload(
                                u32::try_from(index).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                            ),
                            budget,
                        )?;
                        for &field in variant.fields().fields() {
                            self.enqueue(field, selected, budget)?;
                        }
                    }
                }
                SemanticRustcVariantsV1::Single { index } => {
                    let variant = variants
                        .get(*index as usize)
                        .ok_or_else(|| error("source storage single variant is absent"))?;
                    for &field in variant.fields().fields() {
                        self.enqueue(field, selected, budget)?;
                    }
                }
                SemanticRustcVariantsV1::Empty => {}
            },
            SemanticTypeShapeV1::Opaque
                if matches!(
                    declaration.layout().backend_repr(),
                    SemanticBackendReprV1::SimdVector { .. }
                ) => {}
            SemanticTypeShapeV1::Slice { .. }
            | SemanticTypeShapeV1::FunctionPointer { .. }
            | SemanticTypeShapeV1::Opaque => {
                return Err(error(
                    "source storage type requires additional exact physical representation facts",
                ));
            }
        }
        Ok(())
    }

    fn field_rows(
        &self,
        fields: &[SemanticTypeIdV1],
        offsets: &[u64],
        resolver: &SourceStorageOriginalResolverV29<'_>,
        budget: &mut Budget<'_>,
    ) -> Result<Box<[StorageFieldV1]>, Error> {
        if fields.len() != offsets.len() {
            return Err(error(
                "source storage field offsets differ from source field roster",
            ));
        }
        let mut result = self.lease.vector(fields.len(), budget)?;
        for (&ty, &offset) in fields.iter().zip(offsets) {
            self.lease.work(1, budget)?;
            let layout = self.original_resolve(RowKey::ty(ty), resolver, budget)?;
            self.lease
                .push(&mut result, StorageFieldV1 { offset, layout }, budget)?;
        }
        exact_box(result)
    }

    fn aggregate_fields(
        &self,
        declaration: &SemanticTypeDeclV1,
        fields: &[SemanticTypeIdV1],
        resolver: &SourceStorageOriginalResolverV29<'_>,
        budget: &mut Budget<'_>,
    ) -> Result<Box<[StorageFieldV1]>, Error> {
        let offsets = match declaration.layout().details() {
            SemanticTypeLayoutDetailsV1::Aggregate(layout) => layout.field_offsets(),
            SemanticTypeLayoutDetailsV1::None if fields.is_empty() => &[],
            _ => {
                return Err(error(
                    "source storage aggregate has no exact field-offset facts",
                ));
            }
        };
        self.field_rows(fields, offsets, resolver, budget)
    }

    fn union_fields(
        &self,
        declaration: &SemanticTypeDeclV1,
        fields: &[SemanticTypeIdV1],
        resolver: &SourceStorageOriginalResolverV29<'_>,
        budget: &mut Budget<'_>,
    ) -> Result<Box<[StorageFieldV1]>, Error> {
        if !matches!(declaration.layout().fields(), SemanticFieldsShapeV1::Union { field_count }
            if *field_count as usize == fields.len())
            || !matches!(
                declaration.layout().details(),
                SemanticTypeLayoutDetailsV1::None
            )
        {
            return Err(error(
                "source union lacks its exact overlapping field roster",
            ));
        }
        let mut result = self.lease.vector(fields.len(), budget)?;
        for &ty in fields {
            self.lease.work(1, budget)?;
            let layout = self.original_resolve(RowKey::ty(ty), resolver, budget)?;
            self.lease
                .push(&mut result, StorageFieldV1 { offset: 0, layout }, budget)?;
        }
        exact_box(result)
    }

    fn thin_pointer_row(
        &self,
        pointer: &SemanticPointerTypeV1,
        pointee: SemanticTypeIdV1,
        size: u64,
        alignment: u64,
        resolver: &SourceStorageOriginalResolverV29<'_>,
        budget: &mut Budget<'_>,
    ) -> Result<StorageLayoutV1, Error> {
        self.lease.work(1, budget)?;
        if size.checked_mul(8) != Some(u64::from(pointer.pointer_width_bits())) {
            return Err(error(
                "source storage pointer layout disagrees with admitted pointer width",
            ));
        }
        let space = self.pointer_space(pointer.address_space())?;
        Ok(StorageLayoutV1 {
            size,
            alignment: physical_alignment(alignment)?,
            kind: StorageLayoutKindV1::Pointer(StoragePointerV1 {
                pointee: self.original_resolve(RowKey::ty(pointee), resolver, budget)?,
                value_space: space,
                encoded_space: space,
                access: Self::pointer_access(pointer),
                stored_bits: pointer.pointer_width_bits(),
            }),
        })
    }

    fn descriptor_parts(
        &self,
        ty: SemanticTypeIdV1,
    ) -> Result<(SemanticBackendPrimitiveV1, SemanticBackendPrimitiveV1, u64), Error> {
        let declaration = self.declaration(ty)?;
        let SemanticTypeShapeV1::Pointer(pointer) = declaration.shape() else {
            return Err(error("source descriptor row has a non-pointer source type"));
        };
        if pointer.metadata() != SemanticPointerMetadataV1::SliceLength {
            return Err(error(
                "source descriptor row does not carry slice length metadata",
            ));
        }
        let SemanticBackendReprV1::ScalarPair { first, second } =
            declaration.layout().backend_repr()
        else {
            return Err(error(
                "source slice descriptor lacks exact scalar-pair representation",
            ));
        };
        let first = first.primitive();
        let second = second.primitive();
        if !matches!(first, SemanticBackendPrimitiveV1::Pointer { address_space, size_bytes, .. }
            if address_space == pointer.address_space() && size_bytes.checked_mul(8) == Some(u64::from(pointer.pointer_width_bits())))
            || !matches!(
                second,
                SemanticBackendPrimitiveV1::Integer {
                    signed: false,
                    bits: 64,
                    ..
                }
            )
        {
            return Err(error(
                "source slice descriptor data and length representation changed",
            ));
        }
        let first_size = first.size_bytes().ok_or(ArgumentResourceV1::Arithmetic)?;
        let align = second.alignment_bytes();
        let offset = first_size
            .checked_add(align - 1)
            .map(|value| value & !(align - 1))
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        if let SemanticFieldsShapeV1::Arbitrary {
            source_order_offsets_bytes,
            ..
        } = declaration.layout().fields()
            && source_order_offsets_bytes.as_ref() != [0, offset]
        {
            return Err(error(
                "source descriptor field placement disagrees with its scalar pair",
            ));
        }
        Ok((first, second, offset))
    }

    fn descriptor_row(
        &self,
        key: RowKey,
        resolver: &SourceStorageOriginalResolverV29<'_>,
        budget: &mut Budget<'_>,
    ) -> Result<StorageLayoutV1, Error> {
        let declaration = self.declaration(key.ty)?;
        let SemanticTypeShapeV1::Pointer(pointer) = declaration.shape() else {
            return Err(error("source descriptor key changed source type"));
        };
        let (data, length, offset) = self.descriptor_parts(key.ty)?;
        match key.role {
            RowRole::DescriptorData => self.thin_pointer_row(
                pointer,
                self.slice_element(pointer)?,
                data.size_bytes().ok_or(ArgumentResourceV1::Arithmetic)?,
                data.alignment_bytes(),
                resolver,
                budget,
            ),
            RowRole::DescriptorLength => Ok(StorageLayoutV1 {
                size: length.size_bytes().ok_or(ArgumentResourceV1::Arithmetic)?,
                alignment: physical_alignment(length.alignment_bytes())?,
                kind: StorageLayoutKindV1::Scalar(ScalarType::Index),
            }),
            RowRole::Type => Ok(StorageLayoutV1 {
                size: declaration
                    .layout()
                    .size_bytes()
                    .ok_or_else(|| error("source descriptor is unsized"))?,
                alignment: physical_alignment(declaration.layout().alignment_bytes())?,
                kind: StorageLayoutKindV1::Slice {
                    element: self.original_resolve(RowKey::ty(self.slice_element(pointer)?), resolver, budget)?,
                    value_space: self.pointer_space(pointer.address_space())?,
                    access: Self::pointer_access(pointer),
                    data: StorageFieldV1 {
                        offset: 0,
                        layout: self.original_resolve(
                            RowKey {
                                ty: key.ty,
                                role: RowRole::DescriptorData,
                            },
                            resolver,
                            budget,
                        )?,
                    },
                    length: StorageFieldV1 {
                        offset,
                        layout: self.original_resolve(
                            RowKey {
                                ty: key.ty,
                                role: RowRole::DescriptorLength,
                            },
                            resolver,
                            budget,
                        )?,
                    },
                },
            }),
            _ => Err(error("source descriptor row key has the wrong role")),
        }
    }

    fn lower_row(&self, key: RowKey, budget: &mut Budget<'_>) -> Result<StorageLayoutV1, Error> {
        self.lower_row_resolved(key, &SourceStorageOriginalResolverV29::Original, budget)
    }

    fn lower_row_resolved(
        &self,
        key: RowKey,
        resolver: &SourceStorageOriginalResolverV29<'_>,
        budget: &mut Budget<'_>,
    ) -> Result<StorageLayoutV1, Error> {
        self.lease.work(1, budget)?;
        let declaration = self.declaration(key.ty)?;
        match key.role {
            RowRole::Payload(_) | RowRole::Tag => return self.enum_auxiliary_resolved(key, resolver, budget),
            RowRole::DescriptorData | RowRole::DescriptorLength => {
                return self.descriptor_row(key, resolver, budget);
            }
            RowRole::Type => {}
        }
        let layout = declaration.layout();
        let size = layout
            .size_bytes()
            .ok_or_else(|| error("source storage object is unsized"))?;
        let alignment = physical_alignment(layout.alignment_bytes())?;
        let kind = match declaration.shape() {
            SemanticTypeShapeV1::Unit | SemanticTypeShapeV1::Never => {
                StorageLayoutKindV1::Record(exact_box(self.lease.vector(0, budget)?)?)
            }
            SemanticTypeShapeV1::Scalar(scalar) => {
                StorageLayoutKindV1::Scalar(scalar_kind(*scalar)?)
            }
            SemanticTypeShapeV1::ValidityScalar(scalar) => {
                StorageLayoutKindV1::Scalar(scalar_kind(scalar.scalar())?)
            }
            SemanticTypeShapeV1::Pointer(pointer) => match pointer.metadata() {
                SemanticPointerMetadataV1::None => {
                    return self.thin_pointer_row(
                        pointer,
                        pointer.pointee(),
                        size,
                        layout.alignment_bytes(),
                        resolver,
                        budget,
                    );
                }
                SemanticPointerMetadataV1::SliceLength => return self.descriptor_row(key, resolver, budget),
                SemanticPointerMetadataV1::VTable => {
                    return Err(error(
                        "source storage vtable descriptor is not yet represented",
                    ));
                }
            },
            SemanticTypeShapeV1::Array { element, length } => {
                let SemanticFieldsShapeV1::Array {
                    stride_bytes,
                    count,
                } = layout.fields()
                else {
                    return Err(error("source array has no exact rustc stride"));
                };
                if count != length {
                    return Err(error("source array length differs from its layout"));
                }
                StorageLayoutKindV1::Array {
                    element: self.original_resolve(RowKey::ty(*element), resolver, budget)?,
                    length: *length,
                    stride: *stride_bytes,
                }
            }
            SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields) => {
                StorageLayoutKindV1::Record(self.aggregate_fields(
                    declaration,
                    fields.fields(),
                    resolver,
                    budget,
                )?)
            }
            SemanticTypeShapeV1::Union(fields) => StorageLayoutKindV1::Union(self.union_fields(
                declaration,
                fields.fields(),
                resolver,
                budget,
            )?),
            SemanticTypeShapeV1::Enum { .. } => return self.enum_row(key.ty, resolver, budget),
            SemanticTypeShapeV1::Opaque => {
                let SemanticBackendReprV1::SimdVector { element, count } = layout.backend_repr()
                else {
                    return Err(error(
                        "opaque source storage lacks exact vector representation",
                    ));
                };
                let vector = FixedVectorTypeV12::new(
                    primitive_scalar(element.primitive())?,
                    u16::try_from(*count).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                    VectorLayoutV12::Contiguous,
                );
                vector.validate().map_err(|_| {
                    error("source vector exceeds the admitted physical vector contract")
                })?;
                StorageLayoutKindV1::Vector(vector)
            }
            SemanticTypeShapeV1::Slice { .. } | SemanticTypeShapeV1::FunctionPointer { .. } => {
                return Err(error(
                    "source storage type lacks a complete physical row contract",
                ));
            }
        };
        Ok(StorageLayoutV1 {
            size,
            alignment,
            kind,
        })
    }
}
