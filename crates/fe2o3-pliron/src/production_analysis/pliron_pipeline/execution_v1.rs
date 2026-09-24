#[allow(clippy::result_large_err)]
fn run_shared_production_checks_v1<'a>(
    context: &'a Context,
    function: &'a FuncOp,
    atomic_target: Option<&PlironAtomicTargetContextV1>,
    target_contract: Option<&PlironLaunchContractV1>,
    resource_limits: ProductionAnalysisResourceLimitsV1,
    invocation: (
        PipelineFamilyV1<'a>,
        Option<&mut invocation_receipt_v1::InvocationReceiptV1>,
    ),
    #[cfg(test)] capture_fault: Option<CaptureFaultV1>,
) -> Result<PipelineOutcomeV1, PipelineErrorV1> {
    let (family, receipt) = invocation;
    let run = |receipt: Option<&mut invocation_receipt_v1::InvocationReceiptV1>| {
        run_shared_production_checks_inner_v1(
            context,
            function,
            atomic_target,
            target_contract,
            resource_limits,
            (family, receipt),
            #[cfg(test)]
            capture_fault,
        )
    };
    match receipt {
        None => run(None),
        Some(receipt) => receipt.observe_unwind(|receipt| run(Some(receipt))),
    }
}

#[allow(clippy::result_large_err)]
fn run_shared_production_checks_inner_v1<'a>(
    context: &'a Context,
    function: &'a FuncOp,
    atomic_target: Option<&PlironAtomicTargetContextV1>,
    target_contract: Option<&PlironLaunchContractV1>,
    resource_limits: ProductionAnalysisResourceLimitsV1,
    invocation: (
        PipelineFamilyV1<'a>,
        Option<&mut invocation_receipt_v1::InvocationReceiptV1>,
    ),
    #[cfg(test)] capture_fault: Option<CaptureFaultV1>,
) -> Result<PipelineOutcomeV1, PipelineErrorV1> {
    let (family, mut receipt) = invocation;
    maybe_panic_for_test_v1();
    let (mut preservation, mut analyses) = with_invocation_phase_v1(
        receipt.as_deref_mut(),
        ProductionAnalysisResourcePhaseV1::StructuralIdentity,
        0,
        |observer| {
            let provider = LivePlironStructuralIdentityProviderV1::new(context, function);
            let preservation = begin_observed_pass_session_v1(provider, resource_limits, observer)
                .map_err(|error| {
                    if target_contract.is_some()
                        && matches!(
                            &error,
                            PlironPassPreservationErrorV1::IdentityUnavailable { source_code, .. }
                                if *source_code == "FE2O3-PRESERVE-000"
                        )
                    {
                        ProductionPlironPreloweringErrorV2::TargetContract(
                            PlironLaunchContractCheckErrorV1::structural_prerequisite_v1(),
                        )
                    } else {
                        ProductionPlironPreloweringErrorV2::Preservation(error)
                    }
                })?;
            let input_census = preservation.input_census_v1();
            let analyses = PlironAnalysisManagerV1::new_with_resource_contract(
                function,
                input_census,
                preservation.initial_identity_resource_upper_bound_v1(),
                preservation
                    .lineage_identity_resource_upper_bound_v1()
                    .retained_storage_upper_bound(),
                resource_limits,
            )
            .map_err(|error| observed_pipeline_resource_error_v1(observer, error))?;
            let bound = analyses.resource_upper_bound();
            if let Some(observer) = observer {
                observer.require(
                    resource_limits,
                    ProductionAnalysisResourcePhaseV1::FunctionInventory,
                    Ok(bound),
                )?;
            }
            Ok(((preservation, analyses), Some(bound)))
        },
    )?;
    let input_census = preservation.input_census_v1();
    with_invocation_phase_v1(
        receipt.as_deref_mut(),
        ProductionAnalysisResourcePhaseV1::StructuralIdentity,
        0,
        |observer| {
            family
                .authenticate_initial(context, function, &preservation, &mut analyses, observer)
                .map(|transfer| ((), transfer))
        },
    )?;
    let target_contract = with_invocation_phase_v1(
        receipt.as_deref_mut(),
        ProductionAnalysisResourcePhaseV1::LaunchContract,
        0,
        |observer| {
            let phase = ProductionAnalysisResourcePhaseV1::LaunchContract;
            let bound = target_contract
                .map(|_| {
                    let limits = observed_remaining_resource_limits_v1(&analyses, phase, observer)?;
                    preflight_launch_contract_resource_upper_bound_v1(input_census, limits)
                        .map_err(|error| observed_pipeline_resource_error_v1(observer, error))
                })
                .transpose()?;
            if let Some(bound) = bound {
                retain_cache_with_observation_v1(&mut analyses, phase, bound, observer)?;
            }
            let report = target_contract
                .map(|target| {
                    require_observed_launch_v1(context, function, target, &mut analyses, observer)
                })
                .transpose()
                .map_err(ProductionPlironPreloweringErrorV2::TargetContract)?;
            Ok((report, bound))
        },
    )?;

    let sparse_upper_bound = with_invocation_phase_v1(
        receipt.as_deref_mut(),
        ProductionAnalysisResourcePhaseV1::SparseIndex,
        0,
        |observer| {
            let phase = ProductionAnalysisResourcePhaseV1::SparseIndex;
            let limits = observed_remaining_resource_limits_v1(&analyses, phase, observer)?;
            let bound =
                preflight_observed_sparse_v1(context, function, input_census, limits, observer)?;
            retain_cache_with_observation_v1(&mut analyses, phase, bound, observer)?;
            analyses.prepare_sparse_indices(context, function);
            if let Some(observer) = observer {
                if let Err(failure) = analyses.function_inventory() {
                    cache_observation_v1::observe_inventory_failure_v1(observer, &failure);
                }
                if let Err(failure) = analyses.sparse_indices() {
                    cache_observation_v1::observe_sparse_failure_v1(observer, &failure);
                }
            }
            Ok((bound, Some(bound)))
        },
    )?;

    let presburger_upper_bound = with_invocation_phase_v1(
        receipt.as_deref_mut(),
        ProductionAnalysisResourcePhaseV1::Presburger,
        0,
        |observer| {
            let phase = ProductionAnalysisResourcePhaseV1::Presburger;
            let limits = observed_remaining_resource_limits_v1(&analyses, phase, observer)?;
            let bound = preflight_presburger_resource_upper_bound_v1(
                analyses.sparse_indices().ok(),
                limits,
            )
            .map_err(|error| observed_pipeline_resource_error_v1(observer, error))?;
            retain_cache_with_observation_v1(&mut analyses, phase, bound, observer)?;
            analyses.prepare_presburger(context, function);
            if let Some(observer) = observer
                && let Err(failure) = analyses.presburger()
            {
                cache_observation_v1::observe_sparse_failure_v1(observer, &failure);
            }
            Ok((bound, Some(bound)))
        },
    )?;

    let (execution_layout_upper_bound, execution_layout) = with_invocation_phase_v1(
        receipt.as_deref_mut(),
        ProductionAnalysisResourcePhaseV1::LaunchContract,
        0,
        |observer| {
            let phase = ProductionAnalysisResourcePhaseV1::LaunchContract;
            let limits = observed_remaining_resource_limits_v1(&analyses, phase, observer)?;
            let bound = preflight_observed_layout_v1(input_census, limits, observer)?;
            retain_cache_with_observation_v1(&mut analyses, phase, bound, observer)?;
            analyses.prepare_execution_layout(context, function);
            let layout = analyses.execution_layout().map_err(|failure| {
                if let Some(observer) = observer {
                    cache_observation_v1::observe_layout_failure_v1(observer, &failure);
                }
                ProductionPlironPreloweringErrorV2::ResourceLimit {
                    phase,
                    producing_pass: None,
                    resource: "execution-layout analysis",
                }
            })?;
            Ok(((bound, layout), Some(bound)))
        },
    )?;

    let (trace_preflight, trace_admission) = with_invocation_phase_v1(
        receipt.as_deref_mut(),
        ProductionAnalysisResourcePhaseV1::InvocationTrace,
        0,
        |observer| {
            let phase = ProductionAnalysisResourcePhaseV1::InvocationTrace;
            let limits = observed_remaining_resource_limits_v1(&analyses, phase, observer)?;
            let preflight = preflight_observed_trace_v1(
                context,
                analyses.function_inventory().ok(),
                input_census,
                analyses.sparse_indices().ok(),
                execution_layout,
                limits,
                observer,
            )?;
            let bound = preflight.attempt_upper_bound();
            retain_cache_with_observation_v1(&mut analyses, phase, bound, observer)?;
            analyses.prepare_exact_trace(context, function);
            let traces = analyses.exact_trace();
            if let Some(observer) = observer
                && let Err(failure) = &traces
            {
                cache_observation_v1::observe_trace_failure_v1(observer, failure);
            }
            let admission = preflight
                .exact_admission(traces)
                .map_err(|error| observed_pipeline_resource_error_v1(observer, error))?;
            Ok(((preflight, admission), Some(bound)))
        },
    )?;

    let mut report_validation = with_invocation_phase_v1(
        receipt.as_deref_mut(),
        ProductionAnalysisResourcePhaseV1::ReportValidation,
        0,
        |observer| {
            let phase = ProductionAnalysisResourcePhaseV1::ReportValidation;
            let limits = observed_remaining_resource_limits_v1(&analyses, phase, observer)?;
            let (validation, transfer) = ValidationFamilyV1::begin(
                family,
                (context, function),
                atomic_target,
                preservation.validation_handle(),
                input_census,
                limits,
                (&mut analyses, observer),
            )?;
            retain_cache_resource_upper_bound_v1(
                &mut analyses,
                phase,
                validation.setup_resource_upper_bound_v1(),
            )
            .map_err(|error| observed_pipeline_resource_error_v1(observer, error))?;
            Ok((validation, transfer))
        },
    )?;
    let (tensor_layout, _) = run_preflight_production_stage_v1(
        (context, function),
        &mut analyses,
        &mut preservation,
        (&mut report_validation, receipt.as_deref_mut()),
        (
            ProductionAnalysisResourcePhaseV1::TensorLayout,
            |analyses, observer| {
                let phase = ProductionAnalysisResourcePhaseV1::TensorLayout;
                let limits = observed_remaining_resource_limits_v1(analyses, phase, observer)?;
                let bound = preflight_tensor_layout_resource_upper_bound_v1(
                    input_census,
                    trace_admission,
                    limits,
                )
                .map_err(|error| observed_pipeline_resource_error_v1(observer, error))?;
                Ok(PreparedProductionStageV1 {
                    stage: ProductionAnalysisStageV1 {
                        pass: KernelCheckPassKindV1::TensorLayout,
                        producing_phase: phase,
                        producing_phase_upper_bound: bound,
                    },
                    input: (),
                    prefix: ProductionAnalysisResourceUpperBoundV1::default(),
                })
            },
        ),
        |analyses, observer| {
            let report = require_observed_tensor_v1(context, function, analyses, observer);
            maybe_mutate_for_test_v1(context, function);
            report
        },
    )?;
    let tensor_layout = tensor_layout.map_err(ProductionPlironPreloweringErrorV2::TensorLayout)?;
    let (bounds, ranked_bounds_upper_bound) = run_memory_bounds_stage_v1(
        (context, function),
        &mut analyses,
        &mut preservation,
        &mut report_validation,
        receipt.as_deref_mut(),
        family,
        input_census,
        #[cfg(test)]
        capture_fault,
    )?;
    let (atomics, _) = run_preflight_production_stage_v1(
        (context, function),
        &mut analyses,
        &mut preservation,
        (&mut report_validation, receipt.as_deref_mut()),
        (
            ProductionAnalysisResourcePhaseV1::AtomicLegality,
            |analyses, observer| {
                let phase = ProductionAnalysisResourcePhaseV1::AtomicLegality;
                let limits = observed_remaining_resource_limits_v1(analyses, phase, observer)?;
                let bound = preflight_atomic_legality_resource_upper_bound_v1(
                    input_census,
                    atomic_target,
                    limits,
                )
                .map_err(|error| observed_pipeline_resource_error_v1(observer, error))?;
                Ok(PreparedProductionStageV1 {
                    stage: ProductionAnalysisStageV1 {
                        pass: KernelCheckPassKindV1::AtomicLegality,
                        producing_phase: phase,
                        producing_phase_upper_bound: bound,
                    },
                    input: (),
                    prefix: ProductionAnalysisResourceUpperBoundV1::default(),
                })
            },
        ),
        |analyses, observer| {
            require_observed_atomics_v1(context, function, atomic_target, analyses, observer)
        },
    )?;
    let atomics = atomics.map_err(ProductionPlironPreloweringErrorV2::Atomic)?;
    let provenance_upper_bound = with_invocation_phase_v1(
        receipt.as_deref_mut(),
        ProductionAnalysisResourcePhaseV1::ProvenanceAlias,
        0,
        |observer| {
            let phase = ProductionAnalysisResourcePhaseV1::ProvenanceAlias;
            let limits = observed_remaining_resource_limits_v1(&analyses, phase, observer)?;
            let bound = preflight_provenance_alias_resource_upper_bound_v1(input_census, limits)
                .map_err(|error| observed_pipeline_resource_error_v1(observer, error))?;
            retain_cache_with_observation_v1(&mut analyses, phase, bound, observer)?;
            analyses.prepare_provenance_alias(context, function);
            if let Some(observer) = observer
                && let Err(failure) = analyses.provenance_alias()
            {
                cache_observation_v1::observe_provenance_failure_v1(observer, failure);
            }
            Ok((bound, Some(bound)))
        },
    )?;
    let (race, race_upper_bound) = run_preflight_production_stage_v1(
        (context, function),
        &mut analyses,
        &mut preservation,
        (&mut report_validation, receipt.as_deref_mut()),
        (
            ProductionAnalysisResourcePhaseV1::RaceFreedom,
            |analyses, observer| {
                let phase = ProductionAnalysisResourcePhaseV1::RaceFreedom;
                let sparse = analyses.sparse_indices().map_err(|failure| {
                    if let Some(observer) = observer {
                        cache_observation_v1::observe_sparse_failure_v1(observer, &failure);
                    }
                    ProductionPlironPreloweringErrorV2::ResourceLimit {
                        phase: ProductionAnalysisResourcePhaseV1::SparseIndex,
                        producing_pass: None,
                        resource: "sparse-index analysis",
                    }
                })?;
                let limits = observed_remaining_resource_limits_v1(analyses, phase, observer)?;
                let bound = preflight_observed_race_v1(
                    context,
                    function,
                    analyses.function_inventory().ok(),
                    input_census,
                    sparse,
                    execution_layout,
                    crate::production_analysis::pliron_race::RaceResourceAdmissionV1 {
                        limits,
                        observer,
                    },
                )
                .map_err(|error| observed_pipeline_resource_error_v1(observer, error))?;
                Ok(PreparedProductionStageV1 {
                    stage: ProductionAnalysisStageV1 {
                        pass: KernelCheckPassKindV1::RaceFreedom,
                        producing_phase: phase,
                        producing_phase_upper_bound: bound,
                    },
                    input: (),
                    prefix: ProductionAnalysisResourceUpperBoundV1::default(),
                })
            },
        ),
        |analyses, observer| require_observed_race_v1(context, function, analyses, observer),
    )?;
    let race = race.map_err(ProductionPlironPreloweringErrorV2::Race)?;
    let prepare_ownership =
        |analyses: &mut PlironAnalysisManagerV1, observer: PipelineObservationV1<'_, '_, '_>| {
            family.prepare_ownership_stage_v1(
                analyses,
                input_census,
                trace_admission,
                (ranked_bounds_upper_bound, race_upper_bound),
                observer,
            )
        };
    let (ownership, ownership_upper_bound) = match family {
        PipelineFamilyV1::Ordinary => {
            let (report, bound) = run_preflight_and_record_production_stage_v1(
                (context, function),
                &mut analyses,
                &mut preservation,
                (&mut report_validation, receipt.as_deref_mut()),
                (
                    ProductionAnalysisResourcePhaseV1::HierarchicalOwnership,
                    prepare_ownership,
                ),
                |preservation, pass, limits, analyses, _, (producer, checkpoint, _)| {
                    preservation.run_contiguous_pass_with_observation_v1(
                        pass,
                        limits,
                        || require_observed_ownership_v1(context, function, analyses, producer),
                        checkpoint,
                    )
                },
            )?;
            (
                Some(report.map_err(ProductionPlironPreloweringErrorV2::Ownership)?),
                bound,
            )
        }
        PipelineFamilyV1::Conditional(input) => {
            let bounds = borrow_conditional_bounds_v1(
                &report_validation,
                &mut analyses,
                receipt.as_deref_mut(),
            )?;
            let (report, stage) = prepare_and_invoke_production_stage_v1(
                &mut analyses,
                &mut preservation,
                receipt.as_deref_mut(),
                (
                    ProductionAnalysisResourcePhaseV1::HierarchicalOwnership,
                    |analyses, observer| {
                        let prepared = prepare_ownership(analyses, observer)?;
                        Ok(PreparedProductionStageV1 {
                            stage: prepared.stage,
                            input: prepared.input.ok_or(PipelineErrorV1::ConditionalInput)?,
                            prefix: prepared.prefix,
                        })
                    },
                ),
                |preservation, pass, limits, analyses, rows, (producer, checkpoint, additional)| {
                    preservation.run_contiguous_pass_with_observation_v1(
                        pass,
                        limits,
                        || {
                            let report = conditional_ownership::run_preadmitted_with_bounds_v1(
                                conditional_ownership::ExecutionInputV1 {
                                    ctx: context,
                                    function,
                                    census: input.census(),
                                    expected_epoch: input.epoch(),
                                    recipe: input.recipe(),
                                    occurrences: input.occurrences(),
                                },
                                analyses,
                                conditional_ownership::RaceSourceV1::SameInvocation(&race),
                                &conditional_ownership::BoundsSourceV1::SameInvocation(bounds),
                                rows.rows,
                                (producer, additional),
                            );
                            if report.is_clean() {
                                Ok(report)
                            } else {
                                Err(report)
                            }
                        },
                        checkpoint,
                    )
                },
            )?;
            let report = report.map_err(PipelineErrorV1::ConditionalOwnership)?;
            let checkpoint = preservation
                .last_checkpoint()
                .map_err(ProductionPlironPreloweringErrorV2::Preservation)?;
            let ValidationFamilyV1::Conditional(validation) = &mut report_validation else {
                return Err(PipelineErrorV1::ConditionalInput);
            };
            let packet = ProducedConditionalStageV1 {
                subject: input.pending_subject(),
                manager_address: &analyses as *const PlironAnalysisManagerV1 as usize,
                checkpoint,
                payload: ProducedConditionalPayloadV1::Ownership(report),
            };
            with_invocation_phase_v1(
                receipt.as_deref_mut(),
                ProductionAnalysisResourcePhaseV1::ReportValidation,
                0,
                |observer| {
                    #[cfg(test)]
                    let transfer = record_capture_test_v1(
                        validation,
                        packet,
                        &mut analyses,
                        &preservation,
                        &race,
                        capture_fault,
                        observer,
                    )?;
                    #[cfg(not(test))]
                    let transfer = {
                        let bound = validation.record_produced_with_observation_v1(
                            packet,
                            &mut analyses,
                            observer,
                        )?;
                        observer.map(|_| bound)
                    };
                    Ok(((), transfer))
                },
            )?;
            (None, stage.producing_phase_upper_bound)
        }
    };
    let simt_protocol_upper_bound = with_invocation_phase_v1(
        receipt.as_deref_mut(),
        ProductionAnalysisResourcePhaseV1::SimtProtocol,
        0,
        |observer| {
            let phase = ProductionAnalysisResourcePhaseV1::SimtProtocol;
            let limits = observed_remaining_resource_limits_v1(&analyses, phase, observer)?;
            let bound = preflight_simt_protocol_resource_upper_bound_v1(trace_admission, limits)
                .map_err(|error| observed_pipeline_resource_error_v1(observer, error))?;
            retain_cache_with_observation_v1(&mut analyses, phase, bound, observer)?;
            analyses.prepare_simt_protocol(context, function);
            if let Some(observer) = observer {
                cache_observation_v1::observe_simt_cache_v1(observer, &analyses.simt_protocol());
            }
            Ok((bound, Some(bound)))
        },
    )?;
    // This pure dependency preflight originally precedes the barrier probe.
    let pipeline_protocol_upper_bound = with_invocation_phase_v1(
        receipt.as_deref_mut(),
        ProductionAnalysisResourcePhaseV1::PipelineProtocol,
        0,
        |observer| {
            let phase = ProductionAnalysisResourcePhaseV1::PipelineProtocol;
            let limits = observed_remaining_resource_limits_v1(&analyses, phase, observer)?;
            let bound = preflight_pipeline_protocol_resource_upper_bound_v1(input_census, limits)
                .map_err(|error| observed_pipeline_resource_error_v1(observer, error))?;
            Ok((bound, None))
        },
    )?;
    let (barriers, _) = run_preflight_and_record_production_stage_v1(
        (context, function),
        &mut analyses,
        &mut preservation,
        (&mut report_validation, receipt.as_deref_mut()),
        (
            ProductionAnalysisResourcePhaseV1::BarrierConvergence,
            |analyses, observer| {
                let ((bound, progress), probe) = preflight_barrier_stage_admission_v1(
                    context,
                    analyses,
                    &input_census,
                    trace_admission,
                    pipeline_protocol_upper_bound,
                    observer,
                )?;
                Ok(PreparedProductionStageV1 {
                    stage: ProductionAnalysisStageV1 {
                        pass: KernelCheckPassKindV1::BarrierConvergence,
                        producing_phase: ProductionAnalysisResourcePhaseV1::BarrierConvergence,
                        producing_phase_upper_bound: bound,
                    },
                    input: progress,
                    prefix: probe.unwrap_or_default(),
                })
            },
        ),
        |preservation, _, limits, analyses, progress, (producer, checkpoint, _)| {
            preservation.run_scoped_barrier_with_observation_v1(
                limits,
                |input| {
                    with_scoped_producer_observation_v1(producer, |observer| {
                        require_observed_barrier_v1(input, analyses, progress, observer)
                    })
                },
                checkpoint,
            )
        },
    )?;
    let barriers = barriers.map_err(ProductionPlironPreloweringErrorV2::Barrier)?;
    let (pipeline_protocol, _) = run_preflight_production_stage_v1(
        (context, function),
        &mut analyses,
        &mut preservation,
        (&mut report_validation, receipt.as_deref_mut()),
        (
            ProductionAnalysisResourcePhaseV1::PipelineProtocol,
            |_, _| {
                Ok(PreparedProductionStageV1 {
                    stage: ProductionAnalysisStageV1 {
                        pass: KernelCheckPassKindV1::PipelineProtocol,
                        producing_phase: ProductionAnalysisResourcePhaseV1::PipelineProtocol,
                        producing_phase_upper_bound: pipeline_protocol_upper_bound,
                    },
                    input: (),
                    prefix: ProductionAnalysisResourceUpperBoundV1::default(),
                })
            },
        ),
        |analyses, observer| require_observed_protocol_v1(context, function, analyses, observer),
    )?;
    let pipeline_protocol =
        pipeline_protocol.map_err(ProductionPlironPreloweringErrorV2::PipelineProtocol)?;
    let memory_order_admission = with_invocation_phase_v1(
        receipt.as_deref_mut(),
        ProductionAnalysisResourcePhaseV1::MemoryOrder,
        0,
        |observer| {
            let phase = ProductionAnalysisResourcePhaseV1::MemoryOrder;
            let limits = observed_remaining_resource_limits_v1(&analyses, phase, observer)?;
            let admission = preflight_memory_order_attempt_resource_upper_bound_v1(
                input_census,
                trace_admission,
                limits,
            )
            .map_err(|error| observed_pipeline_resource_error_v1(observer, error))?;
            let bound = admission.upper_bound();
            retain_cache_with_observation_v1(&mut analyses, phase, bound, observer)?;
            analyses.prepare_memory_order(context, function);
            if let Some(observer) = observer
                && let Err(failure) = analyses.memory_order()
            {
                cache_observation_v1::observe_memory_order_failure_v1(observer, &failure);
            }
            Ok((admission, Some(bound)))
        },
    )?;
    let (workgroup, _) = run_preflight_production_stage_v1(
        (context, function),
        &mut analyses,
        &mut preservation,
        (&mut report_validation, receipt.as_deref_mut()),
        (
            ProductionAnalysisResourcePhaseV1::WorkgroupMemory,
            |analyses, observer| {
                let phase = ProductionAnalysisResourcePhaseV1::WorkgroupMemory;
                let issues = analyses
                    .memory_order()
                    .map(|memory_order| memory_order.issues());
                let limits = observed_remaining_resource_limits_v1(analyses, phase, observer)?;
                let local = preflight_observed_workgroup_v1(
                    input_census,
                    memory_order_admission,
                    issues,
                    limits,
                    observer,
                )
                .map_err(|error| observed_pipeline_resource_error_v1(observer, error))?;
                let bound = local
                    .checked_with_nested_sequence_discard(&[pipeline_protocol_upper_bound], phase)
                    .map_err(|error| observed_pipeline_resource_error_v1(observer, error))?;
                Ok(PreparedProductionStageV1 {
                    stage: ProductionAnalysisStageV1 {
                        pass: KernelCheckPassKindV1::WorkgroupMemory,
                        producing_phase: phase,
                        producing_phase_upper_bound: bound,
                    },
                    input: (),
                    prefix: ProductionAnalysisResourceUpperBoundV1::default(),
                })
            },
        ),
        |analyses, observer| require_observed_workgroup_v1(context, function, analyses, observer),
    )?;
    let workgroup = workgroup.map_err(ProductionPlironPreloweringErrorV2::Workgroup)?;
    let prepare_semantic =
        |analyses: &mut PlironAnalysisManagerV1, observer: PipelineObservationV1<'_, '_, '_>| {
            family.prepare_semantic_stage_v1(
                analyses,
                input_census,
                ownership_upper_bound,
                observer,
            )
        };
    let semantics = match family {
        PipelineFamilyV1::Ordinary => {
            let (report, _) = run_preflight_and_record_production_stage_v1(
                (context, function),
                &mut analyses,
                &mut preservation,
                (&mut report_validation, receipt.as_deref_mut()),
                (
                    ProductionAnalysisResourcePhaseV1::SemanticRefinement,
                    prepare_semantic,
                ),
                |preservation, _, limits, analyses, (), (producer, checkpoint, _)| {
                    preservation.run_scoped_semantic_refinement_with_observation_v1(
                        limits,
                        |input| {
                            with_scoped_producer_observation_v1(producer, |observer| {
                                require_observed_semantic_v1(input, analyses, observer)
                                    .map(|result| result.map_err(Box::new))
                            })
                        },
                        checkpoint,
                    )
                },
            )?;
            Some(report.map_err(|error| ProductionPlironPreloweringErrorV2::Semantic(*error))?)
        }
        PipelineFamilyV1::Conditional(input) => {
            let bounds = borrow_conditional_bounds_v1(
                &report_validation,
                &mut analyses,
                receipt.as_deref_mut(),
            )?;
            let (report, _) = prepare_and_invoke_production_stage_v1(
                &mut analyses,
                &mut preservation,
                receipt.as_deref_mut(),
                (
                    ProductionAnalysisResourcePhaseV1::SemanticRefinement,
                    |analyses, observer| {
                        let prepared = prepare_semantic(analyses, observer)?;
                        prepare_conditional_semantic_input_v1(
                            input,
                            analyses,
                            prepared.stage,
                            observer,
                        )
                    },
                ),
                |preservation,
                 _,
                 limits,
                 analyses,
                 prepared,
                 (producer, checkpoint, additional)| {
                    preservation.run_scoped_semantic_refinement_with_observation_v1(
                        limits,
                        |input| {
                            with_scoped_producer_observation_v1(producer, |observer| {
                                let (query, admitted) = additional.unzip();
                                with_scoped_producer_observation_v1(query, |query| {
                                    conditional_semantic::require_with_scoped_bounds_v1(
                                        input,
                                        analyses,
                                        prepared,
                                        &conditional_ownership::BoundsSourceV1::SameInvocation(
                                            bounds,
                                        ),
                                        (observer, query.zip(admitted)),
                                    )
                                })
                            })
                        },
                        checkpoint,
                    )
                },
            )?;
            let report = report.map_err(PipelineErrorV1::ConditionalSemantic)?;
            let checkpoint = preservation
                .last_checkpoint()
                .map_err(ProductionPlironPreloweringErrorV2::Preservation)?;
            let ValidationFamilyV1::Conditional(validation) = &mut report_validation else {
                return Err(PipelineErrorV1::ConditionalInput);
            };
            let packet = ProducedConditionalStageV1 {
                subject: input.pending_subject(),
                manager_address: &analyses as *const PlironAnalysisManagerV1 as usize,
                checkpoint,
                payload: ProducedConditionalPayloadV1::Semantic(report),
            };
            with_invocation_phase_v1(
                receipt.as_deref_mut(),
                ProductionAnalysisResourcePhaseV1::ReportValidation,
                0,
                |observer| {
                    #[cfg(test)]
                    let transfer = record_capture_test_v1(
                        validation,
                        packet,
                        &mut analyses,
                        &preservation,
                        &race,
                        capture_fault,
                        observer,
                    )?;
                    #[cfg(not(test))]
                    let transfer = {
                        let bound = validation.record_produced_with_observation_v1(
                            packet,
                            &mut analyses,
                            observer,
                        )?;
                        observer.map(|_| bound)
                    };
                    Ok(((), transfer))
                },
            )?;
            None
        }
    };
    let replaced_identity_storage = preservation
        .lineage_identity_resource_upper_bound_v1()
        .retained_storage_upper_bound();
    let preservation = with_invocation_phase_v1(
        receipt.as_deref_mut(),
        ProductionAnalysisResourcePhaseV1::PassPreservation,
        replaced_identity_storage,
        |observer| {
            let limits = analyses
                .remaining_identity_replacement_resource_limits_v1(replaced_identity_storage)
                .map_err(|error| observed_pipeline_resource_error_v1(observer, error))?;
            let report = preservation
                .finish_with_observation_v1(limits.output, observer)
                .map_err(ProductionPlironPreloweringErrorV2::Preservation)?;
            let bound = report.resource_upper_bound_v1();
            analyses
                .resource_contract_replace_retained_v1(
                    ProductionAnalysisResourcePhaseV1::PassPreservation,
                    replaced_identity_storage,
                    bound,
                    report.output_identity().canonical_len(),
                )
                .map_err(|error| observed_pipeline_resource_error_v1(observer, error))?;
            Ok((report, Some(bound)))
        },
    )?;
    let reports = match report_validation {
        ValidationFamilyV1::Ordinary(validation) => {
            let report_validation = with_invocation_phase_v1(
                receipt.as_deref_mut(),
                ProductionAnalysisResourcePhaseV1::ReportValidation,
                0,
                |observer| {
                    validation
                        .finish_validation_with_observation_v1(&preservation, observer)
                        .map(|report| (report, None))
                        .map_err(|error| {
                            ProductionPlironPreloweringErrorV2::ReportValidation(error).into()
                        })
                },
            )?;
            let (Some(bounds), Some(ownership), Some(semantics)) = (bounds, ownership, semantics)
            else {
                return Err(PipelineErrorV1::ConditionalInput);
            };
            let report = ProductionPlironPreloweringReportV2 {
                target_contract,
                tensor_layout,
                bounds,
                atomics,
                race,
                ownership,
                barriers,
                pipeline_protocol,
                workgroup,
                semantics,
                preservation,
                report_validation,
            };
            PipelineReportsV1::Ordinary(report)
        }
        ValidationFamilyV1::Conditional(validation) => {
            if bounds.is_some() || ownership.is_some() || semantics.is_some() {
                return Err(PipelineErrorV1::ConditionalInput);
            }
            let validation = with_invocation_phase_v1(
                receipt.as_deref_mut(),
                ProductionAnalysisResourcePhaseV1::ReportValidation,
                0,
                |observer| {
                    let (report, bound) = validation.finish_with_observation_v1(
                        &preservation,
                        &mut analyses,
                        observer,
                    )?;
                    Ok((report, observer.map(|_| bound)))
                },
            )?;
            PipelineReportsV1::Conditional(ConditionalPipelineReportV1 {
                target_contract,
                tensor_layout,
                atomics,
                race,
                barriers,
                pipeline_protocol,
                workgroup,
                preservation,
                validation,
            })
        }
    };
    let resource_upper_bound = finish_exclusive_analysis_caches_with_observation_v1(
        analyses,
        ExclusiveAnalysisCacheReservationsV1 {
            sparse: sparse_upper_bound,
            presburger: presburger_upper_bound,
            execution_layout: execution_layout_upper_bound,
            invocation_trace: trace_preflight.attempt_upper_bound(),
            provenance: provenance_upper_bound,
            simt: simt_protocol_upper_bound,
            memory_order: memory_order_admission.upper_bound(),
        },
        receipt,
    )
    .map_err(resource_upper_bound_error_v1)?;
    Ok(match reports {
        PipelineReportsV1::Ordinary(report) => {
            PipelineOutcomeV1::Ordinary(ProductionPlironPreloweringOutcomeV1 {
                report,
                resource_upper_bound,
            })
        }
        PipelineReportsV1::Conditional(report) => {
            PipelineOutcomeV1::Conditional(ConditionalPipelineOutcomeV1 {
                report,
                resource_upper_bound,
            })
        }
    })
}
