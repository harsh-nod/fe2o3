/// One source alias whose actual assertion and original graph attachment agree.
/// The proof kind is diagnostic; the scoped enclosing view retains custody.
pub struct ProductionCanonicalAssertionV1 {
    span: usize,
    binding: SemanticKirAssertConditionBindingV1,
    proof: fe2o3_mir_model::SemanticAssertionProofKindV1,
}
impl ProductionCanonicalAssertionV1 {
    /// Original source-span ordinal within the retained metadata owner.
    pub const fn span(&self) -> usize {
        self.span
    }
    /// Exact original condition and success/failure edge attachment.
    pub const fn binding(&self) -> SemanticKirAssertConditionBindingV1 {
        self.binding
    }
    /// Diagnostic proof classification, not a detachable proof capability.
    pub const fn proof_kind(&self) -> fe2o3_mir_model::SemanticAssertionProofKindV1 {
        self.proof
    }
}

// Only derive() constructs this internal capability, after full bidirectional coverage.
pub(super) struct Coverage<'a> {
    inventory: &'a CanonicalKirInventoryV1<'a>,
    rows: Vec<Option<ProductionCanonicalAssertionV1>>,
    synthetic: Vec<bool>,
}
impl Coverage<'_> {
    pub(super) fn span(
        &self,
        source: &ProductionCanonicalRankedMetadataV1<'_>,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CrPolicyResultV1<()> {
        budget.charge_work(2)?;
        let present = self.rows.get(ordinal).is_some_and(Option::is_some)
            || self.synthetic.get(ordinal) == Some(&true);
        if !std::ptr::eq(self.inventory, source.inventory) || !present {
            return Err(cr_policy_unsupported_v1(
                ProductionCanonicalRankedSourceRequirementV1::Assertion,
                ordinal,
            ));
        }
        Ok(())
    }
}

