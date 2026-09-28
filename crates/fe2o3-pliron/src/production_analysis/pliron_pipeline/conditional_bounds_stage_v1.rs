#[allow(clippy::result_large_err, clippy::too_many_arguments)]
fn run_memory_bounds_stage_v1(
    endpoint: (&Context, &FuncOp),
    analyses: &mut PlironAnalysisManagerV1,
    preservation: &mut PlironPassContractSessionV1<LivePlironStructuralIdentityProviderV1<'_>>,
    validation: &mut ValidationFamilyV1<'_>,
    mut receipt: Option<&mut invocation_receipt_v1::InvocationReceiptV1>,
    family: PipelineFamilyV1<'_>,
    census: ProductionAnalysisInputCensusV1,
    #[cfg(test)] capture_fault: Option<CaptureFaultV1>,
) -> Result<
    (
        Option<RankedBoundsReportV1>,
        ProductionAnalysisResourceUpperBoundV1,
    ),
    PipelineErrorV1,
> {
    let (context, function) = endpoint;
    let phase = ProductionAnalysisResourcePhaseV1::MemoryBounds;
    let prepare = |analyses: &mut PlironAnalysisManagerV1,
                   observer: PipelineObservationV1<'_, '_, '_>| {
        let limits = observed_remaining_resource_limits_v1(analyses, phase, observer)?;
        let bound = match family {
            PipelineFamilyV1::Ordinary
            | PipelineFamilyV1::LifecycleV18(_)
            | PipelineFamilyV1::CanonicalPrivate(_)
            | PipelineFamilyV1::CanonicalPrivateV18(_) => {
                preflight_ranked_bounds_resource_upper_bound_v1(census, limits)
            }
            PipelineFamilyV1::Conditional(input) => conditional_bounds::preflight_v1(
                census,
                input.conditional_reads().map_or(0, <[_]>::len),
                limits,
            ),
        }
        .map_err(|error| observed_pipeline_resource_error_v1(observer, error))?;
        let bound = if let PipelineFamilyV1::LifecycleV18(input) = family {
            let work = input
                .identity_lookup_work()
                .ok_or(ProductionAnalysisResourceLimitV1 {
                    phase,
                    resource: "V18 lifecycle bounds occurrence work",
                })
                .map_err(|error| observed_pipeline_resource_error_v1(observer, error))?;
            let extra = ProductionAnalysisResourceUpperBoundV1::checked_phase(phase, work, 0, 0)
                .map_err(|error| observed_pipeline_resource_error_v1(observer, error))?;
            bound
                .checked_then_retain(extra, phase)
                .map_err(|error| observed_pipeline_resource_error_v1(observer, error))?
        } else {
            bound
        };
        Ok(PreparedProductionStageV1 {
            stage: ProductionAnalysisStageV1 {
                pass: KernelCheckPassKindV1::MemoryBounds,
                producing_phase: phase,
                producing_phase_upper_bound: bound,
            },
            input: (),
            prefix: ProductionAnalysisResourceUpperBoundV1::default(),
        })
    };
    match family {
        PipelineFamilyV1::Ordinary
        | PipelineFamilyV1::LifecycleV18(_)
        | PipelineFamilyV1::CanonicalPrivate(_)
        | PipelineFamilyV1::CanonicalPrivateV18(_) => {
            let (report, bound) = run_preflight_production_stage_v1(
                endpoint,
                analyses,
                preservation,
                (validation, receipt),
                (phase, prepare),
                |analyses, observer| {
                    match family {
            PipelineFamilyV1::CanonicalPrivate(input) =>
                crate::production_analysis::pliron_ranked_bounds::require_canonical_private_bounds_v1(
                    input, analyses, observer,
                ),
            PipelineFamilyV1::CanonicalPrivateV18(input) =>
                crate::production_analysis::pliron_ranked_bounds::require_canonical_private_bounds_v1(
                    input, analyses, observer,
                ),
            PipelineFamilyV1::LifecycleV18(input) =>
                crate::production_analysis::pliron_ranked_bounds::require_canonical_lifecycle_bounds_v18(
                    input, analyses, observer,
                ),
            _ => require_observed_bounds_v1(context, function, analyses, observer),
        }
                },
            )?;
            Ok((
                Some(report.map_err(ProductionPlironPreloweringErrorV2::Bounds)?),
                bound,
            ))
        }
        PipelineFamilyV1::Conditional(input) => {
            let (report, stage) = prepare_and_invoke_production_stage_v1(
                analyses,
                preservation,
                receipt.as_deref_mut(),
                (phase, prepare),
                |preservation, pass, limits, analyses, (), (producer, checkpoint, additional)| {
                    preservation.run_contiguous_pass_with_observation_v1(
                        pass,
                        limits,
                        || {
                            conditional_bounds::run_preadmitted_with_observation_v1(
                                input,
                                analyses,
                                (producer, additional),
                            )
                        },
                        checkpoint,
                    )
                },
            )?;
            let report = report.map_err(PipelineErrorV1::ConditionalBounds)?;
            let checkpoint = preservation
                .last_checkpoint()
                .map_err(ProductionPlironPreloweringErrorV2::Preservation)?;
            let ValidationFamilyV1::Conditional(validation) = validation else {
                return Err(PipelineErrorV1::ConditionalInput);
            };
            let packet = ProducedConditionalStageV1 {
                subject: input.pending_subject(),
                manager_address: analyses as *const PlironAnalysisManagerV1 as usize,
                checkpoint,
                payload: ProducedConditionalPayloadV1::Bounds(report),
            };
            #[cfg(test)]
            let packet = {
                let mut packet = packet;
                match capture_fault {
                    Some(CaptureFaultV1::MissingBounds) => {
                        return Ok((None, stage.producing_phase_upper_bound));
                    }
                    Some(CaptureFaultV1::ForeignBoundsManager) => packet.manager_address = 0,
                    Some(CaptureFaultV1::ForeignBoundsSubject(subject)) => packet.subject = subject,
                    _ => {}
                }
                packet
            };
            with_invocation_phase_v1(
                receipt,
                ProductionAnalysisResourcePhaseV1::ReportValidation,
                0,
                |observer| {
                    let bound = validation
                        .record_produced_with_observation_v1(packet, analyses, observer)?;
                    Ok(((), observer.map(|_| bound)))
                },
            )?;
            #[cfg(all(test, feature = "internal-proof-staging"))]
            if let Some(CaptureFaultV1::BoundsBorrowResources(case)) = capture_fault {
                conditional_bounds_resources_v1::check_borrow(input, validation, analyses, case);
                // No real fixed slot can produce this test-completion sentinel.
                return Err(conditional_validation::ErrorV1::Family {
                    position: usize::MAX,
                }
                .into());
            }
            Ok((None, stage.producing_phase_upper_bound))
        }
    }
}

#[allow(clippy::result_large_err)]
fn borrow_conditional_bounds_v1<'a>(
    validation: &'a ValidationFamilyV1<'_>,
    analyses: &mut PlironAnalysisManagerV1,
    receipt: Option<&mut invocation_receipt_v1::InvocationReceiptV1>,
) -> Result<conditional_validation::BoundsDependencyV1<'a>, PipelineErrorV1> {
    let ValidationFamilyV1::Conditional(validation) = validation else {
        return Err(PipelineErrorV1::ConditionalInput);
    };
    with_invocation_phase_v1(
        receipt,
        ProductionAnalysisResourcePhaseV1::ReportValidation,
        0,
        |observer| {
            let (dependency, bound) =
                validation.borrow_bounds_with_observation_v1(analyses, observer)?;
            Ok((dependency, observer.map(|_| bound)))
        },
    )
}
