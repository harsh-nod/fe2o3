mod observed_validation_boundaries {
    use super::*;
    use crate::production_analysis::pliron_pass_contract::begin_production_pliron_pass_contract_session_with_observation_v1 as begin_pass;
    use crate::production_analysis::pliron_pipeline::invocation_receipt_v1::{
        InvocationReceiptFailureV1 as Failure, InvocationReceiptV1 as Receipt,
    };
    use crate::production_analysis::pliron_tensor_layout::{
        preflight_tensor_layout_resource_upper_bound_v1 as tensor_bound,
        run_pliron_tensor_layout_check_with_analyses_v1 as tensor_run,
    };

    type B = ProductionAnalysisResourceUpperBoundV1;
    type L = ProductionAnalysisResourceLimitsV1;
    type P = ProductionAnalysisResourcePhaseV1;
    type M = PlironAnalysisManagerV1;
    type K = KernelCheckPassKindV1;
    type V<'a> = ProductionAnalysisReportValidationSessionV1<'a>;
    const RV: P = P::ReportValidation;
    const PP: P = P::PassPreservation;

    fn hard() -> L {
        L::production_hard_ceiling()
    }

    fn seed<'a>(c: &'a Context, f: &'a FuncOp) -> (LivePassSession<'a>, M, Receipt<'static>) {
        seed_with_floor(c, f, B::default())
    }

    fn seed_with_floor<'a>(
        c: &'a Context,
        f: &'a FuncOp,
        floor: B,
    ) -> (LivePassSession<'a>, M, Receipt<'static>) {
        let mut receipt = Receipt::new(floor, hard()).unwrap();
        let phase = receipt.phase(P::StructuralIdentity, 0).unwrap();
        let preservation = begin_pass(
            LivePlironStructuralIdentityProviderV1::new(c, f),
            hard(),
            Some(&phase.observer(&Ok)),
        )
        .unwrap();
        let mut analyses = M::new_with_resource_contract(
            f,
            preservation.input_census_v1(),
            preservation.initial_identity_resource_upper_bound_v1(),
            preservation
                .lineage_identity_resource_upper_bound_v1()
                .retained_storage_upper_bound(),
            hard(),
        )
        .unwrap();
        phase
            .observer(&Ok)
            .require(
                hard(),
                P::FunctionInventory,
                Ok(analyses.resource_upper_bound()),
            )
            .unwrap();
        analyses.prepare_function_inventory(c, f);
        assert!(analyses.function_inventory().is_ok());
        // Hold the bootstrap reservation, including its temporary peak.
        drop(phase);
        (preservation, analyses, receipt)
    }

    fn start<'a>(
        c: &'a Context,
        f: &'a FuncOp,
    ) -> (LivePassSession<'a>, M, V<'a>, Receipt<'static>) {
        start_with_floor(c, f, B::default())
    }

    fn start_with_floor<'a>(
        c: &'a Context,
        f: &'a FuncOp,
        floor: B,
    ) -> (LivePassSession<'a>, M, V<'a>, Receipt<'static>) {
        let (preservation, mut analyses, mut receipt) = seed_with_floor(c, f, floor);
        let phase = receipt.phase(RV, 0).unwrap();
        let validation = begin_production_analysis_report_validation_with_observation_v1(
            c,
            f,
            None,
            preservation.validation_handle(),
            preservation.input_census_v1(),
            analyses.remaining_resource_limits(RV).unwrap(),
            Some(&phase.observer(&Ok)),
        )
        .unwrap();
        let bound = validation.setup_resource_upper_bound_v1();
        analyses
            .admit_retained_resource_upper_bound(RV, bound)
            .unwrap();
        phase.commit(bound).unwrap();
        (preservation, analyses, validation, receipt)
    }

    fn checkpoint<T>(
        preservation: &mut LivePassSession<'_>,
        analyses: &mut M,
        receipt: &mut Receipt,
        pass: K,
        producer: Option<(P, B)>,
        execute: impl FnOnce(&mut M) -> T,
    ) -> T {
        let old = preservation
            .lineage_identity_resource_upper_bound_v1()
            .retained_storage_upper_bound();
        let h = producer.map_or(B::default(), |(_, h)| h);
        let phase = receipt.phase(PP, old).unwrap();
        let observer = phase.observer(&Ok);
        observer
            .require(
                hard(),
                producer.map_or(PP, |(kind, _)| kind),
                B::checked_phase(PP, 0, old, 0)
                    .unwrap()
                    .checked_then_retain(h, PP),
            )
            .unwrap();
        if let Some((kind, bound)) = producer {
            analyses
                .admit_retained_resource_upper_bound(kind, bound)
                .unwrap();
        }
        let limits = analyses
            .remaining_identity_replacement_resource_limits_v1(old)
            .unwrap();
        let result = observer
            .with_projection(&|local| h.checked_then_retain(local, PP), |o| {
                preservation.run_contiguous_pass_with_observation_v1(
                    pass,
                    limits,
                    || Ok::<_, ()>(execute(analyses)),
                    Some(o),
                )
            })
            .unwrap()
            .unwrap();
        analyses
            .resource_contract_replace_retained_v1(
                PP,
                old,
                preservation
                    .last_checkpoint_resource_upper_bound_v1()
                    .unwrap(),
                preservation
                    .lineage_identity_resource_upper_bound_v1()
                    .retained_storage_upper_bound(),
            )
            .unwrap();
        drop(phase);
        result
    }

    #[test]
    fn actual_validation_setup_exact_and_local_global_short() {
        let u = {
            let mut c = setup();
            let f = bare_function(&mut c, "observed_setup_baseline");
            let (_, _, validation, _) = start(&c, &f);
            validation.setup_resource_upper_bound_v1()
        };
        for case in 0..5 {
            let mut ordinary = None;
            for observed in [false, true] {
                let mut c = setup();
                let f = bare_function(&mut c, "observed_setup_boundary");
                let (p, mut m, prior) = seed(&c, &f);
                let floor = prior.complete().unwrap();
                assert_eq!(
                    floor.retained_storage_upper_bound(),
                    floor.peak_storage_upper_bound()
                );
                let total = floor.checked_then_retain(u, RV).unwrap();
                hard().require(RV, total).unwrap();
                let local = L::new(
                    u.work_upper_bound() - usize::from(case == 1),
                    u.peak_storage_upper_bound() - usize::from(case == 2),
                );
                let global = L::new(
                    total.work_upper_bound() - usize::from(case == 3),
                    total.peak_storage_upper_bound() - usize::from(case == 4),
                );
                let before = m.resource_upper_bound();
                let mut receipt = Receipt::new(floor, global).unwrap();
                let phase = receipt.phase(RV, 0).unwrap();
                let observer = phase.observer(&Ok);
                let result = begin_production_analysis_report_validation_with_observation_v1(
                    &c,
                    &f,
                    None,
                    p.validation_handle(),
                    p.input_census_v1(),
                    local,
                    observed.then_some(&observer),
                );
                let scalar = result
                    .as_ref()
                    .map(|v| (v.next, v.stages.len(), v.setup_resource_upper_bound_v1()))
                    .map_err(Clone::clone);
                if !observed {
                    if case >= 3 {
                        assert!(result.is_ok());
                    }
                    ordinary = Some(scalar);
                    drop(phase);
                    continue;
                }
                if case <= 2 {
                    assert_eq!(Some(scalar), ordinary);
                }
                if case == 0 {
                    let validation = result.unwrap();
                    assert_eq!(validation.setup_resource_upper_bound_v1(), u);
                    m.admit_retained_resource_upper_bound(RV, u).unwrap();
                    phase.commit(u).unwrap();
                    assert_eq!(receipt.complete(), Ok(u));
                    assert!(!receipt.snapshot().caught_panic);
                } else {
                    let resource = if case == 2 || case == 4 {
                        "peak storage upper bound"
                    } else {
                        "work upper bound"
                    };
                    assert!(matches!(
                        result,
                        Err(ProductionAnalysisReportValidationErrorV1::ResourceLimit {
                            producing_pass: None, resource: actual,
                        }) if actual == resource
                    ));
                    drop(phase);
                    assert_eq!(receipt.snapshot().committed, B::default());
                    assert_eq!(
                        receipt.complete(),
                        Err(Failure::Denied(ProductionAnalysisResourceLimitV1 {
                            phase: RV,
                            resource
                        },))
                    );
                    assert!(!receipt.snapshot().caught_panic);
                    assert_eq!(m.resource_upper_bound(), before);
                }
            }
        }
    }

    #[test]
    fn actual_empty_tensor_record_exact_and_local_global_short() {
        for case in 0..7 {
            let mut ordinary = None;
            for observed in [false, true] {
                let mut c = setup();
                let f = bare_function(&mut c, "observed_tensor_record");
                let (mut p, mut m, mut v, mut prior) = start(&c, &f);
                let h = tensor_bound(
                    p.input_census_v1(),
                    None,
                    m.remaining_resource_limits(P::TensorLayout).unwrap(),
                )
                .unwrap();
                let report = checkpoint(
                    &mut p,
                    &mut m,
                    &mut prior,
                    K::TensorLayout,
                    Some((P::TensorLayout, h)),
                    |m| tensor_run(&c, &f, m),
                );
                let payload = report
                    .payload_receipt_with_observation_v1(hard(), None)
                    .unwrap();
                let full = report_validation_stage_resource_upper_bound_v1(
                    h,
                    payload,
                    0,
                    K::TensorLayout,
                    p.input_census_v1(),
                    hard(),
                )
                .unwrap();
                let floor = prior.complete().unwrap();
                assert_eq!(
                    floor.retained_storage_upper_bound(),
                    floor.peak_storage_upper_bound()
                );
                let total = floor.checked_then_retain(full, RV).unwrap();
                hard().require(RV, total).unwrap();
                let local = L::new(
                    match case {
                        1 => 1,
                        2 => full.work_upper_bound() - 1,
                        _ => full.work_upper_bound(),
                    },
                    full.peak_storage_upper_bound() - usize::from(case == 3),
                );
                let global = L::new(
                    match case {
                        4 => floor.work_upper_bound() + 1,
                        5 => total.work_upper_bound() - 1,
                        _ => total.work_upper_bound(),
                    },
                    total.peak_storage_upper_bound() - usize::from(case == 6),
                );
                let before = m.resource_upper_bound();
                let mut receipt = Receipt::new(floor, global).unwrap();
                let phase = receipt.phase(RV, 0).unwrap();
                let observer = phase.observer(&Ok);
                let result = v.record_with_observation_v1(
                    ProductionAnalysisReportEndpointV1 {
                        context: &c,
                        function: &f,
                    },
                    p.last_checkpoint().unwrap(),
                    &report,
                    h,
                    local,
                    (&mut m, observed.then_some(&observer)),
                );
                if result.is_ok() {
                    m.admit_retained_resource_upper_bound(RV, full).unwrap();
                }
                let state = (
                    result.clone(),
                    v.next,
                    v.stages.len(),
                    v.last_stage_resource_upper_bound_v1(),
                    m.resource_upper_bound(),
                );
                if !observed {
                    if case >= 4 {
                        assert_eq!(result, Ok(()));
                    }
                    ordinary = Some(state);
                    drop(phase);
                    continue;
                }
                if case <= 3 {
                    assert_eq!(Some(state), ordinary);
                }
                if case == 0 {
                    assert_eq!(result, Ok(()));
                    assert_eq!((v.next, v.stages.len()), (1, 1));
                    assert_eq!(v.last_stage_resource_upper_bound_v1(), Some(full));
                    phase.commit(full).unwrap();
                    assert_eq!(receipt.complete(), Ok(full));
                    assert!(!receipt.snapshot().caught_panic);
                } else {
                    assert_eq!(
                        result,
                        Err(ProductionAnalysisReportValidationErrorV1::ResourceLimit {
                            producing_pass: Some(K::TensorLayout),
                            resource: if case == 3 || case == 6 {
                                "peak storage upper bound"
                            } else {
                                "work upper bound"
                            },
                        })
                    );
                    drop(phase);
                    let accepted = if case == 1 || case == 4 {
                        B::default()
                    } else {
                        B::checked_phase(RV, 2, 0, 0).unwrap()
                    };
                    assert_eq!(receipt.snapshot().committed, accepted);
                    assert_eq!(
                        receipt.complete(),
                        Err(Failure::Denied(ProductionAnalysisResourceLimitV1 {
                            phase: RV,
                            resource: if case == 3 || case == 6 {
                                "peak storage upper bound"
                            } else {
                                "work upper bound"
                            },
                        },))
                    );
                    assert!(!receipt.snapshot().caught_panic);
                    assert_eq!(
                        (
                            v.next,
                            v.stages.len(),
                            v.last_stage_resource_upper_bound_v1()
                        ),
                        (0, 0, None),
                    );
                    assert_eq!(m.resource_upper_bound(), before);
                }
            }
        }
    }
    #[test]
    fn actual_semantic_all_nine_clone_failures_keep_prefix_and_denial() {
        use crate::production_analysis::pliron_pipeline::require_production_pliron_checks_before_lowering_with_resource_limits_v1 as pipeline;
        use crate::production_analysis::pliron_report_payload_receipt::fail_payload_allocation_after_v1 as fail_after;

        let mut c = setup();
        let f = valid_function(&mut c, "observed_semantic_allocation");
        // The complete producer owner stays live as the external floor.
        let source = pipeline(&c, &f, hard()).unwrap();
        let h = source.resource_upper_bound;
        let (mut p, mut m, mut v, mut receipt) = start_with_floor(&c, &f, h);

        macro_rules! record_stage {
            ($pass:ident, $report:expr) => {{
                // Existing reports are covered by H; produce only a checkpoint.
                checkpoint(&mut p, &mut m, &mut receipt, K::$pass, None, |_| ());
                let phase = receipt.phase(RV, 0).unwrap();
                let observer = phase.observer(&Ok);
                let limits = m.remaining_resource_limits(RV).unwrap();
                v.record_with_observation_v1(
                    ProductionAnalysisReportEndpointV1 {
                        context: &c,
                        function: &f,
                    },
                    p.last_checkpoint().unwrap(),
                    $report,
                    h,
                    limits,
                    (&mut m, Some(&observer)),
                )
                .unwrap();
                let bound = v.last_stage_resource_upper_bound_v1().unwrap();
                m.admit_retained_resource_upper_bound(RV, bound).unwrap();
                phase.commit(bound).unwrap();
            }};
        }
        record_stage!(TensorLayout, source.report.tensor_layout());
        record_stage!(MemoryBounds, source.report.bounds());
        record_stage!(AtomicLegality, source.report.atomics());
        record_stage!(RaceFreedom, source.report.race());
        record_stage!(HierarchicalOwnership, source.report.ownership());
        record_stage!(BarrierConvergence, source.report.barriers());
        record_stage!(PipelineProtocol, source.report.pipeline_protocol());
        record_stage!(WorkgroupMemory, source.report.workgroup());
        checkpoint(
            &mut p,
            &mut m,
            &mut receipt,
            K::SemanticRefinement,
            None,
            |_| (),
        );
        assert_eq!((v.next, v.stages.len()), (8, 8));
        assert_eq!(receipt.snapshot().first_denial, None);
        assert!(!receipt.snapshot().caught_panic);

        let report = source.report.semantics();
        assert!(report.progress().certificates().is_empty());
        let payload = report
            .payload_receipt_with_observation_v1(hard(), None)
            .unwrap();
        assert!(payload.is_exact());
        let full = report_validation_stage_resource_upper_bound_v1(
            h,
            payload,
            0,
            K::SemanticRefinement,
            p.input_census_v1(),
            hard(),
        )
        .unwrap();
        let local = L::new(full.work_upper_bound(), full.peak_storage_upper_bound());
        let previous = v.last_stage_resource_upper_bound_v1();
        let manager_before = m.resource_upper_bound();
        let mut first_denial = None;

        // Roots, numerical certificates and progress certificates in each of
        // the three actual clone owners use the existing failure injector.
        for successful_allocations in 0..9 {
            let expected = receipt
                .snapshot()
                .committed
                .checked_then_retain(full, RV)
                .unwrap();
            // Check before arming the injector, so no earlier quota can leave
            // an unconsumed failure for another operation.
            hard()
                .require(RV, h.checked_then_retain(expected, RV).unwrap())
                .unwrap();
            let phase = receipt.phase(RV, 0).unwrap();
            let observer = phase.observer(&Ok);
            fail_after(successful_allocations);
            let result = v.record_with_observation_v1(
                ProductionAnalysisReportEndpointV1 {
                    context: &c,
                    function: &f,
                },
                p.last_checkpoint().unwrap(),
                report,
                h,
                local,
                (&mut m, Some(&observer)),
            );
            assert_eq!(
                result,
                Err(ProductionAnalysisReportValidationErrorV1::ResourceLimit {
                    producing_pass: Some(K::SemanticRefinement),
                    resource: "report payload allocation",
                }),
                "allocation checkpoint {successful_allocations}"
            );
            drop(phase);

            let state = receipt.snapshot();
            let denial = state.first_denial.expect("typed allocation denial");
            let first = *first_denial.get_or_insert(denial);
            assert_eq!(denial, first);
            assert_eq!(denial.phase, RV);
            assert_eq!(denial.resource, "report payload allocation");
            assert!(!state.caught_panic);
            // Failure keeps all of the accepted peak, without owner transfer.
            let held = B::checked_phase(
                RV,
                expected.work_upper_bound(),
                expected.peak_storage_upper_bound(),
                0,
            )
            .unwrap();
            assert_eq!(state.committed, held);
            assert_eq!(state.current, held);
            assert_eq!(receipt.complete(), Err(Failure::Denied(first)));
            assert_eq!((v.next, v.stages.len()), (8, 8));
            assert_eq!(v.last_stage_resource_upper_bound_v1(), previous);
            assert_eq!(m.resource_upper_bound(), manager_before);
        }

        // A different, real census refusal cannot replace the allocation denial.
        let prior = receipt.snapshot();
        let phase = receipt.phase(RV, 0).unwrap();
        let denied = v.record_with_observation_v1(
            ProductionAnalysisReportEndpointV1 {
                context: &c,
                function: &f,
            },
            p.last_checkpoint().unwrap(),
            report,
            h,
            L::new(0, full.peak_storage_upper_bound()),
            (&mut m, Some(&phase.observer(&Ok))),
        );
        assert_eq!(
            denied,
            Err(ProductionAnalysisReportValidationErrorV1::ResourceLimit {
                producing_pass: Some(K::SemanticRefinement),
                resource: "work upper bound",
            })
        );
        drop(phase);
        assert_eq!(receipt.snapshot(), prior);
        assert_eq!((v.next, v.stages.len()), (8, 8));
        assert_eq!(v.last_stage_resource_upper_bound_v1(), previous);
        assert_eq!(m.resource_upper_bound(), manager_before);

        // Retrying may advance validation, but cannot erase the first denial.
        let expected = receipt
            .snapshot()
            .committed
            .checked_then_retain(full, RV)
            .unwrap();
        hard()
            .require(RV, h.checked_then_retain(expected, RV).unwrap())
            .unwrap();
        let phase = receipt.phase(RV, 0).unwrap();
        let observer = phase.observer(&Ok);
        v.record_with_observation_v1(
            ProductionAnalysisReportEndpointV1 {
                context: &c,
                function: &f,
            },
            p.last_checkpoint().unwrap(),
            report,
            h,
            local,
            (&mut m, Some(&observer)),
        )
        .unwrap();
        m.admit_retained_resource_upper_bound(RV, full).unwrap();
        phase.commit(full).unwrap();
        assert_eq!((v.next, v.stages.len()), (9, 9));
        assert_eq!(v.last_stage_resource_upper_bound_v1(), Some(full));
        assert_eq!(
            m.resource_upper_bound(),
            manager_before.checked_then_retain(full, RV).unwrap()
        );
        assert_eq!(receipt.snapshot().committed, expected);
        assert_eq!(receipt.snapshot().first_denial, first_denial);
        assert!(!receipt.snapshot().caught_panic);
        assert_eq!(
            receipt.complete(),
            Err(Failure::Denied(first_denial.unwrap()))
        );
        drop(source);
    }
}

