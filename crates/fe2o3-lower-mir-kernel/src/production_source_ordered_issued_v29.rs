// Selection authenticates an original ordered effect, not a reference loan or
// physical access. The caller still performs its ordinary state transfer once.
impl SourceReferencePlanV29<'_, '_> {
    fn ordered_issued_effect(
        &self,
        site: SourceReferenceSiteV29,
        source: &SemanticPlaceV1,
        role: ExecutionOperandV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        budget.source_reference_owner_v29(self)?;
        let result = source_ordered_issued_effect_v29(self.instances, site, source, role, budget);
        if let Err(error) = &result {
            source_reference_record_failure_v29(self, error);
        }
        result
    }
}

fn source_ordered_issued_effect_v29(
    instances: &ExecutionInstancesV29<'_>,
    site: SourceReferenceSiteV29,
    source: &SemanticPlaceV1,
    role: ExecutionOperandV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    with_canonical_call_scratch_v1(budget, |budget| {
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of::<SourceReferenceSiteV29>(),
            std::mem::size_of::<ExecutionSiteV29>(),
            std::mem::size_of::<ExecutionOperandV29>(),
            std::mem::size_of::<&ExecutionInstancesV29<'_>>(),
            std::mem::size_of::<&SemanticPlaceV1>(),
            std::mem::size_of::<&SemanticFunctionDeclV1>(),
            std::mem::size_of::<Option<usize>>(),
            std::mem::size_of::<Option<&SemanticStatementKindV1>>(),
            std::mem::size_of::<&fe2o3_mir_model::semantic_mir_v1::SemanticLocalDeclV1>(),
            std::mem::size_of::<Option<&fe2o3_mir_model::semantic_mir_v1::SemanticLocalDeclV1>>(),
            std::mem::size_of::<Option<&fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1>>(),
            3 * std::mem::size_of::<Result<bool, ProductionSemanticKirErrorV1>>(),
            std::mem::size_of::<
                Result<Result<bool, ProductionSemanticKirErrorV1>, Box<dyn std::any::Any + Send>>,
            >(),
        ])?)?;
        budget.charge_work(8)?;
        let Some(statement) = site.statement else {
            return Ok(false);
        };
        let execution = execution_site_v29(
            site.block,
            Some(u32::try_from(statement).map_err(|_| ArgumentResourceV1::Arithmetic)?),
        );
        let function = instances
            .instance(site.instance)
            .ok_or_else(source_issued_error_v29)?
            .declaration();
        if !source_descriptor_volatile_source_v29(function, execution, source, role, budget)?
            || source.projections().len() != 1
            || source.projections()[0].kind() != SemanticProjectionKindV1::Dereference
        {
            return Ok(false);
        }
        let local = function
            .locals()
            .get(source.local().index() as usize)
            .ok_or_else(source_issued_error_v29)?;
        if !matches!(instances.owner().source_semantic().types().get(local.ty().index() as usize)
            .map(|ty| ty.shape()), Some(SemanticTypeShapeV1::Pointer(pointer))
                if pointer.kind() == SemanticPointerKindV1::Reference
                    && pointer.metadata() == SemanticPointerMetadataV1::None)
        {
            return Ok(false);
        }
        let mut original = SourceIssuedSemanticV29::new(
            instances,
            site.instance,
            SourceIssuedReplayModeV29::SourceOnly,
            budget,
        )?;
        let effect = original.effect(execution, role, source, budget)?;
        Ok(effect.is_some_and(|effect| effect.volatile))
    })
}
