#[allow(clippy::result_large_err)]
#[cfg(test)]
fn preflight_barrier_stage_v1(
    context: &Context,
    analyses: &mut PlironAnalysisManagerV1,
    input_census: &ProductionAnalysisInputCensusV1,
    trace_admission: Option<crate::production_analysis::pliron_invocation_trace::ProductionInvocationTraceResourceAdmissionV1>,
    pipeline_protocol_upper_bound: ProductionAnalysisResourceUpperBoundV1,
) -> Result<(ProductionAnalysisResourceUpperBoundV1, bool), ProductionPlironPreloweringErrorV2> {
    preflight_barrier_stage_with_observation_v1(
        context,
        analyses,
        input_census,
        trace_admission,
        pipeline_protocol_upper_bound,
        None,
    )
}

#[allow(clippy::result_large_err)]
#[cfg(test)]
fn preflight_barrier_stage_with_observation_v1(
    context: &Context,
    analyses: &mut PlironAnalysisManagerV1,
    input_census: &ProductionAnalysisInputCensusV1,
    trace_admission: Option<crate::production_analysis::pliron_invocation_trace::ProductionInvocationTraceResourceAdmissionV1>,
    pipeline_protocol_upper_bound: ProductionAnalysisResourceUpperBoundV1,
    observer: Option<&invocation_receipt_v1::InvocationObserverV1<'_, '_>>,
) -> Result<(ProductionAnalysisResourceUpperBoundV1, bool), ProductionPlironPreloweringErrorV2> {
    preflight_barrier_stage_admission_v1(
        context,
        analyses,
        input_census,
        trace_admission,
        pipeline_protocol_upper_bound,
        observer,
    )
    .map(|(result, _)| result)
}

#[allow(clippy::result_large_err)]
fn preflight_barrier_stage_admission_v1(
    context: &Context,
    analyses: &mut PlironAnalysisManagerV1,
    input_census: &ProductionAnalysisInputCensusV1,
    trace_admission: Option<crate::production_analysis::pliron_invocation_trace::ProductionInvocationTraceResourceAdmissionV1>,
    pipeline_protocol_upper_bound: ProductionAnalysisResourceUpperBoundV1,
    observer: PipelineObservationV1<'_, '_, '_>,
) -> Result<
    (
        (ProductionAnalysisResourceUpperBoundV1, bool),
        Option<ProductionAnalysisResourceUpperBoundV1>,
    ),
    ProductionPlironPreloweringErrorV2,
> {
    use crate::production_analysis::ProductionAnalysisResourcePhaseV1 as Phase;

    invocation_receipt_v1::observe_resource_preflight_v1(observer, |observer| {
        let probe = if let Some(observer) = observer {
            Some(observer.require(
                remaining_resource_limits_v1(analyses, Phase::BarrierConvergence)?,
                Phase::BarrierConvergence,
                crate::production_analysis::pliron_barrier::barrier_progress_probe_bound_v1(
                    input_census,
                ),
            )?)
        } else {
            None
        };
        // The observer mirrors this one real probe admission; it does not rerun
        // the probe or add another manager charge.
        let barrier_progress_needed =
            admit_barrier_progress_probe_v1(context, analyses, input_census)?;
        let barrier_progress_upper_bound = if barrier_progress_needed {
            Some(preflight_scoped_progress_resource_upper_bound_v1(
                *input_census,
                remaining_resource_limits_v1(analyses, Phase::Progress)?,
            )?)
        } else {
            None
        };
        let barrier_local_upper_bound = preflight_barrier_convergence_resource_upper_bound_v1(
            *input_census,
            trace_admission,
            remaining_resource_limits_v1(analyses, Phase::BarrierConvergence)?,
        )?;
        let barrier_upper_bound = compose_barrier_dependencies_v1(
            barrier_local_upper_bound,
            pipeline_protocol_upper_bound,
            barrier_progress_upper_bound,
        )?;
        Ok(((barrier_upper_bound, barrier_progress_needed), probe))
    })
    .map_err(resource_upper_bound_error_v1)
}

