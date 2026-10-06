fn with_production_optimized_consumer_v18(
    prepared: ProductionPreparedSourceV18,
    budget: &mut ArgumentBudgetV1<'_>,
    consume: impl for<'scope, 'work> FnOnce(
        &ProductionSourceCorrespondenceV18<'scope>,
        &ProductionOptimizedSourceCorrespondenceV18<'scope>,
        &mut ArgumentBudgetV1<'work>,
    ) -> SourceOwnedResultV18<()>,
) -> SourceOwnedResultV18<()> {
    with_production_optimizer_result_v18(prepared, budget, consume).map_err(|error| match error {
        ProductionSourceOptimizationErrorV18::Source(error) => error,
        other => panic!("actual source-owned optimizer and consumer: {other:?}"),
    })
}

type ProductionOptimizerTestResultV18 =
    Result<(), ProductionSourceOptimizationErrorV18<ProductionSourceOwnedViewErrorV18>>;

// Negative controls must observe the real typed refusal, not a panic from the
// positive-only helper. Successful runs keep the same owner/floor assertions.
fn with_production_optimizer_result_v18(
    prepared: ProductionPreparedSourceV18,
    budget: &mut ArgumentBudgetV1<'_>,
    consume: impl for<'scope, 'work> FnOnce(
        &ProductionSourceCorrespondenceV18<'scope>,
        &ProductionOptimizedSourceCorrespondenceV18<'scope>,
        &mut ArgumentBudgetV1<'work>,
    ) -> SourceOwnedResultV18<()>,
) -> ProductionOptimizerTestResultV18 {
    prepared.with_source_consumer_v18(budget, |source, budget| {
        let _ = optimized_source_fixture_precharge_v18(&source.owner.inner.pending.graph);
        let floor = budget.storage();
        let (output, (), receipt) =
            source.with_checked_optimization_v18(budget, |original, optimized, budget| {
                consume(original, optimized, budget)?;
                Ok::<_, ProductionSourceOwnedViewErrorV18>(((), 0))
            })?;
        assert_eq!(
            receipt.retained_storage(),
            std::mem::size_of::<fe2o3_pliron::KirNeutralOwnedOriginStorageV1>()
        );
        assert!(!output.grants_authority());
        assert!(output.storage().retained_storage() > 0);
        drop(output);
        assert_eq!(budget.storage(), floor);
        Ok(())
    })
}

fn run_production_optimized_consumer_v18(
    factory: fn() -> ProductionSemanticSsaOwnerV1,
    consume: impl for<'scope, 'work> FnOnce(
        &ProductionSourceCorrespondenceV18<'scope>,
        &ProductionOptimizedSourceCorrespondenceV18<'scope>,
        &mut ArgumentBudgetV1<'work>,
    ) -> SourceOwnedResultV18<()>,
) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_from_v18(factory, &mut budget);
    with_production_optimized_consumer_v18(prepared, &mut budget, consume).unwrap();
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn production_optimizer_entrance_lends_original_and_actual_output_together() {
    run_production_optimized_consumer_v18(
        folding_source_owner_v18,
        |original, optimized, budget| {
            let input = optimized.input_inventory(budget)?;
            let output = optimized.output_inventory(budget)?;
            assert!(std::ptr::eq(input, original.inventory));
            assert!(std::ptr::eq(
                optimized.original_source(budget)?,
                original.source
            ));
            assert!(!std::ptr::eq(input, output));
            assert_ne!(
                input.owner().canonical_bytes(),
                output.owner().canonical_bytes()
            );
            assert!(input.belongs_to(&original.source.owner.inner.pending.graph));
            assert!(!output.belongs_to(&original.source.owner.inner.pending.graph));
            Ok(())
        },
    );
}

