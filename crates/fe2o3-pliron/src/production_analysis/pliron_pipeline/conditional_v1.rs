use crate::production::{ConditionalPendingSubjectV1, ConditionalPipelineSubjectV1};
use crate::production_analysis::pliron_effect_refinement::conditional_v1 as conditional_effect;
use crate::production_analysis::pliron_pass_contract::PlironStructuralIdentityProviderV1;
use crate::production_analysis::{
    conditional_execution_v1 as conditional_ownership,
    conditional_semantic_v1 as conditional_semantic,
    conditional_validation_v1 as conditional_validation,
};
use pliron::op::Op;

#[derive(Clone, Copy)]
enum PipelineFamilyV1<'a> {
    Ordinary,
    Conditional(&'a ConditionalPipelineSubjectV1<'a>),
}

#[allow(
    clippy::large_enum_variant,
    reason = "keep validation custody inline without changing ordinary-path allocation accounting"
)]
enum ValidationFamilyV1<'a> {
    Ordinary(ProductionAnalysisReportValidationSessionV1<'a>),
    Conditional(conditional_validation::SessionV1<'a>),
}

// Constructed only by the closed dispatcher from its own preservation session
// and stable invocation-local manager, never from caller-supplied handles.
pub(crate) struct BoundConditionalInvocationV1<'a> {
    input: &'a ConditionalPipelineSubjectV1<'a>,
    preservation: crate::PlironPassValidationHandleV1,
    manager_address: usize,
}

impl<'a> BoundConditionalInvocationV1<'a> {
    pub(crate) fn into_parts(
        self,
    ) -> (
        &'a ConditionalPipelineSubjectV1<'a>,
        crate::PlironPassValidationHandleV1,
        usize,
    ) {
        (self.input, self.preservation, self.manager_address)
    }
}

#[derive(Debug, Eq, PartialEq)]
#[allow(
    clippy::large_enum_variant,
    reason = "transfer the admitted producer payload without another fallible allocation"
)]
pub(crate) enum ProducedConditionalPayloadV1 {
    Ownership(conditional_ownership::ReportV1),
    Semantic(conditional_semantic::ReportV1),
}

// The two fixed producer branches mint this packet after their own checkpoint.
// There is no exposed constructor or caller-supplied producer closure.
pub(crate) struct ProducedConditionalStageV1 {
    subject: ConditionalPendingSubjectV1,
    manager_address: usize,
    checkpoint: crate::PlironPassCheckpointTokenV1,
    payload: ProducedConditionalPayloadV1,
}

impl ProducedConditionalStageV1 {
    pub(crate) fn into_parts(
        self,
    ) -> (
        ConditionalPendingSubjectV1,
        usize,
        crate::PlironPassCheckpointTokenV1,
        ProducedConditionalPayloadV1,
    ) {
        (
            self.subject,
            self.manager_address,
            self.checkpoint,
            self.payload,
        )
    }
}

#[derive(Debug)]
#[allow(
    clippy::large_enum_variant,
    reason = "retain complete error payloads without an unadmitted error-path allocation"
)]
pub(crate) enum PipelineErrorV1 {
    Ordinary(ProductionPlironPreloweringErrorV2),
    ConditionalValidation(conditional_validation::ErrorV1),
    ConditionalOwnership(conditional_ownership::ReportV1),
    ConditionalSemantic(conditional_semantic::ErrorV1),
    ConditionalPreparation(conditional_ownership::FailureV1),
    ConditionalInput,
}

impl fmt::Display for PipelineErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ordinary(error) => error.fmt(formatter),
            Self::ConditionalValidation(error) => {
                write!(formatter, "conditional validation: {error:?}")
            }
            Self::ConditionalOwnership(report) => {
                write!(formatter, "conditional ownership rejected: {report:?}")
            }
            Self::ConditionalSemantic(error) => {
                write!(formatter, "conditional semantics rejected: {error:?}")
            }
            Self::ConditionalPreparation(error) => {
                write!(formatter, "conditional preparation: {error:?}")
            }
            Self::ConditionalInput => {
                formatter.write_str("conditional input custody or identity mismatch")
            }
        }
    }
}

