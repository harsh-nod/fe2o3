//! Accounting components only: no signed native facade can be fabricated here.
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

fn receipts() -> Receipts {
    let mut work = Work::new(1024);
    let mut budget = Budget::new(&mut work, 1024);
    let (receipts, storage) =
        Receipts::try_from_preimages_v1(b"kernel", b"formal", &mut budget).unwrap();
    assert_eq!(receipts.retained_storage(), storage.retained_storage());
    assert_eq!(budget.storage(), 0);
    receipts
}

#[test]
fn fixed_final_receipt_envelope_carries_header_without_recharging_payload() {
    let receipts = receipts();
    let bytes = receipts.retained_storage();
    let header =
        size_of::<FixedNativeFinalOutputProductionCompilationPolicy6V1>() - size_of::<FinalOwner>();
    assert!(header > 0);
    for carried in [0, header, header + 19] {
        let (required, branch) = receipt_floors(100 + carried, 100, bytes).unwrap();
        assert_eq!(required, 100 + carried + bytes);
        assert_eq!(branch, 100 + bytes);
        let (retained, extra, receipt) =
            finish_receipt(required, branch, branch + 13, 13, header).unwrap();
        assert_eq!(extra, header.saturating_sub(carried));
        assert_eq!(receipt.retained_storage(), 13 + extra);
        assert_eq!(retained, 100 + bytes + 13 + carried.max(header));
        assert_eq!(retained, required + receipt.retained_storage());
    }
    assert!(is_accounting(&receipt_floors(99, 100, bytes).unwrap_err()));
    assert!(matches!(
        receipt_floors(usize::MAX, 100, bytes),
        Err(ProductionPipelineError::CheckedOutputPolicy6Stage(
            CheckedOutputPolicy6StageErrorV1::Resource(Resource::Arithmetic)
        ))
    ));
}

#[test]
fn fixed_final_receipt_transfer_exact_and_short_limits_keep_both_incoming_owners_reserved() {
    let receipt_bytes = receipts().retained_storage();
    let (required, branch) = receipt_floors(108, 100, receipt_bytes).unwrap();
    let entry = required + 17;
    let peak = entry + 13 + 5;
    for (work_limit, storage_limit, succeeds) in [
        (TRANSFER_WORK, peak, true),
        (TRANSFER_WORK - 1, peak, false),
        (TRANSFER_WORK, peak - 1, false),
        (TRANSFER_WORK, entry + 12, false),
    ] {
        let receipts = receipts();
        let dropped = Rc::new(Cell::new(0));
        let marker = DropCount(dropped.clone());
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(entry).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = transfer(required, &mut budget, |budget, ledger, slot| {
            let retained = (receipts, marker);
            reserve_current(budget, ledger, slot, 13)?;
            let (floor, extra, receipt) = finish_receipt(required, branch, branch + 13, 13, 13)?;
            assert_eq!(extra, 5);
            reserve_current(budget, ledger, slot, extra)?;
            Ok((retained, floor, receipt))
        });
        assert_eq!(result.is_ok(), succeeds);
        assert_eq!(budget.storage(), entry);
        assert!(budget.work_ledger_identity_v1() == ledger);
        if succeeds {
            let (retained, floor, receipt) = result.unwrap();
            assert_eq!(dropped.get(), 0);
            assert_eq!(retained.0.retained_storage(), receipt_bytes);
            assert_eq!(budget.peak_storage(), peak);
            assert_eq!(receipt.retained_storage(), 18);
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            assert_eq!(budget.storage(), floor + 17);
            drop(retained);
            budget.release_storage(receipt.retained_storage()).unwrap();
            assert_eq!(budget.storage(), entry);
        } else if work_limit < TRANSFER_WORK {
            assert!(matches!(
                result,
                Err(ProductionPipelineError::CheckedOutputPolicy6Stage(
                    CheckedOutputPolicy6StageErrorV1::NativeWorker(error)
                )) if matches!(*error, NativeOutputHandoffErrorV1::Resource(Resource::Work(_)))
            ));
        } else {
            assert!(matches!(
                result,
                Err(ProductionPipelineError::CheckedOutputPolicy6Stage(
                    CheckedOutputPolicy6StageErrorV1::Resource(Resource::Storage(_))
                ))
            ));
        }
        assert_eq!(dropped.get(), 1);
    }
}

