#[test]
fn original_scalar_symbol_refusal_survives_option_erasure_retry_and_outer_success() {
    for fault in 0..3 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_v18(&mut budget);
        let completed = std::cell::Cell::new(false);
        let detail = match fault {
            0 => None,
            1 => Some("source expression private read name is absent"),
            2 => Some("source expression entered a ranked symbol namespace"),
            _ => unreachable!(),
        };
        let result = prepared.with_source_consumer_v18(&mut budget,
            |source, budget| -> SourceOwnedResultV18<()> {
            source.with_analysis_v18(budget, |scope| scope.with_inventory_v1(|inventory, budget| {
                source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                    let physical = source.root(0, budget)?.1;
                    let recipe = scalar_leaf_collision_recipe_v18(inventory.functions()[physical].function);
                    let attempted = relation.with_scalar_leaves_v18(0, &recipe, budget,
                        |view, budget| -> SourceOwnedResultV18<()> {
                        let leaves = view.leaves;
                        assert!(leaves.rows.len() >= 4, "the real repeated helper has emitted scalar reads");
                        let leaf = leaves.rows[0];
                        assert!(leaves.rows.iter().all(|row| row.symbol != 0),
                            "the recipe reserves zero without making it an original read");
                        let header = size_of::<CorrelationLedgerV18<'_, '_, '_>>()
                            + size_of::<SourceExpressionLeavesV18<'_, '_, '_, '_, '_, '_>>()
                            + size_of::<SourceCorrelationChargeV18<'_, '_, '_, '_>>();
                        let floor = budget.storage();
                        budget.reserve_storage(header)?;
                        let ledger = CorrelationLedgerV18::new(budget, source.cleanup);
                        let input = SourceExpressionLeavesV18 {
                            leaves, ledger: &ledger, remaining: std::cell::Cell::new(3),
                        };
                        let mut charge = SourceCorrelationChargeV18 {
                            ledger: &ledger,
                            finite: UnsupportedIndexCorrelationBudgetV1 { remaining: MODULE_LIMIT },
                            finite_denied: false,
                        };
                        assert_eq!(input.symbol(leaf.symbol, leaf.scalar, &mut charge),
                            Some(NormalizedScalarExpressionV1::Symbol {
                                symbol: leaf.symbol, scalar: leaf.scalar,
                            }), "the unchanged original lookup must pass before each hostile case");
                        if fault != 0 {
                            let symbol = if fault == 1 { 0 } else { u32::MAX };
                            let ignored = input.symbol(symbol, leaf.scalar, &mut charge);
                            assert!(ignored.is_none());
                            assert_eq!(ledger.failure.get(), None,
                                "a source Binding refusal must not become a resource denial");
                            assert!(ledger.inconsistent_inventory.get());
                            let stopped = ledger.budget.borrow().work();
                            assert!(input.symbol(leaf.symbol, leaf.scalar, &mut charge).is_none());
                            assert_eq!(ledger.budget.borrow().work(), stopped,
                                "even a valid retry stops after the original failure");
                            let first = relation.source.guard.first.get().expect("original failure is retained");
                            assert!(matches!(first.error(), ProductionSourceOwnedViewErrorV18::Binding(actual)
                                if Some(actual) == detail));
                        } else {
                            assert_eq!(ledger.failure.get(), None);
                            assert!(!ledger.inconsistent_inventory.get());
                        }
                        drop(charge);
                        drop(input);
                        drop(ledger);
                        budget.release_storage(header)?;
                        assert_eq!(budget.storage(), floor);
                        completed.set(true);
                        Ok(())
                    });
                    if let Some(detail) = detail {
                        assert!(matches!(attempted, Err(ProductionSourceOwnedViewErrorV18::Binding(actual))
                            if actual == detail));
                        let stopped = budget.work();
                        assert!(matches!(source.root_count(budget),
                            Err(ProductionSourceOwnedViewErrorV18::Binding(actual)) if actual == detail));
                        assert_eq!(budget.work(), stopped);
                        // Ignore the nested refusal too. The original prepared
                        // owner must still reject this nominally successful callback.
                        Ok(())
                    } else {
                        attempted?;
                        assert!(source.root_count(budget)? > 0);
                        Ok(())
                    }
                })
            }))
        });
        assert!(completed.get(), "the actual original lookup and postflight must execute");
        if let Some(detail) = detail {
            assert!(matches!(result, Err(ProductionSourceOwnedViewErrorV18::Binding(actual))
                if actual == detail));
        } else {
            result.unwrap();
        }
        assert_eq!(budget.storage(), MODULE_FLOOR);
        assert_eq!(budget.failed_storage(), None);
    }
}

#[derive(Debug)]
enum SourceConsumerTestErrorV18 {
    Source(ProductionSourceOwnedViewErrorV18),
    Analysis(fe2o3_pliron::CanonicalAnalysisScopeErrorV1),
    Selected(Box<[u8; 3]>),
}

#[test]
fn caught_correspondence_construction_refusal_poison_is_not_lost_before_callback() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_v18(&mut budget);
    let observed = std::cell::Cell::new(None);
    let reached = std::cell::Cell::new(false);
    let result = prepared.with_source_consumer_v18(&mut budget,
        |source, budget| -> SourceOwnedResultV18<()> {
        source.with_analysis_v18(budget, |scope| scope.with_inventory_v1(|inventory, budget| {
            source.with_ranked_correspondence_v18(inventory, budget, |_, _| Ok::<_, ProductionSourceOwnedViewErrorV18>(()))?;
            let header = size_of::<Vec<SourceAttachmentV18>>()
                + size_of::<ProductionSourceCorrespondenceV18<'_>>()
                + size_of::<std::thread::Result<SourceOwnedResultV18<()>>>();
            let padding = budget.storage_limit() - budget.storage() - (header - 1);
            budget.reserve_storage(padding).unwrap();
            let before = budget.work();
            let error = source.with_ranked_correspondence_v18(inventory, budget,
                |_, _| -> SourceOwnedResultV18<()> { panic!("one-short construction cannot enter callback") }).unwrap_err();
            let ProductionSourceOwnedViewErrorV18::Resource(error) = error else { panic!("original storage refusal lost") };
            assert!(matches!(error, ArgumentResourceV1::Storage(_)));
            assert_eq!(budget.failed_storage(), Some(MODULE_LIMIT + 1));
            assert_eq!(budget.work(), before + 1, "only original entry query precedes prepayment");
            observed.set(Some(error));
            budget.release_storage(padding).unwrap();
            let stopped = budget.work();
            assert!(matches!(source.root_count(budget),
                Err(ProductionSourceOwnedViewErrorV18::Resource(actual)) if actual == error));
            assert_eq!(budget.work(), stopped);
            reached.set(true);
            Ok(())
        }))
    });
    assert!(reached.get(), "caught failure and retry must actually execute");
    assert!(matches!(result, Err(ProductionSourceOwnedViewErrorV18::Resource(actual))
        if Some(actual) == observed.get()));
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn caught_analysis_header_and_framework_construction_denials_remain_original() {
    for phase in 0..3 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_v18(&mut budget);
        let observed = std::cell::Cell::new(None);
        let reached = std::cell::Cell::new(false);
        let result = prepared.with_source_consumer_v18(&mut budget,
            |source, budget| -> SourceOwnedResultV18<()> {
            source.with_analysis_v18(budget, |scope| scope.with_inventory_v1(|_, _| Ok::<_, ProductionSourceOwnedViewErrorV18>(())))?;
            let header = size_of::<fe2o3_pliron::CanonicalAnalysisCleanupV1<'_>>()
                + size_of::<std::cell::Cell<bool>>()
                + size_of::<std::thread::Result<Result<(), SourceAnalysisBoundaryV18<ProductionSourceOwnedViewErrorV18>>>>();
            let padding = if phase == 0 {
                let padding = budget.storage_limit() - budget.storage() - (header - 1);
                budget.reserve_storage(padding).unwrap();
                padding
            } else {
                // Entry query consumes one unit. The shared framework then
                // prepays four. Phase 1 is one short for that exact atomic
                // batch; phase 2 reaches the real inventory constructor.
                let available = if phase == 1 { 4 } else { 5 };
                budget.charge_work(MODULE_LIMIT - budget.work() - available).unwrap();
                0
            };
            let before = budget.work();
            let error = source.with_analysis_v18(budget,
                |_| -> SourceOwnedResultV18<()> { panic!("failed construction cannot enter callback") }).unwrap_err();
            let ProductionSourceOwnedViewErrorV18::Resource(error) = error else { panic!("nested resource error was structurally flattened") };
            if phase == 0 {
                assert!(matches!(error, ArgumentResourceV1::Storage(_)));
                assert_eq!(budget.failed_storage(), Some(MODULE_LIMIT + 1));
                assert_eq!(budget.work(), before + 1);
            } else {
                assert!(matches!(error, ArgumentResourceV1::Work(_)));
                assert_eq!(budget.work(), before + if phase == 1 { 1 } else { 5 });
            }
            observed.set(Some(error));
            budget.release_storage(padding).unwrap();
            let stopped = budget.work();
            assert!(matches!(source.root_count(budget),
                Err(ProductionSourceOwnedViewErrorV18::Resource(actual)) if actual == error));
            assert_eq!(budget.work(), stopped);
            reached.set(true);
            Ok(())
        });
        assert!(reached.get());
        assert!(matches!(result, Err(ProductionSourceOwnedViewErrorV18::Resource(actual))
            if Some(actual) == observed.get()));
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn source_construction_reservation_and_release_have_exact_independent_boundaries() {
    for short in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_v18(&mut budget);
        let reached = std::cell::Cell::new(false);
        let result = prepared.with_source_consumer_v18(&mut budget,
            |source, budget| -> SourceOwnedResultV18<()> {
            let floor = budget.storage();
            let header = size_of::<Vec<SourceAttachmentV18>>()
                + size_of::<ProductionSourceCorrespondenceV18<'_>>()
                + size_of::<std::thread::Result<SourceOwnedResultV18<()>>>();
            let padding = MODULE_LIMIT - floor - header + usize::from(short);
            budget.reserve_storage(padding).unwrap();
            let selected = source.retain_construction(|| budget.reserve_storage(header).map_err(Into::into));
            if short {
                assert!(matches!(selected, Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(_)))));
                assert_eq!(budget.failed_storage(), Some(MODULE_LIMIT + 1));
            } else {
                selected.unwrap();
                assert_eq!(budget.storage(), MODULE_LIMIT);
                source.retain_construction(|| budget.release_storage(header).map_err(Into::into))?;
                assert!(source.root_count(budget)? > 0);
            }
            budget.release_storage(padding).unwrap();
            assert_eq!(budget.storage(), floor);
            reached.set(true);
            Ok(())
        });
        assert!(reached.get());
        assert_eq!(result.is_err(), short);
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
    // Final release cannot underflow after a valid production full-floor
    // check. Exercise the same retained settlement primitive directly on the
    // actual source ledger, without weakening that production check.
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_v18(&mut budget);
    let reached = std::cell::Cell::new(false);
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| -> SourceOwnedResultV18<()> {
        let floor = budget.storage();
        let failure = source.retain_construction(|| budget.release_storage(floor + 1).map_err(Into::into));
        assert!(matches!(failure, Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting))));
        assert_eq!(budget.storage(), floor);
        let stopped = budget.work();
        assert!(matches!(source.root_count(budget), Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting))));
        assert_eq!(budget.work(), stopped);
        reached.set(true);
        Ok(())
    });
    assert!(reached.get());
    assert!(matches!(result, Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting))));
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn source_construction_panics_keep_payload_and_cannot_be_caught_into_authority() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_v18(&mut budget);
    let reached = std::cell::Cell::new(false);
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| -> SourceOwnedResultV18<()> {
        let payload = Box::new([17_u8, 31, 53]);
        let pointer = payload.as_ptr();
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            source.retain_construction::<()>(|| std::panic::resume_unwind(payload))
        })).unwrap_err();
        let original = caught.downcast::<[u8; 3]>().unwrap();
        assert_eq!(original.as_ptr(), pointer, "raw payload is moved, never substituted");
        assert_eq!(*original, [17, 31, 53]);
        let stopped = budget.work();
        assert!(matches!(source.root_count(budget), Err(ProductionSourceOwnedViewErrorV18::Binding("source query construction panicked"))));
        assert_eq!(budget.work(), stopped);
        reached.set(true);
        Ok(())
    });
    assert!(reached.get());
    assert!(matches!(result, Err(ProductionSourceOwnedViewErrorV18::Binding("source query construction panicked"))));
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn structural_analysis_diagnostics_stay_typed_and_consumer_errors_are_not_framework_errors() {
    use fe2o3_pliron::CanonicalAnalysisScopeErrorV1 as Analysis;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_v18(&mut budget);
    let reached = std::cell::Cell::new(false);
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| -> SourceOwnedResultV18<()> {
        let selected = source.with_analysis_v18(budget,
            |_| Err::<(), _>(SourceConsumerTestErrorV18::Selected(Box::new([3, 5, 8]))));
        assert!(matches!(selected, Err(SourceConsumerTestErrorV18::Selected(value)) if *value == [3, 5, 8]));
        assert!(source.root_count(budget)? > 0, "selected callback error is not invented as a framework query");
        let failure = Analysis::Inventory(fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1::InconsistentOwner);
        let error = source.retain_query::<()>(Err(failure.into())).unwrap_err();
        assert!(matches!(error, ProductionSourceOwnedViewErrorV18::Analysis(Analysis::Inventory(
            fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1::InconsistentOwner))));
        assert!(std::error::Error::source(&error).is_some());
        let stopped = budget.work();
        assert!(matches!(source.root_count(budget), Err(ProductionSourceOwnedViewErrorV18::Analysis(Analysis::Inventory(
            fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1::InconsistentOwner)))));
        assert_eq!(budget.work(), stopped);
        reached.set(true);
        Ok(())
    });
    assert!(reached.get());
    assert!(matches!(result, Err(ProductionSourceOwnedViewErrorV18::Analysis(Analysis::Inventory(
        fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1::InconsistentOwner)))));
    assert_eq!(budget.storage(), MODULE_FLOOR);
}
impl From<ProductionSourceOwnedViewErrorV18> for SourceConsumerTestErrorV18 {
    fn from(error: ProductionSourceOwnedViewErrorV18) -> Self {
        Self::Source(error)
    }
}
impl From<fe2o3_pliron::CanonicalAnalysisScopeErrorV1> for SourceConsumerTestErrorV18 {
    fn from(error: fe2o3_pliron::CanonicalAnalysisScopeErrorV1) -> Self {
        Self::Analysis(error)
    }
}
impl From<ArgumentResourceV1> for SourceConsumerTestErrorV18 {
    fn from(error: ArgumentResourceV1) -> Self {
        Self::Source(error.into())
    }
}

// This test adapter exercises the allocation/index primitives on the real
// enclosing source ledger. It deliberately provides no source correspondence.
struct GeneratedIndexTestMeterV18<'a, 'b, 'w, 'c>(&'a CorrelationLedgerV18<'b, 'w, 'c>);

// This recipe is only a collision census input. It intentionally makes no
// claim to describe the actual source writes or their ranked effects.
fn scalar_leaf_collision_recipe_v18(function: &Function) -> fe2o3_pliron::ProductionRankedKernelV1 {
    fe2o3_pliron::ProductionRankedKernelV1::new(function.id.as_str(),
        function.signature.parameters.len(), vec![fe2o3_pliron::ProductionRankedBlockV1::new(
            vec![ProductionRankedOperationV1::SemanticSymbol { result: ProductionRankedValueIdV1::new(0), symbol: 0 },
                ProductionRankedOperationV1::SemanticSymbol { result: ProductionRankedValueIdV1::new(1), symbol: 2 }],
            fe2o3_pliron::ProductionRankedTerminatorV1::Return,
        )]).unwrap()
}

// A control/order fixture only. Allocation/value correspondence is deliberately
// not inferred from these test-created Access rows.
fn effect_order_recipe_v18(function: &Function, accesses: usize, backedge: bool)
    -> fe2o3_pliron::ProductionRankedKernelV1
{
    let mut operations = vec![
        ProductionRankedOperationV1::ViewInSpace {
            result: ProductionRankedValueIdV1::new(0), element_width: 32, writable: true, shape: vec![1], dynamic_extents: vec![],
            memory_space: dialect_kernel::MemorySpaceAttr::Private, allocation_origin: 1, noalias_class: 1,
        },
        ProductionRankedOperationV1::IndexConstant { result: ProductionRankedValueIdV1::new(1), value: 0 },
    ];
    operations.extend((0..accesses).map(|_| ProductionRankedOperationV1::Access {
        kind: dialect_kernel::AccessKindAttr::Write, view: ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0)),
        indices: vec![ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(1))],
    }));
    fe2o3_pliron::ProductionRankedKernelV1::new(function.id.as_str(),
        function.signature.parameters.len(), vec![fe2o3_pliron::ProductionRankedBlockV1::new(
            operations, if backedge { fe2o3_pliron::ProductionRankedTerminatorV1::Branch { target: 0 } }
            else { fe2o3_pliron::ProductionRankedTerminatorV1::Return },
        )]).unwrap()
}