impl From<ProductionPlironPreloweringErrorV2> for PipelineErrorV1 {
    fn from(error: ProductionPlironPreloweringErrorV2) -> Self {
        Self::Ordinary(error)
    }
}

impl From<conditional_validation::ErrorV1> for PipelineErrorV1 {
    fn from(error: conditional_validation::ErrorV1) -> Self {
        Self::ConditionalValidation(error)
    }
}

impl From<ProductionAnalysisResourceLimitV1> for PipelineErrorV1 {
    fn from(error: ProductionAnalysisResourceLimitV1) -> Self {
        Self::Ordinary(resource_upper_bound_error_v1(error))
    }
}

#[allow(
    clippy::large_enum_variant,
    reason = "transfer the admitted outcome without changing ordinary-path allocation accounting"
)]
enum PipelineOutcomeV1 {
    Ordinary(ProductionPlironPreloweringOutcomeV1),
    Conditional(ConditionalPipelineOutcomeV1),
}

#[allow(
    clippy::large_enum_variant,
    reason = "retain report custody inline without an unadmitted transfer-path allocation"
)]
enum PipelineReportsV1 {
    Ordinary(ProductionPlironPreloweringReportV2),
    Conditional(ConditionalPipelineReportV1),
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ConditionalPipelineReportV1 {
    target_contract: Option<PlironLaunchContractReportV1>,
    tensor_layout: PlironTensorLayoutReportV1,
    bounds: RankedBoundsReportV1,
    atomics: PlironAtomicLegalityReportV1,
    race: RankedRaceReportV1,
    barriers: PlironBarrierReportV1,
    pipeline_protocol: PlironPipelineProtocolReportV1,
    workgroup: PlironWorkgroupMemoryReportV1,
    preservation: PlironPassPreservationReportV1,
    validation: conditional_validation::ReportV1,
}

pub(crate) struct ConditionalPipelineOutcomeV1 {
    pub(crate) report: ConditionalPipelineReportV1,
    pub(crate) resource_upper_bound: ProductionAnalysisResourceUpperBoundV1,
}

impl ConditionalPipelineReportV1 {
    pub(crate) fn typed_root_commitments_v1(&self) -> Option<&[[u64; 4]]> {
        self.validation.typed_root_commitments_v1()
    }
    #[cfg(test)]
    pub(crate) fn ownership(&self) -> &conditional_ownership::ReportV1 {
        self.validation.ownership()
    }

    #[cfg(test)]
    pub(crate) fn semantics(&self) -> &conditional_semantic::ReportV1 {
        self.validation.semantics()
    }

    #[cfg(test)]
    pub(crate) fn independently_validated(&self) -> bool {
        self.validation.independently_validated()
    }

    #[cfg(test)]
    pub(crate) fn test_pass_count(&self) -> usize {
        self.preservation.certificates().len()
    }