#[cfg(test)]
mod observed_barrier_preflight_tests {
    use super::*;
    use crate::production_analysis::pliron_pass_contract::begin_production_pliron_pass_contract_session_with_observation_v1;
    use crate::production_analysis::{
        ProductionAnalysisResourceLimitV1 as Limit, ProductionAnalysisResourceLimitsV1 as Limits,
        ProductionAnalysisResourcePhaseV1 as Phase,
    };
    use invocation_receipt_v1::{
        InvocationReceiptFailureV1 as ReceiptFailure, InvocationReceiptV1 as Receipt,
    };
    use pliron::{builtin::types::FunctionType, dialect::DialectName, op::Op};
    use std::panic::{AssertUnwindSafe, catch_unwind};

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum Case {
        Exact,
        WorkShort,
        PeakShort,
        BadCensus,
        BorrowPanic,
    }

    fn fixture(
        barrier_loop: bool,
    ) -> (
        Context,
        FuncOp,
        pliron::context::Ptr<pliron::operation::Operation>,
    ) {
        let mut context = Context::new();
        dialect_kernel::register_dialect(
            &mut context,
            &DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
        )
        .unwrap();
        let signature = FunctionType::get(&context, vec![], vec![]);
        let function = FuncOp::new(
            &mut context,
            "observed_barrier_preflight".try_into().unwrap(),
            signature,
        );
        let entry = function.get_entry_block(&context);
        let operation = if barrier_loop {
            dialect_gpu::register_dialect(&mut context).unwrap();
            dialect_gpu::BarrierOp::new(
                &mut context,
                dialect_gpu::HierarchyAttr::Workgroup,
                dialect_gpu::MemoryScopeAttr::Workgroup,
                dialect_gpu::AddressSpaceAttr::Workgroup,
                dialect_gpu::MemoryOrderAttr::AcquireRelease,
            )
            .get_operation()
            .insert_at_back(entry, &context);
            dialect_kernel::BranchOp::new(&mut context, entry).get_operation()
        } else {
            dialect_kernel::ReturnOp::new(&mut context).get_operation()
        };
        operation.insert_at_back(entry, &context);
        (context, function, operation)
    }

