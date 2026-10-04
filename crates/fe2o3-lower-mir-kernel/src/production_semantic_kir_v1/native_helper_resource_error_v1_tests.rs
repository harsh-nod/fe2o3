//! Current typed denials must not be reconstructed from sticky account history.
use super::super::resource_tests::argument_correspondence_tests::native_helper_argument_owner;
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};

#[test]
fn first_current_resource_failure_survives_cleanup_and_prior_denials() {
    for storage_first in [false, true] {
        let mut work = Work::new(8);
        let mut budget = Budget::new(&mut work, 41);
        budget.charge_work(2).unwrap();
        budget.reserve_storage(31).unwrap();
        assert!(budget.charge_work(99).is_err());
        assert!(budget.reserve_storage(100).is_err());
        let mut meter = NativeValueMeter {
            budget: &mut budget,
            allowance: None,
            failed: false,
            resource_error: None,
        };
        if storage_first {
            assert!(meter.reserve(11).is_err());
            assert!(meter.work(7).is_err());
        } else {
            assert!(meter.work(7).is_err());
            assert!(meter.reserve(11).is_err());
        }
        match meter.resource_error.unwrap() {
            ArgumentResourceV1::Storage(error) if storage_first => {
                assert_eq!((error.actual(), error.limit()), (42, 41));
            }
            ArgumentResourceV1::Work(error) if !storage_first => {
                assert_eq!((error.actual(), error.limit()), (9, 8));
            }
            error => panic!("wrong current failure: {error:?}"),
        }
        let current = meter.resource_error;
        meter.release(0).unwrap();
        assert_eq!(meter.resource_error, current);
        assert_eq!((meter.budget.work(), meter.budget.storage()), (2, 31));
        assert_eq!(
            (meter.budget.failed_work(), meter.budget.failed_storage()),
            (Some(101), Some(131))
        );
    }
}

#[test]
fn typed_expansion_preserves_local_and_semantic_failures_despite_history() {
    let owner = native_helper_argument_owner(None);
    for failure in 0..3 {
        let mut work = Work::new(100);
        let mut budget = Budget::new(&mut work, 100);
        assert!(budget.charge_work(101).is_err());
        assert!(budget.reserve_storage(101).is_err());
        let mut allowance = TranslationAllowanceV1::new(&budget, 0, 0);
        let result = with_native_value_expansion_and_allowance_resources_v1(
            None,
            owner.executable.module(),
            &owner.correspondence,
            "",
            &mut budget,
            Some(&mut allowance),
            |expansion| {
                let request = match failure {
                    0 => expansion.meter.work(1),
                    1 => expansion.meter.reserve(1),
                    _ => expansion.meter.work(0),
                };
                request.map_err(|_| ProductionMirPlironTranslationErrorV1::ResourceLimit)?;
                Err(ProductionMirPlironTranslationErrorV1::KernelShape)
            },
        );
        let expected = if failure == 2 {
            ProductionMirPlironTranslationErrorV1::KernelShape
        } else {
            ProductionMirPlironTranslationErrorV1::ResourceLimit
        };
        assert_eq!(result, Err(NativeTranslationErrorV1::Translation(expected)));
        assert_eq!((budget.work(), budget.storage()), (0, 0));
        assert_eq!(
            (budget.failed_work(), budget.failed_storage()),
            (Some(101), Some(101))
        );
    }
}

#[test]
fn typed_expansion_preserves_the_early_owner_scan_denial() {
    let owner = native_helper_argument_owner(None);
    let mut work = Work::new(0);
    let mut budget = Budget::new(&mut work, 32 << 20);
    assert!(budget.charge_work(usize::MAX).is_err());
    let result = with_native_value_expansion_and_allowance_resources_v1(
        Some(&owner.semantic_ssa),
        owner.executable.module(),
        &owner.correspondence,
        "",
        &mut budget,
        None,
        |_| panic!("owner scan must refuse before translation"),
    );
    let Err(NativeTranslationErrorV1::Resource(ArgumentResourceV1::Work(error))) = result else {
        panic!("wrong early-scan failure: {result:?}");
    };
    let expected = owner.executable.module().kernels.len()
        + owner.executable.module().functions.len()
        + owner.correspondence.lowered_functions.len();
    assert_eq!((error.actual(), error.limit()), (expected, 0));
    assert_eq!(budget.failed_work(), Some(usize::MAX));
    assert_eq!((budget.work(), budget.storage()), (0, 0));
}
