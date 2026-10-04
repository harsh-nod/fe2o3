use super::tests::{FLOOR, STORAGE, WORK, actual_claims, effects, fixture, pair};
use super::*;
use fe2o3_kernel_analysis::CanonicalKirTransitionErrorV1;
use fe2o3_kernel_ir::{
    BinaryOp, CanonicalKernelIrWorkBudgetV1 as Work,
    CanonicalKirControlFlowScopeErrorV1 as CfgError, ScalarType,
};
use std::{
    panic::panic_any,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

fn resource(error: Error) -> Resource {
    match error {
        Error::Resource(error)
        | Error::Inventory(CanonicalKirInventoryErrorV1::Resource(error))
        | Error::Continuation(CanonicalKirCommutativeBitwiseCseErrorV1::Resource(error))
        | Error::Continuation(CanonicalKirCommutativeBitwiseCseErrorV1::Transition(
            CanonicalKirTransitionErrorV1::Resource(error),
        ))
        | Error::Continuation(CanonicalKirCommutativeBitwiseCseErrorV1::ControlFlow(
            CfgError::Resource(error),
        )) => error,
        other => panic!("expected resource refusal, got {other:?}"),
    }
}

#[test]
fn deterministic_cumulative_work_exact_and_one_short_limits() {
    let f = fixture(&effects());
    let floor = FLOOR + f.backing;
    let run = |work_limit, storage_limit| {
        let mut work = Work::new(work_limit);
        let (result, accepted, peak, failed_storage) = {
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(floor).unwrap();
            budget.charge_work(17).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let result = check_canonical_policy8_continuation_relation_v1(
                &f.input,
                f.tail.output(),
                actual_claims(&f),
                &mut budget,
            )
            .map(|receipt| (receipt.proved_pairs(), receipt.storage().retained_storage()));
            assert_eq!(budget.storage(), floor);
            assert!(budget.work_ledger_identity_v1() == ledger);
            (
                result,
                budget.work(),
                budget.peak_storage(),
                budget.failed_storage(),
            )
        };
        (result, accepted, peak, failed_storage, work.failed_work())
    };
    let (first, work, peak, denial, work_denial) = run(WORK, STORAGE);
    let first = first.unwrap();
    assert_eq!(first.0, 1);
    assert_eq!(
        first.1,
        size_of::<CheckedCanonicalPolicy8ContinuationRelationV1<'_, '_, '_>>()
    );
    assert!(work > 17);
    assert!(peak > floor + first.1);
    assert_eq!((denial, work_denial), (None, None));
    for _ in 0..2 {
        let (result, again, high, denial, work_denial) = run(work, peak);
        assert_eq!(result.unwrap(), first);
        assert_eq!((again, high, denial, work_denial), (work, peak, None, None));
    }
    let (result, accepted, _, _, failed) = run(work - 1, peak);
    let Resource::Work(limit) = resource(result.err().unwrap()) else {
        panic!("work refusal required")
    };
    assert_eq!(limit.limit(), work - 1);
    assert_eq!(Some(limit.actual()), failed);
    assert!(accepted >= 17 && accepted < work);
    let (result, accepted, high, failed, _) = run(work, peak - 1);
    let Resource::Storage(limit) = resource(result.err().unwrap()) else {
        panic!("storage refusal required")
    };
    assert_eq!(limit.limit(), peak - 1);
    assert_eq!(Some(limit.actual()), failed);
    assert!(accepted >= 17 && accepted <= work);
    assert!(high < peak);
}

#[test]
fn returned_receipt_is_unreserved_and_borrowed_backing_stays_live() {
    let f = fixture(&pair(ScalarType::U32, BinaryOp::BitXor, true));
    let floor = FLOOR + f.backing;
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(floor).unwrap();
    let retained;
    {
        let receipt = check_canonical_policy8_continuation_relation_v1(
            &f.input,
            f.tail.output(),
            actual_claims(&f),
            &mut budget,
        )
        .unwrap();
        assert_eq!(budget.storage(), floor);
        retained = receipt.storage().retained_storage();
        budget.reserve_storage(retained).unwrap();
        assert_eq!(receipt.proved_pairs(), 1);
        assert!(std::ptr::eq(receipt.output(), f.tail.output()));
        assert_eq!(budget.storage(), floor + retained);
    }
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), floor);
    drop(f);
    budget.release_storage(floor).unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn tiny_limits_refuse_before_unfunded_work_and_preserve_floor() {
    let f = fixture(&effects());
    let floor = FLOOR + f.backing;
    let mut work = Work::new(0);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(floor).unwrap();
    let error = check_canonical_policy8_continuation_relation_v1(
        &f.input,
        f.tail.output(),
        actual_claims(&f),
        &mut budget,
    )
    .err()
    .unwrap();
    assert!(matches!(error, Error::Resource(Resource::Work(_))));
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.work(), 0);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, floor);
    budget.reserve_storage(floor).unwrap();
    let error = check_canonical_policy8_continuation_relation_v1(
        &f.input,
        f.tail.output(),
        actual_claims(&f),
        &mut budget,
    )
    .err()
    .unwrap();
    assert!(matches!(error, Error::Resource(Resource::Storage(_))));
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.work(), 0);
}

