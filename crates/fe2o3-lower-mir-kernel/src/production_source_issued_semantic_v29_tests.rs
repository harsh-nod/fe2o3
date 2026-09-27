// These are original-source query tests. Volatile source admission and physical
// effect replay remain separate, closed boundaries in this factoring packet.
fn run_issued_semantic_v29(
    volatility: SemanticVolatilityV1,
    work_limit: usize,
    storage_limit: usize,
    consume: impl FnOnce(&ExecutionInstancesV29<'_>, &mut ArgumentBudgetV1<'_>)
        -> Result<(), ProductionSemanticKirErrorV1>,
) -> (Result<(), ProductionSemanticKirErrorV1>, usize, usize) {
    let owner = owner_with_shape_and_effects(1, 1, true, Some(volatility));
    run_issued_semantic_owner_v29(owner, work_limit, storage_limit, consume)
}

fn run_issued_semantic_owner_v29(
    owner: ProductionSemanticSsaOwnerV1, work_limit: usize, storage_limit: usize,
    consume: impl FnOnce(&ExecutionInstancesV29<'_>, &mut ArgumentBudgetV1<'_>)
        -> Result<(), ProductionSemanticKirErrorV1>,
) -> (Result<(), ProductionSemanticKirErrorV1>, usize, usize) {
    let retained = owner.occurrence_storage().unwrap().retained_storage();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    let result = (|| {
        budget.reserve_storage(37 + retained)?;
        let nested = production_call_instances_v1::with_production_call_instances_v1(
            &owner, ROOT, &mut budget, |instances, budget| {
                Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(
                    with_canonical_call_scratch_v1(budget, |budget| consume(instances, budget)))
            },
        ).map_err(|error| match error {
            production_call_instances_v1::ProductionCallInstanceErrorV1::Resource(error) => error.into(),
            _ => source_issued_error_v29(),
        })?;
        nested?;
        assert_eq!(budget.storage(), 37 + retained);
        budget.release_storage(retained)?;
        assert_eq!(budget.storage(), 37);
        Ok(())
    })();
    (result, budget.work(), budget.peak_storage())
}

fn issued_semantic_sites_v29(function: &SemanticFunctionDeclV1)
    -> [(ExecutionSiteV29, ExecutionOperandV29, &SemanticPlaceV1); 2]
{
    let SemanticStatementKindV1::Assign(read) = function.blocks()[3].statements()[1].kind() else { panic!("original Load"); };
    let SemanticRvalueKindV1::Load(read) = read.value().kind() else { panic!("original Load"); };
    let SemanticStatementKindV1::Store(write) = function.blocks()[3].statements()[2].kind() else { panic!("original Store"); };
    [
        (ExecutionSiteV29::Statement { block: SsaBlockIdV1::new(3), statement: 1 }, ExecutionOperandV29::RvaluePlace, read.source()),
        (ExecutionSiteV29::Statement { block: SsaBlockIdV1::new(3), statement: 2 }, ExecutionOperandV29::StoreDestination, write.destination()),
    ]
}

fn check_issued_semantic_effects_v29(
    instances: &ExecutionInstancesV29<'_>, budget: &mut ArgumentBudgetV1<'_>, volatile: bool,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let instance = instances.root();
    let function = instances.instance(instance).unwrap().declaration();
    let mut resolver = SourceIssuedSemanticV29::new(instances, instance, SourceIssuedReplayModeV29::SourceOnly, budget)?;
    let mut issuer = None;
    for (index, (site, role, place)) in issued_semantic_sites_v29(function).into_iter().enumerate() {
        let value = resolver.use_value(site, role, place, budget)?;
        let source = resolver.resolve(value, budget)?.expect("genuine original issued pointer");
        assert_eq!(source.form, SourceIssuedFormV29::Pointer);
        assert_eq!((source.pointer_type, source.element, source.access), (REFERENCE, ScalarType::U32, AccessMode::ReadWrite));
        assert_eq!(source.block, SemanticBlockIdV1::from_index(1));
        if let Some(previous) = issuer.replace(source.issuer) { assert_eq!(previous, source.issuer); }
        let effect = resolver.effect(site, role, place, budget)?.expect("exact explicit original effect");
        assert_eq!(effect, SourceIssuedOriginalEffectV29 { recipe: source, writing: index == 1, volatile });
        let floor = budget.storage();
        for _ in 0..16 {
            assert_eq!(resolver.effect(site, role, place, budget)?, Some(effect));
            assert_eq!(budget.storage(), floor, "memoized source query has no new storage");
        }
    }
    drop(resolver);
    Ok(())
}

