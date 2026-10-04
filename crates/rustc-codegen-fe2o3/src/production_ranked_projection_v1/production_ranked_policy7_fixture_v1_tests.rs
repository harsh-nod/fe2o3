// Constructed semantic MIR followed by the actual source/SSA/ranked/P6 path.
// No collector, native signed owner or successful proof is fabricated.
pub(crate) fn with_backend_policy7_direct_prefix_v1(
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    duplicate: bool,
    next: impl FnOnce(
        fe2o3_lower_mir_kernel::ProductionCheckedOutputOwnerPolicy6V1,
        AuthenticatedRankedVerificationRosterV1,
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ),
) {
    with_backend_checked_ranked_bound_stores_v1(
        profile,
        Some(if duplicate { 2 } else { 1 }),
        |receipt, bound, ranked, budget| {
            let checked =
                fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy5_v1(&bound, budget)
                    .unwrap();
            budget.reserve_storage(checked.retained_storage()).unwrap();
            let checked = fe2o3_kernel_opt::continue_checked_canonical_kernel_ir_policy6_v1(
                &bound, checked, budget,
            )
            .unwrap();
            let storage = checked.retained_storage();
            budget.reserve_storage(storage).unwrap();
            let floor = budget.storage();
            let owner =
                fe2o3_lower_mir_kernel::ProductionCheckedOutputOwnerPolicy6V1::try_admit_v1(
                    receipt, bound, checked, budget,
                )
                .unwrap();
            next(owner, ranked, budget);
            assert_eq!(budget.storage(), floor);
            budget.release_storage(storage).unwrap();
        },
    );
}

pub(crate) fn with_backend_policy7_erased_prefix_v1(
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    duplicate: bool,
    next: impl FnOnce(
        fe2o3_lower_mir_kernel::ProductionUnitLocalErasedCheckedOutputOwnerPolicy6V1,
        AuthenticatedRankedVerificationRosterV1,
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ),
) {
    canonical_assertion_graph_tests::with_backend_erased_store_roster_v1(
        false,
        2,
        profile,
        true,
        duplicate,
        |source, bound, ranked, budget| {
            assert_eq!(source.deleted_call_count(), 2);
            let checked =
                fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy5_v1(&bound, budget)
                    .unwrap();
            budget.reserve_storage(checked.retained_storage()).unwrap();
            let checked = fe2o3_kernel_opt::continue_checked_canonical_kernel_ir_policy6_v1(
                &bound, checked, budget,
            )
            .unwrap();
            let storage = checked.retained_storage();
            budget.reserve_storage(storage).unwrap();
            let floor = budget.storage();
            let owner = fe2o3_lower_mir_kernel::ProductionUnitLocalErasedCheckedOutputOwnerPolicy6V1::try_admit_v1(source, bound, checked, budget).unwrap();
            next(owner, ranked, budget);
            assert_eq!(budget.storage(), floor);
            budget.release_storage(storage).unwrap();
        },
    );
}
