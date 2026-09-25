use fe2o3_mir_model::semantic_mir_v1::{
    SemanticEnumEncodingV1, SemanticFieldsShapeV1, SemanticProjectionV1, SemanticRustcVariantsV1,
};

#[derive(Clone, Copy)]
enum PartialMoveGeometryStepV1 {
    Field(u32),
    Downcast(u32),
    Index {
        offset: u64,
        from_end: bool,
        minimum_length: Option<u64>,
    },
    Unsupported,
}

impl PartialMoveGeometryStepV1 {
    fn projection(projection: &SemanticProjectionV1) -> Self {
        match projection.kind() {
            SemanticProjectionKindV1::Field(field) => Self::Field(field),
            SemanticProjectionKindV1::Downcast(variant) => Self::Downcast(variant),
            SemanticProjectionKindV1::ConstantIndex {
                offset,
                from_end,
                minimum_length,
            } => Self::Index {
                offset,
                from_end,
                minimum_length: Some(minimum_length),
            },
            _ => Self::Unsupported,
        }
    }

    fn moved(element: &SemanticMovePathElementV1) -> Self {
        match *element {
            SemanticMovePathElementV1::Field(field) => Self::Field(field),
            SemanticMovePathElementV1::Downcast(variant) => Self::Downcast(variant),
            SemanticMovePathElementV1::ConstantIndex { offset, from_end } => Self::Index {
                offset,
                from_end,
                minimum_length: None,
            },
        }
    }
}

// A borrowed original-layout cursor, never a selected representation or an
// initialization proof. Its work is proportional to paths, not array lengths.
#[derive(Clone, Copy)]
struct PartialMoveGeometryV1 {
    ty: SemanticTypeIdV1,
    start: u64,
    size: u64,
    root_size: u64,
    variant: Option<u32>,
}

impl PartialMoveGeometryV1 {
    fn root(
        function: &SemanticFunctionDeclV1,
        types: &[SemanticTypeDeclV1],
        local: u32,
    ) -> Option<Self> {
        let ty = function.locals().get(local as usize)?.ty();
        let layout = types.get(ty.index() as usize)?.layout();
        if layout.is_uninhabited() {
            return None;
        }
        let size = layout.size_bytes()?;
        Some(Self {
            ty,
            start: 0,
            size,
            root_size: size,
            variant: None,
        })
    }

    fn child(
        self,
        types: &[SemanticTypeDeclV1],
        ty: SemanticTypeIdV1,
        offset: u64,
    ) -> Option<Self> {
        let layout = types.get(ty.index() as usize)?.layout();
        if layout.is_uninhabited() {
            return None;
        }
        let size = layout.size_bytes()?;
        if offset.checked_add(size)? > self.size {
            return None;
        }
        let start = self.start.checked_add(offset)?;
        if start.checked_add(size)? > self.root_size {
            return None;
        }
        Some(Self {
            ty,
            start,
            size,
            root_size: self.root_size,
            variant: None,
        })
    }

    fn step(self, types: &[SemanticTypeDeclV1], step: PartialMoveGeometryStepV1) -> Option<Self> {
        let declaration = types.get(self.ty.index() as usize)?;
        match step {
            PartialMoveGeometryStepV1::Downcast(index) if self.variant.is_none() => {
                let SemanticTypeShapeV1::Enum { variants, .. } = declaration.shape() else {
                    return None;
                };
                let logical = variants.get(index as usize)?;
                if logical.is_uninhabited() {
                    return None;
                }
                let size = match declaration.layout().variants() {
                    SemanticRustcVariantsV1::Multiple(layout) => {
                        let variant = layout.variants().get(index as usize)?;
                        if variant.variant_index() != index || variant.is_uninhabited() {
                            return None;
                        }
                        variant.rustc_size_bytes()
                    }
                    SemanticRustcVariantsV1::Single { index: selected } if *selected == index => {
                        self.size
                    }
                    _ => return None,
                };
                if size > self.size {
                    return None;
                }
                Some(Self {
                    size,
                    variant: Some(index),
                    ..self
                })
            }
            PartialMoveGeometryStepV1::Field(index) => {
                let (fields, offsets) = match (declaration.shape(), self.variant) {
                    (
                        SemanticTypeShapeV1::Tuple(aggregate)
                        | SemanticTypeShapeV1::Aggregate(aggregate),
                        None,
                    ) => {
                        let SemanticTypeLayoutDetailsV1::Aggregate(layout) =
                            declaration.layout().details()
                        else {
                            return None;
                        };
                        (aggregate.fields(), layout.field_offsets())
                    }
                    (SemanticTypeShapeV1::Enum { variants, .. }, Some(variant)) => {
                        let fields = variants.get(variant as usize)?.fields().fields();
                        let offsets = match declaration.layout().variants() {
                            SemanticRustcVariantsV1::Multiple(layout) => {
                                let row = layout.variants().get(variant as usize)?;
                                if row.variant_index() != variant || row.is_uninhabited() {
                                    return None;
                                }
                                row.aggregate().field_offsets()
                            }
                            SemanticRustcVariantsV1::Single { index } if *index == variant => {
                                let SemanticTypeLayoutDetailsV1::Aggregate(layout) =
                                    declaration.layout().details()
                                else {
                                    return None;
                                };
                                layout.field_offsets()
                            }
                            _ => return None,
                        };
                        (fields, offsets)
                    }
                    _ => return None,
                };
                if fields.len() != offsets.len() {
                    return None;
                }
                self.child(
                    types,
                    *fields.get(index as usize)?,
                    *offsets.get(index as usize)?,
                )
            }
            PartialMoveGeometryStepV1::Index {
                offset,
                from_end,
                minimum_length,
            } if self.variant.is_none() => {
                let SemanticTypeShapeV1::Array { element, length } = declaration.shape() else {
                    return None;
                };
                let SemanticFieldsShapeV1::Array {
                    stride_bytes,
                    count,
                } = declaration.layout().fields()
                else {
                    return None;
                };
                if count != length || minimum_length.is_some_and(|minimum| minimum > *length) {
                    return None;
                }
                let index = if from_end {
                    length.checked_sub(offset)?
                } else {
                    offset
                };
                if index >= *length {
                    return None;
                }
                let element_size = types.get(element.index() as usize)?.layout().size_bytes()?;
                if element_size > *stride_bytes || stride_bytes.checked_mul(*length)? != self.size {
                    return None;
                }
                self.child(types, *element, index.checked_mul(*stride_bytes)?)
            }
            _ => None,
        }
    }