    #[cfg(all(test, feature = "internal-proof-staging"))]
    pub(crate) fn test_payload_inequality_v1(mut self, other: &Self) -> Self {
        assert_eq!(&self, other);
        self.validation.test_flip_checked_exit_v1();
        assert!(self.ownership().is_clean());
        assert!(!self.independently_validated());
        assert_ne!(&self, other);
        self
    }
}

impl PipelineFamilyV1<'_> {
    #[allow(
        clippy::result_large_err,
        reason = "preserve inline error custody without an unadmitted error-path allocation"
    )]
    fn authenticate_initial(
        self,
        context: &Context,
        function: &FuncOp,
        preservation: &PlironPassContractSessionV1<LivePlironStructuralIdentityProviderV1<'_>>,
        analyses: &mut PlironAnalysisManagerV1,
        observer: PipelineObservationV1<'_, '_, '_>,
    ) -> Result<Option<ProductionAnalysisResourceUpperBoundV1>, PipelineErrorV1> {
        let Self::Conditional(input) = self else {
            return Ok(None);
        };
        let (snapshot, epoch) = preservation
            .initial_subject_v1()
            .map_err(|_| PipelineErrorV1::ConditionalInput)?;
        let phase = crate::ProductionAnalysisResourcePhaseV1::StructuralIdentity;
        let bound = invocation_receipt_v1::observe_resource_preflight_v1(observer, |_| {
            let work = input
                .retained_identity_storage_upper_bound()
                .checked_add(
                    preservation
                        .initial_identity_resource_upper_bound_v1()
                        .retained_storage_upper_bound(),
                )
                .and_then(|n| n.checked_add(1))
                .ok_or(ProductionAnalysisResourceLimitV1 {
                    phase,
                    resource: "conditional initial comparison work",
                })?;
            let scratch = work
                .checked_add(2048)
                .ok_or(ProductionAnalysisResourceLimitV1 {
                    phase,
                    resource: "conditional initial comparison storage",
                })?;
            ProductionAnalysisResourceUpperBoundV1::checked_phase(phase, work, 0, scratch)
        })?;
        if let Some(observer) = observer {
            observer.require(
                analyses
                    .remaining_resource_limits(phase)
                    .map_err(|error| observed_pipeline_resource_error_v1(Some(observer), error))?,
                phase,
                Ok(bound),
            )?;
        }
        analyses
            .admit_retained_resource_upper_bound(phase, bound)
            .map_err(|error| observed_pipeline_resource_error_v1(observer, error))?;
        let provider =
            LivePlironStructuralIdentityProviderV1::new(input.context(), input.function());
        if !std::ptr::eq(context, input.context())
            || function.get_operation() != input.function().get_operation()
            || epoch != input.epoch()
            || preservation.input_census_v1() != input.census()
            || provider.mutation_epoch().ok() != Some(input.epoch())
            || provider
                .require_exact_identity(input.identity(), snapshot)
                .is_err()
            || provider.mutation_epoch().ok() != Some(input.epoch())
        {
            return Err(PipelineErrorV1::ConditionalInput);
        }
        Ok(Some(bound))
    }

    #[allow(
        clippy::result_large_err,
        reason = "preserve inline error custody without an unadmitted error-path allocation"
    )]
    fn prepare_ownership_rows_with_observation_v1(
        self,
        analyses: &mut PlironAnalysisManagerV1,
        observer: PipelineObservationV1<'_, '_, '_>,
    ) -> Result<
        (
            Option<conditional_ownership::PreparedRowsV1>,
            Option<ProductionAnalysisResourceUpperBoundV1>,
        ),
        PipelineErrorV1,
    > {
        match self {
            Self::Ordinary => Ok((None, None)),
            Self::Conditional(input) => conditional_ownership::prepare_rows_with_observation_v1(
                input.context(),
                input.function(),
                analyses,
                input.census(),
                input.epoch(),
                input.occurrences().len(),
                observer,
            )
            .map(|(rows, bound)| (Some(rows), bound))
            .map_err(PipelineErrorV1::ConditionalPreparation),
        }
    }

    #[allow(
        clippy::result_large_err,
        reason = "preserve inline error custody without an unadmitted error-path allocation"
    )]
    fn prepare_ownership_stage_v1(
        self,
        analyses: &mut PlironAnalysisManagerV1,
        census: ProductionAnalysisInputCensusV1,
        trace: Option<crate::production_analysis::pliron_invocation_trace::ProductionInvocationTraceResourceAdmissionV1>,
        dependencies: (
            ProductionAnalysisResourceUpperBoundV1,
            ProductionAnalysisResourceUpperBoundV1,
        ),
        observer: PipelineObservationV1<'_, '_, '_>,
    ) -> Result<
        PreparedProductionStageV1<Option<conditional_ownership::PreparedRowsV1>>,
        PipelineErrorV1,
    > {
        let phase = ProductionAnalysisResourcePhaseV1::HierarchicalOwnership;
        let (rows, prefix) = self.prepare_ownership_rows_with_observation_v1(analyses, observer)?;
        let named = rows.as_ref().map_or(census, |rows| rows.named_census);
        let limits = observed_remaining_resource_limits_v1(analyses, phase, observer)?;
        let local = preflight_hierarchical_ownership_resource_upper_bound_v1(named, trace, limits)
            .map_err(|error| observed_pipeline_resource_error_v1(observer, error))?;
        let bound = compose_hierarchical_ownership_resource_upper_bound_v1(
            census,
            local,
            dependencies.0,
            dependencies.1,
        )
        .map_err(|error| observed_pipeline_resource_error_v1(observer, error))?;
        let bound = self.hold_phase(phase, bound, observer)?;
        Ok(PreparedProductionStageV1 {
            stage: ProductionAnalysisStageV1 {
                pass: KernelCheckPassKindV1::HierarchicalOwnership,
                producing_phase: phase,
                producing_phase_upper_bound: bound,
            },
            input: rows,
            prefix: prefix.unwrap_or_default(),
        })
    }

    #[allow(
        clippy::result_large_err,
        reason = "preserve inline error custody without an unadmitted error-path allocation"
    )]
    fn hold_phase(
        self,
        phase: crate::ProductionAnalysisResourcePhaseV1,
        bound: ProductionAnalysisResourceUpperBoundV1,
        observer: PipelineObservationV1<'_, '_, '_>,
    ) -> Result<ProductionAnalysisResourceUpperBoundV1, PipelineErrorV1> {
        Ok(match self {
            Self::Ordinary => bound,
            Self::Conditional(_) => ProductionAnalysisResourceUpperBoundV1::checked_phase(
                phase,
                bound.work_upper_bound(),
                bound.peak_storage_upper_bound(),
                0,
            )
            .map_err(|error| observed_pipeline_resource_error_v1(observer, error))?,
        })
    }

    #[allow(
        clippy::result_large_err,
        reason = "preserve inline error custody without an unadmitted error-path allocation"
    )]
    fn prepare_semantic_stage_v1(
        self,
        analyses: &mut PlironAnalysisManagerV1,
        census: ProductionAnalysisInputCensusV1,
        ownership: ProductionAnalysisResourceUpperBoundV1,
        observer: PipelineObservationV1<'_, '_, '_>,
    ) -> Result<PreparedProductionStageV1<()>, PipelineErrorV1> {
        use ProductionAnalysisResourcePhaseV1 as Phase;
        let resource_error = |error| observed_pipeline_resource_error_v1(observer, error);
        let effect_local = preflight_effect_refinement_resource_upper_bound_v1(
            census,
            observed_remaining_resource_limits_v1(analyses, Phase::EffectRefinement, observer)?,
        )
        .map_err(resource_error)?;
        let effect = match self {
            Self::Ordinary => {
                compose_effect_refinement_resource_upper_bound_v1(census, effect_local, ownership)
            }
            Self::Conditional(_) => effect_local
                .checked_with_nested_sequence_retain(&[ownership], Phase::EffectRefinement),
        }
        .map_err(resource_error)?;
        let semantic = preflight_semantic_refinement_resource_upper_bound_v1(
            census,
            observed_remaining_resource_limits_v1(analyses, Phase::SemanticRefinement, observer)?,
        )
        .map_err(resource_error)?;
        let progress = preflight_scoped_progress_resource_upper_bound_v1(
            census,
            observed_remaining_resource_limits_v1(analyses, Phase::Progress, observer)?,
        )
        .map_err(resource_error)?;
        let bound = semantic
            .checked_with_nested_sequence_retain(&[progress, effect], Phase::SemanticRefinement)
            .map_err(resource_error)?;
        let bound = self.hold_phase(Phase::SemanticRefinement, bound, observer)?;
        Ok(PreparedProductionStageV1 {
            stage: ProductionAnalysisStageV1 {
                pass: KernelCheckPassKindV1::SemanticRefinement,
                producing_phase: Phase::SemanticRefinement,
                producing_phase_upper_bound: bound,
            },
            input: (),
            prefix: ProductionAnalysisResourceUpperBoundV1::default(),
        })
    }
}

