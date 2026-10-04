include!("production_source_storage_schema_projection_v29.rs");

// Selected schemas share the original module's one physical table. A row is
// representation data only; it never proves allocation, activity or provenance.
#[derive(Default)]
struct SourceStoragePhysicalV29 {
    rows: Vec<StorageLayoutV1>,
    selected: BTreeMap<(RowKey, u64), Vec<StorageLayoutIdV1>>,
    selected_keys: Vec<RowKey>,
    depths: Vec<usize>,
    edges: usize,
    original_extensions: BTreeMap<RowKey, StorageLayoutIdV1>,
}

// Every child selects a closed row in this same table. Missing components are
// admitted only for source-nominal identities, never unknown physical payloads.
pub(super) enum SourceStorageSelectionV29<'a> {
    Original,
    Pointer {
        pointee: StorageLayoutIdV1,
        value_space: AddressSpace,
        access: AccessMode,
    },
    Slice {
        element: StorageLayoutIdV1,
        value_space: AddressSpace,
        access: AccessMode,
    },
    Aggregate {
        variant: Option<u32>,
        fields: &'a [(u32, StorageLayoutIdV1)],
    },
    Array {
        element: StorageLayoutIdV1,
    },
    Enum {
        variants: &'a [(u32, StorageLayoutIdV1)],
    },
}

struct SourceStorageSchemaAllocationV29<'a>(&'a Lease);

impl Drop for SourceStorageSchemaAllocationV29<'_> {
    fn drop(&mut self) {
        self.0.schema_allocation.set(false);
    }
}

