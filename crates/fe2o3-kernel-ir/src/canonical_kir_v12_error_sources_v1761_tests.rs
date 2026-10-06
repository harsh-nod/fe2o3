use super::*;

type Resource = CanonicalKernelIrVerificationResourceErrorV1;

fn resource_cause(error: &(dyn Error + 'static)) -> Option<Resource> {
    if let Some(resource) = error.downcast_ref::<Resource>() {
        return Some(*resource);
    }
    error.source().and_then(resource_cause)
}

#[test]
fn decode_error_sources_preserve_concrete_resource_work_and_encode_causes() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
    let resource = budget.reserve_storage(1).unwrap_err();
    let work_resource = budget.charge_work(1).unwrap_err();
    let Resource::Work(work_error) = work_resource else {
        panic!("expected work")
    };
    let before = (
        budget.work(),
        budget.storage(),
        budget.peak_storage(),
        budget.failed_storage(),
    );
    for resource in [resource, work_resource] {
        let error = KernelIrDecodeError::Resource(resource);
        assert_eq!(
            error.source().unwrap().downcast_ref::<Resource>(),
            Some(&resource)
        );
        assert_eq!(resource_cause(&error), Some(resource));
    }
    let decoded_work = KernelIrDecodeError::WorkLimit(work_error);
    assert!(
        decoded_work
            .source()
            .unwrap()
            .is::<CanonicalKernelIrWorkLimitV1>()
    );
    let encode = KernelIrDecodeError::Encode(KernelIrEncodeError::WorkLimit(work_error));
    assert!(encode.source().unwrap().is::<KernelIrEncodeError>());
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
fn replay_admission_sources_preserve_nested_resource_identity_without_meter_activity() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
    let denied = [
        budget.charge_work(1).unwrap_err(),
        budget.reserve_storage(1).unwrap_err(),
    ];
    let before = (
        budget.work(),
        budget.storage(),
        budget.peak_storage(),
        budget.failed_storage(),
    );
    for expected in denied {
        for error in [
            CanonicalKernelIrReplayAdmissionErrorV12::Resource(expected),
            CanonicalKernelIrReplayAdmissionErrorV12::Decode(KernelIrDecodeError::Resource(
                expected,
            )),
            CanonicalKernelIrReplayAdmissionErrorV12::Canonical(
                MeteredVerifiedCanonicalKernelIrErrorV12::Canonical(
                    VerifiedCanonicalKernelIrErrorV12::Decode(KernelIrDecodeError::Resource(
                        expected,
                    )),
                ),
            ),
        ] {
            assert_eq!(resource_cause(&error), Some(expected));
        }
    }
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
fn nonresource_wire_and_identity_refusals_do_not_fabricate_error_causes() {
    for error in [
        KernelIrDecodeError::TooLarge { max: 0 },
        KernelIrDecodeError::InvalidMagic,
        KernelIrDecodeError::UnknownVersion(0),
        KernelIrDecodeError::UnsupportedFlags(0),
        KernelIrDecodeError::InvalidLength { declared: 0 },
        KernelIrDecodeError::Truncated,
        KernelIrDecodeError::TrailingBytes,
        KernelIrDecodeError::ReservedNonZero { field: "test" },
        KernelIrDecodeError::UnknownTag {
            kind: "test",
            tag: 0,
        },
        KernelIrDecodeError::InvalidUtf8 { field: "test" },
        KernelIrDecodeError::LimitExceeded {
            field: "test",
            actual: 1,
            max: 0,
        },
        KernelIrDecodeError::TypeNestingTooDeep { max: 0 },
        KernelIrDecodeError::NonCanonical,
        KernelIrDecodeError::InvalidSemanticOperationInstance,
    ] {
        assert!(error.source().is_none(), "{error:?}");
        let wrapped = CanonicalKernelIrReplayAdmissionErrorV12::Decode(error);
        assert!(wrapped.source().unwrap().is::<KernelIrDecodeError>());
        assert!(resource_cause(&wrapped).is_none());
    }
    assert!(
        CanonicalKernelIrReplayAdmissionErrorV12::CanonicalMismatch
            .source()
            .is_none()
    );
}

#[test]
fn real_v12_admission_short_storage_keeps_exact_cause_and_caller_floor() {
    fn run(
        limit: usize,
    ) -> (
        Result<(), CanonicalKernelIrReplayAdmissionErrorV12>,
        usize,
        Option<usize>,
    ) {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, limit);
        budget.reserve_storage(7).unwrap();
        let result = VerifiedCanonicalKernelIrV12::from_module_ref_with_verification_budget_v12(
            &Module::new("source-chain"),
            &mut budget,
        )
        .map(|(owner, _)| drop(owner));
        assert_eq!(budget.storage(), 7);
        (result, budget.peak_storage(), budget.failed_storage())
    }
    let (result, peak, failure) = run(usize::MAX);
    result.unwrap();
    assert_eq!(failure, None);
    let (result, exact_peak, failure) = run(peak);
    result.unwrap();
    assert_eq!(exact_peak, peak);
    assert_eq!(failure, None);
    let (result, refused_peak, failure) = run(peak - 1);
    let error = result.unwrap_err();
    assert!(
        matches!(resource_cause(&error), Some(Resource::Storage(error)) if error.actual() == peak && error.limit() == peak - 1)
    );
    assert_eq!(failure, Some(peak));
    assert!(refused_peak < peak);
}