mod observed_validation_custody_tests {
    use super::*;
    use crate::production_analysis::{
        pliron_pipeline::invocation_receipt_v1::InvocationReceiptV1 as Receipt,
        pliron_tensor_layout::preflight_tensor_layout_resource_upper_bound_v1,
    };
    type Bound = ProductionAnalysisResourceUpperBoundV1;
    type Limits = ProductionAnalysisResourceLimitsV1;
    const PHASE: ProductionAnalysisResourcePhaseV1 =
        ProductionAnalysisResourcePhaseV1::ReportValidation;

    #[test]
    fn foreign_checkpoint_is_nonquota_after_exact_validation_admission() {
        let hard = Limits::production_hard_ceiling();
        let mut context = setup();
        let function = valid_function(&mut context, "observed_foreign_checkpoint");
        let new_session = || {
            begin_production_pliron_pass_contract_session_v1(
                LivePlironStructuralIdentityProviderV1::new(&context, &function),
            )
            .unwrap()
        };
        let mut foreign = new_session();
        let report = foreign
            .run_contiguous_pass(KernelCheckPassKindV1::TensorLayout, || {
                Ok::<_, ()>(run_pliron_tensor_layout_check_v1(&context, &function))
            })
            .unwrap()
            .unwrap();
        assert!(report.is_clean());
        let authenticated = new_session();
        let census = authenticated.input_census_v1();
        let producer = preflight_tensor_layout_resource_upper_bound_v1(census, None, hard).unwrap();
        let payload = report.payload_receipt_v1(hard).unwrap();
        payload
            .payload_bounds(report.pass(), producer.retained_storage_upper_bound())
            .unwrap();
        let validation = report_validation_stage_resource_upper_bound_v1(
            producer,
            payload,
            0,
            report.pass(),
            census,
            hard,
        )
        .unwrap();
        let mut manager = PlironAnalysisManagerV1::new_with_resource_contract(
            &function,
            census,
            authenticated.initial_identity_resource_upper_bound_v1(),
            authenticated
                .lineage_identity_resource_upper_bound_v1()
                .retained_storage_upper_bound(),
            hard,
        )
        .unwrap();
        let manager_before = manager.resource_upper_bound();
        assert!(manager_before.work_upper_bound() > 0);
        let endpoint = || ProductionAnalysisReportEndpointV1 {
            context: &context,
            function: &function,
        };
        let mut ordinary = begin_production_analysis_report_validation_with_resource_limits_v1(
            &context,
            &function,
            None,
            authenticated.validation_handle(),
            census,
            hard,
        )
        .unwrap();
        let setup = ordinary.setup_resource_upper_bound_v1();
        let expected = ordinary.record_with_resource_limits_v1(
            endpoint(),
            foreign.last_checkpoint().unwrap(),
            &report,
            producer,
            hard,
            &mut manager,
        );
        assert_eq!(
            expected,
            Err(
                ProductionAnalysisReportValidationErrorV1::CounterfeitOrCrossSessionSeal {
                    position: 0
                }
            )
        );
        assert_eq!(manager.resource_upper_bound(), manager_before);
        drop(ordinary);

        // Fixture owners remain live outside this validation-only measurement.
        let floor = manager_before
            .checked_then_retain(foreign.initial_identity_resource_upper_bound_v1(), PHASE)
            .unwrap()
            .checked_then_retain(
                foreign.last_checkpoint_resource_upper_bound_v1().unwrap(),
                PHASE,
            )
            .unwrap()
            .checked_then_retain(producer, PHASE)
            .unwrap();
        let total = setup.checked_then_retain(validation, PHASE).unwrap();
        let with_floor = floor.checked_then_retain(total, PHASE).unwrap();
        let limits = Limits::new(
            with_floor.work_upper_bound(),
            with_floor.peak_storage_upper_bound(),
        );
        let mut receipt = Receipt::new(floor, limits).unwrap();
        let phase = receipt.phase(PHASE, 0).unwrap();
        let mut observed = begin_production_analysis_report_validation_with_observation_v1(
            &context,
            &function,
            None,
            authenticated.validation_handle(),
            census,
            hard,
            Some(&phase.observer(&Ok)),
        )
        .unwrap();
        assert_eq!(observed.setup_resource_upper_bound_v1(), setup);
        phase.commit(setup).unwrap();
        let phase = receipt.phase(PHASE, 0).unwrap();
        let actual = observed.record_with_observation_v1(
            endpoint(),
            foreign.last_checkpoint().unwrap(),
            &report,
            producer,
            hard,
            (&mut manager, Some(&phase.observer(&Ok))),
        );
        drop(phase);
        assert_eq!(actual, expected);
        assert_eq!(manager.resource_upper_bound(), manager_before);
        assert_eq!(observed.next, 0);
        assert!(observed.stages.is_empty());
        assert_eq!(observed.last_stage_resource_upper_bound_v1(), None);
        let held = Bound::checked_phase(
            PHASE,
            total.work_upper_bound(),
            total.peak_storage_upper_bound(),
            0,
        )
        .unwrap();
        let state = receipt.snapshot();
        assert_eq!(state.committed, held);
        assert_eq!(state.first_denial, None);
        assert!(!state.caught_panic);
        assert_eq!(receipt.complete(), Ok(held));
    }
}

