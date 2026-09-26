// A direct scalar local cannot transport pointer provenance or external memory.
// Its initialized storage and physical effect are still checked by the normal
// read, payload, and final-memory paths. Ordered pointer/indirect reads remain
// outside this small source-reference transfer rule.
fn check_source_direct_volatile_load_v29(
    instances: &ExecutionInstancesV29<'_>,
    site: SourceReferenceSiteV29,
    load: &fe2o3_mir_model::semantic_mir_v1::SemanticMemoryLoadV1,
    result: SemanticTypeIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(12)?;
    let refuse = || source_reference_error_v29(
        "source reference ordered load requires checked addressable effects");
    let original = instances.instance(site.instance).ok_or_else(refuse)?.declaration();
    let statement = original.blocks().get(site.block.index() as usize)
        .and_then(|block| block.statements().get(site.statement?))
        .ok_or_else(refuse)?;
    let SemanticStatementKindV1::Assign(assignment) = statement.kind() else { return Err(refuse()); };
    let SemanticRvalueKindV1::Load(expected) = assignment.value().kind() else { return Err(refuse()); };
    let place = load.source();
    let declaration = original.locals().get(place.local().index() as usize).ok_or_else(refuse)?;
    let ty = instances.owner().source_semantic().types().get(place.ty().index() as usize).ok_or_else(refuse)?;
    if !std::ptr::eq(load, expected)
        || assignment.value().result_type() != result || result != place.ty()
        || declaration.ty() != place.ty() || !place.projections().is_empty()
        || load.volatility() != SemanticVolatilityV1::Volatile || load.atomic().is_some()
        || !matches!(ty.shape(), SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_))
    { return Err(refuse()); }
    Ok(())
}

// The caller separately binds this original RvaluePlace occurrence to the
// actual cell-pointer use. This query derives only its expected effect flag.
fn source_reference_cell_read_volatility_v29(
    instances: &ExecutionInstancesV29<'_>,
    cell: SourceReferenceScalarCellV29,
    site: SourceReferenceSiteV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    budget.charge_work(10)?;
    if site.instance != cell.instance || cell.kind != SourceBackingKindV29::Scalar {
        return Err(execution_call_error_v29());
    }
    let function = instances.instance(site.instance)
        .ok_or_else(execution_call_error_v29)?.declaration();
    let block = function.blocks().get(site.block.index() as usize)
        .ok_or_else(execution_call_error_v29)?;
    let Some(statement) = site.statement else { return Ok(false); };
    let statement = block.statements().get(statement).ok_or_else(execution_call_error_v29)?;
    let SemanticStatementKindV1::Assign(assignment) = statement.kind() else { return Ok(false); };
    let SemanticRvalueKindV1::Load(load) = assignment.value().kind() else { return Ok(false); };
    if load.source().local() != cell.local || load.source().ty() != cell.ty
        || assignment.value().result_type() != cell.ty || !load.source().projections().is_empty()
        || load.atomic().is_some()
    {
        return Err(execution_call_error_v29());
    }
    if load.volatility() == SemanticVolatilityV1::NonVolatile { return Ok(false); }
    check_source_direct_volatile_load_v29(instances, site, load, cell.ty, budget)?;
    Ok(true)
}

fn check_scoped_read_volatility_v29(
    function: &SemanticFunctionDeclV1,
    read: ScopedMemoryReadV29,
    actual: bool,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(6)?;
    let place = scoped_object_original_place_v29(function, read.site, read.role)
        .ok_or_else(scoped_memory_error_v29)?;
    let mut expected = false;
    if read.role == ExecutionOperandV29::RvaluePlace
        && read.prefix as usize == place.projections().len()
        && let Some(SemanticStatementKindV1::Assign(assignment)) = scoped_source_statement_v29(function, read.site)
        && let SemanticRvalueKindV1::Load(load) = assignment.value().kind()
    {
        if !std::ptr::eq(load.source(), place) || load.atomic().is_some() {
            return Err(scoped_memory_error_v29());
        }
        expected = load.volatility() == SemanticVolatilityV1::Volatile;
    }
    if actual != expected { return Err(scoped_memory_error_v29()); }
    Ok(())
}
