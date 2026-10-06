fn capability_storage_expected_headers() -> usize {
    fn h<T>() -> usize {
        std::mem::size_of::<T>()
            + std::mem::size_of::<Result<T, ArgumentResourceV1>>()
            + std::mem::size_of::<Result<T, ProductionSemanticKirErrorV1>>()
    }
    h::<CapabilityOriginCreditV29>()
        + h::<Option<CapabilityOriginCreditV29>>()
        + h::<&CapabilityOriginCreditV29>()
        + h::<Option<&CapabilityOriginCreditV29>>()
        + h::<CapabilityOriginStorageV29<'_>>()
        + h::<Option<CapabilityOriginStorageV29<'_>>>()
        + h::<(
            &mut SemanticCapabilityOriginResolverV1<'_>,
            &ExecutionAvailabilityV29<'_>,
            &mut dyn SemanticEmissionBudgetV1,
        )>()
        + h::<(
            &mut SemanticCapabilityOriginResolverV1<'_>,
            usize,
            &mut dyn SemanticEmissionBudgetV1,
        )>()
        + h::<(
            CapabilityOriginStorageV29<'_>,
            SemanticCapabilityOriginResolverV1<'_>,
            &mut dyn SemanticEmissionBudgetV1,
        )>()
        + h::<(&CapabilityOriginCreditV29, &dyn SemanticEmissionBudgetV1)>()
        + h::<(
            Option<&ExecutionAvailabilityV29<'_>>,
            &mut dyn SemanticEmissionBudgetV1,
        )>()
        + h::<(
            &EmissionServiceGrowthV1<'_>,
            &CapabilityOriginStorageV29<'_>,
            &CapabilityOriginCreditV29,
            &dyn SemanticEmissionBudgetV1,
        )>()
        + h::<&CapabilityOriginStorageV29<'_>>()
        + h::<(
            &dyn SemanticEmissionBudgetV1,
            Option<&SourceReferencePlanV29<'_, '_>>,
            usize,
            fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
            usize,
            usize,
        )>()
        + h::<(
            &mut dyn SemanticEmissionBudgetV1,
            Option<&SourceReferencePlanV29<'_, '_>>,
            usize,
            fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
            usize,
            usize,
        )>()
        + h::<&dyn SemanticEmissionBudgetV1>()
        + h::<Option<&SourceReferencePlanV29<'_, '_>>>()
        + h::<&SourceReferencePlanV29<'_, '_>>()
        + h::<Option<EmissionServiceGrowthV1<'_>>>()
        + h::<&EmissionServiceGrowthV1<'_>>()
        + h::<Option<&EmissionServiceGrowthV1<'_>>>()
        + h::<&source_storage_v29::SourceStorageRootCustodyViewV29<'_, '_>>()
        + h::<Option<&source_storage_v29::SourceStorageRootCustodyViewV29<'_, '_>>>()
        + h::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>()
        + h::<Option<usize>>()
        + h::<[usize; 6]>()
        + h::<&[usize]>()
        + h::<[usize; 2]>()
        + h::<bool>()
        + h::<()>()
}

// Accounting-only control. Production attaches this record only after the
// authentic execution cursor check, with its captured root-growth receipt.
fn capability_test_storage(
    resolver: &mut SemanticCapabilityOriginResolverV1<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<CapabilityOriginStorageV29<'static>, ProductionSemanticKirErrorV1> {
    assert!(resolver.scoped_storage.is_none());
    let slot = budget.emission_service_slot_v1().unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let entry = budget.storage();
    let bytes = capability_storage_expected_headers();
    budget.charge_work(12)?;
    budget.reserve_storage(bytes)?;
    resolver.scoped_storage = Some(CapabilityOriginCreditV29 {
        slot,
        ledger,
        entry,
        bytes,
    });
    Ok(CapabilityOriginStorageV29 {
        source: None,
        growth: None,
        slot,
        ledger,
        entry,
    })
}

#[test]
fn capability_origin_temporary_storage_has_independent_header_cuts() {
    assert_eq!(
        capability_origin_storage_headers_v29().unwrap(),
        capability_storage_expected_headers()
    );
    let (types, callables, function, dominance) = nested_component_capability_fixture_v1(0, false);
    let certified = (0..function.locals().len() as u32).collect();
    for exact in [false, true] {
        let mut resolver = SemanticCapabilityOriginResolverV1::new(
            &types,
            &callables,
            &function,
            &dominance,
            &certified,
            usize::MAX,
            usize::MAX,
        )
        .unwrap();
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(12);
        let required = 37 + capability_storage_expected_headers();
        let limit = required - usize::from(!exact);
        let mut budget = ArgumentBudgetV1::new(&mut work, limit);
        budget.reserve_storage(37).unwrap();
        match capability_test_storage(&mut resolver, &mut budget) {
            Ok(storage) => {
                assert!(exact);
                assert_eq!(budget.storage(), required);
                storage.finish(resolver, &mut budget).unwrap();
                assert_eq!(budget.storage(), 37);
            }
            Err(error) => {
                assert!(!exact);
                assert!(
                    matches!(error, ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Storage(error)) if error.actual() == required && error.limit() == limit)
                );
                assert!(resolver.scoped_storage.is_none());
                assert_eq!(budget.storage(), 37);
            }
        }
        assert_eq!(budget.work(), 12);
    }
}

