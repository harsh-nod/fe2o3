//! Genuine shared-join CPU tests, not signed collector/protected-stage fixtures.
use super::*;
use crate::production_pipeline::{
    checked_output_policy4_v1::prepare_checked_output_artifacts_v1,
    erased_checked_output_policy4_v1::prepare_erased_checked_output_artifacts_v1,
    native_checked_output_handoff_v1::{
        NativeOutputHandoffErrorV1 as E, OutputInputsV1, SourceInputsV1, check_output_inputs_v1,
        check_source_inputs_v1,
    },
};
use crate::production_ranked_projection_v1::{
    with_backend_checked_output_policy4_owned_v1, with_backend_erased_bound_v1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work, InertCanonicalKernelIrContractCatalogV1 as Catalog,
};
use fe2o3_lower_mir_kernel::{
    ProductionSourceLaunchInputV1, ProductionSourceLaunchRootInputV1,
    ProductionSourceLaunchRosterV1, ProductionUnitLocalErasedCheckedOutputOwnerPolicy4V1,
};

fn with_inputs(
    erased: bool,
    profile: ProductionAmdTargetProfileV1,
    next: impl FnOnce(OutputInputsV1<'_>, &mut [TypedDescriptorRootV1], &mut Budget<'_>),
) {
    if erased {
        with_backend_erased_bound_v1(true, 2, profile, |source, bound, budget| {
            let mut roots = super::erased_native_handoff_tests::erased_typed_roots(&source);
            let checked =
                fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy4_v1(&bound, budget)
                    .unwrap();
            assert_eq!(checked.forwarding_rows().len(), 2);
            let checked_bytes = checked.retained_storage();
            budget.reserve_storage(checked_bytes).unwrap();
            let owner = ProductionUnitLocalErasedCheckedOutputOwnerPolicy4V1::try_admit_v1(
                source, bound, checked, budget,
            )
            .unwrap();
            let (artifacts, receipt) =
                prepare_erased_checked_output_artifacts_v1(owner, profile, &roots, None, budget)
                    .unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            let floor = budget.storage();
            next(artifacts.native_worker_output_v1(), &mut roots, budget);
            assert_eq!(budget.storage(), floor);
            drop(artifacts);
            budget
                .release_storage(receipt.retained_storage() + checked_bytes)
                .unwrap();
        });
    } else {
        with_backend_checked_output_policy4_owned_v1(profile, |owner, budget| {
            let mut roots = typed_roots_for_source(owner.source_semantic_kir());
            let (artifacts, receipt) =
                prepare_checked_output_artifacts_v1(owner, profile, &roots, None, budget).unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            let floor = budget.storage();
            next(artifacts.native_worker_output_v1(), &mut roots, budget);
            assert_eq!(budget.storage(), floor);
            drop(artifacts);
            budget.release_storage(receipt.retained_storage()).unwrap();
        });
    }
}

#[test]
fn shared_binding_replays_actual_two_root_o_on_both_routes_and_profiles() {
    for erased in [false, true] {
        for profile in [
            ProductionAmdTargetProfileV1::Gfx942,
            ProductionAmdTargetProfileV1::Gfx950,
        ] {
            with_inputs(erased, profile, |inputs, roots, budget| {
                let source = inputs.owner.source(inputs.catalog).unwrap();
                assert_eq!(source.erased.is_some(), erased);
                assert_eq!(source.launch.roots().len(), 2);
                assert_eq!(inputs.owner.output().module().kernels.len(), 2);
                check_source_inputs_v1(source, source, budget).unwrap();
                check_output_inputs_v1(inputs, profile, roots, budget).unwrap();
                // No fabricated signed source or authenticated rustc transaction
                // is made by this component-only success.
            });
        }
    }
}

#[test]
fn shared_binding_refuses_profile_root_order_and_workgroup_roster_substitution() {
    for erased in [false, true] {
        let profile = ProductionAmdTargetProfileV1::Gfx942;
        with_inputs(erased, profile, |inputs, roots, budget| {
            let floor = budget.storage();
            assert!(
                check_output_inputs_v1(inputs, ProductionAmdTargetProfileV1::Gfx950, roots, budget)
                    .is_err()
            );
            assert_eq!(budget.storage(), floor);
            assert!(check_output_inputs_v1(inputs, profile, &roots[..1], budget).is_err());
            assert_eq!(budget.storage(), floor);
            roots.swap(0, 1);
            assert!(check_output_inputs_v1(inputs, profile, roots, budget).is_err());
            roots.swap(0, 1);
            assert_eq!(budget.storage(), floor);
            let missing = OutputInputsV1 {
                workgroups: &inputs.workgroups[..1],
                ..inputs
            };
            assert!(matches!(
                check_output_inputs_v1(missing, profile, roots, budget),
                Err(E::Mismatch("complete actual-O workgroup roster"))
            ));
            assert_eq!(budget.storage(), floor);
            check_output_inputs_v1(inputs, profile, roots, budget).unwrap();
        });
    }
}

