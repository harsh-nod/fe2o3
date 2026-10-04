//! Same genuine full-history fixture; no reconstructed executed-policy owner.
use super::*;
use crate::CanonicalPolicy8HistoryInputsErrorV1 as InputsError;

#[path = "checked_optimization_policy8_history_inputs_hostile_v1_tests.rs"]
mod hostile;

#[derive(Debug)]
enum Rejection {
    Frame(TransportError),
    Inputs(InputsError),
    Semantic(CanonicalPolicy8CompositionErrorV1),
    Resource(Resource),
}
#[derive(Debug, Eq, PartialEq)]
struct Summary {
    retained: usize,
    pairs: usize,
    loads: usize,
    deletions: usize,
}
struct Observation {
    result: Result<Summary, Rejection>,
    work: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
}

fn drive(
    wire: &[u8],
    k: &Owner,
    budget: &mut Budget<'_>,
    semantic: bool,
) -> Result<Summary, Rejection> {
    let frame = read_inert_policy8_history_v1(wire, k, budget).map_err(Rejection::Frame)?;
    budget
        .reserve_storage(frame.storage().retained_storage())
        .map_err(Rejection::Resource)?;
    let floor = budget.storage();
    let constructed = materialize_policy8_history_inputs_v1(&frame, budget);
    assert_eq!(budget.storage(), floor);
    let owner = constructed.map_err(Rejection::Inputs)?;
    let retained = owner.storage().retained_storage();
    let (inputs, storage) = owner.test_components();
    assert_eq!(retained, storage.into_iter().sum::<usize>());
    assert!(storage[1] >= std::mem::size_of_val(inputs.prefix.prefix.prefix.load_rows));
    assert!(std::ptr::eq(owner.frame(), &frame));
    assert!(std::ptr::eq(owner.external_output(), k));
    assert!(!owner.proves_semantic_preservation());
    assert!(!owner.authenticates_execution());
    assert!(!owner.grants_authority());
    for role in ROLES {
        assert_eq!(
            owner.graph(role).canonical().canonical_bytes(),
            frame.graph_bytes(role)
        );
    }
    assert!(std::ptr::eq(inputs.prefix.output, owner.graph(Role::J)));
    assert!(std::ptr::eq(inputs.output, k));
    let mut summary = Summary {
        retained,
        pairs: 0,
        loads: inputs.prefix.prefix.prefix.load_rows.len(),
        deletions: inputs.prefix.continuation.deletion_rows.len(),
    };
    budget
        .reserve_storage(retained)
        .map_err(Rejection::Resource)?;
    if semantic {
        let floor = budget.storage();
        let checked = owner.check_semantics(budget);
        assert_eq!(budget.storage(), floor);
        let receipt = checked.map_err(Rejection::Semantic)?;
        assert!(std::ptr::eq(
            receipt.continuation().input(),
            owner.graph(Role::J)
        ));
        assert!(std::ptr::eq(receipt.output(), k));
        assert!(std::ptr::eq(
            receipt
                .policy7_relation()
                .continuation()
                .relation()
                .output(),
            owner.graph(Role::J)
        ));
        let p5 = receipt
            .policy7_relation()
            .policy6_relation()
            .policy5_relation();
        assert!(std::ptr::eq(
            p5.load_forwarding_rows(),
            inputs.prefix.prefix.prefix.load_rows
        ));
        assert!(!receipt.authenticates_execution());
        assert!(!receipt.grants_authority());
        summary.pairs = receipt.continuation().proved_pairs();
        budget
            .reserve_storage(receipt.storage().retained_storage())
            .map_err(Rejection::Resource)?;
    }
    // All local owners and borrowed receipts drop before the test driver's refund.
    Ok(summary)
}

fn run(
    wire: &[u8],
    k: &Owner,
    floor: usize,
    work_limit: usize,
    storage_limit: usize,
    semantic: bool,
    prior_denials: bool,
) -> Observation {
    let mut work = Work::new(work_limit);
    let (result, accepted, peak, failed_storage) = {
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(17).unwrap();
        if prior_denials {
            assert!(budget.charge_work(usize::MAX).is_err());
            assert!(budget.reserve_storage(usize::MAX).is_err());
        }
        let ledger = budget.work_ledger_identity_v1();
        let result = drive(wire, k, &mut budget, semantic);
        assert!(budget.work_ledger_identity_v1() == ledger);
        budget
            .release_storage(budget.storage().checked_sub(floor).unwrap())
            .unwrap();
        assert_eq!(budget.storage(), floor);
        (
            result,
            budget.work(),
            budget.peak_storage(),
            budget.failed_storage(),
        )
    };
    Observation {
        result,
        work: accepted,
        peak,
        failed_storage,
        failed_work: work.failed_work(),
    }
}