fn original_store_order_rows_v18(
    relation: &ProductionSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Vec<SourceEffectLocationV18>> {
    let mut rows = Vec::new();
    for instance in 0..relation.source.instance_count(0, budget)? {
        let function = relation.source.instance(0, instance, budget)?.0;
        if function.index() != 2 { continue; }
        for statement in [1, 3] {
            let mut found = None;
            relation.visit_source_operations(0, instance, SemanticBlockIdV1::from_index(0),
                Some(statement), budget, |location, _budget| {
                    let ProductionSourceOperationV18::Operation(operation) = location else { return Ok(()); };
                    let actual = relation.inventory.functions()[operation.block.function.0 as usize]
                        .function.body.as_ref().unwrap().blocks[operation.block.block as usize]
                        .operations.get(operation.operation as usize).unwrap();
                    if matches!(actual.kind, OperationKind::Store { .. }) {
                        assert!(found.replace(operation).is_none());
                    }
                    Ok(())
                })?;
            rows.push(SourceEffectLocationV18 {
                site: SourceEffectSiteV18 { instance, function, block: SemanticBlockIdV1::from_index(0),
                    statement: Some(statement), ordinal: 0 },
                physical: found.expect("the original Store must be emitted"), access: 0,
                ranked: (0, 2 + u32::try_from(rows.len()).unwrap()),
            });
        }
    }
    assert_eq!(rows.len(), 4, "both stores from both real helper invocations are present");
    Ok(rows)
}

#[test]
fn actual_repeated_source_effect_order_uses_original_instances_and_real_cfg() {
    for fault in 0..6 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_v18(&mut budget);
        let reached = std::cell::Cell::new(false);
        let result = prepared.with_source_consumer_v18(&mut budget,
            |source, budget| -> Result<(), SourceConsumerTestErrorV18> {
            source.with_analysis_v18(budget, |scope| scope.with_inventory_v1(|inventory, budget| {
                source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                    let function = inventory.functions()[source.root(0, budget)?.1].function;
                    let mut rows = original_store_order_rows_v18(relation, budget)?;
                    let recipe = effect_order_recipe_v18(function, rows.len(), false);
                    let floor = budget.storage();
                    source_ranked_effect_order_v18(relation, 0, &recipe, &rows, budget)?;
                    assert_eq!(budget.storage(), floor, "unchanged control scratch is dropped before refund");
                    reached.set(true);
                    match fault {
                        0 => return Ok(()),
                        1 => { let first = rows[0].ranked; rows[0].ranked = rows[1].ranked; rows[1].ranked = first; }
                        2 => rows[0].site.instance = rows[2].site.instance,
                        3 => rows[0].access = 1,
                        4 => rows[2].ranked = rows[0].ranked,
                        5 => {}
                        _ => unreachable!(),
                    }
                    let cyclic = effect_order_recipe_v18(function, rows.len(), true);
                    let recipe = if fault == 5 { &cyclic } else { &recipe };
                    let error = source_ranked_effect_order_v18(relation, 0, recipe, &rows, budget).unwrap_err();
                    assert!(matches!(error, ProductionSourceOwnedViewErrorV18::Binding(_)));
                    assert_eq!(budget.storage(), floor, "ordinary mismatch refunds only dropped scratch");
                    let stopped = budget.work();
                    assert!(relation.inventory(budget).is_err(), "caught order mismatch poisons the containing source query");
                    assert_eq!(budget.work(), stopped);
                    Err(error.into())
                })
            }))
        });
        assert!(reached.get(), "each hostile case first passes the unmodified real-source control");
        assert_eq!(result.is_ok(), fault == 0);
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn effect_order_scratch_has_independent_exact_storage_and_work_boundaries() {
    fn allocate(budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<SourceFlowScratchV18> {
        let mut scratch = SourceFlowScratchV18::new(3, 4, 2, budget)?;
        // Allocation-only input, not a fabricated source/effect correspondence.
        scratch.next_count = 4;
        scratch.prepare_output(budget)?;
        Ok(scratch)
    }
    let expected = size_of::<SourceFlowScratchV18>() + 3 * size_of::<u8>()
        + 9 * size_of::<u32>() + 4 * size_of::<SourceEffectSiteV18>()
        + 4 * size_of::<(SourceEffectSiteV18, SourceEffectSiteV18)>();
    // Vec/VecDeque exact reservations may overallocate. Derive the actual
    // reservation from their public capacities, not the producer's receipt.
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let scratch = allocate(&mut budget).unwrap();
    let actual = size_of::<SourceFlowScratchV18>() + scratch.visited.capacity() * size_of::<u8>()
        + scratch.pending.capacity() * size_of::<u32>()
        + (scratch.entry.capacity() + scratch.found.capacity()) * size_of::<SourceEffectSiteV18>()
        + scratch.next.capacity() * size_of::<(SourceEffectSiteV18, SourceEffectSiteV18)>();
    assert!(actual >= expected);
    assert_eq!(budget.storage(), MODULE_FLOOR + actual);
    assert_eq!(budget.work(), 15, "four three-work vector reservations plus three visited initializations");
    drop(scratch);
    budget.release_storage(actual).unwrap();
    for short in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(15);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_FLOOR + actual - usize::from(short));
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let result = allocate(&mut budget);
        assert_eq!(result.is_ok(), !short);
        if let Err(error) = result {
            assert!(matches!(error, ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(_))));
        }
    }
    let mut work = CanonicalKernelIrWorkBudgetV1::new(14);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_FLOOR + actual);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    assert!(matches!(allocate(&mut budget),
        Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(_)))));
    assert_eq!(budget.work(), 12, "the denied final three-work reservation is atomic");
}

#[test]
fn generated_reader_retains_ignored_closure_tail_errors_before_option_erasure() {
    for resource_failure in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_v18(&mut budget);
        let reached = std::cell::Cell::new(false);
        let result = prepared.with_source_consumer_v18(&mut budget,
            |source, budget| -> Result<(), SourceConsumerTestErrorV18> {
            source.with_analysis_v18(budget, |scope| scope.with_inventory_v1(|inventory, budget| {
                source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                    let function = inventory.functions()[source.root(0, budget)?.1].coordinate;
                    value_origin_v1::with_whole_value_origins_v18(relation, function, budget, |origins, budget| {
                        let header = size_of::<CorrelationLedgerV18<'_, '_, '_>>()
                            + size_of::<SourceGeneratedRecipeReaderV18<'_, '_, '_, '_, '_>>();
                        budget.reserve_storage(header)?;
                        let ledger = CorrelationLedgerV18::new(budget, source.cleanup);
                        let reader = SourceGeneratedRecipeReaderV18 { relation, root: 0, instance: 0, origins, ledger: &ledger };
                        assert!(reader.operations(0).is_some(), "the real original instance first passes its reader query");
                        let ignored: Option<()> = reader.query(|budget| {
                            relation.query(budget)?;
                            if resource_failure {
                                budget.charge_work(MODULE_LIMIT - budget.work() + 1)?;
                            }
                            Err(ProductionSourceOwnedViewErrorV18::Binding("generated reader selected tail mismatch"))
                        });
                        assert!(ignored.is_none());
                        let stopped = ledger.budget.borrow().work();
                        assert!(reader.operations(0).is_none(), "None must not reset the reader into a successful no-op");
                        assert_eq!(ledger.budget.borrow().work(), stopped);
                        assert_eq!(ledger.failure.get().is_some(), resource_failure);
                        assert_eq!(ledger.inconsistent_inventory.get(), !resource_failure);
                        drop(reader);
                        drop(ledger);
                        let error = match relation.inventory(budget) {
                            Err(error) => error,
                            Ok(_) => panic!("ignoring a private reader error cannot preserve source authority"),
                        };
                        assert!(if resource_failure {
                            matches!(error, ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(_)))
                        } else {
                            matches!(error, ProductionSourceOwnedViewErrorV18::Binding("generated reader selected tail mismatch"))
                        });
                        assert_eq!(budget.work(), stopped);
                        reached.set(true);
                        Err(error.into())
                    })
                })
            }))
        });
        assert!(reached.get());
        assert!(if resource_failure {
            matches!(result, Err(SourceConsumerTestErrorV18::Source(
                ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(_)))))
        } else {
            matches!(result, Err(SourceConsumerTestErrorV18::Source(
                ProductionSourceOwnedViewErrorV18::Binding("generated reader selected tail mismatch"))))
        });
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn shared_ranked_decoder_uses_paid_sorted_source_metadata_and_preserves_legacy_queries() {
    for fault in 0..4 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_v18(&mut budget);
        let reached = std::cell::Cell::new(false);
        let result = prepared.with_source_consumer_v18(&mut budget,
            |source, budget| -> Result<(), SourceConsumerTestErrorV18> {
            source.with_analysis_v18(budget, |scope| scope.with_inventory_v1(|inventory, budget| {
                source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                    let function = inventory.functions()[source.root(0, budget)?.1].function;
                    let recipe = effect_order_recipe_v18(function, 2, false);
                    let mut sources = [
                        ProductionRankedAccessSourceV1 { semantic_block: 0, semantic_statement: Some(0),
                            semantic_access_ordinal: 0, ranked_block: 0, ranked_operation: 2, output_extent: None },
                        ProductionRankedAccessSourceV1 { semantic_block: 0, semantic_statement: Some(0),
                            semantic_access_ordinal: 1, ranked_block: 0, ranked_operation: 3, output_extent: None },
                    ];
                    // These are inert metadata inputs, not evidence that the
                    // recipe describes this source's actual allocations/RHS.
                    let mut old_work = UnsupportedIndexCorrelationBudgetV1 { remaining: MODULE_LIMIT };
                    let legacy = index_ranked_correlation(&recipe, &sources, MODULE_LIMIT, &mut old_work).unwrap();
                    let floor = budget.storage();
                    let rows = SourceRankedIndexDataV18::build(relation, &recipe, &sources, MODULE_LIMIT, budget)?;
                    let bytes = size_of::<SourceRankedIndexDataV18<'_>>()
                        + size_of::<CorrelationLedgerV18<'_, '_, '_>>()
                        + size_of::<SourceCorrelationChargeV18<'_, '_, '_, '_>>()
                        + rows.sources.capacity() * size_of::<(SemanticAccessSiteV1, IndexedRankedAccessSourceV1)>()
                        + rows.locations.capacity() * size_of::<usize>()
                        + rows.conservative.capacity() * size_of::<(u32, Option<u32>, usize)>()
                        + rows.views.capacity() * size_of::<(ProductionRankedValueIdV1, RankedViewDefinitionV1)>()
                        + rows.expressions.capacity() * size_of::<(ProductionRankedValueIdV1,
                            (&ProductionSemanticExpressionV2, ProductionNumericalContractV2))>();
                    assert_eq!(budget.storage() - floor, bytes, "all actual source metadata capacities remain paid");
                    let query_header = size_of::<SourceRankedIndexV18<'_, '_, '_, '_, '_>>();
                    budget.reserve_storage(query_header)?;
                    let ledger = CorrelationLedgerV18::new(budget, source.cleanup);
                    let index = SourceRankedIndexV18 { rows: &rows, ledger: &ledger };
                    for original in &sources {
                        let site = SemanticAccessSiteV1 { block: original.semantic_block,
                            statement: original.semantic_statement, ordinal: original.semantic_access_ordinal };
                        let expected = legacy.source(site).unwrap();
                        let actual = index.source(site).unwrap();
                        assert_eq!((actual.ranked_block, actual.ranked_operation, actual.access, actual.atomic),
                            (expected.ranked_block, expected.ranked_operation, expected.access, expected.atomic));
                        assert_eq!(index.site((actual.ranked_block, actual.ranked_operation)), Some(site));
                    }
                    assert_eq!(index.sources().count(), legacy.sources().count());
                    let view = ProductionRankedValueIdV1::new(0);
                    assert_eq!(index.view(view).unwrap().memory_space, legacy.view(view).unwrap().memory_space);
                    assert_eq!(index.view(view).unwrap().allocation_origin, legacy.view(view).unwrap().allocation_origin);
                    assert!(index.conservative_source(0, Some(0)).is_none());
                    assert!(index.source(SemanticAccessSiteV1 { block: 1, statement: Some(0), ordinal: 0 }).is_none());
                    assert!(index.expression(view).is_none());
                    assert_eq!(ledger.failure.get(), None);
                    assert!(!ledger.inconsistent_inventory.get());
                    drop(index);
                    drop((ledger, rows, legacy));
                    budget.release_storage(bytes + query_header)?;
                    assert_eq!(budget.storage(), floor);
                    reached.set(true);
                    match fault {
                        0 => return Ok(()),
                        1 => sources[1].semantic_access_ordinal = 0,
                        2 => sources[1].ranked_operation = 2,
                        3 => sources[1].semantic_access_ordinal = 2,
                        _ => unreachable!(),
                    }
                    let mut old_work = UnsupportedIndexCorrelationBudgetV1 { remaining: MODULE_LIMIT };
                    assert!(index_ranked_correlation(&recipe, &sources, MODULE_LIMIT, &mut old_work).is_none());
                    let error = match SourceRankedIndexDataV18::build(relation, &recipe, &sources, MODULE_LIMIT, budget) {
                        Ok(_) => panic!("source metadata cannot accept a decoder/ordinal mutation rejected by legacy"),
                        Err(error) => error,
                    };
                    assert!(matches!(error, ProductionSourceOwnedViewErrorV18::Binding(_)));
                    Err(error.into())
                })
            }))
        });
        assert!(reached.get());
        assert_eq!(result.is_ok(), fault == 0);
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn actual_scalar_store_request_observes_lost_floor_before_new_expression_credit() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_v18(&mut budget);
    let entered = std::cell::Cell::new(false);
    let refused_floor = std::cell::Cell::new(0usize);
    let result = prepared.with_source_consumer_v18(&mut budget,
        |source, budget| -> Result<(), SourceConsumerTestErrorV18> {
        source.with_analysis_v18(budget, |scope| scope.with_inventory_v1(|inventory, budget| {
            source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                let physical = source.root(0, budget)?.1;
                let recipe = scalar_leaf_collision_recipe_v18(inventory.functions()[physical].function);
                relation.with_scalar_leaves_v18(0, &recipe, budget, |leaves, budget| {
                    leaves.visit_store_inputs(budget,
                        |request, budget| -> Result<(), SourceConsumerTestErrorV18> {
                        let scalar = request.scalar(budget)?;
                        assert_eq!(scalar, ProductionSemanticScalarTypeV2::Integer { signed: false, bits: 32 });
                        assert_eq!(request.floor, budget.storage());
                        let incoming = budget.storage();
                        budget.release_storage(1)?;
                        let before = budget.work();
                        let error = request.check_expression(&ProductionSemanticExpressionV2::Constant {
                            scalar, bits: 0,
                        }, budget).unwrap_err();
                        assert!(matches!(error, ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)));
                        assert_eq!(budget.work(), before, "lost incoming custody refuses before expression work");
                        assert_eq!(budget.storage(), incoming - 1, "no new header credit may hide the missing byte");
                        budget.reserve_storage(1)?;
                        let restored_work = budget.work();
                        let retry = request.scalar(budget).unwrap_err();
                        assert!(matches!(retry, ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)));
                        assert_eq!(budget.work(), restored_work, "restoration does not clear the first denial");
                        assert_eq!(budget.storage(), incoming);
                        refused_floor.set(incoming);
                        entered.set(true);
                        Err(error.into())
                    }).map(|_| ())
                })
            })
        }))
    });
    assert!(entered.get(), "the actual source request must reach its hostile boundary");
    assert!(matches!(result, Err(SourceConsumerTestErrorV18::Source(
        ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)))));
    assert_eq!(budget.storage(), refused_floor.get(), "no containing scope may refund the denied reservation");
    assert!(budget.storage() > MODULE_FLOOR);
}

