fn cr_private_source_profile_v1(
    source: &ProductionCanonicalRankedMetadataV1<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> CrPolicyResultV1<()> {
    cr_private_source_profile_with_assertions_v1(source, None, budget)
}

fn cr_private_source_profile_with_assertions_v1(
    source: &ProductionCanonicalRankedMetadataV1<'_>,
    assertions: Option<&canonical_assertion_v1::Coverage<'_>>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> CrPolicyResultV1<()> {
    use ProductionCanonicalRankedSourceRequirementV1 as Need;
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticRvalueKindV1 as Rvalue, SemanticStatementKindV1 as Statement,
        SemanticTerminatorKindV1 as Terminator, SemanticTypeShapeV1 as Shape,
    };
    source.guard.query(budget)?;
    if !matches!(
        source.owner.helper_source_policy_v1(),
        ProductionHelperSourcePolicyV1::RawEmpty | ProductionHelperSourcePolicyV1::UnitLocal
    ) {
        return Err(cr_policy_unsupported_v1(Need::LocalMemory, 0));
    }
    if !source.contracts.assertions.is_empty() {
        if let Some(coverage) = assertions {
            for assertion in &source.contracts.assertions {
                coverage.span(source, assertion.span, budget)?;
            }
        } else {
            budget.charge_work(1)?;
            return Err(cr_policy_unsupported_v1(Need::Assertion, 0));
        }
    }
    budget.charge_work(2)?;
    if !source.contracts.catalog.definitions().is_empty()
        || !source.contracts.catalog.bindings().is_empty()
    {
        return Err(cr_policy_unsupported_v1(Need::Catalog, 0));
    }
    for (ordinal, ty) in source
        .owner
        .semantic_ssa
        .source_semantic()
        .types()
        .iter()
        .enumerate()
    {
        budget.charge_work(1)?;
        match ty.shape() {
            Shape::Unit
            | Shape::Never
            | Shape::Scalar(_)
            | Shape::ValidityScalar(_)
            | Shape::Tuple(_)
            | Shape::Aggregate(_)
            | Shape::Array { .. } => {}
            _ => return Err(cr_policy_unsupported_v1(Need::Type, ordinal)),
        }
    }
    for (ordinal, argument) in source.arguments.rows.iter().enumerate() {
        budget.charge_work(1)?;
        if argument.ownership != SemanticSourceArgumentOwnershipV1::ByValue {
            return Err(cr_policy_unsupported_v1(Need::ArgumentOwnership, ordinal));
        }
    }
    for (ordinal, callable) in source
        .owner
        .semantic_ssa
        .source_semantic()
        .callables()
        .iter()
        .enumerate()
    {
        budget.charge_work(1)?;
        if !matches!(callable, SemanticCallableDeclV1::Defined { .. }) {
            return Err(cr_policy_unsupported_v1(Need::Callable, ordinal));
        }
    }
    for (ordinal, span) in source.source.spans.iter().enumerate() {
        budget.charge_work(1)?;
        match span.site {
            ProductionCanonicalRankedSourceSiteV1::Statement {
                source: statement, ..
            } => match statement.kind() {
                Statement::Assign(assignment) => match assignment.value().kind() {
                    Rvalue::Use(_)
                    | Rvalue::Unary { .. }
                    | Rvalue::Binary { .. }
                    | Rvalue::CheckedBinary(_)
                    | Rvalue::Cast { .. }
                    | Rvalue::Aggregate(_) => {}
                    Rvalue::Load(load) => {
                        budget.charge_work(2)?;
                        if load.volatility() != SemanticVolatilityV1::NonVolatile
                            || load.atomic().is_some()
                        {
                            return Err(cr_policy_unsupported_v1(Need::Rvalue, ordinal));
                        }
                        cr_private_source_occurrence_v1(source, span, ordinal, 0, budget)?;
                    }
                    _ => return Err(cr_policy_unsupported_v1(Need::Rvalue, ordinal)),
                },
                Statement::Nop | Statement::StorageLive(_) | Statement::StorageDead(_) => {}
                Statement::Store(store) => {
                    budget.charge_work(2)?;
                    if store.volatility() != SemanticVolatilityV1::NonVolatile
                        || store.atomic().is_some()
                    {
                        return Err(cr_policy_unsupported_v1(Need::Statement, ordinal));
                    }
                    cr_private_source_occurrence_v1(source, span, ordinal, 1, budget)?;
                }
                _ => return Err(cr_policy_unsupported_v1(Need::Statement, ordinal)),
            },
            ProductionCanonicalRankedSourceSiteV1::Terminator {
                source: terminator, ..
            } => match terminator.kind() {
                Terminator::Goto(_) | Terminator::SwitchInt { .. } | Terminator::Return => {}
                Terminator::Call(_) => {
                    cr_private_source_occurrence_v1(source, span, ordinal, 2, budget)?;
                }
                Terminator::Assert { .. } if assertions.is_some() => {
                    assertions
                        .expect("matched sealed assertion coverage")
                        .span(source, ordinal, budget)?;
                }
                _ => return Err(cr_policy_unsupported_v1(Need::Terminator, ordinal)),
            },
            ProductionCanonicalRankedSourceSiteV1::Synthetic(synthetic) => {
                if synthetic.rule() == SemanticKirSyntheticOperationRuleV1::RuntimeAssertFailureTrap
                    && let Some(coverage) = assertions
                {
                    coverage.span(source, ordinal, budget)?;
                    continue;
                }
                if synthetic.rule() != SemanticKirSyntheticOperationRuleV1::RetainedLocalStorage {
                    return Err(cr_policy_unsupported_v1(Need::Synthetic, ordinal));
                }
                for row in &source.inventory.operations()[span.operations.clone()] {
                    budget.charge_work(1)?;
                    if !matches!(
                        row.operation.kind,
                        OperationKind::Alloca { .. } | OperationKind::Constant(_)
                    ) {
                        return Err(cr_policy_unsupported_v1(Need::Synthetic, ordinal));
                    }
                }
            }
        }
    }
    budget.charge_work(source.contracts.launches.len())?;
    Ok(())
}

fn cr_private_source_occurrence_v1(
    source: &ProductionCanonicalRankedMetadataV1<'_>,
    span: &ProductionCanonicalRankedSpanV1<'_>,
    ordinal: usize,
    kind: u8,
    budget: &mut ArgumentBudgetV1<'_>,
) -> CrPolicyResultV1<()> {
    budget.charge_work(span.operations.len())?;
    let count = source.inventory.operations()[span.operations.clone()]
        .iter()
        .filter(|row| {
            matches!(
                (kind, &row.operation.kind),
                (0, OperationKind::Load { .. })
                    | (1, OperationKind::Store { .. })
                    | (2, OperationKind::Call { .. })
            )
        })
        .count();
    if count != 1 {
        return Err(cr_policy_unsupported_v1(
            ProductionCanonicalRankedSourceRequirementV1::GraphIdentity,
            ordinal,
        ));
    }
    Ok(())
}
