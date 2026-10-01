// A shared reborrow may reconstruct the original witness recipe only to capture
// the child loan. Ordinary dereference Copy/Move never gains an owned witness.
fn source_index_reborrow_headers_v43() -> Result<usize, ArgumentResourceV1> {
    source_reference_emission_headers_v29::<(
        Option<SourceIndexWitnessBorrowV29>,
        Option<SemanticValueBindingV1>,
        SourceReferenceSiteV29,
        Option<usize>,
        ValueDef,
        Option<SemanticCapabilityAvailabilityV1>,
        [&'static (); 8],
        [usize; 8],
    )>()
}

fn source_index_reborrow_proof_v43(
    references: &SourceReferenceEmissionV29<'_, '_>,
    site: SourceReferenceSiteV29,
    place: &SemanticPlaceV1,
    loan: usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Option<SourceIndexWitnessBorrowV29>, ProductionSemanticKirErrorV1> {
    references.check(budget)?;
    budget.source_reference_reserve_v29(references.plan, source_index_reborrow_headers_v43()?)?;
    budget.source_reference_charge_v29(references.plan, 24)?;
    let plan = references.plan;
    let record = plan
        .loans
        .get(loan)
        .ok_or_else(source_index_witness_error_v29)?;
    let Some(parent) = record.parent else {
        return Ok(None);
    };
    let Some(proof) = references
        .index_witnesses
        .get(parent)
        .and_then(std::cell::Cell::get)
    else {
        return Ok(None);
    };
    let parent_record = plan
        .loans
        .get(parent)
        .ok_or_else(source_index_witness_error_v29)?;
    let origin = plan
        .origins
        .get(record.origin)
        .ok_or_else(source_index_witness_error_v29)?;
    let function = plan
        .instances
        .instance(site.instance)
        .ok_or_else(source_index_witness_error_v29)?
        .declaration();
    let Some(statement) = site.statement.and_then(|ordinal| {
        function
            .blocks()
            .get(site.block.index() as usize)
            .and_then(|block| block.statements().get(ordinal))
    }) else {
        return Err(source_index_witness_error_v29());
    };
    let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
        return Err(source_index_witness_error_v29());
    };
    let SemanticRvalueKindV1::Borrow {
        kind: SemanticBorrowKindV1::Shared,
        place: original,
    } = assignment.value().kind()
    else {
        return Err(source_index_witness_error_v29());
    };
    let types = plan.instances.owner().source_semantic().types();
    let root_type = function
        .locals()
        .get(place.local().index() as usize)
        .map(|local| local.ty())
        .ok_or_else(source_index_witness_error_v29)?;
    if !std::ptr::eq(place, original)
        || record.site != site
        || record.source_type != assignment.value().result_type()
        || record.kind != SemanticBorrowKindV1::Shared
        || parent_record.kind != SemanticBorrowKindV1::Shared
        || record.representation != SourceReferenceRepresentationV29::StableReferent
        || parent_record.representation != SourceReferenceRepresentationV29::StableReferent
        || record.origin != parent_record.origin
        || proof.origin != record.origin
        || proof.site != parent_record.site
        || proof.ty != origin.ty
        || proof.ty != place.ty()
        || !origin.projections.is_empty()
        || !references
            .claimed
            .get(parent)
            .is_some_and(std::cell::Cell::get)
        || !matches!(place.projections(), [projection]
            if projection.kind() == SemanticProjectionKindV1::Dereference && projection.result_type() == proof.ty)
        || !matches!(types.get(root_type.index() as usize).map(|ty| ty.shape()),
            Some(SemanticTypeShapeV1::Pointer(pointer))
                if pointer.kind() == SemanticPointerKindV1::Reference
                    && pointer.mutability() == SemanticMutabilityV1::Immutable
                    && pointer.metadata() == SemanticPointerMetadataV1::None
                    && pointer.pointee() == proof.ty)
    {
        return Err(source_index_witness_error_v29());
    }
    Ok(Some(SourceIndexWitnessBorrowV29 { site, ..proof }))
}

impl SourceReferenceEmissionV29<'_, '_> {
    #[allow(clippy::too_many_arguments)]
    fn reborrow_index_referent_v43(
        &self,
        binding: &SemanticSourceReferenceBindingV29,
        source_type: SemanticTypeIdV1,
        site: SourceReferenceSiteV29,
        source: &SemanticPlaceV1,
        projection: usize,
        access: SourceReferenceAccessV29,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<Option<SemanticValueBindingV1>, ProductionSemanticKirErrorV1> {
        self.check(budget)?;
        source_reference_validate_binding_v29(self.plan, binding, budget)?;
        let parent = binding.origin.single_loan()?;
        budget.source_reference_charge_v29(self.plan, 4)?;
        if self
            .index_witnesses
            .get(parent)
            .and_then(std::cell::Cell::get)
            .is_none()
        {
            return Ok(None);
        }
        if access != SourceReferenceAccessV29::Borrow(SemanticBorrowKindV1::Shared)
            || projection != 0
        {
            return Err(source_index_witness_error_v29());
        }
        let loan = self
            .loan_at(site, budget)?
            .ok_or_else(source_index_witness_error_v29)?;
        if self.plan.loans.get(loan).and_then(|loan| loan.parent) != Some(parent) {
            return Err(source_index_witness_error_v29());
        }
        let proof = source_index_reborrow_proof_v43(self, site, source, loan, budget)?
            .ok_or_else(source_index_witness_error_v29)?;
        let id = self.index_reader_value_v29(
            binding,
            source_type,
            (proof.ty, proof.index_space, proof.disjoint),
            budget,
        )?;
        Ok(Some(SemanticValueBindingV1::IndexWitness {
            id,
            index_space: proof.index_space,
            disjoint: proof.disjoint,
            availability: proof.availability,
        }))
    }
}

fn source_reference_index_reborrow_values_v43(
    cursor: &ExecutionAvailabilityV29<'_>,
    place: &SemanticPlaceV1,
    loan: usize,
    referent: &SemanticValueBindingV1,
    values: &mut Vec<ValueDef>,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Option<SourceIndexWitnessBorrowV29>, ProductionSemanticKirErrorV1> {
    let references = cursor
        .references
        .ok_or_else(source_index_witness_error_v29)?;
    references.check(budget)?;
    budget.source_reference_charge_v29(references.plan, 3)?;
    let record = references
        .plan
        .loans
        .get(loan)
        .ok_or_else(source_index_witness_error_v29)?;
    if record.parent.is_none() {
        return Ok(None);
    }
    if record.site.instance != cursor.instance {
        return Err(source_index_witness_error_v29());
    }
    let Some(proof) =
        source_index_reborrow_proof_v43(references, record.site, place, loan, budget)?
    else {
        return Ok(None);
    };
    let SemanticValueBindingV1::IndexWitness {
        id,
        index_space,
        disjoint,
        availability,
    } = referent
    else {
        return Err(source_index_witness_error_v29());
    };
    if !values.is_empty()
        || (*index_space, *disjoint, *availability)
            != (proof.index_space, proof.disjoint, proof.availability)
    {
        return Err(source_index_witness_error_v29());
    }
    emission_push_v1(values, ValueDef::new(*id, Type::INDEX), budget)?;
    Ok(Some(proof))
}