#[test]
fn source_join_refuses_n_e_route_catalog_and_launch_transplants() {
    for erased in [false, true] {
        with_inputs(
            erased,
            ProductionAmdTargetProfileV1::Gfx942,
            |inputs, _, budget| {
                let original = inputs.owner.source(inputs.catalog).unwrap();
                let wrong_n = SourceInputsV1 {
                    original: inputs.owner.output(),
                    ..original
                };
                assert!(matches!(
                    check_source_inputs_v1(original, wrong_n, budget),
                    Err(E::Mismatch("exact original native N"))
                ));
                let wrong_route = SourceInputsV1 {
                    erased: if erased {
                        None
                    } else {
                        Some(original.original)
                    },
                    ..original
                };
                assert!(matches!(
                    check_source_inputs_v1(original, wrong_route, budget),
                    Err(E::Mismatch("direct/erased source route"))
                ));
                if erased {
                    let wrong_e = SourceInputsV1 {
                        erased: Some(original.original),
                        ..original
                    };
                    assert!(matches!(
                        check_source_inputs_v1(original, wrong_e, budget),
                        Err(E::Mismatch("exact independently reconstructed E"))
                    ));
                }
                let (catalog, receipt) =
                    Catalog::from_rows_with_budget([253; 32], &[], &[], budget).unwrap();
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                assert!(matches!(
                    check_source_inputs_v1(
                        original,
                        SourceInputsV1 {
                            catalog: &catalog,
                            ..original
                        },
                        budget
                    ),
                    Err(E::Mismatch("exact original/output contract catalog"))
                ));
                drop(catalog);
                budget.release_storage(receipt.retained_storage()).unwrap();

                // New inert launch inputs keep the same admitted source and exact
                // workgroup, changing only the detached max-grid source contract.
                // Fixture allocations are outside the production checker ledger.
                let launch_rows: Vec<_> = original
                    .launch
                    .roots()
                    .iter()
                    .enumerate()
                    .map(|(index, root)| {
                        let old = root.source_launch();
                        let mut grid = old.max_grid();
                        grid[0] += 1;
                        ProductionSourceLaunchRootInputV1::new(
                            if index == 0 { "first" } else { "second" },
                            root.kernel_binding(),
                            ProductionSourceLaunchInputV1::new(
                                old.rank(),
                                old.exact_workgroup(),
                                grid,
                            ),
                        )
                    })
                    .collect();
                let launch =
                    ProductionSourceLaunchRosterV1::try_new(original.semantic, &launch_rows)
                        .unwrap();
                assert!(matches!(
                    check_source_inputs_v1(
                        original,
                        SourceInputsV1 {
                            launch: &launch,
                            ..original
                        },
                        budget
                    ),
                    Err(E::Mismatch("exact original source launch roster"))
                ));
            },
        );
    }
}

#[test]
fn component_work_and_storage_exact_limits_preserve_complete_input_floor() {
    for erased in [false, true] {
        let profile = ProductionAmdTargetProfileV1::Gfx942;
        with_inputs(erased, profile, |inputs, roots, inherited| {
            let floor = inherited.storage();
            let mut work = Work::new(usize::MAX);
            let mut budget = Budget::new(&mut work, usize::MAX);
            budget.reserve_storage(floor).unwrap();
            check_output_inputs_v1(inputs, profile, roots, &mut budget).unwrap();
            let cost = budget.work();
            let peak = budget.peak_storage();
            assert_eq!(budget.storage(), floor);
            assert!(cost > 4);
            assert!(peak > floor);
            for (work_limit, storage_limit, succeeds) in [
                (cost, peak, true),
                (cost - 1, peak, false),
                (cost, peak - 1, false),
            ] {
                let mut work = Work::new(work_limit);
                let mut budget = Budget::new(&mut work, storage_limit);
                budget.reserve_storage(floor).unwrap();
                assert_eq!(
                    check_output_inputs_v1(inputs, profile, roots, &mut budget).is_ok(),
                    succeeds
                );
                assert_eq!(budget.storage(), floor);
                assert!(budget.work() > 0);
                if work_limit < cost {
                    assert!(budget.work() < cost);
                } else if !succeeds {
                    assert!(budget.failed_storage().is_some());
                }
            }
        });
    }
}

