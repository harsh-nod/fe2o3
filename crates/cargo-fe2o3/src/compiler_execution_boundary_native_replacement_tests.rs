//! Accounting components only; these do not fabricate compiler/readiness/proof owners.
use super::super::{
    ContinuationError, Failure, ParentDurableConditionalArtifact,
    ParentPreparedConditionalArtifact, ParentPublishedConditionalArtifact,
};
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

const INPUT: usize = 97;
const RETIRED: usize = 31;
const OUTPUT: usize = 23;
const HEADER: usize = 11;
const PEAK: usize = INPUT + FRAME + OUTPUT;

#[test]
fn replacement_retires_only_consumed_storage_after_exact_success() {
    let mut work = Work::new(16);
    let mut b = Budget::new(&mut work, PEAK);
    b.reserve_storage(INPUT).unwrap();
    let ledger = b.work_ledger_identity_v1();
    let replacement = Replacement::begin(RETIRED, &mut b).unwrap();
    assert_eq!(b.storage(), INPUT + FRAME);
    replacement.reserve(OUTPUT, &mut b).unwrap();
    assert_eq!(b.storage(), PEAK);
    replacement.finish(OUTPUT, HEADER, &mut b).unwrap();
    assert_eq!(b.storage(), INPUT - RETIRED + OUTPUT + HEADER);
    assert_eq!(b.peak_storage(), PEAK);
    assert_eq!(b.work(), 16);
    assert!(b.work_ledger_identity_v1() == ledger);
}

#[test]
fn replacement_retains_callback_output_after_only_policy_view_scratch_drops() {
    const VIEW: usize = 47;
    for (work_limit, storage_limit) in [(16, PEAK + VIEW), (15, PEAK + VIEW), (16, PEAK + VIEW - 1)]
    {
        let mut work = Work::new(work_limit);
        let mut b = Budget::new(&mut work, storage_limit);
        b.reserve_storage(INPUT).unwrap();
        let ledger = b.work_ledger_identity_v1();
        let replacement = Replacement::begin(RETIRED, &mut b).unwrap();
        b.reserve_storage(VIEW).unwrap();
        let reserved = b.reserve_storage(OUTPUT);
        if storage_limit < PEAK + VIEW {
            assert!(matches!(reserved, Err(Resource::Storage(_))));
            assert_eq!(b.storage(), INPUT + FRAME + VIEW);
            continue;
        }
        reserved.unwrap();
        b.release_storage(VIEW).unwrap();
        assert_eq!(b.storage(), PEAK);
        let result = replacement.finish(OUTPUT, HEADER, &mut b);
        if work_limit < 16 {
            assert!(matches!(result, Err(Resource::Work(_))));
            assert_eq!(b.storage(), PEAK);
        } else {
            result.unwrap();
            assert_eq!(b.storage(), INPUT - RETIRED + OUTPUT + HEADER);
        }
        assert!(b.work_ledger_identity_v1() == ledger);
    }
}

#[test]
fn replacement_entry_refuses_short_floor_work_and_storage_before_retirement() {
    for (input, quota, limit) in [
        (RETIRED - 1, 16, PEAK),
        (INPUT, 7, PEAK),
        (INPUT, 16, INPUT + FRAME - 1),
    ] {
        let mut work = Work::new(quota);
        let mut b = Budget::new(&mut work, limit);
        b.reserve_storage(input).unwrap();
        assert!(Replacement::begin(RETIRED, &mut b).is_err());
        assert_eq!(b.storage(), input);
        if quota == 7 {
            assert_eq!(b.failed_work(), Some(8));
        }
        if limit < INPUT + FRAME {
            assert_eq!(b.failed_storage(), Some(INPUT + FRAME));
        }
    }
}

#[test]
fn replacement_reservation_denial_keeps_the_original_owner_and_scratch() {
    let mut work = Work::new(16);
    let mut b = Budget::new(&mut work, PEAK - 1);
    b.reserve_storage(INPUT).unwrap();
    let replacement = Replacement::begin(RETIRED, &mut b).unwrap();
    assert!(matches!(
        replacement.reserve(OUTPUT, &mut b),
        Err(Resource::Storage(_))
    ));
    assert_eq!(b.storage(), INPUT + FRAME);
    assert_eq!(b.failed_storage(), Some(PEAK));
}

#[test]
fn replacement_finish_work_denial_keeps_all_coexisting_charges() {
    let mut work = Work::new(15);
    let mut b = Budget::new(&mut work, PEAK);
    b.reserve_storage(INPUT).unwrap();
    let replacement = Replacement::begin(RETIRED, &mut b).unwrap();
    replacement.reserve(OUTPUT, &mut b).unwrap();
    assert!(matches!(
        replacement.finish(OUTPUT, HEADER, &mut b),
        Err(Resource::Work(_))
    ));
    assert_eq!(b.storage(), PEAK);
    assert_eq!(b.work(), 8);
    assert_eq!(b.failed_work(), Some(16));
}

