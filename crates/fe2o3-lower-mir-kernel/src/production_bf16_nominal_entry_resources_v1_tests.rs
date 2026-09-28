//! Synthetic entry resource tests only. No source owner or positive nominal
//! authority is fabricated. Genuine original-floor probes run in the backend.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::cell::Cell;
const FLOOR: usize = 37;
const LIMIT: usize = 1_000_000;

#[test]
fn true_original_floor_is_not_masked_by_reserved_entry_frame() {
    for short in [false, true] {
        let mut work = Work::new(LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR - usize::from(short)).unwrap();
        let entered = Cell::new(false);
        let result = bf16_nominal_entry_scope_v1(&mut budget, |budget, original| {
            assert!(budget.storage() > FLOOR);
            assert_eq!(original, FLOOR - usize::from(short));
            bf16_nominal_require_original_floor_v1(original, FLOOR)?;
            entered.set(true);
            Ok(())
        });
        assert_eq!(
            result,
            if short {
                Err(ArgumentResourceV1::Accounting.into())
            } else {
                Ok(())
            }
        );
        assert_eq!(entered.get(), !short);
        assert_eq!(budget.storage(), FLOOR - usize::from(short));
    }
}

#[test]
fn strict_entry_exact_work_storage_and_one_short_preserve_prefix() {
    for mode in 0..3 {
        let entered = Cell::new(false);
        let body = |_: &mut ArgumentBudgetV1<'_>, original: usize| {
            bf16_nominal_require_original_floor_v1(original, FLOOR)?;
            entered.set(true);
            Ok(())
        };
        let frame = bf16_nominal_entry_frame_v1::<()>(std::mem::size_of_val(&body)).unwrap();
        let mut work = Work::new(7 + BF16_NOMINAL_ENTRY_WORK_V1 - usize::from(mode == 1));
        let mut budget = ArgumentBudgetV1::new(&mut work, FLOOR + frame - usize::from(mode == 2));
        budget.charge_work(7).unwrap();
        budget.reserve_storage(FLOOR).unwrap();
        let result = bf16_nominal_entry_scope_v1(&mut budget, body);
        assert_eq!(budget.storage(), FLOOR);
        match mode {
            0 => {
                assert_eq!(result, Ok(()));
                assert!(entered.get());
                assert_eq!(budget.work(), 7 + BF16_NOMINAL_ENTRY_WORK_V1);
                assert_eq!(budget.peak_storage(), FLOOR + frame);
            }
            1 => {
                assert!(matches!(
                    result,
                    Err(Bf16NominalCallQueryErrorV1::Resource(
                        ArgumentResourceV1::Work(_)
                    ))
                ));
                assert!(!entered.get());
            }
            _ => {
                assert!(matches!(
                    result,
                    Err(Bf16NominalCallQueryErrorV1::Resource(
                        ArgumentResourceV1::Storage(_)
                    ))
                ));
                assert!(!entered.get());
            }
        }
    }
}

struct LargeCapture<'a> {
    bytes: [u8; 8192],
    dropped: &'a Cell<bool>,
}
impl Drop for LargeCapture<'_> {
    fn drop(&mut self) {
        self.dropped.set(true);
    }
}

#[test]
fn large_generic_entry_frames_remain_paid_during_initial_query_and_denial() {
    for mode in 0..3 {
        let dropped = Cell::new(false);
        let entered = Cell::new(false);
        let capture = LargeCapture {
            bytes: [7; 8192],
            dropped: &dropped,
        };
        let body = move |budget: &mut ArgumentBudgetV1<'_>,
                         original: usize|
              -> Bf16CallQueryResultV1<[u8; 8192]> {
            bf16_nominal_require_original_floor_v1(original, FLOOR)?;
            let entry_floor = budget.storage();
            assert!(entry_floor >= FLOOR + 8 * 8192);
            assert_eq!(capture.bytes[8191], 7);
            let initial = bf16_call_query_scope_v1(budget, |_| -> Bf16CallQueryResultV1<()> {
                entered.set(true);
                if mode == 1 {
                    Err(Bf16NominalCallQueryErrorV1::Unavailable(
                        "synthetic initial query denial",
                    ))
                } else {
                    Ok(())
                }
            });
            assert_eq!(budget.storage(), entry_floor);
            let bytes = capture.bytes;
            // Consume the complete Drop wrapper inside the paid callback, not
            // just its precisely capturable Copy array field.
            drop(capture);
            initial?;
            Ok(bytes)
        };
        let frame =
            bf16_nominal_entry_frame_v1::<[u8; 8192]>(std::mem::size_of_val(&body)).unwrap();
        let query = bf16_call_query_scratch_v1::<()>().unwrap();
        let mut work = Work::new(LIMIT);
        let mut budget =
            ArgumentBudgetV1::new(&mut work, FLOOR + frame + query - usize::from(mode == 2));
        budget.reserve_storage(FLOOR).unwrap();
        let result = bf16_nominal_entry_scope_v1(&mut budget, body);
        assert!(dropped.get());
        assert_eq!(budget.storage(), FLOOR);
        if mode == 0 {
            assert_eq!(result, Ok([7; 8192]));
            assert_eq!(budget.peak_storage(), FLOOR + frame + query);
        } else if mode == 1 {
            assert_eq!(
                result,
                Err(Bf16NominalCallQueryErrorV1::Unavailable(
                    "synthetic initial query denial"
                ))
            );
            assert_eq!(budget.peak_storage(), FLOOR + frame + query);
        } else {
            assert!(matches!(
                result,
                Err(Bf16NominalCallQueryErrorV1::Resource(
                    ArgumentResourceV1::Storage(_)
                ))
            ));
            assert_eq!(budget.peak_storage(), FLOOR + frame);
            assert!(budget.failed_storage().is_some());
        }
    }
}

