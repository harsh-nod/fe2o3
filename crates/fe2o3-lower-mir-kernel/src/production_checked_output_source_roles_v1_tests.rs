use super::*;
use fe2o3_kernel_ir::{CanonicalKernelIrWorkBudgetV1, VerifiedCanonicalKernelIrModuleV12};

include!("production_checked_output_source_roles_fixtures_v1_tests.rs");
include!("production_conditional_source_wrapper_v1_tests.rs");

#[test]
fn selected_result_bodies_keep_exact_single_shared_and_distinct_root_roles() {
    for (roots, shared) in [(1, false), (2, false), (2, true)] {
        for helper in [false, true] {
            let fixture = Fixture {
                roots,
                shared,
                helper,
                ..Fixture::default()
            };
            let source = owner(fixture);
            source.verify_equivalence().unwrap();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
            budget.reserve_storage(FLOOR).unwrap();
            let roles = check_source(&source, &mut budget).unwrap();
            for root in 0..roots {
                let id = SemanticFunctionIdV1::from_index(root);
                let selected = source
                    .semantic()
                    .semantic()
                    .select_kernel_body_for_root_v1(id)
                    .unwrap();
                assert_ne!(selected.root(), selected.body());
                assert_eq!(
                    roles.functions[root as usize].selected,
                    Some(selected.body())
                );
                assert_eq!(roles.functions[root as usize].flags, ROOT | ROOT_ENTRY_SEEN);
                assert_eq!(
                    roles.functions[selected.body().index() as usize].flags,
                    SELECTED_BODY
                );
                assert!(
                    !roles
                        .retained_helper(selected.body().index() as usize)
                        .unwrap()
                );
            }
            if helper {
                let last = roles.functions.len() - 1;
                assert!(roles.retained_helper(last).unwrap());
                assert_eq!(roles.functions[last].flags, RETAINED_HELPER);
                assert_eq!(
                    source
                        .correspondence
                        .lowered_functions
                        .iter()
                        .filter(|row| row.role == SemanticKirFunctionRoleV1::InternalHelper)
                        .count(),
                    roots as usize
                );
            }
            assert!(roles.retained_helper(usize::MAX).is_err());
            drop(roles);
            budget.release_storage(budget.storage() - FLOOR).unwrap();
            assert_eq!(budget.storage(), FLOOR);
        }
    }
}

#[test]
fn selected_root_body_may_use_root_intrinsics_without_becoming_a_scalar_helper() {
    let source = owner(Fixture {
        intrinsic: true,
        ..Fixture::default()
    });
    source.verify_equivalence().unwrap();
    assert!(
        source
            .module()
            .functions
            .iter()
            .filter_map(|function| function.body.as_ref())
            .flat_map(|body| &body.blocks)
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(operation.kind, OperationKind::Intrinsic(_)))
    );
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    super::super::census::source(&source, &mut budget).unwrap();
    budget.release_storage(budget.storage() - FLOOR).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn malformed_computing_wrapper_has_no_admitted_body_selection() {
    let semantic = request(Fixture {
        malformed: true,
        ..Fixture::default()
    });
    assert!(
        semantic
            .select_kernel_body_for_root_v1(SemanticFunctionIdV1::from_index(0))
            .is_none()
    );
    // The exact selector rejects this shape before it can confer the selected
    // role. Merely being a two-block Unit function is not wrapper authority.
}

#[test]
fn selected_entry_missing_duplicate_wrong_body_and_foreign_root_fail_closed() {
    for mutation in 0..6 {
        let mut source = owner(Fixture {
            roots: 2,
            ..Fixture::default()
        });
        let mut rows = source.correspondence.lowered_functions.to_vec();
        match mutation {
            0 => {
                rows.remove(0);
            }
            1 => rows.push(rows[0].clone()),
            2 => rows[0].semantic_function = rows[0].correspondence_owner,
            3 => rows[0].semantic_function = rows[1].semantic_function,
            4 => rows[0].correspondence_owner = rows[0].semantic_function,
            5 => rows[0].correspondence_owner = SemanticFunctionIdV1::from_index(u32::MAX),
            _ => unreachable!(),
        }
        source.correspondence.lowered_functions = rows.into_boxed_slice();
        assert!(
            run_census(&source, WORK, STORAGE).0.is_err(),
            "mutation {mutation}"
        );
        assert!(
            source.verify_equivalence().is_err(),
            "original source/N join remains mandatory"
        );
    }
}

#[test]
fn selected_body_also_claimed_as_retained_helper_cannot_skip_scalar_abi() {
    let mut source = owner(Fixture::default());
    let mut rows = source.correspondence.lowered_functions.to_vec();
    let mut additional = rows[0].clone();
    additional.role = SemanticKirFunctionRoleV1::InternalHelper;
    rows.push(additional);
    source.correspondence.lowered_functions = rows.into_boxed_slice();
    assert!(matches!(
        run_census(&source, WORK, STORAGE).0,
        Err(E::Unsupported {
            phase: "scalar helpers",
            detail: "direct scalar or exact Unit source return",
        })
    ));
    assert!(source.verify_equivalence().is_err());
}