mod observed_semantic_payload_census_tests {
    use super::*;
    use crate::production_analysis::pliron_pipeline::invocation_receipt_v1::{
        InvocationReceiptFailureV1 as Failure, InvocationReceiptV1 as Receipt,
    };
    type Bound = ProductionAnalysisResourceUpperBoundV1;
    type Limits = ProductionAnalysisResourceLimitsV1;
    const PHASE: ProductionAnalysisResourcePhaseV1 =
        ProductionAnalysisResourcePhaseV1::ReportValidation;

    fn check_census_limits(global_refusal: bool) {
        let hard = Limits::production_hard_ceiling();
        let report = PlironSemanticRefinementReportV1::validation_payload_test_report_v1();
        let certificates = report.progress().certificates().len();
        assert_eq!(certificates, 1);
        assert_eq!(report.numerical_certificates().len(), 1);
        assert_eq!(report.typed_root_commitments().len(), 2);
        let header = 48;
        let whole = header + 11 * certificates;
        assert_eq!(whole, 59);
        let baseline = report.payload_receipt_v1(hard).unwrap();
        assert!(baseline.is_exact());

        // The payload remains live, carrying its storage and completed census.
        let floor =
            Bound::checked_phase(PHASE, whole, baseline.owner_storage_for_test_v1(), 0).unwrap();
        assert!(floor.retained_storage_upper_bound() > 0);
        for room in [header - 1, whole - 1, whole] {
            let local = if global_refusal {
                hard
            } else {
                Limits::new(room, hard.max_peak_storage())
            };
            let global = Limits::new(
                floor.work_upper_bound() + if global_refusal { room } else { whole },
                floor.peak_storage_upper_bound(),
            );
            let ordinary = report.payload_receipt_v1(local);
            let mut receipt = Receipt::new(floor, global).unwrap();
            let phase = receipt.phase(PHASE, 0).unwrap();
            let actual =
                report.payload_receipt_with_observation_v1(local, Some(&phase.observer(&Ok)));
            drop(phase);
            if global_refusal {
                assert_eq!(ordinary, Ok(baseline));
            } else {
                assert_eq!(actual, ordinary);
            }
            let admitted = if room < header {
                0
            } else if room < whole {
                header
            } else {
                whole
            };
            let expected = Bound::checked_phase(PHASE, admitted, 0, 0).unwrap();
            let denial = (room < whole).then_some(ProductionAnalysisResourceLimitV1 {
                phase: PHASE,
                resource: "work upper bound",
            });
            let state = receipt.snapshot();
            assert_eq!(state.current, expected);
            assert_eq!(state.committed, expected);
            assert_eq!(state.first_denial, denial);
            assert!(!state.caught_panic);
            assert_eq!(actual, denial.map_or(Ok(baseline), Err));
            assert_eq!(
                receipt.complete(),
                denial.map_or(Ok(expected), |error| Err(Failure::Denied(error)))
            );
            global
                .require(PHASE, floor.checked_then_retain(expected, PHASE).unwrap())
                .unwrap();
        }
    }

    #[test]
    fn semantic_payload_local_header_and_whole_census_limits() {
        check_census_limits(false);
    }

    #[test]
    fn semantic_payload_global_floor_and_single_census_charge() {
        check_census_limits(true);
    }
}