#[test]
fn actual_optimized_qualified_ranked_reader_keeps_original_ids_and_paid_decoded_rows() {
    run_production_optimized_consumer_v18(
        scalar_payload_owner_v18,
        |original, optimized, budget| {
            let function =
                optimized_source_root_function_v18(original, optimized, 0, budget)?.function;
            // An inert decoder fixture, not a claim that these accesses describe
            // the source's allocations, scalar values, or complete effect census.
            let recipe = effect_order_recipe_v18(function, 2, false);
            let source_instance = 1;
            let source_function = original.source.instance(0, source_instance, budget)?.0;
            assert_eq!(
                source_function.index(),
                2,
                "select the fixture's actual helper statement"
            );
            let mut rows = [
                ProductionSourceRankedAccessV18::new(
                    source_instance,
                    source_function,
                    ProductionRankedAccessSourceV1 {
                        semantic_block: 0,
                        semantic_statement: Some(0),
                        semantic_access_ordinal: 9,
                        ranked_block: 0,
                        ranked_operation: 3,
                        output_extent: None,
                    },
                ),
                ProductionSourceRankedAccessV18::new(
                    source_instance,
                    source_function,
                    ProductionRankedAccessSourceV1 {
                        semantic_block: 0,
                        semantic_statement: Some(0),
                        semantic_access_ordinal: 5,
                        ranked_block: 0,
                        ranked_operation: 2,
                        output_extent: None,
                    },
                ),
            ];
            let floor = budget.storage();
            prepare_source_ranked_accesses_v18(original, optimized, 0, &mut rows, budget)?;
            assert_eq!(
                budget.storage(),
                floor,
                "key assignment owns no retained collection"
            );
            let data = QualifiedSourceRankedDataV18::build(
                original,
                optimized,
                0,
                &recipe,
                &rows,
                MODULE_LIMIT,
                budget,
            )?;
            let shared = &data.shared;
            let retained = size_of::<QualifiedSourceRankedDataV18<'_, '_, '_, '_>>()
                + size_of::<SourceOwnedResultV18<QualifiedSourceRankedDataV18<'_, '_, '_, '_>>>()
                + size_of::<Option<(SourceEffectSiteV18, SemanticAccessSiteV1)>>()
                + 2 * size_of::<RankedIndexSourcesV18<'_>>()
                + size_of::<SourceOwnedResultV18<SourceRankedIndexDataV18<'_>>>()
                + size_of::<SourceRankedIndexDataV18<'_>>()
                + size_of::<CorrelationLedgerV18<'_, '_, '_>>()
                + size_of::<SourceCorrelationChargeV18<'_, '_, '_, '_>>()
                + shared.sources.capacity()
                    * size_of::<(SemanticAccessSiteV1, IndexedRankedAccessSourceV1)>()
                + shared.locations.capacity() * size_of::<usize>()
                + shared.conservative.capacity() * size_of::<(u32, Option<u32>, usize)>()
                + shared.views.capacity()
                    * size_of::<(ProductionRankedValueIdV1, RankedViewDefinitionV1)>()
                + shared.expressions.capacity()
                    * size_of::<(
                        ProductionRankedValueIdV1,
                        (
                            &ProductionSemanticExpressionV2,
                            ProductionNumericalContractV2,
                        ),
                    )>();
            assert_eq!(budget.storage() - floor, retained);
            let ledger = CorrelationLedgerV18::new(budget, original.source.cleanup);
            let index = data.query_index(&recipe, &ledger)?;
            for (ordinal, row) in rows.iter().enumerate() {
                let key = index
                    .original_site(row.original())
                    .expect("exact original qualified key");
                assert_eq!(key.ordinal, ordinal as u32);
                assert_eq!(row.projected.semantic_access_ordinal, [5, 9][ordinal]);
                let actual = index.source(key).unwrap();
                assert_eq!(
                    (actual.ranked_block, actual.ranked_operation),
                    (row.projected.ranked_block, row.projected.ranked_operation)
                );
                assert_eq!(
                    index.site((actual.ranked_block, actual.ranked_operation)),
                    Some(key)
                );
            }
            assert_eq!(index.sources().count(), 2);
            assert_eq!(ledger.failure.get(), None);
            assert!(!ledger.inconsistent_inventory.get());
            let query_headers = qualified_source_ranked_query_headers_v18()?;
            assert_eq!(
                ledger.with_budget(|budget| Ok(budget.storage()))?,
                floor + retained + query_headers
            );
            drop(index);
            drop(ledger);
            drop(data);
            budget.release_storage(retained + query_headers)?;
            assert_eq!(budget.storage(), floor);
            Ok(())
        },
    );
}