#[test]
fn ignored_scalar_query_tail_errors_remain_sticky_through_finalization() {
    for work_tail in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_v18(&mut budget);
        let entered = std::cell::Cell::new(false);
        let result = prepared.with_source_consumer_v18(&mut budget,
            |source, budget| -> Result<(), SourceConsumerTestErrorV18> {
            source.with_analysis_v18(budget, |scope| scope.with_inventory_v1(|inventory, budget| {
                source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                    let physical = source.root(0, budget)?.1;
                    let recipe = scalar_leaf_collision_recipe_v18(inventory.functions()[physical].function);
                    let ignored = relation.with_scalar_leaves_v18(0, &recipe, budget, |leaves, budget| {
                        let ignored = leaves.visit_store_inputs(budget,
                            |request, budget| -> Result<(), SourceConsumerTestErrorV18> {
                            let (_, original) = request.original(budget)?;
                            let before = budget.work();
                            request.check(budget)?;
                            let check_work = budget.work() - before;
                            let before = budget.work();
                            let _ = request.input_for(original, budget)?;
                            assert_eq!(budget.work() - before, check_work + 4,
                                "the independently selected tail is four work units after the same check");
                            if work_tail {
                                let remaining = check_work.checked_add(3).unwrap();
                                budget.charge_work(MODULE_LIMIT.checked_sub(budget.work())
                                    .and_then(|available| available.checked_sub(remaining)).unwrap())?;
                                let ignored = match request.input_for(original, budget) {
                                    Err(error) => error,
                                    Ok(_) => panic!("the four-work public-query tail must refuse with only three remaining"),
                                };
                                assert!(matches!(ignored, ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(_))));
                            } else {
                                let ignored = request.check_expression(&ProductionSemanticExpressionV2::Constant {
                                    scalar: ProductionSemanticScalarTypeV2::Bool, bits: 0,
                                }, budget).unwrap_err();
                                assert!(matches!(ignored, ProductionSourceOwnedViewErrorV18::Binding(
                                    "scalar Store expression type changed")));
                            }
                            let stopped = budget.work();
                            let retry = request.scalar(budget).unwrap_err();
                            assert!(if work_tail {
                                matches!(retry, ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(_)))
                            } else {
                                matches!(retry, ProductionSourceOwnedViewErrorV18::Binding("scalar Store expression type changed"))
                            });
                            assert_eq!(budget.work(), stopped, "ignored failure cannot authorize a later query");
                            entered.set(true);
                            Ok(())
                        });
                        assert!(ignored.is_err());
                        Ok::<_, SourceConsumerTestErrorV18>(())
                    });
                    assert!(ignored.is_err());
                    Ok::<_, SourceConsumerTestErrorV18>(())
                })
            }))
        });
        assert!(entered.get(), "the unchanged Store query must run before the ignored error");
        assert!(if work_tail {
            matches!(result, Err(SourceConsumerTestErrorV18::Source(
                ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(_)))))
        } else {
            matches!(result, Err(SourceConsumerTestErrorV18::Source(
                ProductionSourceOwnedViewErrorV18::Binding("scalar Store expression type changed"))))
        }, "the original query refusal must survive ignored visitor and leaf-scope errors");
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn actual_read_to_store_expression_uses_the_exact_private_occurrence_symbol() {
    for substitute in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_from_v18(scalar_read_store_owner_v18, &mut budget);
        let controls = std::cell::Cell::new(0usize);
        let mutated = std::cell::Cell::new(false);
        let result = prepared.with_source_consumer_v18(&mut budget,
            |source, budget| -> Result<(), SourceConsumerTestErrorV18> {
            source.with_analysis_v18(budget, |scope| scope.with_inventory_v1(|inventory, budget| {
                source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                    let physical = source.root(0, budget)?.1;
                    let recipe = scalar_leaf_collision_recipe_v18(inventory.functions()[physical].function);
                    relation.with_scalar_leaves_v18(0, &recipe, budget, |leaves, budget| {
                        leaves.visit_store_inputs(budget,
                            |request, budget| -> Result<(), SourceConsumerTestErrorV18> {
                            if !matches!(request.source, ScopedMemoryStoreSourceV29::Operand {
                                source: ScopedMemoryOperandSourceV29::Memory { .. }, ..
                            }) { return Ok(()); }
                            let ProductionSourceScalarInputV18::Operand {
                                operand: SemanticOperandV1::Copy(place), ..
                            } = request.input(budget)? else { panic!("the real retained operand is Copy"); };
                            let (instance, function) = request.original(budget)?;
                            let expression = leaves.original_place(instance, function, place, budget)?
                                .expect("the exact original operand has an actual prior Load");
                            request.check_expression(&expression, budget)?;
                            controls.set(controls.get() + 1);
                            if substitute {
                                let ProductionSemanticExpressionV2::Symbol { symbol, scalar } = expression else {
                                    panic!("a source-only read names one exact occurrence");
                                };
                                let different = leaves.leaves.rows.iter()
                                    .find(|row| row.scalar == scalar && row.symbol != symbol)
                                    .expect("the repeated source fixture has another same-typed actual read");
                                mutated.set(true);
                                request.check_expression(&ProductionSemanticExpressionV2::Symbol {
                                    symbol: different.symbol, scalar,
                                }, budget)?;
                            }
                            Ok(())
                        }).map(|_| ())
                    })
                })
            }))
        });
        if substitute {
            assert!(mutated.get() && controls.get() > 0);
            assert!(matches!(result, Err(SourceConsumerTestErrorV18::Source(
                ProductionSourceOwnedViewErrorV18::Binding("actual scalar expression differs from its original source value")))));
        } else {
            result.unwrap();
            assert!(controls.get() >= 2, "both original helper instances reach the shared normalizer");
        }
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn actual_scalar_store_normalization_preserves_original_constant_and_rejects_typed_substitution() {
    for fault in 0..4 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_v18(&mut budget);
        let controls = std::cell::Cell::new(0usize);
        let mutated = std::cell::Cell::new(false);
        let result = prepared.with_source_consumer_v18(&mut budget,
            |source, budget| -> Result<(), SourceConsumerTestErrorV18> {
            source.with_analysis_v18(budget, |scope| scope.with_inventory_v1(|inventory, budget| {
                source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                    let root = source.root(0, budget)?.1;
                    let recipe = scalar_leaf_collision_recipe_v18(inventory.functions()[root].function);
                    relation.with_scalar_leaves_v18(0, &recipe, budget, |leaves, budget| {
                        let visited = leaves.visit_store_inputs(budget,
                            |request, budget| -> Result<(), SourceConsumerTestErrorV18> {
                            let ProductionSourceScalarInputV18::Operand {
                                operand: SemanticOperandV1::Constant(original), ..
                            } = request.input(budget)? else { return Ok(()); };
                            let SemanticConstantValueV1::Scalar(original) = original.value() else {
                                panic!("the admitted fixture's literal is scalar");
                            };
                            let expected = lower_constant(lower_scalar_type(
                                source.source_semantic(budget)?.types(), U32).unwrap(), *original).unwrap();
                            let (scalar, bits) = normalize_kir_constant_v1(&expected).unwrap();
                            let expression = ProductionSemanticExpressionV2::Constant { scalar, bits };
                            request.check_expression(&expression, budget)?;
                            controls.set(controls.get() + 1);
                            if fault == 0 { return Ok(()); }
                            mutated.set(true);
                            match fault {
                                1 => request.check_expression(&ProductionSemanticExpressionV2::Constant {
                                    scalar, bits: bits ^ 1,
                                }, budget)?,
                                2 | 3 => {
                                    let mut changed = ProductionSourceScalarStoreV18 {
                                        leaves: request.leaves, arguments: request.arguments, origins: request.origins,
                                        inline_scalar: request.inline_scalar,
                                        instance: request.instance, function: request.function,
                                        row: request.row, anchor: request.anchor, operation: request.operation,
                                        source: request.source, scalar: request.scalar, value: request.value, floor: request.floor,
                                    };
                                    if fault == 2 {
                                        changed.value = leaves.leaves.rows.iter()
                                            .find(|row| row.scalar == scalar && row.value != request.value)
                                            .expect("actual same-typed helper read exists").value;
                                    } else {
                                        let function = source.instance(0, request.instance, budget)?.0;
                                        let mut alternate = None;
                                        for instance in 0..source.instance_count(0, budget)? {
                                            if instance != request.instance
                                                && source.instance(0, instance, budget)?.0 == function
                                            { alternate = Some(instance); break; }
                                        }
                                        changed.instance = alternate.expect("the same original helper is invoked twice");
                                    }
                                    changed.check_expression(&expression, budget)?;
                                }
                                _ => unreachable!(),
                            }
                            panic!("original scalar Store mutation was accepted");
                        })?;
                        assert!(visited >= controls.get());
                        Ok::<_, SourceConsumerTestErrorV18>(())
                    })
                })
            }))
        });
        assert!(controls.get() > 0, "the real original Store must pass before mutation");
        if fault == 0 {
            result.unwrap();
            assert!(controls.get() >= 2, "both original repeated helper literals were compared");
        } else {
            assert!(mutated.get());
            assert!(matches!(result, Err(SourceConsumerTestErrorV18::Source(
                ProductionSourceOwnedViewErrorV18::Binding(_)))));
        }
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn actual_source_scalar_leaves_keep_occurrences_instances_and_symbol_names_distinct() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_v18(&mut budget);
    let completed = std::cell::Cell::new(false);
    prepared.with_source_consumer_v18(&mut budget, |source, budget| -> Result<(), SourceConsumerTestErrorV18> {
        source.with_analysis_v18(budget, |scope| scope.with_inventory_v1(|inventory, budget| {
            source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                let physical = source.root(0, budget)?.1;
                let function = inventory.functions()[physical].function;
                let recipe = scalar_leaf_collision_recipe_v18(function);
                relation.with_scalar_leaves_v18(0, &recipe, budget, |view, budget| {
                    let leaves = view.leaves;
                    assert!(leaves.rows.len() >= 4, "the actual repeated helper must emit scalar reads");
                    for (index, row) in leaves.rows.iter().enumerate() {
                        assert_ne!(row.symbol, 0);
                        assert_ne!(row.symbol, 2);
                        assert!(row.symbol < PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2);
                        assert!(leaves.rows[..index].iter().all(|previous| previous.symbol != row.symbol));
                        let semantic = source.source_semantic(budget)?;
                        let original = source.instance(0, row.instance, budget)?.0;
                        let declaration = &semantic.functions()[original.index() as usize];
                        let place = match scoped_source_operand_v29(declaration, row.read.site, row.read.role) {
                            Some(SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) => place,
                            _ => scoped_source_place_v29(declaration, row.read.site, row.read.role).unwrap(),
                        };
                        assert_eq!(view.original_place(row.instance, declaration, place, budget)?,
                            Some(ProductionSemanticExpressionV2::Symbol { symbol: row.symbol, scalar: row.scalar }));
                        assert_eq!(leaves.actual_value(function, row.value, budget)?,
                            Some(NormalizedScalarExpressionV1::Symbol { symbol: row.symbol, scalar: row.scalar }));
                    }
                    assert!(leaves.rows.iter().any(|first| leaves.rows.iter().any(|second|
                        first.instance != second.instance && first.place == second.place
                            && first.read == second.read && first.symbol != second.symbol
                            && first.value != second.value)), "same lexical helper reads retain distinct invocation identities");
                    let first = leaves.rows[0];
                    assert_eq!(first.symbol, 1, "source occurrence order receives the first unoccupied public name");
                    let before_adapter = budget.storage();
                    let arguments = SourceRootArgumentsV18::build(relation, 0, budget)?;
                    budget.reserve_storage(size_of::<CorrelationLedgerV18<'_, '_, '_>>()
                        + size_of::<SourceScalarNormalizationInputV18<'_, '_, '_>>()
                        + size_of::<InventoryCorrelationV18<'_, '_, '_, '_, '_, '_>>())?;
                    let ledger = CorrelationLedgerV18::new(budget, source.cleanup);
                    let scalar_source = SourceScalarNormalizationInputV18 { leaves, arguments: &arguments };
                    let graph = InventoryCorrelationV18 { origins: None, inventory, function: inventory.functions()[physical].coordinate,
                        ledger: &ledger, inline_scalar: &Gfx942InlineScalarCorrespondenceV30::empty(),
                        scalar_source: Some(&scalar_source) };
                    for leaf in &leaves.rows {
                        assert_eq!(graph.scalar_leaf(function, leaf.value), Some(Some(
                            NormalizedScalarExpressionV1::Symbol { symbol: leaf.symbol, scalar: leaf.scalar })));
                    }
                    let scalar = kir_semantic_scalar_v1(&function.signature.parameters[0]).unwrap();
                    assert_eq!(graph.scalar_argument(function, 0, scalar), Some(NormalizedScalarExpressionV1::Symbol {
                        symbol: PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2, scalar,
                    }));
                    assert_eq!(ledger.failure.get(), None);
                    assert!(!ledger.inconsistent_inventory.get());
                    drop(graph);
                    drop(scalar_source);
                    drop(ledger);
                    drop(arguments);
                    budget.release_storage(budget.storage() - before_adapter)?;
                    completed.set(true);
                    Ok::<_, SourceConsumerTestErrorV18>(())
                })
            })
        }))
    }).unwrap();
    assert!(completed.get());
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn actual_source_scalar_leaf_queries_reject_coherent_typed_result_and_capture_substitution() {
    for fault in 0..6 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_v18(&mut budget);
        let positive = std::cell::Cell::new(false);
        let rejected = std::cell::Cell::new(false);
        let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| -> Result<(), SourceConsumerTestErrorV18> {
            source.with_analysis_v18(budget, |scope| scope.with_inventory_v1(|inventory, budget| {
                source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                    let function = inventory.functions()[source.root(0, budget)?.1].function;
                    let recipe = scalar_leaf_collision_recipe_v18(function);
                    let mut leaves = SourceScalarLeavesV18::build(relation, 0, &recipe, budget)?;
                    let first = leaves.rows[0];
                    let other = *leaves.rows.iter().find(|row| row.instance != first.instance
                        && row.place == first.place && row.scalar == first.scalar).unwrap();
                    assert_eq!(leaves.actual_value(function, first.value, budget)?,
                        Some(NormalizedScalarExpressionV1::Symbol { symbol: first.symbol, scalar: first.scalar }));
                    positive.set(true);
                    match fault {
                        0 => leaves.rows[0].symbol = other.symbol,
                        1 => leaves.rows[0].instance = other.instance,
                        2 => {
                            leaves.rows[0].operation = other.operation;
                            leaves.rows[0].value = other.value;
                        }
                        3 => {
                            let replacement = *leaves.rows.iter().find(|row|
                                row.instance == first.instance && row.read != first.read && row.scalar == first.scalar).unwrap();
                            leaves.rows[0].read = replacement.read;
                        }
                        4 => leaves.rows[0].anchor = usize::MAX,
                        5 => {}
                        _ => unreachable!(),
                    }
                    let query = if fault == 5 {
                        let other_function = inventory.functions()[source.root(1, budget)?.1].function;
                        assert_eq!(other_function.signature.parameters, function.signature.parameters);
                        leaves.actual_value(other_function, first.value, budget)
                    } else {
                        leaves.actual_value(function, first.value, budget)
                    };
                    rejected.set(matches!(query, Err(ProductionSourceOwnedViewErrorV18::Binding(_))));
                    query.map(|_| ()).map_err(SourceConsumerTestErrorV18::from)
                })
            }))
        });
        assert!(positive.get(), "the real source/canonical positive must run before each fault");
        assert!(rejected.get());
        assert!(matches!(result, Err(SourceConsumerTestErrorV18::Source(ProductionSourceOwnedViewErrorV18::Binding(_)))));
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn source_scalar_symbol_census_uses_original_bounded_expression_tree_and_exact_work() {
    use ProductionSemanticExpressionV2 as E;
    let scalar = ProductionSemanticScalarTypeV2::Integer { signed: false, bits: 32 };
    let expression = E::Binary { operation: ProductionSemanticBinaryOpV2::Add, scalar,
        overflow: ProductionOverflowContractV2::Wrapping,
        lhs: Box::new(E::Symbol { symbol: 0, scalar }),
        rhs: Box::new(E::Symbol { symbol: 2, scalar }) };
    const STACK: usize = MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 * 2 + 1;
    // Fixed stack initialization, three node visits and two child pushes.
    const REQUIRED: usize = STACK + 3 + 2;
    for limit in [REQUIRED - 1, REQUIRED] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let storage = size_of::<[Option<(&E, usize)>; STACK]>();
        let mut budget = ArgumentBudgetV1::new(&mut work, storage);
        budget.reserve_storage(storage).unwrap();
        let mut observed = [u32::MAX; 2];
        let mut count = 0;
        let result = visit_source_expression_symbols_v18(&expression, &mut budget, &mut |symbol, _| {
            observed[count] = symbol;
            count += 1;
            Ok(())
        });
        if limit == REQUIRED {
            result.unwrap();
            assert_eq!(observed, [0, 2]);
            assert_eq!(count, 2);
            assert_eq!(budget.work(), REQUIRED);
        } else {
            assert!(matches!(result, Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(_)))));
            assert_eq!(count, 1);
        }
        assert_eq!(budget.storage(), storage);
        budget.release_storage(storage).unwrap();
    }
}

#[test]
fn actual_root_parameter_origins_use_checked_source_argument_rows_not_slot_equality() {
    for fault in 0..3 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_v18(&mut budget);
        let completed = std::cell::Cell::new(false);
        let rejected = std::cell::Cell::new(false);
        let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| -> Result<(), SourceConsumerTestErrorV18> {
            source.with_analysis_v18(budget, |scope| scope.with_inventory_v1(|inventory, budget| {
                source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                    let physical = source.root(0, budget)?.1;
                    let function = inventory.functions()[physical].function;
                    let mut arguments = SourceRootArgumentsV18::build(relation, 0, budget)?;
                    assert_eq!(arguments.parameters.len(), function.signature.parameters.len());
                    assert_eq!(arguments.parameters.len(), 1, "the original root takes its one source U32 argument");
                    assert_eq!(arguments.original_argument(function, 0, budget)?, 0);
                    completed.set(true);
                    if fault == 0 { return Ok::<_, SourceConsumerTestErrorV18>(()); }
                    let result = if fault == 1 {
                        let first = arguments.parameters[0].as_mut().unwrap();
                        first.value = ValueId(first.value.0.checked_add(1).unwrap());
                        arguments.original_argument(function, 0, budget)
                    } else {
                        let other_root = source.root(1, budget)?.1;
                        let other = inventory.functions()[other_root].function;
                        assert_eq!(other.signature.parameters, function.signature.parameters,
                            "the hostile original root has the same physical parameter types");
                        arguments.original_argument(other, 0, budget)
                    };
                    rejected.set(matches!(&result, Err(ProductionSourceOwnedViewErrorV18::Binding(_))));
                    result.map(|_| ()).map_err(SourceConsumerTestErrorV18::from)
                })
            }))
        });
        assert!(completed.get(), "actual source/ABI correspondence must run before the hostile query");
        if fault == 0 { result.unwrap(); }
        else {
            assert!(rejected.get());
            assert!(matches!(result, Err(SourceConsumerTestErrorV18::Source(ProductionSourceOwnedViewErrorV18::Binding(_)))));
        }
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

