//! Private-owner corruption controls below are inert unit fixtures, never a
//! public way to forge custody. Production callers cannot mutate owner bytes.
use super::*;
use crate::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
const LIMIT: usize = 16_000_000;
fn owner(module: &Module) -> (VerifiedCanonicalKernelIrModuleV21, usize) {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let (owner, receipt) =
        VerifiedCanonicalKernelIrModuleV21::from_module_ref_with_verification_budget_v21(
            module,
            &mut budget,
        )
        .unwrap();
    (owner, receipt.retained_storage())
}
fn resource(
    error: &CanonicalKernelIrReplayAdmissionErrorV21,
) -> Option<CanonicalKernelIrVerificationResourceErrorV1> {
    match error {
        CanonicalKernelIrReplayAdmissionErrorV21::Resource(e)
        | CanonicalKernelIrReplayAdmissionErrorV21::Decode(KernelIrDecodeError::Resource(e)) => {
            Some(*e)
        }
        CanonicalKernelIrReplayAdmissionErrorV21::Decode(KernelIrDecodeError::WorkLimit(e))
        | CanonicalKernelIrReplayAdmissionErrorV21::Decode(KernelIrDecodeError::Encode(
            KernelIrEncodeError::WorkLimit(e),
        )) => Some(CanonicalKernelIrVerificationResourceErrorV1::Work(*e)),
        _ => None,
    }
}
#[test]
fn independent_mutable_plain_view_does_not_change_owner_or_identity() {
    let (owner, owner_storage) = owner(&super::tests::fixture::module());
    let bytes = owner.canonical_bytes().to_vec();
    let identity = *owner.identity();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(owner_storage + 73).unwrap();
    let (mut view, receipt) = owner
        .decoded_inert_view_with_verification_budget_v21(&mut budget)
        .unwrap();
    assert_eq!(&view, owner.module());
    assert_ne!(view.functions.as_ptr(), owner.module().functions.as_ptr());
    assert_eq!(budget.storage(), owner_storage + 73);
    assert!(receipt.retained_storage() >= std::mem::size_of::<Module>());
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    view.functions.clear();
    assert!(!owner.module().functions.is_empty());
    assert_eq!(owner.canonical_bytes(), bytes);
    assert_eq!(owner.identity(), &identity);
    drop(view);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), owner_storage + 73);
}
#[test]
fn exact_reader_work_storage_and_one_below_restore_existing_owner_floor() {
    for module in [
        Module::new("ordinary_view"),
        super::tests::fixture::module(),
    ] {
        let (owner, owner_storage) = owner(&module);
        let floor = owner_storage + 79;
        let mut work = Work::new(LIMIT);
        work.charge_work(11).unwrap();
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(floor).unwrap();
        drop(
            owner
                .decoded_inert_view_with_verification_budget_v21(&mut budget)
                .unwrap(),
        );
        let needed_work = budget.work();
        let needed_storage = budget.peak_storage();
        assert!(needed_work > 11 && needed_storage > floor);
        for case in 0..3 {
            let mut work = Work::new(needed_work - usize::from(case == 1));
            work.charge_work(11).unwrap();
            let mut budget = Budget::new(&mut work, needed_storage - usize::from(case == 2));
            budget.reserve_storage(floor).unwrap();
            let result = owner.decoded_inert_view_with_verification_budget_v21(&mut budget);
            assert_eq!(result.is_ok(), case == 0);
            match (case, result) {
                (0, Ok((view, receipt))) => {
                    assert_eq!(view, module);
                    assert_eq!(budget.work(), needed_work);
                    budget.reserve_storage(receipt.retained_storage()).unwrap();
                    drop(view);
                    budget.release_storage(receipt.retained_storage()).unwrap();
                }
                (1, Err(error)) => {
                    let Some(CanonicalKernelIrVerificationResourceErrorV1::Work(e)) =
                        resource(&error)
                    else {
                        panic!("{error:?}")
                    };
                    assert_eq!(e.actual(), needed_work);
                    assert_eq!(e.limit() + 1, e.actual());
                }
                (2, Err(error)) => {
                    let Some(CanonicalKernelIrVerificationResourceErrorV1::Storage(e)) =
                        resource(&error)
                    else {
                        panic!("{error:?}")
                    };
                    assert_eq!(e.actual(), needed_storage);
                    assert_eq!(e.limit() + 1, e.actual());
                    assert_eq!(budget.failed_storage(), Some(needed_storage));
                }
                _ => unreachable!(),
            }
            assert_eq!(budget.storage(), floor);
        }
    }
}
#[test]
fn private_corruption_control_version_and_trailing_byte_are_not_reinterpreted() {
    for version in [0u16, 12, 20, 22, u16::MAX] {
        let (mut owner, retained) = owner(&super::tests::fixture::module());
        owner.canonical.canonical_bytes[8..10].copy_from_slice(&version.to_le_bytes());
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(retained + 83).unwrap();
        let error = owner
            .decoded_inert_view_with_verification_budget_v21(&mut budget)
            .unwrap_err();
        assert!(
            matches!(error, CanonicalKernelIrReplayAdmissionErrorV21::Decode(KernelIrDecodeError::UnknownVersion(v)) if v == version)
        );
        assert_eq!(budget.storage(), retained + 83);
    }
    let (mut owner, retained) = owner(&super::tests::fixture::module());
    owner.canonical.canonical_bytes.push(0);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(retained + 101).unwrap();
    let mut prior_work = 0;
    let mut prior_peak = 0;
    for _ in 0..3 {
        assert!(matches!(
            owner.decoded_inert_view_with_verification_budget_v21(&mut budget),
            Err(CanonicalKernelIrReplayAdmissionErrorV21::Decode(_))
        ));
        assert_eq!(budget.storage(), retained + 101);
        assert!(budget.work() > prior_work);
        assert!(budget.peak_storage() >= prior_peak);
        prior_work = budget.work();
        prior_peak = budget.peak_storage();
    }
}
#[test]
fn early_header_denial_preserves_floor_and_previous_failure_history() {
    let (owner, retained) = owner(&super::tests::fixture::module());
    let floor = retained + 127;
    let mut work = Work::new(LIMIT);
    work.charge_work(17).unwrap();
    let limit = floor + std::mem::size_of::<Module>() - 1;
    let mut budget = Budget::new(&mut work, limit);
    budget.reserve_storage(floor).unwrap();
    for _ in 0..2 {
        let error = owner
            .decoded_inert_view_with_verification_budget_v21(&mut budget)
            .unwrap_err();
        let Some(CanonicalKernelIrVerificationResourceErrorV1::Storage(e)) = resource(&error)
        else {
            panic!("{error:?}")
        };
        assert_eq!(e.actual(), limit + 1);
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.failed_storage(), Some(limit + 1));
        assert_eq!(budget.peak_storage(), floor);
        assert_eq!(budget.work(), 17);
    }
}