#[test]
fn actual_retained_optimizer_output_stays_paid_across_original_source_scope_exit() {
    struct Payload(std::sync::Arc<std::sync::atomic::AtomicUsize>);
    impl Drop for Payload {
        fn drop(&mut self) {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }
    let dropped = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_from_v18(folding_source_owner_v18, &mut budget);
    let output = prepared
        .with_source_consumer_v18(&mut budget, |source, budget| {
            let _ = optimized_source_fixture_precharge_v18(&source.owner.inner.pending.graph);
            let floor = budget.storage();
            let payload = Payload(dropped.clone());
            let result = source
                .with_retained_checked_optimization_v18(budget, |original, optimized, budget| {
                    assert!(std::ptr::eq(
                        original.inventory,
                        optimized.input_inventory(budget)?
                    ));
                    assert!(
                        optimized
                            .output_inventory(budget)?
                            .owner()
                            .module()
                            .kernels
                            .len()
                            > 0
                    );
                    Ok::<_, ProductionSourceOwnedViewErrorV18>((payload, size_of::<Payload>()))
                })
                .expect("actual retained source optimizer");
            assert_eq!(
                budget.storage(),
                floor + result.0.storage().retained_storage() + result.2.retained_storage()
            );
            assert_eq!(dropped.load(std::sync::atomic::Ordering::SeqCst), 0);
            Ok::<_, ProductionSourceOwnedViewErrorV18>(result)
        })
        .unwrap();
    let retained = output.0.storage().retained_storage() + output.2.retained_storage();
    assert_eq!(budget.storage(), MODULE_FLOOR + retained);
    assert!(output.0.owner().module().kernels.len() > 0);
    assert!(!output.0.grants_authority() && !output.2.grants_authority());
    assert_eq!(dropped.load(std::sync::atomic::Ordering::SeqCst), 0);
    drop(output);
    assert_eq!(dropped.load(std::sync::atomic::Ordering::SeqCst), 1);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn actual_output_analysis_is_lazy_and_reuses_exact_inventory_reports() {
    run_production_optimized_consumer_v18(
        folding_source_owner_v18,
        |original, optimized, budget| {
            let floor = budget.storage();
            original.with_optimized_analysis_v18(optimized, budget, |analyses, budget| {
                assert!(analyses.sparse.is_none());
                assert!(analyses.input_memory.is_none());
                assert!(analyses.output_memory.is_none());
                let empty = budget.storage();
                let mut pointers = None;
                let input_inventory = analyses.input;
                let output_inventory = analyses.output;
                analyses.with_projection_facts(budget, |sparse, input, output, _| {
                    assert!(sparse.belongs_to(output_inventory));
                    assert!(input.belongs_to(input_inventory));
                    assert!(output.belongs_to(output_inventory));
                    pointers = Some((
                        std::ptr::from_ref(sparse) as usize,
                        std::ptr::from_ref(input) as usize,
                        std::ptr::from_ref(output) as usize,
                    ));
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                })?;
                let populated = budget.storage();
                assert!(populated > empty);
                analyses.with_projection_facts(budget, |sparse, input, output, _| {
                    assert_eq!(
                        pointers,
                        Some((
                            std::ptr::from_ref(sparse) as usize,
                            std::ptr::from_ref(input) as usize,
                            std::ptr::from_ref(output) as usize
                        ))
                    );
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                })?;
                assert_eq!(budget.storage(), populated);
                analyses.with_memory_versions(budget, |input, output, _| {
                    assert!(input.belongs_to(input_inventory));
                    assert!(output.belongs_to(output_inventory));
                    assert!(!input.belongs_to(output_inventory));
                    assert!(!output.belongs_to(input_inventory));
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                })?;
                assert!(budget.storage() > populated);
                Ok::<_, ProductionSourceOwnedViewErrorV18>(())
            })?;
            assert_eq!(budget.storage(), floor);
            Ok(())
        },
    );
}
#[test]
fn indexed_optimized_span_entries_match_the_complete_existing_visitor() {
    for factory in [
        scalar_payload_owner_v18 as fn() -> _,
        active_gap_owner_v18 as fn() -> _,
    ] {
        run_production_optimized_consumer_v18(factory, |original, optimized, budget| {
            let source = original.source(budget)?;
            let mut visited = 0usize;
            let mut gaps = 0usize;
            for root in 0..source.root_count(budget)? {
                let root_row = source.root_row(root)?;
                for row in &root_row.coordinates.spans.rows {
                    let (block, statement) = match row.source {
                        InstanceSpanSourceV1::Statement(site) => {
                            (site.semantic_block, Some(site.statement_ordinal))
                        }
                        InstanceSpanSourceV1::Terminator(site) => (site.semantic_block, None),
                        _ => continue,
                    };
                    let instance = row.instance.index();
                    let count = optimized
                        .source_span_entry_count(root, instance, block, statement, budget)?;
                    let mut ordinal = 0usize;
                    optimized.visit_source_operations(
                        root,
                        instance,
                        block,
                        statement,
                        budget,
                        |entry, budget| {
                            assert_eq!(
                                entry,
                                optimized.source_span_entry(
                                    root, instance, block, statement, ordinal, budget
                                )?
                            );
                            gaps += usize::from(matches!(
                                entry,
                                ProductionOptimizedSourceSpanV18::Gap(_)
                            ));
                            ordinal += 1;
                            Ok(())
                        },
                    )?;
                    assert_eq!(
                        ordinal, count,
                        "the count covers entries, not only physical operations"
                    );
                    visited += count;
                }
            }
            assert!(visited > 0);
            // Both fixtures have ordinary source spans even where no physical
            // operation is needed. They remain explicit indexed entries.
            assert!(gaps > 0);
            Ok(())
        });
    }
}
