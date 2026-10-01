//! Synthetic meter controls: no source authority or allocation measurement.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};

#[test]
fn local_work_exact_then_one_short_refuses_before_original_charge() {
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, 100);
    budget.reserve_storage(31).unwrap();
    let mut allowance = TranslationAllowanceV1::new(&budget, 7, 11);
    let mut meter = NativeValueMeter {
        budget: &mut budget,
        allowance: Some(&mut allowance),
        failed: false,
    };
    meter.work(7).unwrap();
    assert!(meter.work(1).is_err());
    assert_eq!(meter.budget.work(), 7);
    assert!(meter.budget.failed_work().is_none()); // local refusal, not forged parent denial
    assert!(meter.exhausted());
    assert!(meter.work(0).is_err()); // sticky even for a no-op request
    assert_eq!(meter.budget.storage(), 31);
}

#[test]
fn local_storage_exact_then_one_short_refuses_before_original_reservation() {
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, 100);
    budget.reserve_storage(31).unwrap();
    let mut allowance = TranslationAllowanceV1::new(&budget, 7, 11);
    let mut meter = NativeValueMeter {
        budget: &mut budget,
        allowance: Some(&mut allowance),
        failed: false,
    };
    meter.reserve(11).unwrap();
    assert!(meter.reserve(1).is_err());
    assert_eq!(meter.budget.storage(), 42);
    assert_eq!(meter.budget.peak_storage(), 42);
    assert!(meter.budget.failed_storage().is_none());
    meter.release(11).unwrap(); // cleanup remains allowed after refusal
    assert_eq!(meter.budget.storage(), 31);
    assert!(meter.reserve(0).is_err());
}

#[test]
fn allowance_is_shared_across_meter_views_not_refreshed_per_root() {
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, 100);
    budget.reserve_storage(31).unwrap();
    let identity = budget.work_ledger_identity_v1();
    let mut allowance = TranslationAllowanceV1::new(&budget, 7, 11);
    {
        let mut meter = NativeValueMeter {
            budget: &mut budget,
            allowance: Some(&mut allowance),
            failed: false,
        };
        meter.work(3).unwrap();
        meter.reserve(11).unwrap();
        meter.release(11).unwrap();
    }
    {
        let mut meter = NativeValueMeter {
            budget: &mut budget,
            allowance: Some(&mut allowance),
            failed: false,
        };
        meter.work(4).unwrap();
        assert!(meter.work(1).is_err());
        assert!(meter.exhausted());
    }
    assert_eq!(budget.work(), 7);
    assert_eq!(budget.storage(), 31);
    assert!(budget.work_ledger_identity_v1() == identity);
    assert_eq!(allowance.work, 7);
    assert!(allowance.failed);
}

#[test]
fn original_work_and_storage_limits_remain_stricter_when_smaller() {
    for storage_case in [false, true] {
        let mut work = Work::new(if storage_case { 100 } else { 6 });
        let mut budget = Budget::new(&mut work, if storage_case { 41 } else { 100 });
        budget.reserve_storage(31).unwrap();
        let mut allowance = TranslationAllowanceV1::new(&budget, 7, 11);
        let mut meter = NativeValueMeter {
            budget: &mut budget,
            allowance: Some(&mut allowance),
            failed: false,
        };
        let result = if storage_case {
            meter.reserve(11)
        } else {
            meter.work(7)
        };
        assert!(result.is_err() && meter.exhausted());
        assert_eq!(meter.budget.work(), 0);
        assert_eq!(meter.budget.storage(), 31);
        assert_eq!(meter.budget.failed_work().is_some(), !storage_case);
        assert_eq!(meter.budget.failed_storage().is_some(), storage_case);
    }
}

#[test]
fn foreign_original_account_and_floor_release_are_rejected_before_effect() {
    let mut first_work = Work::new(100);
    let mut second_work = Work::new(100);
    let mut first = Budget::new(&mut first_work, 100);
    let mut second = Budget::new(&mut second_work, 100);
    first.reserve_storage(31).unwrap();
    second.reserve_storage(31).unwrap();
    let mut allowance = TranslationAllowanceV1::new(&first, 7, 11);
    {
        let mut meter = NativeValueMeter {
            budget: &mut second,
            allowance: Some(&mut allowance),
            failed: false,
        };
        assert!(meter.work(1).is_err());
        assert_eq!(meter.budget.work(), 0);
    }
    let mut allowance = TranslationAllowanceV1::new(&first, 7, 11);
    let mut meter = NativeValueMeter {
        budget: &mut first,
        allowance: Some(&mut allowance),
        failed: false,
    };
    assert!(meter.release(1).is_err());
    assert_eq!(meter.budget.storage(), 31);
    assert!(meter.exhausted());
}

#[test]
fn local_arithmetic_overflow_refuses_without_forwarding() {
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let mut allowance = TranslationAllowanceV1::new(&budget, usize::MAX, usize::MAX);
    {
        let mut meter = NativeValueMeter {
            budget: &mut budget,
            allowance: Some(&mut allowance),
            failed: false,
        };
        meter.work(usize::MAX).unwrap(); // synthetic charge, no work is executed
        assert!(meter.work(1).is_err());
        assert_eq!(meter.budget.work(), usize::MAX);
        assert!(meter.budget.failed_work().is_none());
    }
    let mut allowance = TranslationAllowanceV1::new(&budget, usize::MAX, usize::MAX);
    let mut meter = NativeValueMeter {
        budget: &mut budget,
        allowance: Some(&mut allowance),
        failed: false,
    };
    meter.reserve(usize::MAX).unwrap(); // synthetic reservation, no allocation
    assert!(meter.reserve(1).is_err());
    assert_eq!(meter.budget.storage(), usize::MAX);
    assert!(meter.budget.failed_storage().is_none());
    meter.release(usize::MAX).unwrap();
}

#[test]
fn none_keeps_the_existing_caller_budget_behavior() {
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, 100);
    let mut meter = NativeValueMeter {
        budget: &mut budget,
        allowance: None,
        failed: false,
    };
    meter.work(8).unwrap();
    meter.reserve(12).unwrap();
    assert_eq!(meter.budget.work(), 8);
    assert_eq!(meter.budget.storage(), 12);
    assert!(!meter.exhausted());
    meter.release(12).unwrap();
}
