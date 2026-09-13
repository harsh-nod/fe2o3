#[cfg(test)]
mod resource_upper_bound_tests {
    use super::*;

    fn memory_bounds_census() -> ProductionAnalysisInputCensusV1 {
        ProductionAnalysisInputCensusV1 {
            operations: 3,
            ..ProductionAnalysisInputCensusV1::default()
        }
    }

    #[test]
    fn bounds_witness_charges_rank_and_map_work_at_exact_limits() {
        let census = memory_bounds_census();
        let unlimited = ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX);
        let bound = preflight_production_analysis_witness_resource_upper_bound_v1(
            KernelCheckPassKindV1::MemoryBounds,
            census,
            unlimited,
        )
        .unwrap();

        let attempts = MAX_BOUNDS_WITNESS_EVALUATION_STEPS_V1 + 1;
        let obligations = census.operations * MAX_RANKED_MEMORY_RANK;
        let witness_storage = obligations * (MAX_RANKED_MEMORY_RANK * 3 + 16) + 1;
        let evaluation_stack_storage =
            (census.operations * 2 + 1) * RAW_INDEX_STACK_STORAGE_ITEMS_PER_FRAME_V1;
        let one_replay_work = census.operations * 2
            + attempts
            + attempts * (MAX_RANKED_MEMORY_RANK + 2)
            + attempts * RAW_INDEX_STACK_WORK_PER_EVALUATION_ATTEMPT_V1
            + obligations * (MAX_RANKED_MEMORY_RANK * 4 + 16)
            + MAX_RANKED_MEMORY_RANK * 8
            + 32;
        let expected_work = one_replay_work * 2
            + witness_storage
            + MAX_PRODUCTION_ANALYSIS_WITNESS_REASON_BYTES_V1 * 2;
        let expected_retained =
            witness_storage.max(MAX_PRODUCTION_ANALYSIS_WITNESS_REASON_BYTES_V1 * 2);
        let one_replay_temporary =
            census.operations * 6 + evaluation_stack_storage + MAX_RANKED_MEMORY_RANK * 8 + 32;
        let expected_peak = expected_retained
            + witness_storage
            + one_replay_temporary
            + MAX_PRODUCTION_ANALYSIS_WITNESS_REASON_BYTES_V1 * 2;
        assert_eq!(bound.work_upper_bound(), expected_work);
        assert_eq!(bound.retained_storage_upper_bound(), expected_retained);
        assert_eq!(bound.peak_storage_upper_bound(), expected_peak);

        assert_eq!(
            preflight_production_analysis_witness_resource_upper_bound_v1(
                KernelCheckPassKindV1::MemoryBounds,
                census,
                ProductionAnalysisResourceLimitsV1::new(expected_work, expected_peak),
            ),
            Ok(bound)
        );
        assert_eq!(
            preflight_production_analysis_witness_resource_upper_bound_v1(
                KernelCheckPassKindV1::MemoryBounds,
                census,
                ProductionAnalysisResourceLimitsV1::new(expected_work - 1, expected_peak),
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::ReportValidation,
                resource: "work upper bound",
            })
        );
        assert_eq!(
            preflight_production_analysis_witness_resource_upper_bound_v1(
                KernelCheckPassKindV1::MemoryBounds,
                census,
                ProductionAnalysisResourceLimitsV1::new(expected_work, expected_peak - 1),
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::ReportValidation,
                resource: "peak storage upper bound",
            })
        );
    }

    #[test]
    fn hostile_witness_census_overflow_rejects_before_admission() {
        let census = ProductionAnalysisInputCensusV1 {
            operations: usize::MAX,
            ..ProductionAnalysisInputCensusV1::default()
        };
        assert_eq!(
            preflight_production_analysis_witness_resource_upper_bound_v1(
                KernelCheckPassKindV1::MemoryBounds,
                census,
                ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
            ),
            Err(witness_resource_error_v1())
        );
    }
}

#[cfg(test)]
mod tests {
    use dialect_kernel::{DIALECT_NAME, register_dialect};
    use fe2o3_pliron_owner_core::{ensure_context_identity, require_context_identity};
    use pliron::{
        builtin::ops::FuncOp, context::Context, dialect::DialectName, op::Op, operation::Operation,
        parsable::parse_from_str,
    };