    #[test]
    fn real_barrier_preflight_preserves_charges_floor_and_failure_prefix() {
        let hard = Limits::production_hard_ceiling();
        let phase_kind = Phase::BarrierConvergence;
        let (context, function, operation) = fixture(false);

        let mut setup_receipt = Receipt::new(Default::default(), hard).unwrap();
        let setup_phase = setup_receipt.phase(Phase::StructuralIdentity, 0).unwrap();
        let preservation = begin_production_pliron_pass_contract_session_with_observation_v1(
            LivePlironStructuralIdentityProviderV1::new(&context, &function),
            hard,
            Some(&setup_phase.observer(&Ok)),
        )
        .unwrap();
        let census = preservation.input_census_v1();
        let make_manager = || {
            PlironAnalysisManagerV1::new_with_resource_contract(
                &function,
                census,
                preservation.initial_identity_resource_upper_bound_v1(),
                preservation
                    .lineage_identity_resource_upper_bound_v1()
                    .retained_storage_upper_bound(),
                hard,
            )
            .unwrap()
        };
        let mut seed = make_manager();
        let setup = seed.resource_upper_bound();
        setup_phase
            .observer(&Ok)
            .require(hard, Phase::FunctionInventory, Ok(setup))
            .unwrap();
        seed.prepare_function_inventory(&context, &function);
        assert!(!seed.function_inventory().unwrap().operations().is_empty());
        // Keep the accepted setup peak reservation; this is not an owner release.
        drop(setup_phase);
        let floor = setup_receipt.complete().unwrap();
        assert_eq!(
            floor.retained_storage_upper_bound(),
            floor.peak_storage_upper_bound()
        );
        assert!(floor.work_upper_bound() > 0);
        drop(seed);
        let protocol = preflight_pipeline_protocol_resource_upper_bound_v1(census, hard).unwrap();

        for case in [
            Case::Exact,
            Case::WorkShort,
            Case::PeakShort,
            Case::BadCensus,
            Case::BorrowPanic,
        ] {
            let mut input = census;
            if case == Case::BadCensus {
                input.operations = 0;
            }
            let h =
                crate::production_analysis::pliron_barrier::barrier_progress_probe_bound_v1(&input)
                    .unwrap();
            let mut ordinary = make_manager();
            ordinary.prepare_function_inventory(&context, &function);
            let held = (case == Case::BorrowPanic).then(|| operation.deref_mut(&context));
            let mut baseline_result = None;
            let baseline = catch_unwind(AssertUnwindSafe(|| {
                baseline_result = Some(preflight_barrier_stage_v1(
                    &context,
                    &mut ordinary,
                    &input,
                    None,
                    protocol,
                ));
            }));
            let baseline = match baseline {
                Ok(()) => Ok(baseline_result.unwrap()),
                Err(error) => Err(error),
            };
            drop(held);
            let ordinary_after = ordinary.resource_upper_bound();
            drop(ordinary);
            let total = floor.checked_then_retain(h, phase_kind).unwrap();
            let limits = Limits::new(
                total.work_upper_bound() - usize::from(case == Case::WorkShort),
                total.peak_storage_upper_bound() - usize::from(case == Case::PeakShort),
            );
            let mut receipt = Receipt::new(floor, limits).unwrap();
            let phase = receipt.phase(phase_kind, 0).unwrap();
            let mut observed = make_manager();
            observed.prepare_function_inventory(&context, &function);
            let denied = matches!(case, Case::WorkShort | Case::PeakShort);
            // A denied admission must not reach this conflicting IR borrow.
            let held = (denied || case == Case::BorrowPanic).then(|| operation.deref_mut(&context));
            let mut observed_result = None;
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                observed_result = Some(preflight_barrier_stage_with_observation_v1(
                    &context,
                    &mut observed,
                    &input,
                    None,
                    protocol,
                    Some(&phase.observer(&Ok)),
                ));
            }));
            let outcome = match outcome {
                Ok(()) => Ok(observed_result.unwrap()),
                Err(error) => Err(error),
            };
            drop(held);
            drop(phase);
            let state = receipt.snapshot();
            if denied {
                assert_eq!(observed.resource_upper_bound(), setup);
                assert_eq!(state.committed, Default::default());
            } else {
                assert_eq!(observed.resource_upper_bound(), ordinary_after);
                assert_eq!(
                    ordinary_after,
                    setup.checked_then_retain(h, phase_kind).unwrap()
                );
                assert_eq!(state.committed.work_upper_bound(), h.work_upper_bound());
                assert_eq!(
                    state.committed.peak_storage_upper_bound(),
                    h.peak_storage_upper_bound()
                );
                assert_eq!(
                    state.committed.retained_storage_upper_bound(),
                    h.peak_storage_upper_bound()
                );
            }
            if case == Case::BorrowPanic {
                assert!(baseline.is_err() && outcome.is_err());
                assert_eq!(state.first_denial, None);
                assert!(state.caught_panic);
                assert_eq!(receipt.complete(), Err(ReceiptFailure::CaughtPanic));
            } else if case == Case::Exact {
                let expected = baseline.unwrap().unwrap();
                assert!(!expected.1);
                assert_eq!(outcome.unwrap().unwrap(), expected);
                assert_eq!(state.first_denial, None);
                assert!(!state.caught_panic);
                assert_eq!(receipt.complete(), Ok(state.committed));
            } else {
                let error = Limit {
                    phase: phase_kind,
                    resource: match case {
                        Case::WorkShort => "work upper bound",
                        Case::PeakShort => "peak storage upper bound",
                        Case::BadCensus => "barrier dependency census mismatch",
                        _ => unreachable!(),
                    },
                };
                let public_error = resource_upper_bound_error_v1(error);
                if case == Case::BadCensus {
                    assert_eq!(baseline.unwrap(), Err(public_error.clone()));
                }
                assert_eq!(outcome.unwrap(), Err(public_error));
                assert_eq!(state.first_denial, Some(error));
                assert!(!state.caught_panic);
                assert_eq!(receipt.complete(), Err(ReceiptFailure::Denied(error)));
            }
        }
        drop(preservation);
    }

    #[test]
    fn real_barrier_loop_progress_preflight_retains_the_accepted_probe() {
        let hard = Limits::production_hard_ceiling();
        let phase_kind = Phase::BarrierConvergence;
        let (context, function, _) = fixture(true);
        let preservation = crate::production_analysis::pliron_pass_contract::begin_production_pliron_pass_contract_session_with_resource_limits_v1(
            LivePlironStructuralIdentityProviderV1::new(&context, &function),
            hard,
        )
        .unwrap();
        let census = preservation.input_census_v1();
        let make_manager = |limits| {
            PlironAnalysisManagerV1::new_with_resource_contract(
                &function,
                census,
                preservation.initial_identity_resource_upper_bound_v1(),
                preservation
                    .lineage_identity_resource_upper_bound_v1()
                    .retained_storage_upper_bound(),
                limits,
            )
            .unwrap()
        };
        let setup = make_manager(hard).resource_upper_bound();
        let probe =
            crate::production_analysis::pliron_barrier::barrier_progress_probe_bound_v1(&census)
                .unwrap();
        let accepted = setup.checked_then_retain(probe, phase_kind).unwrap();
        let progress = preflight_scoped_progress_resource_upper_bound_v1(census, hard).unwrap();
        let protocol = preflight_pipeline_protocol_resource_upper_bound_v1(census, hard).unwrap();

        for short in [false, true] {
            let local = if short {
                Limits::new(
                    accepted.work_upper_bound() + progress.work_upper_bound() - 1,
                    hard.max_peak_storage(),
                )
            } else {
                hard
            };
            let mut ordinary = make_manager(local);
            ordinary.prepare_function_inventory(&context, &function);
            let expected =
                preflight_barrier_stage_v1(&context, &mut ordinary, &census, None, protocol);
            assert_eq!(ordinary.resource_upper_bound(), accepted);

            let mut observed = make_manager(local);
            observed.prepare_function_inventory(&context, &function);
            let mut receipt = Receipt::new(setup, hard).unwrap();
            let phase = receipt.phase(phase_kind, 0).unwrap();
            let actual = preflight_barrier_stage_with_observation_v1(
                &context,
                &mut observed,
                &census,
                None,
                protocol,
                Some(&phase.observer(&Ok)),
            );
            drop(phase);
            let state = receipt.snapshot();
            assert_eq!(actual, expected);
            assert_eq!(observed.resource_upper_bound(), accepted);
            assert_eq!(state.committed.work_upper_bound(), probe.work_upper_bound());
            assert_eq!(
                state.committed.retained_storage_upper_bound(),
                probe.peak_storage_upper_bound()
            );
            assert_eq!(
                state.committed.peak_storage_upper_bound(),
                probe.peak_storage_upper_bound()
            );
            assert!(!state.caught_panic);
            if short {
                let error = Limit {
                    phase: Phase::Progress,
                    resource: "work upper bound",
                };
                assert_eq!(actual, Err(resource_upper_bound_error_v1(error)));
                assert_eq!(state.first_denial, Some(error));
                assert_eq!(receipt.complete(), Err(ReceiptFailure::Denied(error)));
            } else {
                assert!(actual.unwrap().1);
                assert_eq!(state.first_denial, None);
                assert_eq!(receipt.complete(), Ok(state.committed));
            }
        }
    }
}
