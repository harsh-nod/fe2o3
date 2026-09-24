use super::*;

// Stop before the last transition-state table, not at the observed whole-history
// failure. The unchanged P7/P8/history wrappers add no work before this prefix.
fn transition_pending_prefix(p: &Complete, replay: bool) -> (usize, usize) {
    use fe2o3_kernel_analysis::CanonicalKirInventoryV1 as Inventory;
    use fe2o3_kernel_ir::InertCanonicalKirTransitionReceiptV1 as Wire;
    let inputs = p.inputs().prefix.prefix.prefix;
    let mut work = Work::new(HWORK);
    let mut budget = Budget::new(&mut work, HSTORE);
    budget.reserve_storage(p.floor).unwrap();
    budget.charge_work(17).unwrap();
    if replay {
        budget
            .charge_work(std::mem::size_of::<HLimits>() + 1)
            .unwrap();
    }
    // P6 wrapper, actual claim decoder, then exact fixed composition record.
    budget.charge_work(3).unwrap();
    let _claim = fe2o3_pliron::read_unauthenticated_integer_continuation_claim_v1(
        inputs.prefix.output,
        inputs.output,
        inputs.continuation.integer_record,
        &mut budget,
    )
    .unwrap();
    budget
        .charge_work(2 + crate::POLICY6_EXECUTION_RECORD_BYTES_V1)
        .unwrap();
    let prefix =
        crate::check_published_policy5_semantic_relation_v1(inputs.prefix, &mut budget).unwrap();
    budget
        .reserve_storage(prefix.storage().retained_storage())
        .unwrap();
    budget.charge_work(1).unwrap();
    let (wire, storage) =
        Wire::decode_with_budget(inputs.continuation.transition_wire, &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let (input, storage) = Inventory::derive(inputs.prefix.output, &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let (output, storage) = Inventory::derive(inputs.output, &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    // Endpoint comparisons, checked transition entry, and State::new entry.
    budget.charge_work(80 + 1 + 1).unwrap();
    let fill_counts = [
        output.functions().len(),
        input.functions().len(),
        input.blocks().len(),
        input.blocks().len(),
        output.blocks().len(),
        output.blocks().len(),
        input.operations().len(),
        output.operations().len(),
        output.definitions().len(),
        output.definitions().len(),
        input.definitions().len(),
        input.definitions().len(),
        input.blocks().len(),
        input.edges().len(),
        input.edges().len(),
        input.edges().len(),
        input.blocks().len(),
    ];
    for count in fill_counts {
        budget.charge_work(count).unwrap();
    }
    // pending is the final table. Its fill charge follows its refused reserve.
    let pending = input
        .blocks()
        .len()
        .checked_mul(std::mem::size_of::<usize>())
        .unwrap();
    assert!(pending > 0);
    let accepted = budget.work();
    drop((input, output, wire, prefix));
    budget.release_storage(budget.storage() - p.floor).unwrap();
    assert_eq!((budget.failed_storage(), budget.storage()), (None, p.floor));
    (accepted, pending)
}

fn assert_transition_pending_denial(p: &Complete, replay: bool, short: Measured, peak: usize) {
    use crate::{
        CanonicalPolicy6SemanticErrorV1 as P6Error, CanonicalPolicy7SemanticErrorV1 as P7Error,
        CanonicalPolicy8CompositionErrorV1 as P8Error,
        KernelIrCheckedOptimizationReceiptErrorV1 as ReceiptError,
    };
    use fe2o3_kernel_analysis::CanonicalKirTransitionErrorV1 as TransitionError;
    let (accepted, pending) = transition_pending_prefix(p, replay);
    let Err(HError::Prefix(error)) = short.result else {
        panic!("history P8 prefix");
    };
    let P8Error::Policy7(error) = *error else {
        panic!("history P7 prefix");
    };
    let P7Error::Policy6(error) = *error else {
        panic!("history P6 prefix");
    };
    let P6Error::Transition(ReceiptError::Transition(TransitionError::Resource(
        Resource::Storage(error),
    ))) = *error
    else {
        panic!("exact transition pending-table Storage phase");
    };
    assert_eq!((error.actual(), error.limit()), (peak, peak - 1));
    assert_eq!(
        (
            short.work,
            short.peak,
            short.failed_work,
            short.failed_storage
        ),
        (
            accepted,
            peak.checked_sub(pending).unwrap(),
            None,
            Some(peak)
        )
    );
}

struct Measured {
    result: Result<usize, HError>,
    work: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
}
fn observe(p: &Complete, work_limit: usize, storage_limit: usize) -> Measured {
    let mut work = Work::new(work_limit);
    let (result, accepted, peak, failed_storage) = {
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(p.floor).unwrap();
        budget.reserve_storage(23).unwrap();
        budget.charge_work(17).unwrap();
        let floor = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        let result =
            check_canonical_refined_forwarding_history_v1(p.inputs(), &mut budget).map(|receipt| {
                let retained = receipt.storage().retained_storage();
                budget.reserve_storage(retained).unwrap();
                assert!(std::ptr::eq(receipt.output(), p.forwarded.output()));
                drop(receipt);
                budget.release_storage(retained).unwrap();
                retained
            });
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        let measured = (
            result,
            budget.work(),
            budget.peak_storage(),
            budget.failed_storage(),
        );
        budget.release_storage(23).unwrap();
        assert_eq!(budget.storage(), p.floor);
        measured
    };
    Measured {
        result,
        work: accepted,
        peak,
        failed_work: work.failed_work(),
        failed_storage,
    }
}

#[test]
fn refined_forwarding_history_exact_resource_success_and_final_work_denial() {
    for mutation in [false, true] {
        let p = complete(prepared(mutation, mutation));
        let measured = observe(&p, HWORK, HSTORE);
        let retained = measured.result.unwrap();
        assert!(retained > 0);
        assert_eq!(
            (measured.failed_work, measured.failed_storage),
            (None, None)
        );
        let exact = observe(&p, measured.work, measured.peak);
        assert_eq!(exact.result.unwrap(), retained);
        assert_eq!((exact.work, exact.peak), (measured.work, measured.peak));
        assert_eq!((exact.failed_work, exact.failed_storage), (None, None));
        let short = observe(&p, measured.work - 1, measured.peak);
        let Err(HError::Resource(Resource::Work(limit))) = short.result else {
            panic!("exact final history charge must be Work");
        };
        assert_eq!(
            (limit.actual(), limit.limit()),
            (measured.work, measured.work - 1)
        );
        assert_eq!(short.work, measured.work - 1);
        assert_eq!(short.failed_work, Some(measured.work));
        assert_eq!(short.failed_storage, None);
        assert_eq!(short.peak, measured.peak);
    }
}

#[test]
fn refined_forwarding_history_storage_short_observes_phase_for_strict_successor() {
    for mutation in [false, true] {
        let p = complete(prepared(mutation, mutation));
        let measured = observe(&p, HWORK, HSTORE);
        measured.result.unwrap();
        let short = observe(&p, measured.work, measured.peak - 1);
        assert_transition_pending_denial(&p, false, short, measured.peak);
    }
}

#[test]
fn refined_forwarding_history_replay_preserves_prior_denials_and_live_receipt_floor() {
    let p = complete(prepared(true, true));
    let mut work = Work::new(HWORK);
    {
        let mut budget = Budget::new(&mut work, HSTORE);
        budget.reserve_storage(p.floor + 29).unwrap();
        assert!(matches!(
            budget.charge_work(HWORK + 1),
            Err(Resource::Work(_))
        ));
        assert!(matches!(
            budget.reserve_storage(HSTORE + 1),
            Err(Resource::Storage(_))
        ));
        let expected_storage = p.floor + 29 + HSTORE + 1;
        assert_eq!(budget.failed_storage(), Some(expected_storage));
        let receipt =
            check_canonical_refined_forwarding_history_v1(p.inputs(), &mut budget).unwrap();
        let retained = receipt.storage().retained_storage();
        budget.reserve_storage(retained).unwrap();
        let floor = budget.storage();
        let token = budget.work_ledger_identity_v1();
        receipt.replay(p.inputs().limits, &mut budget).unwrap();
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.failed_storage(), Some(expected_storage));
        assert!(budget.work_ledger_identity_v1() == token);
        drop(receipt);
        budget.release_storage(retained).unwrap();
        assert_eq!(budget.storage(), p.floor + 29);
        budget.release_storage(29).unwrap();
    }
    assert_eq!(work.failed_work(), Some(HWORK + 1));
}

#[test]
fn refined_forwarding_history_receipt_replay_refuses_missing_storage() {
    let p = complete(prepared(false, false));
    let mut work = Work::new(HWORK);
    let mut budget = Budget::new(&mut work, HSTORE);
    budget.reserve_storage(p.floor).unwrap();
    let receipt = check_canonical_refined_forwarding_history_v1(p.inputs(), &mut budget).unwrap();
    let retained = receipt.storage().retained_storage();
    budget.reserve_storage(retained).unwrap();
    let mut other_work = Work::new(HWORK);
    let mut other = Budget::new(&mut other_work, HSTORE);
    other.reserve_storage(retained - 1).unwrap();
    assert!(matches!(
        receipt.replay(p.inputs().limits, &mut other),
        Err(HError::Resource(Resource::Accounting))
    ));
    assert_eq!(other.storage(), retained - 1);
    assert_eq!(other.work(), 0);
    assert_eq!(budget.storage(), p.floor + retained);
    drop(receipt);
    budget.release_storage(retained).unwrap();
}

fn observe_replay(p: &Complete, work_limit: usize, storage_limit: usize) -> Measured {
    let mut parent_work = Work::new(HWORK);
    let mut parent = Budget::new(&mut parent_work, HSTORE);
    parent.reserve_storage(p.floor).unwrap();
    let receipt = check_canonical_refined_forwarding_history_v1(p.inputs(), &mut parent).unwrap();
    let retained = receipt.storage().retained_storage();
    parent.reserve_storage(retained).unwrap();
    let floor = parent.storage() + 23;
    let mut work = Work::new(work_limit);
    let (result, accepted, peak, failed_storage) = {
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(17).unwrap();
        let token = budget.work_ledger_identity_v1();
        let result = receipt
            .replay(p.inputs().limits, &mut budget)
            .map(|()| retained);
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == token);
        (
            result,
            budget.work(),
            budget.peak_storage(),
            budget.failed_storage(),
        )
    };
    drop(receipt);
    parent.release_storage(retained).unwrap();
    Measured {
        result,
        work: accepted,
        peak,
        failed_work: work.failed_work(),
        failed_storage,
    }
}

#[test]
fn refined_forwarding_history_receipt_replay_boundaries_and_storage_observation() {
    for mutation in [false, true] {
        let p = complete(prepared(mutation, mutation));
        let measured = observe_replay(&p, HWORK, HSTORE);
        let retained = measured.result.unwrap();
        let exact = observe_replay(&p, measured.work, measured.peak);
        assert_eq!(exact.result.unwrap(), retained);
        assert_eq!((exact.work, exact.peak), (measured.work, measured.peak));
        assert_eq!((exact.failed_work, exact.failed_storage), (None, None));
        let short = observe_replay(&p, measured.work - 1, measured.peak);
        let Err(HError::Resource(Resource::Work(limit))) = short.result else {
            panic!("exact final replay Work charge");
        };
        assert_eq!(
            (limit.actual(), limit.limit()),
            (measured.work, measured.work - 1)
        );
        assert_eq!(short.work, measured.work - 1);
        assert_eq!(short.peak, measured.peak);
        assert_eq!(short.failed_work, Some(measured.work));
        assert_eq!(short.failed_storage, None);
        let short = observe_replay(&p, measured.work, measured.peak - 1);
        assert_transition_pending_denial(&p, true, short, measured.peak);
    }
}