impl GeneratedRecipeSourceV18 for GeneratedIndexTestMeterV18<'_, '_, '_, '_> {
    fn values(&self, _: u32) -> Option<GeneratedRecipeValuesV18> { panic!("index test has no source values") }
    fn operations(&self, _: u32) -> Option<NeutralRecipeOperationsV18<'_>> { panic!("index test has no operations") }
    fn producer_contains(&self, _: u32, _: FunctionOperationLocation) -> Option<bool> { panic!("index test has no producer") }
    fn operation_origin(&self, _: &FunctionBody, _: &dyn KirCorrelationGraphV18, _: ValueId) -> Option<ValueId> {
        panic!("index test has no origin authority")
    }
    fn charge(&self, amount: usize) -> Result<(), ProductionMirPlironTranslationErrorV1> {
        self.0.with_budget(|budget| budget.charge_work(amount))
            .map_err(|_| ProductionMirPlironTranslationErrorV1::ResourceLimit)
    }
    fn reserve(&self, bytes: usize) -> Result<(), ProductionMirPlironTranslationErrorV1> {
        self.0.with_budget(|budget| budget.reserve_storage(bytes))
            .map_err(|_| ProductionMirPlironTranslationErrorV1::ResourceLimit)
    }
}

#[test]
fn generated_effect_index_has_independent_exact_work_storage_and_no_retry_after_denial() {
    // Two rows pay 2 visits. Heapsort [2, 1] pays 1 + 10 + 5.
    // Lower/upper bound of 1 in [1, 2] pay (7 + 7 + 1) + (7 + 8 + 1).
    const INDEX_WORK: usize = 2 + 1 + 10 + 5 + 15 + 16;
    let payload = std::mem::size_of::<Vec<u64>>() + 2 * std::mem::size_of::<u64>();
    for (short_work, short_storage) in [(false, false), (true, false), (false, true)] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = prepared_source_fixture(ModuleFixture::Ordinary, false, &mut budget);
        let completed = std::cell::Cell::new(false);
        let denial = std::cell::Cell::new(None);
        let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| -> SourceOwnedResultV18<()> {
            let floor = budget.storage();
            let header = std::mem::size_of::<CorrelationLedgerV18<'_, '_, '_>>()
                + std::mem::size_of::<GeneratedIndexTestMeterV18<'_, '_, '_, '_>>();
            budget.reserve_storage(header)?;
            let space = payload.checked_sub(usize::from(short_storage)).unwrap();
            let padding = MODULE_LIMIT.checked_sub(budget.storage()).unwrap().checked_sub(space).unwrap();
            budget.reserve_storage(padding)?;
            let remaining = INDEX_WORK - usize::from(short_work);
            let work_padding = MODULE_LIMIT.checked_sub(budget.work()).unwrap().checked_sub(remaining).unwrap();
            budget.charge_work(work_padding)?;
            let before_work = budget.work();
            let before_storage = budget.storage();
            let ledger = CorrelationLedgerV18::new(budget, source.cleanup);
            let reader = GeneratedIndexTestMeterV18(&ledger);
            let indexed = (|| {
                let mut rows = generated_rows_v18::<u64>(&reader, 2)?;
                assert_eq!(rows.capacity(), 2, "the exact Vec payload premise is independently checked");
                generated_push_v18(&mut rows, 2)?;
                generated_push_v18(&mut rows, 1)?;
                assert!(generated_push_v18(&mut rows, 3).is_err(), "an unreserved append cannot grow capacity");
                generated_sort_v18(&mut rows, |row| [*row as usize], &reader)?;
                let range = generated_range_v18(&rows, |row| [*row as usize], [1], &reader)?;
                assert_eq!(rows, [1, 2]);
                assert_eq!(range, 0..1);
                assert_eq!(ledger.budget.borrow().work(), before_work + INDEX_WORK);
                assert_eq!(ledger.budget.borrow().storage(), before_storage + payload,
                    "result credit remains live until the owning rows drop");
                drop(rows);
                Ok::<_, ProductionMirPlironTranslationErrorV1>(())
            })();
            let error = ledger.failure.get();
            if short_work || short_storage {
                assert!(matches!(indexed, Err(ProductionMirPlironTranslationErrorV1::ResourceLimit)));
                let first = error.expect("the original ledger records the exact failed resource operation");
                assert_eq!(matches!(first, ArgumentResourceV1::Work(_)), short_work);
                let unchanged_work = ledger.budget.borrow().work();
                let unchanged_storage = ledger.budget.borrow().storage();
                assert!(reader.charge(0).is_err());
                assert!(reader.reserve(0).is_err());
                assert_eq!(ledger.failure.get(), Some(first));
                assert_eq!(ledger.budget.borrow().work(), unchanged_work);
                assert_eq!(ledger.budget.borrow().storage(), unchanged_storage);
                denial.set(Some(first));
            } else {
                indexed.unwrap();
                assert_eq!(error, None);
            }
            completed.set(true);
            drop(reader);
            drop(ledger);
            // All index rows are gone. Only this known source-scope attempt can
            // settle their credits; helpers never refund an inner failed scope.
            budget.release_storage(budget.storage().checked_sub(floor).unwrap())?;
            match error { Some(error) => Err(error.into()), None => Ok(()) }
        });
        assert!(completed.get(), "the original prepared source callback must execute");
        match denial.get() {
            Some(expected) => assert!(matches!(result, Err(ProductionSourceOwnedViewErrorV18::Resource(actual)) if actual == expected)),
            None => result.unwrap(),
        }
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn actual_generated_sequence_reader_preserves_instance_coordinates_and_nonoperation_gaps() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = prepared_source_fixture(ModuleFixture::Mixed, false, &mut budget);
    let checked = std::cell::Cell::new((0usize, 0usize));
    prepared.with_source_consumer_v18(&mut budget, |source, budget| -> Result<(), SourceConsumerTestErrorV18> {
        source.with_analysis_v18(budget, |scope| scope.with_inventory_v1(|inventory, budget| {
            source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                let mut actual = 0usize;
                let mut gaps = 0usize;
                let roots = source.root_count(budget)?;
                for root in 0..roots {
                    let (_, physical) = source.root(root, budget)?;
                    let function = inventory.functions()[physical].coordinate;
                    value_origin_v1::with_whole_value_origins_v18(relation, function, budget, |origins, budget| {
                let floor = budget.storage();
                let header = std::mem::size_of::<CorrelationLedgerV18<'_, '_, '_>>()
                    + std::mem::size_of::<SourceGeneratedRecipeReaderV18<'_, '_, '_, '_, '_>>()
                    + std::mem::size_of::<InventoryCorrelationV18<'_, '_, '_, '_, '_, '_>>()
                    + std::mem::size_of::<NeutralRecipeOperationsV18<'_>>();
                budget.reserve_storage(header)?;
                let ledger = CorrelationLedgerV18::new(budget, source.cleanup);
                let graph = InventoryCorrelationV18 { origins: Some(origins), inventory, function, ledger: &ledger,
                    inline_scalar: &Gfx942InlineScalarCorrespondenceV30::empty(), scalar_source: None };
                    let instances = ledger.with_budget(|budget| Ok(source.instance_count(root, budget)))??;
                    for instance in 0..instances {
                        let reader = SourceGeneratedRecipeReaderV18 { relation, root, instance, origins, ledger: &ledger };
                        let sidecar = ledger.with_budget(|budget| Ok(source.sidecar(root, instance, budget)))??;
                        for site in &sidecar.terminator_operation_spans {
                            let sequence = reader.operations(site.semantic_block.index()).unwrap();
                            let rows = ledger.with_budget(|budget| Ok(relation.source_operation_rows(
                                root, instance, site.semantic_block, None, budget,
                            )))??;
                            assert_eq!(sequence.len(), rows.len());
                            assert_eq!(reader.producer_contains(site.semantic_block.index(),
                                FunctionOperationLocation::new(BlockId(u32::MAX), usize::MAX)), Some(false));
                            for (index, row) in rows.iter().enumerate() {
                                let expected = ledger.with_budget(|budget| Ok(relation.mapped_source_operation(row.location, budget)))??;
                                match expected {
                                    ProductionSourceOperationV18::Operation(coordinate) => {
                                        let (operation, location) = sequence.get(index).unwrap();
                                        let function = &inventory.functions()[coordinate.block.function.0 as usize];
                                        let block = &inventory.blocks()[function.blocks.start + coordinate.block.block as usize];
                                        assert!(std::ptr::eq(operation, &block.block.operations[coordinate.operation as usize]));
                                        assert_eq!(location, FunctionOperationLocation::new(block.block.id, coordinate.operation as usize));
                                        assert_eq!(reader.producer_contains(site.semantic_block.index(), location), Some(true));
                                        for result in &operation.results {
                                            assert_eq!(reader.operation_origin(function.function.body.as_ref().unwrap(), &graph, result.id),
                                                Some(result.id), "an operation result keeps its exact paid origin");
                                        }
                                        actual += 1;
                                    }
                                    ProductionSourceOperationV18::Gap { .. }
                                    | ProductionSourceOperationV18::RemovedCall
                                    | ProductionSourceOperationV18::NoOperations => {
                                        assert!(sequence.get(index).is_none(), "nonoperations cannot become recipe instructions");
                                        gaps += 1;
                                    }
                                }
                            }
                        }
                    }
                assert_eq!(ledger.failure.get(), None);
                assert!(!ledger.inconsistent_inventory.get());
                drop(graph);
                drop(ledger);
                budget.release_storage(header)?;
                assert_eq!(budget.storage(), floor);
                Ok::<_, SourceConsumerTestErrorV18>(())
                    })?;
                }
                checked.set((actual, gaps));
                Ok::<_, SourceConsumerTestErrorV18>(())
            })
        }))
    }).unwrap();
    assert!(checked.get().0 > 0, "actual emitted source operations were checked");
    assert!(checked.get().1 > 0, "actual removed calls or zero-op spans were checked");
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

fn scalar_payload_owner_v18() -> ProductionSemanticSsaOwnerV1 {
    let base = entrance_control_owner(false);
    let semantic = base.source_semantic();
    let mut functions = semantic.functions().to_vec();
    let helper = &functions[2];
    let mut locals = helper.locals().to_vec();
    locals.push(local(215, U32, SemanticLocalRoleV1::Temporary));
    let store = |value| SemanticStatementV1::new(SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
            place(2, U32), value, SemanticVolatilityV1::NonVolatile, None,
        )));
    let load = || assign(place(1, U32), SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
        place(2, U32), SemanticVolatilityV1::NonVolatile, None,
    )));
    functions[2] = function(210, helper.role(), helper.abi().clone(), locals,
        vec![block(214, vec![assign(place(1, U32), SemanticRvalueKindV1::Use(literal(13))),
            store(SemanticOperandV1::Copy(place(1, U32))), load(),
            store(literal(37)), load()], SemanticTerminatorKindV1::Return)]);
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(), semantic.types().to_vec(), vec![], vec![], vec![],
        functions, semantic.callables().to_vec(), semantic.roots().to_vec(),
    ).unwrap().admit_exact_v29(SemanticMirLimitsV1::default()).unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default()).unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    ).unwrap()
}

fn active_gap_owner_v18() -> ProductionSemanticSsaOwnerV1 {
    active_source_owner_v18(false)
}

fn active_shared_target_owner_v18() -> ProductionSemanticSsaOwnerV1 {
    active_source_owner_v18(true)
}

fn active_source_owner_v18(shared_target: bool) -> ProductionSemanticSsaOwnerV1 {
    use fe2o3_mir_model::semantic_mir_v1::{SemanticSwitchTargetV1, SemanticSwitchTargetsV1};
    let base = scalar_payload_owner_v18();
    let semantic = base.source_semantic();
    let mut functions = semantic.functions().to_vec();
    let root = &functions[0];
    let edge = |role, block| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(block));
    let entry = if shared_target {
        SemanticTerminatorKindV1::SwitchInt {
            discriminant: SemanticOperandV1::Copy(place(1, U32)),
            targets: SemanticSwitchTargetsV1::new(
                vec![SemanticSwitchTargetV1::new(0, edge(SemanticEdgeRoleV1::SwitchValue, 2))],
                edge(SemanticEdgeRoleV1::SwitchOtherwise, 4),
            ).unwrap(),
        }
    } else {
        SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 2))
    };
    let call = |tag, target| block(tag, vec![], SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(2),
            vec![SemanticOperandV1::Copy(place(1, U32))],
            Some(SemanticCallDestinationV1::new(place(0, UNIT), edge(SemanticEdgeRoleV1::CallReturn, target))),
            SemanticUnwindActionV1::Unreachable,
        ).unwrap(),
    ));
    functions[0] = function(60, root.role(), root.abi().clone(), root.locals().to_vec(), vec![
        block(221, vec![], entry), call(222, 4), call(223, 1), call(224, 5), call(225, 3),
        block(226, vec![], SemanticTerminatorKindV1::Return),
    ]).with_kernel_entry(root.kernel_entry().unwrap().clone());
    // All four calls are SSA-reachable along 2 -> 1 -> 4 -> 3. The
    // helper's absent normal return, not missing syntax, deactivates their
    // return continuations. The second switch arm independently reaches 4.
    let helper = &functions[2];
    functions[2] = function(210, helper.role(), helper.abi().clone(), helper.locals().to_vec(), vec![
        block(214, helper.blocks()[0].statements().to_vec(),
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1))),
        block(227, vec![], SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1))),
    ]);
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(), semantic.types().to_vec(), vec![], vec![], vec![],
        functions, semantic.callables().to_vec(), semantic.roots().to_vec(),
    ).unwrap().admit_exact_v29(SemanticMirLimitsV1::default()).unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default()).unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    ).unwrap()
}