fn complete_source_roster(
    source: &ProductionCanonicalRankedMetadataV1<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> R<()> {
    let semantic = source.owner.semantic_ssa.source_semantic();
    let origins = source.owner.assert_origins();
    if !std::ptr::eq(origins.executable(), source.inventory.owner()) {
        return Err(binding(None, "assertion origin owner"));
    }
    let mut count = 0usize;
    for group in &source.calls.groups {
        budget.charge_work(1)?;
        let association = group.function.source();
        let function = &semantic.functions()[association.semantic_function().index() as usize];
        for (block, body) in function.blocks().iter().enumerate() {
            budget.charge_work(1)?;
            if !matches!(
                body.terminator().kind(),
                SemanticTerminatorKindV1::Assert { .. }
            ) {
                continue;
            }
            let block = SemanticBlockIdV1::from_index(
                u32::try_from(block).map_err(|_| ArgumentResourceV1::Arithmetic)?,
            );
            // Unsupported unmaterialized source assertions are explicit refusals,
            // not an absent attachment silently interpreted as a proved predicate.
            if !origins
                .is_materialized_block(
                    association.correspondence_owner(),
                    association.semantic_function(),
                    block,
                    budget,
                )
                .map_err(query_origin)?
            {
                return Err(binding(
                    None,
                    "source assertion is outside the original materialization roster",
                ));
            }
            let mut matches = 0usize;
            for row in &source.contracts.assertions {
                budget.charge_work(1)?;
                if let ProductionCanonicalRankedSourceSiteV1::Terminator {
                    span,
                    source: actual,
                } = source.source.spans[row.span].site
                {
                    if span.correspondence_owner == association.correspondence_owner()
                        && span.semantic_function == association.semantic_function()
                        && span.semantic_block == block
                        && std::ptr::eq(actual, body.terminator())
                    {
                        matches += 1;
                    }
                }
            }
            if matches != 1 {
                return Err(binding(None, "missing or duplicate source assertion alias"));
            }
            count = count.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
        }
    }
    budget.charge_work(2)?;
    if count != source.contracts.assertions.len() || count != origins.source_site_count() {
        return Err(binding(None, "complete assertion alias roster"));
    }
    Ok(())
}

fn derive<'a, 's, 'g: 's>(
    source: &ProductionCanonicalRankedMetadataV1<'a>,
    policies: &impl AssertionTrapReaderV1<'s, 'g>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> R<Coverage<'a>> {
    derive_with_limits(
        source,
        policies,
        SemanticAssertionLimitsV1::new(usize::MAX, usize::MAX),
        budget,
    )
}
fn derive_with_limits<'a, 's, 'g: 's>(
    source: &ProductionCanonicalRankedMetadataV1<'a>,
    policies: &impl AssertionTrapReaderV1<'s, 'g>,
    limits: SemanticAssertionLimitsV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> R<Coverage<'a>> {
    source.guard.query(budget)?;
    if !std::ptr::eq(policies.owner(budget)?, source.inventory.owner()) {
        return Err(binding(
            None,
            "trap policies belong to another original graph",
        ));
    }
    complete_source_roster(source, budget)?;
    budget.reserve_storage(std::mem::size_of::<Coverage<'_>>())?;
    let mut coverage = Coverage {
        inventory: source.inventory,
        rows: rows(source.source.spans.len(), budget)?,
        synthetic: rows(source.source.spans.len(), budget)?,
    };
    budget.charge_work(argument_product_v1(source.source.spans.len(), 2)?)?;
    coverage
        .rows
        .resize_with(source.source.spans.len(), || None);
    coverage.synthetic.resize(source.source.spans.len(), false);
    scoped(budget, |budget| {
        let pairs = policies.pair_count(budget)?;
        let mut incoming_count = 0usize;
        for ordinal in 0..pairs {
            let range = policies.pair(ordinal, budget)?.incoming_edges();
            if range.start != incoming_count || range.is_empty() {
                return Err(binding(None, "trap incoming roster"));
            }
            incoming_count = range.end;
        }
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of::<AssertGraphIndexV1<'_>>(),
            std::mem::size_of::<Vec<usize>>(),
        ])?)?;
        let graph = AssertGraphIndexV1::build(source.inventory.owner().module(), true, budget)
            .map_err(query_origin)?;
        let (sparse, receipt) = CanonicalKirSparseV1::derive(
            source.inventory,
            fe2o3_kernel_analysis::CanonicalKirSparseLimitsV1::default(),
            budget,
        )
        .map_err(Failure::Sparse)?;
        budget.reserve_storage(receipt.retained_storage())?;
        let mut incoming = rows::<usize>(incoming_count, budget)?;
        budget.charge_work(incoming_count)?;
        incoming.resize(incoming_count, 0);
        let semantic = source.owner.semantic_ssa.source_semantic();
        // One live analysis per source function, with every root alias queried.
        for (function, _) in semantic.functions().iter().enumerate() {
            budget.charge_work(argument_sum_v1(&[source.contracts.assertions.len(), 1])?)?;
            let needed = source.contracts.assertions.iter().any(|row| {
                matches!(source.source.spans[row.span].site,
                    ProductionCanonicalRankedSourceSiteV1::Terminator { span, .. }
                        if span.semantic_function.index() as usize == function)
            });
            if !needed {
                continue;
            }
            crate::with_production_semantic_assertion_query_v1(
                semantic,
                SemanticFunctionIdV1::from_index(
                    u32::try_from(function).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                ),
                limits,
                budget,
                |query, budget| {
                    Ok((|| {
                        for row in &source.contracts.assertions {
                            budget.charge_work(1)?;
                            let ProductionCanonicalRankedSourceSiteV1::Terminator {
                                span,
                                source: term,
                            } = source.source.spans[row.span].site
                            else {
                                return Err(binding(Some(row.span), "assertion source site"));
                            };
                            if span.semantic_function.index() as usize != function {
                                continue;
                            }
                            let outcome = query
                                .assertion(span.semantic_block, budget)
                                .map_err(Failure::SourceQuery)?;
                            let fact = match outcome {
                                SemanticAssertionOutcomeV1::Proved(fact) => fact,
                                SemanticAssertionOutcomeV1::Refuted(_) => {
                                    return Err(Failure::Refuted { span: row.span });
                                }
                                SemanticAssertionOutcomeV1::NotProved(reason) => {
                                    return Err(Failure::NotProved {
                                        span: row.span,
                                        reason,
                                    });
                                }
                                SemanticAssertionOutcomeV1::NotAnAssertion => {
                                    return Err(binding(
                                        Some(row.span),
                                        "source proof has no assertion",
                                    ));
                                }
                            };
                            let SemanticTerminatorKindV1::Assert { condition, .. } = term.kind()
                            else {
                                return Err(binding(Some(row.span), "source proof assertion kind"));
                            };
                            budget.charge_work(7)?;
                            if !std::ptr::eq(fact.types(), semantic.types())
                                || !std::ptr::eq(fact.function(), &semantic.functions()[function])
                                || fact.block() != span.semantic_block
                                || !std::ptr::eq(fact.assertion(), term.kind())
                                || !std::ptr::eq(fact.condition(), condition)
                                || fact.expected() != row.binding.expected()
                                || fact.success_edge().target() != row.binding.semantic_success()
                            {
                                return Err(binding(
                                    Some(row.span),
                                    "borrowed source proof subject",
                                ));
                            }
                            join(
                                source,
                                row,
                                &graph,
                                &sparse,
                                policies,
                                &mut incoming,
                                budget,
                            )?;
                            if coverage.rows[row.span].is_some() {
                                return Err(binding(Some(row.span), "duplicate proved alias"));
                            }
                            coverage.rows[row.span] = Some(ProductionCanonicalAssertionV1 {
                                span: row.span,
                                binding: row.binding,
                                proof: fact.proof_kind(),
                            });
                        }
                        Ok(())
                    })())
                },
            )
            .map_err(Failure::SourceQuery)??;
        }
        budget.charge_work(incoming.len())?;
        if incoming.contains(&0) {
            return Err(binding(None, "unbound graph trap incoming edge"));
        }
        join_synthetic(source, policies, &mut coverage, budget)?;
        Ok(())
    })?;
    Ok(coverage)
}