#[test]
fn large_entry_refuses_original_floor_minus_one_before_initial_query() {
    let entered = Cell::new(false);
    let payload = [9u8; 8192];
    let entered_ref = &entered;
    let body = move |budget: &mut ArgumentBudgetV1<'_>,
                     original: usize|
          -> Bf16CallQueryResultV1<[u8; 8192]> {
        assert!(budget.storage() >= FLOOR + 8 * 8192 - 1);
        bf16_nominal_require_original_floor_v1(original, FLOOR)?;
        entered_ref.set(true);
        Ok(payload)
    };
    let mut work = Work::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR - 1).unwrap();
    let result = bf16_nominal_entry_scope_v1(&mut budget, body);
    assert_eq!(result, Err(ArgumentResourceV1::Accounting.into()));
    assert!(!entered.get());
    assert_eq!(budget.storage(), FLOOR - 1);
}

#[test]
fn entry_success_error_panic_and_surplus_are_owned_exactly() {
    for mode in 0..3 {
        let mut work = Work::new(LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let identity = budget.work_ledger_identity_v1();
        let result =
            bf16_nominal_entry_scope_v1(&mut budget, |budget, _| -> Bf16CallQueryResultV1<()> {
                budget.reserve_storage(23)?;
                budget.charge_work(17)?;
                match mode {
                    0 => Ok(()),
                    1 => Err(Bf16NominalCallQueryErrorV1::Unavailable("entry error")),
                    _ => panic!("entry panic"),
                }
            });
        assert_eq!(
            result,
            match mode {
                0 => Ok(()),
                1 => Err(Bf16NominalCallQueryErrorV1::Unavailable("entry error")),
                _ => Err(Bf16NominalCallQueryErrorV1::CallbackPanicked),
            }
        );
        assert!(budget.work_ledger_identity_v1() == identity);
        assert_eq!(budget.storage(), FLOOR + 23);
        assert_eq!(budget.work(), BF16_NOMINAL_ENTRY_WORK_V1 + 17);
    }
}

#[test]
fn entry_sticky_denials_are_not_success_and_corruption_is_not_repaired() {
    for mode in 0..3 {
        let mut work = Work::new(LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let observed = Cell::new(0usize);
        let result = bf16_nominal_entry_scope_v1(&mut budget, |budget, _| {
            observed.set(budget.storage());
            match mode {
                0 => {
                    let _ = budget.charge_work(LIMIT + 1);
                }
                1 => {
                    let _ = budget.reserve_storage(LIMIT + 1);
                }
                _ => budget.release_storage(1)?,
            }
            Ok(())
        });
        assert_eq!(result, Err(ArgumentResourceV1::Accounting.into()));
        assert_eq!(
            budget.storage(),
            if mode == 2 { observed.get() - 1 } else { FLOOR }
        );
        assert_eq!(budget.failed_work().is_some(), mode == 0);
        assert_eq!(budget.failed_storage().is_some(), mode == 1);
    }
}

#[test]
fn entry_replaced_ledger_is_left_untouched() {
    let mut original = Work::new(LIMIT);
    let mut foreign = Work::new(LIMIT);
    let replacement = ArgumentBudgetV1::new(&mut foreign, LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut original, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let identity = budget.work_ledger_identity_v1();
    let result = bf16_nominal_entry_scope_v1(&mut budget, |budget, _| {
        *budget = replacement;
        Ok(())
    });
    assert_eq!(result, Err(ArgumentResourceV1::Accounting.into()));
    assert!(budget.work_ledger_identity_v1() != identity);
    assert_eq!(
        (budget.storage(), budget.work(), budget.peak_storage()),
        (0, 0, 0)
    );
}

#[test]
fn entry_preexisting_denial_does_not_run_or_reserve() {
    let mut work = Work::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
    assert!(budget.charge_work(LIMIT + 1).is_err());
    let result = bf16_nominal_entry_scope_v1(&mut budget, |_, _| -> Bf16CallQueryResultV1<()> {
        panic!("pre-denied entry callback");
    });
    assert_eq!(result, Err(ArgumentResourceV1::Accounting.into()));
    assert_eq!(
        (budget.storage(), budget.work(), budget.peak_storage()),
        (0, 0, 0)
    );
    assert_eq!(
        bf16_nominal_entry_frame_v1::<()>(usize::MAX),
        Err(ArgumentResourceV1::Arithmetic.into())
    );
}

#[test]
fn entry_panic_payload_is_dropped_before_refund() {
    struct Flag(std::sync::Arc<std::sync::atomic::AtomicBool>);
    impl Drop for Flag {
        fn drop(&mut self) {
            self.0.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }
    let dropped = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut work = Work::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
    let result = bf16_nominal_entry_scope_v1(&mut budget, |_, _| -> Bf16CallQueryResultV1<()> {
        std::panic::panic_any(Flag(dropped.clone()))
    });
    assert_eq!(result, Err(Bf16NominalCallQueryErrorV1::CallbackPanicked));
    assert!(dropped.load(std::sync::atomic::Ordering::SeqCst));
    assert_eq!(budget.storage(), 0);
}
