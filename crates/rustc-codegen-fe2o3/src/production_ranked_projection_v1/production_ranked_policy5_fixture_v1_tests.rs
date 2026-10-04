// Included beside the existing genuine direct ranked-bound fixture. No free
// source, ranked, collector or signed-proof owner is fabricated by these tests.
pub(crate) fn with_backend_checked_output_policy5_owned_v1(
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    next: impl FnOnce(
        fe2o3_lower_mir_kernel::ProductionCheckedOutputOwnerPolicy5V1,
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ),
) {
    with_backend_checked_ranked_bound_v1(profile, |receipt, bound, _, budget| {
        let checked =
            fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy5_v1(&bound, budget)
                .unwrap();
        let storage = checked.retained_storage();
        budget.reserve_storage(storage).unwrap();
        let floor = budget.storage();
        let admitted = fe2o3_lower_mir_kernel::ProductionCheckedOutputOwnerPolicy5V1::try_admit_v1(
            receipt, bound, checked, budget,
        )
        .unwrap();
        next(admitted, budget);
        assert_eq!(budget.storage(), floor);
        budget.release_storage(storage).unwrap();
    });
}

pub(crate) fn with_backend_erased_output_policy5_owned_v1(
    expected: bool,
    roots: usize,
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    next: impl FnOnce(
        fe2o3_lower_mir_kernel::ProductionUnitLocalErasedCheckedOutputOwnerPolicy5V1,
        AuthenticatedRankedVerificationRosterV1,
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ),
) {
    with_backend_erased_load_roster_v1(
        expected,
        roots,
        profile,
        |source, bound, ranked, budget| {
            let checked =
                fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy5_v1(&bound, budget)
                    .unwrap();
            assert_eq!(checked.load_forwarding_rows().len(), roots);
            assert!(checked.intermediate_policy4().forwarding_rows().is_empty());
            let storage = checked.retained_storage();
            budget.reserve_storage(storage).unwrap();
            let floor = budget.storage();
            let admitted = fe2o3_lower_mir_kernel::ProductionUnitLocalErasedCheckedOutputOwnerPolicy5V1::try_admit_v1(source, bound, checked, budget).unwrap();
            next(admitted, ranked, budget);
            assert_eq!(budget.storage(), floor);
            budget.release_storage(storage).unwrap();
        },
    );
}
