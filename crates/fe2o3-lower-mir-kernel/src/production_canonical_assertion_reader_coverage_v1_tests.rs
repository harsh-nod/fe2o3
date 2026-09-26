// Test-only inert reader inputs. No helper constructs an owner, C report or Fact.
fn assertion_reader_baseline_v1(
    source: &ProductionCanonicalRankedMetadataV1<'_>,
    policies: &CheckedCanonicalTrapPoliciesV1<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> R<()> {
    scoped(budget, |budget| {
        let mut coverage = derive(source, policies, budget)?;
        assert_eq!(source.contracts.assertions.len(), 2);
        let first = &source.contracts.assertions[0];
        let second = &source.contracts.assertions[1];
        assert_ne!(
            source.source.spans[first.span].association,
            source.source.spans[second.span].association,
        );
        assert_eq!(first.binding, second.binding);
        assert!(coverage.rows[first.span].is_some());
        assert!(coverage.rows[second.span].is_some());
        assert_eq!(policies.pair_count(budget)?, 1);
        assert_eq!(policies.pair(0, budget)?.incoming_edges().len(), 1);
        join_synthetic(source, policies, &mut coverage, budget)
    })
}

fn assertion_reader_duplicate_alias_v1(
    contracts: &mut CrContractsV1<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> R<()> {
    budget.reserve_storage(std::mem::size_of::<Vec<ProductionCanonicalRankedAssertionV1>>())?;
    let mut duplicate = rows(
        contracts
            .assertions
            .len()
            .checked_add(1)
            .ok_or(ArgumentResourceV1::Arithmetic)?,
        budget,
    )?;
    for row in &contracts.assertions {
        budget.charge_work(1)?;
        duplicate.push(ProductionCanonicalRankedAssertionV1 {
            span: row.span,
            binding: row.binding,
        });
    }
    budget.charge_work(1)?;
    let first = &contracts.assertions[0];
    duplicate.push(ProductionCanonicalRankedAssertionV1 {
        span: first.span,
        binding: first.binding,
    });
    // Retired capacity remains paid until the containing test scope ends.
    contracts.assertions = duplicate;
    Ok(())
}

pub(super) fn read_test_complete_alias_roster_v1(
    source: &ProductionCanonicalRankedMetadataV1<'_>,
    policies: &CheckedCanonicalTrapPoliciesV1<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> R<()> {
    assertion_reader_baseline_v1(source, policies, budget)?;
    for duplicate in [false, true] {
        let floor = budget.storage();
        scoped(budget, |budget| {
            budget.reserve_storage(argument_sum_v1(&[
                std::mem::size_of::<CrContractsV1<'_>>(),
                std::mem::size_of::<ProductionCanonicalRankedMetadataV1<'_>>(),
            ])?)?;
            let mut contracts =
                cr_build_contracts_v1(source.owner, source.inventory, source.source, budget)?;
            {
                let baseline = ProductionCanonicalRankedMetadataV1 {
                    owner: source.owner,
                    inventory: source.inventory,
                    calls: source.calls,
                    source: source.source,
                    arguments: source.arguments,
                    contracts: &contracts,
                    guard: source.guard,
                };
                complete_source_roster(&baseline, budget)?;
            }
            if duplicate {
                assertion_reader_duplicate_alias_v1(&mut contracts, budget)?;
            } else {
                budget.charge_work(contracts.assertions.len())?;
                contracts.assertions.remove(0);
            }
            let hostile = ProductionCanonicalRankedMetadataV1 {
                owner: source.owner,
                inventory: source.inventory,
                calls: source.calls,
                source: source.source,
                arguments: source.arguments,
                contracts: &contracts,
                guard: source.guard,
            };
            let error = match derive(&hostile, policies, budget) {
                Ok(_) => panic!("unauthenticated alias roster unexpectedly accepted"),
                Err(error) => error,
            };
            assert!(matches!(
                error,
                Failure::Binding {
                    span: None,
                    detail: "missing or duplicate source assertion alias",
                }
            ));
            Ok(())
        })?;
        assert_eq!(budget.storage(), floor);
    }
    Ok(())
}

pub(super) fn read_test_proved_sink_aliases_v1(
    source: &ProductionCanonicalRankedMetadataV1<'_>,
    policies: &CheckedCanonicalTrapPoliciesV1<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> R<()> {
    assertion_reader_baseline_v1(source, policies, budget)?;
    for duplicate in [false, true] {
        let floor = budget.storage();
        scoped(budget, |budget| {
            let mut coverage = derive(source, policies, budget)?;
            let error = if duplicate {
                budget.reserve_storage(argument_sum_v1(&[
                    std::mem::size_of::<CrContractsV1<'_>>(),
                    std::mem::size_of::<ProductionCanonicalRankedMetadataV1<'_>>(),
                ])?)?;
                let mut contracts =
                    cr_build_contracts_v1(source.owner, source.inventory, source.source, budget)?;
                assertion_reader_duplicate_alias_v1(&mut contracts, budget)?;
                let hostile = ProductionCanonicalRankedMetadataV1 {
                    owner: source.owner,
                    inventory: source.inventory,
                    calls: source.calls,
                    source: source.source,
                    arguments: source.arguments,
                    contracts: &contracts,
                    guard: source.guard,
                };
                // Only this private aggregate reader sees the duplicate. Full
                // derive rejects it earlier in complete_source_roster.
                join_synthetic(&hostile, policies, &mut coverage, budget).unwrap_err()
            } else {
                let span = source.contracts.assertions[0].span;
                let _removed_proof = coverage.rows[span].take().expect("genuine proved alias");
                join_synthetic(source, policies, &mut coverage, budget).unwrap_err()
            };
            assert!(matches!(
                error,
                Failure::Binding {
                    span: Some(_),
                    detail: "shared trap sink alias coverage",
                }
            ));
            Ok(())
        })?;
        assert_eq!(budget.storage(), floor);
    }
    Ok(())
}

pub(super) fn read_test_synthetic_aliases_v1(
    source: &ProductionCanonicalRankedMetadataV1<'_>,
    policies: &CheckedCanonicalTrapPoliciesV1<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> R<()> {
    assertion_reader_baseline_v1(source, policies, budget)?;
    for extra in [false, true] {
        let floor = budget.storage();
        scoped(budget, |budget| {
            budget.reserve_storage(argument_sum_v1(&[
                std::mem::size_of::<CrSourceRowsV1<'_>>(),
                std::mem::size_of::<ProductionCanonicalRankedMetadataV1<'_>>(),
            ])?)?;
            let mut source_rows =
                cr_build_source_rows_v1(source.owner, source.inventory, source.calls, budget)?;
            cr_check_source_rows_v1(
                source.owner,
                source.inventory,
                source.calls,
                &source_rows,
                budget,
            )?;
            let last = source_rows
                .spans
                .last()
                .expect("actual synthetic counterpart");
            assert!(
                matches!(last.site, ProductionCanonicalRankedSourceSiteV1::Synthetic(row)
                if row.rule() == SemanticKirSyntheticOperationRuleV1::RuntimeAssertFailureTrap)
            );
            assert_eq!(last.block, policies.pair(0, budget)?.terminal_block());
            assert!(
                source
                    .contracts
                    .assertions
                    .iter()
                    .all(|row| row.span < source_rows.spans.len() - 1)
            );
            if extra {
                budget.reserve_storage(std::mem::size_of::<
                    Vec<ProductionCanonicalRankedSpanV1<'_>>,
                >())?;
                let mut spans = rows(
                    source_rows
                        .spans
                        .len()
                        .checked_add(1)
                        .ok_or(ArgumentResourceV1::Arithmetic)?,
                    budget,
                )?;
                for row in source_rows.spans.iter().chain(std::iter::once(last)) {
                    budget.charge_work(1)?;
                    spans.push(ProductionCanonicalRankedSpanV1 {
                        association: row.association,
                        block: row.block,
                        operations: row.operations.clone(),
                        site: row.site,
                    });
                }
                source_rows.spans = spans;
            } else {
                budget.charge_work(1)?;
                source_rows
                    .spans
                    .pop()
                    .expect("selected final synthetic span");
            }
            // The copied inverse origin tables are intentionally NOT re-sealed.
            // This is hostile reader input, never a replacement public owner.
            let hostile = ProductionCanonicalRankedMetadataV1 {
                owner: source.owner,
                inventory: source.inventory,
                calls: source.calls,
                source: &source_rows,
                arguments: source.arguments,
                contracts: source.contracts,
                guard: source.guard,
            };
            let error = match derive(&hostile, policies, budget) {
                Ok(_) => panic!("unauthenticated synthetic roster unexpectedly accepted"),
                Err(error) => error,
            };
            assert!(matches!(
                error,
                Failure::Binding {
                    span: None,
                    detail: "graph trap is missing its source alias",
                }
            ));
            Ok(())
        })?;
        assert_eq!(budget.storage(), floor);
    }
    Ok(())
}