#[test]
fn actual_active_source_instances_keep_dense_ids_and_compact_payloads_through_correspondence() {
    for shared in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let factory = if shared { active_shared_target_owner_v18 } else { active_gap_owner_v18 };
        let prepared = scalar_payload_prepared_from_v18(factory, &mut budget);
        let checked = std::cell::Cell::new((0usize, 0usize));
        prepared.with_source_consumer_v18(&mut budget, |source, budget| -> Result<(), SourceConsumerTestErrorV18> {
            assert_eq!(source.instance_count(0, budget)?, 5);
            let expected = if shared { [Some(0), Some(1), None, Some(2), None] }
                else { [Some(0), Some(1), None, None, None] };
            let root = source.root_row(0)?;
            for (original, block) in [(1usize, 2u32), (2, 1), (3, 4), (4, 3)] {
                let call = root.coordinates.sources.rows[original].incoming.unwrap();
                assert_eq!(call.caller.index(), 0);
                assert_eq!(call.block, SemanticBlockIdV1::from_index(block));
                assert_eq!(root.coordinates.sources.rows[original].function, SemanticFunctionIdV1::from_index(2));
            }
            assert_eq!(root.active_instances.rows, expected);
            let locator = root.active_instances.rows.as_ptr();
            let canonical = source.canonical(budget)?.canonical_bytes().as_ptr();
            assert_eq!(root.sidecars.rows.len(), if shared { 3 } else { 2 });
            for (original, ordinal) in expected.into_iter().enumerate() {
                assert_eq!(source.active_ordinal(0, original, budget)?, ordinal);
                assert_eq!(source.instance_active(0, original, budget)?, ordinal.is_some());
                if let Some(ordinal) = ordinal {
                    assert_eq!(root.sidecars.rows[ordinal].source_call_instance.unwrap().index(), original);
                } else {
                    assert_eq!(source.memory_anchor_count(0, original, budget)?, 0);
                    assert!(source.invocation_entry(0, original, budget)?.is_none());
                }
            }
            source.with_analysis_v18(budget, |scope| scope.with_inventory_v1(|inventory, budget| {
                source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                    let mut later_memory = 0;
                    for row in relation.attachments {
                        if matches!(row.key.family,
                            TileAttachmentFamilyV29::RawSidecar
                            | TileAttachmentFamilyV29::SourceSlot
                            | TileAttachmentFamilyV29::MemoryAnchor
                            | TileAttachmentFamilyV29::PrivateArray
                            | TileAttachmentFamilyV29::Lifecycle
                            | TileAttachmentFamilyV29::Assertion)
                        {
                            assert!(expected[row.key.instance].is_some(),
                                "attachment keys retain original active instance IDs");
                            if row.key.instance == 3
                                && row.key.family == TileAttachmentFamilyV29::MemoryAnchor
                            { later_memory += 1; }
                        }
                    }
                    assert_eq!(later_memory > 0, shared,
                        "the live instance after an inactive gap keeps its own memory attachments");
                    for (original, block) in [(1usize, 2u32), (2, 1), (3, 4), (4, 3)] {
                        assert_eq!(relation.defined_call_instance(0, 0, SemanticBlockIdV1::from_index(block), budget)?, original);
                    }
                    for original in [2, 4] {
                        assert!(relation.source_block_entry(0, original, SemanticBlockIdV1::from_index(0), budget)?.is_none());
                    }
                    if !shared {
                        assert_eq!(relation.unique_source_instance(0, SemanticFunctionIdV1::from_index(2), budget)?, Some(1));
                    }
                    let physical = source.root(0, budget)?.1;
                    let recipe = scalar_leaf_collision_recipe_v18(inventory.functions()[physical].function);
                    relation.with_scalar_leaves_v18(0, &recipe, budget, |leaves, budget| {
                        leaves.visit_store_inputs(budget, |request, budget| -> Result<(), SourceConsumerTestErrorV18> {
                            assert_eq!(request.scalar(budget)?, ProductionSemanticScalarTypeV2::Integer { signed: false, bits: 32 });
                            assert!(request.instance == 1 || shared && request.instance == 3);
                            let (first, second) = checked.get();
                            checked.set(if request.instance == 1 { (first + 1, second) } else { (first, second + 1) });
                            Ok(())
                        }).map(|_| ())
                    })
                })
            }))?;
            assert_eq!(root.active_instances.rows.as_ptr(), locator, "all consumers borrow the one retained locator");
            assert_eq!(source.canonical(budget)?.canonical_bytes().as_ptr(), canonical);
            Ok(())
        }).unwrap();
        assert!(checked.get().0 > 0, "actual original instance 1 Stores reached the scalar consumer");
        assert_eq!(checked.get().1 > 0, shared, "shared target retains the distinct original instance 3");
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn actual_active_source_memory_rejects_compact_ordinal_attachment_substitution() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_from_v18(active_shared_target_owner_v18, &mut budget);
    let checked = std::cell::Cell::new(false);
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| -> Result<(), SourceConsumerTestErrorV18> {
        source.with_analysis_v18(budget, |scope| scope.with_inventory_v1(|inventory, budget| {
            source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                assert_eq!(source.active_ordinal(0, 3, budget)?, Some(2));
                assert!(!source.instance_active(0, 2, budget)?);
                assert_eq!(source.instance(0, 2, budget)?.0, source.instance(0, 3, budget)?.0);
                let original = relation.attachments.iter().find(|row| {
                    row.key.root == 0 && row.key.instance == 3
                        && row.key.family == TileAttachmentFamilyV29::MemoryAnchor
                        && row.key.field == TileAttachmentFieldV29::MemoryStoreUse
                        && matches!(row.location, TileAttachmentLocationV29::Use(_))
                }).expect("the later active instance has an actual Store");
                let TileAttachmentLocationV29::Use(
                    fe2o3_kernel_ir::CanonicalKirUseCoordinateV1::OperationOperand { operation, .. }
                ) = original.location else { panic!("Store RHS is an operation operand"); };
                let actual = &inventory.functions()[operation.block.function.0 as usize].function
                    .body.as_ref().unwrap().blocks[operation.block.block as usize]
                    .operations[operation.operation as usize];
                let pointer = match actual.kind {
                    OperationKind::Store { pointer, .. } | OperationKind::GuardedStore { pointer, .. } => pointer,
                    _ => panic!("original Store attachment must name an actual Store"),
                };
                assert_eq!(relation.retained_memory_access(0, operation, pointer, budget)?.unwrap().instance, 3);
                let floor = budget.storage();
                budget.reserve_storage(size_of::<Vec<SourceAttachmentV18>>()
                    + size_of::<ProductionSourceCorrespondenceV18<'_>>())?;
                let mut rows = source_attachments_v18(source, inventory, budget)?;
                let scratch = budget.storage().checked_sub(floor).unwrap();
                let mut changed = 0;
                for row in &mut rows {
                    if row.key.family == TileAttachmentFamilyV29::MemoryAnchor && row.key.instance == 3 {
                        row.key.instance = 2;
                        changed += 1;
                    }
                }
                assert!(changed > 0);
                assert!(rows.windows(2).all(|pair|
                    source_attachment_key_v18(pair[0].key) < source_attachment_key_v18(pair[1].key)));
                let tampered = ProductionSourceCorrespondenceV18 {
                    source, inventory, attachments: &rows,
                    slot: std::ptr::from_ref(budget) as usize,
                    ledger: budget.work_ledger_identity_v1(), floor: budget.storage(),
                };
                let error = match tampered.retained_memory_access(0, operation, pointer, budget) {
                    Err(error) => error,
                    Ok(_) => panic!("compact ordinal cannot authenticate another original instance"),
                };
                assert!(matches!(error, ProductionSourceOwnedViewErrorV18::Binding(_)));
                checked.set(true);
                drop(tampered);
                drop(rows);
                budget.release_storage(scratch)?;
                assert_eq!(budget.storage(), floor);
                Err::<(), _>(SourceConsumerTestErrorV18::Source(error))
            })
        }))
    });
    assert!(checked.get(), "genuine source lookup succeeds before metadata substitution");
    assert!(matches!(result, Err(SourceConsumerTestErrorV18::Source(
        ProductionSourceOwnedViewErrorV18::Binding(_)))));
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn actual_active_source_queries_retain_ambiguity_and_out_of_roster_errors() {
    for ambiguous in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_from_v18(active_shared_target_owner_v18, &mut budget);
        let checked = std::cell::Cell::new(false);
        let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| -> Result<(), SourceConsumerTestErrorV18> {
            if ambiguous {
                source.with_analysis_v18(budget, |scope| scope.with_inventory_v1(|inventory, budget| {
                    source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| -> Result<(), SourceConsumerTestErrorV18> {
                        let error = relation.unique_source_instance(0, SemanticFunctionIdV1::from_index(2), budget).unwrap_err();
                        assert!(matches!(error, ProductionSourceOwnedViewErrorV18::Binding("source function has multiple call instances")));
                        let before = budget.work();
                        assert!(matches!(source.instance_active(0, 2, budget), Err(ProductionSourceOwnedViewErrorV18::Binding("source function has multiple call instances"))));
                        assert_eq!(budget.work(), before);
                        checked.set(true);
                        Ok(())
                    })
                }))?;
            } else {
                assert!(matches!(source.instance_active(0, 5, budget), Err(ProductionSourceOwnedViewErrorV18::Binding(_))));
                let before = budget.work();
                assert!(matches!(source.instance_active(0, 2, budget), Err(ProductionSourceOwnedViewErrorV18::Binding(_))));
                assert_eq!(budget.work(), before);
                checked.set(true);
            }
            Ok(())
        });
        assert!(checked.get(), "actual query must execute before the source boundary retains its error");
        assert!(matches!(result, Err(SourceConsumerTestErrorV18::Source(ProductionSourceOwnedViewErrorV18::Binding(_)))));
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn actual_active_instance_index_has_independent_exact_work_storage_and_lookup_limits() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_from_v18(active_gap_owner_v18, &mut budget);
    let checked = std::cell::Cell::new(false);
    prepared.with_source_consumer_v18(&mut budget, |source, budget| -> SourceOwnedResultV18<()> {
        let root = source.root_row(0)?;
        let result = production_call_instances_v1::with_production_call_instances_v1(
            &source.owner.inner.source.owner, root.coordinates.root, budget, |instances, _| {
                let n = instances.instances().len();
                let s = root.sidecars.rows.len();
                assert_eq!((n, s), (5, 2));
                let headers = size_of::<PendingActiveInstanceIndexV1>()
                    + size_of::<Result<PendingActiveInstanceIndexV1, ProductionSemanticKirErrorV1>>();
                let required_work = 2 * (n + s + 2) + 3;
                let required_storage = headers + n * size_of::<Option<usize>>();
                for (work_limit, storage_limit, pass) in [
                    (required_work, MODULE_FLOOR + required_storage, true),
                    (required_work - 1, MODULE_FLOOR + required_storage, false),
                    (required_work, MODULE_FLOOR + required_storage - 1, false),
                ] {
                    let mut probe_work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
                    let mut probe = ArgumentBudgetV1::new(&mut probe_work, storage_limit);
                    probe.reserve_storage(MODULE_FLOOR).unwrap();
                    let result = pending_active_instance_index_v1(instances, &root.sidecars.rows, &mut probe);
                    assert_eq!(result.is_ok(), pass);
                    if let Ok(index) = &result {
                        assert_eq!(index.rows.capacity(), n, "independent exact allocation premise");
                        assert_eq!(index.storage, required_storage);
                        assert_eq!(probe.work(), required_work);
                        assert_eq!(probe.storage(), MODULE_FLOOR + required_storage);
                    } else if work_limit < required_work {
                        assert!(matches!(&result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(_)))));
                    } else {
                        assert!(matches!(&result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Storage(_)))));
                    }
                    drop(result);
                    probe.release_storage(probe.storage() - MODULE_FLOOR).unwrap();
                    assert_eq!(probe.storage(), MODULE_FLOOR);
                }
                for limit in [7, 8] {
                    let mut probe_work = CanonicalKernelIrWorkBudgetV1::new(limit);
                    let mut probe = ArgumentBudgetV1::new(&mut probe_work, 0);
                    let result = root.active_instances.sidecar_ordinal(1, n, &root.sidecars.rows, &mut probe);
                    if limit == 8 { assert_eq!(result.unwrap(), Some(1)); }
                    else { assert!(matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(_))))); }
                    assert_eq!(probe.storage(), 0, "a retained locator lookup allocates no storage");
                }
                checked.set(true);
                Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
            },
        );
        result.unwrap();
        Ok(())
    }).unwrap();
    assert!(checked.get(), "resource probes must use actual original source instances and sidecars");
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

fn active_owned_module_v18(budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedScopedModuleV29 {
    let owner = active_gap_owner_v18();
    let (input, launch) = with_module_fixture_view(&owner, ModuleFixture::Ordinary, budget,
        |source, budget| OwnedExecutionInputV29::capture(source, budget)).unwrap();
    let mut donor = Some(ScopedSourceInputsV29 { owner, launch, input: input.unwrap() });
    let result = SourceOwnedScopedModuleV29::try_new(
        &mut donor, ProductionSemanticKirLimitsV1::default(), budget,
    ).unwrap();
    assert!(donor.is_none());
    result
}

#[test]
fn actual_active_instance_index_replay_rejects_omission_duplicate_order_and_wrong_original_id() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let mut owner = active_owned_module_v18(&mut budget);
    let retained = owner.retained_storage;
    let floor = budget.storage();
    assert_eq!(floor, MODULE_FLOOR + retained);
    let root = &owner.pending.roots[0];
    let index_storage = size_of::<PendingActiveInstanceIndexV1>()
        + size_of::<Result<PendingActiveInstanceIndexV1, ProductionSemanticKirErrorV1>>()
        + root.active_instances.rows.capacity() * size_of::<Option<usize>>();
    assert_eq!(root.active_instances.storage, index_storage);
    assert!(root.inherited_assembly_storage >= index_storage, "the moved index remains paid with its owning root");
    let locator = root.active_instances.rows.as_ptr();
    let graph = owner.pending.graph.canonical_bytes().as_ptr();
    owner.replay(&mut budget).unwrap();
    assert_eq!(budget.storage(), floor, "fresh replay scratch cannot release the original retained index");
    for fault in 0..6 {
        let root = &mut owner.pending.roots[0];
        let old = root.active_instances.rows[1];
        let old_id = root.sidecars.rows[1].source_call_instance;
        let mut last = None;
        match fault {
            0 => root.active_instances.rows[2] = Some(1),
            1 => root.active_instances.rows[1] = None,
            2 => root.active_instances.rows[1] = Some(0),
            3 => root.sidecars.rows.swap(0, 1),
            4 => root.sidecars.rows[1].source_call_instance = root.sidecars.rows[0].source_call_instance,
            5 => last = root.active_instances.rows.pop(),
            _ => unreachable!(),
        }
        assert!(matches!(owner.replay(&mut budget), Err(ScopedModuleErrorV29::Source(ProductionSemanticKirErrorV1::Unsupported { .. }))));
        assert_eq!(budget.storage(), floor, "failed fresh replay preserves the live original owner reservation");
        let root = &mut owner.pending.roots[0];
        match fault {
            0 => root.active_instances.rows[2] = None,
            1 | 2 => root.active_instances.rows[1] = old,
            3 => root.sidecars.rows.swap(0, 1),
            4 => root.sidecars.rows[1].source_call_instance = old_id,
            5 => root.active_instances.rows.push(last.unwrap()),
            _ => unreachable!(),
        }
        owner.replay(&mut budget).unwrap();
        assert_eq!(budget.storage(), floor);
        assert_eq!(owner.pending.roots[0].active_instances.rows.as_ptr(), locator);
        assert_eq!(owner.pending.graph.canonical_bytes().as_ptr(), graph);
    }
    drop(owner);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn actual_active_instance_source_plan_check_rejects_coherent_inactive_and_omitted_rows() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let mut owner = active_owned_module_v18(&mut budget);
    let retained = owner.retained_storage;
    let floor = budget.storage();
    let root = &mut owner.pending.roots[0];
    production_call_instances_v1::with_production_call_instances_v1(
        &owner.source.owner, root.coordinates.root, &mut budget, |instances, budget| {
            root.active_instances.check_source_plan(instances, &root.sidecars.rows, budget).unwrap();
            let id = root.sidecars.rows[1].source_call_instance;
            root.sidecars.rows[1].source_call_instance = instances.id_at(2);
            root.active_instances.rows[2] = Some(1);
            root.active_instances.rows[1] = None;
            assert!(root.active_instances.check_source_plan(instances, &root.sidecars.rows, budget).is_err(),
                "coherent same-function substitution cannot activate an original inactive call");
            root.sidecars.rows[1].source_call_instance = id;
            root.active_instances.rows[2] = None;
            root.active_instances.rows[1] = Some(1);
            let omitted = root.sidecars.rows.pop().unwrap();
            assert!(root.active_instances.check_source_plan(instances, &root.sidecars.rows, budget).is_err(),
                "a populated original bit cannot replace its missing actual sidecar");
            root.sidecars.rows.push(omitted);
            root.active_instances.check_source_plan(instances, &root.sidecars.rows, budget).unwrap();
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
        },
    ).unwrap();
    assert_eq!(budget.storage(), floor);
    owner.replay(&mut budget).unwrap();
    drop(owner);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn actual_active_instance_queries_do_not_restore_lost_or_foreign_custody() {
    for foreign in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut other = ArgumentBudgetV1::new(&mut foreign_work, MODULE_LIMIT);
        other.reserve_storage(73).unwrap();
        let prepared = scalar_payload_prepared_from_v18(active_gap_owner_v18, &mut budget);
        let observed = std::cell::Cell::new(None);
        let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| -> SourceOwnedResultV18<()> {
            assert!(source.instance_active(0, 1, budget)?);
            let incoming = budget.storage();
            let spent = budget.work();
            if foreign {
                assert!(matches!(source.instance_active(0, 1, &mut other), Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting))));
                assert_eq!((other.work(), other.storage()), (0, 73));
            } else {
                budget.release_storage(1)?;
                assert!(matches!(source.instance_active(0, 1, budget), Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting))));
                budget.reserve_storage(1)?;
            }
            assert_eq!(budget.work(), spent, "custody refusal precedes locator work");
            assert!(matches!(source.instance_active(0, 1, budget), Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting))));
            assert_eq!(budget.work(), spent, "ignoring the denial cannot revive this view");
            observed.set(Some(incoming));
            Ok(())
        });
        assert!(matches!(result, Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting))));
        assert_eq!(budget.storage(), observed.get().expect("real source callback reached the retained index"));
        assert!(budget.storage() > MODULE_FLOOR, "the containing source attempt must preserve denied refunds");
        assert_eq!((other.work(), other.storage()), (0, 73));
    }
}

#[test]
fn actual_active_instance_owner_drops_before_refund_on_raw_consumer_unwind() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_from_v18(active_shared_target_owner_v18, &mut budget);
    let payload = Box::new(0x1973_u64);
    let address = std::ptr::from_ref(payload.as_ref());
    let observed = std::cell::Cell::new(false);
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        prepared.with_source_consumer_v18(&mut budget, |source, budget| -> SourceOwnedResultV18<()> {
            assert!(source.instance_active(0, 1, budget)?);
            assert!(source.instance_active(0, 3, budget)?);
            assert_eq!(source.root_row(0)?.active_instances.rows.len(), 5);
            observed.set(true);
            std::panic::panic_any(payload)
        })
    })).unwrap_err();
    assert!(observed.get(), "the original shared-target source reached the owning callback");
    let payload = caught.downcast::<Box<u64>>().expect("outer panic payload stays exact");
    assert_eq!(std::ptr::from_ref(payload.as_ref().as_ref()), address);
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

fn scalar_payload_prepared_v18(budget: &mut ArgumentBudgetV1<'_>) -> ProductionPreparedSourceV18 {
    scalar_payload_prepared_from_v18(scalar_payload_owner_v18, budget)
}

fn scalar_payload_prepared_from_v18(
    factory: fn() -> ProductionSemanticSsaOwnerV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> ProductionPreparedSourceV18 {
    let projection = factory();
    let owner = factory();
    let (_, launch) = with_module_fixture_view(&owner, ModuleFixture::Ordinary, budget, |_, _| ()).unwrap();
    with_module_fixture_view(&projection, ModuleFixture::Ordinary, budget, |source, budget| {
        ProductionPendingScopedSourceOwnerV29::prepare_source_with_budget_v18(
            owner, launch, source.input, ProductionSemanticKirLimitsV1::default(), budget,
        ).unwrap()
    }).unwrap().0
}

fn scalar_read_store_owner_v18() -> ProductionSemanticSsaOwnerV1 {
    let base = scalar_payload_owner_v18();
    let semantic = base.source_semantic();
    let mut functions = semantic.functions().to_vec();
    let helper = &functions[2];
    let mut statements = helper.blocks()[0].statements().to_vec();
    statements.insert(3, SemanticStatementV1::new(SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
            place(2, U32), SemanticOperandV1::Copy(place(2, U32)),
            SemanticVolatilityV1::NonVolatile, None,
        ))));
    functions[2] = function(210, helper.role(), helper.abi().clone(), helper.locals().to_vec(),
        vec![block(214, statements, SemanticTerminatorKindV1::Return)]);
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(), semantic.types().to_vec(), vec![], vec![], vec![],
        functions, semantic.callables().to_vec(), semantic.roots().to_vec(),
    ).unwrap().admit_exact_v29(SemanticMirLimitsV1::default()).unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default()).unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    ).unwrap()
}

