use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_lower_mir_kernel::ProductionSourceOptimizationErrorV18 as Source;
use fe2o3_pliron::{
    KirCheckedNeutralOptimizationErrorV1 as Adoption, KirNeutralOptimizationErrorV18 as Observation,
};

#[test]
fn optimized_pipeline_error_keeps_existing_compactness_limit() {
    assert!(std::mem::size_of::<ProductionPipelineError>() <= 128);
}

#[test]
fn optimized_native_policy_error_payload_is_prepaid_at_exact_and_short_limits() {
    let payload = std::mem::size_of::<fe2o3_pliron::CanonicalRankedPolicyChecksErrorV1>();
    assert!(payload > std::mem::size_of::<Box<fe2o3_pliron::CanonicalRankedPolicyChecksErrorV1>>());
    for short in [false, true] {
        let mut work = Work::new(0);
        let limit = 17 + payload - usize::from(short);
        let mut budget = Budget::new(&mut work, limit);
        budget.reserve_storage(17).unwrap();
        let result = super::native_policies::reserve_policy_error_payload(&mut budget);
        if short {
            assert!(matches!(result, Err(Resource::Storage(error))
                if error.actual() == 17 + payload && error.limit() == limit));
            assert_eq!((budget.storage(), budget.peak_storage()), (17, 17));
            assert_eq!(budget.failed_storage(), Some(17 + payload));
        } else {
            result.unwrap();
            assert_eq!((budget.storage(), budget.peak_storage()), (limit, limit));
            budget.release_storage(payload).unwrap();
            assert_eq!(budget.storage(), 17);
            assert_eq!(budget.failed_storage(), None);
        }
        assert_eq!(budget.work(), 0);
    }
}

#[test]
fn optimized_prepared_header_has_independent_enclosing_layout_and_one_short_oracles() {
    use std::mem::{size_of, size_of_val};
    type Payload = [u128; 3];
    type Output = (
        fe2o3_pliron::CheckedNeutralKernelIrOwnerV18,
        Payload,
        fe2o3_pliron::KirNeutralOwnedOriginStorageV1,
    );
    type Prepared = PreparedMaterializationV29<Output>;
    let captured = [9u128; 7];
    let closure = move || std::hint::black_box(captured);
    let expected = size_of_val(&closure)
        + size_of::<std::cell::Cell<usize>>()
        + size_of::<OwnedBudget>()
        + size_of::<PreparedSsaMaterializationV29>()
        + size_of::<Prepared>()
        + size_of::<Result<Prepared, ProductionPipelineError>>()
        + size_of::<ProductionPipelineError>();
    assert_eq!(
        optimized_prepared_headers_v18::<Payload>(size_of_val(&closure)).unwrap(),
        expected
    );
    for one_short in [false, true] {
        let limit = 7 + expected - usize::from(one_short);
        let mut account = OwnedBudget::new(Work::new(0), limit);
        let result = account.with_budget(|budget| {
            budget.reserve_storage(7).unwrap();
            budget
                .reserve_storage(
                    optimized_prepared_headers_v18::<Payload>(size_of_val(&closure)).unwrap(),
                )
                .map_err(source_resource_v18)
        });
        if one_short {
            let Err(ProductionPipelineError::SourceOwnedEntrance(
                fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18::Resource(
                    Resource::Storage(error),
                ),
            )) = result
            else {
                panic!("one-short header must retain the source resource error");
            };
            assert_eq!((error.actual(), error.limit()), (7 + expected, limit));
            assert_eq!((account.storage(), account.peak_storage()), (7, 7));
            assert_eq!(account.failed_storage(), Some(7 + expected));
        } else {
            result.unwrap();
            assert_eq!((account.storage(), account.peak_storage()), (limit, limit));
            assert_eq!(account.failed_storage(), None);
        }
        assert_eq!((account.work(), account.failed_work()), (0, None));
    }
    assert!(matches!(
        optimized_prepared_headers_v18::<Payload>(usize::MAX),
        Err(ProductionPipelineError::SourceOwnedEntrance(
            fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18::Resource(
                Resource::Arithmetic
            )
        ))
    ));
}

#[test]
fn optimized_pipeline_conversion_cannot_replace_first_storage_refusal() {
    let mut work = Work::new(64);
    let mut budget = Budget::new(&mut work, 8);
    budget.reserve_storage(8).unwrap();
    let actual = budget.reserve_storage(1).unwrap_err();
    let before = (
        budget.work(),
        budget.storage(),
        budget.peak_storage(),
        budget.failed_storage(),
    );
    let first = source_optimization_error_v18(Source::Observation(Observation::Resource(actual)));
    let later = source_optimization_error_v18(Source::Adoption(Adoption::OriginAccounting));
    assert!(matches!(
        later,
        ProductionPipelineError::SourceOptimizationAdoption(Adoption::OriginAccounting)
    ));
    let ProductionPipelineError::SourceOptimizationObservation(Observation::Resource(
        Resource::Storage(error),
    )) = first
    else {
        panic!("storage refusal lost its exact payload");
    };
    assert_eq!(error.actual(), 9);
    assert_eq!(error.limit(), 8);
    assert_eq!(
        (
            budget.work(),
            budget.storage(),
            budget.peak_storage(),
            budget.failed_storage()
        ),
        before
    );
}

#[test]
fn optimized_pipeline_conversion_cannot_replace_first_work_refusal() {
    let mut work = Work::new(3);
    {
        let mut budget = Budget::new(&mut work, 64);
        budget.charge_work(3).unwrap();
        let actual = budget.charge_work(7).unwrap_err();
        let before = (
            budget.work(),
            budget.storage(),
            budget.peak_storage(),
            budget.failed_storage(),
        );
        let converted = source_optimization_error_v18(Source::Adoption(Adoption::Resource(actual)));
        assert!(
            matches!(converted, ProductionPipelineError::SourceOptimizationAdoption(Adoption::Resource(Resource::Work(error))) if error.actual() == 10 && error.limit() == 3)
        );
        let _ = source_optimization_error_v18(Source::Observation(Observation::Resource(
            Resource::Accounting,
        )));
        assert_eq!(
            (
                budget.work(),
                budget.storage(),
                budget.peak_storage(),
                budget.failed_storage()
            ),
            before
        );
    }
    assert_eq!(work.failed_work(), Some(10));
}
