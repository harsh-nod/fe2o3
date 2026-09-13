#[cfg(test)]
mod tests {
    use dialect_gpu::{ExecutionDomainAttr, ExecutionLayoutOp};
    use dialect_kernel::{
        AtomicScopeAttr, DIALECT_NAME, InvocationIndexOp, MemorySpaceAttr, ReturnOp,
        TensorConvergenceAttr, TensorLayoutOp, register_dialect,
    };
    use fe2o3_kernel_ir::TensorLayoutContractV1;
    use fe2o3_pliron_owner_core::ensure_context_identity;
    use pliron::{
        builtin::{ops::FuncOp, types::FunctionType},
        context::Context,
        dialect::DialectName,
        op::Op,
    };
    use std::{cell::Cell, rc::Rc, sync::Arc};

    use super::*;
    use crate::production_analysis::pliron_ir_identity::BuiltIdentityV1;
    use crate::production_analysis::pliron_pass_contract::{
        BoundedPlironIdentityCaptureV1, IdentityCaptureFailureV1, IdentityComparisonFailureV1,
        MutationEpochCaptureFailureV1, PlironStructuralIdentityProviderV1,
    };
    use crate::{
        LivePlironStructuralIdentityProviderV1, PlironPassContractSessionV1,
        begin_production_pliron_pass_contract_session_v1,
        require_production_pliron_checks_before_lowering_v2, run_pliron_atomic_legality_check_v1,
        run_pliron_ranked_bounds_check_v1, run_pliron_ranked_race_check_v1,
        run_pliron_tensor_layout_check_v1,
    };

    fn setup() -> Context {
        let mut context = Context::new();
        register_dialect(
            &mut context,
            &DialectName::try_new(DIALECT_NAME).expect("valid kernel dialect name"),
        )
        .expect("register kernel dialect");
        dialect_gpu::register_dialect(&mut context).expect("register gpu dialect");
        ensure_context_identity(&mut context).expect("context identity");
        context
    }

    fn valid_function(context: &mut Context, name: &str) -> FuncOp {
        let function = FuncOp::new(
            context,
            name.try_into().expect("valid function name"),
            FunctionType::get(context, vec![], vec![]),
        );
        let entry = function.get_entry_block(context);
        for operation in [
            ExecutionLayoutOp::new_with_domain(
                context,
                7,
                [64, 1, 1],
                [64, 1, 1],
                64,
                ExecutionDomainAttr::FullPhysicalWorkgroups,
            )
            .get_operation(),
            InvocationIndexOp::new(context, 0, 64).get_operation(),
            TensorLayoutOp::new(
                context,
                &TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64(),
                TensorConvergenceAttr::UniformSubgroup,
                64,
            )
            .get_operation(),
            ReturnOp::new(context).get_operation(),
        ] {
            operation.insert_at_back(entry, context);
        }
        function
    }

    fn bare_function(context: &mut Context, name: &str) -> FuncOp {
        let function = FuncOp::new(
            context,
            name.try_into().expect("valid function name"),
            FunctionType::get(context, vec![], vec![]),
        );
        ReturnOp::new(context)
            .get_operation()
            .insert_at_back(function.get_entry_block(context), context);
        function
    }

    type LivePassSession<'a> =
        PlironPassContractSessionV1<LivePlironStructuralIdentityProviderV1<'a>>;