#[test]
fn pass_length_preflight_is_bounded_even_for_long_external_claim() {
    let f = fixture(&effects());
    let long = "x".repeat(64 * 1024);
    for name in ["", long.as_str()] {
        let mut work = Work::new(3);
        let mut budget = Budget::new(&mut work, STORAGE);
        let floor = FLOOR + f.backing + long.capacity();
        budget.reserve_storage(floor).unwrap();
        let mut claims = actual_claims(&f);
        claims.pass_name = name;
        assert!(matches!(
            check_canonical_policy8_continuation_relation_v1(
                &f.input,
                f.tail.output(),
                claims,
                &mut budget,
            ),
            Err(Error::PassIdentity)
        ));
        assert_eq!(budget.work(), 3);
        assert_eq!(budget.storage(), floor);
    }
}

struct Dropped(Arc<AtomicBool>);
impl Drop for Dropped {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}
#[test]
fn scope_success_error_and_panic_keep_work_and_drop_owned_scratch() {
    for exit in 0..3 {
        let mut work = Work::new(5);
        let mut budget = Budget::new(&mut work, 41);
        budget.reserve_storage(11).unwrap();
        budget.charge_work(2).unwrap();
        let dropped = Arc::new(AtomicBool::new(false));
        let ledger = budget.work_ledger_identity_v1();
        let result = scoped(&mut budget, |budget| {
            budget.reserve_storage(30)?;
            budget.charge_work(3)?;
            let _scratch = Dropped(dropped.clone());
            match exit {
                0 => Ok(7),
                1 => Err(Error::PassIdentity),
                _ => panic!("test-only callback panic"),
            }
        });
        match exit {
            0 => assert_eq!(result.unwrap(), 7),
            1 => assert!(matches!(result, Err(Error::PassIdentity))),
            _ => assert!(matches!(result, Err(Error::Panicked))),
        }
        assert!(dropped.load(Ordering::SeqCst));
        assert_eq!(budget.storage(), 11);
        assert_eq!(budget.peak_storage(), 41);
        assert_eq!(budget.work(), 5);
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
}

#[test]
fn scope_refuses_undercut_and_never_refunds_a_foreign_ledger() {
    let mut work = Work::new(10);
    let mut budget = Budget::new(&mut work, 50);
    budget.reserve_storage(11).unwrap();
    let result = scoped(&mut budget, |budget| {
        budget.release_storage(1)?;
        Ok(())
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    assert_eq!(budget.storage(), 10);
    let foreign = Box::leak(Box::new(Work::new(10)));
    let result = scoped(&mut budget, |budget| {
        *budget = Budget::new(foreign, 100);
        budget.reserve_storage(37)?;
        budget.charge_work(4)?;
        Ok(())
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    assert_eq!(budget.storage(), 37);
    assert_eq!(budget.work(), 4);
}

struct HostilePayload;
impl Drop for HostilePayload {
    fn drop(&mut self) {
        panic!("test-only panic payload destructor");
    }
}
#[test]
fn hostile_panic_payload_is_destroyed_only_after_exact_cleanup() {
    let mut work = Work::new(10);
    let mut budget = Budget::new(&mut work, 50);
    budget.reserve_storage(11).unwrap();
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _: Result<(), Error> = scoped(&mut budget, |budget| {
            budget.reserve_storage(30)?;
            budget.charge_work(7)?;
            panic_any(HostilePayload)
        });
    }));
    assert!(result.is_err());
    assert_eq!(budget.storage(), 11);
    assert_eq!(budget.work(), 7);
    assert_eq!(budget.peak_storage(), 41);
}

struct RejectedResult;
impl Drop for RejectedResult {
    fn drop(&mut self) {
        panic!("test-only rejected result destructor");
    }
}
#[test]
fn accounting_refusal_keeps_rejected_result_destructor_panic_contained() {
    let mut work = Work::new(10);
    let mut budget = Budget::new(&mut work, 50);
    budget.reserve_storage(11).unwrap();
    let result = scoped(&mut budget, |budget| {
        budget.release_storage(1)?;
        budget.charge_work(4)?;
        Ok(RejectedResult)
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    assert_eq!(budget.storage(), 10);
    assert_eq!(budget.work(), 4);
}
