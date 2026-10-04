//! Scope/profile tests only; actual source/L artifact qualification is separate.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

fn accounting(error: &ProductionPipelineError) -> bool {
    matches!(
        error,
        ProductionPipelineError::SourceLocalOrderStage(SourceLocalOrderStageErrorV1::Resource(
            Resource::Accounting
        ))
    )
}

#[test]
fn only_exact_gfx942_raw_empty_profile_is_admitted() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for helper in [HelperPolicy::RawEmpty, HelperPolicy::UnitLocal] {
            let mut work = Work::new(10);
            let mut budget = Budget::new(&mut work, 100);
            budget.reserve_storage(17).unwrap();
            assert_eq!(
                check_profile(profile, helper, &mut budget).is_ok(),
                profile == Profile::Gfx942 && helper == HelperPolicy::RawEmpty
            );
            assert_eq!(budget.storage(), 17);
            assert_eq!(budget.work(), 3);
        }
    }
}

#[test]
fn transfer_scope_returns_unreserved_delta_and_keeps_one_ledger() {
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, 100);
    budget.reserve_storage(17).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let value = scoped(17, &mut budget, |budget| {
        budget.charge_work(11).map_err(resource)?;
        budget.reserve_storage(23).map_err(resource)?;
        Ok(23usize)
    })
    .unwrap();
    assert_eq!(value, 23);
    assert_eq!(budget.storage(), 17);
    assert_eq!(budget.peak_storage(), 40);
    assert_eq!(budget.work(), 11);
    assert!(budget.work_ledger_identity_v1() == ledger);
    budget.reserve_storage(value).unwrap();
    budget.release_storage(value).unwrap();
    assert_eq!(budget.storage(), 17);
}

#[test]
fn retained_scope_keeps_successful_stage_reservations() {
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, 100);
    budget.reserve_storage(17).unwrap();
    let value = retained(17, &mut budget, |budget| {
        budget.charge_work(11).map_err(resource)?;
        budget.reserve_storage(23).map_err(resource)?;
        Ok(budget.storage())
    })
    .unwrap();
    assert_eq!(value, 40);
    assert_eq!(budget.storage(), 40);
    assert_eq!(budget.work(), 11);
    budget.release_storage(23).unwrap();
    assert_eq!(budget.storage(), 17);
}

#[test]
fn both_scopes_restore_nonzero_floor_on_refusal_and_unwind() {
    for retain in [false, true] {
        for unwind in [false, true] {
            let mut work = Work::new(100);
            let mut budget = Budget::new(&mut work, 100);
            budget.reserve_storage(17).unwrap();
            let run = |budget: &mut Budget<'_>| -> ResultL<()> {
                budget.charge_work(11).map_err(resource)?;
                budget.reserve_storage(23).map_err(resource)?;
                assert!(!unwind, "synthetic local-order scope unwind");
                Err(execution("synthetic refusal"))
            };
            let result = if retain {
                retained(17, &mut budget, run)
            } else {
                scoped(17, &mut budget, run)
            };
            assert!(
                matches!(
                    result,
                    Err(ProductionPipelineError::SourceLocalOrderStage(
                        SourceLocalOrderStageErrorV1::Panicked
                    ))
                ) == unwind
            );
            assert!(result.is_err());
            assert_eq!(budget.storage(), 17);
            assert_eq!(budget.peak_storage(), 40);
            assert_eq!(budget.work(), 11);
        }
    }
}

#[test]
fn required_floor_refuses_before_callback_and_no_work_is_reset() {
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, 100);
    budget.reserve_storage(17).unwrap();
    budget.charge_work(7).unwrap();
    let mut entered = false;
    let result = scoped(18, &mut budget, |_| {
        entered = true;
        Ok(())
    });
    assert!(accounting(&result.unwrap_err()));
    assert!(!entered);
    assert_eq!(budget.storage(), 17);
    assert_eq!(budget.work(), 7);
}

#[test]
fn exact_and_one_short_scope_limits_preserve_first_failure_history() {
    for retain in [false, true] {
        for case in 0..3 {
            let mut work = Work::new(35 - usize::from(case == 1));
            let mut budget = Budget::new(&mut work, 26 - usize::from(case == 2));
            budget.reserve_storage(17).unwrap();
            budget.charge_work(5).unwrap();
            let run = |budget: &mut Budget<'_>| -> ResultL<()> {
                budget.charge_work(13).map_err(resource)?;
                budget.reserve_storage(9).map_err(resource)?;
                budget.charge_work(17).map_err(resource)?;
                Ok(())
            };
            let result = if retain {
                retained(17, &mut budget, run)
            } else {
                scoped(17, &mut budget, run)
            };
            assert_eq!(result.is_ok(), case == 0);
            assert_eq!(budget.storage(), if retain && case == 0 { 26 } else { 17 });
            assert_eq!(budget.work(), if case == 0 { 35 } else { 18 });
            if case == 2 {
                assert_eq!(budget.failed_storage(), Some(26));
            }
        }
    }
}

#[test]
fn replacing_work_ledger_is_not_accepted_or_released_as_original_storage() {
    let mut original_work = Work::new(100);
    let mut other_work = Work::new(100);
    let mut budget = Budget::new(&mut original_work, 100);
    let mut other = Budget::new(&mut other_work, 100);
    budget.reserve_storage(17).unwrap();
    other.reserve_storage(29).unwrap();
    let result = scoped(17, &mut budget, |budget| {
        std::mem::swap(budget, &mut other);
        Ok(())
    });
    assert!(accounting(&result.unwrap_err()));
    assert_eq!(budget.storage(), 29);
    assert_eq!(other.storage(), 17);
}

#[test]
fn released_inherited_storage_is_refused_without_fabricating_a_reservation() {
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, 100);
    budget.reserve_storage(17).unwrap();
    let result = scoped(17, &mut budget, |budget| {
        budget.release_storage(1).map_err(resource)?;
        Ok(())
    });
    assert!(accounting(&result.unwrap_err()));
    assert_eq!(budget.storage(), 16);
}
