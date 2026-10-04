//! Public-adapter boundaries from the empty graph's actual structural census.
//! Input admission is separate; every phase inside the adapter shares one meter.

use super::*;
use fe2o3_pliron::{
    OperationHandle, PlironOptimizationErrorV12, PlironOptimizationPassReportV1,
    PlironOptimizationReportV1,
};
use std::mem::size_of;

// Module("m"): 37 wire bytes; root tree3; no functions/blocks/values/edges.
// Native import: census37 + envelope1576 + digest94 + witness4.
// Occurrences: census4 + prepaid512. Seven-pass execution:27367936.
// Extraction: census40 + envelope1576 + connected output admission294 +
// digest94 + endpoint1 + roster4 + map census3 + map finish512.
// Checked finish: pointer1 + entry1 + inventories4 + checker7 + input copy37.
const COMPLETE: usize = 27_372_737;
const BEFORE_HISTORY_COPY: usize = 27_372_700;
const BEFORE_EXECUTION_RESERVE: usize = 27_370_163;

// Pinned 64-bit logical layout: OperationHandle, seven usize census fields,
// and one empty Vec header. No private observer or measured receipt is used.
fn witness_header() -> usize {
    size_of::<OperationHandle>() + 7 * size_of::<usize>() + size_of::<Vec<usize>>()
}

fn exact_peak_payload() -> usize {
    // Import6720 + occurrence capture11520 + execution persistent272176 +
    // execution temporary528992, with the witness and seven-pass report live.
    819_408
        + witness_header()
        + size_of::<PlironOptimizationReportV1>()
        + 7 * size_of::<PlironOptimizationPassReportV1>()
}

fn seed_history(budget: &mut Budget<'_>, seeded: bool) {
    if seeded {
        assert!(budget.charge_work(usize::MAX).is_err());
        assert!(budget.reserve_storage(usize::MAX).is_err());
    }
}

#[test]
#[cfg(target_pointer_width = "64")]
fn public_empty_adapter_has_exact_and_one_under_complete_work() {
    assert_eq!(
        [
            37, 1576, 94, 4, 4, 512, 27_367_936, 40, 1576, 294, 94, 1, 4, 3, 512, 50
        ]
        .into_iter()
        .sum::<usize>(),
        COMPLETE
    );
    for short in [false, true] {
        for seeded in [false, true] {
            let input = admit(&Module::new("m"));
            assert_eq!(input.0.canonical().canonical_bytes().len(), 37);
            assert_eq!(input.1, size_of::<Owner>() + 38);
            let floor = PREFIX + input.1;
            let limit = PRIOR_WORK + COMPLETE - usize::from(short);
            let mut work = Work::new(limit);
            let mut budget = Budget::new(&mut work, STORAGE);
            budget.charge_work(PRIOR_WORK).unwrap();
            budget.reserve_storage(floor).unwrap();
            seed_history(&mut budget, seeded);
            let result = optimize_checked_canonical_kernel_ir_v1(&input.0, &mut budget);
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.peak_storage(), floor + exact_peak_payload());
            assert_eq!(budget.failed_storage(), seeded.then_some(usize::MAX));
            if short {
                assert!(matches!(
                    result,
                    Err(KernelIrCheckedOptimizationErrorV1::Check(
                        KirCheckedNeutralOptimizationErrorV1::Resource(Resource::Work(error))
                    )) if error.actual() == PRIOR_WORK + COMPLETE && error.limit() == limit
                ));
                assert_eq!(budget.work(), PRIOR_WORK + BEFORE_HISTORY_COPY);
            } else {
                let checked = result.unwrap();
                let retained = checked.storage().retained_storage();
                budget.reserve_storage(retained).unwrap();
                assert_eq!(budget.work(), PRIOR_WORK + COMPLETE);
                assert_eq!(
                    checked.owner().canonical().canonical_bytes(),
                    input.0.canonical().canonical_bytes()
                );
                assert_eq!(
                    checked.native_input_audit_bytes(),
                    input.0.canonical().canonical_bytes()
                );
                assert_eq!(checked.report().passes().len(), 7);
                assert!(checked.report().passes().iter().all(|pass| !pass.changed()));
                assert!(checked.map().matches_execution(checked.report()));
                assert!(!checked.grants_authority());
                drop(checked);
                budget.release_storage(retained).unwrap();
            }
            assert_eq!(budget.storage(), floor);
            let input_storage = input.1;
            drop(input);
            budget.release_storage(input_storage).unwrap();
            assert_eq!(budget.storage(), PREFIX);
            let failed = if seeded {
                Some(usize::MAX)
            } else {
                short.then_some(PRIOR_WORK + COMPLETE)
            };
            assert_eq!(work.failed_work(), failed);
        }
    }
}

#[test]
#[cfg(target_pointer_width = "64")]
fn public_empty_adapter_has_exact_and_one_under_complete_storage() {
    for short in [false, true] {
        for seeded in [false, true] {
            let input = admit(&Module::new("m"));
            assert_eq!(input.0.canonical().canonical_bytes().len(), 37);
            assert_eq!(input.1, size_of::<Owner>() + 38);
            let floor = PREFIX + input.1;
            let exact = floor + exact_peak_payload();
            let limit = exact - usize::from(short);
            let mut work = Work::new(PRIOR_WORK + COMPLETE);
            let mut budget = Budget::new(&mut work, limit);
            budget.charge_work(PRIOR_WORK).unwrap();
            budget.reserve_storage(floor).unwrap();
            seed_history(&mut budget, seeded);
            let result = optimize_checked_canonical_kernel_ir_v1(&input.0, &mut budget);
            assert_eq!(budget.storage(), floor);
            if short {
                assert!(matches!(
                    result,
                    Err(KernelIrCheckedOptimizationErrorV1::Observation(
                        KirNeutralOptimizationErrorV1::Execution(
                            PlironOptimizationErrorV12::Resources(Resource::Storage(error))
                        )
                    )) if error.actual() == exact && error.limit() == limit
                ));
                // The complete execution charge precedes its atomic reservation.
                // No pass mutation starts, and all imported/capture owners drop.
                assert_eq!(budget.work(), PRIOR_WORK + BEFORE_EXECUTION_RESERVE);
                assert_eq!(
                    budget.peak_storage(),
                    floor + 6720 + witness_header() + 11_520
                );
            } else {
                let checked = result.unwrap();
                let retained = checked.storage().retained_storage();
                budget.reserve_storage(retained).unwrap();
                assert_eq!(budget.work(), PRIOR_WORK + COMPLETE);
                assert_eq!(budget.peak_storage(), exact);
                assert_eq!(checked.owner().module(), input.0.module());
                assert!(!checked.grants_authority());
                drop(checked);
                budget.release_storage(retained).unwrap();
            }
            let failed = if seeded {
                Some(usize::MAX)
            } else {
                short.then_some(exact)
            };
            assert_eq!(budget.failed_storage(), failed);
            assert_eq!(budget.storage(), floor);
            let input_storage = input.1;
            drop(input);
            budget.release_storage(input_storage).unwrap();
            assert_eq!(budget.storage(), PREFIX);
            assert_eq!(work.failed_work(), seeded.then_some(usize::MAX));
        }
    }
}