#[test]
fn selected_roles_do_not_replace_exact_native_ids_or_retained_helper_roster_replay() {
    for mutation in 0..3 {
        let mut source = owner(Fixture {
            helper: true,
            ..Fixture::default()
        });
        let mut rows = source.correspondence.lowered_functions.to_vec();
        match mutation {
            0 => rows[0].kernel_ir_function = FunctionId::new("wrong_selected_native_entry"),
            1 => rows.push(rows.last().unwrap().clone()),
            2 => {
                rows.pop();
            }
            _ => unreachable!(),
        }
        source.correspondence.lowered_functions = rows.into_boxed_slice();
        assert!(source.verify_equivalence().is_err(), "mutation {mutation}");
        if mutation == 2 {
            assert!(run_census(&source, WORK, STORAGE).0.is_err());
        } else {
            // These fields are deliberately the existing reconstruction's
            // responsibility, not a claim made by this smaller role census.
            assert!(run_census(&source, WORK, STORAGE).0.is_ok());
        }
    }
}

#[test]
fn selected_roles_meter_selector_scans_and_exact_work_storage_boundaries() {
    let source = owner(Fixture {
        roots: 2,
        shared: true,
        helper: true,
        ..Fixture::default()
    });
    let (result, exact_work, exact_storage) = run_census(&source, WORK, STORAGE);
    result.unwrap();
    assert!(exact_work > 0 && exact_storage > 0);
    assert!(
        run_census(&source, exact_work, FLOOR + exact_storage)
            .0
            .is_ok()
    );
    assert!(matches!(
        run_census(&source, exact_work - 1, STORAGE).0,
        Err(E::Resource(AssertOriginResourceV1::Work(_)))
    ));
    assert!(matches!(
        run_census(&source, WORK, FLOOR + exact_storage - 1).0,
        Err(E::Resource(AssertOriginResourceV1::Storage(_)))
    ));
    assert!(matches!(
        run_census(&source, 2, FLOOR).0,
        Err(E::Resource(AssertOriginResourceV1::Work(_)))
    ));
    let padded = owner(Fixture {
        administrative: 19,
        ..Fixture::default()
    });
    let plain = owner(Fixture::default());
    let (_, plain_work, plain_storage) = run_census(&plain, WORK, STORAGE);
    let (result, padded_work, padded_storage) = run_census(&padded, WORK, STORAGE);
    result.unwrap();
    assert_eq!(padded_work - plain_work, 19 * 8);
    assert_eq!(padded_storage, plain_storage);
}

fn bound_input(
    receipt: &ProductionMaterializedRankedModuleReceiptV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> (VerifiedCanonicalKernelIrModuleV12, usize) {
    let binding = dialect_amdgcn::bind_production_target_v1(
        receipt.materialized.executable().module(),
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
    )
    .unwrap();
    let (bound, storage) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            binding.module(),
            budget,
        )
        .unwrap();
    let storage = storage.retained_storage();
    budget.reserve_storage(storage).unwrap();
    (bound, storage)
}

#[test]
fn selected_result_wrappers_reach_real_policy3_and_policy4_admission() {
    for fixture in [
        Fixture::default(),
        Fixture {
            roots: 2,
            shared: true,
            helper: true,
            ..Fixture::default()
        },
        Fixture {
            roots: 2,
            shared: false,
            ..Fixture::default()
        },
    ] {
        for policy4 in [false, true] {
            let receipt = receipt(fixture);
            let source_storage = receipt
                .materialized
                .unit_local_source_storage_floor_v1()
                .unwrap();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
            budget.reserve_storage(FLOOR + source_storage).unwrap();
            let (bound, _) = bound_input(&receipt, &mut budget);
            if policy4 {
                let checked = fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy4_v1(
                    &bound,
                    &mut budget,
                )
                .unwrap();
                assert_eq!(
                    checked.forwarding_rows().len(),
                    fixture.roots.max(1) as usize
                );
                budget.reserve_storage(checked.retained_storage()).unwrap();
                let floor = budget.storage();
                let actual = *checked.owner().canonical().identity();
                let admitted = crate::ProductionCheckedOutputOwnerPolicy4V1::try_admit_v1(
                    receipt,
                    bound,
                    checked,
                    &mut budget,
                )
                .unwrap();
                assert_eq!(admitted.output().canonical().identity(), &actual);
                assert_eq!(admitted.kernels().len(), fixture.roots.max(1) as usize);
                assert!(!admitted.grants_artifact_or_launch_authority());
                admitted.verify_equivalence(&mut budget).unwrap();
                assert_eq!(budget.storage(), floor);
            } else {
                let checked = fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy3_v1(
                    &bound,
                    &mut budget,
                )
                .unwrap();
                budget
                    .reserve_storage(checked.storage().retained_storage())
                    .unwrap();
                let floor = budget.storage();
                let actual = *checked.owner().canonical().identity();
                let admitted = crate::ProductionCheckedOutputOwnerPolicy3V1::try_admit_general_v1(
                    receipt,
                    bound,
                    checked,
                    &mut budget,
                )
                .unwrap();
                assert_eq!(admitted.output().canonical().identity(), &actual);
                assert_eq!(admitted.kernels().len(), fixture.roots.max(1) as usize);
                assert!(!admitted.grants_artifact_or_launch_authority());
                admitted.verify_equivalence(&mut budget).unwrap();
                assert_eq!(budget.storage(), floor);
            }
        }
    }
}