fn promoted_reassignment_owner_v18() -> ProductionSemanticSsaOwnerV1 {
    let base = scalar_payload_owner_v18();
    let semantic = base.source_semantic();
    let mut functions = semantic.functions().to_vec();
    let helper = &functions[2];
    let mut statements = helper.blocks()[0].statements().to_vec();
    statements.insert(1, assign(place(1, U32), SemanticRvalueKindV1::Use(literal(29))));
    functions[2] = function(210, helper.role(), helper.abi().clone(), helper.locals().to_vec(),
        vec![block(214, statements, SemanticTerminatorKindV1::Return)]);
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(), semantic.types().to_vec(), vec![], vec![], vec![],
        functions, semantic.callables().to_vec(), semantic.roots().to_vec(),
    ).unwrap().admit_exact_v29(SemanticMirLimitsV1::default()).unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default()).unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    ).unwrap()
}

fn repeated_inline_owner_v18() -> ProductionSemanticSsaOwnerV1 {
    use fe2o3_mir_model::semantic_mir_v1::*;
    let base = entrance_control_owner(false);
    let semantic = base.source_semantic();
    let mut functions = semantic.functions().to_vec();
    let mut callables = semantic.callables().to_vec();
    let helper = &functions[2];
    let abi_value = || SemanticAbiValueV1::new(U32, SemanticAbiPassModeV1::Direct(
        SemanticAbiValueAttributesV1::new(
            SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
            SemanticAbiExtensionV1::None, 0, None,
        ).unwrap(),
    ));
    let source = SemanticSourceProvenanceV1::unavailable();
    let id = SemanticCallableIdV1::from_index(callables.len() as u32);
    callables.push(SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([181; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([182; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([183; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([184; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([185; 32]), source,
            SemanticFunctionAbiV1::new(
                SemanticAbiIdentityV1::from_sha256([186; 32]),
                SemanticLayoutIdentityV1::from_sha256([187; 32]),
                SemanticCanonAbiV1::Rust, false, false, vec![abi_value(), abi_value()], abi_value(),
            ).unwrap(),
        ),
        operation: SemanticCompilerIntrinsicOperationV1::Gfx942InlineU32(
            SemanticGfx942InlineU32V30::new(SemanticGfx942InlineInstructionV30::VAddU32,
                SemanticGfx942InlineU32V30::NOMEM_OPTION_BITS).unwrap(),
        ),
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([188; 32]),
    });
    let mut locals = helper.locals().to_vec();
    locals.push(local(216, U32, SemanticLocalRoleV1::Temporary));
    let call = SemanticDirectCallV1::new_callable(id, vec![literal(13), literal(29)],
        Some(SemanticCallDestinationV1::new(place(2, U32), SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::CallReturn, SemanticBlockIdV1::from_index(1),
        ))), SemanticUnwindActionV1::Unreachable,
    ).unwrap().with_inline_assembly_source_v30(SemanticInlineAssemblySourceV30::new(
        [189; 32], helper.identity(), [190; 32], [191; 32],
    ).unwrap());
    functions[2] = function(210, helper.role(), helper.abi().clone(), locals, vec![
        block(214, vec![], SemanticTerminatorKindV1::Call(call)),
        block(217, vec![], SemanticTerminatorKindV1::Return),
    ]);
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        semantic.types().to_vec(), vec![], vec![], vec![], functions, callables, semantic.roots().to_vec(),
    ).unwrap().admit_current_production(SemanticMirLimitsV1::default()).unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default()).unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    ).unwrap()
}

#[test]
fn actual_source_inline_reader_preserves_repeated_instances_without_a_second_type_index() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_from_v18(repeated_inline_owner_v18, &mut budget);
    let checked = std::cell::Cell::new(0usize);
    prepared.with_source_consumer_v18(&mut budget, |source, budget| -> Result<(), SourceConsumerTestErrorV18> {
        source.with_analysis_v18(budget, |scope| scope.with_inventory_v1(|inventory, budget| {
            source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                for root in 0..source.root_count(budget)? {
                    let cache = Gfx942InlineScalarCorrespondenceV30::build_source_v18(relation, root, budget)?;
                    let function = &inventory.functions()[source.root(root, budget)?.1];
                    budget.reserve_storage(std::mem::size_of::<CorrelationLedgerV18<'_, '_, '_>>()
                        + std::mem::size_of::<SourceCorrelationChargeV18<'_, '_, '_, '_>>())?;
                    let ledger = CorrelationLedgerV18::new(budget, source.cleanup);
                    let mut charge = SourceCorrelationChargeV18 {
                        ledger: &ledger, finite: UnsupportedIndexCorrelationBudgetV1 { remaining: 4096 },
                        finite_denied: false,
                    };
                    for row in &inventory.operations()[function.operations.clone()] {
                        if !matches!(row.operation.kind, OperationKind::InlineAssembly(_)) { continue; }
                        let result = row.operation.results[0].id;
                        let actual = cache.normalize(row.operation, result, &mut charge, |value, _| {
                            ledger.inventory(|budget| {
                                inventory.definition_for_value(function.coordinate, value, budget)
                            }).flatten().and_then(|definition| {
                                let fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::Result { operation, .. } =
                                    definition.coordinate else { return None; };
                                let operation = &function.function.body.as_ref()?.blocks[operation.block.block as usize]
                                    .operations[operation.operation as usize];
                                let OperationKind::Constant(Constant::U32(bits)) = operation.kind else { return None; };
                                Some(NormalizedScalarExpressionV1::Constant {
                                    scalar: ProductionSemanticScalarTypeV2::Integer { signed: false, bits: 32 },
                                    bits: u64::from(bits),
                                })
                            })
                        }).expect("source-matched actual instruction normalizes");
                        assert!(matches!(actual, NormalizedScalarExpressionV1::Binary {
                            operation: ProductionSemanticBinaryOpV2::Add,
                            overflow: ProductionOverflowContractV2::Wrapping, ..
                        }));
                        checked.set(checked.get() + 1);
                    }
                    assert!(ledger.failure.get().is_none());
                    assert!(!ledger.inconsistent_inventory.get());
                    drop(charge);
                    drop(ledger);
                    drop(cache);
                }
                Ok::<_, SourceConsumerTestErrorV18>(())
            })
        }))
    }).unwrap();
    assert_eq!(checked.get(), 2, "both original repeated helper calls remain separately authenticated");
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn actual_source_inline_reader_rejects_missing_and_duplicate_instance_outputs() {
    for duplicate in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_from_v18(repeated_inline_owner_v18, &mut budget);
        let checked = std::cell::Cell::new(false);
        let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| -> Result<(), SourceConsumerTestErrorV18> {
            source.with_analysis_v18(budget, |scope| scope.with_inventory_v1(|inventory, budget| {
                source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                    let floor = budget.storage();
                    let original = Gfx942InlineScalarCorrespondenceV30::build_source_v18(relation, 0, budget)?;
                    drop(original);
                    budget.release_storage(budget.storage().checked_sub(floor).unwrap())?;
                    budget.reserve_storage(std::mem::size_of::<Vec<SourceAttachmentV18>>()
                        + std::mem::size_of::<ProductionSourceCorrespondenceV18<'_>>())?;
                    let mut rows = source_attachments_v18(source, inventory, budget)?;
                    let function = &inventory.functions()[source.root(0, budget)?.1];
                    let mut first = None;
                    let mut second = None;
                    for (index, row) in rows.iter().enumerate() {
                        if row.key.root != 0 || row.key.family != TileAttachmentFamilyV29::InstanceSpans
                            || row.key.field != TileAttachmentFieldV29::Span { continue; }
                        let ProductionSourceOperationV18::Operation(coordinate) = relation.mapped_source_operation(row.location, budget)?
                            else { continue; };
                        let actual = &function.function.body.as_ref().unwrap().blocks[coordinate.block.block as usize]
                            .operations[coordinate.operation as usize];
                        if !matches!(actual.kind, OperationKind::InlineAssembly(_)) { continue; }
                        if first.is_none() { first = Some(index); } else { second = Some(index); break; }
                    }
                    let first = first.expect("first actual source helper instruction");
                    let second = second.expect("second actual source helper instruction");
                    assert_ne!(rows[first].key.instance, rows[second].key.instance);
                    assert_ne!(rows[first].location, rows[second].location);
                    rows[second].location = if duplicate { rows[first].location } else { TileAttachmentLocationV29::NoOutput };
                    let tampered = ProductionSourceCorrespondenceV18 {
                        source, inventory, attachments: &rows, slot: std::ptr::from_ref(budget) as usize,
                        ledger: budget.work_ledger_identity_v1(), floor: budget.storage(),
                    };
                    let selected = Gfx942InlineScalarCorrespondenceV30::build_source_v18(&tampered, 0, budget);
                    let error = match selected {
                        Err(error) => error,
                        Ok(value) => { drop(value); panic!("changed source instruction output must reject"); }
                    };
                    assert!(matches!(error, ProductionSourceOwnedViewErrorV18::Binding(_)));
                    checked.set(true);
                    drop(tampered);
                    drop(rows);
                    budget.release_storage(budget.storage().checked_sub(floor).unwrap())?;
                    Err::<(), _>(SourceConsumerTestErrorV18::Source(error))
                })
            }))
        });
        assert!(checked.get(), "the unchanged source cache must succeed before metadata mutation");
        assert!(matches!(result, Err(SourceConsumerTestErrorV18::Source(ProductionSourceOwnedViewErrorV18::Binding(_)))));
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn promoted_store_uses_current_ssa_definition_not_an_earlier_same_typed_assignment() {
    for substitute in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_from_v18(promoted_reassignment_owner_v18, &mut budget);
        let checked = std::cell::Cell::new(0usize);
        let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| -> Result<(), SourceConsumerTestErrorV18> {
            source.with_analysis_v18(budget, |scope| scope.with_inventory_v1(|inventory, budget| {
                source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                    let function = &inventory.functions()[source.root(0, budget)?.1];
                    value_origin_v1::with_whole_value_origins_v18(relation, function.coordinate, budget, |origins, budget| {
                        for row in &inventory.operations()[function.operations.clone()] {
                            let OperationKind::Store { pointer, .. } = row.operation.kind else { continue; };
                            let Some(access) = relation.retained_memory_access(0, row.coordinate, pointer, budget)? else { continue; };
                            let Some(payload) = relation.retained_scalar_payload_v18(0, row.coordinate, &access, budget)? else { continue; };
                            let Some(value) = source_promoted_literal_store_v18(relation, 0, &access, &payload, origins, budget)? else { continue; };
                            assert_eq!(value, Constant::U32(29));
                            checked.set(checked.get() + 1);
                            if substitute {
                                let stale = inventory.operations()[function.operations.clone()].iter()
                                    .find(|row| matches!(row.operation.kind, OperationKind::Constant(Constant::U32(13))))
                                    .expect("original earlier same-typed definition remains in the unoptimized source graph")
                                    .operation.results[0].id;
                                let wrong = SourcePhysicalPayloadV18 {
                                    source: payload.source, value: stale, store_use: payload.store_use,
                                };
                                let result = source_promoted_literal_store_v18(relation, 0, &access, &wrong, origins, budget);
                                assert!(matches!(result, Err(ProductionSourceOwnedViewErrorV18::Binding(
                                    "stored scalar literal differs from its original typed source"
                                ))));
                                return result.map(|_| ());
                            }
                        }
                        Ok(())
                    })?;
                    Ok::<_, SourceConsumerTestErrorV18>(())
                })
            }))
        });
        if substitute {
            assert_eq!(checked.get(), 1, "unchanged source relation must succeed before stale-value mutation");
            assert!(matches!(result, Err(SourceConsumerTestErrorV18::Source(ProductionSourceOwnedViewErrorV18::Binding(
                "stored scalar literal differs from its original typed source"
            )))));
        } else {
            result.unwrap();
            assert_eq!(checked.get(), 2, "both original helper instances reach current-definition checking");
        }
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn original_retained_operand_joins_its_load_and_store_without_granting_read_from() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_from_v18(scalar_read_store_owner_v18, &mut budget);
    let checked = std::cell::Cell::new(0usize);
    prepared.with_source_consumer_v18(&mut budget, |source, budget| -> Result<(), SourceConsumerTestErrorV18> {
        source.with_analysis_v18(budget, |scope| scope.with_inventory_v1(|inventory, budget| {
            source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                for root in 0..source.root_count(budget)? {
                    let function = &inventory.functions()[source.root(root, budget)?.1];
                    value_origin_v1::with_whole_value_origins_v18(relation, function.coordinate, budget, |origins, budget| {
                        for row in &inventory.operations()[function.operations.clone()] {
                            let OperationKind::Store { pointer, .. } = row.operation.kind else { continue; };
                            let Some(access) = relation.retained_memory_access(root, row.coordinate, pointer, budget)? else { continue; };
                            let Some(payload) = relation.retained_scalar_payload_v18(root, row.coordinate, &access, budget)? else { continue; };
                            let Some(transport) = source_scalar_read_store_v18(relation, root, &access, &payload, budget)? else { continue; };
                            assert_eq!(transport.instance, access.instance);
                            assert_eq!(transport.value, payload.value);
                            assert_eq!(transport.read.ty, U32);
                            assert_eq!(transport.read.prefix, 0);
                            assert_eq!(transport.read.role, ExecutionOperandV29::StoreValue);
                            assert_eq!(transport.read.site, ExecutionSiteV29::Statement {
                                block: SsaBlockIdV1::new(0), statement: 3,
                            });
                            assert!(matches!(transport.read.occurrence, ScopedMemoryOccurrenceV29::Retained { .. }));
                            assert_ne!(transport.operation, row.coordinate, "the Load and Store remain distinct effects");
                            checked.set(checked.get() + 1);
                        }
                        Ok::<_, SourceConsumerTestErrorV18>(())
                    })?;
                }
                Ok::<_, SourceConsumerTestErrorV18>(())
            })
        }))
    }).unwrap();
    assert_eq!(checked.get(), 2, "both original helper instances reach the same checked relation");
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn retained_operand_rejects_a_same_typed_load_from_another_original_occurrence() {
    for other_instance in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_from_v18(scalar_read_store_owner_v18, &mut budget);
        let checked = std::cell::Cell::new(false);
        let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| -> Result<(), SourceConsumerTestErrorV18> {
            source.with_analysis_v18(budget, |scope| scope.with_inventory_v1(|inventory, budget| {
                source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                    let function = &inventory.functions()[source.root(0, budget)?.1];
                    value_origin_v1::with_whole_value_origins_v18(relation, function.coordinate, budget, |origins, budget| {
                        for row in &inventory.operations()[function.operations.clone()] {
                            let OperationKind::Store { pointer, .. } = row.operation.kind else { continue; };
                            let Some(access) = relation.retained_memory_access(0, row.coordinate, pointer, budget)? else { continue; };
                            let Some(payload) = relation.retained_scalar_payload_v18(0, row.coordinate, &access, budget)? else { continue; };
                            let Some(original) = source_scalar_read_store_v18(relation, 0, &access, &payload, budget)? else { continue; };
                            for substitute in &inventory.operations()[function.operations.clone()] {
                                let OperationKind::Load { pointer, .. } = substitute.operation.kind else { continue; };
                                let Some(load) = relation.retained_memory_access(0, substitute.coordinate, pointer, budget)? else { continue; };
                                if (load.instance != original.instance) != other_instance
                                    || substitute.coordinate == original.operation { continue; }
                                let Some(value) = relation.retained_scalar_payload_v18(0, substitute.coordinate, &load, budget)? else { continue; };
                                let wrong = SourcePhysicalPayloadV18 {
                                    source: payload.source, value: value.value, store_use: payload.store_use,
                                };
                                let result = source_scalar_read_store_v18(relation, 0, &access, &wrong, budget);
                                assert!(matches!(result, Err(ProductionSourceOwnedViewErrorV18::Binding(
                                    "stored scalar Load differs from its original operand occurrence"
                                ))));
                                checked.set(true);
                                return result.map(|_| ());
                            }
                        }
                        panic!("the unchanged real fixture must first prove its original Load transport");
                    })?;
                    Ok::<_, SourceConsumerTestErrorV18>(())
                })
            }))
        });
        assert!(checked.get(), "the exact occurrence or repeated-instance rejection must execute");
        assert!(matches!(result, Err(SourceConsumerTestErrorV18::Source(ProductionSourceOwnedViewErrorV18::Binding(
            "stored scalar Load differs from its original operand occurrence"
        )))));
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn actual_scalar_payloads_join_repeated_instances_results_values_and_operand_uses() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_v18(&mut budget);
    let observed = std::cell::Cell::new((0usize, 0usize, 0usize, 0usize));
    prepared.with_source_consumer_v18(&mut budget, |source, budget| -> Result<(), SourceConsumerTestErrorV18> {
        source.with_analysis_v18(budget, |scope| scope.with_inventory_v1(|inventory, budget| {
            source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                let mut loads = 0usize;
                let mut stores = 0usize;
                let mut instances = 0usize;
                let mut literals = 0usize;
                for root in 0..source.root_count(budget)? {
                    let physical = source.root(root, budget)?.1;
                    let function = &inventory.functions()[physical];
                    value_origin_v1::with_whole_value_origins_v18(relation, function.coordinate, budget, |origins, budget| {
                    for instance in 0..source.instance_count(root, budget)? {
                        if source.instance(root, instance, budget)?.0.index() != 2 { continue; }
                        instances += 1;
                        let mut local_loads = 0usize;
                        let mut local_stores = 0usize;
                        for row in inventory.operations()[function.operations.clone()].iter() {
                            let pointer = match row.operation.kind {
                                OperationKind::Load { pointer, .. } | OperationKind::GuardedLoad { pointer, .. }
                                | OperationKind::Store { pointer, .. } | OperationKind::GuardedStore { pointer, .. } => pointer,
                                _ => continue,
                            };
                            let Some(access) = relation.retained_memory_access(root, row.coordinate, pointer, budget)? else { continue; };
                            if access.instance != instance { continue; }
                            let payload = relation.retained_scalar_payload_v18(root, row.coordinate, &access, budget)?
                                .expect("the original scalar grammar must retain its actual payload");
                            match payload.source {
                                ScopedMemoryPayloadV29::IndexLoad { .. } => panic!("ordinary scalar fixture has no projection index loads"),
                                ScopedMemoryPayloadV29::Load { read, .. } => {
                                    assert_eq!(read.ty, U32);
                                    assert_eq!(read.prefix, 0);
                                    assert!(matches!(read.occurrence, ScopedMemoryOccurrenceV29::Retained { .. }));
                                    assert_eq!(payload.value, row.operation.results[0].id);
                                    assert!(payload.store_use.is_none());
                                    local_loads += 1;
                                }
                                ScopedMemoryPayloadV29::Store { source, .. } => {
                                    assert!(matches!(source, ScopedMemoryStoreSourceV29::Operand { ty, .. } if *ty == U32));
                                    let OperationKind::Store { value, .. } = row.operation.kind else { panic!("actual scalar fixture Store"); };
                                    assert_eq!(payload.value, value);
                                    assert_eq!(payload.store_use, Some(fe2o3_kernel_ir::CanonicalKirUseCoordinateV1::OperationOperand {
                                        operation: row.coordinate, operand: 1,
                                    }));
                                    let literal = source_scalar_literal_store_v18(relation, root, &access, &payload, origins, budget)?;
                                    if matches!(source, ScopedMemoryStoreSourceV29::Operand { source: ScopedMemoryOperandSourceV29::Constant, .. }) {
                                        assert_eq!(literal, Some(Constant::U32(37)));
                                        literals += 1;
                                    } else { assert_eq!(literal, None, "a nonliteral capture is still an unchecked value relation"); }
                                    local_stores += 1;
                                }
                            }
                        }
                        assert_eq!((local_loads, local_stores), (2, 2), "each helper invocation owns its own exact payloads");
                        loads += local_loads;
                        stores += local_stores;
                    }
                    Ok::<_, SourceConsumerTestErrorV18>(())
                    })?;
                }
                observed.set((instances, loads, stores, literals));
                Ok::<_, SourceConsumerTestErrorV18>(())
            })
        }))
    }).unwrap();
    assert_eq!(observed.get(), (2, 4, 4, 2), "both actual repeated original calls reached correspondence");
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn scalar_store_use_normalization_keeps_equal_operands_distinct_and_checks_full_grammar() {
    for guarded in [false, true] {
        let kind = if guarded {
            OperationKind::GuardedStore { pointer: ValueId(7), predicate: ValueId(7), value: ValueId(7),
                access: MemoryAccess::new(AddressSpace::Global, 4) }
        } else {
            OperationKind::Store { pointer: ValueId(7), value: ValueId(7), access: MemoryAccess::new(AddressSpace::Global, 4) }
        };
        let operation = Operation::new(vec![], kind);
        let required = if guarded { 4 } else { 3 };
        for limit in [required - 1, required] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
            let mut budget = ArgumentBudgetV1::new(&mut work, 0);
            let result = tile_store_payload_operand_v18(&operation, ValueId(7), ValueId(7), &mut budget);
            if limit == required {
                assert_eq!(result.unwrap(), if guarded { 2 } else { 1 });
            } else { assert!(matches!(result, Err(ScopedTileFailureKindV29::Resource(ArgumentResourceV1::Work(_))))); }
            assert_eq!(budget.storage(), 0);
        }
        for (pointer, value) in [(ValueId(8), ValueId(7)), (ValueId(7), ValueId(8))] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(10);
            let mut budget = ArgumentBudgetV1::new(&mut work, 0);
            assert!(matches!(tile_store_payload_operand_v18(&operation, pointer, value, &mut budget),
                Err(ScopedTileFailureKindV29::ReplayMismatch)));
        }
    }
}

