// Exact original-source geometry only; no initializedness or active-root authority.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct SourceStorageRangeV29 {
    start: u64,
    end: u64,
}

impl SourceStorageRangeV29 {
    pub(super) fn new(start: u64, length: u64, extent: u64) -> Result<Self, Error> {
        let range = Self {
            start,
            end: start
                .checked_add(length)
                .ok_or(ArgumentResourceV1::Arithmetic)?,
        };
        range.check(extent)?;
        Ok(range)
    }
    fn check(self, extent: u64) -> Result<(), Error> {
        if self.start > self.end || self.end > extent {
            return Err(error("source storage range is outside its original object"));
        }
        Ok(())
    }
    fn length(self) -> u64 {
        self.end - self.start
    }
    fn overlaps(self, other: Self) -> bool {
        self.start < self.end
            && other.start < other.end
            && self.start < other.end
            && other.start < self.end
    }
    fn contains(self, other: Self) -> bool {
        self.start <= other.start && other.end <= self.end
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum SubobjectStep {
    Field(u32),
    Element(u64),
    Variant(u32),
    Selection(usize),
}

// Paths retain logical identity even when zero-sized fields have equal addresses.
pub(super) struct SourceStorageSubobjectV29<'layout, 'source> {
    layouts: &'layout SourceStorageLayoutsV29<'source>,
    root: SemanticTypeIdV1,
    ty: SemanticTypeIdV1,
    variant: Option<u32>,
    initialization_floor: usize,
    selected: bool,
    path: Vec<SubobjectStep>,
    range: SourceStorageRangeV29,
}

impl<'layout, 'source> SourceStorageSubobjectV29<'layout, 'source> {
    fn copy(&self, budget: &mut Budget<'_>) -> Result<Self, Error> {
        self.layouts.lease.reserve(size_of::<Self>(), budget)?;
        let mut path = self.layouts.lease.vector(self.path.len(), budget)?;
        self.layouts.lease.work(self.path.len(), budget)?;
        path.extend_from_slice(&self.path);
        Ok(Self {
            layouts: self.layouts,
            root: self.root,
            ty: self.ty,
            variant: self.variant,
            initialization_floor: self.initialization_floor,
            selected: self.selected,
            path,
            range: self.range,
        })
    }

    pub(super) fn discard(self, budget: &mut Budget<'_>) -> Result<(), Error> {
        let layouts = self.layouts;
        layouts.lease.discard_vec(self.path, budget)?;
        layouts.lease.refund(size_of::<Self>(), budget)
    }
}

impl SourceStorageSubobjectV29<'_, '_> {
    fn has_selection(&self, _budget: &mut Budget<'_>) -> Result<bool, Error> {
        Ok(self.selected)
    }
}

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
        let size = self
            .declaration(ty)?
            .layout()
            .size_bytes()
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

#[cfg(test)]
mod geometry_tests {
    use super::layout_tests::*;
    use super::*;
    include!("production_source_storage_geometry_v29_tests.rs");
}
