mod checked_output_session_tests {
    use super::super::super::checked_output_session_v1::with_checked_output_assertions_budget_v1;
    use super::*;
    use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
    use fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12 as Owner;
    use fe2o3_lower_mir_kernel::{
        ProductionPreRankedKirOwnerV1, ProductionSourceOutputBlockV1,
        ProductionSourceOutputErrorV1, derive_source_output_occurrences_v1,
    };
    use fe2o3_pliron::CheckedNeutralKernelIrOwnerV1;

    const LIMIT: usize = 1_000_000_000_000;
    const STORAGE_LIMIT: usize = 1_000_000_000;
    const PREFIX: usize = 23;

    include!("source_output_pipeline_catalog_v1_tests.rs");
    include!("checked_output_root_projection_v1_tests.rs");
    include!("checked_control_v1_tests.rs");
    include!("source_output_global_accesses_v1_tests.rs");
    include!("shared_slice_metadata_v1_tests.rs");

    fn with_actual(
        source: &ProductionPreRankedKirOwnerV1,
        profile: Profile,
        body: impl FnOnce(&Owner, &CheckedNeutralKernelIrOwnerV1, &mut Budget<'_>),
    ) {
        let bound =
            dialect_amdgcn::bind_production_target_v1(source.executable().module(), profile)
                .unwrap();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
        budget.charge_work(7).unwrap();
        budget.reserve_storage(PREFIX).unwrap();
        let source_bytes = source.executable_storage().retained_storage()
            + source.assert_origin_storage().payload_storage();
        budget.reserve_storage(source_bytes).unwrap();
        let (input, receipt) =
            Owner::from_module_ref_with_verification_budget_v12(bound.module(), &mut budget)
                .unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let checked =
            fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_v1(&input, &mut budget).unwrap();
        let output_bytes = checked.storage().retained_storage();
        budget.reserve_storage(output_bytes).unwrap();
        let floor = budget.storage();
        body(&input, &checked, &mut budget);
        assert_eq!(budget.storage(), floor);
        assert!(!checked.grants_authority());
        drop(checked);
        budget.release_storage(output_bytes).unwrap();
        drop(input);
        budget.release_storage(receipt.retained_storage()).unwrap();
        budget.release_storage(source_bytes).unwrap();
        assert_eq!(budget.storage(), PREFIX);
    }

    #[test]
    fn actual_source_binder_and_checked_output_preserve_literal_polarity_and_failure() {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            for expected in [false, true] {
                for value in [false, true] {
                    for alias in [false, true] {
                        let function = literal_assertion(value, expected, alias);
                        let source = assertion_materialized(function.clone());
                        with_actual(&source, profile, |bound, checked, budget| {
                            assert_eq!(bound.module().kernels, checked.owner().module().kernels);
                            assert_ne!(
                                bound.canonical().canonical_bytes(),
                                checked.owner().canonical().canonical_bytes()
                            );
                            with_checked_output_assertions_budget_v1(
                                &source,
                                bound,
                                checked,
                                profile,
                                budget,
                                |session| {
                                    let mut facts = session.for_source(ROOT, ROOT);
                                    assert!(facts.is_materialized_block(0)?);
                                    assert_eq!(
                                        facts.condition(
                                            0,
                                            expected,
                                            SemanticBlockIdV1::from_index(1)
                                        )?,
                                        ProjectedAssertionConditionV1::Bool(value)
                                    );
                                    let result = projected_cfg_terminator(
                                        &function,
                                        0,
                                        &[],
                                        true,
                                        &mut facts,
                                        &[],
                                        &[],
                                    );
                                    if value == expected {
                                        assert_eq!(result?, ProjectedCfgTerminatorV1::Branch(1));
                                    } else {
                                        assert!(matches!(
                                            result,
                                            Err(
                                                ProductionRankedProjectionErrorV1::UnprovenAssert {
                                                    block: 0,
                                                    kind: "division-by-zero",
                                                    ..
                                                }
                                            )
                                        ));
                                    }
                                    Ok(())
                                },
                            )
                            .unwrap();
                        });
                    }
                }
            }
        }
    }