#[allow(
    clippy::result_large_err,
    reason = "preserve inline error custody without an unadmitted error-path allocation"
)]
fn prepare_conditional_semantic_input_v1<'a>(
    input: &'a ConditionalPipelineSubjectV1<'a>,
    analyses: &mut PlironAnalysisManagerV1,
    stage: ProductionAnalysisStageV1,
    observer: PipelineObservationV1<'_, '_, '_>,
) -> Result<PreparedProductionStageV1<conditional_effect::PreparedV1<'a>>, PipelineErrorV1> {
    use ProductionAnalysisResourcePhaseV1 as Phase;
    let census = input.census();
    let count = input.occurrences().len();
    let resource_error = |error| observed_pipeline_resource_error_v1(observer, error);
    let effect = conditional_effect::preflight_v1(census, count).map_err(resource_error)?;
    retain_cache_with_observation_v1(analyses, Phase::EffectRefinement, effect, observer)
        .map_err(resource_error)?;
    let semantic = with_pipeline_projection_v1(
        observer,
        &|local| effect.checked_then_retain(local, Phase::SemanticRefinement),
        |nested| {
            let bound = conditional_semantic::preflight_v1(census, count)
                .map_err(|error| observed_pipeline_resource_error_v1(nested, error))?;
            retain_cache_with_observation_v1(analyses, Phase::SemanticRefinement, bound, nested)
                .map_err(|error| observed_pipeline_resource_error_v1(nested, error))?;
            Ok::<_, PipelineErrorV1>(bound)
        },
    )?;
    let headers = observer
        .map(|_| effect.checked_then_retain(semantic, Phase::SemanticRefinement))
        .transpose()
        .map_err(resource_error)?
        .unwrap_or_default();
    let (rows, row_bound) = with_pipeline_projection_v1(
        observer,
        &|local| headers.checked_then_retain(local, Phase::HierarchicalOwnership),
        |nested| {
            PipelineFamilyV1::Conditional(input)
                .prepare_ownership_rows_with_observation_v1(analyses, nested)
        },
    )?;
    let rows = rows.ok_or(PipelineErrorV1::ConditionalInput)?;
    let prepared =
        conditional_effect::prepare_preadmitted_v1(input, rows.rows).map_err(resource_error)?;
    let prefix = match observer {
        None => ProductionAnalysisResourceUpperBoundV1::default(),
        Some(_) => headers
            .checked_then_retain(
                row_bound.ok_or(PipelineErrorV1::ConditionalInput)?,
                Phase::SemanticRefinement,
            )
            .map_err(resource_error)?,
    };
    Ok(PreparedProductionStageV1 {
        stage,
        input: prepared,
        prefix,
    })
}