#[test]
fn issued_pointer_semantic_resolver_derives_nonvolatile_and_volatile_original_load_store() {
    // Existing complete producer tests still prove the physical nonvolatile
    // path; these two cases isolate the original semantic resolver itself.
    for volatility in [SemanticVolatilityV1::NonVolatile, SemanticVolatilityV1::Volatile] {
        run_issued_semantic_v29(volatility, 1_000_000_000, 1_000_000_000,
            |instances, budget| check_issued_semantic_effects_v29(instances, budget,
                volatility == SemanticVolatilityV1::Volatile)).0.unwrap();
    }
}

#[test]
fn issued_pointer_semantic_queries_reject_cloned_foreign_wrong_site_and_role() {
    run_issued_semantic_v29(SemanticVolatilityV1::Volatile, 1_000_000_000, 1_000_000_000,
        |instances, budget| {
            let instance = instances.root();
            let function = instances.instance(instance).unwrap().declaration();
            let mut resolver = SourceIssuedSemanticV29::new(instances, instance, SourceIssuedReplayModeV29::SourceOnly, budget)?;
            let sites = issued_semantic_sites_v29(function);
            let foreign = owner_with_shape_and_effects(1, 1, true, Some(SemanticVolatilityV1::Volatile));
            let foreign_sites = issued_semantic_sites_v29(&foreign.source_semantic().functions()[0]);
            for (ordinal, (site, role, place)) in sites.into_iter().enumerate() {
                let positive = resolver.effect(site, role, place, budget)?.unwrap();
                let copied = place.clone();
                let refused = resolver.effect(site, role, &copied, budget);
                assert!(matches!(refused, Err(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "source issued pointer differs from its original issuer or actual guard", ..
                })), "copied source locator: {refused:?}");
                let refused = resolver.effect(site, role, foreign_sites[ordinal].2, budget);
                assert!(matches!(refused, Err(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "source issued pointer differs from its original issuer or actual guard", ..
                })), "different original owner at equal coordinates: {refused:?}");
                let wrong_role = if role == ExecutionOperandV29::RvaluePlace {
                    ExecutionOperandV29::StoreDestination
                } else { ExecutionOperandV29::RvaluePlace };
                assert_eq!(resolver.effect(site, wrong_role, place, budget)?, None);
                assert_eq!(resolver.effect(ExecutionSiteV29::Statement {
                    block: SsaBlockIdV1::new(0), statement: 0,
                }, role, place, budget)?, None);
                assert_eq!(resolver.effect(site, role, place, budget)?, Some(positive));
            }
            let (site, role, place) = sites[0];
            let mut other_work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
            let mut other = ArgumentBudgetV1::new(&mut other_work, 1_000_000);
            let before = (other.work(), other.storage());
            assert!(matches!(resolver.effect(site, role, place, &mut other),
                Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Accounting))));
            assert_eq!((other.work(), other.storage()), before);
            let authentic = (budget.work(), budget.storage());
            assert!(matches!(resolver.effect(site, role, place, budget),
                Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Accounting))));
            assert_eq!((budget.work(), budget.storage()), authentic,
                "foreign custody refusal remains first and consumes no authentic credit");
            drop(resolver);
            Ok(())
        }).0.unwrap();
}