#[test]
fn literal_capture_does_not_bless_a_different_same_typed_stored_value() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_v18(&mut budget);
    let checked = std::cell::Cell::new(false);
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| -> Result<(), SourceConsumerTestErrorV18> {
        source.with_analysis_v18(budget, |scope| scope.with_inventory_v1(|inventory, budget| {
            source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                let function = &inventory.functions()[source.root(0, budget)?.1];
                value_origin_v1::with_whole_value_origins_v18(relation, function.coordinate, budget, |origins, budget| {
                    let substituted = inventory.operations()[function.operations.clone()].iter()
                        .find(|row| matches!(row.operation.kind, OperationKind::Constant(Constant::U32(13))))
                        .expect("the independent original Copy producer has a same-typed unequal literal").operation.results[0].id;
                    for row in &inventory.operations()[function.operations.clone()] {
                        let OperationKind::Store { pointer, .. } = row.operation.kind else { continue; };
                        let Some(access) = relation.retained_memory_access(0, row.coordinate, pointer, budget)? else { continue; };
                        let Some(payload) = relation.retained_scalar_payload_v18(0, row.coordinate, &access, budget)? else { continue; };
                        if source_scalar_literal_store_v18(relation, 0, &access, &payload, origins, budget)?.is_none() { continue; }
                        let wrong = SourcePhysicalPayloadV18 {
                            source: payload.source, value: substituted, store_use: payload.store_use,
                        };
                        let result = source_scalar_literal_store_v18(relation, 0, &access, &wrong, origins, budget);
                        assert!(matches!(result, Err(ProductionSourceOwnedViewErrorV18::Binding(
                            "stored scalar literal differs from its original typed source"
                        ))));
                        checked.set(true);
                        return result.map(|_| ());
                    }
                    panic!("the original fixture must first prove its unmodified scalar literal");
                })?;
                Ok::<_, SourceConsumerTestErrorV18>(())
            })
        }))
    });
    assert!(checked.get(), "the hostile value check must run outside any caught panic");
    assert!(matches!(result, Err(SourceConsumerTestErrorV18::Source(ProductionSourceOwnedViewErrorV18::Binding(
        "stored scalar literal differs from its original typed source"
    )))));
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn actual_scalar_payload_correspondence_rejects_value_use_result_and_instance_substitutions() {
    for mutation in 0..5 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_v18(&mut budget);
        let checked = std::cell::Cell::new(false);
        let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| -> Result<(), SourceConsumerTestErrorV18> {
            source.with_analysis_v18(budget, |scope| scope.with_inventory_v1(|inventory, budget| {
                source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                    let physical = source.root(0, budget)?.1;
                    let function = &inventory.functions()[physical];
                    for row in &inventory.operations()[function.operations.clone()] {
                        let pointer = match row.operation.kind {
                            OperationKind::Load { pointer, .. } if mutation == 0 || mutation == 3 => pointer,
                            OperationKind::Store { pointer, .. } if mutation == 1 || mutation == 2 || mutation == 4 => pointer,
                            _ => continue,
                        };
                        let Some(access) = relation.retained_memory_access(0, row.coordinate, pointer, budget)? else { continue; };
                        if relation.retained_scalar_payload_v18(0, row.coordinate, &access, budget)?.is_none() { continue; }
                        let floor = budget.storage();
                        let headers = std::mem::size_of::<Vec<SourceAttachmentV18>>()
                            + std::mem::size_of::<ProductionSourceCorrespondenceV18<'_>>()
                            + std::mem::size_of::<SourcePhysicalAccessV18<'_>>();
                        budget.reserve_storage(headers)?;
                        let mut rows = source_attachments_v18(source, inventory, budget)?;
                        let target = match mutation {
                            0 | 3 => TileAttachmentFieldV29::MemoryLoadResult,
                            1 => TileAttachmentFieldV29::MemoryStoreValue,
                            _ => TileAttachmentFieldV29::MemoryStoreUse,
                        };
                        let replacement = if mutation == 3 { TileAttachmentLocationV29::NoOutput } else {
                            rows.iter().find(|row| row.key.root == 0
                                && row.key.family == TileAttachmentFamilyV29::MemoryAnchor
                                && row.key.instance == access.instance && row.key.row == access.row
                                && row.key.field == TileAttachmentFieldV29::MemoryPointer).unwrap().location
                        };
                        if mutation != 4 {
                            let field = rows.iter_mut().find(|row| row.key.root == 0
                                && row.key.family == TileAttachmentFamilyV29::MemoryAnchor
                                && row.key.instance == access.instance && row.key.row == access.row
                                && row.key.field == target).unwrap();
                            let original = field.location;
                            field.location = replacement;
                            assert_ne!(original, field.location, "mutation changes an independently accepted exact payload projection");
                        }
                        let tampered = ProductionSourceCorrespondenceV18 {
                            source, inventory, attachments: &rows, slot: std::ptr::from_ref(budget) as usize,
                            ledger: budget.work_ledger_identity_v1(), floor: budget.storage(),
                        };
                        let wrong_instance = SourcePhysicalAccessV18 {
                            instance: if access.instance == 1 { 2 } else { 1 },
                            row: access.row, anchor: access.anchor,
                        };
                        let selected = tampered.retained_scalar_payload_v18(0, row.coordinate,
                            if mutation == 4 { &wrong_instance } else { &access }, budget);
                        assert!(matches!(&selected, Err(ProductionSourceOwnedViewErrorV18::Binding(_))),
                            "same typed values/instances cannot substitute for exact original payloads: {mutation}");
                        let error = match selected { Err(error) => error, Ok(_) => unreachable!() };
                        checked.set(true);
                        drop(tampered);
                        drop(rows);
                        budget.release_storage(budget.storage().checked_sub(floor).unwrap())?;
                        return Err(error.into());
                    }
                    panic!("actual prepared scalar fixture must reach a positive payload before mutation");
                })
            }))
        });
        assert!(checked.get(), "hostile tests must execute after their unchanged positive control");
        assert!(matches!(result, Err(SourceConsumerTestErrorV18::Source(ProductionSourceOwnedViewErrorV18::Binding(_)))));
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn actual_source_memory_census_joins_original_accesses_and_backings_without_epoch_claims() {
    use fe2o3_kernel_ir::KirLocalMemoryEffectRefV1 as Effect;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = prepared_source_fixture(ModuleFixture::Array, false, &mut budget);
    let checked = std::cell::Cell::new((0usize, 0usize));
    prepared.with_source_consumer_v18(&mut budget, |source, budget| -> Result<(), SourceConsumerTestErrorV18> {
        source.with_analysis_v18(budget, |scope| scope.with_inventory_v1(|inventory, budget| {
            source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                let mut backings = 0usize;
                let mut accesses = 0usize;
                for root in 0..source.root_count(budget)? {
                    let (_, physical) = source.root(root, budget)?;
                    let function = &inventory.functions()[physical];
                    let floor = budget.storage();
                    budget.reserve_storage(std::mem::size_of::<SourcePointerSpacesV18<'_, '_>>()
                        + std::mem::size_of::<Vec<SourceMemoryEffectV18>>()
                        + std::mem::size_of::<Vec<SourceMemoryBindingV18>>())?;
                    let spaces = SourcePointerSpacesV18::derive(relation, inventory, function.coordinate, budget)?;
                    let rows = source_memory_effects_v18(relation, root, &spaces, MODULE_LIMIT, budget)?;
                    let bindings = source_memory_bindings_v18(relation, root, &rows, budget)?;
                    assert_eq!(bindings.len(), rows.len());
                    let expected = inventory.effects()[function.effects.clone()].iter()
                        .filter(|row| matches!(row.effect, Effect::Read(_) | Effect::Write(_)
                            | Effect::VolatileRead(_) | Effect::VolatileWrite(_) | Effect::Atomic { .. })).count();
                    assert_eq!(rows.len(), expected, "no physical effect is omitted from the census");
                    for (row, binding) in rows.iter().zip(&bindings) {
                        let block = &inventory.blocks()[function.blocks.start + row.coordinate.block.block as usize];
                        let operation = &block.block.operations[row.coordinate.operation as usize];
                        assert_eq!(row.location.block, block.block.id);
                        assert_eq!(row.location.operation_index, row.coordinate.operation as usize);
                        assert_eq!(row.ordinal, 0, "the actual array fixture emits one access per operation");
                        assert_eq!(row.static_space, AddressSpace::Private);
                        assert_eq!(row.concrete_space, Some(AddressSpace::Private));
                        assert!(!row.storage_operation);
                        assert_eq!(row.ranked_consumer().unwrap().memory_space, dialect_kernel::MemorySpaceAttr::Private);
                        let evidence = relation.retained_memory_access(root, row.coordinate, row.pointer, budget)?.unwrap();
                        assert_eq!(binding.instance, Some(evidence.instance));
                        assert!(binding.span.is_some() || binding.lifecycle.is_some() || binding.failure.is_some());
                        let sidecar = source.sidecar(root, evidence.instance, budget)?;
                        assert!(std::ptr::eq(evidence.anchor, &sidecar.scoped_memory_anchors.as_ref().unwrap().rows[evidence.row]));
                        assert_eq!(scoped_memory_pointer_v29(&operation.kind), Some(row.pointer));
                        accesses += 1;
                    }
                    for row in &inventory.operations()[function.operations.clone()] {
                        if let Some(backing) = relation.retained_allocation(root, row.coordinate, budget)? {
                            let source_root = source.root_row(root)?;
                            assert!(std::ptr::eq(backing.slot, &source_root.source_slots.slots[backing.row]));
                            assert_eq!(backing.slot.instance.index(), backing.instance);
                            assert!(backing.slot.scalar_array().unwrap().length > 0);
                            let OperationKind::Alloca { .. } = row.operation.kind else { panic!("backing is not an allocation") };
                            backings += 1;
                        }
                    }
                    drop(rows);
                    drop(bindings);
                    drop(spaces);
                    let reserved = budget.storage().checked_sub(floor).unwrap();
                    budget.release_storage(reserved)?;
                    assert_eq!(budget.storage(), floor);
                }
                checked.set((backings, accesses));
                Ok::<_, SourceConsumerTestErrorV18>(())
            })
        }))
    }).unwrap();
    assert!(checked.get().0 > 0);
    assert!(checked.get().1 > 0);
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn actual_inventory_correlation_queries_keep_exact_definitions_and_edge_occurrences() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = prepared_source_fixture(ModuleFixture::Mixed, false, &mut budget);
    let visited = std::cell::Cell::new(0_usize);
    prepared
        .with_source_consumer_v18(
            &mut budget,
            |source, budget| -> Result<(), SourceConsumerTestErrorV18> {
                source.with_analysis_v18(budget, |scope| {
                    scope.with_inventory_v1(|inventory, budget| {
                        let floor = budget.storage();
                        let header = std::mem::size_of::<CorrelationLedgerV18<'_, '_, '_>>()
                            + std::mem::size_of::<InventoryCorrelationV18<'_, '_, '_, '_, '_, '_>>(
                            );
                        budget.reserve_storage(header)?;
                        let ledger = CorrelationLedgerV18::new(budget, source.cleanup);
                        for function in inventory.functions() {
                            let graph = InventoryCorrelationV18 {
                                origins: None,
                                inventory,
                                function: function.coordinate,
                                ledger: &ledger,
                                inline_scalar: &Gfx942InlineScalarCorrespondenceV30::empty(),
                                scalar_source: None,
                            };
                            for block in &inventory.blocks()[function.blocks.clone()] {
                                assert!(std::ptr::eq(
                                    graph.block_operations(block.block.id).unwrap(),
                                    block.block.operations.as_slice()
                                ));
                                for operation in &inventory.operations()[block.operations.clone()] {
                                    let location = FunctionOperationLocation::new(
                                        block.block.id,
                                        operation.coordinate.operation as usize,
                                    );
                                    assert!(std::ptr::eq(
                                        graph.operation(location).unwrap(),
                                        operation.operation
                                    ));
                                    for result in &operation.operation.results {
                                        assert!(std::ptr::eq(
                                            graph.definition(result.id).unwrap(),
                                            operation.operation
                                        ));
                                        assert_eq!(
                                            graph.definition_location(result.id),
                                            Some(location)
                                        );
                                        assert_eq!(
                                            graph.visit_incoming(result.id, &mut |_| panic!(
                                                "operation result is not an incoming block argument"
                                            )),
                                            Some(None)
                                        );
                                    }
                                    visited.set(visited.get() + 1);
                                }
                                for parameter in &block.block.parameters {
                                    let mut ordinal = 0;
                                    let expected = inventory.edge_arguments()
                                        [function.edge_arguments.clone()]
                                    .iter()
                                    .filter(|row| {
                                        inventory.definitions()[row.target_definition].value
                                            == Some(parameter.id)
                                    })
                                    .count();
                                    let actual = graph.visit_incoming(parameter.id, &mut |value| {
                                        let expected_row = inventory.edge_arguments()
                                            [function.edge_arguments.clone()]
                                        .iter()
                                        .filter(|row| {
                                            inventory.definitions()[row.target_definition].value
                                                == Some(parameter.id)
                                        })
                                        .nth(ordinal)
                                        .unwrap();
                                        assert_eq!(value, expected_row.value);
                                        ordinal += 1;
                                        Some(())
                                    });
                                    assert_eq!(actual, Some(Some(expected)));
                                    assert_eq!(ordinal, expected);
                                }
                            }
                            assert!(
                                graph
                                    .operation(FunctionOperationLocation::new(BlockId(u32::MAX), 0))
                                    .is_none()
                            );
                            assert!(graph.definition(ValueId(u32::MAX)).is_none());
                        }
                        assert_eq!(ledger.failure.get(), None);
                        assert!(!ledger.inconsistent_inventory.get());
                        drop(ledger);
                        budget.release_storage(header)?;
                        assert_eq!(budget.storage(), floor);
                        Ok::<_, SourceConsumerTestErrorV18>(())
                    })
                })
            },
        )
        .unwrap();
    assert!(visited.get() > 0);
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn source_correlation_work_uses_original_ledger_and_latches_exact_failed_batch() {
    for remaining in [4_usize, 5] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = prepared_source_fixture(ModuleFixture::Ordinary, false, &mut budget);
        let completed = std::cell::Cell::new(false);
        let refusal = std::cell::Cell::new(None);
        let result = prepared.with_source_consumer_v18(
            &mut budget,
            |source, budget| -> SourceOwnedResultV18<()> {
                source.source_semantic(budget)?;
                let header = std::mem::size_of::<CorrelationLedgerV18<'_, '_, '_>>()
                    + std::mem::size_of::<SourceCorrelationChargeV18<'_, '_, '_, '_>>();
                budget.reserve_storage(header)?;
                let padding = MODULE_LIMIT
                    .checked_sub(budget.work())
                    .unwrap()
                    .checked_sub(remaining)
                    .unwrap();
                budget.charge_work(padding)?;
                let before = budget.work();
                let ledger = CorrelationLedgerV18::new(budget, source.cleanup);
                let mut charge = SourceCorrelationChargeV18 {
                    ledger: &ledger,
                    finite: UnsupportedIndexCorrelationBudgetV1 { remaining: 19 },
                    finite_denied: false,
                };
                if remaining == 5 {
                    assert_eq!(charge.charge_many(5), Some(()));
                    assert_eq!(charge.finite.remaining, 14);
                    assert_eq!(ledger.budget.borrow().work(), before + 5);
                    assert_eq!(ledger.failure.get(), None);
                } else {
                    assert_eq!(charge.charge_many(5), None);
                    assert_eq!(
                        charge.finite.remaining, 19,
                        "failed batch cannot debit the finite counter"
                    );
                    assert_eq!(ledger.budget.borrow().work(), before);
                    let error = ledger.failure.get().unwrap();
                    assert!(matches!(error, ArgumentResourceV1::Work(_)));
                    refusal.set(Some(error));
                    assert_eq!(charge.charge(), None);
                    assert_eq!(ledger.failure.get(), Some(error));
                    assert_eq!(
                        ledger.budget.borrow().work(),
                        before,
                        "no smaller retry may perform work after denial"
                    );
                }
                completed.set(true);
                drop(charge);
                drop(ledger);
                budget.release_storage(header)?;
                if let Some(error) = refusal.get() {
                    Err(error.into())
                } else {
                    Ok(())
                }
            },
        );
        assert!(completed.get());
        if let Some(expected) = refusal.get() {
            assert!(
                matches!(result, Err(ProductionSourceOwnedViewErrorV18::Resource(actual)) if actual == expected)
            );
        } else {
            result.unwrap();
        }
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
    let mut legacy = UnsupportedIndexCorrelationBudgetV1 { remaining: 3 };
    assert_eq!(legacy.charge_many(4), None);
    assert_eq!(legacy.remaining, 3);
    assert_eq!(legacy.charge_many(3), Some(()));
    assert_eq!(legacy.remaining, 0);
}