    use super::*;
    use crate::{KernelCheckPassKindV1, require_production_pliron_checks_before_lowering_v2};

    const SAFE_AFFINE: &str = r#"
builtin.func @bounds_witness_safe: builtin.function <() -> ()>
{
  ^entry_block1v1():
    v0 = kernel.ranked_view () [] [kernel_memory_space: kernel.memory_space Global]: <() -> (kernel.ranked_view <32,false,[16]>)>;
    v1 = kernel.invocation_index () [] [kernel_invocation_dimension: kernel.invocation_dimension 0, kernel_launch_extent: kernel.launch_extent 8]: <() -> (kernel.index )>;
    v2 = kernel.index_constant () [] [kernel_index_value: kernel.index_value 2]: <() -> (kernel.index )>;
    v3 = kernel.index_constant () [] [kernel_index_value: kernel.index_value 1]: <() -> (kernel.index )>;
    v4 = kernel.index_binary (v1, v2) [] [kernel_index_binary_kind: kernel.index_binary_kind Multiply]: <(kernel.index , kernel.index ) -> (kernel.index )>;
    v5 = kernel.index_binary (v4, v3) [] [kernel_index_binary_kind: kernel.index_binary_kind Add]: <(kernel.index , kernel.index ) -> (kernel.index )>;
    kernel.access (v0, v5) [] [kernel_access_kind: kernel.access_kind Read]: <(kernel.ranked_view <32,false,[16]>, kernel.index ) -> ()>;
    kernel.return () [] []: <() -> ()>
}
"#;

    fn setup() -> Context {
        let mut context = Context::new();
        register_dialect(
            &mut context,
            &DialectName::try_new(DIALECT_NAME).expect("valid dialect"),
        )
        .expect("kernel dialect");
        dialect_gpu::register_dialect(&mut context).expect("gpu dialect");
        dialect_proof::register_dialect(&mut context).expect("proof dialect");
        ensure_context_identity(&mut context).expect("context identity");
        context
    }

    fn parse_function(context: &mut Context) -> FuncOp {
        parse_source(context, SAFE_AFFINE)
    }

    fn parse_source(context: &mut Context, source: &str) -> FuncOp {
        let operation = parse_from_str(Operation::top_level_parser(), context, source)
            .expect("parse witness function");
        FuncOp::from_operation(operation)
    }

    fn bounds_envelope(
        report: &crate::ProductionPlironPreloweringReportV2,
    ) -> (
        &crate::ProductionAnalysisStageValidationV1,
        ProductionAnalysisWitnessEnvelopeV1,
    ) {
        let stage = &report.report_validation().stages()[1];
        (stage, stage.witness().clone())
    }

    fn replay_with_clean_bounds_report(
        context: &mut Context,
        source: &str,
    ) -> Result<
        SupportedWitnessBuildV1<BoundsPresburgerWitnessV1>,
        ProductionAnalysisWitnessValidationErrorV1,
    > {
        // The clean report is intentionally from a distinct valid subject.
        // These tests exercise the independent witness replay boundary, not
        // the primary bounds verifier that issued that report.
        let safe = parse_function(context);
        let safe_report = require_production_pliron_checks_before_lowering_v2(context, &safe)
            .expect("safe bounds report");
        let candidate = parse_source(context, source);
        let mut analyses = PlironAnalysisManagerV1::new(&candidate);
        build_bounds_presburger_witness(context, &candidate, safe_report.bounds(), &mut analyses)
    }

    fn expect_incomplete_bounds_replay(
        replay: Result<
            SupportedWitnessBuildV1<BoundsPresburgerWitnessV1>,
            ProductionAnalysisWitnessValidationErrorV1,
        >,
        expected_reason: &str,
    ) {
        match replay {
            Ok(SupportedWitnessBuildV1::Incomplete(reason)) => assert!(
                reason.contains(expected_reason),
                "incomplete reason {reason:?} did not contain {expected_reason:?}"
            ),
            Ok(SupportedWitnessBuildV1::Complete(_)) => {
                panic!("hostile bounds replay must never be Complete")
            }
            Err(error) => panic!("expected Incomplete bounds replay, got rejection: {error}"),
        }
    }