#[test]
fn issued_pointer_semantic_resource_failure_stays_first_after_completed_or_partial_memo() {
    const LIMIT: usize = 1_000_000_000;
    for fault in 0..3 {
        let completed = std::cell::Cell::new(false);
        let expected = std::cell::Cell::new(None);
        let result = run_issued_semantic_v29(SemanticVolatilityV1::Volatile, LIMIT, LIMIT,
            |instances, budget| {
                let instance = instances.root();
                let function = instances.instance(instance).unwrap().declaration();
                let (site, role, place) = issued_semantic_sites_v29(function)[0];
                let mut resolver = SourceIssuedSemanticV29::new(instances, instance,
                    SourceIssuedReplayModeV29::SourceOnly, budget)?;
                let value = resolver.use_value(site, role, place, budget)?;
                if fault == 0 {
                    assert!(resolver.effect(site, role, place, budget)?.is_some());
                }
                let before_owned = resolver.owned;
                let before_storage = budget.storage();
                let padding = if fault == 1 { LIMIT - budget.storage() } else { 0 };
                budget.reserve_storage(padding)?;
                let allowance = if fault == 0 { 15 } else { 64 };
                if fault != 1 {
                    budget.charge_work(LIMIT - budget.work() - allowance)?;
                }
                let error = if fault == 0 {
                    resolver.effect(site, role, place, budget).unwrap_err()
                } else {
                    resolver.resolve(value, budget).unwrap_err()
                };
                let ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(first) = error else {
                    panic!("first resolver resource failure: {error:?}")
                };
                match (fault, first) {
                    (1, ArgumentResourceV1::Storage(error)) => {
                        assert_eq!(error.limit(), LIMIT);
                        assert!(error.actual() > LIMIT);
                    }
                    (0 | 2, ArgumentResourceV1::Work(error)) => {
                        assert_eq!(error.limit(), LIMIT);
                        assert!(error.actual() > LIMIT);
                    }
                    other => panic!("exact resolver denial family: {other:?}"),
                }
                let accepted = if fault == 2 {
                    // Two fixed 32-work empty-map lookups precede one accepted
                    // split-path reserve; the definition lookup then denies.
                    2 * 32 * std::mem::size_of::<(SsaValueV1, SourceIssuedMemoV29, usize)>()
                } else { 0 };
                assert_eq!(resolver.owned, before_owned + accepted);
                assert_eq!(budget.storage(), before_storage + padding + accepted);
                if fault == 2 {
                    assert!(matches!(resolver.memo.get(&value), Some(SourceIssuedMemoV29::Visiting)));
                }
                budget.release_storage(padding)?;
                // A later work exhaustion or custody mismatch must not replace
                // the exact first denial, even for a smaller memoized query.
                budget.charge_work(LIMIT - budget.work())?;
                let before = (budget.work(), budget.storage());
                let replay = resolver.use_value(site, role, place, budget).unwrap_err();
                assert!(matches!(replay,
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error) if error == first));
                assert!(matches!(resolver.resolve(value, budget),
                    Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error)) if error == first));
                assert_eq!((budget.work(), budget.storage()), before);
                let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(0);
                let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, 0);
                assert!(matches!(resolver.effect(site, role, place, &mut foreign),
                    Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error)) if error == first));
                assert_eq!((foreign.work(), foreign.storage()), (0, 0));
                expected.set(Some(first));
                completed.set(true);
                drop(resolver);
                Err(first.into())
            }).0;
        assert!(completed.get(), "all post-denial checks must finish for fault {fault}");
        assert!(matches!(result,
            Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error)) if Some(error) == expected.get()),
            "outer original scope must retain the selected resource failure: {result:?}");
    }
}

