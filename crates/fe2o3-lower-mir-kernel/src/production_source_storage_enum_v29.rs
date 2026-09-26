impl<'layout, 'source> SourceStorageStateV29<'layout, 'source> {
    fn discriminant_is_initialized(
        &self,
        place: &SourceStorageSubobjectV29<'layout, 'source>,
        budget: &mut Budget<'_>,
    ) -> Result<bool, Error> {
        self.check_place(place, budget)?;
        if place.variant.is_some() {
            return Err(error("source discriminant read names a payload instead of its enum"));
        }
        self.layouts.enum_parts(place.ty)?;
        for (depth, step) in place.path.iter().enumerate() {
            self.layouts.lease.work(1, budget)?;
            if matches!(step, SubobjectStep::Variant(_)) && !self.guard_holds(place, depth, budget)? {
                return Ok(false);
            }
        }
        let proof = self.initialization_proof(place, true, budget)?;
        let whole = proof.initialized && self.guards_hold(place, &proof.guards, budget)?;
        proof.discard(&self.layouts.lease, budget)?;
        let readable = whole || if let Some(index) = self.enum_entry(place, budget)? {
            !self.enums[index].readable.is_empty()
                && self.guards_hold(place, &self.enums[index].guards, budget)?
        } else {
            whole
        };
        if !readable {
            return Ok(false);
        }
        match self.layouts.enum_tag_range(place)? {
            Some(range) => self.bytes_initialized(range, budget),
            None => Ok(whole || readable),
        }
    }

    fn restrict_discriminant(
        &mut self,
        place: &SourceStorageSubobjectV29<'layout, 'source>,
        allowed: &[u32],
        budget: &mut Budget<'_>,
    ) -> Result<bool, Error> {
        if !self.discriminant_is_initialized(place, budget)? {
            return Err(error("source discriminator edge has no initialized current tag"));
        }
        for (ordinal, &variant) in allowed.iter().enumerate() {
            self.layouts.lease.work(1, budget)?;
            self.check_variant(place, variant)?;
            if ordinal != 0 && allowed[ordinal - 1] >= variant {
                return Err(error("source discriminator edge repeats or reorders variants"));
            }
        }
        let existing = match self.enum_entry(place, budget)? {
            Some(index) if !self.enums[index].readable.is_empty()
                && self.guards_hold(place, &self.enums[index].guards, budget)? => Some(index),
            _ => None,
        };
        let mut selected = self.layouts.lease.vector(allowed.len(), budget)?;
        let mut cursor = 0;
        for &variant in allowed {
            self.layouts.lease.work(2, budget)?;
            let present = if let Some(index) = existing {
                let readable = &self.enums[index].readable;
                while cursor < readable.len() && readable[cursor] < variant {
                    self.layouts.lease.work(2, budget)?;
                    cursor += 1;
                }
                readable.get(cursor) == Some(&variant)
            } else {
                true
            };
            if present {
                self.layouts.lease.push(&mut selected, variant, budget)?;
            }
        }
        if selected.is_empty() {
            self.layouts.lease.discard_vec(selected, budget)?;
            return Ok(false);
        }
        let index = self.enum_state_mut(place, budget)?;
        if existing.is_none() {
            self.enums[index].guards.clear();
        }
        let old = std::mem::replace(&mut self.enums[index].readable, selected);
        self.layouts.lease.discard_vec(old, budget)?;
        // Conditional payload facts stay conditional; guards_hold now evaluates
        // the narrower readable set. No byte or logical initialization is added.
        Ok(true)
    }

    fn enum_state_mut(
        &mut self,
        place: &SourceStorageSubobjectV29<'layout, 'source>,
        budget: &mut Budget<'_>,
    ) -> Result<usize, Error> {
        self.check_place(place, budget)?;
        if place.variant.is_some()
            || !matches!(
                self.layouts.declaration(place.ty)?.shape(),
                SemanticTypeShapeV1::Enum { .. }
            )
        {
            return Err(error(
                "source enum state must name the original enum object",
            ));
        }
        if let Some(index) = self.enum_entry(place, budget)? {
            return Ok(index);
        }
        let copy = place.copy(budget)?;
        self.layouts.lease.push(
            &mut self.enums,
            EnumState {
                place: copy,
                construction: None,
                readable: Vec::new(),
                guards: Vec::new(),
            },
            budget,
        )?;
        Ok(self.enums.len() - 1)
    }

    fn check_variant(
        &self,
        place: &SourceStorageSubobjectV29<'layout, 'source>,
        variant: u32,
    ) -> Result<(), Error> {
        let (_, variants) = self.layouts.enum_parts(place.ty)?;
        let variant = variants
            .get(variant as usize)
            .ok_or_else(|| error("source enum state variant is absent"))?;
        if variant.is_uninhabited() {
            return Err(error(
                "source enum state cannot activate an uninhabited variant",
            ));
        }
        Ok(())
    }

    pub(super) fn begin_variant(
        &mut self,
        place: &SourceStorageSubobjectV29<'layout, 'source>,
        variant: u32,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        self.check_variant(place, variant)?;
        self.deinitialize(place, budget)?;
        let index = self.enum_state_mut(place, budget)?;
        self.enums[index].construction = Some(variant);
        self.enums[index].readable.clear();
        self.enums[index].guards.clear();
        Ok(())
    }

    pub(super) fn variant_is_readable(
        &self,
        place: &SourceStorageSubobjectV29<'layout, 'source>,
        variant: u32,
        budget: &mut Budget<'_>,
    ) -> Result<bool, Error> {
        self.check_place(place, budget)?;
        self.check_variant(place, variant)?;
        let Some(index) = self.enum_entry(place, budget)? else {
            return Ok(false);
        };
        Ok(self.enums[index].readable.as_slice() == [variant])
    }

    pub(super) fn set_discriminant(
        &mut self,
        place: &SourceStorageSubobjectV29<'layout, 'source>,
        variant: u32,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        self.check_place(place, budget)?;
        self.check_variant(place, variant)?;
        let snapshot = self.copy(budget)?;
        let declaration = self.layouts.declaration(place.ty)?;
        match declaration.layout().variants() {
            SemanticRustcVariantsV1::Multiple(layout) => {
                let (offset, primitive) = match layout.encoding() {
                    SemanticEnumEncodingV1::Direct(tag) => {
                        self.prepare_tag_assignment(&snapshot, place, variant, budget)?;
                        (tag.tag_offset_bytes(), tag.tag().primitive())
                    }
                    SemanticEnumEncodingV1::Niche(tag) => {
                        let payload = self.niche_payload(place, tag, budget)?;
                        if variant == tag.untagged_variant() {
                            let valid =
                                snapshot.untagged_payload_is_valid(place, &payload, tag, budget)?;
                            payload.discard(budget)?;
                            if !valid {
                                // This encoding emits no write. A requested variant is
                                // not evidence that its niche payload became valid.
                                return snapshot.discard(budget);
                            }
                            self.prepare_tag_assignment(&snapshot, place, variant, budget)?;
                            let index = self.enum_state_mut(place, budget)?;
                            self.enums[index].readable.clear();
                            self.enums[index].guards.clear();
                            self.layouts.lease.push(
                                &mut self.enums[index].readable,
                                variant,
                                budget,
                            )?;
                            self.enums[index].construction = None;
                            return snapshot.discard(budget);
                        }
                        self.prepare_tag_assignment(&snapshot, place, variant, budget)?;
                        let range = self
                            .layouts
                            .enum_tag_range(place)?
                            .ok_or(ArgumentResourceV1::Accounting)?;
                        self.invalidate_tag_payload(&snapshot, &payload, range, budget)?;
                        payload.discard(budget)?;
                        (tag.source().expected_offset_bytes(), tag.tag().primitive())
                    }
                };
                let start = place
                    .range
                    .start
                    .checked_add(offset)
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
                let extent = self
                    .layouts
                    .declaration(self.ty)?
                    .layout()
                    .size_bytes()
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
                let range = SourceStorageRangeV29::new(
                    start,
                    primitive
                        .size_bytes()
                        .ok_or(ArgumentResourceV1::Arithmetic)?,
                    extent,
                )?;
                self.invalidate_tag_aliases(&snapshot, place, range, budget)?;
                self.update_bytes(range, true, budget)?;
            }
            SemanticRustcVariantsV1::Single { index } if *index == variant => {}
            _ => {
                return Err(error(
                    "source discriminant state differs from its source enum layout",
                ));
            }
        }
        let index = self.enum_state_mut(place, budget)?;
        self.enums[index].readable.clear();
        self.enums[index].guards.clear();
        self.layouts
            .lease
            .push(&mut self.enums[index].readable, variant, budget)?;
        self.enums[index].construction = None;
        snapshot.discard(budget)
    }
}

