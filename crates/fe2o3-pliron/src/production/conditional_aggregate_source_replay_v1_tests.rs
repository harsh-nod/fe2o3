use super::*;

#[test]
fn aggregate_rejects_same_bytes_different_canonical_owner_before_pipeline() {
    let module = canonical(true);
    let other_module = canonical(true);
    let source = SourceFixture::new(true);
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let relation = source.relation(&other_module, &mut budget);
    let pending = pending(true, false);
    budget
        .reserve_storage(pending.retained_analysis_storage_v1())
        .unwrap();
    let canonical = facts(&module, &mut budget);
    assert!(matches!(
        pending.verify_conditional_final_graph_v1(
            canonical,
            proposal(true),
            &relation,
            subjects(),
            &mut budget,
        ),
        Err(ProductionConditionalAggregateErrorV1::Source(
            crate::ProductionSourceArgumentErrorV1::CorrespondenceMismatch,
        ))
    ));
}

#[test]
fn aggregate_replay_requires_exact_source_owner_and_association_not_sha() {
    let module = canonical(false);
    let source = SourceFixture::new(false);
    let replacement = SourceFixture::new(false);
    assert_eq!(
        source.owner.source_semantic_sha256(),
        replacement.owner.source_semantic_sha256()
    );
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let relation = source.relation(&module, &mut budget);
    let state = aggregate(&module, &relation, false, &mut budget);
    let replacement_relation = replacement.relation(&module, &mut budget);
    assert!(matches!(
        state.with_input_v1(&replacement_relation, &mut budget, |_, _| panic!(
            "same digest does not authorize a different source owner"
        )),
        Err(ProductionConditionalAggregateErrorV1::Source(
            crate::ProductionSourceArgumentErrorV1::CorrespondenceMismatch,
        ))
    ));
    let association = source.association.clone();
    let replacement_relation = source.relation_with_association(&module, &association, &mut budget);
    assert!(matches!(
        state.with_input_v1(&replacement_relation, &mut budget, |_, _| panic!(
            "same root coordinates do not replace the retained association"
        )),
        Err(ProductionConditionalAggregateErrorV1::Subject(
            "conditional source/root owner substitution"
        ))
    ));
    state
        .with_input_v1(&relation, &mut budget, |input, budget| {
            input.require_current_graph_v1(budget)
        })
        .unwrap()
        .unwrap();
}

#[test]
fn aggregate_relation_does_not_admit_a_foreign_replay_budget() {
    let module = canonical(false);
    let source = SourceFixture::new(false);
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let relation = source.relation(&module, &mut budget);
    let state = aggregate(&module, &relation, false, &mut budget);
    let floor = budget.storage();
    let mut foreign_work = Work::new(usize::MAX);
    let mut foreign = Budget::new(&mut foreign_work, usize::MAX);
    foreign.reserve_storage(floor).unwrap();
    assert!(matches!(
        state.with_input_v1(&relation, &mut foreign, |_, _| panic!(
            "replay must use the relation's active ledger"
        )),
        Err(ProductionConditionalAggregateErrorV1::Resource(
            Resource::Accounting
        ))
    ));
    assert_eq!(budget.storage(), floor);
    assert_eq!(foreign.storage(), floor);
}

#[test]
fn bare_aggregate_replay_does_not_attest_original_work_account_continuity() {
    use kir::CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Owned;
    let module = canonical(false);
    let source = SourceFixture::new(false);
    let mut original = Owned::new(Work::new(usize::MAX), usize::MAX);
    let state = original.with_budget(|budget| {
        let relation = source.relation(&module, budget);
        aggregate(&module, &relation, false, budget)
    });
    let floor = original.storage();
    original.with_budget(|budget| budget.charge_work(usize::MAX - budget.work()).unwrap());

    // The public graph retains owners and checked facts, not the original meter.
    // This is not a production request or authority to reset a continuation.
    let mut replacement = Owned::new(Work::new(usize::MAX), usize::MAX);
    replacement.with_budget(|budget| {
        budget.reserve_storage(floor).unwrap();
        let relation = source.relation(&module, budget);
        state
            .with_input_v1(&relation, budget, |input, budget| {
                input.require_current_graph_v1(budget)
            })
            .unwrap()
            .unwrap();
    });
    assert_eq!(original.work(), usize::MAX);
    assert_eq!(original.storage(), floor);
    assert_eq!(replacement.storage(), floor);
    assert!(replacement.work() > 0);
}

#[test]
fn aggregate_durable_state_uses_fresh_relations_in_original_owned_callbacks() {
    use kir::CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Owned;
    let module = canonical(true);
    let source = SourceFixture::new(true);
    let mut ledger = Owned::new(Work::new(usize::MAX), usize::MAX);
    let state = ledger.with_budget(|budget| {
        let relation = source.relation(&module, budget);
        aggregate(&module, &relation, true, budget)
    });
    let floor = ledger.storage();
    let before = ledger.work();
    ledger.with_budget(|budget| {
        let relation = source.relation(&module, budget);
        state
            .with_input_v1(&relation, budget, |input, budget| {
                input.require_current_graph_v1(budget)
            })
            .unwrap()
            .unwrap();
    });
    assert_eq!(ledger.storage(), floor);
    assert!(ledger.work() > before);
    let pending = ledger.with_budget(|budget| {
        let relation = source.relation(&module, budget);
        state.into_pending_analysis_v1(&relation, budget).unwrap()
    });
    assert_eq!(
        pending
            .legacy_report()
            .coverage_summary()
            .total_view_proved(),
        0
    );
}
