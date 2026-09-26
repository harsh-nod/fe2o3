use super::*;
use std::mem::size_of;

#[test]
fn canonical_scalar_factory_source_floor_and_first_work_denials_are_independent() {
    // cs_scope prepays 2, then the consuming factory prepays 3 before its header.
    for (limit, accepted, denied) in [(1, 0, 2), (4, 2, 5)] {
        let source = cr_noop(&["work_prefix"]);
        let floor = source.unit_local_source_storage_floor_v1().unwrap() + SIBLING;
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, S);
        budget.reserve_storage(floor).unwrap();
        assert!(
            ProductionCanonicalScalarFixedPointOwnerV1::try_prepare_v1(source, &mut budget)
                .is_err()
        );
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.work(), accepted);
        assert_eq!(budget.peak_storage(), floor);
        assert_eq!(budget.failed_storage(), None);
        drop(budget);
        assert_eq!(work.failed_work(), Some(denied));
    }
    let source = cr_noop(&["unpaid_source"]);
    let short = source.unit_local_source_storage_floor_v1().unwrap() - 1;
    let mut work = Work::new(5);
    let mut budget = Budget::new(&mut work, S);
    budget.reserve_storage(short).unwrap();
    assert!(matches!(
        ProductionCanonicalScalarFixedPointOwnerV1::try_prepare_v1(source, &mut budget),
        Err(CsError::Resource(ArgumentResourceV1::Accounting))
    ));
    assert_eq!(
        (budget.storage(), budget.work(), budget.peak_storage()),
        (short, 5, short)
    );
}

#[test]
fn canonical_scalar_factory_owner_header_one_short_does_not_begin_source_replay() {
    let source = cr_noop(&["header_short"]);
    let floor = source.unit_local_source_storage_floor_v1().unwrap() + SIBLING;
    let admitted = floor + size_of::<ProductionCanonicalScalarFixedPointOwnerV1>();
    let mut work = Work::new(5);
    let mut budget = Budget::new(&mut work, admitted - 1);
    budget.reserve_storage(floor).unwrap();
    assert!(
        ProductionCanonicalScalarFixedPointOwnerV1::try_prepare_v1(source, &mut budget).is_err()
    );
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.work(), 5);
    assert_eq!(budget.peak_storage(), floor);
    assert_eq!(budget.failed_storage(), Some(admitted));
    drop(budget);
    assert_eq!(work.failed_work(), None);
}

#[test]
fn canonical_scalar_complete_original_plus_history_receipt_is_required() {
    let owner = cs_prepare(cs_source());
    assert_eq!(
        owner.additional_storage().retained_storage(),
        size_of::<ProductionCanonicalScalarFixedPointOwnerV1>()
            + owner.history().retained_storage()
    );
    assert_eq!(
        owner.input_storage_floor_v1(),
        owner
            .original_source()
            .unit_local_source_storage_floor_v1()
            .unwrap()
    );
    let short = owner.retained_storage_floor_v1() - 1;
    let mut work = Work::new(3);
    let mut budget = Budget::new(&mut work, S);
    budget.reserve_storage(short).unwrap();
    assert!(matches!(
        owner.with_policy_checks_v1(&mut budget, |_, _| -> CsResultV1<()> {
            panic!("unpaid history")
        }),
        Err(CsError::Resource(ArgumentResourceV1::Accounting))
    ));
    assert_eq!(
        (budget.storage(), budget.work(), budget.peak_storage()),
        (short, 3, short)
    );
    assert_eq!(budget.failed_storage(), None);
    // A failed invocation does not consume the owning source/history.
    cs_run(&owner, |view, budget| {
        view.policies(budget)?.function_count(budget)?;
        Ok(())
    })
    .unwrap();
}

