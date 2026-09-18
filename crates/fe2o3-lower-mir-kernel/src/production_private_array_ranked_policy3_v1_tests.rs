use super::*;
use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
use fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1;

// Admitted semantic/ranked components, not a rustc-source or GPU qualification.
fn with_ranked_policy3_output(
    case: ArrayCase,
    profile: Profile,
    next: impl FnOnce(
        &ProductionMaterializedRankedModuleReceiptV1,
        &VerifiedCanonicalKernelIrModuleV12,
        &CheckedNeutralKernelIrOwnerPolicy3V1,
        &mut AssertOriginBudgetV1<'_>,
    ),
) {
    let receipt = array_output_ranked_receipt_v1(array_owner(case));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    let source_storage = receipt.materialized.retained_analysis_storage_v1();
    budget.reserve_storage(FLOOR + source_storage).unwrap();
    let binding = dialect_amdgcn::bind_production_target_v1(
        receipt.materialized.executable().module(),
        profile,
    )
    .unwrap();
    let (bound, bound_storage) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            binding.module(),
            &mut budget,
        )
        .unwrap();
    budget
        .reserve_storage(bound_storage.retained_storage())
        .unwrap();
    drop(binding);
    let (coordinates, coordinate_storage) =
        dialect_amdgcn::check_production_target_coordinate_preservation_v1(
            receipt.materialized.executable(),
            &bound,
            profile,
            &mut budget,
        )
        .unwrap();
    budget
        .reserve_storage(coordinate_storage.retained_storage())
        .unwrap();
    assert!(std::ptr::eq(coordinates.output(), &bound));
    let checked =
        fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy3_v1(&bound, &mut budget)
            .unwrap();
    let checked_storage = checked.storage().retained_storage();
    budget.reserve_storage(checked_storage).unwrap();
    let floor = budget.storage();
    next(&receipt, &bound, &checked, &mut budget);
    assert_eq!(budget.storage(), floor);
    drop(checked);
    budget.release_storage(checked_storage).unwrap();
    drop(coordinates);
    budget
        .release_storage(coordinate_storage.retained_storage())
        .unwrap();
    drop(bound);
    budget
        .release_storage(bound_storage.retained_storage())
        .unwrap();
    drop(receipt);
    budget.release_storage(source_storage).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn policy3_private_ranked_facts_join_actual_target_output_and_fresh_formal_analysis() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        with_ranked_policy3_output(
            ArrayCase::Initializer {
                values: [0, 1, 2, 7, 31, 255, 1024, u32::MAX],
                repetitions: 2,
                float: false,
            },
            profile,
            |receipt, bound, output, budget| {
                let formal =
                    crate::analyze_checked_output_formal_memory_policy3_v1(output).unwrap();
                assert!(std::ptr::eq(formal.output(), output.owner()));
                assert_eq!(formal.kernels().len(), 1);
                drop(formal);
                receipt
                    .with_checked_private_array_output_policy3_v1(
                        bound,
                        output,
                        budget,
                        |scope, budget| {
                            assert_eq!(scope.len(budget)?, 17);
                            assert!(!scope.grants_artifact_or_launch_authority());
                            for index in 0..17 {
                                let fact = scope.get(index, budget)?.unwrap();
                                assert!(!fact.grants_authority());
                                let store = fact.output().unwrap();
                                assert!(store.executable());
                                let operation = source_output_operation_v1(
                                    output.owner(),
                                    store.access().operation,
                                    budget,
                                )
                                .unwrap();
                                assert!(matches!(operation.kind,
                                OperationKind::Store { value, .. } if value == store.value()));
                            }
                            assert!(scope.get(17, budget)?.is_none());
                            Ok(())
                        },
                    )
                    .unwrap();
            },
        );
    }
}

#[test]
fn policy3_private_ranked_scope_rejects_foreign_history_before_callback() {
    with_ranked_policy3_output(
        ArrayCase::Write { sparse: false },
        Profile::Gfx942,
        |receipt, _, output, budget| {
            let called = std::cell::Cell::new(false);
            let result = receipt.with_checked_private_array_output_policy3_v1(
                receipt.materialized.executable(),
                output,
                budget,
                |_, _| {
                    called.set(true);
                    Ok(())
                },
            );
            assert!(matches!(
                result,
                Err(ProductionSourceOutputErrorV1::InputCustody)
            ));
            assert!(!called.get());
        },
    );
}