    fn tag(self, types: &[SemanticTypeDeclV1]) -> Option<(u64, u64)> {
        if self.variant.is_some() {
            return None;
        }
        let declaration = types.get(self.ty.index() as usize)?;
        let SemanticTypeShapeV1::Enum { variants, .. } = declaration.shape() else {
            return None;
        };
        if declaration.layout().is_uninhabited() || declaration.layout().size_bytes()? != self.size
        {
            return None;
        }
        let SemanticRustcVariantsV1::Multiple(layout) = declaration.layout().variants() else {
            return None;
        };
        if variants.len() < 2 || variants.len() != layout.variants().len() {
            return None;
        }
        let (offset, primitive) = match layout.encoding() {
            SemanticEnumEncodingV1::Direct(direct) => {
                if direct.tag_field() != 0 || direct.tag_offset_bytes() != 0 {
                    return None;
                }
                (direct.tag_offset_bytes(), direct.tag().primitive())
            }
            SemanticEnumEncodingV1::Niche(niche) => {
                if niche.tag_field() != 0
                    || niche.source().expected_offset_bytes() != niche.source_niche().offset_bytes()
                    || niche.source_niche().primitive() != niche.tag().primitive()
                {
                    return None;
                }
                (
                    niche.source().expected_offset_bytes(),
                    niche.tag().primitive(),
                )
            }
        };
        let width = primitive.size_bytes()?;
        if width == 0 || offset.checked_add(width)? > self.size {
            return None;
        }
        let start = self.start.checked_add(offset)?;
        let end = start.checked_add(width)?;
        if end > self.root_size {
            return None;
        }
        Some((start, end))
    }
}

fn partial_move_query_geometry_v1(
    function: &SemanticFunctionDeclV1,
    types: &[SemanticTypeDeclV1],
    place: &SemanticPlaceV1,
    budget: &mut SemanticPartialMoveBudgetV1,
) -> Result<Option<PartialMoveGeometryV1>, ProductionSemanticSsaErrorV1> {
    budget.charge_work()?;
    let Some(mut cursor) = PartialMoveGeometryV1::root(function, types, place.local().index())
    else {
        return Ok(None);
    };
    for projection in place.projections() {
        budget.charge_work()?;
        let Some(next) = cursor.step(types, PartialMoveGeometryStepV1::projection(projection))
        else {
            return Ok(None);
        };
        if next.ty != projection.result_type() {
            return Ok(None);
        }
        cursor = next;
    }
    Ok((cursor.variant.is_none() && cursor.ty == place.ty()).then_some(cursor))
}

fn validate_partial_move_discriminant_read_v1(
    function: &SemanticFunctionDeclV1,
    types: Option<&[SemanticTypeDeclV1]>,
    place: &SemanticPlaceV1,
    location: SemanticPartialMoveLocationV1,
    state: &SemanticPartialMoveStateV1,
    budget: &mut SemanticPartialMoveBudgetV1,
) -> Result<(), ProductionSemanticSsaErrorV1> {
    let local = place.local().index();
    let Some(moved) = state.get(&local) else {
        return validate_partial_move_projection_indices_v1(place, location, state, budget);
    };
    budget.charge_work()?;
    let tag = if let Some(types) = types {
        let geometry = partial_move_query_geometry_v1(function, types, place, budget)?;
        budget.charge_work()?;
        geometry.and_then(|cursor| cursor.tag(types))
    } else {
        None
    };
    let (Some(types), Some((tag_start, tag_end))) = (types, tag) else {
        // Unknown geometry and constant/no-tag enums keep the old availability
        // rule. Absence of an encoded tag is not permission to read a moved value.
        return validate_partial_move_place_read_v1(
            function, types, place, location, state, budget,
        );
    };
    let unavailable = || {
        partial_move_error_v1(
            location,
            local,
            SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
        )
    };
    for path in moved {
        budget.charge_work()?;
        if path.is_empty() {
            return Err(unavailable());
        }
        budget.charge_work()?;
        let mut cursor =
            PartialMoveGeometryV1::root(function, types, local).ok_or_else(unavailable)?;
        for element in path {
            budget.charge_work()?;
            cursor = cursor
                .step(types, PartialMoveGeometryStepV1::moved(element))
                .ok_or_else(unavailable)?;
        }
        if cursor.variant.is_some() {
            return Err(unavailable());
        }
        budget.charge_work()?;
        let end = cursor
            .start
            .checked_add(cursor.size)
            .ok_or_else(unavailable)?;
        if cursor.size != 0 && cursor.start < tag_end && tag_start < end {
            return Err(unavailable());
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "partial_move_discriminants_v1_tests.rs"]
mod partial_move_discriminants_v1_tests;
