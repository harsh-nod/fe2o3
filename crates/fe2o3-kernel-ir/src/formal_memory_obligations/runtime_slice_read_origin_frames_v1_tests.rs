use super::*;
use meter::LiveGuardMeter;

fn independent_frame() -> usize {
    4 * size_of::<ReadGuard>()
        + size_of::<FormalRuntimeSliceReadDomainV1>()
        + 2 * size_of::<RuntimeSliceReadConditionsV1>()
        + 2 * size_of::<Result<Option<RuntimeSliceReadConditionsV1>, ResourceError>>()
        + size_of::<Result<Option<FormalRuntimeSliceReadDomainV1>, ResourceError>>()
        + size_of::<Option<RuntimeSliceReadConditionsV1>>()
        + size_of::<(
            &mut GuardedAnalysisV1<'_, LiveGuardMeter<'_, '_>>,
            FunctionOperationLocation,
            ValueId,
            FormalMemoryAccessKind,
            MemoryAccess,
            Option<ValueId>,
        )>()
}

#[test]
fn normalized_read_origin_runtime_frame_exact_and_one_short_preserve_floor() {
    let expected = independent_frame();
    assert_eq!(
        read_conditions_frame_bytes::<LiveGuardMeter<'_, '_>>().unwrap(),
        expected
    );
    assert!(expected > 2 * size_of::<RuntimeSliceReadConditionsV1>());
    for allowed in [expected - 1, expected] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
        let mut budget = Budget::new(&mut work, 17 + allowed);
        budget.reserve_storage(17).unwrap();
        let result = {
            let mut meter = LiveGuardMeter::new(&mut budget, 100, expected, 1);
            reserve_read_conditions_frame(&mut meter)
        };
        if allowed == expected {
            result.unwrap();
            assert_eq!(budget.storage(), 17 + expected);
            assert_eq!(budget.peak_storage(), 17 + expected);
            budget.release_storage(expected).unwrap();
        } else {
            assert!(matches!(result, Err(ResourceError::Storage { .. })));
            assert_eq!(budget.storage(), 17);
            assert_eq!(budget.peak_storage(), 17);
            assert_eq!(budget.failed_storage(), Some(17 + expected));
        }
        assert_eq!(budget.storage(), 17);
        assert_eq!(budget.work(), 0);
    }
}

#[test]
fn normalized_read_origin_runtime_frame_local_limit_denies_before_external_debit() {
    let expected = independent_frame();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
    let mut budget = Budget::new(&mut work, 17 + expected);
    budget.reserve_storage(17).unwrap();
    let result = {
        let mut meter = LiveGuardMeter::new(&mut budget, 100, expected - 1, 1);
        reserve_read_conditions_frame(&mut meter)
    };
    assert!(
        matches!(result, Err(ResourceError::Storage { actual, limit })
        if actual == expected && limit == expected - 1)
    );
    assert_eq!(budget.storage(), 17);
    assert_eq!(budget.peak_storage(), 17);
    assert_eq!(budget.failed_storage(), None);
}