    #[test]
    fn actual_source_noop_keeps_the_exact_bound_output_subject() {
        let source = assertion_materialized(assertion_root_with_access(
            vec![(A_UNIT, SemanticLocalRoleV1::Return)],
            vec![],
            vec![block(201, vec![], SemanticTerminatorKindV1::Return)],
            false,
        ));
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_actual(&source, profile, |bound, checked, budget| {
                assert_eq!(bound.module(), checked.owner().module());
                with_checked_output_assertions_budget_v1(
                    &source,
                    bound,
                    checked,
                    profile,
                    budget,
                    |session| {
                        assert!(session.for_source(ROOT, ROOT).is_materialized_block(0)?);
                        Ok(())
                    },
                )
                .unwrap();
            });
        }
    }

    #[test]
    fn output_dynamic_condition_is_not_relabelled_as_a_source_proof() {
        let function = dynamic_assertion();
        let source = assertion_materialized(function.clone());
        with_actual(&source, Profile::Gfx942, |bound, checked, budget| {
            with_checked_output_assertions_budget_v1(
                &source,
                bound,
                checked,
                Profile::Gfx942,
                budget,
                |session| {
                    let mut facts = session.for_source(ROOT, ROOT);
                    assert_eq!(
                        facts.condition(0, true, SemanticBlockIdV1::from_index(1))?,
                        ProjectedAssertionConditionV1::Dynamic
                    );
                    assert!(matches!(
                        projected_cfg_terminator(&function, 0, &[], false, &mut facts, &[], &[]),
                        Err(ProductionRankedProjectionErrorV1::UnprovenAssert { block: 0, .. })
                    ));
                    Ok(())
                },
            )
            .unwrap();
        });
    }

    #[test]
    fn wrapped_source_body_and_root_aliases_remain_exact_after_optimization() {
        let source = wrapped_literal_assertion();
        with_actual(&source, Profile::Gfx942, |bound, checked, budget| {
            with_checked_output_assertions_budget_v1(
                &source,
                bound,
                checked,
                Profile::Gfx942,
                budget,
                |session| {
                    let body = SemanticFunctionIdV1::from_index(1);
                    assert_eq!(
                        session.for_source(ROOT, body).condition(
                            0,
                            true,
                            SemanticBlockIdV1::from_index(1)
                        )?,
                        ProjectedAssertionConditionV1::Bool(true)
                    );
                    for (owner, function) in [(ROOT, ROOT), (body, body)] {
                        assert!(
                            session
                                .for_source(owner, function)
                                .condition(0, true, SemanticBlockIdV1::from_index(1))
                                .is_err()
                        );
                    }
                    Ok(())
                },
            )
            .unwrap();
        });
    }

    #[test]
    fn actual_optimizer_omits_unreachable_source_success_without_erasing_original_materialization()
    {
        let source = assertion_materialized(literal_assertion(false, true, false));
        with_actual(&source, Profile::Gfx942, |bound, checked, budget| {
            let floor = budget.storage();
            let (coordinates, receipt) =
                dialect_amdgcn::check_production_target_coordinate_preservation_v1(
                    source.executable(),
                    bound,
                    Profile::Gfx942,
                    budget,
                )
                .unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            let (view, retained) =
                derive_source_output_occurrences_v1(&source, &coordinates, checked, budget)
                    .unwrap();
            budget.reserve_storage(retained.retained_storage()).unwrap();
            assert!(std::ptr::eq(view.source(), &source));
            assert!(std::ptr::eq(view.bound(), bound));
            assert!(std::ptr::eq(view.output(), checked.owner()));
            assert!(!view.grants_authority());
            assert!(matches!(
                view.block(ROOT, ROOT, SemanticBlockIdV1::from_index(1), budget)
                    .unwrap(),
                ProductionSourceOutputBlockV1::Materialized {
                    placement: None,
                    executable: false,
                    ..
                }
            ));
            drop(view);
            budget.release_storage(retained.retained_storage()).unwrap();
            drop(coordinates);
            budget.release_storage(receipt.retained_storage()).unwrap();
            assert_eq!(budget.storage(), floor);
        });
    }

    #[test]
    fn equal_canonical_bound_owner_is_accepted_but_foreign_source_pointer_is_not() {
        let source = assertion_materialized(literal_assertion(true, true, false));
        let foreign = assertion_materialized(literal_assertion(true, true, false));
        assert_eq!(
            source.executable().canonical().canonical_bytes(),
            foreign.executable().canonical().canonical_bytes()
        );
        with_actual(&source, Profile::Gfx942, |bound, checked, budget| {
            let (equal, receipt) =
                Owner::from_module_ref_with_verification_budget_v12(bound.module(), budget)
                    .unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            assert!(!std::ptr::eq(&equal, bound));
            with_checked_output_assertions_budget_v1(
                &source,
                &equal,
                checked,
                Profile::Gfx942,
                budget,
                |session| {
                    assert_eq!(
                        session.for_source(ROOT, ROOT).condition(
                            0,
                            true,
                            SemanticBlockIdV1::from_index(1)
                        )?,
                        ProjectedAssertionConditionV1::Bool(true)
                    );
                    Ok(())
                },
            )
            .unwrap();
            let (coordinates, storage) =
                dialect_amdgcn::check_production_target_coordinate_preservation_v1(
                    source.executable(),
                    bound,
                    Profile::Gfx942,
                    budget,
                )
                .unwrap();
            budget.reserve_storage(storage.retained_storage()).unwrap();
            assert!(matches!(
                derive_source_output_occurrences_v1(&foreign, &coordinates, checked, budget),
                Err(ProductionSourceOutputErrorV1::InputCustody)
            ));
            drop(coordinates);
            budget.release_storage(storage.retained_storage()).unwrap();
            drop(equal);
            budget.release_storage(receipt.retained_storage()).unwrap();
        });
    }

    #[test]
    fn wrong_target_or_complete_historical_bound_subject_has_no_source_fallback() {
        let source = assertion_materialized(literal_assertion(true, true, false));
        let other_source = assertion_materialized(literal_assertion(false, true, false));
        with_actual(&source, Profile::Gfx942, |bound, checked, budget| {
            assert!(matches!(
                with_checked_output_assertions_budget_v1(
                    &source,
                    bound,
                    checked,
                    Profile::Gfx950,
                    budget,
                    |_| Ok(())
                ),
                Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                    CanonicalAssertionErrorV1::Target(_)
                ))
            ));
            let other_bound = dialect_amdgcn::bind_production_target_v1(
                other_source.executable().module(),
                Profile::Gfx942,
            )
            .unwrap();
            let (other, receipt) =
                Owner::from_module_ref_with_verification_budget_v12(other_bound.module(), budget)
                    .unwrap();
            let source_bytes = other_source.executable_storage().retained_storage()
                + other_source.assert_origin_storage().payload_storage();
            budget.reserve_storage(source_bytes).unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            assert!(matches!(
                with_checked_output_assertions_budget_v1(
                    &other_source,
                    &other,
                    checked,
                    Profile::Gfx942,
                    budget,
                    |_| Ok(())
                ),
                Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                    CanonicalAssertionErrorV1::Output(ProductionSourceOutputErrorV1::InputCustody)
                ))
            ));
            drop(other);
            budget.release_storage(receipt.retained_storage()).unwrap();
            budget.release_storage(source_bytes).unwrap();
        });
    }

    #[test]
    fn output_private_memory_invalid_source_occurrences_do_not_gain_a_fallback() {
        let source = assertion_materialized(literal_assertion(true, true, false));
        with_actual(&source, Profile::Gfx942, |bound, checked, budget| {
            with_checked_output_assertions_budget_v1(
                &source,
                bound,
                checked,
                Profile::Gfx942,
                budget,
                |session| {
                    let mut facts = session.for_source(ROOT, ROOT);
                    let site = fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1::Statement {
                        block: fe2o3_mir_model::SsaBlockIdV1::new(0),
                        statement: 1,
                    };
                    let role = fe2o3_pliron::ProductionSemanticSsaOperandRoleV1::Destination;
                    assert!(facts.private_array_access(site, role).unwrap());
                    assert_eq!(facts.private_array_constant_index(site, role).unwrap(), Some(0));
                    let site = fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1::Statement {
                        block: fe2o3_mir_model::SsaBlockIdV1::new(0),
                        statement: u32::MAX,
                    };
                    let access = facts.private_array_access(site, role);
                    assert!(matches!(
                        &access,
                        Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(CanonicalAssertionErrorV1::Output(
    fe2o3_lower_mir_kernel::ProductionSourceOutputErrorV1::PrivateArray(
        fe2o3_lower_mir_kernel::SemanticKirPrivateArrayQueryErrorV1::InvalidSource("private array source coordinate is absent")))))
                    ), "unexpected private-array access result: {access:?}");
                    let index = facts.private_array_constant_index(site, role);
                    assert!(matches!(
                        &index,
                        Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(CanonicalAssertionErrorV1::Output(
    fe2o3_lower_mir_kernel::ProductionSourceOutputErrorV1::PrivateArray(
        fe2o3_lower_mir_kernel::SemanticKirPrivateArrayQueryErrorV1::InvalidSource("private array source coordinate is absent")))))
                    ), "unexpected private-array index result: {index:?}");
                    Ok(())
                },
            )
            .unwrap();
        });
    }

    #[test]
    fn output_session_restores_live_storage_on_refusal_and_callback_unwind() {
        let source = assertion_materialized(literal_assertion(true, true, false));
        with_actual(&source, Profile::Gfx942, |bound, checked, budget| {
            let floor = budget.storage();
            let work_before_panic = budget.work();
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = with_checked_output_assertions_budget_v1(
                    &source,
                    bound,
                    checked,
                    Profile::Gfx942,
                    budget,
                    |_| -> Result<(), ProductionRankedProjectionErrorV1> {
                        panic!("output-session-test");
                    },
                );
            }));
            assert!(panic.is_err());
            assert_eq!(budget.storage(), floor);
            let work_after_panic = budget.work();
            let peak_after_panic = budget.peak_storage();
            assert!(work_after_panic > work_before_panic);
            let refusal = with_checked_output_assertions_budget_v1(
                &source,
                bound,
                checked,
                Profile::Gfx942,
                budget,
                |_| -> Result<(), ProductionRankedProjectionErrorV1> {
                    Err(ProductionRankedProjectionErrorV1::Incomplete(
                        "output-session-test-refusal",
                    ))
                },
            );
            assert!(matches!(
                refusal,
                Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "output-session-test-refusal"
                ))
            ));
            assert_eq!(budget.storage(), floor);
            let work_after_refusal = budget.work();
            let peak_after_refusal = budget.peak_storage();
            assert!(work_after_refusal > work_after_panic);
            assert!(peak_after_refusal >= peak_after_panic);
            with_checked_output_assertions_budget_v1(
                &source,
                bound,
                checked,
                Profile::Gfx942,
                budget,
                |_| Ok(()),
            )
            .unwrap();
            assert_eq!(budget.storage(), floor);
            assert!(budget.work() > work_after_refusal);
            assert!(budget.peak_storage() >= peak_after_refusal);
            let mut short_work = Work::new(0);
            let mut short = Budget::new(&mut short_work, STORAGE_LIMIT);
            short.reserve_storage(floor).unwrap();
            assert!(
                with_checked_output_assertions_budget_v1(
                    &source,
                    bound,
                    checked,
                    Profile::Gfx942,
                    &mut short,
                    |_| Ok(())
                )
                .is_err()
            );
            assert_eq!(short.storage(), floor);
            assert_eq!(short.work(), 0);
            assert_eq!(short_work.failed_work(), Some(1));
        });
    }

    #[test]
    fn repeated_actual_occurrence_queries_share_a_literal_four_unit_component_boundary() {
        let source = assertion_materialized(literal_assertion(true, true, false));
        with_actual(&source, Profile::Gfx942, |bound, checked, budget| {
            let (coordinates, coordinates_storage) =
                dialect_amdgcn::check_production_target_coordinate_preservation_v1(
                    source.executable(),
                    bound,
                    Profile::Gfx942,
                    budget,
                )
                .unwrap();
            budget
                .reserve_storage(coordinates_storage.retained_storage())
                .unwrap();
            let (view, view_storage) =
                derive_source_output_occurrences_v1(&source, &coordinates, checked, budget)
                    .unwrap();
            budget
                .reserve_storage(view_storage.retained_storage())
                .unwrap();
            let floor = budget.storage();
            // Independently bounded query component: one alias binary visit and
            // one binding get per singleton query. This is not constructor work.
            for limit in [4, 3] {
                let mut work = Work::new(7 + limit);
                {
                    let mut query = Budget::new(&mut work, floor);
                    query.charge_work(7).unwrap();
                    query.reserve_storage(floor).unwrap();
                    view.assertion(ROOT, ROOT, SemanticBlockIdV1::from_index(0), &mut query)
                        .unwrap();
                    assert_eq!(query.work(), 9);
                    let second =
                        view.assertion(ROOT, ROOT, SemanticBlockIdV1::from_index(0), &mut query);
                    if limit == 4 {
                        assert!(second.is_ok());
                        assert_eq!(query.work(), 11);
                    } else {
                        assert!(matches!(second, Err(ProductionSourceOutputErrorV1::Assertion(
                            fe2o3_lower_mir_kernel::SemanticKirOptimizedAssertOriginErrorV1::Resource(
                                fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Work(_)
                            )
                        ))));
                        assert_eq!(query.work(), 10);
                    }
                    assert_eq!(query.storage(), floor);
                }
                assert_eq!(work.failed_work(), (limit == 3).then_some(11));
            }
            drop(view);
            budget
                .release_storage(view_storage.retained_storage())
                .unwrap();
            drop(coordinates);
            budget
                .release_storage(coordinates_storage.retained_storage())
                .unwrap();
        });
    }
    mod private_array_output_tests {
        include!("checked_output_private_array_v1_tests.rs");
    }
}