#[test]
fn actual_source_analysis_and_attachment_reader_keep_one_graph_and_table() {
    for kind in [
        ModuleFixture::Ordinary,
        ModuleFixture::Mixed,
        ModuleFixture::Array,
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = prepared_source_fixture(kind, false, &mut budget);
        let original = *prepared.source.owner.source_semantic_sha256();
        let called = std::cell::Cell::new(false);
        prepared.with_source_consumer_v18(&mut budget, |source, budget| -> Result<(), SourceConsumerTestErrorV18> {
            let graph = source.canonical(budget)?;
            let table = graph.module().storage_layouts.as_ptr();
            source.with_analysis_v18(budget, |scope| {
                scope.with_sparse_v1(|report, _| {
                    assert!(std::ptr::eq(report.inventory().owner(), graph));
                    Ok::<_, SourceConsumerTestErrorV18>(())
                })?;
                scope.with_memory_ssa_v1(|report, _| {
                    assert_eq!(report.inventory().owner().module().storage_layouts.as_ptr(), table);
                    Ok::<_, SourceConsumerTestErrorV18>(())
                })?;
                scope.with_inventory_v1(|inventory, budget| {
                    source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                        called.set(true);
                        assert!(std::ptr::eq(relation.inventory(budget)?, inventory));
                        assert_eq!(relation.source(budget)?.source_ssa(budget)?.source_semantic_sha256(), &original);
                        assert!(!relation.attachments.is_empty());
                        let mut mapped_sites = 0;
                        let mut initializers = 0;
                        for root in 0..source.root_count(budget)? {
                            let root_row = source.root_row(root)?;
                            for span in &root_row.coordinates.spans.rows {
                                let (block, statement) = match span.source {
                                    InstanceSpanSourceV1::Statement(site) => (site.semantic_block, Some(site.statement_ordinal)),
                                    InstanceSpanSourceV1::Terminator(site) => (site.semantic_block, None),
                                    _ => continue,
                                };
                                let mut output = 0;
                                relation.visit_source_operations(root, span.instance.index(), block, statement, budget, |operation, _| {
                                    match operation {
                                        ProductionSourceOperationV18::Operation(coordinate) => {
                                            let actual = &graph.module().functions[coordinate.block.function.0 as usize].body.as_ref().unwrap().blocks[coordinate.block.block as usize].operations[coordinate.operation as usize];
                                            assert!(inventory.operations().iter().any(|row| row.coordinate == coordinate && std::ptr::eq(row.operation, actual)));
                                        }
                                        ProductionSourceOperationV18::Gap { block, operation } => {
                                            assert!(operation as usize <= graph.module().functions[block.function.0 as usize].body.as_ref().unwrap().blocks[block.block as usize].operations.len());
                                        }
                                        ProductionSourceOperationV18::RemovedCall => assert!(span.removed_call.is_some()),
                                        ProductionSourceOperationV18::NoOperations => assert!(span.segments.iter().all(Option::is_none)),
                                    }
                                    output += 1;
                                    Ok(())
                                })?;
                                assert!(output > 0, "even zero-operation source sites retain a mapped outcome");
                                if let Some(statement) = statement {
                                    let function = source.instance(root, span.instance.index(), budget)?.0;
                                    let original = &source.source_semantic(budget)?.functions()[function.index() as usize].blocks()[block.index() as usize].statements()[statement as usize];
                                    if let SemanticStatementKindV1::Assign(assignment) = original.kind() {
                                        if let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind() {
                                            if aggregate.kind() == &SemanticAggregateKindV1::Array {
                                                let count = relation.private_array_initializer_count(root, span.instance.index(), block, statement, budget)?;
                                                assert_eq!(count, Some(aggregate.operands().len() as u64));
                                                initializers += 1;
                                            }
                                        }
                                    }
                                }
                                mapped_sites += 1;
                            }
                        }
                        assert!(mapped_sites > 0);
                        if matches!(kind, ModuleFixture::Array) {
                            assert!(initializers > 0, "the exact source array relation must run");
                        }
                        Ok::<_, SourceConsumerTestErrorV18>(())
                    })
                })
            })
        }).unwrap();
        assert!(called.get());
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn actual_prepared_source_propagates_analysis_no_refund_and_selected_error_or_panic() {
    for panic in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = prepared_source_fixture(ModuleFixture::Ordinary, false, &mut budget);
        let observed = std::cell::Cell::new(None);
        let selected = Box::new([11u8, 19, 23]);
        let address = std::ptr::from_ref(&*selected);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            prepared.with_source_consumer_v18(
                &mut budget,
                |source, budget| -> Result<(), SourceConsumerTestErrorV18> {
                    source.with_analysis_v18(budget, |scope| {
                        scope.with_inventory_v1(|_, budget| {
                            budget.release_storage(1).unwrap();
                            observed.set(Some(budget.storage()));
                            if panic {
                                std::panic::panic_any(selected);
                            }
                            Err(SourceConsumerTestErrorV18::Selected(selected))
                        })
                    })
                },
            )
        }));
        let selected = match result {
            Ok(Err(SourceConsumerTestErrorV18::Selected(selected))) if !panic => selected,
            Err(payload) if panic => *payload.downcast::<Box<[u8; 3]>>().unwrap(),
            other => panic!("source owner replaced selected analysis result: {other:?}"),
        };
        assert_eq!(std::ptr::from_ref(&*selected), address);
        assert_eq!(*selected, [11, 19, 23]);
        assert_eq!(Some(budget.storage()), observed.get());
        assert!(
            budget.storage() > MODULE_FLOOR,
            "denied inner cleanup must reach the actual consuming owner"
        );
    }
}

#[test]
fn actual_source_correspondence_does_not_hide_partial_metadata_floor_loss() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = prepared_source_fixture(ModuleFixture::Mixed, false, &mut budget);
    let observed = std::cell::Cell::new(None);
    let selected = Box::new([2u8, 3, 5]);
    let address = std::ptr::from_ref(&*selected);
    let result = prepared.with_source_consumer_v18(
        &mut budget,
        |source, budget| -> Result<(), SourceConsumerTestErrorV18> {
            source.with_analysis_v18(budget, |scope| {
                scope.with_inventory_v1(|inventory, budget| {
                    source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                        assert!(!relation.attachments.is_empty());
                        budget.release_storage(1).unwrap();
                        observed.set(Some(budget.storage()));
                        Err(SourceConsumerTestErrorV18::Selected(selected))
                    })
                })
            })
        },
    );
    let Err(SourceConsumerTestErrorV18::Selected(selected)) = result else {
        panic!("replaced selected metadata failure: {result:?}")
    };
    assert_eq!(std::ptr::from_ref(&*selected), address);
    assert_eq!(*selected, [2, 3, 5]);
    assert_eq!(Some(budget.storage()), observed.get());
}

#[test]
fn actual_original_root_argument_data_uses_retained_parameter_rows() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = prepared_source_fixture(ModuleFixture::Ordinary, false, &mut budget);
    let completed = std::cell::Cell::new(0);
    prepared.with_source_consumer_v18(&mut budget, |source, budget| -> Result<(), SourceConsumerTestErrorV18> {
        source.with_analysis_v18(budget, |scope| scope.with_inventory_v1(|inventory, budget| {
            source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                for root in 0..source.root_count(budget)? {
                    let (original_root, physical) = source.root(root, budget)?;
                    let sidecar = source.sidecar(root, 0, budget)?;
                    let target = inventory.functions()[physical].function;
                    let floor = budget.storage();
                    relation.with_root_argument_data_v18(root, budget, |data, budget| {
                        assert_eq!(data.semantic_function, original_root);
                        assert!(std::ptr::eq(data.target, target));
                        assert_eq!(data.logical.source_arguments().len(), 1);
                        let mut visited = 0;
                        data.visit_nodes_scoped(budget, |node, _| {
                            assert_eq!(node.source_argument(), 0);
                            assert_eq!(node.semantic_type(), U32);
                            assert!(node.source_path().is_empty());
                            let ProductionArgumentCoverageV1::Parameter(parameter) = node.coverage() else {
                                panic!("ordinary original argument lost its physical parameter");
                            };
                            assert_eq!(parameter.slot(), 0);
                            assert_eq!(parameter.value(), target.body.as_ref().unwrap().parameters[0]);
                            let ProductionArgumentTraceV1::Direct(row) = parameter.trace() else {
                                panic!("ordinary source argument unexpectedly became a component");
                            };
                            assert!(std::ptr::eq(row, &sidecar.parameter_bindings[0]));
                            assert_eq!(row.semantic_function, original_root);
                            assert_eq!(row.correspondence_owner, original_root);
                            visited += 1;
                            Ok(())
                        })?;
                        assert_eq!(visited, 1);
                        Ok(())
                    })?;
                    assert_eq!(budget.storage(), floor);
                    completed.set(completed.get() + 1);
                }
                Ok::<_, SourceConsumerTestErrorV18>(())
            })
        }))
    }).unwrap();
    assert_eq!(completed.get(), 2);
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn actual_argument_path_loss_cannot_be_refunded_after_caught_error_or_panic() {
    let mut completed = 0;
    for restore in [false, true] {
        for mode in 0..3 {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
            let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
            budget.reserve_storage(MODULE_FLOOR).unwrap();
            let prepared = prepared_source_fixture(ModuleFixture::Ordinary, false, &mut budget);
            let residual = std::cell::Cell::new(None);
            let selected = Box::new([29u8, 31, 37]);
            let address = std::ptr::from_ref(&*selected);
            let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| -> Result<(), SourceConsumerTestErrorV18> {
                source.with_analysis_v18(budget, |scope| scope.with_inventory_v1(|inventory, budget| {
                    source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                        relation.with_root_argument_data_v18(0, budget, |data, budget| {
                            let argument_floor = budget.storage();
                            let mut selected = Some(selected);
                            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                data.visit_nodes_scoped(budget, |_, budget| {
                                    assert!(budget.storage() > argument_floor, "the actual path backing must be live");
                                    budget.release_storage(1).unwrap();
                                    residual.set(Some(budget.storage()));
                                    match mode {
                                        0 => Ok(()),
                                        1 => Err(ArgumentResourceV1::Arithmetic.into()),
                                        _ => std::panic::panic_any(selected.take().unwrap()),
                                    }
                                })
                            }));
                            match (mode, result) {
                                (0, Ok(Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Accounting)))) => (),
                                (1, Ok(Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Arithmetic)))) => (),
                                (2, Err(payload)) => {
                                    let payload = *payload.downcast::<Box<[u8; 3]>>().unwrap();
                                    assert_eq!(std::ptr::from_ref(&*payload), address);
                                    assert_eq!(*payload, [29, 31, 37]);
                                },
                                (_, other) => panic!("argument path cleanup replaced callback chronology: {other:?}"),
                            }
                            assert_eq!(Some(budget.storage()), residual.get());
                            assert!(data.cleanup.is_denied());
                            if restore {
                                budget.reserve_storage(1).unwrap();
                                residual.set(Some(budget.storage()));
                            }
                            let before = budget.work();
                            assert!(matches!(data.physical(0, budget), Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Accounting))));
                            let retry = data.visit_nodes_scoped(budget, |_, _| panic!("poisoned view reached a node"));
                            assert!(matches!(retry, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Accounting))));
                            assert_eq!(budget.work(), before);
                            completed += 1;
                            Ok(())
                        })?;
                        panic!("denied argument custody escaped the source correspondence");
                    })
                }))
            });
            assert!(matches!(
                result,
                Err(SourceConsumerTestErrorV18::Source(
                    ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
                ))
            ));
            assert_eq!(Some(budget.storage()), residual.get());
            assert!(budget.storage() > MODULE_FLOOR);
        }
    }
    assert_eq!(completed, 6);
}

#[test]
fn actual_instance_effect_census_includes_nested_synthetic_and_lifecycle_operations() {
    for kind in [
        ModuleFixture::Ordinary,
        ModuleFixture::Mixed,
        ModuleFixture::Array,
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = prepared_source_fixture(kind, false, &mut budget);
        let completed = std::cell::Cell::new(0);
        prepared
            .with_source_consumer_v18(
                &mut budget,
                |source, budget| -> Result<(), SourceConsumerTestErrorV18> {
                    source.with_analysis_v18(budget, |scope| {
                        scope.with_inventory_v1(|inventory, budget| {
                            let (effects, receipt) =
                                fe2o3_kernel_analysis::CanonicalKirCallEffectsV18::derive_v18(
                                    inventory, budget,
                                )
                                .unwrap();
                            budget.reserve_storage(receipt.retained_storage())?;
                            let result = source.with_ranked_correspondence_v18(
                                inventory,
                                budget,
                                |relation, budget| {
                                    for root in 0..source.root_count(budget)? {
                                        let (_, physical) = source.root(root, budget)?;
                                        let expected = effects
                                            .decision(
                                                inventory.functions()[physical].coordinate,
                                                budget,
                                            )
                                            .unwrap();
                                        assert_eq!(
                                            relation.instance_effect_decision(
                                                root, 0, &effects, budget
                                            )?,
                                            expected
                                        );
                                        let instances =
                                            &source.root_row(root)?.coordinates.sources.rows;
                                        for child in instances {
                                            if let Some(incoming) = child.incoming {
                                                assert_eq!(
                                                    relation.defined_call_instance(
                                                        root,
                                                        incoming.caller.index(),
                                                        incoming.block,
                                                        budget
                                                    )?,
                                                    child.instance.index()
                                                );
                                                relation.instance_effect_decision(
                                                    root,
                                                    child.instance.index(),
                                                    &effects,
                                                    budget,
                                                )?;
                                            }
                                        }
                                        completed.set(completed.get() + 1);
                                    }
                                    Ok::<_, SourceConsumerTestErrorV18>(())
                                },
                            );
                            drop(effects);
                            budget.release_storage(receipt.retained_storage())?;
                            result
                        })
                    })
                },
            )
            .unwrap();
        assert_eq!(
            completed.get(),
            if matches!(kind, ModuleFixture::Ordinary) {
                2
            } else {
                3
            }
        );
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}