#[test]
fn issued_pointer_semantic_scope_exact_and_one_short_resources_preserve_source_queries() {
    let run = |work, storage| run_issued_semantic_v29(SemanticVolatilityV1::Volatile, work, storage,
        |instances, budget| check_issued_semantic_effects_v29(instances, budget, true));
    let (positive, work, storage) = run(1_000_000_000, 1_000_000_000);
    positive.unwrap();
    let exact = run(work, storage);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2), (work, storage));
    for (work_limit, storage_limit, is_work) in [(work - 1, storage, true), (work, storage - 1, false)] {
        let error = run(work_limit, storage_limit).0.unwrap_err();
        match (is_work, error) {
            (true, ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(error))) => {
                assert_eq!(error.limit(), work_limit);
                assert!(error.actual() > work_limit);
            }
            (false, ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Storage(error))) => {
                assert_eq!(error.limit(), storage_limit);
                assert!(error.actual() > storage_limit);
            }
            other => panic!("semantic source exact boundary: {other:?}"),
        }
    }
}

#[test]
fn issued_pointer_semantic_original_none_edge_cannot_issue_a_some_payload() {
    run_issued_semantic_v29(SemanticVolatilityV1::Volatile, 1_000_000_000, 1_000_000_000,
        |instances, budget| check_issued_semantic_effects_v29(instances, budget, true)).0.unwrap();
    let owner = owner_with_shape_effects_and_guard(1, 1, true, Some(SemanticVolatilityV1::Volatile), false);
    let completed = std::cell::Cell::new(false);
    let refused = run_issued_semantic_owner_v29(owner, 1_000_000_000, 1_000_000_000, |instances, budget| {
        let instance = instances.root();
        let floor = budget.storage();
        let refused: Result<(), ProductionSemanticKirErrorV1> = with_canonical_call_scratch_v1(budget, |budget| {
            // Whole-function Option dominance rejects the original None edge
            // before any per-access effect query can obtain a resolver.
            match SourceIssuedSemanticV29::new(instances, instance, SourceIssuedReplayModeV29::SourceOnly, budget) {
                Err(error) => Err(error),
                Ok(resolver) => { drop(resolver); panic!("original None edge admitted a resolver"); }
            }
        });
        assert_eq!(budget.storage(), floor, "refused resolver releases its partial scratch");
        completed.set(true);
        refused
    }).0;
    assert!(completed.get(), "must reach original option-dominance refusal");
    assert!(matches!(refused, Err(ProductionSemanticKirErrorV1::Unsupported {
        function: 0, block: None, statement: None,
        detail: "source issued pointer differs from its original issuer or actual guard",
    })), "original None edge: {refused:?}");
}

#[test]
fn issued_pointer_semantic_atomic_effects_stay_outside_the_original_volatile_selector() {
    let owner = owner_with_shape_effects_guard_and_atomic(1, 1, true,
        Some(SemanticVolatilityV1::NonVolatile), true, true);
    let completed = std::cell::Cell::new(false);
    run_issued_semantic_owner_v29(owner, 1_000_000_000, 1_000_000_000, |instances, budget| {
        let instance = instances.root();
        let function = instances.instance(instance).unwrap().declaration();
        let mut resolver = SourceIssuedSemanticV29::new(instances, instance, SourceIssuedReplayModeV29::SourceOnly, budget)?;
        for (site, role, place) in issued_semantic_sites_v29(function) {
            // The exact issuer is supported; atomic effect classification is
            // independently absent, not an unsupported-issuer coincidence.
            let value = resolver.use_value(site, role, place, budget)?;
            assert!(resolver.resolve(value, budget)?.is_some());
            assert_eq!(resolver.effect(site, role, place, budget)?, None);
        }
        completed.set(true);
        drop(resolver);
        Ok(())
    }).0.unwrap();
    assert!(completed.get());
}