#[test]
fn canonical_scalar_identity_lineage_storage_has_exact_actual_capacities() {
    let owner = cs_prepare(cr_noop(&["one", "two"]));
    let mut work = Work::new(1 << 48);
    let mut budget = Budget::new(&mut work, S);
    let floor = owner.retained_storage_floor_v1() + SIBLING;
    budget.reserve_storage(floor).unwrap();
    cs_scope_v1(&mut budget, |budget| {
        let (inventory, receipt) =
            CanonicalKirInventoryV1::derive(owner.original_source().executable(), budget)?;
        budget.reserve_storage(receipt.retained_storage())?;
        let entry = budget.storage();
        let rows = cs_identity_v1(&inventory, budget)?;
        assert_eq!(rows.functions.len(), 2);
        assert_eq!(rows.blocks.len(), 2);
        assert_eq!(rows.segments.len(), 2);
        assert!(rows.definitions.is_empty() && rows.uses.is_empty() && rows.edges.is_empty());
        let capacities = rows.functions.capacity() * size_of::<CsFunctionV1>()
            + rows.operations.capacity() * size_of::<ProductionCanonicalScalarOperationOriginV1>()
            + rows.definitions.capacity() * size_of::<CsDefinitionLinkV1>()
            + rows.ancestors.capacity() * size_of::<std::ops::Range<usize>>()
            + rows.blocks.capacity() * size_of::<std::ops::Range<usize>>()
            + rows.segments.capacity() * size_of::<ProductionCanonicalScalarBlockSegmentV1>()
            + rows.block_controls.capacity() * size_of::<ProductionCanonicalScalarBlockControlV1>()
            + rows.edge_controls.capacity() * size_of::<ProductionCanonicalScalarEdgeControlV1>()
            + rows.uses.capacity() * size_of::<CsUseV1>()
            + rows.edges.capacity() * size_of::<CsEdgeV1>()
            + rows.arguments.capacity() * size_of::<CsEdgeArgumentV1>();
        assert_eq!(rows.storage, size_of::<CsLineageV1>() + capacities);
        assert_eq!(budget.storage(), entry + rows.storage);
        Ok(())
    })
    .unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn canonical_scalar_callback_error_panic_and_poison_keep_the_whole_owner_live() {
    let owner = cs_prepare(cs_source());
    assert!(matches!(
        cs_run(&owner, |_, _| -> CsResultV1<()> {
            Err(CsError::Invalid("callback refusal"))
        }),
        Err(CsError::Invalid("callback refusal"))
    ));
    let error = cs_run(&owner, |_, _| -> CsResultV1<()> {
        panic!("actual final callback panic")
    })
    .unwrap_err();
    let CsError::FinalPolicy(error) = error else {
        panic!("precise final policy error required")
    };
    assert!(matches!(
        error.failure(),
        fe2o3_pliron::CanonicalRankedPolicyFailureV1::Panicked
    ));
    // This panic occurs after all analysis invocations, in the real user callback.
    // The analysis-domain caught_panic bit must not be relabeled as callback state.
    assert!(!error.observation().caught_panic());
    assert!(error.observation().work_upper_bound() > 0);
    assert!(
        cs_run(&owner, |view, budget| {
            let line = view.lineage(budget)?;
            assert!(line.operation_origin(usize::MAX, budget).is_err());
            Ok(())
        })
        .is_err(),
        "ignored hostile source query must remain sticky"
    );
    cs_run(&owner, |view, budget| {
        assert!(view.policies(budget)?.report(0, budget)?.is_clean());
        Ok(())
    })
    .unwrap();
}

#[test]
fn canonical_scalar_foreign_or_moved_query_budget_never_debits_substitute() {
    let owner = cs_prepare(cr_noop(&["custody"]));
    for moved in [false, true] {
        let mut work = Work::new(1 << 48);
        let mut other_work = Work::new(1 << 48);
        let mut budget = Budget::new(&mut work, S);
        let mut other = Budget::new(&mut other_work, S);
        let floor = owner.retained_storage_floor_v1() + SIBLING;
        budget.reserve_storage(floor).unwrap();
        other.reserve_storage(SIBLING).unwrap();
        let result = owner.with_policy_checks_v1(&mut budget, |view, budget| {
            let line = view.lineage(budget)?;
            if moved {
                std::mem::swap(budget, &mut other);
                let before = other.work();
                assert!(line.function_origin(0, &mut other).is_err());
                assert_eq!(other.work(), before);
                std::mem::swap(budget, &mut other);
            } else {
                assert!(line.function_origin(0, &mut other).is_err());
            }
            Ok(())
        });
        assert!(result.is_err());
        assert_eq!(budget.storage(), floor);
        assert_eq!((other.storage(), other.work()), (SIBLING, 0));
    }
}

#[test]
fn canonical_scalar_real_callback_panic_payload_drops_once() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Payload(Arc<AtomicUsize>);
    impl Drop for Payload {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    let owner = cs_prepare(cr_noop(&["payload"]));
    let drops = Arc::new(AtomicUsize::new(0));
    assert!(
        cs_run(&owner, |_, _| -> CsResultV1<()> {
            std::panic::panic_any(Payload(Arc::clone(&drops)))
        })
        .is_err()
    );
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}
