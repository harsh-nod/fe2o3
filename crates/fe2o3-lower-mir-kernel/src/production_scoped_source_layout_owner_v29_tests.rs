#[test]
fn scoped_layout_owner_retains_the_complete_original_table_without_new_memory_operations() {
    for kind in [
        ModuleFixture::Ordinary,
        ModuleFixture::Mixed,
        ModuleFixture::Array,
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        with_module_fixture(kind, &mut budget, |source, budget| {
            let floor = budget.storage();
            let pending = admit_pending_scoped_module_v29(
                source,
                ProductionSemanticKirLimitsV1::default(),
                budget,
            )?;
            let retained = budget.storage();
            check_scoped_module(&pending, source, kind);
            let demands =
                source_storage_demands_v29::SourceStorageDemandsV29::collect(source.owner, budget)?;
            let layouts = source_storage_v29::SourceStorageLayoutsV29::new_with_limits(
                source.owner,
                demands.types(source.owner, budget)?,
                ProductionSemanticKirLimitsV1::default().storage_layout_limits(),
                budget,
            )?;
            {
                let rows = layouts.rows(source.owner, budget)?;
                assert_eq!(&*rows, pending.graph.module().storage_layouts.as_slice());
                if matches!(kind, ModuleFixture::Ordinary) {
                    assert!(rows.is_empty());
                }
                if matches!(kind, ModuleFixture::Array) {
                    assert!(!rows.is_empty());
                }
            }
            layouts.release(budget)?;
            demands.discard(budget)?;
            assert_eq!(budget.storage(), retained);
            for function in &pending.graph.module().functions {
                if let Some(body) = &function.body {
                    for block in &body.blocks {
                        assert!(
                            block.operations.iter().all(|operation| !matches!(
                                operation.kind,
                                OperationKind::Storage(_)
                            ))
                        );
                    }
                }
            }
            for _ in 0..3 {
                let assertions = reconstruct_scoped_source_v29(
                    &pending,
                    source,
                    ProductionSemanticKirLimitsV1::default(),
                    budget,
                )?;
                assert_origin_drop_v1(assertions, budget)
                    .map_err(ProductionSemanticKirErrorV1::from)?;
                assert_eq!(budget.storage(), retained);
            }
            let owned = pending.retained_storage;
            drop(pending);
            budget.release_storage(owned)?;
            assert_eq!(budget.storage(), floor);
            Ok::<_, ScopedModuleErrorV29>(())
        })
        .unwrap()
        .unwrap();
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn scoped_layout_replay_compares_table_rows_not_only_unchanged_functions() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    with_module_fixture(ModuleFixture::Array, &mut budget, |source, budget| {
        let floor = budget.storage();
        let pending = admit_pending_scoped_module_v29(
            source,
            ProductionSemanticKirLimitsV1::default(),
            budget,
        )?;
        let retained = budget.storage();
        with_scoped_source_cleanup_v29(budget, retained, |cleanup, budget| {
            let entry = budget.storage();
            let (mut candidate, roots, _) = scoped_source_candidate_v29(
                source,
                ProductionSemanticKirLimitsV1::default(),
                cleanup,
                budget,
            )?;
            let scratch = budget.storage() - entry;
            assert!(
                pending
                    .graph
                    .matches_module_with_budget_v18(&candidate, budget)
                    .map_err(ScopedModuleErrorV29::Canonical)?
            );
            let row = candidate
                .storage_layouts
                .pop()
                .expect("original array layout closure");
            assert!(
                !pending
                    .graph
                    .matches_module_with_budget_v18(&candidate, budget)
                    .map_err(ScopedModuleErrorV29::Canonical)?
            );
            candidate.storage_layouts.push(row);
            assert!(
                pending
                    .graph
                    .matches_module_with_budget_v18(&candidate, budget)
                    .map_err(ScopedModuleErrorV29::Canonical)?
            );
            drop((candidate, roots));
            budget.release_storage(scratch)?;
            assert_eq!(budget.storage(), entry);
            Ok::<_, ScopedModuleErrorV29>(())
        })?;
        assert_eq!(budget.storage(), retained);
        let owned = pending.retained_storage;
        drop(pending);
        budget.release_storage(owned)?;
        assert_eq!(budget.storage(), floor);
        Ok::<_, ScopedModuleErrorV29>(())
    })
    .unwrap()
    .unwrap();
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn scoped_layout_caller_policy_refuses_original_nonempty_table_before_emission() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    with_module_fixture(ModuleFixture::Array, &mut budget, |source, budget| {
        let floor = budget.storage();
        let limits = ProductionSemanticKirLimitsV1::default().with_storage_layout_limits(
            fe2o3_kernel_ir::StorageLayoutLimitsV1 {
                rows: 0,
                edges: 0,
                containment_depth: 0,
                object_bytes: 0,
            },
        );
        let result = admit_pending_scoped_module_v29(source, limits, budget);
        assert!(matches!(
            result,
            Err(ScopedModuleErrorV29::Source(
                ProductionSemanticKirErrorV1::Unsupported {
                    detail: "source storage table exceeds retained row policy",
                    ..
                }
            ))
        ));
        assert_eq!(budget.storage(), floor);
        let owner = admit_pending_scoped_module_v29(
            source,
            ProductionSemanticKirLimitsV1::default(),
            budget,
        )?;
        assert!(!owner.graph.module().storage_layouts.is_empty());
        let owned = owner.retained_storage;
        drop(owner);
        budget.release_storage(owned)?;
        assert_eq!(budget.storage(), floor);
        Ok::<_, ScopedModuleErrorV29>(())
    })
    .unwrap()
    .unwrap();
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn scoped_layout_owned_receipt_excludes_every_transient_cleanup_header() {
    for kind in [ModuleFixture::Ordinary, ModuleFixture::Array] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let source = owning_source_fixture(kind, true, &mut budget).unwrap();
        let rollback_floor = budget.storage() - source.input.retained_storage;
        let mut donor = Some(source);
        let owner = SourceOwnedScopedModuleV29::try_new(
            &mut donor,
            ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        )
        .unwrap();
        assert!(donor.is_none());
        let retained_output = owner.retained_storage;
        assert_eq!(budget.storage(), rollback_floor + retained_output);
        for _ in 0..3 {
            owner.replay(&mut budget).unwrap();
            assert_eq!(budget.storage(), rollback_floor + retained_output);
        }
        drop(owner);
        budget.release_storage(retained_output).unwrap();
        assert_eq!(budget.storage(), rollback_floor);
    }
}
