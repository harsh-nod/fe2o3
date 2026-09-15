mod missing_effect_fixture_tests {
    use super::*;
    use crate::PlironEffectRefinementFindingV1;
    use crate::pliron_ir_identity::LivePlironStructuralIdentityProviderV1;
    use crate::pliron_pass_contract::begin_production_pliron_pass_contract_session_v1;
    use crate::pliron_report_validation::begin_production_analysis_report_validation_v1;

    fn missing_effect_report(
        error: &ProductionPlironPreloweringErrorV2,
    ) -> &PlironSemanticRefinementReportV1 {
        let ProductionPlironPreloweringErrorV2::Semantic(error) = error else {
            panic!("fixture must reach semantic effect refinement, got {error:?}");
        };
        let report = error.report();
        assert_eq!(report.status(), KernelCheckStatusV1::Rejected);
        assert!(report.findings().is_empty(), "{report:?}");
        assert!(report.progress().is_clean(), "{report:?}");
        let effects = report.effect_refinement();
        assert_eq!(effects.status(), KernelCheckStatusV1::Rejected);
        assert_eq!(effects.contract_count(), 0);
        assert_eq!(effects.proved_contract_count(), 0);
        assert!(matches!(
            effects.findings(),
            [PlironEffectRefinementFindingV1::UnmodeledWriteSite { .. }]
        ));
        assert!(!effects.all_declared_effects_are_proved());
        assert!(!report.grants_compiler_refinement_authority());
        assert!(!report.grants_artifact_or_launch_authority());
        report
    }

    pub(super) fn assert_exact_stage_evidence(
        source: &str,
        passes: &[(KernelCheckPassKindV1, ProductionAnalysisWitnessCheckerV1)],
    ) -> PlironSemanticRefinementReportV1 {
        let (context, function) = parse_fixture(source);
        let target = atomic_target();
        let error = require_production_pliron_checks_with_atomic_target_before_lowering_v2(
            &context, &function, &target,
        )
        .expect_err("stage-local positives do not model the global write's reference effects");
        let rejected = missing_effect_report(&error);

        // Collect diagnostic stage evidence from the unchanged live graph,
        // including the real rejected semantic report. Never create a clean
        // production report or a full W4 witness for this fixture.
        let provider = LivePlironStructuralIdentityProviderV1::new(&context, &function);
        let mut preservation = begin_production_pliron_pass_contract_session_v1(provider).unwrap();
        let mut validation = begin_production_analysis_report_validation_v1(
            &context,
            &function,
            Some(&target),
            preservation.validation_handle(),
        );
        macro_rules! record {
            ($pass:ident, $report:expr, $status:ident) => {{
                let report = preservation
                    .run_contiguous_pass(KernelCheckPassKindV1::$pass, || {
                        Ok::<_, std::convert::Infallible>($report)
                    })
                    .unwrap()
                    .unwrap();
                assert_eq!(report.status(), KernelCheckStatusV1::$status, "{report:?}");
                validation
                    .record(
                        &context,
                        &function,
                        preservation.last_checkpoint().unwrap(),
                        &report,
                    )
                    .unwrap();
                report
            }};
        }
        record!(
            TensorLayout,
            crate::run_pliron_tensor_layout_check_v1(&context, &function),
            Clean
        );
        record!(
            MemoryBounds,
            crate::run_pliron_ranked_bounds_check_v1(&context, &function),
            Clean
        );
        record!(
            AtomicLegality,
            crate::run_pliron_atomic_legality_check_with_target_v1(&context, &function, &target),
            Clean
        );
        record!(
            RaceFreedom,
            crate::run_pliron_ranked_race_check_v1(&context, &function),
            Clean
        );
        record!(
            HierarchicalOwnership,
            crate::run_pliron_hierarchical_ownership_check_v1(&context, &function),
            Clean
        );
        record!(
            BarrierConvergence,
            crate::run_pliron_barrier_convergence_check_v1(&context, &function),
            Clean
        );
        record!(
            PipelineProtocol,
            crate::run_pliron_pipeline_protocol_check_v1(&context, &function),
            Clean
        );
        record!(
            WorkgroupMemory,
            crate::run_pliron_workgroup_memory_check_v1(&context, &function),
            Clean
        );
        let semantics = record!(
            SemanticRefinement,
            crate::run_pliron_semantic_refinement_check_v1(&context, &function),
            Rejected
        );
        assert_eq!(&semantics, rejected);
        let preservation = preservation.finish().unwrap();
        let validation = validation.finish_validation(&preservation).unwrap();
        assert!(preservation.is_exact_identity());
        let epoch = preservation.input_mutation_epoch();
        assert_eq!(preservation.output_mutation_epoch(), epoch);
        assert_eq!(validation.status(), KernelCheckStatusV1::Rejected);
        assert!(!validation.all_reports_independently_validated());
        assert_eq!(
            validation.stages().len(),
            PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2.len()
        );
        for &(pass, checker) in passes {
            let stage = validation
                .stages()
                .iter()
                .find(|stage| stage.checkpoint().pass() == pass)
                .unwrap_or_else(|| panic!("missing exact independent evidence for {pass:?}"));
            assert_eq!(stage.analysis_status(), KernelCheckStatusV1::Clean);
            assert_eq!(
                stage.independent_validation_status(),
                KernelCheckStatusV1::Clean
            );
            assert_eq!(stage.checkpoint().mutation_epoch(), epoch);
            assert_eq!(
                stage.checkpoint().identity_label(),
                preservation.input_identity()
            );
            assert_eq!(stage.witness().checker(), checker);
            assert!(stage.witness().coverage().is_complete());
            assert_ne!(stage.witness().coverage().obligation_count(), 0);
            assert!(!stage.witness().grants_compiler_refinement_authority());
            assert!(!stage.witness().grants_lowering_or_launch_authority());
        }
        let semantic_stage = validation.stages().last().unwrap();
        assert_eq!(
            semantic_stage.checkpoint().pass(),
            KernelCheckPassKindV1::SemanticRefinement
        );
        assert_eq!(
            semantic_stage.analysis_status(),
            KernelCheckStatusV1::Rejected
        );
        assert_eq!(
            semantic_stage.independent_validation_status(),
            KernelCheckStatusV1::Rejected
        );
        semantics
    }

    pub(super) fn assert_full_w4_rejection(name: &str, source: &str) {
        let ProductionW4FinalGraphExecutionV1::NonClean(result) =
            full_fixture_execution(name, source)
        else {
            panic!("{name} must not receive full W4 admission without effect contracts");
        };
        let ProductionW4CapabilityOutcomeV1::Rejected(diagnostic) = result.outcome() else {
            panic!("{name} must reject its unmodeled global write: {result}");
        };
        assert_eq!(
            diagnostic.stage(),
            ProductionCapabilityAnalysisKindV1::EffectRefinement
        );
        assert_eq!(diagnostic.function(), Some(&FunctionId::new(name)));
        missing_effect_report(
            result
                .pipeline_error()
                .expect("retained typed pipeline error"),
        );
        result.require_exact_subject_v1(result.subject()).unwrap();
        assert!(!result.grants_any_authority());
    }
}