#[cfg(test)]
#[allow(
    clippy::result_large_err,
    reason = "preserve inline error custody without an unadmitted error-path allocation"
)]
pub(crate) fn run_conditional_production_checks_v1(
    input: &ConditionalPipelineSubjectV1<'_>,
    atomic_target: Option<&PlironAtomicTargetContextV1>,
    target: Option<&PlironLaunchContractV1>,
    limits: ProductionAnalysisResourceLimitsV1,
) -> Result<ConditionalPipelineOutcomeV1, PipelineErrorV1> {
    run_conditional_production_checks_with_observation_v1(
        input,
        atomic_target,
        target,
        limits,
        None,
    )
}

#[allow(
    clippy::result_large_err,
    reason = "preserve inline error custody without an unadmitted error-path allocation"
)]
pub(crate) fn run_conditional_production_checks_with_observation_v1(
    input: &ConditionalPipelineSubjectV1<'_>,
    atomic_target: Option<&PlironAtomicTargetContextV1>,
    target: Option<&PlironLaunchContractV1>,
    limits: ProductionAnalysisResourceLimitsV1,
    receipt: Option<&mut invocation_receipt_v1::InvocationReceiptV1>,
) -> Result<ConditionalPipelineOutcomeV1, PipelineErrorV1> {
    match run_shared_production_checks_v1(
        input.context(),
        input.function(),
        atomic_target,
        target,
        limits,
        (PipelineFamilyV1::Conditional(input), receipt),
        #[cfg(test)]
        None,
    )? {
        PipelineOutcomeV1::Conditional(outcome) => Ok(outcome),
        PipelineOutcomeV1::Ordinary(_) => Err(PipelineErrorV1::ConditionalInput),
    }
}