impl SourceStorageLayoutsV29<'_> {
    fn preserves_niche_tag(
        &self,
        state: &EnumState<'_, '_>,
        write: &SourceStorageSubobjectV29<'_, '_>,
        budget: &mut Budget<'_>,
    ) -> Result<bool, Error> {
        let SemanticRustcVariantsV1::Multiple(layout) =
            self.declaration(state.place.ty)?.layout().variants()
        else {
            return Ok(false);
        };
        let SemanticEnumEncodingV1::Niche(niche) = layout.encoding() else {
            return Ok(false);
        };
        let variant = niche.untagged_variant();
        if state.readable.as_slice() != [variant] && state.construction != Some(variant) {
            return Ok(false);
        }
        let depth = state.place.path.len();
        if write.path.len() <= depth + 1
            || !prefix_path(&state.place.path, &write.path, &self.lease, budget)?
            || write.path[depth] != SubobjectStep::Variant(variant)
        {
            return Ok(false);
        }
        let suffix = &write.path[depth + 1..];
        let source = niche.source().path();
        if suffix.len() > source.len() {
            return Ok(false);
        }
        for (step, source) in suffix.iter().zip(source) {
            self.lease.work(1, budget)?;
            let expected = match source {
                SemanticNichePathComponentV1::Field(field) => SubobjectStep::Field(*field),
                SemanticNichePathComponentV1::ArrayElement(index) => SubobjectStep::Element(*index),
            };
            if *step != expected {
                return Ok(false);
            }
        }
        Ok(self
            .enum_tag_range(&state.place)?
            .is_some_and(|tag| write.range.contains(tag)))
    }

    fn enum_tag_range(
        &self,
        place: &SourceStorageSubobjectV29<'_, '_>,
    ) -> Result<Option<SourceStorageRangeV29>, Error> {
        let SemanticRustcVariantsV1::Multiple(layout) =
            self.declaration(place.ty)?.layout().variants()
        else {
            return Ok(None);
        };
        let (offset, primitive) = match layout.encoding() {
            SemanticEnumEncodingV1::Direct(tag) => (tag.tag_offset_bytes(), tag.tag().primitive()),
            SemanticEnumEncodingV1::Niche(tag) => {
                (tag.source().expected_offset_bytes(), tag.tag().primitive())
            }
        };
        Ok(Some(SourceStorageRangeV29::new(
            place
                .range
                .start
                .checked_add(offset)
                .ok_or(ArgumentResourceV1::Arithmetic)?,
            primitive
                .size_bytes()
                .ok_or(ArgumentResourceV1::Arithmetic)?,
            self.declaration(place.root)?
                .layout()
                .size_bytes()
                .ok_or(ArgumentResourceV1::Arithmetic)?,
        )?))
    }
    fn enum_parts(
        &self,
        ty: SemanticTypeIdV1,
    ) -> Result<(&SemanticTypeDeclV1, &[SemanticEnumVariantV1]), Error> {
        let declaration = self.declaration(ty)?;
        let SemanticTypeShapeV1::Enum { variants, .. } = declaration.shape() else {
            return Err(error("source storage enum role belongs to a non-enum type"));
        };
        Ok((declaration, variants))
    }

    fn enum_variant_layout(
        &self,
        ty: SemanticTypeIdV1,
        variant: u32,
    ) -> Result<&SemanticEnumVariantLayoutV1, Error> {
        let (declaration, _) = self.enum_parts(ty)?;
        let SemanticRustcVariantsV1::Multiple(layout) = declaration.layout().variants() else {
            return Err(error("source enum payload has no multi-variant layout"));
        };
        layout
            .variants()
            .get(variant as usize)
            .filter(|row| row.variant_index() == variant)
            .ok_or_else(|| error("source enum payload layout identity changed"))
    }

    fn niche_terminal(
        &self,
        ty: SemanticTypeIdV1,
        budget: &mut Budget<'_>,
    ) -> Result<(SemanticTypeIdV1, u64), Error> {
        use fe2o3_mir_model::semantic_mir_v1::SemanticNichePathComponentV1 as Part;
        let (declaration, variants) = self.enum_parts(ty)?;
        let SemanticRustcVariantsV1::Multiple(layout) = declaration.layout().variants() else {
            return Err(error("source niche is not a multi-variant enum"));
        };
        let SemanticEnumEncodingV1::Niche(niche) = layout.encoding() else {
            return Err(error("source enum does not have niche encoding"));
        };
        let variant = variants
            .get(niche.untagged_variant() as usize)
            .ok_or_else(|| error("source niche untagged variant is absent"))?;
        let variant_layout = self.enum_variant_layout(ty, niche.untagged_variant())?;
        let mut current = None;
        let mut offset = 0_u64;
        for part in niche.source().path() {
            self.lease.work(1, budget)?;
            let (next, relative) = match (part, current) {
                (Part::Field(index), None) => {
                    let index = *index as usize;
                    (
                        *variant
                            .fields()
                            .fields()
                            .get(index)
                            .ok_or_else(|| error("source niche field is absent"))?,
                        *variant_layout
                            .aggregate()
                            .field_offsets()
                            .get(index)
                            .ok_or_else(|| error("source niche field offset is absent"))?,
                    )
                }
                (Part::Field(index), Some(id)) => {
                    let declaration = self.declaration(id)?;
                    let (
                        SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields),
                        SemanticTypeLayoutDetailsV1::Aggregate(layout),
                    ) = (declaration.shape(), declaration.layout().details())
                    else {
                        return Err(error("source niche field does not project an aggregate"));
                    };
                    (
                        *fields
                            .fields()
                            .get(*index as usize)
                            .ok_or_else(|| error("source niche nested field is absent"))?,
                        *layout
                            .field_offsets()
                            .get(*index as usize)
                            .ok_or_else(|| error("source niche nested offset is absent"))?,
                    )
                }
                (Part::ArrayElement(index), Some(id)) => {
                    let declaration = self.declaration(id)?;
                    let (
                        SemanticTypeShapeV1::Array { element, length },
                        SemanticFieldsShapeV1::Array {
                            stride_bytes,
                            count,
                        },
                    ) = (declaration.shape(), declaration.layout().fields())
                    else {
                        return Err(error("source niche index does not project an exact array"));
                    };
                    if index >= length || count != length {
                        return Err(error("source niche index is out of bounds"));
                    }
                    (
                        *element,
                        index
                            .checked_mul(*stride_bytes)
                            .ok_or(ArgumentResourceV1::Arithmetic)?,
                    )
                }
                _ => return Err(error("source niche path lacks its original field root")),
            };
            offset = offset
                .checked_add(relative)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            current = Some(next);
        }
        let relative = niche
            .source()
            .expected_offset_bytes()
            .checked_sub(offset)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        Ok((
            current.ok_or_else(|| error("source niche path is empty"))?,
            relative,
        ))
    }

    fn niche_pointer_target(
        &self,
        ty: SemanticTypeIdV1,
        primitive: SemanticBackendPrimitiveV1,
        budget: &mut Budget<'_>,
    ) -> Result<(&SemanticPointerTypeV1, SemanticTypeIdV1), Error> {
        let (mut terminal, mut relative) = self.niche_terminal(ty, budget)?;
        // The admitted by-value graph is acyclic. Keep the walk iterative and
        // bounded even if a future source admission accidentally weakens that.
        for _ in 0..self.owner.source_semantic().types().len() {
            self.lease.work(1, budget)?;
            let declaration = self.declaration(terminal)?;
            match declaration.shape() {
                SemanticTypeShapeV1::Pointer(pointer) if relative == 0 => {
                    let pointee = match pointer.metadata() {
                        SemanticPointerMetadataV1::None => pointer.pointee(),
                        SemanticPointerMetadataV1::SliceLength => self.slice_element(pointer)?,
                        SemanticPointerMetadataV1::VTable => {
                            return Err(error(
                                "source pointer niche requires the vtable descriptor contract",
                            ));
                        }
                    };
                    if !matches!(primitive, SemanticBackendPrimitiveV1::Pointer { address_space, .. } if address_space == pointer.address_space())
                    {
                        return Err(error(
                            "source pointer niche changed its encoded address space",
                        ));
                    }
                    return Ok((pointer, pointee));
                }
                SemanticTypeShapeV1::Enum { .. } => {
                    let niche = declaration
                        .layout()
                        .largest_niche()
                        .ok_or_else(|| error("source nested enum has no retained niche"))?;
                    if niche.offset_bytes() != relative || niche.primitive() != primitive {
                        return Err(error("source nested pointer niche representation changed"));
                    }
                    (terminal, relative) = self.niche_terminal(terminal, budget)?;
                }
                _ => {
                    return Err(error(
                        "source pointer niche is not grounded in its original pointer field",
                    ));
                }
            }
        }
        Err(error("source pointer niche containment is cyclic"))
    }

    fn enum_auxiliary(
        &self,
        key: RowKey,
        budget: &mut Budget<'_>,
    ) -> Result<StorageLayoutV1, Error> {
        self.enum_auxiliary_resolved(key, &SourceStorageOriginalResolverV29::Original, budget)
    }

    fn enum_auxiliary_resolved(
        &self,
        key: RowKey,
        resolver: &SourceStorageOriginalResolverV29<'_>,
        budget: &mut Budget<'_>,
    ) -> Result<StorageLayoutV1, Error> {
        self.lease.work(1, budget)?;
        let (declaration, variants) = self.enum_parts(key.ty)?;
        let SemanticRustcVariantsV1::Multiple(layout) = declaration.layout().variants() else {
            return Err(error(
                "source enum auxiliary row lacks multi-variant layout",
            ));
        };
        match key.role {
            RowRole::Payload(index) => {
                let variant = variants
                    .get(index as usize)
                    .ok_or_else(|| error("source enum variant is absent"))?;
                let row = self.enum_variant_layout(key.ty, index)?;
                Ok(StorageLayoutV1 {
                    size: row.rustc_size_bytes(),
                    alignment: physical_alignment(row.alignment_bytes())?,
                    kind: StorageLayoutKindV1::Record(self.field_rows(
                        variant.fields().fields(),
                        row.aggregate().field_offsets(),
                        resolver,
                        budget,
                    )?),
                })
            }
            RowRole::Tag => {
                let primitive = match layout.encoding() {
                    SemanticEnumEncodingV1::Direct(encoding) => encoding.tag().primitive(),
                    SemanticEnumEncodingV1::Niche(encoding) => encoding.tag().primitive(),
                };
                if matches!(primitive, SemanticBackendPrimitiveV1::Pointer { .. }) {
                    let (pointer, pointee) = self.niche_pointer_target(key.ty, primitive, budget)?;
                    return self.thin_pointer_row(pointer, pointee,
                        primitive.size_bytes().ok_or(ArgumentResourceV1::Arithmetic)?,
                        primitive.alignment_bytes(), resolver, budget);
                }
                Ok(StorageLayoutV1 {
                    size: primitive
                        .size_bytes()
                        .ok_or(ArgumentResourceV1::Arithmetic)?,
                    alignment: physical_alignment(primitive.alignment_bytes())?,
                    kind: StorageLayoutKindV1::Scalar(primitive_scalar(primitive)?),
                })
            }
            _ => Err(error("source enum auxiliary row has the wrong role")),
        }
    }

    fn enum_row(
        &self,
        ty: SemanticTypeIdV1,
        resolver: &SourceStorageOriginalResolverV29<'_>,
        budget: &mut Budget<'_>,
    ) -> Result<StorageLayoutV1, Error> {
        self.lease.work(1, budget)?;
        let (declaration, variants) = self.enum_parts(ty)?;
        let layout = declaration.layout();
        let kind = match layout.variants() {
            SemanticRustcVariantsV1::Empty => {
                StorageLayoutKindV1::Record(exact_box(self.lease.vector(0, budget)?)?)
            }
            SemanticRustcVariantsV1::Single { index } => {
                let variant = variants
                    .get(*index as usize)
                    .ok_or_else(|| error("source enum single variant is absent"))?;
                StorageLayoutKindV1::Record(self.aggregate_fields(
                    declaration,
                    variant.fields().fields(),
                    resolver,
                    budget,
                )?)
            }
            SemanticRustcVariantsV1::Multiple(enumeration) => {
                let tag_layout = self.original_resolve(
                    RowKey {
                        ty,
                        role: RowRole::Tag,
                    },
                    resolver,
                    budget,
                )?;
                let (encoding, tag_bits) = match enumeration.encoding() {
                    SemanticEnumEncodingV1::Direct(direct) => {
                        let width = direct
                            .tag()
                            .primitive()
                            .size_bytes()
                            .and_then(|size| size.checked_mul(8))
                            .filter(|bits| (1..=128).contains(bits))
                            .ok_or(ArgumentResourceV1::Arithmetic)?;
                        (
                            StorageVariantEncodingV1::Direct {
                                tag: StorageFieldV1 {
                                    offset: direct.tag_offset_bytes(),
                                    layout: tag_layout,
                                },
                            },
                            Some(width),
                        )
                    }
                    SemanticEnumEncodingV1::Niche(niche) => {
                        let (first, last) = niche.niche_variant_range();
                        (
                            StorageVariantEncodingV1::Niche {
                                tag: StorageFieldV1 {
                                    offset: niche.source().expected_offset_bytes(),
                                    layout: tag_layout,
                                },
                                untagged_variant: niche.untagged_variant(),
                                first_niche_variant: first,
                                last_niche_variant: last,
                                niche_start: niche.niche_start(),
                            },
                            None,
                        )
                    }
                };
                let mut rows = self.lease.vector(variants.len(), budget)?;
                for (index, variant) in variants.iter().enumerate() {
                    self.lease.work(1, budget)?;
                    let index = u32::try_from(index).map_err(|_| ArgumentResourceV1::Arithmetic)?;
                    let payload = self.enum_variant_layout(ty, index)?;
                    let direct_tag_bits = tag_bits.map(|bits| {
                        if bits == 128 {
                            variant.discriminant()
                        } else {
                            variant.discriminant() & ((1_u128 << bits) - 1)
                        }
                    });
                    let row = StorageVariantV1 {
                        discriminant: variant.discriminant(),
                        direct_tag_bits,
                        uninhabited: variant.is_uninhabited() || payload.is_uninhabited(),
                        layout: self.original_resolve(
                            RowKey {
                                ty,
                                role: RowRole::Payload(index),
                            },
                            resolver,
                            budget,
                        )?,
                    };
                    self.lease.push(&mut rows, row, budget)?;
                }
                StorageLayoutKindV1::Variants {
                    encoding,
                    variants: exact_box(rows)?,
                }
            }
        };
        Ok(StorageLayoutV1 {
            size: layout
                .size_bytes()
                .ok_or_else(|| error("source enum storage is unsized"))?,
            alignment: physical_alignment(layout.alignment_bytes())?,
            kind,
        })
    }
}