thread_local! {
    static ISSUED_SEMANTIC_REPLAY_OBSERVE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static ISSUED_SEMANTIC_REPLAY_COMPLETE: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn observe_issued_semantic_replay_v29(
    pending: &PendingScopedRootEmissionV29, instances: &ExecutionInstancesV29<'_>,
    references: &SourceReferencePlanV29<'_, '_>, budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    references.check_owner(instances, budget)?;
    with_canonical_call_scratch_v1(budget, |budget| {
        let index = SourceAddressSourceIndexV29::new(instances, pending, budget)?;
        let instance = instances.root();
        let function = instances.instance(instance).unwrap().declaration();
        let mut original = SourceIssuedOriginalV29::new(instances, instance, &index, budget)?;
        let mut source = SourceIssuedSemanticV29::new(instances, instance, SourceIssuedReplayModeV29::SourceOnly, budget)?;
        for (site, role, place) in issued_semantic_sites_v29(function) {
            let value = original.use_value(site, role, place, budget)?;
            let semantic = source.resolve(value, budget)?.unwrap();
            let physical = original.resolve(value, budget)?.unwrap();
            assert_eq!((physical.issuer, physical.block, physical.pointer_type, physical.form),
                (semantic.issuer, semantic.block, semantic.pointer_type, semantic.form));
            original.check_archive(value, physical, budget)?;
            // Copied expected-component controls, not mutations of the actual
            // retained archive or claims of downstream native authority.
            for fault in 0..3 {
                let mut wrong = physical;
                match fault {
                    0 => wrong.element = ScalarType::F32,
                    1 => wrong.access = AccessMode::ReadOnly,
                    _ => wrong.pointer = physical.present,
                }
                let error = original.check_archive(value, wrong, budget).unwrap_err();
                assert!(matches!(error, ProductionSemanticKirErrorV1::Unsupported {
                    detail: "source issued pointer differs from its original issuer or actual guard", ..
                }), "copied archive expectation {fault}: {error:?}");
            }
            // Even a memo from the same authentic source cannot substitute for
            // the mandatory emitted replay mode.
            let emitted = std::mem::replace(&mut original.semantic, source);
            let error = original.resolve(value, budget).err().expect("source-only memo must not yield physical recipe");
            assert!(matches!(error, ProductionSemanticKirErrorV1::Unsupported {
                detail: "source issued pointer differs from its original issuer or actual guard", ..
            }));
            source = std::mem::replace(&mut original.semantic, emitted);
            let restored = original.resolve(value, budget)?.unwrap();
            original.check_archive(value, restored, budget)?;
        }
        drop((source, original));
        drop(index);
        Ok(())
    })?;
    ISSUED_SEMANTIC_REPLAY_COMPLETE.set(ISSUED_SEMANTIC_REPLAY_COMPLETE.get() + 1);
    Ok(())
}

#[test]
fn issued_pointer_semantic_memo_never_replaces_complete_emitted_archive_replay() {
    struct Restore(bool, usize);
    impl Drop for Restore {
        fn drop(&mut self) {
            ISSUED_SEMANTIC_REPLAY_OBSERVE.set(self.0);
            ISSUED_SEMANTIC_REPLAY_COMPLETE.set(self.1);
        }
    }
    let _restore = Restore(ISSUED_SEMANTIC_REPLAY_OBSERVE.replace(true), ISSUED_SEMANTIC_REPLAY_COMPLETE.replace(0));
    run_original_owner(0, true, true,
        owner_with_shape_and_effects(1, 1, true, Some(SemanticVolatilityV1::NonVolatile)));
    assert!(ISSUED_SEMANTIC_REPLAY_COMPLETE.get() > 0, "same original candidate must finish source and emitted queries");
}

fn issued_ordered_selection_v29(instances: &ExecutionInstancesV29<'_>, budget: &mut ArgumentBudgetV1<'_>, expected: bool)
    -> Result<(), ProductionSemanticKirErrorV1>
{
    let instance = instances.root();
    let function = instances.instance(instance).unwrap().declaration();
    let floor = budget.storage();
    for (execution, role, place) in issued_semantic_sites_v29(function) {
        let ExecutionSiteV29::Statement { block, statement } = execution else { unreachable!(); };
        let site = SourceReferenceSiteV29 { instance, block: SemanticBlockIdV1::from_index(block.get()),
            statement: Some(statement as usize) };
        for _ in 0..3 {
            assert_eq!(source_ordered_issued_effect_v29(instances, site, place, role, budget)?, expected);
            assert_eq!(budget.storage(), floor, "no resolver or memo escapes the lexical selector");
        }
    }
    Ok(())
}

#[test]
fn issued_pointer_ordered_selector_uses_exact_source_and_releases_every_query() {
    for volatility in [SemanticVolatilityV1::NonVolatile, SemanticVolatilityV1::Volatile] {
        run_issued_semantic_v29(volatility, 1_000_000_000, 1_000_000_000, |instances, budget|
            issued_ordered_selection_v29(instances, budget, volatility == SemanticVolatilityV1::Volatile)).0.unwrap();
    }
}

#[test]
fn issued_pointer_ordered_selector_has_exact_work_storage_boundaries() {
    let run = |work, storage| run_issued_semantic_v29(SemanticVolatilityV1::Volatile, work, storage,
        |instances, budget| issued_ordered_selection_v29(instances, budget, true));
    let (positive, work, storage) = run(1_000_000_000, 1_000_000_000);
    positive.unwrap();
    let exact = run(work, storage);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2), (work, storage));
    assert!(matches!(run(work - 1, storage).0,
        Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(error)))
            if error.limit() == work - 1 && error.actual() > work - 1));
    assert!(matches!(run(work, storage - 1).0,
        Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Storage(error)))
            if error.limit() == storage - 1 && error.actual() > storage - 1));
}