#[cfg(test)]
pub(crate) fn test_conditional_foreign_endpoint_v1(
    input: &ConditionalPipelineSubjectV1<'_>,
    foreign: &ConditionalPipelineSubjectV1<'_>,
    limits: ProductionAnalysisResourceLimitsV1,
) -> bool {
    matches!(
        run_shared_production_checks_v1(
            foreign.context(),
            foreign.function(),
            None,
            None,
            limits,
            (PipelineFamilyV1::Conditional(input), None),
            None,
        ),
        Err(PipelineErrorV1::ConditionalInput)
    )
}

#[cfg(test)]
#[derive(Clone, Copy, Debug)]
pub(crate) enum CaptureFaultV1 {
    Duplicate,
    OutOfOrder,
    WrongFamily,
    Partial,
    ForeignManager(usize),
    ForeignSubject(ConditionalPendingSubjectV1),
}

#[cfg(test)]
pub(crate) fn test_capture_rejection_v1(
    input: &ConditionalPipelineSubjectV1<'_>,
    limits: ProductionAnalysisResourceLimitsV1,
    fault: CaptureFaultV1,
) -> PipelineErrorV1 {
    run_shared_production_checks_v1(
        input.context(),
        input.function(),
        None,
        None,
        limits,
        (PipelineFamilyV1::Conditional(input), None),
        Some(fault),
    )
    .err()
    .expect("fault must reject")
}

#[cfg(test)]
#[allow(
    clippy::result_large_err,
    reason = "preserve inline error custody without an unadmitted error-path allocation"
)]
fn record_capture_test_v1(
    validation: &mut conditional_validation::SessionV1<'_>,
    mut packet: ProducedConditionalStageV1,
    analyses: &mut PlironAnalysisManagerV1,
    preservation: &PlironPassContractSessionV1<LivePlironStructuralIdentityProviderV1<'_>>,
    race: &RankedRaceReportV1,
    fault: Option<CaptureFaultV1>,
    observer: PipelineObservationV1<'_, '_, '_>,
) -> Result<Option<ProductionAnalysisResourceUpperBoundV1>, PipelineErrorV1> {
    use CaptureFaultV1::*;

    match (fault, packet.checkpoint.position()) {
        (Some(OutOfOrder), 4) | (Some(Partial), 8) => return Ok(None),
        (Some(ForeignManager(address)), _) => packet.manager_address = address,
        (Some(ForeignSubject(subject)), _) => packet.subject = subject,
        (Some(WrongFamily), 8) => validation.test_swap_ownership_v1(&mut packet.payload),
        _ => {}
    }
    let duplicate = matches!(fault, Some(Duplicate)) && packet.checkpoint.position() == 4;
    let header = validation.record_produced_with_observation_v1(packet, analyses, observer)?;
    if duplicate {
        let token = preservation
            .last_checkpoint()
            .map_err(ProductionPlironPreloweringErrorV2::Preservation)?;
        let repeat = |observer: PipelineObservationV1<'_, '_, '_>| {
            validation.record_ordinary_with_observation_v1(
                token,
                race,
                ProductionAnalysisResourceUpperBoundV1::default(),
                analyses,
                observer,
            )
        };
        match observer {
            None => repeat(None)?,
            Some(observer) => observer.with_projection(
                &|local| {
                    header.checked_then_retain(
                        local,
                        ProductionAnalysisResourcePhaseV1::ReportValidation,
                    )
                },
                |nested| repeat(Some(nested)),
            )?,
        };
    }
    Ok(observer.map(|_| header))
}

