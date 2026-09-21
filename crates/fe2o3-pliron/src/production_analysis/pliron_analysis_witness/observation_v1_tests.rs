mod observed_witness {
    use super::*;
    use crate::production_analysis as pa;
    use pa::pliron_ir_identity::LivePlironStructuralIdentityProviderV1 as Provider;
    use pa::pliron_pass_contract::{
        PlironPassContractSessionV1 as Session,
        begin_production_pliron_pass_contract_session_with_resource_limits_v1 as begin,
    };
    use pa::pliron_pipeline::invocation_receipt_v1::{
        InvocationReceiptFailureV1 as Failure, InvocationReceiptV1 as Receipt,
    };
    use std::panic::{AssertUnwindSafe, catch_unwind};

    type B = ProductionAnalysisResourceUpperBoundV1;
    type L = ProductionAnalysisResourceLimitsV1;
    type P = ProductionAnalysisResourcePhaseV1;
    type M = PlironAnalysisManagerV1;
    const RV: P = P::ReportValidation;

    fn hard() -> L {
        L::production_hard_ceiling()
    }

    fn seed<'a>(c: &'a Context, f: &'a FuncOp) -> (Session<Provider<'a>>, M, RankedBoundsReportV1) {
        let p = begin(Provider::new(c, f), hard()).unwrap();
        let census = p.input_census_v1();
        let mut m = M::new_with_resource_contract(
            f,
            census,
            p.initial_identity_resource_upper_bound_v1(),
            p.lineage_identity_resource_upper_bound_v1()
                .retained_storage_upper_bound(),
            hard(),
        )
        .unwrap();
        m.prepare_function_inventory(c, f);
        assert!(m.function_inventory().is_ok());
        let b = pa::pliron_sparse_index::preflight_sparse_index_resource_upper_bound_v1(
            c,
            f,
            census,
            m.remaining_resource_limits(P::SparseIndex).unwrap(),
        )
        .unwrap();
        m.admit_retained_resource_upper_bound(P::SparseIndex, b)
            .unwrap();
        m.prepare_sparse_indices(c, f);
        assert!(m.sparse_indices().is_ok());
        let b = pa::preflight_presburger_resource_upper_bound_v1(
            m.sparse_indices().ok(),
            m.remaining_resource_limits(P::Presburger).unwrap(),
        )
        .unwrap();
        m.admit_retained_resource_upper_bound(P::Presburger, b)
            .unwrap();
        m.prepare_presburger(c, f);
        assert!(m.presburger().is_ok());
        let b = pa::pliron_ranked_bounds::preflight_ranked_bounds_resource_upper_bound_v1(
            census,
            m.remaining_resource_limits(P::MemoryBounds).unwrap(),
        )
        .unwrap();
        m.admit_retained_resource_upper_bound(P::MemoryBounds, b)
            .unwrap();
        let report =
            pa::pliron_ranked_bounds::run_pliron_ranked_bounds_check_with_analyses_v1(c, f, &mut m);
        (p, m, report)
    }

    fn reserve(m: &mut M) -> (Receipt, B) {
        let h = preflight_production_analysis_witness_resource_upper_bound_v1(
            KernelCheckPassKindV1::MemoryBounds,
            m.input_census().unwrap(),
            m.remaining_resource_limits(RV).unwrap(),
        )
        .unwrap();
        let receipt = Receipt::new(m.resource_upper_bound(), hard()).unwrap();
        m.admit_retained_resource_upper_bound(RV, h).unwrap();
        (receipt, h)
    }

    #[test]
    fn actual_witness_launch_work_and_nonquota_controls() {
        assert_eq!(16 * 65_536, MAX_BOUNDS_WITNESS_EVALUATION_STEPS_V1);
        for (lanes, accesses, index, extra_block, reason, quota) in [
            (65_536, 1, 0, false, "", None),
            (65_536, 16, 0, false, "", None),
            (
                65_537,
                1,
                0,
                false,
                "needs 65537 invocations",
                Some("bounds witness invocation limit"),
            ),
            (
                65_536,
                17,
                0,
                false,
                "deterministic work cap",
                Some("raw-index evaluation exceeded its deterministic work cap"),
            ),
            (4, 1, 0, true, "exhaustive CFG path domains", None),
            (4, 1, 16, false, "only a Clean bounds report", None),
        ] {
            let mut source =
                constant_layout_source("observed_witness", [lanes, 1, 1], &[], index, accesses);
            if extra_block {
                source = source.replace(
                    "\n}\n",
                    "\n  ^unreachable_block():\n    kernel.return () [] []: <() -> ()>\n}\n",
                );
            }
            let mut c = setup();
            let f = parse_source(&mut c, &source);
            let (_p, mut m, report) = seed(&c, &f);
            if !extra_block && index == 0 {
                assert_eq!(report.status(), KernelCheckStatusV1::Clean);
            }
            let mut ordinary = None;
            for observed in [false, true] {
                let (mut receipt, h) = reserve(&mut m);
                let before = m.resource_upper_bound();
                let phase = receipt.phase(RV, 0).unwrap();
                let observer = phase.observer(&Ok);
                observer.require(hard(), RV, Ok(h)).unwrap();
                let actual = match build_bounds_presburger_witness_with_observation_v1(
                    &c,
                    &f,
                    &report,
                    &mut m,
                    observed.then_some(&observer),
                )
                .unwrap()
                {
                    SupportedWitnessBuildV1::Complete(w) => Ok(w),
                    SupportedWitnessBuildV1::Incomplete(s) => Err(s),
                };
                if reason.is_empty() {
                    let w = actual.as_ref().unwrap();
                    assert_eq!(w.obligations.len(), accesses);
                    for o in &w.obligations {
                        assert_layout_obligation(o, [lanes, 1, 1], lanes);
                    }
                } else {
                    assert!(actual.as_ref().unwrap_err().contains(reason));
                }
                if index == 16 {
                    let inventory = m.function_inventory_handle().unwrap();
                    let site = inventory
                        .operations()
                        .iter()
                        .find(|s| {
                            Operation::get_op_dyn(s.pointer(), &c)
                                .downcast_ref::<RankedAccessOp>()
                                .is_some()
                        })
                        .unwrap();
                    let op = Operation::get_op_dyn(site.pointer(), &c);
                    let value = op.downcast_ref::<RankedAccessOp>().unwrap().indices(&c)[0];
                    let mut steps = 0;
                    let result = exhaustively_check_raw_index(
                        &c,
                        value,
                        &[4, 1, 1],
                        16,
                        &mut steps,
                        raw_index_stack_frame_upper_bound_v1(inventory.operations().len()).unwrap(),
                        (
                            RawBoundsReplaySiteV1 {
                                operation: site.operation(),
                                dimension: 0,
                            },
                            observed.then_some(&observer),
                        ),
                    );
                    assert!(
                        matches!(result, Err(RawBoundsReplayFailureV1::Counterexample(
                        ProductionAnalysisWitnessValidationErrorV1::BoundsCounterexample {
                            block: 0, operation, dimension: 0, invocation, index: 16, extent: 16,
                        }
                    )) if operation == site.operation() && invocation == vec![0, 0, 0])
                    );
                    assert_eq!(steps, 1);
                }
                if observed {
                    assert_eq!(ordinary.as_ref(), Some(&actual));
                } else {
                    ordinary = Some(actual);
                }
                assert_eq!(m.resource_upper_bound(), before);
                drop(phase);
                let state = receipt.snapshot();
                assert!(!state.caught_panic);
                assert_eq!(
                    state.first_denial,
                    quota.filter(|_| observed).map(|resource| {
                        ProductionAnalysisResourceLimitV1 {
                            phase: RV,
                            resource,
                        }
                    })
                );
                assert_eq!(
                    state.committed,
                    B::checked_phase(RV, h.work_upper_bound(), h.peak_storage_upper_bound(), 0)
                        .unwrap()
                );
                assert_eq!(state.current, state.committed);
                assert_eq!(
                    receipt.complete(),
                    state
                        .first_denial
                        .map_or(Ok(state.committed), |error| Err(Failure::Denied(error)),)
                );
            }
        }
    }

    #[test]
    fn actual_witness_quota_survives_cached_ir_borrow_panic() {
        let mut c = setup();
        let f = parse_source(
            &mut c,
            &constant_layout_source("observed_witness_panic", [65_537, 1, 1], &[], 0, 1),
        );
        let (_p, mut m, report) = seed(&c, &f);
        let (mut receipt, h) = reserve(&mut m);
        {
            let phase = receipt.phase(RV, 0).unwrap();
            let o = phase.observer(&Ok);
            o.require(hard(), RV, Ok(h)).unwrap();
            assert!(
                matches!(build_bounds_presburger_witness_with_observation_v1(
                &c, &f, &report, &mut m, Some(&o),
            ).unwrap(), SupportedWitnessBuildV1::Incomplete(s) if s.contains("needs 65537 invocations"))
            );
        }
        let first = ProductionAnalysisResourceLimitV1 {
            phase: RV,
            resource: "bounds witness invocation limit",
        };
        assert_eq!(receipt.snapshot().first_denial, Some(first));
        assert!(!receipt.snapshot().caught_panic);
        m.admit_retained_resource_upper_bound(RV, h).unwrap();
        let before = m.resource_upper_bound();
        let pointer = m.function_inventory().unwrap().operations()[0].pointer();
        let expected = receipt
            .snapshot()
            .committed
            .checked_then_retain(h, RV)
            .unwrap();
        {
            let phase = receipt.phase(RV, 0).unwrap();
            let o = phase.observer(&Ok);
            o.require(hard(), RV, Ok(h)).unwrap();
            let _held = pointer.deref_mut(&c);
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    let _ = build_bounds_presburger_witness_with_observation_v1(
                        &c,
                        &f,
                        &report,
                        &mut m,
                        Some(&o),
                    );
                }))
                .is_err()
            );
        }
        assert_eq!(m.resource_upper_bound(), before);
        assert_eq!(receipt.snapshot().first_denial, Some(first));
        assert!(receipt.snapshot().caught_panic);
        let held = B::checked_phase(
            RV,
            expected.work_upper_bound(),
            expected.peak_storage_upper_bound(),
            0,
        )
        .unwrap();
        assert_eq!(receipt.snapshot().committed, held);
        assert_eq!(receipt.snapshot().current, held);
        assert_eq!(receipt.complete(), Err(Failure::Denied(first)));
    }
}