fn success(p: &Prepared8) -> Summary {
    let wire = make(p);
    run(
        wire.canonical_bytes(),
        p.tail.output(),
        p.floor + wire.storage().retained_storage(),
        WORK,
        STORAGE,
        true,
        false,
    )
    .result
    .unwrap()
}

#[test]
fn one_frame_materializes_and_checks_all_genuine_store_swap_combinations() {
    for stores in [false, true] {
        for swaps in [false, true] {
            let p = prepared(stores, swaps);
            let result = success(&p);
            assert_eq!(result.pairs, usize::from(swaps));
            assert_eq!(result.deletions, if stores { 2 } else { 0 });
            assert_eq!(
                p.inputs().prefix.output.canonical().canonical_bytes()
                    != p.tail.output().canonical().canonical_bytes(),
                swaps
            );
        }
    }
}

#[test]
fn empty_all_external_history_keeps_zero_pairs_and_no_deletions() {
    let p = prepared_module(&Module::new("decoded-empty-policy8"));
    let result = success(&p);
    assert_eq!((result.pairs, result.loads, result.deletions), (0, 0, 0));
    let wire = make(&p);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let frame = read_inert_policy8_history_v1(wire.canonical_bytes(), p.tail.output(), &mut budget)
        .unwrap();
    assert_eq!(frame.stored_graph_count(), 0);
    for role in ROLES {
        assert!(frame.role(role).is_external_output());
    }
}

#[test]
fn genuine_nonempty_p5_load_rows_are_materialized_and_replayed() {
    let p = prepared_module(&crate::checked_load_forwarding_v1::tests::fixture());
    assert_eq!(p.inputs().prefix.prefix.prefix.load_rows.len(), 2);
    let result = success(&p);
    assert_eq!(result.loads, 2);
    assert_eq!(result.pairs, 0);
}

#[test]
fn full_reader_constructor_and_checker_have_exact_cumulative_limits() {
    let p = prepared(true, true);
    let wire = make(&p);
    let floor = p.floor + wire.storage().retained_storage() + 31;
    for semantic in [false, true] {
        let baseline = run(
            wire.canonical_bytes(),
            p.tail.output(),
            floor,
            WORK,
            STORAGE,
            semantic,
            false,
        );
        let wanted = baseline.result.unwrap();
        for _ in 0..2 {
            let exact = run(
                wire.canonical_bytes(),
                p.tail.output(),
                floor,
                baseline.work,
                baseline.peak,
                semantic,
                false,
            );
            assert_eq!(exact.result.unwrap(), wanted);
            assert_eq!(
                (
                    exact.work,
                    exact.peak,
                    exact.failed_work,
                    exact.failed_storage
                ),
                (baseline.work, baseline.peak, None, None)
            );
        }
        let short = run(
            wire.canonical_bytes(),
            p.tail.output(),
            floor,
            baseline.work - 1,
            baseline.peak,
            semantic,
            false,
        );
        assert!(short.result.is_err());
        assert!(
            short
                .failed_work
                .is_some_and(|attempt| attempt > baseline.work - 1)
        );
        assert_eq!(short.failed_storage, None);
        let short = run(
            wire.canonical_bytes(),
            p.tail.output(),
            floor,
            baseline.work,
            baseline.peak - 1,
            semantic,
            false,
        );
        assert!(short.result.is_err());
        assert!(
            short
                .failed_storage
                .is_some_and(|attempt| attempt > baseline.peak - 1)
        );
        assert_eq!(short.failed_work, None);
        if let Err(Rejection::Resource(error)) = &short.result {
            assert!(matches!(error, Resource::Storage(_)));
        }
    }
}