#[test]
fn fixed_final_receipt_transfer_checks_combined_floor_before_delegate() {
    let receipts = receipts();
    let (required, _) = receipt_floors(108, 100, receipts.retained_storage()).unwrap();
    let dropped = Rc::new(Cell::new(0));
    let marker = DropCount(dropped.clone());
    let entered = Cell::new(false);
    let mut work = Work::new(TRANSFER_WORK);
    let mut budget = Budget::new(&mut work, required);
    budget.reserve_storage(required - 1).unwrap();
    let result: Result<(), _> = transfer(required, &mut budget, |_, _, _| {
        let _owned = (receipts, marker);
        entered.set(true);
        Ok(())
    });
    assert!(is_accounting(&result.unwrap_err()));
    assert!(!entered.get());
    assert_eq!(dropped.get(), 1);
    assert_eq!(budget.storage(), required - 1);
    assert_eq!(budget.work(), 0);
}

#[test]
fn fixed_final_receipt_refusal_and_panic_drop_owned_inputs_before_return() {
    for panic in [false, true] {
        let receipts = receipts();
        let (required, _) = receipt_floors(108, 100, receipts.retained_storage()).unwrap();
        let dropped = Rc::new(Cell::new(0));
        let marker = DropCount(dropped.clone());
        let mut work = Work::new(TRANSFER_WORK + 3);
        let mut budget = Budget::new(&mut work, required + 17);
        budget.reserve_storage(required).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result: Result<(), _> = transfer(required, &mut budget, |budget, ledger, slot| {
            let _owned = (receipts, marker);
            reserve_current(budget, ledger, slot, 17)?;
            budget.charge_work(3).map_err(resource)?;
            if panic {
                std::panic::panic_any(0x445_u32);
            }
            Err(native_error(NativeOutputHandoffErrorV1::Mismatch(
                "final KernelIr subject",
            )))
        });
        assert!(matches!(
            result,
            Err(ProductionPipelineError::CheckedOutputPolicy6Stage(
                CheckedOutputPolicy6StageErrorV1::NativeWorker(error)
            )) if matches!(*error, NativeOutputHandoffErrorV1::Panicked) && panic
                || matches!(*error, NativeOutputHandoffErrorV1::Mismatch("final KernelIr subject")) && !panic
        ));
        assert_eq!(dropped.get(), 1);
        assert_eq!(budget.storage(), required);
        assert_eq!(budget.work(), TRANSFER_WORK + 3);
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
}

#[test]
fn fixed_final_receipt_transfer_drops_inputs_without_charging_a_replacement_ledger() {
    let receipts = receipts();
    let (required, _) = receipt_floors(108, 100, receipts.retained_storage()).unwrap();
    let dropped = Rc::new(Cell::new(0));
    let marker = DropCount(dropped.clone());
    let mut original_work = Work::new(TRANSFER_WORK);
    let mut foreign_work = Work::new(TRANSFER_WORK);
    let mut budget = Budget::new(&mut original_work, required + 17);
    let mut foreign = Budget::new(&mut foreign_work, required + 17);
    budget.reserve_storage(required).unwrap();
    foreign.reserve_storage(19).unwrap();
    let original_ledger = budget.work_ledger_identity_v1();
    let foreign_ledger = foreign.work_ledger_identity_v1();
    let result: Result<(), _> = transfer(required, &mut budget, |budget, ledger, slot| {
        let _owned = (receipts, marker);
        std::mem::swap(budget, &mut foreign);
        reserve_current(budget, ledger, slot, 17)
    });
    assert!(matches!(
        result,
        Err(ProductionPipelineError::CheckedOutputPolicy6Stage(
            CheckedOutputPolicy6StageErrorV1::NativeWorker(error)
        )) if matches!(*error, NativeOutputHandoffErrorV1::Resource(Resource::Accounting))
    ));
    assert_eq!(dropped.get(), 1);
    assert_eq!(budget.storage(), 19);
    assert_eq!(budget.work(), 0);
    assert!(budget.work_ledger_identity_v1() == foreign_ledger);
    assert_eq!(foreign.storage(), required);
    assert_eq!(foreign.work(), TRANSFER_WORK);
    assert!(foreign.work_ledger_identity_v1() == original_ledger);
}