#[test]
fn replacement_rejects_changed_recovery_or_revalidation_reservations() {
    for after_reserve in [false, true] {
        for missing in [false, true] {
            let mut work = Work::new(16);
            let mut b = Budget::new(&mut work, PEAK + 1);
            b.reserve_storage(INPUT).unwrap();
            let replacement = Replacement::begin(RETIRED, &mut b).unwrap();
            if after_reserve {
                replacement.reserve(OUTPUT, &mut b).unwrap();
            }
            if missing {
                b.release_storage(1).unwrap();
            } else {
                b.reserve_storage(1).unwrap();
            }
            let before = b.storage();
            let result = if after_reserve {
                replacement.finish(OUTPUT, HEADER, &mut b)
            } else {
                replacement.reserve(OUTPUT, &mut b)
            };
            assert_eq!(result, Err(Resource::Accounting));
            assert_eq!(b.storage(), before);
        }
    }
}

#[test]
fn replacement_rejects_replaced_work_ledger_before_reserve_and_finish() {
    for after_reserve in [false, true] {
        let mut original = Work::new(16);
        let mut other = Work::new(16);
        let mut b = Budget::new(&mut original, PEAK);
        b.reserve_storage(INPUT).unwrap();
        let replacement = Replacement::begin(RETIRED, &mut b).unwrap();
        if after_reserve {
            replacement.reserve(OUTPUT, &mut b).unwrap();
        }
        let stored = b.storage();
        b = Budget::new(&mut other, PEAK);
        b.reserve_storage(stored).unwrap();
        let result = if after_reserve {
            replacement.finish(OUTPUT, HEADER, &mut b)
        } else {
            replacement.reserve(OUTPUT, &mut b)
        };
        assert_eq!(result, Err(Resource::Accounting));
        assert_eq!(b.storage(), stored);
    }
}

#[test]
fn replacement_rejects_same_ledger_moved_to_another_budget_address() {
    let mut work = Work::new(16);
    let mut b = Budget::new(&mut work, PEAK);
    b.reserve_storage(INPUT).unwrap();
    let replacement = Replacement::begin(RETIRED, &mut b).unwrap();
    let mut moved = Box::new(b);
    assert_eq!(
        replacement.reserve(OUTPUT, &mut moved),
        Err(Resource::Accounting)
    );
    assert_eq!(moved.storage(), INPUT + FRAME);
}

#[test]
fn replacement_overflow_and_unpaid_header_never_retire_inputs() {
    for overflow in [false, true] {
        let mut work = Work::new(16);
        let mut b = Budget::new(&mut work, PEAK);
        b.reserve_storage(INPUT).unwrap();
        let replacement = Replacement::begin(RETIRED, &mut b).unwrap();
        replacement.reserve(OUTPUT, &mut b).unwrap();
        let error = if overflow {
            replacement.finish(usize::MAX, HEADER, &mut b).unwrap_err()
        } else {
            replacement.finish(OUTPUT, FRAME + 1, &mut b).unwrap_err()
        };
        assert_eq!(
            error,
            if overflow {
                Resource::Arithmetic
            } else {
                Resource::Accounting
            }
        );
        assert_eq!(b.storage(), PEAK);
    }
}

#[test]
fn replacement_drop_after_refusal_or_unwind_does_not_refund() {
    for unwind in [false, true] {
        let mut work = Work::new(16);
        let mut b = Budget::new(&mut work, PEAK);
        b.reserve_storage(INPUT).unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let replacement = Replacement::begin(RETIRED, &mut b)?;
            replacement.reserve(OUTPUT, &mut b)?;
            if unwind {
                panic!("component-only validation unwind");
            }
            Err::<(), _>(Resource::Accounting)
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
        assert_eq!(b.storage(), PEAK);
        assert_eq!(b.work(), 8);
    }
}

#[test]
fn replacement_preserves_prior_work_and_denial_history() {
    let mut work = Work::new(21);
    let mut b = Budget::new(&mut work, PEAK);
    b.reserve_storage(INPUT).unwrap();
    b.charge_work(5).unwrap();
    assert!(b.reserve_storage(usize::MAX).is_err());
    assert!(b.charge_work(usize::MAX).is_err());
    let denials = (b.failed_work(), b.failed_storage());
    let replacement = Replacement::begin(RETIRED, &mut b).unwrap();
    replacement.reserve(OUTPUT, &mut b).unwrap();
    replacement.finish(OUTPUT, HEADER, &mut b).unwrap();
    assert_eq!((b.failed_work(), b.failed_storage()), denials);
    assert_eq!(b.work(), 21);
}

#[test]
fn continuation_errors_cannot_expose_nested_refundable_resources() {
    for error in [
        ContinuationError::from(Resource::Accounting),
        ContinuationError::from(Failure::Resource(Resource::Arithmetic)),
        ContinuationError::from(
            fe2o3_hsaco_finalize::ConditionalWorkerOutputErrorV5::Resource(Resource::Accounting),
        ),
        ContinuationError::from(
            fe2o3_runtime_protocol::ConditionalWorkerReadinessErrorV5::Resource(
                Resource::Accounting,
            ),
        ),
    ] {
        assert!(std::error::Error::source(&error).is_none());
        assert!(!error.to_string().is_empty());
    }
}

#[test]
fn replacement_frame_covers_actual_parent_headers() {
    assert!(FRAME >= ParentPreparedConditionalArtifact::HEADER);
    assert!(FRAME >= ParentDurableConditionalArtifact::HEADER);
    assert!(FRAME >= ParentPublishedConditionalArtifact::HEADER);
}