impl SourceStorageLayoutsV29<'_> {
    fn selected_descriptor_length_row(
        &self,
        ty: SemanticTypeIdV1,
        budget: &mut Budget<'_>,
    ) -> Result<StorageLayoutIdV1, Error> {
        self.lease.work(1, budget)?;
        if !self.lease.schema_allocation.get() {
            return Err(error(
                "selected descriptor length lacks persistent schema custody",
            ));
        }
        let key = RowKey {
            ty,
            role: RowRole::DescriptorLength,
        };
        match self.row_id(key, budget) {
            Ok(id) => Ok(id),
            Err(Error::Unsupported { .. }) => {
                let row = self.lower_row(key, budget)?;
                self.intern_schema(key, row, budget)
            }
            Err(error) => Err(error),
        }
    }

    pub(super) fn select_original_leaf_schema(
        &self,
        owner: &ProductionSemanticSsaOwnerV1,
        ty: SemanticTypeIdV1,
        budget: &mut Budget<'_>,
    ) -> Result<StorageLayoutIdV1, Error> {
        let result = (|| {
            self.check_owner(owner, budget)?;
            self.lease.work(1, budget)?;
            if !matches!(
                self.declaration(ty)?.shape(),
                SemanticTypeShapeV1::Unit
                    | SemanticTypeShapeV1::Never
                    | SemanticTypeShapeV1::Scalar(_)
                    | SemanticTypeShapeV1::ValidityScalar(_)
                    | SemanticTypeShapeV1::Opaque
            ) {
                return Err(error(
                    "original leaf selection cannot invent a composite or pointer schema",
                ));
            }
            if let Some(id) = self.original_schema(owner, ty, budget)? {
                self.check_selected_schema(owner, ty, id, budget)?;
                return Ok(id);
            }
            let _persistent = self.schema_allocation(budget)?;
            let key = RowKey::ty(ty);
            let row = self.lower_row(key, budget)?;
            self.intern_schema(key, row, budget)
        })();
        result.inspect_err(|error| self.lease.record(error))
    }

    pub(super) fn original_schema(
        &self,
        owner: &ProductionSemanticSsaOwnerV1,
        ty: SemanticTypeIdV1,
        budget: &mut Budget<'_>,
    ) -> Result<Option<StorageLayoutIdV1>, Error> {
        self.check_owner(owner, budget)?;
        charge_execution_cfg_lookup_v29(self.keys.len(), budget)
            .inspect_err(|error| self.lease.record(error))?;
        self.keys
            .binary_search(&RowKey::ty(ty))
            .ok()
            .map(|index| {
                u32::try_from(index)
                    .map(StorageLayoutIdV1)
                    .map_err(|_| ArgumentResourceV1::Arithmetic.into())
            })
            .transpose()
    }

    pub(super) fn check_selected_schema(
        &self,
        owner: &ProductionSemanticSsaOwnerV1,
        ty: SemanticTypeIdV1,
        id: StorageLayoutIdV1,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        self.check_owner(owner, budget)?;
        self.selected_child(id, ty, RowRole::Type, budget)
    }

    fn contained_child(row: &StorageLayoutV1, index: usize) -> Option<StorageLayoutIdV1> {
        match &row.kind {
            StorageLayoutKindV1::Record(fields) | StorageLayoutKindV1::Union(fields) => {
                fields.get(index).map(|field| field.layout)
            }
            StorageLayoutKindV1::Array { element, .. } => (index == 0).then_some(*element),
            StorageLayoutKindV1::Slice { data, length, .. } => match index {
                0 => Some(data.layout),
                1 => Some(length.layout),
                _ => None,
            },
            StorageLayoutKindV1::Variants { encoding, variants } => {
                if index == variants.len() {
                    Some(encoding.tag().layout)
                } else {
                    variants.get(index).map(|variant| variant.layout)
                }
            }
            _ => None,
        }
    }

    fn initialize_schema_metrics(&mut self, budget: &mut Budget<'_>) -> Result<(), Error> {
        let count = self.keys.len();
        if count == 0 {
            return Ok(());
        }
        self.lease.reserve(
            argument_sum_v1(&[
                size_of::<Vec<u8>>(),
                size_of::<Vec<usize>>(),
                size_of::<Vec<(usize, usize, usize)>>(),
            ])?,
            budget,
        )?;
        let mut depths = self.lease.vector::<usize>(count, budget)?;
        let mut colors = self.lease.vector(count, budget)?;
        let mut frames = self.lease.vector::<(usize, usize, usize)>(count, budget)?;
        self.lease.work(argument_product_v1(count, 2)?, budget)?;
        depths.resize(count, 0);
        colors.resize(count, 0_u8);
        let mut edges = 0_usize;
        for (index, key) in self.keys.iter().copied().enumerate() {
            self.lease.work(1, budget)?;
            let (size, row_edges) = self.original_row_metrics(key)?;
            edges = edges
                .checked_add(row_edges)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            if edges > self.limits.edges {
                return Err(error("source storage table exceeds retained edge policy"));
            }
            if size > self.limits.object_bytes {
                return Err(error(
                    "source storage table violates retained object-size policy",
                ));
            }
            if colors[index] != 0 {
                continue;
            }
            if self.limits.containment_depth == 0 {
                return Err(error(
                    "source storage table exceeds retained containment policy",
                ));
            }
            colors[index] = 1;
            self.lease.push(&mut frames, (index, 0, 1), budget)?;
            while let Some((parent, next, height)) = frames.last().copied() {
                self.lease.work(1, budget)?;
                let top = frames.len() - 1;
                if let Some(child) = self.original_contained_key(self.keys[parent], next)? {
                    frames[top].1 = next.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
                    let child = self.row_id(child, budget)?.0 as usize;
                    match colors.get(child).copied() {
                        Some(0) => {
                            if frames.len() >= self.limits.containment_depth {
                                return Err(error(
                                    "source storage table exceeds retained containment policy",
                                ));
                            }
                            colors[child] = 1;
                            self.lease.push(&mut frames, (child, 0, 1), budget)?;
                        }
                        Some(2) => {
                            frames[top].2 = height.max(
                                depths[child]
                                    .checked_add(1)
                                    .ok_or(ArgumentResourceV1::Arithmetic)?,
                            );
                        }
                        _ => {
                            return Err(error(
                                "source storage original containment is cyclic or missing",
                            ));
                        }
                    }
                } else {
                    if height > self.limits.containment_depth {
                        return Err(error(
                            "source storage table exceeds retained containment policy",
                        ));
                    }
                    colors[parent] = 2;
                    depths[parent] = height;
                    frames.pop();
                    if let Some(outer) = frames.last_mut() {
                        outer.2 = outer.2.max(
                            height
                                .checked_add(1)
                                .ok_or(ArgumentResourceV1::Arithmetic)?,
                        );
                    }
                }
            }
        }
        self.lease.discard_vec(frames, budget)?;
        self.lease.discard_vec(colors, budget)?;
        self.physical.get_mut().depths = depths;
        self.physical.get_mut().edges = edges;
        self.lease.refund(
            argument_sum_v1(&[
                size_of::<Vec<u8>>(),
                size_of::<Vec<usize>>(),
                size_of::<Vec<(usize, usize, usize)>>(),
            ])?,
            budget,
        )
    }

    fn check_row_capacity(&self, present: usize, budget: &mut Budget<'_>) -> Result<(), Error> {
        self.lease.work(1, budget)?;
        if present >= self.limits.rows || u32::try_from(present).is_err() {
            return Err(error("source storage table exceeds retained row policy"));
        }
        Ok(())
    }

    fn schema_allocation(
        &self,
        budget: &Budget<'_>,
    ) -> Result<SourceStorageSchemaAllocationV29<'_>, Error> {
        self.lease.check(budget)?;
        if self.lease.schema_allocation.replace(true) {
            return Err(error("source storage schema allocation is already active"));
        }
        Ok(SourceStorageSchemaAllocationV29(&self.lease))
    }

    fn row_backing(row: &StorageLayoutV1) -> Result<usize, Error> {
        Ok(match &row.kind {
            StorageLayoutKindV1::Record(fields) | StorageLayoutKindV1::Union(fields) => {
                argument_product_v1(fields.len(), size_of::<StorageFieldV1>())?
            }
            StorageLayoutKindV1::Variants { variants, .. } => {
                argument_product_v1(variants.len(), size_of::<StorageVariantV1>())?
            }
            _ => 0,
        })
    }

    fn row_edges(row: &StorageLayoutV1) -> Result<usize, Error> {
        Ok(match &row.kind {
            StorageLayoutKindV1::Scalar(_) | StorageLayoutKindV1::Vector(_) => 0,
            StorageLayoutKindV1::Pointer(_) | StorageLayoutKindV1::Array { .. } => 1,
            StorageLayoutKindV1::Record(fields) | StorageLayoutKindV1::Union(fields) => {
                fields.len()
            }
            StorageLayoutKindV1::Slice { .. } => 3,
            StorageLayoutKindV1::Variants { variants, .. } => variants
                .len()
                .checked_add(1)
                .ok_or(ArgumentResourceV1::Arithmetic)?,
        })
    }

    fn schema_hash(&self, row: &StorageLayoutV1, budget: &mut Budget<'_>) -> Result<u64, Error> {
        use std::hash::{Hash, Hasher};
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        self.lease
            .work(argument_sum_v1(&[Self::row_edges(row)?, 1])?, budget)?;
        row.size.hash(&mut hash);
        row.alignment.hash(&mut hash);
        std::mem::discriminant(&row.kind).hash(&mut hash);
        let field = |field: StorageFieldV1,
                     hash: &mut std::collections::hash_map::DefaultHasher| {
            field.offset.hash(hash);
            field.layout.hash(hash);
        };
        match &row.kind {
            StorageLayoutKindV1::Scalar(value) => value.hash(&mut hash),
            StorageLayoutKindV1::Vector(value) => value.hash(&mut hash),
            StorageLayoutKindV1::Pointer(value) => {
                value.pointee.hash(&mut hash);
                value.value_space.hash(&mut hash);
                value.encoded_space.hash(&mut hash);
                value.access.hash(&mut hash);
                value.stored_bits.hash(&mut hash);
            }
            StorageLayoutKindV1::Record(fields) | StorageLayoutKindV1::Union(fields) => {
                fields.len().hash(&mut hash);
                for value in fields.iter() {
                    field(*value, &mut hash);
                }
            }
            StorageLayoutKindV1::Array {
                element,
                length,
                stride,
            } => {
                element.hash(&mut hash);
                length.hash(&mut hash);
                stride.hash(&mut hash);
            }
            StorageLayoutKindV1::Slice {
                element,
                value_space,
                access,
                data,
                length,
            } => {
                element.hash(&mut hash);
                value_space.hash(&mut hash);
                access.hash(&mut hash);
                field(*data, &mut hash);
                field(*length, &mut hash);
            }
            StorageLayoutKindV1::Variants { encoding, variants } => {
                std::mem::discriminant(encoding).hash(&mut hash);
                field(encoding.tag(), &mut hash);
                if let StorageVariantEncodingV1::Niche {
                    untagged_variant,
                    first_niche_variant,
                    last_niche_variant,
                    niche_start,
                    ..
                } = encoding
                {
                    untagged_variant.hash(&mut hash);
                    first_niche_variant.hash(&mut hash);
                    last_niche_variant.hash(&mut hash);
                    niche_start.hash(&mut hash);
                }
                variants.len().hash(&mut hash);
                for variant in variants.iter() {
                    variant.discriminant.hash(&mut hash);
                    variant.direct_tag_bits.hash(&mut hash);
                    variant.uninhabited.hash(&mut hash);
                    variant.layout.hash(&mut hash);
                }
            }
        }
        Ok(hash.finish())
    }

    fn schema_key(&self, id: StorageLayoutIdV1, budget: &mut Budget<'_>) -> Result<RowKey, Error> {
        self.lease.work(1, budget)?;
        if let Some(key) = self.keys.get(id.0 as usize) {
            return Ok(*key);
        }
        let physical = self
            .physical
            .try_borrow()
            .map_err(|_| error("source storage schema identity has an active builder"))?;
        physical
            .selected_keys
            .get(
                (id.0 as usize)
                    .checked_sub(self.keys.len())
                    .ok_or(ArgumentResourceV1::Accounting)?,
            )
            .copied()
            .ok_or_else(|| {
                error("selected source storage schema is absent from its original owner")
            })
    }

    fn intern_schema(
        &self,
        key: RowKey,
        row: StorageLayoutV1,
        budget: &mut Budget<'_>,
    ) -> Result<StorageLayoutIdV1, Error> {
        self.lease.check(budget)?;
        if !self.lease.schema_allocation.get() {
            return Err(error(
                "source storage schema publication lacks persistent custody",
            ));
        }
        let hash = self.schema_hash(&row, budget)?;
        let mut physical = self
            .physical
            .try_borrow_mut()
            .map_err(|_| error("source storage schema publication has an outstanding row view"))?;
        let original = match self.row_id(key, budget) {
            Ok(id) => Some(id.0 as usize),
            Err(Error::Unsupported { .. }) => None,
            Err(error) => return Err(error),
        };
        if let Some(id) = original {
            self.lease
                .work(argument_sum_v1(&[Self::row_edges(&row)?, 1])?, budget)?;
            if physical.rows.get(id) == Some(&row) {
                let backing = Self::row_backing(&row)?;
                drop(row);
                self.lease.refund(backing, budget)?;
                return Ok(StorageLayoutIdV1(
                    u32::try_from(id).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                ));
            }
        }
        charge_execution_cfg_lookup_v29(physical.selected.len(), budget)?;
        if let Some(ids) = physical.selected.get(&(key, hash)) {
            for &id in ids {
                self.lease
                    .work(argument_sum_v1(&[Self::row_edges(&row)?, 1])?, budget)?;
                if physical.rows.get(id.0 as usize) == Some(&row) {
                    let backing = Self::row_backing(&row)?;
                    drop(row);
                    self.lease.refund(backing, budget)?;
                    return Ok(id);
                }
            }
        }
        self.check_row_capacity(physical.rows.len(), budget)?;
        let edges = physical
            .edges
            .checked_add(Self::row_edges(&row)?)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        if edges > self.limits.edges {
            return Err(error("source storage table exceeds retained edge policy"));
        }
        if row.size > self.limits.object_bytes {
            return Err(error(
                "source storage table violates retained object-size policy",
            ));
        }
        let mut depth = 1;
        let mut next = 0;
        while let Some(child) = Self::contained_child(&row, next) {
            self.lease.work(1, budget)?;
            next = next.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
            let child = *physical
                .depths
                .get(child.0 as usize)
                .ok_or_else(|| error("selected storage containment child is not closed"))?;
            depth = depth.max(child.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?);
        }
        if depth > self.limits.containment_depth {
            return Err(error(
                "source storage table exceeds retained containment policy",
            ));
        }
        let id = StorageLayoutIdV1(
            u32::try_from(physical.rows.len()).map_err(|_| ArgumentResourceV1::Arithmetic)?,
        );
        self.lease.push(&mut physical.rows, row, budget)?;
        // Source-bound constructors preserve size, alignment and placement;
        // closed child IDs plus the checks above preserve the table caps.
        // The independent whole-table checker runs once at consuming install,
        // not once per append (which would make schema discovery quadratic).
        if !physical.selected.contains_key(&(key, hash)) {
            let before = budget.storage();
            let result = reserve_execution_cfg_map_entry_v29::<(RowKey, u64), Vec<StorageLayoutIdV1>>(
                physical.selected.len(),
                budget,
            );
            self.lease.observe(before, budget.storage())?;
            result?;
            physical.selected.insert((key, hash), Vec::new());
        }
        let ids = physical
            .selected
            .get_mut(&(key, hash))
            .ok_or(ArgumentResourceV1::Accounting)?;
        self.lease.push(ids, id, budget)?;
        self.lease.push(&mut physical.selected_keys, key, budget)?;
        self.lease.push(&mut physical.depths, depth, budget)?;
        physical.edges = edges;
        Ok(id)
    }

    fn selected_child(
        &self,
        id: StorageLayoutIdV1,
        ty: SemanticTypeIdV1,
        role: RowRole,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        if self.schema_key(id, budget)? != (RowKey { ty, role }) {
            return Err(error(
                "selected storage child differs from its original source component",
            ));
        }
        Ok(())
    }

    fn selected_pointer(
        &self,
        ty: SemanticTypeIdV1,
        pointee: StorageLayoutIdV1,
        value_space: AddressSpace,
        access: AccessMode,
        descriptor: bool,
        budget: &mut Budget<'_>,
    ) -> Result<StorageLayoutV1, Error> {
        self.lease.work(5, budget)?;
        let declaration = self.declaration(ty)?;
        let SemanticTypeShapeV1::Pointer(pointer) = declaration.shape() else {
            return Err(error(
                "selected pointer differs from its original source shape",
            ));
        };
        let expected_metadata = if descriptor {
            SemanticPointerMetadataV1::SliceLength
        } else {
            SemanticPointerMetadataV1::None
        };
        if pointer.metadata() != expected_metadata
            || access != Self::pointer_access(pointer)
            || (value_space == AddressSpace::Constant && access != AccessMode::ReadOnly)
        {
            return Err(error(
                "selected pointer changes original metadata or access",
            ));
        }
        let source_pointee = if descriptor {
            self.slice_element(pointer)?
        } else {
            pointer.pointee()
        };
        self.selected_child(pointee, source_pointee, RowRole::Type, budget)?;
        let encoded_space = self.pointer_space(pointer.address_space())?;
        // C1 chooses value space; original rustc geometry chooses encoded bits.
        // This cannot shrink an AS0 field when its actual value is Private.
        if encoded_space != AddressSpace::Generic && encoded_space != value_space {
            return Err(error(
                "selected pointer cannot change original stored address-space encoding",
            ));
        }
        let (size, alignment) = if descriptor {
            let (data, _, _) = self.descriptor_parts(ty)?;
            (
                data.size_bytes().ok_or(ArgumentResourceV1::Arithmetic)?,
                data.alignment_bytes(),
            )
        } else {
            (
                declaration
                    .layout()
                    .size_bytes()
                    .ok_or(ArgumentResourceV1::Arithmetic)?,
                declaration.layout().alignment_bytes(),
            )
        };
        if size.checked_mul(8) != Some(u64::from(pointer.pointer_width_bits())) {
            return Err(error("selected pointer changes original stored width"));
        }
        Ok(StorageLayoutV1 {
            size,
            alignment: physical_alignment(alignment)?,
            kind: StorageLayoutKindV1::Pointer(StoragePointerV1 {
                pointee,
                value_space,
                encoded_space,
                access,
                stored_bits: pointer.pointer_width_bits(),
            }),
        })
    }

    fn selected_fields(
        &self,
        ty: SemanticTypeIdV1,
        variant: Option<u32>,
        selected: &[(u32, StorageLayoutIdV1)],
        budget: &mut Budget<'_>,
    ) -> Result<(RowKey, StorageLayoutV1), Error> {
        self.lease.work(1, budget)?;
        let declaration = self.declaration(ty)?;
        let (fields, offsets, size, alignment, role, union) = match (declaration.shape(), variant) {
            (SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields), None) => {
                let SemanticTypeLayoutDetailsV1::Aggregate(layout) = declaration.layout().details()
                else {
                    return Err(error("selected record has no original field placement"));
                };
                (
                    fields.fields(),
                    Some(layout.field_offsets()),
                    declaration
                        .layout()
                        .size_bytes()
                        .ok_or(ArgumentResourceV1::Arithmetic)?,
                    declaration.layout().alignment_bytes(),
                    RowRole::Type,
                    false,
                )
            }
            (SemanticTypeShapeV1::Union(fields), None) => {
                if !matches!(declaration.layout().fields(), SemanticFieldsShapeV1::Union { field_count }
                    if *field_count as usize == fields.fields().len())
                {
                    return Err(error("selected union changes original field roster"));
                }
                (
                    fields.fields(),
                    None,
                    declaration
                        .layout()
                        .size_bytes()
                        .ok_or(ArgumentResourceV1::Arithmetic)?,
                    declaration.layout().alignment_bytes(),
                    RowRole::Type,
                    true,
                )
            }
            (SemanticTypeShapeV1::Enum { variants, .. }, Some(index)) => {
                let fields = variants
                    .get(index as usize)
                    .ok_or_else(|| error("selected enum payload has no original variant"))?
                    .fields()
                    .fields();
                match declaration.layout().variants() {
                    SemanticRustcVariantsV1::Multiple(_) => {
                        let layout = self.enum_variant_layout(ty, index)?;
                        (
                            fields,
                            Some(layout.aggregate().field_offsets()),
                            layout.rustc_size_bytes(),
                            layout.alignment_bytes(),
                            RowRole::Payload(index),
                            false,
                        )
                    }
                    SemanticRustcVariantsV1::Single { index: actual } if *actual == index => {
                        let SemanticTypeLayoutDetailsV1::Aggregate(layout) =
                            declaration.layout().details()
                        else {
                            return Err(error(
                                "selected single enum payload lacks original offsets",
                            ));
                        };
                        (
                            fields,
                            Some(layout.field_offsets()),
                            declaration
                                .layout()
                                .size_bytes()
                                .ok_or(ArgumentResourceV1::Arithmetic)?,
                            declaration.layout().alignment_bytes(),
                            RowRole::Type,
                            false,
                        )
                    }
                    _ => return Err(error("selected enum payload changes original encoding")),
                }
            }
            _ => {
                return Err(error(
                    "selected field representation changes original type shape",
                ));
            }
        };
        if offsets.is_some_and(|offsets| offsets.len() != fields.len())
            || selected.len() > self.limits.edges
            || size > self.limits.object_bytes
        {
            return Err(error(
                "selected fields exceed original geometry or retained policy",
            ));
        }
        let mut output = self.lease.vector(selected.len(), budget)?;
        let mut cursor = 0;
        for (index, &field_ty) in fields.iter().enumerate() {
            self.lease.work(2, budget)?;
            let nominal =
                execution_cfg_nominal_kind_v29(self.owner.source_semantic().types(), field_ty)?
                    .is_some();
            let found = selected.get(cursor).copied();
            if found.is_some_and(|(field, _)| field as usize == index) {
                if nominal {
                    return Err(error(
                        "source-nominal identity cannot become a physical storage row",
                    ));
                }
                let (_, layout) = found.ok_or(ArgumentResourceV1::Accounting)?;
                self.selected_child(layout, field_ty, RowRole::Type, budget)?;
                let offset = offsets.map_or(0, |offsets| offsets[index]);
                self.lease
                    .push(&mut output, StorageFieldV1 { offset, layout }, budget)?;
                cursor += 1;
            } else if !nominal {
                return Err(error(
                    "selected storage schema omitted an original represented component",
                ));
            }
        }
        if cursor != selected.len() {
            return Err(error(
                "selected storage schema repeats or reorders original field paths",
            ));
        }
        let fields = exact_box(output)?;
        Ok((
            RowKey { ty, role },
            StorageLayoutV1 {
                size,
                alignment: physical_alignment(alignment)?,
                kind: if union {
                    StorageLayoutKindV1::Union(fields)
                } else {
                    StorageLayoutKindV1::Record(fields)
                },
            },
        ))
    }

    pub(super) fn select_schema(
        &self,
        owner: &ProductionSemanticSsaOwnerV1,
        ty: SemanticTypeIdV1,
        selection: SourceStorageSelectionV29<'_>,
        budget: &mut Budget<'_>,
    ) -> Result<StorageLayoutIdV1, Error> {
        let result = (|| {
            self.check_owner(owner, budget)?;
            if matches!(selection, SourceStorageSelectionV29::Original) {
                return self.row_id(RowKey::ty(ty), budget);
            }
            let _persistent = self.schema_allocation(budget)?;
            let declaration = self.declaration(ty)?;
            let (key, row) = match selection {
                SourceStorageSelectionV29::Original => {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                SourceStorageSelectionV29::Pointer {
                    pointee,
                    value_space,
                    access,
                } => (
                    RowKey::ty(ty),
                    self.selected_pointer(ty, pointee, value_space, access, false, budget)?,
                ),
                SourceStorageSelectionV29::Slice {
                    element,
                    value_space,
                    access,
                } => {
                    let data =
                        self.selected_pointer(ty, element, value_space, access, true, budget)?;
                    let data = self.intern_schema(
                        RowKey {
                            ty,
                            role: RowRole::DescriptorData,
                        },
                        data,
                        budget,
                    )?;
                    let length = self.selected_descriptor_length_row(ty, budget)?;
                    let (_, _, offset) = self.descriptor_parts(ty)?;
                    (
                        RowKey::ty(ty),
                        StorageLayoutV1 {
                            size: declaration
                                .layout()
                                .size_bytes()
                                .ok_or(ArgumentResourceV1::Arithmetic)?,
                            alignment: physical_alignment(declaration.layout().alignment_bytes())?,
                            kind: StorageLayoutKindV1::Slice {
                                element,
                                value_space,
                                access,
                                data: StorageFieldV1 {
                                    offset: 0,
                                    layout: data,
                                },
                                length: StorageFieldV1 {
                                    offset,
                                    layout: length,
                                },
                            },
                        },
                    )
                }
                SourceStorageSelectionV29::Aggregate { variant, fields } => {
                    self.selected_fields(ty, variant, fields, budget)?
                }
                SourceStorageSelectionV29::Array { element } => {
                    let SemanticTypeShapeV1::Array {
                        element: original,
                        length,
                    } = declaration.shape()
                    else {
                        return Err(error("selected array changes original source type"));
                    };
                    self.selected_child(element, *original, RowRole::Type, budget)?;
                    let SemanticFieldsShapeV1::Array {
                        stride_bytes,
                        count,
                    } = declaration.layout().fields()
                    else {
                        return Err(error("selected array lacks original stride"));
                    };
                    if count != length {
                        return Err(error("selected array changes original length"));
                    }
                    (
                        RowKey::ty(ty),
                        StorageLayoutV1 {
                            size: declaration
                                .layout()
                                .size_bytes()
                                .ok_or(ArgumentResourceV1::Arithmetic)?,
                            alignment: physical_alignment(declaration.layout().alignment_bytes())?,
                            kind: StorageLayoutKindV1::Array {
                                element,
                                length: *length,
                                stride: *stride_bytes,
                            },
                        },
                    )
                }
                SourceStorageSelectionV29::Enum { variants } => {
                    (RowKey::ty(ty), self.selected_enum(ty, variants, budget)?)
                }
            };
            self.intern_schema(key, row, budget)
        })();
        match result {
            Ok(id) => Ok(id),
            Err(first) => {
                self.lease.failure.record(first);
                Err(self
                    .lease
                    .failure
                    .first_error()
                    .unwrap_or_else(|| ArgumentResourceV1::Accounting.into()))
            }
        }
    }

    fn selected_enum(
        &self,
        ty: SemanticTypeIdV1,
        selected: &[(u32, StorageLayoutIdV1)],
        budget: &mut Budget<'_>,
    ) -> Result<StorageLayoutV1, Error> {
        self.lease.work(2, budget)?;
        let (declaration, variants) = self.enum_parts(ty)?;
        let SemanticRustcVariantsV1::Multiple(enumeration) = declaration.layout().variants() else {
            return Err(error(
                "selected variant table has no original multi-variant encoding",
            ));
        };
        if selected.len() != variants.len() || variants.len() >= self.limits.edges {
            return Err(error(
                "selected enum changes original variant roster or edge policy",
            ));
        }
        let tag_key = RowKey {
            ty,
            role: RowRole::Tag,
        };
        // Keep the original encoding and original niche path bound through ty.
        // The tag is a view of its original encoded bytes, not a new source
        // pointer value or permission to reinterpret payload provenance.
        let tag = match enumeration.encoding() {
            SemanticEnumEncodingV1::Niche(niche)
                if matches!(
                    niche.tag().primitive(),
                    SemanticBackendPrimitiveV1::Pointer { .. }
                ) =>
            {
                self.selected_niche_pointer_row(ty, selected, budget)?
            }
            _ => self.enum_auxiliary(tag_key, budget)?,
        };
        let tag_layout = self.intern_schema(tag_key, tag, budget)?;
        let (encoding, width) = match enumeration.encoding() {
            SemanticEnumEncodingV1::Direct(direct) => {
                let width = direct
                    .tag()
                    .primitive()
                    .size_bytes()
                    .and_then(|bytes| bytes.checked_mul(8))
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
        let mut output = self.lease.vector(variants.len(), budget)?;
        for (index, (original, &(variant, layout))) in variants.iter().zip(selected).enumerate() {
            self.lease.work(2, budget)?;
            if variant as usize != index {
                return Err(error("selected enum reorders original alternatives"));
            }
            self.selected_child(layout, ty, RowRole::Payload(variant), budget)?;
            let payload = self.enum_variant_layout(ty, variant)?;
            self.lease.push(
                &mut output,
                StorageVariantV1 {
                    discriminant: original.discriminant(),
                    direct_tag_bits: width.map(|bits| {
                        if bits == 128 {
                            original.discriminant()
                        } else {
                            original.discriminant() & ((1_u128 << bits) - 1)
                        }
                    }),
                    uninhabited: original.is_uninhabited() || payload.is_uninhabited(),
                    layout,
                },
                budget,
            )?;
        }
        Ok(StorageLayoutV1 {
            size: declaration
                .layout()
                .size_bytes()
                .ok_or(ArgumentResourceV1::Arithmetic)?,
            alignment: physical_alignment(declaration.layout().alignment_bytes())?,
            kind: StorageLayoutKindV1::Variants {
                encoding,
                variants: exact_box(output)?,
            },
        })
    }
}
