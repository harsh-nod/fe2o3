//! Address-transparent reads of a checked wrapper, not Matrix issuance.
use super::*;

impl<'a> ScopedCaptures<'a> {
    pub(in super::super) fn register_wrapper_references(
        &mut self,
        types: &[SemanticTypeDeclV1],
        pairs: &mut BTreeMap<SemanticTypeIdV1, SemanticTypeIdV1>,
        limit: usize,
    ) -> Result<(), ProductionSemanticSsaErrorV1> {
        if self.wrapper_types.is_empty() {
            return Ok(());
        }
        // Retained type slice and exact view for reads in the existing use walk.
        self.charge(3, limit)?;
        // Bind has no receiver-reference slot. The owner remains the wrapper.
        for (index, declaration) in types.iter().enumerate() {
            self.charge(1, limit)?;
            let SemanticTypeShapeV1::Pointer(pointer) = declaration.shape() else {
                continue;
            };
            self.charge(8 + key_work(self.wrapper_types.len()), limit)?;
            let reference = SemanticTypeIdV1::from_index(index as u32);
            let owned = pointer.pointee();
            if !self.wrapper_types.contains(&owned) || !shared(types, reference, owned) {
                continue;
            }
            self.charge(key_work(pairs.len()) + 2, limit)?;
            if pairs
                .insert(reference, owned)
                .is_some_and(|old| old != owned)
            {
                return Err(mismatch());
            }
        }
        Ok(())
    }

    pub(in super::super) fn copied_matrix_field(
        &self,
        types: &[SemanticTypeDeclV1],
        view: &SemanticExpandedRootV1,
        site: SemanticTransparentBorrowSiteV1,
        kind: &SemanticStatementKindV1,
        charge: &mut impl FnMut(usize) -> Result<(), ProductionSemanticSsaErrorV1>,
    ) -> Result<Option<[u32; 1]>, ProductionSemanticSsaErrorV1> {
        charge(1)?;
        let SemanticStatementKindV1::Assign(assignment) = kind else {
            return Ok(None);
        };
        let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)) = assignment.value().kind()
        else {
            return Ok(None);
        };
        let [deref, field] = place.projections() else {
            return Ok(None);
        };
        charge(4)?;
        if deref.kind() != SemanticProjectionKindV1::Dereference
            || field.kind() != SemanticProjectionKindV1::Field(0)
        {
            return Ok(None);
        }
        let owned = deref.result_type();
        charge(key_work(self.wrapper_types.len()))?;
        if !self.wrapper_types.contains(&owned) {
            return Ok(None);
        }
        charge(16 + key_work(self.matrix_leaves.len()))?;
        if field.result_type() != place.ty()
            || !self.matrix_leaves.contains_key(&place.ty())
            || !assignment.destination().projections().is_empty()
            || assignment.destination().ty() != place.ty()
            || assignment.value().result_type() != place.ty()
            || view
                .body()
                .locals()
                .get(place.local().index() as usize)
                .is_none_or(|local| !shared(types, local.ty(), owned))
            || view
                .body()
                .locals()
                .get(assignment.destination().local().index() as usize)
                .is_none_or(|local| local.ty() != place.ty())
        {
            return Ok(None);
        }
        let Some(SemanticTypeShapeV1::Aggregate(fields)) = types
            .get(owned.index() as usize)
            .map(SemanticTypeDeclV1::shape)
        else {
            return Err(mismatch());
        };
        if fields.fields().len() != 4 || fields.fields()[0] != place.ty() {
            return Ok(None);
        }
        if !std::ptr::eq(original_assignment(view, site)?, assignment) {
            return Ok(None);
        }
        // A shared field points to Matrix, not to the wrapper. It is a read
        // sink, never a Bound-to-Matrix alias, new Borrow or stored capture.
        Ok(Some([place.local().index()]))
    }
}
