// This selects an original effect, not a physical pointer permission. The
// normal source read resolves its origins and state before the retained join.
fn check_source_raw_volatile_load_v29(
    instances: &ExecutionInstancesV29<'_>,
    site: SourceReferenceSiteV29,
    load: &fe2o3_mir_model::semantic_mir_v1::SemanticMemoryLoadV1,
    result: SemanticTypeIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(12)?;
    let refuse = || {
        source_reference_error_v29(
            "source reference ordered load requires checked addressable effects",
        )
    };
    let original = instances
        .instance(site.instance)
        .ok_or_else(refuse)?
        .declaration();
    let statement = original
        .blocks()
        .get(site.block.index() as usize)
        .and_then(|block| block.statements().get(site.statement?))
        .ok_or_else(refuse)?;
    let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
        return Err(refuse());
    };
    let SemanticRvalueKindV1::Load(expected) = assignment.value().kind() else {
        return Err(refuse());
    };
    let shape = instances
        .owner()
        .source_semantic()
        .types()
        .get(result.index() as usize)
        .map(SemanticTypeDeclV1::shape)
        .ok_or_else(refuse)?;
    if !std::ptr::eq(load, expected)
        || assignment.value().result_type() != result
        || result != load.source().ty()
        || load.source().projections().is_empty()
        || load.volatility() != SemanticVolatilityV1::Volatile
        || load.atomic().is_some()
        || !matches!(
            shape,
            SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_)
        )
    {
        return Err(refuse());
    }
    Ok(())
}

impl SourceReferencePlanV29<'_, '_> {
    fn check_resolved_raw_volatile_load_v29(
        &self,
        site: SourceReferenceSiteV29,
        load: &fe2o3_mir_model::semantic_mir_v1::SemanticMemoryLoadV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.check_owner(self.instances, budget)?;
        let result = (|| {
            // Recheck the borrowed original at this query boundary; an inert
            // access row cannot authenticate a different load.
            check_source_raw_volatile_load_v29(
                self.instances,
                site,
                load,
                load.source().ty(),
                budget,
            )?;
            let source = load.source();
            self.charge(argument_sum_v1(&[source.projections().len(), 10])?, budget)?;
            let refuse = || {
                source_reference_error_v29(
                    "source ordered raw read lacks its exact resolved access",
                )
            };
            let projection = source
                .projections()
                .iter()
                .rposition(|part| part.kind() == SemanticProjectionKindV1::Dereference)
                .ok_or_else(refuse)?;
            let key = source_reference_access_key_v29(site, source, SourceReferenceAccessV29::Read);
            charge_execution_cfg_lookup_v29(self.raw_accesses.len(), budget)?;
            let row = self.raw_accesses.get(&(key, projection)).ok_or_else(|| {
                source_reference_error_v29(
                    "source reference ordered load requires checked addressable effects",
                )
            })?;
            if row.site != site
                || row.source != source as *const SemanticPlaceV1 as usize
                || row.access != SourceReferenceAccessV29::Read
                || row.crossing != SourceReferenceAccessV29::Read
                || row.projection != projection
                || row.pointee != source.projections()[projection].result_type()
                || row.ty != source.ty()
                || self
                    .raw_sets
                    .get(row.set)
                    .is_none_or(|set| set.ty != row.pointee || set.count == 0)
            {
                return Err(refuse());
            }
            Ok(())
        })();
        result.inspect_err(|error| source_reference_record_failure_v29(self, error))
    }
}