#[test]
fn repeated_semantics_matches_one_unchanged_checker_plus_fixed_assembly_work() {
    let p = prepared(true, true);
    let wire = make(&p);
    let mut preparation = Work::new(WORK);
    let mut budget = Budget::new(&mut preparation, STORAGE);
    let backing = p.floor + wire.storage().retained_storage();
    budget.reserve_storage(backing).unwrap();
    let frame = read_inert_policy8_history_v1(wire.canonical_bytes(), p.tail.output(), &mut budget)
        .unwrap();
    budget
        .reserve_storage(frame.storage().retained_storage())
        .unwrap();
    let owner = materialize_policy8_history_inputs_v1(&frame, &mut budget).unwrap();
    let floor =
        backing + frame.storage().retained_storage() + owner.storage().retained_storage() + 31;
    let manual = {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(128).unwrap();
        let (inputs, _) = owner.test_components();
        let receipt = check_published_policy8_semantic_relation_v1(inputs, &mut budget).unwrap();
        let retained = receipt.storage().retained_storage();
        assert_eq!(budget.storage(), floor);
        drop(receipt);
        (budget.work(), budget.peak_storage(), retained)
    };
    for _ in 0..2 {
        let mut work = Work::new(manual.0);
        let mut budget = Budget::new(&mut work, manual.1);
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let receipt = owner.check_semantics(&mut budget).unwrap();
        assert_eq!(receipt.storage().retained_storage(), manual.2);
        assert_eq!(receipt.continuation().proved_pairs(), 1);
        drop(receipt);
        assert_eq!(
            (budget.storage(), budget.work(), budget.peak_storage()),
            (floor, manual.0, manual.1)
        );
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
    for (work_limit, storage_limit) in [(manual.0 - 1, manual.1), (manual.0, manual.1 - 1)] {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        assert!(owner.check_semantics(&mut budget).is_err());
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn numeric_transfer_precondition_is_not_original_ledger_provenance() {
    let p = prepared(false, false);
    let wire = make(&p);
    let mut first = Work::new(WORK);
    let mut budget = Budget::new(&mut first, STORAGE);
    let frame = read_inert_policy8_history_v1(wire.canonical_bytes(), p.tail.output(), &mut budget)
        .unwrap();
    budget
        .reserve_storage(frame.storage().retained_storage())
        .unwrap();
    let owner = materialize_policy8_history_inputs_v1(&frame, &mut budget).unwrap();
    let mut other = Work::new(WORK);
    let mut transferred = Budget::new(&mut other, STORAGE);
    let storage = owner.storage().retained_storage();
    transferred.reserve_storage(storage - 1).unwrap();
    assert!(matches!(
        owner.check_semantics(&mut transferred),
        Err(CanonicalPolicy8CompositionErrorV1::Resource(
            Resource::Accounting
        ))
    ));
    assert_eq!(
        (transferred.storage(), transferred.work()),
        (storage - 1, 0)
    );
    transferred.reserve_storage(1).unwrap();
    // External backing is explicitly outside this second call's ledger. The
    // numeric precondition must not invent a constructor-ledger identity claim.
    let receipt = owner.check_semantics(&mut transferred).unwrap();
    assert_eq!(receipt.continuation().proved_pairs(), 0);
    drop(receipt);
    assert_eq!(transferred.storage(), storage);
}

#[test]
fn first_denial_history_survives_complete_success_and_later_semantic_refusal() {
    let p = prepared(true, true);
    let wire = make(&p);
    let floor = p.floor + wire.storage().retained_storage();
    let result = run(
        wire.canonical_bytes(),
        p.tail.output(),
        floor,
        WORK,
        STORAGE,
        true,
        true,
    );
    assert!(result.result.is_ok());
    assert_eq!(
        (result.failed_work, result.failed_storage),
        (Some(usize::MAX), Some(usize::MAX))
    );
    let mut bad = wire.canonical_bytes().to_vec();
    let start = section(&bad, 2).start;
    bad[start] ^= 1;
    let result = run(
        &bad,
        p.tail.output(),
        floor + bad.capacity(),
        WORK,
        STORAGE,
        true,
        true,
    );
    assert!(matches!(result.result, Err(Rejection::Semantic(_))));
    assert_eq!(
        (result.failed_work, result.failed_storage),
        (Some(usize::MAX), Some(usize::MAX))
    );
}

#[test]
fn constructor_matches_one_real_pool_and_each_typed_row_phase_with_complete_receipts() {
    for p in [
        prepared(true, true),
        prepared_module(&crate::checked_load_forwarding_v1::tests::fixture()),
    ] {
        let wire = make(&p);
        let mut preparation = Work::new(WORK);
        let mut preparation_budget = Budget::new(&mut preparation, STORAGE);
        let frame = read_inert_policy8_history_v1(
            wire.canonical_bytes(),
            p.tail.output(),
            &mut preparation_budget,
        )
        .unwrap();
        let floor =
            p.floor + wire.storage().retained_storage() + frame.storage().retained_storage() + 31;
        let (wanted, storage) = {
            let mut work = Work::new(WORK);
            let mut budget = Budget::new(&mut work, STORAGE);
            budget.reserve_storage(floor).unwrap();
            let owner = materialize_policy8_history_inputs_v1(&frame, &mut budget).unwrap();
            assert_eq!(budget.storage(), floor);
            let (_, storage) = owner.test_components();
            let wanted = (
                owner.storage().retained_storage(),
                budget.work(),
                budget.peak_storage(),
            );
            drop(owner);
            (wanted, storage)
        };
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        let phase_work;
        {
            budget.reserve_storage(storage[0]).unwrap();
            let pool = admit_policy8_history_graph_pool_v1(&frame, &mut budget).unwrap();
            assert_eq!(pool.storage().retained_storage(), storage[2]);
            budget.reserve_storage(storage[2]).unwrap();
            let pool_end = budget.work();
            let source = p.inputs().prefix.prefix.prefix.load_rows;
            let requested = std::mem::size_of_val(source);
            // Independent metadata/copy schedule, using actual typed fixture rows.
            assert_eq!(frame.load_row_bytes().len(), 4 + source.len() * 24);
            budget.charge_work(1).unwrap();
            budget
                .charge_work(source.len() * 24 + requested + source.len())
                .unwrap();
            budget.reserve_storage(requested).unwrap();
            let mut copied = Vec::new();
            copied.try_reserve_exact(source.len()).unwrap();
            let backing = copied.capacity()
                * size_of::<fe2o3_kernel_analysis::CanonicalKirLoadForwardingRowV1>();
            budget.reserve_storage(backing - requested).unwrap();
            copied.extend_from_slice(source);
            assert_eq!(backing, storage[1]);
            let load_end = budget.work();
            let p7 = decode_canonical_policy7_rows_v1(frame.policy7_record(), &mut budget).unwrap();
            assert_eq!(p7.storage().retained_storage(), storage[3]);
            budget.reserve_storage(storage[3]).unwrap();
            let p7_end = budget.work();
            let (tail, receipt) = fe2o3_kernel_ir::materialize_canonical_kir_occurrence_rows_v1(
                frame.tail_rows(),
                &mut budget,
            )
            .unwrap();
            assert_eq!(receipt.retained_storage(), storage[4]);
            budget.reserve_storage(storage[4]).unwrap();
            assert_eq!(
                (
                    budget.storage() - floor,
                    budget.work(),
                    budget.peak_storage()
                ),
                wanted
            );
            phase_work = [pool_end, load_end, p7_end];
            drop((tail, p7, copied, pool));
        }
        budget.release_storage(wanted.0).unwrap();
        assert_eq!(budget.storage(), floor);
        // Exhaust work after each completed real phase. Construction must refuse
        // the next phase and drop every earlier retained owner before refund.
        for limit in phase_work {
            let mut work = Work::new(limit);
            let (used, peak) = {
                let mut budget = Budget::new(&mut work, STORAGE);
                budget.reserve_storage(floor).unwrap();
                assert!(materialize_policy8_history_inputs_v1(&frame, &mut budget).is_err());
                assert_eq!(budget.storage(), floor);
                (budget.work(), budget.peak_storage())
            };
            assert!(used <= limit);
            assert!(work.failed_work().is_some_and(|attempt| attempt > limit));
            assert!(peak <= wanted.2);
        }
    }
}

#[test]
fn complete_new_header_is_prepaid_before_graph_admission_or_row_work() {
    let p = prepared(true, true);
    let wire = make(&p);
    let mut preparation = Work::new(WORK);
    let mut preparation_budget = Budget::new(&mut preparation, STORAGE);
    let frame = read_inert_policy8_history_v1(
        wire.canonical_bytes(),
        p.tail.output(),
        &mut preparation_budget,
    )
    .unwrap();
    let floor = p.floor + wire.storage().retained_storage() + frame.storage().retained_storage();
    let header = size_of::<DecodedPolicy8HistoryInputsV1<'_, '_, '_>>();
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, floor + header - 1);
    budget.reserve_storage(floor).unwrap();
    assert!(matches!(
        materialize_policy8_history_inputs_v1(&frame, &mut budget),
        Err(InputsError::Resource(Resource::Storage(_)))
    ));
    assert_eq!(
        (budget.storage(), budget.work(), budget.failed_storage()),
        (floor, 0, Some(floor + header))
    );
}
