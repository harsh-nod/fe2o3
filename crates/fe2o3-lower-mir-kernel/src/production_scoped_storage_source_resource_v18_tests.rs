#[test]
fn source_owned_v18_preserves_narrow_caller_policy_through_reconstruction() {
    let policy = fe2o3_kernel_ir::StorageLayoutLimitsV1 {
        rows: 64,
        edges: 128,
        containment_depth: 16,
        object_bytes: 4096,
    };
    let limits = ProductionSemanticKirLimitsV1::default().with_storage_layout_limits(policy);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let mut pending = with_pending_api_input(
        ModuleFixture::Array,
        false,
        &mut budget,
        |owner, launch, input, _, budget| {
            ProductionPendingScopedSourceOwnerV29::try_materialize_with_budget(
                owner, launch, input, limits, budget,
            )
            .unwrap()
        },
    );
    assert_eq!(pending.inner.limits.storage_layout_limits(), policy);
    assert!(!pending.pending_module().storage_layouts.is_empty());
    let floor = budget.storage();
    pending.replay_with_budget(&mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
    pending.inner.limits = limits
        .with_storage_layout_limits(fe2o3_kernel_ir::StorageLayoutLimitsV1 { rows: 0, ..policy });
    assert!(pending.replay_with_budget(&mut budget).is_err());
    assert_eq!(budget.storage(), floor);
    pending.inner.limits = limits;
    pending.replay_with_budget(&mut budget).unwrap();
    let retained = pending.adopted_storage();
    drop(pending);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn source_owned_v18_never_infers_larger_layout_limits_from_the_candidate() {
    for fault in 0..4 {
        let mut policy = ProductionSemanticKirLimitsV1::default().storage_layout_limits();
        match fault {
            0 => policy.rows = 0,
            1 => policy.edges = 0,
            2 => policy.containment_depth = 0,
            3 => policy.object_bytes = 0,
            _ => unreachable!(),
        }
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        with_pending_api_input(
            ModuleFixture::Array,
            false,
            &mut budget,
            |owner, launch, input, _, budget| {
                let result = ProductionPendingScopedSourceOwnerV29::try_materialize_with_budget(
                    owner,
                    launch,
                    input,
                    ProductionSemanticKirLimitsV1::default().with_storage_layout_limits(policy),
                    budget,
                );
                assert!(result.is_err(), "policy fault {fault}");
                drop(result);
                assert_eq!(budget.storage(), MODULE_FLOOR);
            },
        );
    }
}

#[test]
fn actual_candidate_transfers_only_owned_row_capacity_and_boxes_once() {
    use fe2o3_kernel_ir::{StorageFieldV1, StorageLayoutKindV1, StorageLayoutV1, StorageVariantV1};
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
            let (candidate, roots, row_storage) = with_scoped_source_cleanup_v29(
                budget,
                floor,
                |cleanup, budget| {
                    scoped_source_candidate_v29(
                        source,
                        ProductionSemanticKirLimitsV1::default(),
                        cleanup,
                        budget,
                    )
                },
            )
            .unwrap();
            let expected = candidate.storage_layouts.capacity() * size_of::<StorageLayoutV1>()
                + candidate
                    .storage_layouts
                    .iter()
                    .map(|row| match &row.kind {
                        StorageLayoutKindV1::Record(fields)
                        | StorageLayoutKindV1::Union(fields) => {
                            fields.len() * size_of::<StorageFieldV1>()
                        }
                        StorageLayoutKindV1::Variants { variants, .. } => {
                            variants.len() * size_of::<StorageVariantV1>()
                        }
                        _ => 0,
                    })
                    .sum::<usize>();
            assert_eq!(row_storage, expected);
            assert!(budget.storage() >= floor + expected);
            assert_eq!(roots.len(), source.launch.roots().len());
            drop((candidate, roots));
            budget.release_storage(budget.storage() - floor).unwrap();
        })
        .unwrap();
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}