#[test]
fn policy3_private_ranked_scope_preserves_callback_errors_panics_and_floor() {
    with_ranked_policy3_output(
        ArrayCase::Write { sparse: false },
        Profile::Gfx942,
        |receipt, bound, output, budget| {
            let incoming = budget.storage();
            let result: Result<(), _> = receipt.with_checked_private_array_output_policy3_v1(
                bound,
                output,
                budget,
                |scope, budget| {
                    assert_eq!(scope.len(budget)?, 1);
                    Err(ProductionSourceOutputErrorV1::Invalid("caller error"))
                },
            );
            assert!(matches!(
                result,
                Err(ProductionSourceOutputErrorV1::Invalid("caller error"))
            ));
            assert_eq!(budget.storage(), incoming);
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _: Result<(), _> = receipt.with_checked_private_array_output_policy3_v1(
                    bound,
                    output,
                    budget,
                    |scope, budget| {
                        assert_eq!(scope.len(budget)?, 1);
                        std::panic::panic_any(271_u32);
                    },
                );
            }))
            .unwrap_err();
            assert_eq!(panic.downcast_ref::<u32>(), Some(&271));
            assert_eq!(budget.storage(), incoming);
        },
    );
}

#[test]
fn policy3_private_ranked_scope_charges_entry_and_actual_header_before_allocation() {
    with_ranked_policy3_output(
        ArrayCase::Write { sparse: false },
        Profile::Gfx942,
        |receipt, bound, output, budget| {
            let floor = budget.storage();
            let header = std::mem::size_of::<CheckedPrivateArrayOutputRankedV1<'_>>();
            for work_limit in [7, 8] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
                let mut denied = AssertOriginBudgetV1::new(&mut work, floor + header - 1);
                denied.reserve_storage(floor).unwrap();
                let result = receipt.with_checked_private_array_output_policy3_v1(
                    bound,
                    output,
                    &mut denied,
                    |_, _| -> Result<(), _> { panic!("resource denial must precede callback") },
                );
                if work_limit == 7 {
                    assert!(
                        matches!(result, Err(ProductionSourceOutputErrorV1::Resource(
                        AssertOriginResourceV1::Work(error))) if error.actual() == 8 && error.limit() == 7)
                    );
                    assert_eq!(denied.work(), 0);
                } else {
                    assert!(
                        matches!(result, Err(ProductionSourceOutputErrorV1::Resource(
                        AssertOriginResourceV1::Storage(error)))
                        if error.actual() == floor + header && error.limit() == floor + header - 1)
                    );
                    assert_eq!(denied.work(), 8);
                }
                assert_eq!((denied.storage(), denied.peak_storage()), (floor, floor));
            }
        },
    );
}

#[test]
fn policy3_private_ranked_scope_rejects_another_ledger_and_a_released_floor() {
    with_ranked_policy3_output(
        ArrayCase::Write { sparse: false },
        Profile::Gfx942,
        |receipt, bound, output, budget| {
            receipt
                .with_checked_private_array_output_policy3_v1(
                    bound,
                    output,
                    budget,
                    |scope, budget| {
                        let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(WORK);
                        let mut foreign = AssertOriginBudgetV1::new(&mut foreign_work, STORAGE);
                        foreign.reserve_storage(budget.storage()).unwrap();
                        assert!(matches!(
                            scope.len(&mut foreign),
                            Err(ProductionSourceOutputErrorV1::Resource(
                                AssertOriginResourceV1::Accounting
                            ))
                        ));
                        assert!(matches!(
                            scope.get(0, &mut foreign),
                            Err(ProductionSourceOutputErrorV1::Resource(
                                AssertOriginResourceV1::Accounting
                            ))
                        ));
                        assert_eq!(scope.len(budget)?, 1);
                        Ok(())
                    },
                )
                .unwrap();
            let result = receipt.with_checked_private_array_output_policy3_v1(
                bound,
                output,
                budget,
                |scope, budget| {
                    budget.release_storage(1).unwrap();
                    assert!(matches!(
                        scope.get(0, budget),
                        Err(ProductionSourceOutputErrorV1::Resource(
                            AssertOriginResourceV1::Accounting
                        ))
                    ));
                    Ok(())
                },
            );
            assert!(matches!(
                result,
                Err(ProductionSourceOutputErrorV1::Resource(
                    AssertOriginResourceV1::Accounting
                ))
            ));
        },
    );
}