    fn affine_source_with_execution_layout(name: &str, global: [u64; 3]) -> String {
        let [global_x, global_y, global_z] = global;
        let layout = format!(
            "    gpu.execution_layout () [] [gpu_execution_grid_identity: gpu.grid_identity 7, gpu_execution_global_x: gpu.execution_extent {global_x}, gpu_execution_global_y: gpu.execution_extent {global_y}, gpu_execution_global_z: gpu.execution_extent {global_z}, gpu_execution_workgroup_x: gpu.execution_extent {global_x}, gpu_execution_workgroup_y: gpu.execution_extent {global_y}, gpu_execution_workgroup_z: gpu.execution_extent {global_z}, gpu_execution_subgroup_size: gpu.subgroup_size 4, gpu_execution_domain: gpu.execution_domain FullPhysicalWorkgroups]: <() -> ()>;"
        );
        SAFE_AFFINE
            .replace("@bounds_witness_safe", &format!("@{name}"))
            .replace(
                "  ^entry_block1v1():",
                &format!("  ^entry_block1v1():\n{layout}"),
            )
    }

    #[test]
    fn affine_bounds_witness_replays_every_access_dimension() {
        let context = &mut setup();
        let function = parse_function(context);
        let report = require_production_pliron_checks_before_lowering_v2(context, &function)
            .expect("safe affine function");
        let (stage, envelope) = bounds_envelope(&report);

        assert_eq!(
            stage.checkpoint().pass(),
            KernelCheckPassKindV1::MemoryBounds
        );
        assert_eq!(
            envelope.checker(),
            ProductionAnalysisWitnessCheckerV1::BoundsExhaustiveRawIrReplayV1
        );
        assert_eq!(envelope.coverage().obligation_count(), 1);
        assert!(envelope.coverage().is_complete());
        assert_eq!(
            stage.independent_validation_status(),
            KernelCheckStatusV1::Clean
        );
        assert!(!envelope.grants_compiler_refinement_authority());
        assert!(!envelope.grants_lowering_or_launch_authority());
    }

    #[test]
    fn subject_binding_report_and_obligation_mutations_fail_closed() {
        let context = &mut setup();
        let function = parse_function(context);
        let report = require_production_pliron_checks_before_lowering_v2(context, &function)
            .expect("safe affine function");
        let (stage, envelope) = bounds_envelope(&report);
        let captured = CapturedProductionAnalysisReportV1::Bounds(report.bounds().clone());
        let foreign_context = &mut setup();
        let foreign_identity = require_context_identity(foreign_context).expect("foreign identity");

        let validate = |candidate: &ProductionAnalysisWitnessEnvelopeV1| {
            let mut analyses = PlironAnalysisManagerV1::new(&function);
            validate_production_analysis_witness_v1(
                context,
                &function,
                ExpectedProductionAnalysisWitnessV1 {
                    checkpoint: stage.checkpoint(),
                    implementation: stage.implementation(),
                    configuration: stage.configuration(),
                    report: &captured,
                },
                candidate,
                &mut analyses,
            )
        };
        validate(&envelope).expect("unaltered envelope");

        let mut forged_subject = envelope.clone();
        forged_subject.context_identity = Some(foreign_identity);
        assert!(matches!(
            validate(&forged_subject),
            Err(ProductionAnalysisWitnessValidationErrorV1::SubjectMismatch)
        ));

        let mut substituted_report = envelope.clone();
        substituted_report.report =
            CapturedProductionAnalysisReportV1::TensorLayout(report.tensor_layout().clone());
        assert!(matches!(
            validate(&substituted_report),
            Err(ProductionAnalysisWitnessValidationErrorV1::ReportMismatch)
        ));

        let mut omitted = envelope.clone();
        let ProductionAnalysisWitnessPayloadV1::Bounds(witness) = &mut omitted.payload else {
            panic!("bounds witness payload");
        };
        witness.obligations.clear();
        assert!(matches!(
            validate(&omitted),
            Err(ProductionAnalysisWitnessValidationErrorV1::PayloadMismatch)
        ));

        let mut forged_extent = envelope.clone();
        let ProductionAnalysisWitnessPayloadV1::Bounds(witness) = &mut forged_extent.payload else {
            panic!("bounds witness payload");
        };
        witness.obligations[0].extent += 1;
        assert!(matches!(
            validate(&forged_extent),
            Err(ProductionAnalysisWitnessValidationErrorV1::PayloadMismatch)
        ));

        let mut forged_transcript = envelope.clone();
        let ProductionAnalysisWitnessPayloadV1::Bounds(witness) = &mut forged_transcript.payload
        else {
            panic!("bounds witness payload");
        };
        witness.obligations[0].checked_invocations += 1;
        assert!(matches!(
            validate(&forged_transcript),
            Err(ProductionAnalysisWitnessValidationErrorV1::PayloadMismatch)
        ));
    }

