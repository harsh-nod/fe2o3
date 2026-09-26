use super::*;
use crate::ProductionLicmErrorV1 as LicmError;

#[test]
fn source_licm_refuses_one_short_prefix_and_output_floors_before_work() {
    for mutation in [false, true] {
        let (prefix, _) = direct::prefix(Profile::Gfx942, mutation);
        let minimum = prefix.retained_input_storage_floor_v1().unwrap();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(minimum - 1).unwrap();
        assert!(matches!(
            prefix.continue_licm_v1(&mut budget),
            Err(LicmError::Resource(AssertOriginResourceV1::Accounting))
        ));
        assert_eq!((budget.storage(), budget.work()), (minimum - 1, 0));

        let (prefix, inherited) = direct::prefix(Profile::Gfx942, mutation);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(inherited).unwrap();
        let (owner, _) = prefix.continue_licm_v1(&mut budget).unwrap();
        let minimum = owner.retained_input_storage_floor_v1().unwrap();
        let mut short_work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut short = AssertOriginBudgetV1::new(&mut short_work, STORAGE);
        short.reserve_storage(minimum - 1).unwrap();
        assert!(matches!(
            owner.verify_equivalence(&mut short),
            Err(LicmError::Resource(AssertOriginResourceV1::Accounting))
        ));
        assert_eq!((short.storage(), short.work()), (minimum - 1, 0));
    }
}

type Measurements = (
    Result<(), LicmError>,
    usize,
    usize,
    Option<usize>,
    Option<usize>,
);

fn measured(profile: Profile, mutation: bool, limit: usize, storage: usize) -> Measurements {
    let (prefix, inherited) = direct::prefix(profile, mutation);
    let sibling = vec![0x6d_u8; 43];
    let floor = inherited + std::mem::size_of_val(&sibling) + sibling.capacity();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
    let (result, accepted, peak, failed_storage) = {
        let mut budget = AssertOriginBudgetV1::new(&mut work, storage);
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(17).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = prefix.continue_licm_v1(&mut budget).map(|(owner, added)| {
            assert_eq!(
                owner.additional_retained_storage_v1(),
                added.retained_storage()
            );
            if mutation {
                actual_mutation(
                    owner.prefix().output(),
                    owner.output(),
                    owner.operation_origins(),
                );
            } else {
                assert_eq!(
                    owner.prefix().output().canonical().canonical_bytes(),
                    owner.output().canonical().canonical_bytes()
                );
                assert!(!std::ptr::eq(owner.prefix().output(), owner.output()));
            }
            drop(owner);
        });
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert!(sibling.iter().all(|byte| *byte == 0x6d));
        (
            result,
            budget.work(),
            budget.peak_storage(),
            budget.failed_storage(),
        )
    };
    (result, accepted, peak, failed_storage, work.failed_work())
}

#[test]
fn source_licm_exact_work_and_one_short_keep_first_denial_and_live_sibling() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for mutation in [false, true] {
            let (result, work, peak, storage_denial, work_denial) =
                measured(profile, mutation, WORK, STORAGE);
            result.unwrap();
            assert_eq!((storage_denial, work_denial), (None, None));
            let (result, accepted, actual_peak, storage_denial, work_denial) =
                measured(profile, mutation, work, peak);
            result.unwrap();
            assert_eq!(
                (accepted, actual_peak, storage_denial, work_denial),
                (work, peak, None, None)
            );
            let (result, accepted, actual_peak, storage_denial, work_denial) =
                measured(profile, mutation, work - 1, peak);
            match result {
                Err(LicmError::Resource(AssertOriginResourceV1::Work(error))) => {
                    assert_eq!(error.actual(), work);
                    assert_eq!(error.limit(), work - 1);
                }
                other => panic!("exact final one-unit source-LICM work refusal: {other:?}"),
            }
            assert_eq!(
                (accepted, actual_peak, storage_denial, work_denial),
                (work - 1, peak, None, Some(work))
            );
        }
    }
}

#[test]
fn source_licm_one_short_storage_preserves_exact_nested_phase() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for mutation in [false, true] {
            let (result, work, peak, storage_denial, work_denial) =
                measured(profile, mutation, WORK, STORAGE);
            result.unwrap();
            assert_eq!((storage_denial, work_denial), (None, None));
            // The historical V20 baseline below predates the inline optional
            // nominal-helper relation. This phase retains one source owner,
            // so pay its actual new header once without changing work or the
            // exact nested first-denial phase on either target profile.
            let nominal_header = std::mem::size_of::<Option<Box<SealedBf16CallRelationV1>>>();
            let expected = if mutation {
                (
                    2_733_569,
                    3_335_168 + nominal_header,
                    2_686_896,
                    3_309_341 + nominal_header,
                )
            } else {
                (
                    249_631,
                    1_949_434 + nominal_header,
                    238_296,
                    1_933_493 + nominal_header,
                )
            };
            assert_eq!((work, peak), (expected.0, expected.1));
            let (result, accepted, actual_peak, storage_denial, work_denial) =
                measured(profile, mutation, work, peak - 1);
            let error =
                result.expect_err("one-short storage cannot complete the same allocation history");
            assert_eq!((storage_denial, work_denial), (Some(peak), None));
            let LicmError::Admission(error) = error else {
                panic!("expected final-source admission Storage refusal")
            };
            let crate::ProductionPrivateCellPromotionContinuationErrorV1::Prefix(error) = *error
            else {
                panic!("expected retained private-cell prefix refusal")
            };
            let crate::ProductionCommutativeContinuationErrorV1::Prefix(error) = *error else {
                panic!("expected retained commutative prefix refusal")
            };
            let crate::ProductionRedundantStoreAdmissionErrorV1::Prefix(error) = *error else {
                panic!("expected retained redundant-store prefix refusal")
            };
            let crate::ProductionCheckedOutputAdmissionErrorPolicy6V1::Optimization(
                fe2o3_kernel_opt::CanonicalPolicy6OptimizationErrorV1::Map(
                    fe2o3_pliron::KirOptimizationMapErrorV12::Resources(
                        fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Storage(
                            error,
                        ),
                    ),
                ),
            ) = *error
            else {
                panic!("expected exact Policy6 replay map Storage refusal")
            };
            assert_eq!((error.actual(), error.limit()), (peak, peak - 1));
            assert_eq!((accepted, actual_peak), (expected.2, expected.3));
        }
    }
}

#[test]
fn source_licm_nested_error_and_panic_drop_actual_moved_candidate() {
    let (prefix, inherited) = direct::prefix(Profile::Gfx942, true);
    prefix.exercise_licm_failed_candidate_v1(inherited);
}
