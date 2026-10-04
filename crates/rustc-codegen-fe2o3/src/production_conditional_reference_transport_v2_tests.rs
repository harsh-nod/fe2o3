//! Inert copy/accounting tests. No Request, proof, signer or execution is made.
use super::*;
use crate::production_ranked_projection_v1::guarded_source_progress_v1::resources;
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as OwnedBudget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use std::mem::size_of;

const FLOOR: usize = 7;
const WORK: usize = 3;
const CPU: &[u8] = b"inert, not a CPU frame";

fn signature() -> InertFunctionalRefinementReceiptSignatureV2 {
    InertFunctionalRefinementReceiptSignatureV2::from_untrusted_parts(
        [0; fe2o3_functional_proof::FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2],
        [0; 32],
    )
}

fn staging() -> Staging {
    Staging {
        receipt: [1; 32],
        effect: [2; 32],
        signer: [3; 32],
        execution: [4; 32],
        toolchain: [[5; 32]; 5],
    }
}

fn extent() -> usize {
    size_of::<ConditionalReplayTransportV2>() + CPU.len() + size_of::<Staging>()
}

fn account(work: usize, storage: usize) -> OwnedBudget {
    let mut account = OwnedBudget::new(Work::new(work), storage);
    account.with_budget(|budget| {
        budget.charge_work(WORK).unwrap();
        budget.reserve_storage(FLOOR).unwrap();
    });
    account
}

fn copy(budget: &mut Budget<'_>) -> Result<ConditionalReplayTransportV2, Error> {
    copy_inert_transport_v2(CPU, [staging()].into_iter(), signature(), budget)
}

#[test]
fn conditional_transport_exact_copy_retains_original_account_and_extent() {
    let mut original = account(WORK + extent() + 1, FLOOR + extent());
    original.with_budget(|budget| {
        let ledger = budget.work_ledger_identity_v1();
        let retained = copy(budget).unwrap();
        assert_eq!(retained.cpu_input_v1(), CPU);
        assert_eq!(retained.staging_commitments_v1(), &[staging()]);
        assert_eq!(retained.formula_receipt_v2(), &signature());
        assert_eq!(retained.retained_storage_v2(), extent());
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(budget.storage(), FLOOR + extent());
        assert_eq!(budget.work(), WORK + extent() + 1);
        drop(retained);
        budget.release_storage(extent()).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    });
}

#[test]
fn conditional_transport_one_short_work_preserves_floor_and_denial() {
    let mut original = account(WORK + extent(), FLOOR + extent());
    original.with_budget(|budget| {
        let ledger = budget.work_ledger_identity_v1();
        assert!(copy(budget).is_err());
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.work(), WORK);
        assert_eq!(budget.failed_work(), Some(WORK + extent() + 1));
    });
}

#[test]
fn conditional_transport_one_short_storage_keeps_spent_work_and_denial() {
    let mut original = account(WORK + extent() + 1, FLOOR + extent() - 1);
    original.with_budget(|budget| {
        assert!(copy(budget).is_err());
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.work(), WORK + extent() + 1);
        assert_eq!(budget.failed_storage(), Some(FLOOR + extent()));
    });
}

#[test]
fn conditional_transport_comparison_checks_cpu_all_staging_fields_wire_and_key() {
    let mut original = account(1_000_000, 1_000_000);
    original.with_budget(|budget| {
        let previous = copy(budget).unwrap();
        let equal = copy(budget).unwrap();
        equal.require_same_v2(&previous, budget).unwrap();
        drop(equal);
        budget.release_storage(extent()).unwrap();
        for mutation in 0..12 {
            let mut changed = copy(budget).unwrap();
            match mutation {
                0 => changed.cpu_input[0] ^= 1,
                1 => changed.staging[0].receipt[0] ^= 1,
                2 => changed.staging[0].effect[0] ^= 1,
                3 => changed.staging[0].signer[0] ^= 1,
                4 => changed.staging[0].execution[0] ^= 1,
                5..=9 => changed.staging[0].toolchain[mutation - 5][0] ^= 1,
                10 => {
                    let mut wire = *signature().wire();
                    wire[0] = 1;
                    changed.formula_receipt =
                        InertFunctionalRefinementReceiptSignatureV2::from_untrusted_parts(
                            wire, [0; 32],
                        );
                }
                11 => {
                    changed.formula_receipt =
                        InertFunctionalRefinementReceiptSignatureV2::from_untrusted_parts(
                            *signature().wire(),
                            [1; 32],
                        );
                }
                _ => unreachable!(),
            }
            assert!(changed.require_same_v2(&previous, budget).is_err());
            drop(changed);
            budget.release_storage(extent()).unwrap();
        }
        drop(previous);
        budget.release_storage(extent()).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    });
}

