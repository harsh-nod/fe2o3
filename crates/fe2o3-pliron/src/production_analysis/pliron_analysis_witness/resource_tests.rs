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

    fn constant_layout_source(
        name: &str,
        global: [u64; 3],
        coordinates: &[(usize, u64)],
        index: u64,
        accesses: usize,
    ) -> String {
        let [x, y, z] = global;
        let mut source = format!(
            r#"
builtin.func @{name}: builtin.function <() -> ()>
{{
  ^entry_block1v1():
    gpu.execution_layout () [] [gpu_execution_grid_identity: gpu.grid_identity 7, gpu_execution_global_x: gpu.execution_extent {x}, gpu_execution_global_y: gpu.execution_extent {y}, gpu_execution_global_z: gpu.execution_extent {z}, gpu_execution_workgroup_x: gpu.execution_extent 1, gpu_execution_workgroup_y: gpu.execution_extent 1, gpu_execution_workgroup_z: gpu.execution_extent 1, gpu_execution_subgroup_size: gpu.subgroup_size 1, gpu_execution_domain: gpu.execution_domain FullPhysicalWorkgroups]: <() -> ()>;
    v0 = kernel.ranked_view () [] [kernel_memory_space: kernel.memory_space Global]: <() -> (kernel.ranked_view <32,false,[16]>)>;
"#
        );
        for (ordinal, (dimension, extent)) in coordinates.iter().enumerate() {
            source.push_str(&format!("    coordinate_{ordinal} = kernel.invocation_index () [] [kernel_invocation_dimension: kernel.invocation_dimension {dimension}, kernel_launch_extent: kernel.launch_extent {extent}]: <() -> (kernel.index )>;\n"));
        }
        source.push_str(&format!("    v1 = kernel.index_constant () [] [kernel_index_value: kernel.index_value {index}]: <() -> (kernel.index )>;\n"));
        for _ in 0..accesses {
            source.push_str("    kernel.access (v0, v1) [] [kernel_access_kind: kernel.access_kind Read]: <(kernel.ranked_view <32,false,[16]>, kernel.index ) -> ()>;\n");
        }
        source.push_str("    kernel.return () [] []: <() -> ()>\n}\n");
        source
    }

    fn assert_layout_obligation(
        obligation: &BoundsPresburgerObligationV1,
        extents: [u64; 3],
        checked: u64,
    ) {
        assert_eq!(obligation.checked_invocations, checked);
        let domain = obligation.normalized_map.domain();
        assert!(domain.constraints().is_empty());
        assert_eq!(domain.domain().lower(), &[0, 0, 0]);
        assert_eq!(domain.domain().upper_exclusive(), &extents.map(i128::from));
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
    fn no_layout_constant_witness_uses_the_zero_rank_raw_domain() {
        let context = &mut setup();
        let coordinate = "v1 = kernel.invocation_index () [] [kernel_invocation_dimension: kernel.invocation_dimension 0, kernel_launch_extent: kernel.launch_extent 8]: <() -> (kernel.index )>;";
        assert_eq!(SAFE_AFFINE.matches(coordinate).count(), 1);
        let source = SAFE_AFFINE
            .replace("@bounds_witness_safe", "@bounds_witness_no_coordinates")
            .replace(coordinate, "v1 = kernel.index_constant () [] [kernel_index_value: kernel.index_value 3]: <() -> (kernel.index )>;");
        assert!(!source.contains("kernel.invocation_index"));
        assert!(!source.contains("gpu.execution_layout"));
        let function = parse_source(context, &source);
        let report = require_production_pliron_checks_before_lowering_v2(context, &function)
            .expect("real no-layout constant read passes normal policy checks");
        assert!(report.is_clean());
        let (stage, envelope) = bounds_envelope(&report);
        assert!(envelope.coverage().is_complete());
        assert_eq!(
            stage.independent_validation_status(),
            KernelCheckStatusV1::Clean
        );
        assert!(
            !report
                .report_validation()
                .all_reports_independently_validated()
        );
        assert!(!envelope.grants_lowering_or_launch_authority());
        let ProductionAnalysisWitnessPayloadV1::Bounds(witness) = envelope.payload else {
            panic!("bounds payload");
        };
        assert_eq!(witness.obligations.len(), 1);
        let obligation = &witness.obligations[0];
        assert_eq!(obligation.extent, 16);
        assert_eq!(obligation.checked_invocations, 1);
        let domain = obligation.normalized_map.domain();
        assert!(domain.constraints().is_empty());
        assert!(domain.domain().lower().is_empty());
        assert!(domain.domain().upper_exclusive().is_empty());
        assert_eq!(obligation.normalized_map.evaluate(&[]).unwrap(), vec![7]);
    }

    #[test]
    fn no_layout_nonconsecutive_coordinates_keep_singleton_holes_and_raw_axes() {
        for x_extent in [1_u64, 3] {
            let context = &mut setup();
            let mut source = SAFE_AFFINE
                .replace(
                    "@bounds_witness_safe",
                    &format!("@bounds_witness_sparse_{x_extent}"),
                )
                .replace(
                    "kernel.invocation_dimension 0",
                    "kernel.invocation_dimension 2",
                )
                .replace("kernel.launch_extent 8", "kernel.launch_extent 2");
            if x_extent != 1 {
                source = source.replace("  ^entry_block1v1():", &format!(
                    "  ^entry_block1v1():\n    unused_x = kernel.invocation_index () [] [kernel_invocation_dimension: kernel.invocation_dimension 0, kernel_launch_extent: kernel.launch_extent {x_extent}]: <() -> (kernel.index )>;"
                ));
            }
            // The existing remainder rule proves the primary bound without
            // requiring a complete sparse coordinate roster. Replay still
            // interprets the actual affine z DAG and the remainder operation.
            let access = "    kernel.access (v0, v5)";
            assert_eq!(source.matches(access).count(), 1);
            source = source.replace(access,
                "    modulus = kernel.index_constant () [] [kernel_index_value: kernel.index_value 16]: <() -> (kernel.index )>;\n    wrapped = kernel.index_binary (v5, modulus) [] [kernel_index_binary_kind: kernel.index_binary_kind Remainder]: <(kernel.index , kernel.index ) -> (kernel.index )>;\n    kernel.access (v0, wrapped)");
            assert!(!source.contains("gpu.execution_layout"));
            assert_eq!(
                source.matches("kernel.invocation_index").count(),
                if x_extent == 1 { 1 } else { 2 }
            );
            let function = parse_source(context, &source);
            let report = require_production_pliron_checks_before_lowering_v2(context, &function)
                .expect("real no-layout remainder read passes normal policy checks");
            assert!(report.is_clean());
            let (stage, envelope) = bounds_envelope(&report);
            assert!(envelope.coverage().is_complete());
            assert_eq!(
                stage.independent_validation_status(),
                KernelCheckStatusV1::Clean
            );
            assert!(
                !report
                    .report_validation()
                    .all_reports_independently_validated()
            );
            assert!(!envelope.grants_lowering_or_launch_authority());
            let ProductionAnalysisWitnessPayloadV1::Bounds(witness) = envelope.payload else {
                panic!("bounds payload");
            };
            assert_eq!(witness.obligations.len(), 1);
            let obligation = &witness.obligations[0];
            assert_eq!(obligation.extent, 16);
            assert_layout_obligation(obligation, [x_extent, 1, 2], x_extent * 2);
            for x in 0..i128::from(x_extent) {
                for z in 0..2 {
                    assert_eq!(
                        obligation.normalized_map.evaluate(&[x, 0, z]).unwrap(),
                        vec![2 * z + 1]
                    );
                }
            }
        }
    }

    #[test]
    fn execution_layout_supplies_active_axes_without_coordinate_operations() {
        let context = &mut setup();
        let source =
            affine_source_with_execution_layout("bounds_witness_missing_active_axis", [8, 4, 1]);
        let function = parse_source(context, &source);
        let report = require_production_pliron_checks_before_lowering_v2(context, &function)
            .expect("read-only affine access is safe across the declared layout");
        let (_, envelope) = bounds_envelope(&report);
        let ProductionAnalysisWitnessPayloadV1::Bounds(witness) = envelope.payload else {
            panic!("static layout must supply the complete raw bounds domain");
        };
        assert_eq!(witness.obligations.len(), 1);
        assert_layout_obligation(&witness.obligations[0], [8, 4, 1], 32);
        assert_eq!(
            witness.obligations[0]
                .normalized_map
                .evaluate(&[7, 3, 0])
                .unwrap(),
            vec![15]
        );
    }

    #[test]
    fn layout_constant_reads_complete_with_absent_or_sparse_coordinates() {
        let mut normalized = None;
        for (ordinal, coordinates) in [vec![], vec![(2, 2)], vec![(0, 8), (2, 2)]]
            .into_iter()
            .enumerate()
        {
            let context = &mut setup();
            let source = constant_layout_source(
                &format!("layout_constant_{ordinal}"),
                [8, 4, 2],
                &coordinates,
                3,
                1,
            );
            let function = parse_source(context, &source);
            let report = require_production_pliron_checks_before_lowering_v2(context, &function)
                .expect("real constant read must pass normal policy checks");
            assert!(report.is_clean());
            let (stage, envelope) = bounds_envelope(&report);
            assert!(envelope.coverage().is_complete());
            assert_eq!(
                stage.independent_validation_status(),
                KernelCheckStatusV1::Clean
            );
            assert!(
                !report
                    .report_validation()
                    .all_reports_independently_validated()
            );
            assert!(!envelope.grants_lowering_or_launch_authority());
            let ProductionAnalysisWitnessPayloadV1::Bounds(witness) = envelope.payload else {
                panic!("bounds payload");
            };
            assert_eq!(witness.obligations.len(), 1);
            let obligation = &witness.obligations[0];
            assert_eq!(obligation.extent, 16);
            assert_layout_obligation(obligation, [8, 4, 2], 64);
            assert_eq!(
                obligation.normalized_map.evaluate(&[7, 3, 1]).unwrap(),
                vec![3]
            );
            if let Some(expected) = &normalized {
                assert_eq!(&obligation.normalized_map, expected);
            } else {
                normalized = Some(obligation.normalized_map.clone());
            }
        }
    }

    #[test]
    fn sparse_coordinate_dag_keeps_its_actual_axis_in_the_full_layout_domain() {
        let context = &mut setup();
        let source = affine_source_with_execution_layout("layout_sparse_z", [8, 4, 2])
            .replace(
                "kernel.invocation_dimension 0",
                "kernel.invocation_dimension 2",
            )
            .replace("kernel.launch_extent 8", "kernel.launch_extent 2");
        let function = parse_source(context, &source);
        let report = require_production_pliron_checks_before_lowering_v2(context, &function)
            .expect("z-dependent affine read is safe");
        let (_, envelope) = bounds_envelope(&report);
        let ProductionAnalysisWitnessPayloadV1::Bounds(witness) = envelope.payload else {
            panic!("bounds payload");
        };
        assert_eq!(witness.obligations.len(), 1);
        let obligation = &witness.obligations[0];
        assert_layout_obligation(obligation, [8, 4, 2], 64);
        for x in 0..8 {
            for y in 0..4 {
                for z in 0..2 {
                    assert_eq!(
                        obligation.normalized_map.evaluate(&[x, y, z]).unwrap(),
                        vec![2 * z + 1]
                    );
                }
            }
        }
    }

    #[test]
    fn layout_raw_replay_rejects_constant_and_sparse_axis_counterexamples() {
        let context = &mut setup();
        let constant = constant_layout_source("layout_constant_oob", [8, 4, 2], &[], 16, 1);
        assert!(
            matches!(replay_with_clean_bounds_report(context, &constant),
                Err(ProductionAnalysisWitnessValidationErrorV1::BoundsCounterexample {
                    invocation, index: 16, extent: 16, ..
                }) if invocation == vec![0, 0, 0]
            )
        );
        let sparse = affine_source_with_execution_layout("layout_sparse_oob", [8, 4, 2])
            .replace(
                "kernel.invocation_dimension 0",
                "kernel.invocation_dimension 2",
            )
            .replace("kernel.launch_extent 8", "kernel.launch_extent 2")
            .replace("[16]", "[3]");
        assert!(matches!(replay_with_clean_bounds_report(context, &sparse),
            Err(ProductionAnalysisWitnessValidationErrorV1::BoundsCounterexample {
                invocation, index: 3, extent: 3, ..
            }) if invocation == vec![0, 0, 1]
        ));
    }

    #[test]
    fn layout_coordinate_declarations_remain_exact_for_constant_accesses() {
        for (name, coordinates, reason) in [
            (
                "conflicting",
                vec![(0, 4)],
                "inconsistent with gpu.execution_layout",
            ),
            (
                "duplicate",
                vec![(0, 8), (0, 8)],
                "duplicate invocation dimension 0",
            ),
            (
                "out_of_range",
                vec![(3, 1)],
                "outside the three-dimensional gpu.execution_layout",
            ),
        ] {
            let context = &mut setup();
            let source = constant_layout_source(name, [8, 4, 2], &coordinates, 3, 1);
            expect_incomplete_bounds_replay(
                replay_with_clean_bounds_report(context, &source),
                reason,
            );
        }
    }

    #[test]
    fn layout_absent_coordinates_keep_dynamic_domain_and_replay_caps() {
        let context = &mut setup();
        let exact = constant_layout_source("layout_exact_cap", [65_536, 1, 1], &[], 0, 1);
        let SupportedWitnessBuildV1::Complete(witness) =
            replay_with_clean_bounds_report(context, &exact).expect("exact replay cap")
        else {
            panic!("the exact static cap remains supported");
        };
        assert_eq!(witness.obligations.len(), 1);
        assert_layout_obligation(&witness.obligations[0], [65_536, 1, 1], 65_536);
        for (name, extents, accesses, reason) in [
            (
                "dynamic",
                [0, 1, 1],
                1,
                "cannot enumerate dynamic gpu.execution_layout axis 0",
            ),
            (
                "domain_cap",
                [65_536, 2, 1],
                1,
                "needs 131072 invocations, exceeding its exhaustive replay cap",
            ),
            (
                "cardinality_overflow",
                [2, u64::MAX, 1],
                1,
                "launch-domain cardinality overflows u64",
            ),
            (
                "work_cap",
                [65_536, 1, 1],
                17,
                "raw-index evaluation exceeded its deterministic work cap",
            ),
        ] {
            let source = constant_layout_source(name, extents, &[], 0, accesses);
            expect_incomplete_bounds_replay(
                replay_with_clean_bounds_report(context, &source),
                reason,
            );
        }
    }

    #[test]
    fn layout_does_not_extend_the_single_block_witness_fragment() {
        let context = &mut setup();
        let source = constant_layout_source("layout_multiblock", [8, 4, 2], &[], 0, 1).replace(
            "\n}\n",
            "\n  ^unreachable_block():\n    kernel.return () [] []: <() -> ()>\n}\n",
        );
        expect_incomplete_bounds_replay(
            replay_with_clean_bounds_report(context, &source),
            "cannot yet enumerate exhaustive CFG path domains",
        );
    }

    #[test]
    fn raw_constant_evaluator_still_requires_its_initial_stack_frame() {
        let context = &mut setup();
        let zero = IndexConstantOp::new(context, 0).result(context);
        let mut steps = 0;
        assert!(matches!(
            evaluate_raw_index_iterative(
                context,
                zero,
                &[7, 3, 1],
                &mut HashMap::new(),
                &mut HashSet::new(),
                &mut steps,
                0,
            ),
            Err(RawIndexEvaluationFailureV1::Incomplete(
                "raw-index evaluation exceeded its deterministic stack cap"
            ))
        ));
        assert_eq!(steps, 0);
        assert!(matches!(
            evaluate_raw_index_iterative(
                context,
                zero,
                &[7, 3, 1],
                &mut HashMap::new(),
                &mut HashSet::new(),
                &mut steps,
                1,
            ),
            Ok(0)
        ));
        assert_eq!(steps, 1);
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