    #[test]
    fn exhaustive_resource_cap_and_dynamic_launch_remain_incomplete() {
        let context = &mut setup();
        let capped_source = SAFE_AFFINE
            .replace("@bounds_witness_safe", "@bounds_witness_capped")
            .replace("kernel.launch_extent 8", "kernel.launch_extent 65537")
            .replace("kernel.index_value 2", "kernel.index_value 16")
            .replace("kernel.index_value 1", "kernel.index_value 0")
            .replace(
                "kernel.index_binary_kind Multiply",
                "kernel.index_binary_kind Remainder",
            )
            .replace("kernel.access (v0, v5)", "kernel.access (v0, v4)");
        let capped = parse_source(context, &capped_source);
        let capped_report = require_production_pliron_checks_before_lowering_v2(context, &capped)
            .expect("analysis accepts bounded remainder");
        let capped_coverage = capped_report.report_validation().stages()[1]
            .witness()
            .coverage();
        assert_eq!(capped_coverage.status(), KernelCheckStatusV1::Incomplete);
        assert!(
            capped_coverage
                .incomplete_reason()
                .expect("resource reason")
                .contains("exhaustive replay cap")
        );

        let dynamic_source = SAFE_AFFINE
            .replace("@bounds_witness_safe", "@bounds_witness_dynamic")
            .replace("kernel.launch_extent 8", "kernel.launch_extent 0")
            .replace("kernel.access (v0, v5)", "kernel.access (v0, v3)");
        let dynamic = parse_source(context, &dynamic_source);
        let dynamic_report = require_production_pliron_checks_before_lowering_v2(context, &dynamic)
            .expect("constant access is analysis-safe with a dynamic launch");
        let dynamic_coverage = dynamic_report.report_validation().stages()[1]
            .witness()
            .coverage();
        assert_eq!(dynamic_coverage.status(), KernelCheckStatusV1::Incomplete);
        assert!(
            dynamic_coverage
                .incomplete_reason()
                .expect("dynamic reason")
                .contains("dynamic launch dimension")
        );
    }

    #[test]
    fn raw_evaluator_replays_a_concrete_counterexample_independently() {
        let context = &mut setup();
        let safe = parse_function(context);
        let safe_report = require_production_pliron_checks_before_lowering_v2(context, &safe)
            .expect("safe report");
        let out_of_bounds_source = SAFE_AFFINE
            .replace("@bounds_witness_safe", "@bounds_witness_counterexample")
            .replace("[16]", "[15]");
        let out_of_bounds = parse_source(context, &out_of_bounds_source);

        assert!(matches!(
            build_bounds_presburger_witness(
                context,
                &out_of_bounds,
                safe_report.bounds(),
                &mut PlironAnalysisManagerV1::new(&out_of_bounds),
            ),
            Err(ProductionAnalysisWitnessValidationErrorV1::BoundsCounterexample {
                invocation,
                index: 15,
                extent: 15,
                ..
            }) if invocation == vec![7]
        ));
    }

    #[test]
    fn matching_execution_layout_preserves_the_supported_fragment() {
        let context = &mut setup();
        let source = affine_source_with_execution_layout("bounds_witness_layout_match", [8, 1, 1]);
        let replay = replay_with_clean_bounds_report(context, &source)
            .expect("matching layout is replayable");
        let SupportedWitnessBuildV1::Complete(witness) = replay else {
            panic!("matching static execution layout must remain supported")
        };

        assert_eq!(witness.obligations.len(), 1);
        assert_eq!(witness.obligations[0].checked_invocations, 8);
    }

