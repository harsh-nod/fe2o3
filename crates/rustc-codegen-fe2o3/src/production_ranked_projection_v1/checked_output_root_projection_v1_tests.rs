// Included below the existing genuine checked-output session fixtures.
mod checked_output_root_projection_tests {
    use super::*;

    include!("checked_output_private_ranked_hook_v1_tests.rs");

    fn project_output<T>(
        source: &ProductionPreRankedKirOwnerV1,
        bound: &Owner,
        checked: &CheckedNeutralKernelIrOwnerV1,
        profile: Profile,
        budget: &mut Budget<'_>,
        body: impl FnOnce(
            &[ProductionRankedRootProgramV1],
            &fe2o3_lower_mir_kernel::ProductionSourceOutputOccurrencesV1<'_, '_>,
            &mut Budget<'_>,
        ) -> Result<T, ProductionRankedProjectionErrorV1>,
    ) -> Result<(Box<[ProductionRankedRootProgramV1]>, T), ProductionRankedProjectionErrorV1> {
        with_projected_checked_output_roots_v1(
            source,
            bound,
            checked,
            profile,
            &[ranked_root_input_1d(A_NAME, 247, 64)],
            &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
            budget,
            body,
        )
    }

    #[test]
    fn shared_root_projector_uses_actual_checked_output_and_mandatory_reports() {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            for expected in [false, true] {
                let source = assertion_materialized(literal_assertion(expected, expected, true));
                with_actual(&source, profile, |bound, checked, budget| {
                    assert_ne!(
                        bound.canonical().canonical_bytes(),
                        checked.owner().canonical().canonical_bytes()
                    );
                    let before = budget.work();
                    let (roots, observed_roots) = project_output(
                        &source,
                        bound,
                        checked,
                        profile,
                        budget,
                        |roots, view, budget| {
                            assert!(std::ptr::eq(view.source(), &source));
                            assert!(std::ptr::eq(view.bound(), bound));
                            assert!(std::ptr::eq(view.output(), checked.owner()));
                            assert!(!view.grants_authority());
                            let [root] = roots else {
                                panic!("the actual source has exactly one selected root");
                            };
                            assert_eq!(root.semantic_root(), ROOT);
                            assert_eq!(root.function_name(), A_NAME);
                            assert!(root.bounds_are_clean());
                            assert!(root.all_kernel_checks_are_clean());
                            assert_eq!(root.access_sources.len(), 1);
                            assert!(root.executable_effect_sources.is_empty());
                            let source_root = source.source_launch().roots()[0];
                            assert_eq!(*root.kernel_binding(), source_root.kernel_binding());
                            assert!(budget.work() > before);
                            Ok(roots.as_ptr())
                        },
                    )
                    .unwrap();
                    assert_eq!(roots.as_ptr(), observed_roots);
                    assert_eq!(roots.len(), 1);
                });
            }
        }
    }

    #[test]
    fn output_root_projection_preserves_assertion_refusals_before_consumer() {
        for function in [literal_assertion(false, true, false), dynamic_assertion()] {
            let source = assertion_materialized(function);
            with_actual(&source, Profile::Gfx942, |bound, checked, budget| {
                let called = std::cell::Cell::new(false);
                let result = project_output(
                    &source,
                    bound,
                    checked,
                    Profile::Gfx942,
                    budget,
                    |_, _, _| {
                        called.set(true);
                        Ok(())
                    },
                );
                assert!(!called.get());
                assert!(matches!(
                    result,
                    Err(ProductionRankedProjectionErrorV1::UnprovenAssert {
                        block: 0,
                        kind: "division-by-zero",
                        ..
                    })
                ));
            });
        }
    }

    #[test]
    fn output_root_projection_keeps_selected_wrapper_body() {
        let source = wrapped_literal_assertion();
        with_actual(&source, Profile::Gfx942, |bound, checked, budget| {
            project_output(
                &source,
                bound,
                checked,
                Profile::Gfx942,
                budget,
                |roots, _, _| {
                    let [root] = roots else {
                        panic!("one wrapped source root");
                    };
                    assert_eq!(root.semantic_root(), ROOT);
                    assert_eq!(
                        root.semantic_u32_induction.function(),
                        SemanticFunctionIdV1::from_index(1)
                    );
                    assert!(root.all_kernel_checks_are_clean());
                    Ok(())
                },
            )
            .unwrap();
        });
    }

    #[test]
    fn output_root_projection_restores_session_storage_on_consumer_failure_and_unwind() {
        let source = assertion_materialized(literal_assertion(true, true, false));
        with_actual(&source, Profile::Gfx942, |bound, checked, budget| {
            let before = budget.work();
            let floor = budget.storage();
            let result = project_output(
                &source,
                bound,
                checked,
                Profile::Gfx942,
                budget,
                |roots, view, _| {
                    assert_eq!(roots.len(), 1);
                    assert!(std::ptr::eq(view.output(), checked.owner()));
                    Err::<(), _>(ProductionRankedProjectionErrorV1::Incomplete(
                        "test consumer has not established final O correlation",
                    ))
                },
            );
            assert!(matches!(
                result,
                Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "test consumer has not established final O correlation"
                ))
            ));
            assert_eq!(budget.storage(), floor);
            assert!(budget.work() > before);
            let before = budget.work();
            let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                project_output(
                    &source,
                    bound,
                    checked,
                    Profile::Gfx942,
                    budget,
                    |_, _, _| -> Result<(), ProductionRankedProjectionErrorV1> {
                        panic!("checked-output consumer unwind control");
                    },
                )
            }));
            assert!(unwind.is_err());
            assert_eq!(budget.storage(), floor);
            assert!(budget.work() > before);
        });
    }
}