#[test]
fn source_join_rejects_a_distinct_genuine_semantic_source_before_graph_checks() {
    let profile = ProductionAmdTargetProfileV1::Gfx942;
    with_inputs(false, profile, |left, _, budget| {
        let original = left.owner.source(left.catalog).unwrap();
        with_inputs(true, profile, |right, _, _| {
            let other = right.owner.source(right.catalog).unwrap();
            let changed = SourceInputsV1 {
                semantic: other.semantic,
                ..original
            };
            assert!(matches!(
                check_source_inputs_v1(original, changed, budget),
                Err(E::Mismatch("exact original semantic source"))
            ));
        });
    });
}

#[test]
fn genuine_cross_handoff_substitution_reaches_and_fails_actual_o_native_join() {
    for profile in [
        ProductionAmdTargetProfileV1::Gfx942,
        ProductionAmdTargetProfileV1::Gfx950,
    ] {
        for erased in [false, true] {
            with_inputs(erased, profile, |original, roots, budget| {
                check_output_inputs_v1(original, profile, roots, budget).unwrap();
                let floor = budget.storage();
                with_inputs(!erased, profile, |other, other_roots, other_budget| {
                    check_output_inputs_v1(other, profile, other_roots, other_budget).unwrap();
                    assert_ne!(
                        original.owner.output().canonical().canonical_bytes(),
                        other.owner.output().canonical().canonical_bytes(),
                    );
                    let (expected, _, _) = original.prepared.native_output_parts_v1();
                    let (substituted, _, _) = other.prepared.native_output_parts_v1();
                    assert_eq!(expected.target(), substituted.target());
                    assert_eq!(
                        expected.code_object_version(),
                        substituted.code_object_version()
                    );
                    assert_ne!(expected.module_bytes(), substituted.module_bytes());

                    // Both complete genuine input sets coexist. Only the
                    // prepared handoff changes; source/owner/catalog/roots,
                    // profile, and actual-O workgroups stay original.
                    let other_floor = other_budget.storage();
                    budget.reserve_storage(other_floor).unwrap();
                    let retained = budget.storage();
                    let changed = OutputInputsV1 {
                        prepared: other.prepared,
                        ..original
                    };
                    assert!(matches!(
                        check_output_inputs_v1(changed, profile, roots, budget),
                        Err(E::Native(_))
                    ));
                    assert_eq!(budget.storage(), retained);
                    budget.release_storage(other_floor).unwrap();
                });
                assert_eq!(budget.storage(), floor);
                check_output_inputs_v1(original, profile, roots, budget).unwrap();
            });
        }
    }
}

#[test]
fn handoff_diagnostics_preserve_error_details() {
    let prefix = "native checked-output worker binding: ";
    assert_eq!(
        E::Mismatch("exact native target").to_string(),
        format!("{prefix}exact native target")
    );
    assert_eq!(
        E::Resource(Resource::Accounting).to_string(),
        format!("{prefix}resource: {}", Resource::Accounting)
    );
    assert_eq!(
        E::Panicked.to_string(),
        format!("{prefix}validation panicked")
    );
}

#[test]
fn shared_scratch_scope_restores_error_and_panic_without_refunding_work() {
    use crate::production_pipeline::native_checked_output_handoff_v1::scoped;
    for panic in [false, true] {
        let mut work = Work::new(100);
        let mut budget = Budget::new(&mut work, 100);
        budget.reserve_storage(11).unwrap();
        let result: Result<(), E> = scoped(&mut budget, |budget| {
            budget.charge_work(7)?;
            budget.reserve_storage(13)?;
            assert!(!panic, "test-only checker unwind");
            Err(E::Mismatch("test-only checked error"))
        });
        if panic {
            assert!(matches!(result, Err(E::Panicked)));
        } else {
            assert!(matches!(
                result,
                Err(E::Mismatch("test-only checked error"))
            ));
        }
        assert_eq!(budget.storage(), 11);
        assert_eq!(budget.peak_storage(), 24);
        assert_eq!(budget.work(), 7);
    }
}

#[test]
fn shared_scratch_scope_rejects_floor_loss_and_replacement_work_ledger() {
    use crate::production_pipeline::native_checked_output_handoff_v1::scoped;
    use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
    let mut work = Work::new(100);
    let mut foreign = Work::new(100);
    let mut budget = Budget::new(&mut work, 100);
    budget.reserve_storage(11).unwrap();
    let result = scoped(&mut budget, |budget| {
        budget.release_storage(1)?;
        Ok(())
    });
    assert!(matches!(result, Err(E::Resource(Resource::Accounting))));
    budget.reserve_storage(1).unwrap();
    let result = scoped(&mut budget, |budget| {
        let mut replacement = Budget::new(&mut foreign, 100);
        replacement.reserve_storage(11)?;
        *budget = replacement;
        Ok(())
    });
    assert!(matches!(result, Err(E::Resource(Resource::Accounting))));
    assert_eq!(budget.storage(), 11);
}
