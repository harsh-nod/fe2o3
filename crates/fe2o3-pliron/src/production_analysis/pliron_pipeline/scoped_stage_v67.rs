use crate::ProductionAnalysisResourcePhaseV1;

type PipelineObservationV1<'o, 'p, 'r> =
    Option<&'o invocation_receipt_v1::InvocationObserverV1<'p, 'r>>;

type StageObservationsV1<'o, 'p, 'r> = (
    PipelineObservationV1<'o, 'p, 'r>,
    PipelineObservationV1<'o, 'p, 'r>,
    invocation_receipt_v1::AdditionalObservationV1<'o, 'p, 'r>,
);

struct PreparedProductionStageV1<T> {
    stage: ProductionAnalysisStageV1,
    input: T,
    // Real admissions made by preparation before the main producer admission.
    prefix: ProductionAnalysisResourceUpperBoundV1,
}

fn with_pipeline_projection_v1<T>(
    observer: PipelineObservationV1<'_, '_, '_>,
    project: &dyn Fn(
        ProductionAnalysisResourceUpperBoundV1,
    ) -> Result<
        ProductionAnalysisResourceUpperBoundV1,
        ProductionAnalysisResourceLimitV1,
    >,
    run: impl FnOnce(PipelineObservationV1<'_, '_, '_>) -> T,
) -> T {
    match observer {
        None => run(None),
        Some(observer) => observer.with_projection(project, |nested| run(Some(nested))),
    }
}

fn with_scoped_producer_observation_v1<T>(
    observer: PipelineObservationV1<'_, '_, '_>,
    run: impl FnOnce(PipelineObservationV1<'_, '_, '_>) -> Result<T, PlironPassPreservationErrorV1>,
) -> Result<T, PlironPassPreservationErrorV1> {
    let Some(observer) = observer else {
        return run(None);
    };
    observer.with_projection(&Ok, |observer| {
        let phase = ProductionAnalysisResourcePhaseV1::PassPreservation;
        let scope = crate::production_analysis::pliron_pass_contract::scoped_progress_input_resource_upper_bound_v1()
            .inspect_err(|error| {
                if let PlironPassPreservationErrorV1::ResourceLimit { resource } = error {
                    observer.deny(ProductionAnalysisResourceLimitV1 { phase, resource });
                }
            })?;
        observer.with_projection(&|local| scope.checked_then_retain(local, phase), |nested| run(Some(nested)))
    })
}

#[allow(clippy::result_large_err)]
fn prepare_and_invoke_production_stage_v1<T, E, P>(
    analyses: &mut PlironAnalysisManagerV1,
    preservation: &mut PlironPassContractSessionV1<LivePlironStructuralIdentityProviderV1<'_>>,
    receipt: Option<&mut invocation_receipt_v1::InvocationReceiptV1>,
    preparation: (
        ProductionAnalysisResourcePhaseV1,
        impl FnOnce(
            &mut PlironAnalysisManagerV1,
            PipelineObservationV1<'_, '_, '_>,
        ) -> Result<PreparedProductionStageV1<P>, PipelineErrorV1>,
    ),
    invoke: impl FnOnce(
        &mut PlironPassContractSessionV1<LivePlironStructuralIdentityProviderV1<'_>>,
        KernelCheckPassKindV1,
        crate::production_analysis::pliron_resource_envelope::ProductionAnalysisReplacementLimitsV1,
        &mut PlironAnalysisManagerV1,
        P,
        StageObservationsV1<'_, '_, '_>,
    ) -> Result<Result<T, E>, PlironPassPreservationErrorV1>,
) -> Result<(Result<T, E>, ProductionAnalysisStageV1), PipelineErrorV1> {
    let (phase, prepare) = preparation;
    let replaced = preservation
        .lineage_identity_resource_upper_bound_v1()
        .retained_storage_upper_bound();
    with_invocation_phase_v1(receipt, phase, replaced, |observer| {
        let old = || ProductionAnalysisResourceUpperBoundV1::checked_phase(phase, 0, replaced, 0);
        let PreparedProductionStageV1 {
            stage,
            input,
            prefix,
        } = with_pipeline_projection_v1(
            observer,
            &|local| old()?.checked_then_retain(local, phase),
            |nested| prepare(analyses, nested),
        )?;
        let additional = std::cell::Cell::new(ProductionAnalysisResourceUpperBoundV1::default());
        let (result, checkpoint) = with_pipeline_projection_v1(
            observer,
            &|local| {
                old()?
                    .checked_then_retain(prefix, phase)?
                    .checked_then_retain(local, phase)
            },
            |producer| {
                with_pipeline_projection_v1(
                    observer,
                    &|local| {
                        prefix
                            .checked_then_retain(stage.producing_phase_upper_bound, phase)?
                            .checked_then_retain(additional.get(), phase)?
                            .checked_then_retain(local, phase)
                    },
                    |checkpoint| {
                        with_pipeline_projection_v1(
                            producer,
                            &|local| {
                                stage
                                    .producing_phase_upper_bound
                                    .checked_then_retain(local, phase)
                            },
                            |extra| {
                                invoke_production_analysis_stage_with_observation_v1(
                                    analyses,
                                    preservation,
                                    stage,
                                    (
                                        producer,
                                        checkpoint,
                                        extra.map(|observer| (observer, &additional)),
                                    ),
                                    |preservation, pass, limits, analyses, observations| {
                                        invoke(
                                            preservation,
                                            pass,
                                            limits,
                                            analyses,
                                            input,
                                            observations,
                                        )
                                    },
                                )
                            },
                        )
                    },
                )
            },
        )?;
        let transfer = observer
            .map(|_| {
                prefix
                    .checked_then_retain(stage.producing_phase_upper_bound, phase)?
                    .checked_then_retain(additional.get(), phase)?
                    .checked_then_retain(checkpoint, phase)
            })
            .transpose()
            .map_err(|error| observed_pipeline_resource_error_v1(observer, error))?;
        Ok(((result, stage), transfer))
    })
}

#[allow(clippy::result_large_err)]
fn run_preflight_and_record_production_stage_v1<T, E, P>(
    endpoint: (&Context, &FuncOp),
    analyses: &mut PlironAnalysisManagerV1,
    preservation: &mut PlironPassContractSessionV1<LivePlironStructuralIdentityProviderV1<'_>>,
    validation: (
        &mut ValidationFamilyV1<'_>,
        Option<&mut invocation_receipt_v1::InvocationReceiptV1>,
    ),
    preparation: (
        ProductionAnalysisResourcePhaseV1,
        impl FnOnce(
            &mut PlironAnalysisManagerV1,
            PipelineObservationV1<'_, '_, '_>,
        ) -> Result<PreparedProductionStageV1<P>, PipelineErrorV1>,
    ),
    invoke: impl FnOnce(
        &mut PlironPassContractSessionV1<LivePlironStructuralIdentityProviderV1<'_>>,
        KernelCheckPassKindV1,
        crate::production_analysis::pliron_resource_envelope::ProductionAnalysisReplacementLimitsV1,
        &mut PlironAnalysisManagerV1,
        P,
        StageObservationsV1<'_, '_, '_>,
    ) -> Result<Result<T, E>, PlironPassPreservationErrorV1>,
) -> Result<(Result<T, E>, ProductionAnalysisResourceUpperBoundV1), PipelineErrorV1>
where
    T: SealedProductionAnalysisReportV1,
{
    let (validation, mut receipt) = validation;
    let (result, stage) = prepare_and_invoke_production_stage_v1(
        analyses,
        preservation,
        receipt.as_deref_mut(),
        preparation,
        invoke,
    )?;
    if let Ok(report) = &result {
        record_production_stage_result_v1(
            endpoint,
            analyses,
            preservation,
            (validation, receipt),
            stage,
            report,
        )?;
    }
    Ok((result, stage.producing_phase_upper_bound))
}

#[allow(clippy::result_large_err)]
fn run_preflight_production_stage_v1<T, E>(
    endpoint: (&Context, &FuncOp),
    analyses: &mut PlironAnalysisManagerV1,
    preservation: &mut PlironPassContractSessionV1<LivePlironStructuralIdentityProviderV1<'_>>,
    validation: (
        &mut ValidationFamilyV1<'_>,
        Option<&mut invocation_receipt_v1::InvocationReceiptV1>,
    ),
    preparation: (
        ProductionAnalysisResourcePhaseV1,
        impl FnOnce(
            &mut PlironAnalysisManagerV1,
            PipelineObservationV1<'_, '_, '_>,
        ) -> Result<PreparedProductionStageV1<()>, PipelineErrorV1>,
    ),
    execute: impl FnOnce(
        &mut PlironAnalysisManagerV1,
        PipelineObservationV1<'_, '_, '_>,
    ) -> Result<T, E>,
) -> Result<(Result<T, E>, ProductionAnalysisResourceUpperBoundV1), PipelineErrorV1>
where
    T: SealedProductionAnalysisReportV1,
{
    run_preflight_and_record_production_stage_v1(
        endpoint,
        analyses,
        preservation,
        validation,
        preparation,
        |preservation, pass, limits, analyses, (), (producer, checkpoint, _)| {
            preservation.run_contiguous_pass_with_observation_v1(
                pass,
                limits,
                || execute(analyses, producer),
                checkpoint,
            )
        },
    )
}

#[allow(clippy::result_large_err)]
fn with_invocation_phase_v1<T>(
    receipt: Option<&mut invocation_receipt_v1::InvocationReceiptV1>,
    owner: ProductionAnalysisResourcePhaseV1,
    replaced: usize,
    run: impl FnOnce(
        PipelineObservationV1<'_, '_, '_>,
    )
        -> Result<(T, Option<ProductionAnalysisResourceUpperBoundV1>), PipelineErrorV1>,
) -> Result<T, PipelineErrorV1> {
    let Some(receipt) = receipt else {
        return run(None).map(|(value, _)| value);
    };
    let phase = receipt.phase(owner, replaced)?;
    let (value, transfer) = phase
        .observer(&Ok)
        .with_projection(&Ok, |observer| run(Some(observer)))?;
    if let Some(transfer) = transfer {
        phase.commit(transfer)?;
    }
    Ok(value)
}

fn observed_pipeline_resource_error_v1(
    observer: PipelineObservationV1<'_, '_, '_>,
    error: ProductionAnalysisResourceLimitV1,
) -> PipelineErrorV1 {
    if let Some(observer) = observer {
        observer.deny(error);
    }
    error.into()
}

#[allow(
    clippy::result_large_err,
    reason = "preserve inline error custody without an unadmitted error-path allocation"
)]
fn observed_remaining_resource_limits_v1(
    analyses: &PlironAnalysisManagerV1,
    phase: ProductionAnalysisResourcePhaseV1,
    observer: PipelineObservationV1<'_, '_, '_>,
) -> Result<ProductionAnalysisResourceLimitsV1, PipelineErrorV1> {
    remaining_resource_limits_v1(analyses, phase)
        .map_err(|error| observed_pipeline_resource_error_v1(observer, error))
}
