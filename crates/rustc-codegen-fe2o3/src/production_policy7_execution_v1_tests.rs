use super::*;
use crate::production_ranked_projection_v1::with_backend_policy7_direct_prefix_v1;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[test]
fn policy7_execution_extent_is_complete_checked_header_plus_rows() {
    assert_eq!(execution::extent_counts(0, 0).unwrap(), 384);
    assert_eq!(execution::extent_counts(2, 5).unwrap(), 552);
    for pair in [
        (usize::MAX, 1),
        (usize::MAX / 24 + 1, 0),
        (usize::MAX / 24, 0),
    ] {
        assert!(execution::extent_counts(pair.0, pair.1).is_err());
    }
}

#[test]
fn policy7_execution_real_owner_has_exact_storage_work_and_same_ledger() {
    with_backend_policy7_direct_prefix_v1(Profile::Gfx942, true, |prefix, _, parent| {
        let (admitted, storage) = prefix.continue_redundant_private_stores_v1(parent).unwrap();
        parent.reserve_storage(storage.retained_storage()).unwrap();
        let owner = Admitted7::Direct(admitted);
        assert_eq!(owner.continuation().rows().len(), 2);
        let floor = parent.storage();
        let run = |work_limit, storage_limit| {
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(floor).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let result = Policy7ExecutionWitnessV1::prepare(&owner, &mut budget);
            let accepted = result.is_ok();
            drop(result);
            assert_eq!(budget.storage(), floor);
            assert!(budget.work_ledger_identity_v1() == ledger);
            (accepted, budget.work(), budget.peak_storage())
        };
        let (success, work, peak) = run(1_000_000_000, 1024 * 1024 * 1024);
        assert!(success);
        assert!(run(work, peak).0);
        assert!(!run(work - 1, peak).0);
        assert!(!run(work, peak - 1).0);
        execution::exercise_exact_record(&owner, parent);
        drop(owner);
        parent.release_storage(storage.retained_storage()).unwrap();
    });
}

struct Mark(Arc<AtomicUsize>);
impl Drop for Mark {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn policy7_scope_preserves_unrelated_floor_and_drops_failure_before_cleanup() {
    for panic in [false, true] {
        let drops = Arc::new(AtomicUsize::new(0));
        let mut work = Work::new(1000);
        let mut budget = Budget::new(&mut work, 1000);
        budget.reserve_storage(37).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result: Result7<()> = scoped(37, &mut budget, |budget| {
            budget.reserve_storage(61).map_err(resource)?;
            let _mark = Mark(Arc::clone(&drops));
            if panic {
                panic!("Policy7 owned failure control");
            }
            Err(execution_error("failure control"))
        });
        assert!(result.is_err());
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert_eq!(budget.storage(), 37);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert!(scoped(38, &mut budget, |_| Ok(())).is_err());
        assert_eq!(budget.storage(), 37);
    }
}

#[test]
fn policy7_scope_refuses_foreign_ledger_on_success_error_and_panic() {
    for exit in 0..3 {
        let drops = Arc::new(AtomicUsize::new(0));
        let mut work = Work::new(1000);
        let mut other_work = Work::new(1000);
        let mut budget = Budget::new(&mut work, 1000);
        let mut other = Budget::new(&mut other_work, 1000);
        budget.reserve_storage(41).unwrap();
        other.reserve_storage(73).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let foreign = other.work_ledger_identity_v1();
        let result = scoped(41, &mut budget, |budget| {
            let mark = Mark(Arc::clone(&drops));
            std::mem::swap(budget, &mut other);
            match exit {
                0 => Ok(mark),
                1 => Err(execution_error("foreign")),
                _ => panic!("foreign"),
            }
        });
        assert!(matches!(
            result,
            Err(ProductionPipelineError::CheckedOutputPolicy7Stage(
                CheckedOutputPolicy7StageErrorV1::Resource(Resource::Accounting)
            ))
        ));
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert_eq!(budget.storage(), 73);
        assert_eq!(other.storage(), 41);
        assert!(budget.work_ledger_identity_v1() == foreign);
        assert!(other.work_ledger_identity_v1() == ledger);
    }
}

#[test]
fn policy7_scope_restores_floor_before_panicking_payload_destructor() {
    struct Payload;
    impl Drop for Payload {
        fn drop(&mut self) {
            panic!("Policy7 payload destructor");
        }
    }
    let mut work = Work::new(1000);
    let mut budget = Budget::new(&mut work, 1000);
    budget.reserve_storage(43).unwrap();
    let result = catch_unwind(AssertUnwindSafe(|| {
        scoped::<()>(43, &mut budget, |budget| {
            budget.reserve_storage(67).map_err(resource)?;
            std::panic::panic_any(Payload)
        })
    }));
    assert!(result.is_err());
    assert_eq!(budget.storage(), 43);
}