#[test]
fn issued_pointer_ordered_selector_refuses_cloned_roles_atomic_and_original_none_edge() {
    run_issued_semantic_v29(SemanticVolatilityV1::Volatile, 1_000_000_000, 1_000_000_000, |instances, budget| {
        issued_ordered_selection_v29(instances, budget, true)?;
        let instance = instances.root();
        let function = instances.instance(instance).unwrap().declaration();
        for (execution, role, place) in issued_semantic_sites_v29(function) {
            let ExecutionSiteV29::Statement { block, statement } = execution else { unreachable!(); };
            let site = SourceReferenceSiteV29 { instance, block: SemanticBlockIdV1::from_index(block.get()),
                statement: Some(statement as usize) };
            let floor = budget.storage();
            assert!(!source_ordered_issued_effect_v29(instances, site, &place.clone(), role, budget)?);
            let wrong = if role == ExecutionOperandV29::RvaluePlace { ExecutionOperandV29::StoreDestination }
                else { ExecutionOperandV29::RvaluePlace };
            assert!(!source_ordered_issued_effect_v29(instances, site, place, wrong, budget)?);
            assert!(!source_ordered_issued_effect_v29(instances, SourceReferenceSiteV29 { statement: None, ..site },
                place, role, budget)?);
            assert_eq!(budget.storage(), floor);
        }
        Ok(())
    }).0.unwrap();
    let atomic = owner_with_shape_effects_guard_and_atomic(1, 1, true,
        Some(SemanticVolatilityV1::NonVolatile), true, true);
    run_issued_semantic_owner_v29(atomic, 1_000_000_000, 1_000_000_000,
        |instances, budget| issued_ordered_selection_v29(instances, budget, false)).0.unwrap();
    let unavailable = owner_with_shape_effects_and_guard(1, 1, true, Some(SemanticVolatilityV1::Volatile), false);
    let result = run_issued_semantic_owner_v29(unavailable, 1_000_000_000, 1_000_000_000,
        |instances, budget| issued_ordered_selection_v29(instances, budget, true)).0;
    assert!(matches!(result, Err(ProductionSemanticKirErrorV1::Unsupported {
        detail: "source issued pointer differs from its original issuer or actual guard", .. })), "{result:?}");
}

#[test]
fn issued_pointer_ordered_original_source_and_actual_added_dropped_effects() {
    for volatility in [SemanticVolatilityV1::NonVolatile, SemanticVolatilityV1::Volatile] {
        for fault in [0, 7, 8] {
            run_original_owner(0, true, true, owner_with_shape_and_effects(1, 1, true, Some(volatility)));
            run_original_owner(fault, true, true, owner_with_shape_and_effects(1, 1, true, Some(volatility)));
        }
    }
}
