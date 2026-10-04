//! Actual checked-query denials distinguish local caps from stricter caller caps.
use super::super::{
    native_helper_value_context_v1::with_first_owner_call_for_test,
    resource_tests::argument_correspondence_tests::native_helper_argument_owner,
};
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Owned,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};

const WORK: usize = 100_000_000;
const STORAGE: usize = 32 << 20;
const PREFIX: usize = 23;
const FLOOR: usize = 4096;

fn query(
    owner: &ProductionPreRankedKirOwnerV1,
    budget: &mut Budget<'_>,
    work: usize,
    storage: usize,
) -> (Result<bool, &'static str>, Option<ArgumentResourceV1>) {
    let floor = budget.storage();
    let mut allowance = TranslationAllowanceV1::new(budget, work, storage);
    let observation = with_first_owner_call_for_test(owner, |query| {
        let mut meter = NativeValueMeter {
            budget,
            allowance: Some(&mut allowance),
            failed: false,
            resource_error: None,
        };
        let result = meter.check_call(query);
        assert_eq!(meter.exhausted(), result.is_err());
        (result, meter.resource_error)
    });
    assert_eq!(budget.storage(), floor);
    observation
}

#[test]
fn checked_query_work_denial_is_inside_query_and_uses_the_strictest_cap() {
    let owner = native_helper_argument_owner(None);
    let cap = Budget::BOUNDED_SCRATCH_WORK_V1 + 1;
    for (parent, local, parent_error) in [
        (WORK, cap, false),
        (PREFIX + cap, cap + 1, true),
        (PREFIX + cap, cap, false),
    ] {
        let mut work = Work::new(parent);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.charge_work(PREFIX).unwrap();
        budget.reserve_storage(FLOOR).unwrap();
        let (result, resource) = query(&owner, &mut budget, local, STORAGE - FLOOR);
        assert!(result.is_err());
        // Entry work/frame succeeded; the first checked-call charge is two.
        assert_eq!(budget.work(), PREFIX + Budget::BOUNDED_SCRATCH_WORK_V1);
        assert!(budget.peak_storage() >= FLOOR + Budget::BOUNDED_SCRATCH_STORAGE_V1);
        if parent_error {
            let Some(ArgumentResourceV1::Work(error)) = resource else {
                panic!("lost parent work cap: {resource:?}");
            };
            assert_eq!((error.actual(), error.limit()), (PREFIX + cap + 1, parent));
        } else {
            assert_eq!(resource, None);
        }
    }
}

#[test]
fn checked_query_storage_preserves_parent_caps_and_inherited_windows() {
    let owner = native_helper_argument_owner(Some(true));
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.charge_work(PREFIX).unwrap();
    budget.reserve_storage(FLOOR).unwrap();
    assert_eq!(
        query(&owner, &mut budget, WORK - PREFIX, STORAGE - FLOOR),
        (Ok(true), None)
    );
    let peak = budget.peak_storage() - FLOOR;
    assert!(peak > Budget::BOUNDED_SCRATCH_STORAGE_V1 + 1);
    let cap = peak - 1;
    for (parent, local, parent_error) in [
        (STORAGE, cap, false),
        (FLOOR + cap, cap + 1, true),
        (FLOOR + cap, cap, false),
    ] {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, parent);
        budget.charge_work(PREFIX).unwrap();
        budget.reserve_storage(FLOOR).unwrap();
        let (result, resource) = query(&owner, &mut budget, WORK - PREFIX, local);
        assert!(result.is_err());
        assert!(budget.work() > PREFIX + Budget::BOUNDED_SCRATCH_WORK_V1);
        assert!(budget.peak_storage() >= FLOOR + Budget::BOUNDED_SCRATCH_STORAGE_V1);
        if parent_error {
            let Some(ArgumentResourceV1::Storage(error)) = resource else {
                panic!("lost parent storage cap: {resource:?}");
            };
            assert!(error.actual() > error.limit());
            assert_eq!(error.limit(), parent);
        } else {
            assert_eq!(resource, None);
        }
    }
    let mut account = Owned::new(Work::new(WORK), STORAGE);
    account.with_budget(|budget| {
        budget.charge_work(PREFIX).unwrap();
        budget.reserve_storage(FLOOR).unwrap();
        budget
            .with_additional_storage_window_v1::<_, ArgumentResourceV1>(cap, |budget| {
                assert_eq!(budget.storage_limit(), STORAGE);
                let (result, resource) = query(
                    &owner,
                    budget,
                    WORK - PREFIX - Budget::STORAGE_WINDOW_WORK_V1,
                    cap + 1,
                );
                assert!(result.is_err());
                let Some(ArgumentResourceV1::Storage(error)) = resource else {
                    panic!("lost inherited storage cap: {resource:?}");
                };
                assert_eq!(error.limit(), FLOOR + cap);
                assert!(error.actual() > error.limit());
                Ok(())
            })
            .unwrap();
    });
    assert_eq!(account.storage(), FLOOR);
}