#[test]
fn capability_origin_temporary_storage_preserves_results_and_interleaved_retained_credit() {
    let (types, callables, function, dominance) = nested_component_capability_fixture_v1(0, false);
    let certified = (0..function.locals().len() as u32).collect();
    let make = || {
        SemanticCapabilityOriginResolverV1::new(
            &types,
            &callables,
            &function,
            &dominance,
            &certified,
            usize::MAX,
            usize::MAX,
        )
        .unwrap()
    };
    let expected = make().resolve(SemanticLocalIdV1::from_index(8)).unwrap();
    assert!(expected.is_some());
    for queries in [1usize, 4, 16] {
        let mut resolver = make();
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1_000_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 2_000_000);
        budget.reserve_storage(37).unwrap();
        let storage = capability_test_storage(&mut resolver, &mut budget).unwrap();
        for _ in 0..queries {
            assert_eq!(
                resolver
                    .resolve_with_budget_v29(
                        SemanticLocalIdV1::from_index(8),
                        &mut Some(&mut budget)
                    )
                    .unwrap(),
                expected
            );
            // This separately retained credit is not part of the resolver.
            budget.reserve_storage(73).unwrap();
        }
        assert!(!resolver.memo.is_empty());
        assert!(resolver.visiting.is_empty());
        let own = resolver.scoped_storage.unwrap().bytes;
        assert!(own > capability_storage_expected_headers());
        assert_eq!(budget.storage(), 37 + 73 * queries + own);
        let before_work = budget.work();
        storage.finish(resolver, &mut budget).unwrap();
        assert_eq!(budget.storage(), 37 + 73 * queries);
        assert_eq!(budget.work(), before_work, "settlement uses prepaid work");
    }
}

#[test]
fn capability_origin_temporary_storage_refuses_foreign_custody_and_higher_floor_loss() {
    let (types, callables, function, dominance) = nested_component_capability_fixture_v1(0, false);
    let certified = (0..function.locals().len() as u32).collect();
    for fault in 0..4 {
        let mut resolver = SemanticCapabilityOriginResolverV1::new(
            &types,
            &callables,
            &function,
            &dominance,
            &certified,
            usize::MAX,
            usize::MAX,
        )
        .unwrap();
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1_000_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 2_000_000);
        budget.reserve_storage(37).unwrap();
        let mut storage = capability_test_storage(&mut resolver, &mut budget).unwrap();
        resolver
            .resolve_with_budget_v29(SemanticLocalIdV1::from_index(8), &mut Some(&mut budget))
            .unwrap();
        match fault {
            0 => storage.slot = storage.slot.checked_add(1).unwrap(),
            1 => {
                let mut foreign_work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1);
                let foreign = ArgumentBudgetV1::new(&mut foreign_work, 1);
                storage.ledger = foreign.work_ledger_identity_v1();
            }
            2 => budget.release_storage(1).unwrap(),
            3 => storage.entry = storage.entry.checked_add(1).unwrap(),
            _ => unreachable!(),
        }
        let before = (budget.work(), budget.storage());
        assert!(matches!(
            storage.finish(resolver, &mut budget),
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting
                )
            )
        ));
        assert_eq!(
            (budget.work(), budget.storage()),
            before,
            "denied refund releases no credit"
        );
    }
}

#[test]
fn capability_origin_temporary_storage_keeps_exact_query_resource_failures() {
    let (types, callables, function, dominance) = nested_component_capability_fixture_v1(0, false);
    let certified = (0..function.locals().len() as u32).collect();
    let run = |work_limit, storage_limit| {
        let mut resolver = SemanticCapabilityOriginResolverV1::new(
            &types,
            &callables,
            &function,
            &dominance,
            &certified,
            usize::MAX,
            usize::MAX,
        )
        .unwrap();
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(37).unwrap();
        let storage = capability_test_storage(&mut resolver, &mut budget).unwrap();
        let result = resolver
            .resolve_with_budget_v29(SemanticLocalIdV1::from_index(8), &mut Some(&mut budget));
        assert!(resolver.visiting.is_empty());
        let observed = (
            budget.work(),
            budget.peak_storage(),
            budget.failed_work(),
            budget.failed_storage(),
        );
        // Preserve the selected query result independently of successful
        // destruction/own-credit settlement; cleanup performs no new work.
        storage.finish(resolver, &mut budget).unwrap();
        assert_eq!(budget.storage(), 37);
        (result, observed)
    };
    let (result, (work, peak, _, _)) = run(1_000_000, 2_000_000);
    let expected = result.unwrap();
    assert!(expected.is_some());
    let (exact, observed) = run(work, peak);
    assert_eq!(exact.unwrap(), expected);
    assert_eq!((observed.0, observed.1), (work, peak));
    let (short, observed) = run(work - 1, peak);
    assert!(
        matches!(short, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
        ArgumentResourceV1::Work(error))) if error.actual() == observed.2.unwrap() && error.limit() == work - 1)
    );
    let (short, observed) = run(work, peak - 1);
    assert!(
        matches!(short, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
        ArgumentResourceV1::Storage(error))) if error.actual() == peak && error.actual() == observed.3.unwrap() && error.limit() == peak - 1)
    );
}
