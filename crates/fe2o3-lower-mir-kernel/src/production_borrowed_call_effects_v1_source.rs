fn checked_borrowed_slice_source_read_v1(
    query: &SliceQuery<'_, '_>,
    facts: &SliceFacts<'_>,
    budget: &mut SliceBudget<'_>,
) -> SliceResult<()> {
    use fe2o3_pliron::{
        ProductionSemanticSsaOccurrenceSiteV1 as Site,
        ProductionSemanticSsaOperandRoleV1 as Operand,
    };
    let subject = query.subject;
    let site = query.site;
    budget.charge_work(5)?;
    let mismatch = || ProductionSemanticKirErrorV1::CorrespondenceMismatch;
    let statement = site
        .statement
        .ok_or_else(|| site.unsupported("helper slice read requires an exact source statement"))?;
    if site.access != 0 {
        return Err(mismatch());
    }
    let source = subject
        .semantic_ssa
        .source_semantic()
        .functions()
        .get(site.function.index() as usize)
        .and_then(|function| function.blocks().get(site.block.index() as usize))
        .and_then(|block| block.statements().get(statement as usize))
        .ok_or_else(mismatch)?;
    let SemanticStatementKindV1::Assign(assignment) = source.kind() else {
        return Err(mismatch());
    };
    let (place, operand) = match assignment.value().kind() {
        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)) => {
            (place, Operand::RvalueOperand(0))
        }
        SemanticRvalueKindV1::Load(load) => {
            if load.volatility() != SemanticVolatilityV1::NonVolatile || load.atomic().is_some() {
                return Err(
                    site.unsupported("helper slice source load must be plain and nonvolatile")
                );
            }
            (load.source(), Operand::RvaluePlace)
        }
        _ => return Err(mismatch()),
    };
    if !assignment.destination().projections().is_empty()
        || place.projections().len() != 2
        || !matches!(
            place.projections()[0].kind(),
            SemanticProjectionKindV1::Dereference
        )
        || !matches!(
            place.projections()[1].kind(),
            SemanticProjectionKindV1::Index(_)
        )
    {
        return Err(
            site.unsupported("helper slice read is not a whole shared-slice element access")
        );
    }
    checked_borrowed_slice_source_input_v1(
        subject,
        site.root,
        site.function,
        facts.input,
        place.local(),
        Site::Statement {
            block: fe2o3_mir_model::SsaBlockIdV1::new(site.block.index()),
            statement,
        },
        operand,
        budget,
    )?;
    let SemanticProjectionKindV1::Index(index) = place.projections()[1].kind() else {
        return Err(mismatch());
    };
    let value = query.borrowed_index_use_v1(
        Site::Statement {
            block: fe2o3_mir_model::SsaBlockIdV1::new(site.block.index()),
            statement,
        },
        operand,
        fe2o3_pliron::ProductionSemanticSsaEventRoleV1::ProjectionIndexUse(1),
        index,
        budget,
    )?;
    if query.borrowed_index_definition_v1(value, budget)? != facts.index {
        return Err(mismatch());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn checked_borrowed_slice_source_input_v1(
    subject: CanonicalCallSubjectV1<'_>,
    root: SemanticFunctionIdV1,
    function: SemanticFunctionIdV1,
    input: SliceDefinition,
    local: SemanticLocalIdV1,
    site: fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1,
    operand: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
    budget: &mut SliceBudget<'_>,
) -> SliceResult<()> {
    use fe2o3_mir_model::{SsaEventV1, SsaResolvedEventV1};
    use fe2o3_pliron::ProductionSemanticSsaEventRoleV1 as Role;
    let mismatch = || ProductionSemanticKirErrorV1::CorrespondenceMismatch;
    budget.charge_work(4)?;
    let capture = subject
        .semantic_ssa
        .occurrence_storage()
        .ok_or_else(mismatch)?;
    if budget.storage() < capture.retained_storage() {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    let SliceDefinition::FunctionArgument { argument, .. } = input else {
        return Err(mismatch());
    };
    with_owner_arguments_v1(
        subject.semantic_ssa.source_semantic(),
        subject.executable.module(),
        subject.correspondence,
        (root, function),
        budget,
        |arguments| {
            let mut count = 0;
            arguments.visit_nodes(|node| {
                if matches!(node.coverage(), ProductionArgumentCoverageV1::Parameter(parameter)
                    if parameter.slot() == argument as usize)
                {
                    if node.local_binding() != Some((local, &[][..]))
                        || !node.source_path().is_empty()
                    {
                        return Err(mismatch());
                    }
                    count += 1;
                }
                Ok(())
            })?;
            if count != 1 {
                return Err(mismatch());
            }
            Ok(())
        },
    )?;
    let occurrences = subject.semantic_ssa.occurrences_v1().ok_or_else(mismatch)?;
    let occurrences = occurrences.function(function).ok_or_else(mismatch)?;
    budget.charge_work(argument_sum_v1(&[
        occurrences.entry_definitions().len(),
        occurrences.events().len(),
        occurrences.edge_definitions().len(),
    ])?)?;
    let mut entries = occurrences
        .entry_definitions()
        .iter()
        .filter(|entry| entry.variable().get() == local.index());
    if entries.next().and_then(|entry| entry.value()).is_none() || entries.next().is_some() {
        return Err(mismatch());
    }
    let mut selected = 0;
    // A direct shared descriptor is invariant only when the complete captured
    // source has no definition/kill (including edge results) of that entry local.
    for event in occurrences.events() {
        if event.event().variable().get() != local.index() {
            continue;
        }
        if !matches!(event.event(), SsaEventV1::Use(_)) || !event.is_promoted() {
            return Err(mismatch());
        }
        if event.site() == site && event.operand() == operand && event.role() == Role::BaseUse {
            if !event.is_reachable()
                || !matches!(event.resolved(), Some(SsaResolvedEventV1::Use { variable, .. }) if variable.get() == local.index())
            {
                return Err(mismatch());
            }
            selected += 1;
        }
    }
    if selected != 1
        || occurrences
            .edge_definitions()
            .iter()
            .any(|entry| entry.variable().get() == local.index())
    {
        return Err(mismatch());
    }
    Ok(())
}