#[test]
fn conditional_transport_comparison_work_denial_never_accepts_equal_bytes() {
    let comparison = CPU.len()
        + size_of::<Staging>()
        + size_of::<InertFunctionalRefinementReceiptSignatureV2>()
        + 1;
    let used = WORK + 2 * (extent() + 1);
    let mut original = account(used + comparison - 1, FLOOR + 2 * extent());
    original.with_budget(|budget| {
        let first = copy(budget).unwrap();
        let second = copy(budget).unwrap();
        assert!(second.require_same_v2(&first, budget).is_err());
        assert_eq!(budget.failed_work(), Some(used + comparison));
        assert_eq!(budget.storage(), FLOOR + 2 * extent());
        drop((first, second));
        budget.release_storage(2 * extent()).unwrap();
    });
}

#[test]
fn conditional_transport_oversized_roster_refuses_before_iteration_or_allocation() {
    let count = fe2o3_verifier::portable_reference_v1::codec::MAX_NATIVE_CPU_INPUT_BYTES_V1
        / size_of::<Staging>()
        + 1;
    let mut original = account(WORK, FLOOR);
    original.with_budget(|budget| {
        let rows = std::iter::repeat_n(staging(), count).map(|_| panic!("oversized iteration"));
        assert!(copy_inert_transport_v2(&[], rows, signature(), budget).is_err());
        assert_eq!((budget.work(), budget.storage()), (WORK, FLOOR));
        assert_eq!(
            (budget.failed_work(), budget.failed_storage()),
            (None, None)
        );
    });
}

#[test]
fn conditional_transport_copy_unwind_drops_allocations_before_refund() {
    let mut original = account(1_000_000, 1_000_000);
    original.with_budget(|budget| {
        let rows = [staging()]
            .into_iter()
            .map(|_| panic!("inert iterator unwind"));
        assert!(copy_inert_transport_v2(CPU, rows, signature(), budget).is_err());
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.work(), WORK + extent() + 1);
    });
}

#[test]
fn conditional_transport_provisional_owner_never_returns_after_late_error_or_unwind() {
    for unwind in [false, true] {
        let mut original = account(1_000_000, 1_000_000);
        original.with_budget(|budget| {
            let result: Result<ConditionalReplayTransportV2, Error> = resources::owned(
                budget,
                0,
                capture_error,
                || Error::UnsupportedReference("late inert unwind"),
                |budget| {
                    let _provisional = copy(budget)?;
                    if unwind {
                        panic!("late inert unwind");
                    }
                    Err(Error::UnsupportedReference("late inert postcheck"))
                },
            );
            assert!(result.is_err());
            assert_eq!(budget.storage(), FLOOR);
            assert_eq!(budget.work(), WORK + extent() + 1);
        });
    }
}

#[test]
fn conditional_transport_provisional_success_is_overridden_by_floor_damage() {
    let mut original = account(1_000_000, 1_000_000);
    original.with_budget(|budget| {
        let result = resources::owned(
            budget,
            0,
            capture_error,
            || capture_error(Resource::Accounting),
            |budget| {
                let provisional = copy(budget)?;
                budget.release_storage(1).map_err(capture_error)?;
                Ok((provisional, extent()))
            },
        );
        assert!(result.is_err());
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.work(), WORK + extent() + 1);
    });
}