    #[test]
    fn execution_layout_must_agree_with_invocation_inventory() {
        let context = &mut setup();
        let source =
            affine_source_with_execution_layout("bounds_witness_layout_mismatch", [4, 1, 1]);

        expect_incomplete_bounds_replay(
            replay_with_clean_bounds_report(context, &source),
            "inconsistent with gpu.execution_layout",
        );
    }

    #[test]
    fn execution_layout_active_axes_require_invocation_dimensions() {
        let context = &mut setup();
        let source =
            affine_source_with_execution_layout("bounds_witness_missing_active_axis", [8, 4, 1]);

        expect_incomplete_bounds_replay(
            replay_with_clean_bounds_report(context, &source),
            "active axis 1 extent 4 without an invocation dimension",
        );
    }

    #[test]
    fn duplicate_execution_layout_records_are_incomplete() {
        let context = &mut setup();
        let source =
            affine_source_with_execution_layout("bounds_witness_duplicate_layout", [8, 1, 1]);
        let layout = source
            .lines()
            .find(|line| line.contains("gpu.execution_layout"))
            .expect("layout line");
        let source = source.replacen(layout, &format!("{layout}\n{layout}"), 1);

        expect_incomplete_bounds_replay(
            replay_with_clean_bounds_report(context, &source),
            "more than one gpu.execution_layout",
        );
    }

    #[test]
    fn malformed_execution_layout_is_incomplete() {
        let context = &mut setup();
        let source =
            affine_source_with_execution_layout("bounds_witness_malformed_layout", [8, 1, 1])
                .replace("gpu.subgroup_size 4", "gpu.subgroup_size 0");

        expect_incomplete_bounds_replay(
            replay_with_clean_bounds_report(context, &source),
            "malformed gpu.execution_layout",
        );
    }

    #[test]
    fn zero_or_dynamic_invocation_extent_cannot_vacuously_complete() {
        let context = &mut setup();
        let source = SAFE_AFFINE
            .replace("@bounds_witness_safe", "@bounds_witness_zero_invocations")
            .replace("kernel.launch_extent 8", "kernel.launch_extent 0")
            .replace("kernel.index_value 1", "kernel.index_value 99")
            .replace("kernel.access (v0, v5)", "kernel.access (v0, v3)");

        expect_incomplete_bounds_replay(
            replay_with_clean_bounds_report(context, &source),
            "cannot enumerate dynamic launch dimension 0",
        );
    }

    #[test]
    fn zero_divisors_are_incomplete_for_divide_and_remainder() {
        for (name, operator) in [
            ("bounds_witness_divide_zero", "Divide"),
            ("bounds_witness_remainder_zero", "Remainder"),
        ] {
            let context = &mut setup();
            let source = SAFE_AFFINE
                .replace("@bounds_witness_safe", &format!("@{name}"))
                .replace("kernel.index_value 2", "kernel.index_value 0")
                .replace(
                    "kernel.index_binary_kind Multiply",
                    &format!("kernel.index_binary_kind {operator}"),
                )
                .replace("kernel.access (v0, v5)", "kernel.access (v0, v4)");

            expect_incomplete_bounds_replay(
                replay_with_clean_bounds_report(context, &source),
                "divisor is zero",
            );
        }
    }

    #[test]
    fn duplicate_invocation_dimensions_cannot_self_certify() {
        let context = &mut setup();
        let source = SAFE_AFFINE
            .replace("@bounds_witness_safe", "@bounds_witness_duplicate_dimension")
            .replace(
                "    v2 = kernel.index_constant",
                "    v6 = kernel.invocation_index () [] [kernel_invocation_dimension: kernel.invocation_dimension 0, kernel_launch_extent: kernel.launch_extent 8]: <() -> (kernel.index )>;\n    v2 = kernel.index_constant",
            );

        expect_incomplete_bounds_replay(
            replay_with_clean_bounds_report(context, &source),
            "duplicate invocation dimension 0",
        );
    }

