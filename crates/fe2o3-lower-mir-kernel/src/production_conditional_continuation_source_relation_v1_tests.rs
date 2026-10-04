#[test]
fn conditional_source_relation_replayed_inside_each_original_owned_callback() {
    use fe2o3_kernel_ir::CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Owned;
    let source = materialize(Fixture::default());
    let selected = source
        .semantic_ssa()
        .source_semantic()
        .select_kernel_body_for_root_v1(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    assert_ne!(selected.root(), selected.body());
    let mut ledger = Owned::new(CanonicalKernelIrWorkBudgetV1::new(WORK), STORAGE);
    let floor = source.retained_analysis_storage_v1() + FLOOR;
    let account = ledger.with_budget(|budget| {
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(7).unwrap();
        budget.work_ledger_identity_v1()
    });
    for _ in 0..3 {
        let before = ledger.work();
        ledger.with_budget(|budget| {
            let relation =
                conditional_source_relation_v1(&source, selected.root().index(), budget).unwrap();
            assert_eq!(
                relation.association().correspondence_owner(),
                selected.root()
            );
            assert_eq!(relation.association().semantic_function(), selected.body());
            relation
                .require_source_owner_v1(source.semantic_ssa(), budget)
                .unwrap();
            relation
                .replay_v1(
                    relation.canonical_module(),
                    relation.canonical_function(),
                    budget,
                )
                .unwrap();
            assert!(budget.work_ledger_identity_v1() == account);
        });
        assert_eq!(ledger.storage(), floor);
        assert!(ledger.work() > before);
    }
}

#[test]
fn conditional_source_relation_refuses_selected_body_as_root_and_equal_owner_digest() {
    let source = materialize(Fixture::default());
    let other = materialize(Fixture::default());
    assert_eq!(
        source.semantic_ssa().source_semantic_sha256(),
        other.semantic_ssa().source_semantic_sha256()
    );
    let selected = source
        .semantic_ssa()
        .source_semantic()
        .select_kernel_body_for_root_v1(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    let floor = source.retained_analysis_storage_v1() + FLOOR;
    budget.reserve_storage(floor).unwrap();
    assert!(matches!(
        conditional_source_relation_v1(&source, selected.body().index(), &mut budget),
        Err(ProductionConditionalContinuationErrorV1::Subject(
            "source root association"
        ))
    ));
    let relation =
        conditional_source_relation_v1(&source, selected.root().index(), &mut budget).unwrap();
    assert!(matches!(
        relation.require_source_owner_v1(other.semantic_ssa(), &mut budget),
        Err(fe2o3_pliron::ProductionSourceArgumentErrorV1::CorrespondenceMismatch)
    ));
    assert_eq!(budget.storage(), floor);
}

#[test]
fn conditional_source_relation_rejects_work_reset_inside_owned_original_guard() {
    use fe2o3_kernel_ir::{
        CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Owned,
        CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    };
    let source = materialize(Fixture::default());
    let mut ledger = Owned::new(CanonicalKernelIrWorkBudgetV1::new(WORK), STORAGE);
    let floor = source.retained_analysis_storage_v1() + FLOOR;
    ledger
        .with_budget(|budget| budget.reserve_storage(floor))
        .unwrap();
    let mut root = accounting_root(&source, &mut ledger);
    let result = visit_accounting_root(&mut root, |budget| {
        let relation = conditional_source_relation_v1(&source, 0, budget)?;
        let protected = budget.storage();
        *budget = AssertOriginBudgetV1::new(
            Box::leak(Box::new(CanonicalKernelIrWorkBudgetV1::new(WORK))),
            STORAGE,
        );
        budget.reserve_storage(protected)?;
        assert!(matches!(
            relation.replay_v1(
                relation.canonical_module(),
                relation.canonical_function(),
                budget
            ),
            Err(
                fe2o3_pliron::ProductionSourceArgumentErrorV1::ArgumentCorrespondenceResource(
                    Resource::Accounting
                )
            )
        ));
        Ok(())
    });
    assert!(matches!(
        result,
        Err(ProductionConditionalContinuationErrorV1::Resource(
            Resource::Accounting
        ))
    ));
    assert!(root.poisoned);
}

#[test]
fn original_guard_rejects_fresh_relation_checked_after_budget_replacement() {
    use fe2o3_kernel_ir::{
        CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Owned,
        CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    };
    let source = materialize(Fixture::default());
    let mut ledger = Owned::new(CanonicalKernelIrWorkBudgetV1::new(WORK), STORAGE);
    let floor = source.retained_analysis_storage_v1() + FLOOR;
    ledger
        .with_budget(|budget| budget.reserve_storage(floor))
        .unwrap();
    let mut root = accounting_root(&source, &mut ledger);
    let result = visit_accounting_root(&mut root, |budget| {
        let protected = budget.storage();
        *budget = AssertOriginBudgetV1::new(
            Box::leak(Box::new(CanonicalKernelIrWorkBudgetV1::new(WORK))),
            STORAGE,
        );
        budget.reserve_storage(protected)?;
        // This relation is genuinely checked on the replacement meter. Source
        // replay succeeds, but cannot authorize replacement of the root account.
        let fresh = conditional_source_relation_v1(&source, 0, budget)?;
        fresh
            .replay_v1(fresh.canonical_module(), fresh.canonical_function(), budget)
            .unwrap();
        Ok(())
    });
    assert!(matches!(
        result,
        Err(ProductionConditionalContinuationErrorV1::Resource(
            Resource::Accounting
        ))
    ));
    assert!(root.poisoned);
    assert!(
        visit_accounting_root(&mut root, |_| -> Result<(), _> {
            panic!("a freshly authenticated relation cannot repair the original guard")
        })
        .is_err()
    );
}