    fn first_bound<'a>(
        context: &'a Context,
        function: &'a FuncOp,
    ) -> (
        LivePassSession<'a>,
        ProductionAnalysisReportValidationSessionV1<'a>,
        BoundProductionAnalysisReportV1,
    ) {
        let provider = LivePlironStructuralIdentityProviderV1::new(context, function);
        let mut preservation =
            begin_production_pliron_pass_contract_session_v1(provider).expect("identity session");
        let validation_handle = preservation.validation_handle();
        let validation = begin_production_analysis_report_validation_v1(
            context,
            function,
            None,
            validation_handle,
        );
        let report = preservation
            .run_contiguous_pass(KernelCheckPassKindV1::TensorLayout, || {
                Ok::<_, ()>(run_pliron_tensor_layout_check_v1(context, function))
            })
            .expect("preserved pass")
            .expect("analysis result");
        let checkpoint = preservation.last_checkpoint().expect("checkpoint");
        let bound = validation
            .issue(
                context,
                function,
                checkpoint,
                CapturedProductionAnalysisReportV1::TensorLayout(report),
            )
            .expect("sealed first report");
        (preservation, validation, bound)
    }

    #[test]
    fn production_happy_path_completes_only_supported_witness_fragments() {
        let context = &mut setup();
        let function = valid_function(context, "validation_happy");
        let report = require_production_pliron_checks_before_lowering_v2(context, &function)
            .expect("ordinary policy pipeline remains usable");
        let validation = report.report_validation();

        assert_eq!(validation.stages().len(), 9);
        assert_eq!(validation.status(), KernelCheckStatusV1::Incomplete);
        assert!(!validation.all_reports_independently_validated());
        assert!(!validation.grants_compiler_refinement_authority());
        assert!(!validation.grants_lowering_or_launch_authority());
        for (position, stage) in validation.stages().iter().enumerate() {
            assert_eq!(stage.checkpoint().position(), position);
            assert_eq!(
                stage.checkpoint().pass(),
                PRODUCTION_PLIRON_PASS_CONTRACTS_V1[position].pass()
            );
            assert_eq!(stage.implementation().pass(), stage.checkpoint().pass());
            assert_eq!(stage.analysis_status(), KernelCheckStatusV1::Clean);
            let expected_status =
                if stage.checkpoint().pass() == KernelCheckPassKindV1::MemoryBounds {
                    KernelCheckStatusV1::Clean
                } else {
                    KernelCheckStatusV1::Incomplete
                };
            assert_eq!(stage.independent_validation_status(), expected_status);
            if expected_status == KernelCheckStatusV1::Clean {
                assert_eq!(stage.remaining_witness_gap(), None);
            } else {
                let gap = stage
                    .remaining_witness_gap()
                    .expect("remaining witness gap");
                assert_eq!(gap.pass(), stage.checkpoint().pass());
                assert!(!gap.required_evidence().is_empty());
            }
            assert!(!stage.witness().grants_compiler_refinement_authority());
            assert!(!stage.witness().grants_lowering_or_launch_authority());
        }
        assert_eq!(
            validation.stages()[2].configuration(),
            &ProductionAnalysisConfigurationV1::AtomicTargetAgnostic
        );
    }

    #[test]
    fn unchanged_report_and_checkpoint_are_accepted_without_a_second_snapshot() {
        let context = &mut setup();
        let function = valid_function(context, "validation_noop");
        let (_preservation, mut validation, bound) = first_bound(context, &function);
        validation
            .accept(context, &function, &bound)
            .expect("unaltered compiler-owned seal");
        assert_eq!(validation.next, 1);
        assert_eq!(
            validation.stages[0].analysis_status(),
            KernelCheckStatusV1::Clean
        );
    }

    #[test]
    fn cross_session_and_stale_seals_are_rejected() {
        let context = &mut setup();
        let function = valid_function(context, "validation_stale");
        let (_first_preservation, _first_validation, bound) = first_bound(context, &function);

        let provider = LivePlironStructuralIdentityProviderV1::new(context, &function);
        let second_preservation =
            begin_production_pliron_pass_contract_session_v1(provider).expect("second session");
        let mut second_validation = begin_production_analysis_report_validation_v1(
            context,
            &function,
            None,
            second_preservation.validation_handle(),
        );
        assert!(matches!(
            second_validation.accept(context, &function, &bound),
            Err(
                ProductionAnalysisReportValidationErrorV1::CounterfeitOrCrossSessionSeal {
                    position: 0
                }
            )
        ));
    }

    #[test]
    fn cross_context_and_cross_function_use_are_rejected() {
        let first_context = &mut setup();
        let first_function = valid_function(first_context, "same_text");
        let other_function = valid_function(first_context, "other_function");
        let (_preservation, mut validation, bound) = first_bound(first_context, &first_function);

        let second_context = &mut setup();
        let second_function = valid_function(second_context, "same_text");
        assert!(matches!(
            validation.accept(second_context, &second_function, &bound),
            Err(ProductionAnalysisReportValidationErrorV1::CrossContextReport { position: 0 })
        ));

        assert!(matches!(
            validation.accept(first_context, &other_function, &bound),
            Err(ProductionAnalysisReportValidationErrorV1::CrossFunctionReport { position: 0 })
        ));
    }

    #[test]
    fn replay_and_cross_stage_report_swaps_are_rejected() {
        let context = &mut setup();
        let function = valid_function(context, "validation_replay");
        let (mut preservation, mut validation, first) = first_bound(context, &function);
        let wrong_report = first.issued_report.clone();
        validation
            .accept(context, &function, &first)
            .expect("first use accepted");
        assert!(matches!(
            validation.accept(context, &function, &first),
            Err(ProductionAnalysisReportValidationErrorV1::ReplayedReport {
                issued_position: 0,
                current_position: 1
            })
        ));

        preservation
            .run_contiguous_pass(KernelCheckPassKindV1::MemoryBounds, || Ok::<_, ()>(()))
            .expect("preserved bounds checkpoint")
            .expect("pass result");
        let swapped = validation
            .issue(
                context,
                &function,
                preservation.last_checkpoint().expect("bounds checkpoint"),
                wrong_report,
            )
            .expect("seal records the actual submitted report kind");
        assert!(matches!(
            validation.accept(context, &function, &swapped),
            Err(
                ProductionAnalysisReportValidationErrorV1::StageOrderMismatch {
                    position: 1,
                    expected: KernelCheckPassKindV1::MemoryBounds,
                    observed: KernelCheckPassKindV1::TensorLayout,
                }
            )
        ));
    }

    #[test]
    fn checkpoint_implementation_configuration_payload_and_status_tampering_are_rejected() {
        let context = &mut setup();
        let function = valid_function(context, "validation_tamper");

        let (_preservation, mut validation, mut bound) = first_bound(context, &function);
        bound.submitted_checkpoint.pass = KernelCheckPassKindV1::MemoryBounds;
        assert!(matches!(
            validation.accept(context, &function, &bound),
            Err(
                ProductionAnalysisReportValidationErrorV1::CheckpointMetadataTampered {
                    position: 0
                }
            )
        ));

        let (_preservation, mut validation, mut bound) = first_bound(context, &function);
        bound.submitted_checkpoint.mutation_epoch =
            bound.submitted_checkpoint.mutation_epoch.wrapping_add(1);
        assert!(matches!(
            validation.accept(context, &function, &bound),
            Err(
                ProductionAnalysisReportValidationErrorV1::CheckpointMetadataTampered {
                    position: 0
                }
            )
        ));

        let (_preservation, mut validation, mut bound) = first_bound(context, &function);
        bound.implementation = ProductionAnalysisImplementationV1::PlironRankedBoundsV1;
        assert!(matches!(
            validation.accept(context, &function, &bound),
            Err(ProductionAnalysisReportValidationErrorV1::ImplementationTampered { position: 0 })
        ));

        let (_preservation, mut validation, mut bound) = first_bound(context, &function);
        bound.configuration = ProductionAnalysisConfigurationV1::AtomicTargetAgnostic;
        assert!(matches!(
            validation.accept(context, &function, &bound),
            Err(ProductionAnalysisReportValidationErrorV1::ConfigurationTampered { position: 0 })
        ));

        let (_preservation, mut validation, mut bound) = first_bound(context, &function);
        bound.submitted_report = CapturedProductionAnalysisReportV1::Bounds(
            run_pliron_ranked_bounds_check_v1(context, &function),
        );
        assert!(matches!(
            validation.accept(context, &function, &bound),
            Err(ProductionAnalysisReportValidationErrorV1::ReportPayloadTampered { position: 0 })
        ));

        let (_preservation, mut validation, mut bound) = first_bound(context, &function);
        bound.claimed_status = KernelCheckStatusV1::Incomplete;
        assert!(matches!(
            validation.accept(context, &function, &bound),
            Err(ProductionAnalysisReportValidationErrorV1::ReportStatusTampered { position: 0 })
        ));
    }

    #[test]
    fn omitted_report_is_terminal_before_manifest_admission() {
        let context = &mut setup();
        let function = valid_function(context, "validation_omitted");
        let completed = require_production_pliron_checks_before_lowering_v2(context, &function)
            .expect("completed reference run");
        let provider = LivePlironStructuralIdentityProviderV1::new(context, &function);
        let preservation =
            begin_production_pliron_pass_contract_session_v1(provider).expect("new session");
        let validation = begin_production_analysis_report_validation_v1(
            context,
            &function,
            None,
            preservation.validation_handle(),
        );
        assert!(matches!(
            validation.finish(completed.preservation()),
            Err(ProductionAnalysisReportValidationErrorV1::OmittedReport {
                position: 0,
                pass: KernelCheckPassKindV1::TensorLayout,
            })
        ));
    }

    #[test]
    fn non_clean_report_remains_non_clean_and_independently_incomplete() {
        let context = &mut setup();
        let function = bare_function(context, "validation_negative");
        let provider = LivePlironStructuralIdentityProviderV1::new(context, &function);
        let mut preservation =
            begin_production_pliron_pass_contract_session_v1(provider).expect("identity session");
        let mut validation = begin_production_analysis_report_validation_v1(
            context,
            &function,
            None,
            preservation.validation_handle(),
        );

        let tensor = preservation
            .run_contiguous_pass(KernelCheckPassKindV1::TensorLayout, || {
                Ok::<_, ()>(run_pliron_tensor_layout_check_v1(context, &function))
            })
            .unwrap()
            .unwrap();
        validation
            .record(
                context,
                &function,
                preservation.last_checkpoint().unwrap(),
                &tensor,
            )
            .unwrap();
        let bounds = preservation
            .run_contiguous_pass(KernelCheckPassKindV1::MemoryBounds, || {
                Ok::<_, ()>(run_pliron_ranked_bounds_check_v1(context, &function))
            })
            .unwrap()
            .unwrap();
        validation
            .record(
                context,
                &function,
                preservation.last_checkpoint().unwrap(),
                &bounds,
            )
            .unwrap();
        let atomics = preservation
            .run_contiguous_pass(KernelCheckPassKindV1::AtomicLegality, || {
                Ok::<_, ()>(run_pliron_atomic_legality_check_v1(context, &function))
            })
            .unwrap()
            .unwrap();
        validation
            .record(
                context,
                &function,
                preservation.last_checkpoint().unwrap(),
                &atomics,
            )
            .unwrap();
        let race = preservation
            .run_contiguous_pass(KernelCheckPassKindV1::RaceFreedom, || {
                Ok::<_, ()>(run_pliron_ranked_race_check_v1(context, &function))
            })
            .unwrap()
            .unwrap();
        let observed_status = race.status();
        validation
            .record(
                context,
                &function,
                preservation.last_checkpoint().unwrap(),
                &race,
            )
            .unwrap();
        assert_eq!(validation.stages[3].analysis_status(), observed_status);
        let mut rejected = validation.stages[3].clone();
        rejected.analysis_status = KernelCheckStatusV1::Rejected;
        assert_eq!(
            rejected.independent_validation_status(),
            KernelCheckStatusV1::Rejected
        );
        assert_eq!(
            validation.stages[3].independent_validation_status(),
            KernelCheckStatusV1::Incomplete
        );
    }

    struct CountingProvider<'a> {
        inner: LivePlironStructuralIdentityProviderV1<'a>,
        captures: Rc<Cell<usize>>,
    }

    impl PlironStructuralIdentityProviderV1 for CountingProvider<'_> {
        type Snapshot = BuiltIdentityV1;

        fn mutation_epoch(&self) -> Result<u64, MutationEpochCaptureFailureV1> {
            self.inner.mutation_epoch()
        }

        fn capture_with_resource_limits_v1(
            &mut self,
            limits: ProductionAnalysisResourceLimitsV1,
        ) -> Result<BoundedPlironIdentityCaptureV1<Self::Snapshot>, IdentityCaptureFailureV1>
        {
            self.captures.set(self.captures.get() + 1);
            self.inner.capture_with_resource_limits_v1(limits)
        }

        fn label(&self, snapshot: &Self::Snapshot) -> PlironStructuralIdentityLabelV1 {
            self.inner.label(snapshot)
        }

        fn require_exact_identity(
            &self,
            expected: &Self::Snapshot,
            observed: &Self::Snapshot,
        ) -> Result<(), IdentityComparisonFailureV1> {
            self.inner.require_exact_identity(expected, observed)
        }

        fn retain_exact_identity(&self, snapshot: Self::Snapshot) -> Arc<[u8]> {
            self.inner.retain_exact_identity(snapshot)
        }
    }

    #[test]
    fn complete_report_validation_keeps_structural_capture_count_at_nine() {
        let context = &mut setup();
        let function = valid_function(context, "validation_capture_count");
        let reports = require_production_pliron_checks_before_lowering_v2(context, &function)
            .expect("source reports");
        let captures = Rc::new(Cell::new(0));
        let provider = CountingProvider {
            inner: LivePlironStructuralIdentityProviderV1::new(context, &function),
            captures: Rc::clone(&captures),
        };
        let mut preservation =
            begin_production_pliron_pass_contract_session_v1(provider).expect("identity session");
        let mut validation = begin_production_analysis_report_validation_v1(
            context,
            &function,
            None,
            preservation.validation_handle(),
        );

        macro_rules! checkpoint_report {
            ($pass:expr, $report:expr) => {{
                preservation
                    .run_contiguous_pass($pass, || Ok::<_, ()>(()))
                    .expect("exact checkpoint")
                    .expect("pass result");
                validation
                    .record(
                        context,
                        &function,
                        preservation.last_checkpoint().expect("checkpoint token"),
                        $report,
                    )
                    .expect("sealed report");
            }};
        }
        checkpoint_report!(KernelCheckPassKindV1::TensorLayout, reports.tensor_layout());
        checkpoint_report!(KernelCheckPassKindV1::MemoryBounds, reports.bounds());
        checkpoint_report!(KernelCheckPassKindV1::AtomicLegality, reports.atomics());
        checkpoint_report!(KernelCheckPassKindV1::RaceFreedom, reports.race());
        checkpoint_report!(
            KernelCheckPassKindV1::HierarchicalOwnership,
            reports.ownership()
        );
        checkpoint_report!(
            KernelCheckPassKindV1::BarrierConvergence,
            reports.barriers()
        );
        checkpoint_report!(
            KernelCheckPassKindV1::PipelineProtocol,
            reports.pipeline_protocol()
        );
        checkpoint_report!(KernelCheckPassKindV1::WorkgroupMemory, reports.workgroup());
        checkpoint_report!(
            KernelCheckPassKindV1::SemanticRefinement,
            reports.semantics()
        );

        let preservation = preservation.finish().expect("complete preservation");
        validation
            .finish_validation(&preservation)
            .expect("custody-bound validation");
        assert_eq!(
            captures.get(),
            1 + PRODUCTION_ANALYSIS_REPORT_COUNT_V1,
            "report sealing must not add structural identity captures"
        );
    }

    #[test]
    fn report_validation_bound_has_an_independent_exact_sum_and_one_under_rejects() {
        let producing = ProductionAnalysisResourceUpperBoundV1::checked_phase(
            ProductionAnalysisResourcePhaseV1::TensorLayout,
            101,
            17,
            5,
        )
        .unwrap();
        let census = ProductionAnalysisInputCensusV1::default();
        let limits = ProductionAnalysisResourceLimitsV1::production_hard_ceiling();
        let bound = report_validation_stage_resource_upper_bound_v1(
            producing,
            ProductionAnalysisReportPayloadReceiptV1::fallback(
                KernelCheckPassKindV1::TensorLayout,
                0,
            ),
            3,
            KernelCheckPassKindV1::TensorLayout,
            census,
            limits,
        )
        .unwrap();
        let witness = preflight_production_analysis_witness_resource_upper_bound_v1(
            KernelCheckPassKindV1::TensorLayout,
            census,
            limits,
        )
        .unwrap();
        let expected_work = 17 * 9 + 3 * 6 + 32;
        let expected_retained = 17 + 3 * 2 + 1;
        let expected_temporary = 17 * 2 + 3 * 3 + 1;
        assert_eq!(
            bound.work_upper_bound(),
            expected_work + witness.work_upper_bound()
        );
        assert_eq!(
            bound.retained_storage_upper_bound(),
            expected_retained + witness.retained_storage_upper_bound()
        );
        assert_eq!(
            bound.peak_storage_upper_bound(),
            (expected_retained + expected_temporary) + witness.peak_storage_upper_bound()
        );
        let expected_work = bound.work_upper_bound();
        let expected_storage = bound.peak_storage_upper_bound();
        assert!(
            ProductionAnalysisResourceLimitsV1::new(expected_work, expected_storage).admits(bound)
        );
        assert!(
            !ProductionAnalysisResourceLimitsV1::new(expected_work - 1, expected_storage)
                .admits(bound)
        );
        assert!(
            !ProductionAnalysisResourceLimitsV1::new(expected_work, expected_storage - 1)
                .admits(bound)
        );
    }

    #[test]
    fn report_validation_setup_is_admitted_before_target_and_stage_allocation() {
        let context = &mut setup();
        let function = valid_function(context, "validation_setup_resource_boundary");
        let target = PlironAtomicTargetContextV1::new([
            PlironAtomicTargetCapabilityV1::new(
                32,
                MemorySpaceAttr::Global,
                AtomicScopeAttr::Device,
            )
            .unwrap(),
            PlironAtomicTargetCapabilityV1::new(
                64,
                MemorySpaceAttr::Global,
                AtomicScopeAttr::System,
            )
            .unwrap(),
        ])
        .unwrap();
        let provider = LivePlironStructuralIdentityProviderV1::new(context, &function);
        let preservation =
            begin_production_pliron_pass_contract_session_v1(provider).expect("identity session");

        // Two copied capabilities plus eight explicitly enumerated fixed setup
        // actions; storage is two capabilities, nine stage slots, and one
        // fixed session owner.
        let exact_work = 2 + 8;
        let exact_storage = 2 + PRODUCTION_ANALYSIS_REPORT_COUNT_V1 + 1;
        let validation = begin_production_analysis_report_validation_with_resource_limits_v1(
            context,
            &function,
            Some(&target),
            preservation.validation_handle(),
            ProductionAnalysisInputCensusV1::default(),
            ProductionAnalysisResourceLimitsV1::new(exact_work, exact_storage),
        )
        .expect("exact setup envelope");
        assert_eq!(
            validation.setup_resource_upper_bound_v1(),
            ProductionAnalysisResourceUpperBoundV1::checked_phase(
                ProductionAnalysisResourcePhaseV1::ReportValidation,
                exact_work,
                exact_storage,
                0,
            )
            .unwrap()
        );
        assert_eq!(validation.stages.len(), 0);
        assert!(validation.stages.capacity() >= PRODUCTION_ANALYSIS_REPORT_COUNT_V1);
        assert!(matches!(
            validation.atomic_configuration,
            ProductionAnalysisConfigurationV1::AtomicTarget { ref capabilities }
                if capabilities.len() == 2
        ));

        assert!(matches!(
            begin_production_analysis_report_validation_with_resource_limits_v1(
                context,
                &function,
                Some(&target),
                preservation.validation_handle(),
                ProductionAnalysisInputCensusV1::default(),
                ProductionAnalysisResourceLimitsV1::new(exact_work - 1, exact_storage),
            ),
            Err(ProductionAnalysisReportValidationErrorV1::ResourceLimit {
                producing_pass: None,
                resource: "work upper bound"
            })
        ));
        assert!(matches!(
            begin_production_analysis_report_validation_with_resource_limits_v1(
                context,
                &function,
                Some(&target),
                preservation.validation_handle(),
                ProductionAnalysisInputCensusV1::default(),
                ProductionAnalysisResourceLimitsV1::new(exact_work, exact_storage - 1),
            ),
            Err(ProductionAnalysisReportValidationErrorV1::ResourceLimit {
                producing_pass: None,
                resource: "peak storage upper bound"
            })
        ));
    }

    #[test]
    fn empty_report_payload_has_exact_bound_without_releasing_producer_storage() {
        let context = &mut setup();
        let function = valid_function(context, "validation_exact_payload");
        let producing = ProductionAnalysisResourceUpperBoundV1::checked_phase(
            ProductionAnalysisResourcePhaseV1::TensorLayout,
            101,
            1_000_000,
            5,
        )
        .unwrap();
        let limits = ProductionAnalysisResourceLimitsV1::production_hard_ceiling();
        let report = run_pliron_tensor_layout_check_v1(context, &function);
        let receipt = report.payload_receipt_v1(limits).unwrap();
        assert!(receipt.is_exact());
        let census = ProductionAnalysisInputCensusV1::default();
        let bound = report_validation_stage_resource_upper_bound_v1(
            producing,
            receipt,
            0,
            report.pass(),
            census,
            limits,
        )
        .unwrap();
        let witness = preflight_production_analysis_witness_resource_upper_bound_v1(
            report.pass(),
            census,
            limits,
        )
        .unwrap();
        assert_eq!(
            bound.work_upper_bound(),
            2 + 3 * 3 + 2 + 4 + 32 + witness.work_upper_bound()
        );
        assert_eq!(
            bound.retained_storage_upper_bound(),
            1 + witness.retained_storage_upper_bound()
        );
        assert_eq!(
            bound.peak_storage_upper_bound(),
            2 + witness.peak_storage_upper_bound()
        );
        let composed = producing
            .checked_then_retain(bound, ProductionAnalysisResourcePhaseV1::ReportValidation)
            .unwrap();
        assert_eq!(
            composed.retained_storage_upper_bound(),
            1_000_000 + bound.retained_storage_upper_bound()
        );
        assert!(
            composed.peak_storage_upper_bound() >= 1_000_000 + bound.peak_storage_upper_bound()
        );

        for (work, storage, expected) in [
            (
                bound.work_upper_bound() - 1,
                bound.peak_storage_upper_bound(),
                Some("work upper bound"),
            ),
            (
                bound.work_upper_bound(),
                bound.peak_storage_upper_bound() - 1,
                Some("peak storage upper bound"),
            ),
            (
                bound.work_upper_bound(),
                bound.peak_storage_upper_bound(),
                None,
            ),
        ] {
            let (preservation, mut validation, _) = first_bound(context, &function);
            let mut analyses = PlironAnalysisManagerV1::new(&function);
            let result = validation.record_with_resource_limits_v1(
                ProductionAnalysisReportEndpointV1 {
                    context,
                    function: &function,
                },
                preservation.last_checkpoint().unwrap(),
                &report,
                producing,
                ProductionAnalysisResourceLimitsV1::new(work, storage),
                &mut analyses,
            );
            if let Some(resource) = expected {
                assert_eq!(
                    result,
                    Err(ProductionAnalysisReportValidationErrorV1::ResourceLimit {
                        producing_pass: Some(KernelCheckPassKindV1::TensorLayout),
                        resource,
                    })
                );
                assert_eq!(validation.next, 0);
                assert!(validation.stages.is_empty());
                assert!(validation.last_stage_resource_upper_bound_v1().is_none());
            } else {
                result.unwrap();
                assert_eq!(validation.next, 1);
                assert_eq!(validation.stages.len(), 1);
                assert_eq!(validation.last_stage_resource_upper_bound_v1(), Some(bound));
            }
        }
    }

    #[test]
    fn allocation_failure_does_not_append_a_stage_or_poison_retry() {
        let context = &mut setup();
        let function = valid_function(context, "validation_payload_allocation");
        let reports = require_production_pliron_checks_before_lowering_v2(context, &function)
            .expect("real production reports");
        let provider = LivePlironStructuralIdentityProviderV1::new(context, &function);
        let mut preservation = begin_production_pliron_pass_contract_session_v1(provider).unwrap();
        let mut validation = begin_production_analysis_report_validation_v1(
            context,
            &function,
            None,
            preservation.validation_handle(),
        );

        macro_rules! record_stage {
            ($pass:ident, $report:expr) => {
                preservation
                    .run_contiguous_pass(KernelCheckPassKindV1::$pass, || Ok::<_, ()>(()))
                    .unwrap()
                    .unwrap();
                validation
                    .record(
                        context,
                        &function,
                        preservation.last_checkpoint().unwrap(),
                        $report,
                    )
                    .unwrap();
            };
        }
        record_stage!(TensorLayout, reports.tensor_layout());
        record_stage!(MemoryBounds, reports.bounds());
        record_stage!(AtomicLegality, reports.atomics());
        record_stage!(RaceFreedom, reports.race());
        record_stage!(HierarchicalOwnership, reports.ownership());
        record_stage!(BarrierConvergence, reports.barriers());
        record_stage!(PipelineProtocol, reports.pipeline_protocol());
        record_stage!(WorkgroupMemory, reports.workgroup());
        preservation
            .run_contiguous_pass(
                KernelCheckPassKindV1::SemanticRefinement,
                || Ok::<_, ()>(()),
            )
            .unwrap()
            .unwrap();
        let previous_receipt = validation.last_stage_resource_upper_bound_v1();
        let report = reports.semantics();
        assert!(report.progress().certificates().is_empty());
        assert!(
            report
                .payload_receipt_v1(ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),)
                .unwrap()
                .is_exact()
        );

        // Each semantic clone reserves roots, numerical certificates, and
        // progress certificates. Fail every reserve in all three clone owners.
        for successful_allocations in 0..9 {
            super::super::pliron_report_payload_receipt::fail_payload_allocation_after_v1(
                successful_allocations,
            );
            let result = validation.record(
                context,
                &function,
                preservation.last_checkpoint().unwrap(),
                report,
            );
            assert_eq!(
                result,
                Err(ProductionAnalysisReportValidationErrorV1::ResourceLimit {
                    producing_pass: Some(KernelCheckPassKindV1::SemanticRefinement),
                    resource: "report payload allocation",
                }),
                "allocation checkpoint {successful_allocations}"
            );
            assert_eq!(validation.next, 8);
            assert_eq!(validation.stages.len(), 8);
            assert_eq!(
                validation.last_stage_resource_upper_bound_v1(),
                previous_receipt
            );
        }
        validation
            .record(
                context,
                &function,
                preservation.last_checkpoint().unwrap(),
                report,
            )
            .unwrap();
        assert_eq!(validation.next, 9);
        assert_eq!(validation.stages.len(), 9);
        validation
            .finish_validation(&preservation.finish().unwrap())
            .unwrap();
    }

    #[test]
    fn report_validation_bound_overflow_is_typed() {
        let producing = ProductionAnalysisResourceUpperBoundV1::checked_phase(
            ProductionAnalysisResourcePhaseV1::TensorLayout,
            0,
            usize::MAX,
            0,
        )
        .unwrap();
        let error = report_validation_stage_resource_upper_bound_v1(
            producing,
            ProductionAnalysisReportPayloadReceiptV1::fallback(
                KernelCheckPassKindV1::TensorLayout,
                0,
            ),
            0,
            KernelCheckPassKindV1::TensorLayout,
            ProductionAnalysisInputCensusV1::default(),
            ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
        )
        .unwrap_err();
        assert_eq!(
            error.phase,
            ProductionAnalysisResourcePhaseV1::ReportValidation
        );
        assert_eq!(error.resource, "report payload accounting overflow");
    }
}