    #[test]
    fn unresolved_index_definition_is_incomplete() {
        let context = &mut setup();
        let source = r#"
builtin.func @bounds_witness_unresolved: builtin.function <(kernel.index ) -> ()>
{
  ^entry_block1v1(unresolved_v0: kernel.index ):
    v1 = kernel.ranked_view () [] [kernel_memory_space: kernel.memory_space Global]: <() -> (kernel.ranked_view <32,false,[16]>)>;
    v2 = kernel.invocation_index () [] [kernel_invocation_dimension: kernel.invocation_dimension 0, kernel_launch_extent: kernel.launch_extent 8]: <() -> (kernel.index )>;
    kernel.access (v1, unresolved_v0) [] [kernel_access_kind: kernel.access_kind Read]: <(kernel.ranked_view <32,false,[16]>, kernel.index ) -> ()>;
    kernel.return () [] []: <() -> ()>
}
"#;

        expect_incomplete_bounds_replay(
            replay_with_clean_bounds_report(context, source),
            "block arguments are outside the V1 raw-index fragment",
        );
    }

    #[test]
    fn cumulative_multi_access_multi_dimension_work_is_capped() {
        let context = &mut setup();
        let source = r#"
builtin.func @bounds_witness_cumulative_cap: builtin.function <() -> ()>
{
  ^entry_block1v1():
    v0 = kernel.ranked_view () [] [kernel_memory_space: kernel.memory_space Global]: <() -> (kernel.ranked_view <32,false,[16,16]>)>;
    v1 = kernel.invocation_index () [] [kernel_invocation_dimension: kernel.invocation_dimension 0, kernel_launch_extent: kernel.launch_extent 65536]: <() -> (kernel.index )>;
    v2 = kernel.index_constant () [] [kernel_index_value: kernel.index_value 16]: <() -> (kernel.index )>;
    v3 = kernel.index_binary (v1, v2) [] [kernel_index_binary_kind: kernel.index_binary_kind Remainder]: <(kernel.index , kernel.index ) -> (kernel.index )>;
    kernel.access (v0, v3, v3) [] [kernel_access_kind: kernel.access_kind Read]: <(kernel.ranked_view <32,false,[16,16]>, kernel.index , kernel.index ) -> ()>;
    kernel.access (v0, v3, v3) [] [kernel_access_kind: kernel.access_kind Read]: <(kernel.ranked_view <32,false,[16,16]>, kernel.index , kernel.index ) -> ()>;
    kernel.access (v0, v3, v3) [] [kernel_access_kind: kernel.access_kind Read]: <(kernel.ranked_view <32,false,[16,16]>, kernel.index , kernel.index ) -> ()>;
    kernel.return () [] []: <() -> ()>
}
"#;

        expect_incomplete_bounds_replay(
            replay_with_clean_bounds_report(context, source),
            "raw-index evaluation exceeded its deterministic work cap",
        );
    }

    #[test]
    fn iterative_raw_evaluator_handles_a_deep_acyclic_definition_dag() {
        const DEPTH: usize = 20_000;

        let context = &mut setup();
        let zero = IndexConstantOp::new(context, 0);
        let zero_value = zero.result(context);
        let mut value = zero_value;
        for _ in 0..DEPTH {
            value = IndexBinaryOp::new(context, IndexBinaryKindAttr::Add, value, zero_value)
                .result(context);
        }

        let mut evaluation_steps = 0;
        assert_eq!(
            evaluate_raw_index_at_invocation_v1(context, value, &[], &mut evaluation_steps),
            Some(0)
        );
        assert_eq!(evaluation_steps, DEPTH + 1);
    }

    #[test]
    fn iterative_raw_evaluator_rejects_a_definition_cycle() {
        let context = &mut setup();
        let zero = IndexConstantOp::new(context, 0);
        let zero_value = zero.result(context);
        let first = IndexBinaryOp::new(context, IndexBinaryKindAttr::Add, zero_value, zero_value);
        let second = IndexBinaryOp::new(
            context,
            IndexBinaryKindAttr::Add,
            first.result(context),
            zero_value,
        );
        Operation::replace_operand(first.get_operation(), context, 0, second.result(context));

        let mut evaluation_steps = 0;
        let failure = evaluate_raw_index_iterative(
            context,
            second.result(context),
            &[],
            &mut HashMap::new(),
            &mut HashSet::new(),
            &mut evaluation_steps,
            16,
        )
        .unwrap_err();
        assert!(matches!(
            failure,
            RawIndexEvaluationFailureV1::Incomplete("the raw index definition graph is cyclic")
        ));
    }
}
