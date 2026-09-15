//! Field reads of initialized private scalar data, using the existing flow.
//! No geometry, capability, allocation, or scalar-expression authority is issued.

use super::*;

impl Analysis<'_> {
    pub(super) fn aggregate_candidate<'s>(
        &self,
        statement: &'s SemanticStatementV1,
    ) -> Option<&'s SemanticPlaceV1> {
        let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
            return None;
        };
        let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)) = assignment.value().kind()
        else {
            return None;
        };
        let destination = assignment.destination();
        if !destination.projections().is_empty()
            || self
                .function()
                .locals()
                .get(destination.local().index() as usize)?
                .ty()
                != destination.ty()
            || destination.ty() != place.ty()
            || assignment.value().result_type() != place.ty()
        {
            return None;
        }
        self.aggregate_read_shape(place)?;
        Some(place)
    }

    fn aggregate_read_shape(&self, place: &SemanticPlaceV1) -> Option<(SemanticTypeIdV1, bool)> {
        let (dereference, fields) = place.projections().split_first()?;
        if fields.is_empty()
            || fields.len() > MAX_DEPTH
            || dereference.kind() != SemanticProjectionKindV1::Dereference
        {
            return None;
        }
        let base = self
            .function()
            .locals()
            .get(place.local().index() as usize)?
            .ty();
        let root = dereference.result_type();
        let (_, mutable) = self.reference_type(base)?;
        let mutability = if mutable {
            SemanticMutabilityV1::Mutable
        } else {
            SemanticMutabilityV1::Immutable
        };
        if !is_exact_reference_to_v1(self.types, base, root, mutability) {
            return None;
        }
        let mut ty = root;
        for projection in fields {
            let SemanticProjectionKindV1::Field(field) = projection.kind() else {
                return None;
            };
            ty = self.field_type(ty, field)?;
            if ty != projection.result_type() {
                return None;
            }
        }
        (ty == place.ty()).then_some((root, mutable))
    }

    pub(super) fn aggregate_read_reference(
        &mut self,
        place: &SemanticPlaceV1,
        flow: &Flow,
    ) -> Result<Option<Reference>> {
        self.budget.charge(place.projections().len() + 1)?;
        let Some((root_ty, mutable)) = self.aggregate_read_shape(place) else {
            return Ok(None);
        };
        let Some(Value::Reference(reference)) = flow
            .values
            .get_metered(place.local().index(), &mut self.budget)?
        else {
            return Ok(None);
        };
        if reference.mutable != mutable
            || flow.escaped.contains(&reference.target)
            || flow.dead.contains(&reference.target)
            || self
                .function()
                .locals()
                .get(reference.target as usize)
                .is_none_or(|local| {
                    local.ty() != root_ty
                        || matches!(local.role(), SemanticLocalRoleV1::Argument(_))
                })
        {
            return Ok(None);
        }
        if mutable && !self.original_mutable_aggregate_borrow(*reference, root_ty)? {
            return Ok(None);
        }
        let Some(mut value) = flow
            .values
            .get_metered(reference.target, &mut self.budget)?
        else {
            return Ok(None);
        };
        let mut ty = root_ty;
        for projection in &place.projections()[1..] {
            self.budget.charge(1)?;
            let SemanticProjectionKindV1::Field(field) = projection.kind() else {
                return Ok(None);
            };
            match value {
                Value::Fields(values) => {
                    let Some(types) = self.aggregate_fields(ty) else {
                        return Ok(None);
                    };
                    if types.len() != values.len() || values.len() > MAX_FIELDS {
                        return Ok(None);
                    }
                    let Some(Some(selected)) = values.get(field as usize) else {
                        return Ok(None);
                    };
                    value = selected;
                }
                // A join can erase partial aggregate detail into Opaque. Only
                // retained field construction proves this selected path.
                Value::Opaque | Value::Reference(_) | Value::Variants { .. } => return Ok(None),
            }
            ty = projection.result_type();
        }
        Ok(self
            .initialized_scalar_data(ty, value, 0)?
            .then_some(*reference))
    }

    fn original_mutable_aggregate_borrow(
        &mut self,
        reference: Reference,
        root: SemanticTypeIdV1,
    ) -> Result<bool> {
        // Recheck only the retained defining site; no alternate alias search or
        // loan issuance. Existing transfer/meet invalidation owns its lifetime.
        self.budget.charge(1)?;
        let site = reference.borrow;
        if !self.checked_site(site) {
            return Ok(false);
        }
        let Some(statement) = self
            .function()
            .blocks()
            .get(site.block)
            .and_then(|block| block.statements().get(site.statement))
        else {
            return Ok(false);
        };
        let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
            return Ok(false);
        };
        let SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Mutable,
            place,
        } = assignment.value().kind()
        else {
            return Ok(false);
        };
        let destination = assignment.destination();
        Ok(place.projections().is_empty()
            && place.local().index() == reference.target
            && place.ty() == root
            && destination.projections().is_empty()
            && self
                .function()
                .locals()
                .get(destination.local().index() as usize)
                .is_some_and(|local| local.ty() == destination.ty())
            && destination.ty() == assignment.value().result_type()
            && is_exact_reference_to_v1(
                self.types,
                destination.ty(),
                root,
                SemanticMutabilityV1::Mutable,
            ))
    }

    fn aggregate_fields(&self, ty: SemanticTypeIdV1) -> Option<&[SemanticTypeIdV1]> {
        let declaration = self.types.get(ty.index() as usize)?;
        if declaration.layout().is_uninhabited() || declaration.layout().size_bytes()? == 0 {
            return None;
        }
        match declaration.shape() {
            SemanticTypeShapeV1::Aggregate(fields) | SemanticTypeShapeV1::Tuple(fields) => {
                Some(fields.fields())
            }
            _ => None,
        }
    }

    fn initialized_scalar_data(
        &mut self,
        ty: SemanticTypeIdV1,
        value: &Value,
        depth: usize,
    ) -> Result<bool> {
        self.budget.charge(1)?;
        if self.scalar(ty) {
            return Ok(matches!(value, Value::Opaque));
        }
        if depth == MAX_DEPTH {
            return Ok(false);
        }
        let Some(fields) = self.aggregate_fields(ty) else {
            return Ok(false);
        };
        let count = fields.len();
        if count == 0 || count > MAX_FIELDS {
            return Ok(false);
        }
        let Value::Fields(values) = value else {
            return Ok(false);
        };
        if values.len() != count {
            return Ok(false);
        }
        for index in 0..count {
            // Do not clone the value tree or allocate a second proof domain.
            let field_ty = self.aggregate_fields(ty).ok_or(())?[index];
            let field_value = match &values[index] {
                Some(value) => value,
                None => return Ok(false),
            };
            if !self.initialized_scalar_data(field_ty, field_value, depth + 1)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

#[cfg(test)]
mod flow_tests {
    use super::*;
    include!("aggregate_read_flow_tests.rs");
}

#[cfg(test)]
mod exclusive_flow_tests {
    use super::*;
    include!("exclusive_aggregate_flow_tests.rs");
}