impl<'a> ValidationFamilyV1<'a> {
    #[allow(
        clippy::result_large_err,
        reason = "preserve inline error custody without an unadmitted error-path allocation"
    )]
    fn begin(
        family: PipelineFamilyV1<'a>,
        endpoint: (&'a Context, &'a FuncOp),
        atomic_target: Option<&PlironAtomicTargetContextV1>,
        preservation: crate::PlironPassValidationHandleV1,
        census: ProductionAnalysisInputCensusV1,
        limits: ProductionAnalysisResourceLimitsV1,
        observed_analyses: (
            &mut PlironAnalysisManagerV1,
            PipelineObservationV1<'_, '_, '_>,
        ),
    ) -> Result<(Self, Option<ProductionAnalysisResourceUpperBoundV1>), PipelineErrorV1> {
        let (context, function) = endpoint;
        let (analyses, observer) = observed_analyses;
        match family {
            PipelineFamilyV1::Ordinary => {
                let session = begin_observed_report_validation_v1(
                    context,
                    function,
                    atomic_target,
                    preservation,
                    census,
                    limits,
                    observer,
                )
                .map_err(ProductionPlironPreloweringErrorV2::ReportValidation)?;
                let transfer = observer.map(|_| session.setup_resource_upper_bound_v1());
                Ok((Self::Ordinary(session), transfer))
            }
            PipelineFamilyV1::Conditional(input) => {
                let (session, bound) =
                    conditional_validation::SessionV1::begin_with_observation_v1(
                        BoundConditionalInvocationV1 {
                            input,
                            preservation,
                            manager_address: analyses as *const PlironAnalysisManagerV1 as usize,
                        },
                        atomic_target,
                        analyses,
                        observer,
                    )?;
                Ok((Self::Conditional(session), observer.map(|_| bound)))
            }
        }
    }

    fn setup_resource_upper_bound_v1(&self) -> ProductionAnalysisResourceUpperBoundV1 {
        match self {
            Self::Ordinary(session) => session.setup_resource_upper_bound_v1(),
            Self::Conditional(_) => ProductionAnalysisResourceUpperBoundV1::default(),
        }
    }

    #[allow(
        clippy::result_large_err,
        reason = "preserve inline error custody without an unadmitted error-path allocation"
    )]
    fn record_standard<R: SealedProductionAnalysisReportV1>(
        &mut self,
        context: &Context,
        function: &FuncOp,
        checkpoint: crate::PlironPassCheckpointTokenV1,
        report: &R,
        stage: ProductionAnalysisStageV1,
        observed_analyses: (
            &mut PlironAnalysisManagerV1,
            PipelineObservationV1<'_, '_, '_>,
        ),
    ) -> Result<Option<ProductionAnalysisResourceUpperBoundV1>, PipelineErrorV1> {
        let (analyses, observer) = observed_analyses;
        match self {
            Self::Ordinary(session) => {
                let pass = stage.pass;
                let phase = crate::ProductionAnalysisResourcePhaseV1::ReportValidation;
                let resource_error = |error| {
                    if let Some(observer) = observer {
                        observer.deny(error);
                    }
                    stage_resource_upper_bound_error_v1(error, pass)
                };
                let limits = analyses
                    .remaining_resource_limits(phase)
                    .map_err(resource_error)?;
                session
                    .record_with_observation_v1(
                        ProductionAnalysisReportEndpointV1 { context, function },
                        checkpoint,
                        report,
                        stage.producing_phase_upper_bound,
                        limits,
                        (analyses, observer),
                    )
                    .map_err(ProductionPlironPreloweringErrorV2::ReportValidation)?;
                let bound = session.last_stage_resource_upper_bound_v1().ok_or(
                    ProductionPlironPreloweringErrorV2::ResourceLimit {
                        phase,
                        producing_pass: Some(pass),
                        resource: "report-validation resource upper bound",
                    },
                )?;
                analyses
                    .admit_retained_resource_upper_bound(phase, bound)
                    .map_err(resource_error)?;
                Ok(observer.map(|_| bound))
            }
            Self::Conditional(session) => session
                .record_ordinary_with_observation_v1(
                    checkpoint,
                    report,
                    stage.producing_phase_upper_bound,
                    analyses,
                    observer,
                )
                .map_err(Into::into),
        }
    }
}
