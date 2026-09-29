fn mixed_native_launches_v26(
    original: &ProductionSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Vec<fe2o3_kernel_ir::ExplicitLaunchExtent>> {
    Ok(vec![
        fe2o3_kernel_ir::ExplicitLaunchExtent::Exact {
            rank: 1,
            extents: [64, 1, 1],
        };
        original.source.root_count(budget)?
    ])
}

#[test]
fn mixed_native_completion_covers_every_original_root_and_nested_source_helper() {
    for nested_helpers in [false, true] {
        let owner = if nested_helpers {
            global_expression_helper_owner_v23(true)
        } else {
            issued_metadata_two_root_owner_v18()
        };
        let expected_roots = owner.source_semantic().roots().len();
        assert_eq!(expected_roots, if nested_helpers { 1 } else { 2 });
        let abi = issued_descriptor_role_abi_v18(&owner);
        let reached = std::cell::Cell::new(false);
        let (result, _, _) = run_descriptor_role_owner_with_abi_v18(
            owner,
            abi,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |original, optimized, budget| {
                if nested_helpers {
                    assert!(original.source.instance_count(0, budget)? >= 3);
                }
                let floor = budget.storage();
                let launches = mixed_native_launches_v26(original, budget)?;
                let output = optimized.output_inventory(budget)?;
                with_mixed_source_completion_v26(
                    original,
                    optimized,
                    &launches,
                    fe2o3_kernel_ir::FormalIndexWidth::Bits64,
                    budget,
                    &mut |native, budget| {
                        native.check_source_subject_v26(original, optimized, budget)?;
                        let census = native.source_census(budget)?;
                        assert_eq!(census[0], expected_roots);
                        assert!(census[3] > 0);
                        assert_eq!(native.function_count(budget)?, output.functions().len());
                        let mut definitions = 0;
                        for (ordinal, function) in output.functions().iter().enumerate() {
                            let report = native.report(ordinal, budget)?;
                            let history = native.history(ordinal, budget)?;
                            if function.function.body.is_some() {
                                definitions += 1;
                                let report = report.expect("each actual definition has a report");
                                assert_eq!(report.paired_stage_count(), 9);
                                assert!(report.reports().is_clean());
                                assert!(history.is_some());
                            } else {
                                assert!(report.is_none());
                                assert!(history.is_none());
                            }
                        }
                        assert!(definitions >= expected_roots);
                        let premises = native.runtime_premises(budget)?;
                        let occurrences = native.runtime_occurrences(budget)?;
                        for root in 0..expected_roots {
                            assert!(premises.iter().any(|premise| premise.root() == root));
                        }
                        for occurrence in occurrences {
                            let premise = &premises[occurrence.premise_index()];
                            assert_eq!(premise.launch(), launches[premise.root()]);
                            assert_eq!(
                                premise.index_width(),
                                fe2o3_kernel_ir::FormalIndexWidth::Bits64
                            );
                            assert!(occurrence.requires_address_formation_domain());
                            assert!(!occurrence.grants_artifact_or_launch_authority());
                        }
                        assert!(native.source_roles_are_complete());
                        assert!(!native.runtime_requirements_are_discharged());
                        assert!(!native.ranked_verification_is_complete());
                        assert!(!native.grants_artifact_or_launch_authority());
                        reached.set(true);
                        Ok(())
                    },
                )
                .unwrap();
                assert_eq!(budget.storage(), floor);
                Ok(())
            },
        );
        assert!(
            result.is_ok(),
            "nested_helpers={nested_helpers}: {result:?}"
        );
        assert!(reached.get());
    }
}

#[test]
fn mixed_native_completion_cannot_swallow_invalid_report_or_history_queries() {
    for history in [false, true] {
        let owner = issued_descriptor_role_owner_v18(DescriptorRoleSourceV18::Arithmetic);
        let abi = issued_descriptor_role_abi_v18(&owner);
        let queried = std::cell::Cell::new(false);
        let refused = std::cell::Cell::new(false);
        let (result, _, _) = run_descriptor_role_owner_result_with_abi_v18(
            owner,
            abi,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |original, optimized, budget| {
                let floor = budget.storage();
                let launches = mixed_native_launches_v26(original, budget)?;
                let checked = with_mixed_source_completion_v26(
                    original,
                    optimized,
                    &launches,
                    fe2o3_kernel_ir::FormalIndexWidth::Bits64,
                    budget,
                    &mut |native, budget| {
                        let absent = native.function_count(budget)?;
                        let error = if history {
                            native.history(absent, budget).unwrap_err()
                        } else {
                            match native.report(absent, budget) {
                                Err(error) => error,
                                Ok(_) => panic!("absent mixed function report was accepted"),
                            }
                        };
                        assert!(format!("{error:?}").contains("InvalidQuery"));
                        assert!(native.source_census(budget).is_err());
                        assert!(native.runtime_premises(budget).is_err());
                        queried.set(true);
                        // The enclosing view must reject even a callback that
                        // deliberately ignores its authentic query failure.
                        Ok(())
                    },
                );
                assert!(
                    checked.is_err(),
                    "invalid query published a complete mixed view"
                );
                assert!(format!("{checked:?}").contains("InvalidQuery"));
                assert_eq!(budget.storage(), floor);
                refused.set(true);
                Ok(())
            },
        );
        assert!(queried.get(), "history={history}: {result:?}");
        assert!(refused.get(), "history={history}: {result:?}");
    }
}
