//! Pure transfer accounting and error tests, not fabricated signed-source owners.
use super::*;
use std::{cell::Cell, rc::Rc};

struct DropCount(Rc<Cell<usize>>);
impl Drop for DropCount {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

fn is_accounting(error: &ProductionPipelineError) -> bool {
    matches!(
        error,
        ProductionPipelineError::CheckedOutputPolicy6Stage(
            CheckedOutputPolicy6StageErrorV1::Resource(Resource::Accounting)
        )
    )
}

#[test]
fn fixed_native_receipt_carries_each_original_header_and_only_charges_growth() {
    let wrapper = std::mem::size_of::<FixedNativeCheckedOutputProductionCompilationPolicy6V1>()
        - std::mem::size_of::<NativeOwner>();
    assert!(wrapper > 0);
    for policy in [SourcePolicy::RawEmpty, SourcePolicy::UnitLocal] {
        let carried = additional_header(policy).unwrap();
        for required in [wrapper, carried, carried + 13] {
            let (floor, extra, receipt) =
                finish_receipt(100 + carried, 100, 151, 51, required).unwrap();
            assert_eq!(extra, required.saturating_sub(carried));
            assert_eq!(receipt.retained_storage(), 51 + extra);
            assert_eq!(floor, 151 + carried.max(required));
            assert!(floor >= 151 + required);
        }
    }
    for values in [(99, 100, 151, 51, 8), (108, 100, 150, 51, 8)] {
        assert!(is_accounting(
            &finish_receipt(values.0, values.1, values.2, values.3, values.4).unwrap_err()
        ));
    }
    assert!(matches!(
        finish_receipt(usize::MAX, usize::MAX, usize::MAX, 0, 1),
        Err(ProductionPipelineError::CheckedOutputPolicy6Stage(
            CheckedOutputPolicy6StageErrorV1::Resource(Resource::Arithmetic)
        ))
    ));
}

#[test]
fn fixed_native_transfer_checks_prepaid_floor_before_work_or_owned_stage_entry() {
    let dropped = Rc::new(Cell::new(0));
    let marker = DropCount(dropped.clone());
    let entered = Cell::new(false);
    let mut work = Work::new(TRANSFER_WORK);
    let mut budget = Budget::new(&mut work, 100);
    budget.reserve_storage(40).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result: Result<(), _> = transfer(41, &mut budget, |_, _, _| {
        let _marker = marker;
        entered.set(true);
        Ok(())
    });
    assert!(is_accounting(&result.unwrap_err()));
    assert!(!entered.get());
    assert_eq!(dropped.get(), 1);
    assert_eq!(budget.work(), 0);
    assert_eq!(budget.storage(), 40);
    assert!(budget.work_ledger_identity_v1() == ledger);
}

#[test]
fn fixed_native_transfer_exact_receipt_boundaries_preserve_unrelated_reserved_floor() {
    // These are logical test values, not source/proof/native owners.
    const OWNER: usize = 108;
    const UNRELATED: usize = 17;
    const SOURCE: usize = 31;
    const WORKER: usize = 20;
    const EXTRA: usize = 5;
    let entry = OWNER + UNRELATED;
    let peak = entry + SOURCE + WORKER + EXTRA;
    for (work_limit, storage_limit, succeeds) in [
        (TRANSFER_WORK, peak, true),
        (TRANSFER_WORK - 1, peak, false),
        (TRANSFER_WORK, peak - 1, false),
        (TRANSFER_WORK, entry + SOURCE - 1, false),
        (TRANSFER_WORK, entry + SOURCE + WORKER - 1, false),
    ] {
        let dropped = Rc::new(Cell::new(0));
        let marker = DropCount(dropped.clone());
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(entry).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = transfer(OWNER, &mut budget, |budget, ledger, slot| {
            let marker = marker;
            reserve_current(budget, ledger, slot, SOURCE)?;
            reserve_current(budget, ledger, slot, WORKER)?;
            let (floor, extra, receipt) = finish_receipt(OWNER, 100, 151, 51, 13)?;
            assert_eq!(extra, EXTRA);
            reserve_current(budget, ledger, slot, extra)?;
            Ok((marker, floor, receipt))
        });
        assert_eq!(result.is_ok(), succeeds);
        assert_eq!(budget.storage(), entry);
        assert!(budget.work_ledger_identity_v1() == ledger);
        if succeeds {
            let (marker, floor, receipt) = result.unwrap();
            assert_eq!(dropped.get(), 0);
            assert_eq!(budget.peak_storage(), peak);
            assert_eq!(floor, OWNER + receipt.retained_storage());
            // The returned receipt is not silently left reserved by the scope.
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            assert_eq!(budget.storage(), UNRELATED + floor);
            drop(marker);
            budget.release_storage(receipt.retained_storage()).unwrap();
            assert_eq!(budget.storage(), entry);
        } else if work_limit < TRANSFER_WORK {
            assert!(
                matches!(result, Err(ProductionPipelineError::CheckedOutputPolicy6Stage(
                CheckedOutputPolicy6StageErrorV1::NativeWorker(error)
            )) if matches!(*error, NativeOutputHandoffErrorV1::Resource(Resource::Work(_))))
            );
        } else {
            assert!(matches!(
                result,
                Err(ProductionPipelineError::CheckedOutputPolicy6Stage(
                    CheckedOutputPolicy6StageErrorV1::Resource(Resource::Storage(_))
                ))
            ));
            assert!(budget.failed_storage().is_some());
        }
        assert_eq!(dropped.get(), 1);
    }
}

#[test]
fn fixed_native_transfer_preserves_original_unsigned_source_error_and_cleans_partial_storage() {
    use crate::production_native_source_lineage_v1::NativeSourceLineageErrorV1;
    let dropped = Rc::new(Cell::new(0));
    let marker = DropCount(dropped.clone());
    let expected = ProductionPipelineError::CheckedOutputPolicy6Stage(
        CheckedOutputPolicy6StageErrorV1::NativeSource(Box::new(
            NativeSourceLineageErrorV1::MissingSignedRankedReceipt { root: 7 },
        )),
    );
    let expected_display = expected.to_string();
    let mut work = Work::new(TRANSFER_WORK * 2);
    let mut budget = Budget::new(&mut work, 100);
    budget.reserve_storage(41).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result: Result<(), _> = transfer(41, &mut budget, |budget, ledger, slot| {
        let _marker = marker;
        reserve_current(budget, ledger, slot, 17)?;
        Err(expected)
    });
    let error = result.unwrap_err();
    assert_eq!(error.to_string(), expected_display);
    assert!(
        matches!(error, ProductionPipelineError::CheckedOutputPolicy6Stage(
        CheckedOutputPolicy6StageErrorV1::NativeSource(error)
    ) if matches!(*error, NativeSourceLineageErrorV1::MissingSignedRankedReceipt { root: 7 }))
    );
    assert_eq!(dropped.get(), 1);
    assert_eq!(budget.storage(), 41);
    assert_eq!(budget.work(), TRANSFER_WORK);
    assert!(budget.work_ledger_identity_v1() == ledger);
}

#[test]
fn fixed_native_transfer_drops_panicked_stage_before_scope_returns_and_retains_work() {
    let dropped = Rc::new(Cell::new(0));
    let marker = DropCount(dropped.clone());
    let mut work = Work::new(TRANSFER_WORK + 3);
    let mut budget = Budget::new(&mut work, 100);
    budget.reserve_storage(41).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result: Result<(), _> = transfer(41, &mut budget, |budget, ledger, slot| {
        let _marker = marker;
        reserve_current(budget, ledger, slot, 17)?;
        budget.charge_work(3).map_err(resource)?;
        std::panic::panic_any(0x439_u32)
    });
    assert!(
        matches!(result, Err(ProductionPipelineError::CheckedOutputPolicy6Stage(
        CheckedOutputPolicy6StageErrorV1::NativeWorker(error)
    )) if matches!(*error, NativeOutputHandoffErrorV1::Panicked))
    );
    assert_eq!(dropped.get(), 1);
    assert_eq!(budget.storage(), 41);
    assert_eq!(budget.work(), TRANSFER_WORK + 3);
    assert!(budget.work_ledger_identity_v1() == ledger);
}

#[test]
fn fixed_native_transfer_refuses_replaced_current_ledger_before_receipt_charge() {
    let mut first_work = Work::new(TRANSFER_WORK);
    let mut second_work = Work::new(TRANSFER_WORK);
    let mut budget = Budget::new(&mut first_work, 100);
    let mut foreign = Budget::new(&mut second_work, 100);
    budget.reserve_storage(41).unwrap();
    let first = budget.work_ledger_identity_v1();
    let second = foreign.work_ledger_identity_v1();
    let result: Result<(), _> = transfer(41, &mut budget, |budget, ledger, slot| {
        std::mem::swap(budget, &mut foreign);
        reserve_current(budget, ledger, slot, 17)
    });
    assert!(
        matches!(result, Err(ProductionPipelineError::CheckedOutputPolicy6Stage(
        CheckedOutputPolicy6StageErrorV1::NativeWorker(error)
    )) if matches!(*error, NativeOutputHandoffErrorV1::Resource(Resource::Accounting)))
    );
    assert!(budget.work_ledger_identity_v1() == second);
    assert_eq!(budget.storage(), 0);
    assert_eq!(budget.work(), 0);
    assert!(foreign.work_ledger_identity_v1() == first);
    assert_eq!(foreign.storage(), 41);
    assert_eq!(foreign.work(), TRANSFER_WORK);
}
