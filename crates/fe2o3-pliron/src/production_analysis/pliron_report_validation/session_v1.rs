type ReportObservationV1<'o, 'p, 'r> =
    super::pliron_report_payload_receipt::PayloadObservationV1<'o, 'p, 'r>;

fn with_report_observation_v1<T>(
    observer: ReportObservationV1<'_, '_, '_>,
    run: impl FnOnce(
        ReportObservationV1<'_, '_, '_>,
    ) -> Result<T, ProductionAnalysisReportValidationErrorV1>,
) -> Result<T, ProductionAnalysisReportValidationErrorV1> {
    match observer {
        None => run(None),
        Some(observer) => observer.with_projection(&Ok, |nested| run(Some(nested))),
    }
}

fn observed_report_resource_error_v1(
    observer: ReportObservationV1<'_, '_, '_>,
    pass: Option<KernelCheckPassKindV1>,
    error: ProductionAnalysisResourceLimitV1,
) -> ProductionAnalysisReportValidationErrorV1 {
    if let Some(observer) = observer {
        observer.deny(error);
    }
    match pass {
        None => report_resource_error_v1(error),
        Some(pass) => report_stage_resource_error_v1(pass, error),
    }
}

impl<'a> ProductionAnalysisReportValidationSessionV1<'a> {
    fn new_with_observation_v1(
        context: &'a Context,
        function: &'a FuncOp,
        atomic_target: Option<&PlironAtomicTargetContextV1>,
        preservation: PlironPassValidationHandleV1,
        input_census: ProductionAnalysisInputCensusV1,
        limits: ProductionAnalysisResourceLimitsV1,
        observer: ReportObservationV1<'_, '_, '_>,
    ) -> Result<Self, ProductionAnalysisReportValidationErrorV1> {
        with_report_observation_v1(observer, |observer| {
            let resource_error = |error| observed_report_resource_error_v1(observer, None, error);
            let configuration_items = atomic_target.map_or(0, |target| target.capabilities().len());
            let setup_resource_upper_bound =
                report_validation_setup_resource_upper_bound_v1(configuration_items)
                    .map_err(resource_error)?;
            super::pliron_pipeline::invocation_receipt_v1::require_observed_v1(
                limits,
                ProductionAnalysisResourcePhaseV1::ReportValidation,
                Ok(setup_resource_upper_bound),
                observer,
            )
            .map_err(resource_error)?;

            let atomic_configuration = match atomic_target {
                None => ProductionAnalysisConfigurationV1::AtomicTargetAgnostic,
                Some(target) => {
                    let mut capabilities = Vec::new();
                    capabilities
                        .try_reserve_exact(configuration_items)
                        .map_err(|_| {
                            resource_error(ProductionAnalysisResourceLimitV1 {
                                phase: ProductionAnalysisResourcePhaseV1::ReportValidation,
                                resource: "report validation target capability allocation",
                            })
                        })?;
                    capabilities.extend(target.capabilities().iter().copied());
                    ProductionAnalysisConfigurationV1::AtomicTarget { capabilities }
                }
            };
            let mut stages = Vec::new();
            stages
                .try_reserve_exact(PRODUCTION_ANALYSIS_REPORT_COUNT_V1)
                .map_err(|_| {
                    resource_error(ProductionAnalysisResourceLimitV1 {
                        phase: ProductionAnalysisResourcePhaseV1::ReportValidation,
                        resource: "report validation stage allocation",
                    })
                })?;
            Ok(Self {
                preservation,
                context_address: context as *const Context as usize,
                function: function.get_operation(),
                atomic_configuration,
                input_census,
                next: 0,
                stages,
                setup_resource_upper_bound,
                last_stage_resource_upper_bound: None,
                _subject_borrow: PhantomData,
            })
        })
    }

    fn subject_v1(&self) -> ReportValidationSubjectV1<'_> {
        ReportValidationSubjectV1 {
            preservation: &self.preservation,
            context_address: self.context_address,
            function: self.function,
            atomic_configuration: &self.atomic_configuration,
        }
    }

    fn issue_with_payload_receipt_v1(
        &self,
        context: &Context,
        function: &FuncOp,
        checkpoint_token: PlironPassCheckpointTokenV1,
        report: CapturedProductionAnalysisReportV1,
        payload_receipt: ProductionAnalysisReportPayloadReceiptV1,
        observer: ReportObservationV1<'_, '_, '_>,
    ) -> Result<BoundProductionAnalysisReportV1, ProductionAnalysisReportValidationErrorV1> {
        self.subject_v1().issue_at_v1(
            self.next,
            context,
            function,
            checkpoint_token,
            report,
            (payload_receipt, observer),
        )
    }

    fn accept_with_resource_upper_bound_v1(
        &mut self,
        context: &Context,
        function: &FuncOp,
        bound: &BoundProductionAnalysisReportV1,
        resource_upper_bound: ProductionAnalysisResourceUpperBoundV1,
        analyses: &mut PlironAnalysisManagerV1,
        observer: ReportObservationV1<'_, '_, '_>,
    ) -> Result<(), ProductionAnalysisReportValidationErrorV1> {
        let stage = self
            .subject_v1()
            .accept_at_v1(self.next, context, function, bound, analyses, observer)?;
        self.stages.push(stage);
        self.last_stage_resource_upper_bound = Some(resource_upper_bound);
        self.next += 1;
        Ok(())
    }
}

// One stage's custody and witness checks, shared without an ordinary session.
struct ReportValidationSubjectV1<'a> {
    preservation: &'a PlironPassValidationHandleV1,
    context_address: usize,
    function: Ptr<Operation>,
    atomic_configuration: &'a ProductionAnalysisConfigurationV1,
}

impl ReportValidationSubjectV1<'_> {
    fn expected_configuration(
        &self,
        pass: KernelCheckPassKindV1,
    ) -> ProductionAnalysisConfigurationV1 {
        if pass == KernelCheckPassKindV1::AtomicLegality {
            self.atomic_configuration.clone()
        } else {
            ProductionAnalysisConfigurationV1::FixedByImplementation
        }
    }

    fn issue_at_v1(
        &self,
        position: usize,
        context: &Context,
        function: &FuncOp,
        checkpoint_token: PlironPassCheckpointTokenV1,
        report: CapturedProductionAnalysisReportV1,
        payload: (
            ProductionAnalysisReportPayloadReceiptV1,
            ReportObservationV1<'_, '_, '_>,
        ),
    ) -> Result<BoundProductionAnalysisReportV1, ProductionAnalysisReportValidationErrorV1> {
        let (payload_receipt, observer) = payload;
        self.require_subject_handles(context, function, position)?;
        if !self.preservation.same_custody(&checkpoint_token) {
            return Err(
                ProductionAnalysisReportValidationErrorV1::CounterfeitOrCrossSessionSeal {
                    position,
                },
            );
        }
        let expected = PRODUCTION_PLIRON_PASS_CONTRACTS_V1
            .get(position)
            .ok_or(ProductionAnalysisReportValidationErrorV1::OmittedReport {
                position,
                pass: report.pass(),
            })?
            .pass();
        let submitted_checkpoint = ProductionAnalysisCheckpointV1 {
            position: checkpoint_token.position(),
            pass: checkpoint_token.pass(),
            identity: checkpoint_token.identity(),
            mutation_epoch: checkpoint_token.mutation_epoch(),
        };
        Ok(BoundProductionAnalysisReportV1 {
            payload_receipt,
            checkpoint_token,
            context_address: self.context_address,
            function: self.function,
            submitted_checkpoint,
            implementation: implementation_for(expected),
            configuration: self.expected_configuration(expected),
            claimed_status: report.status(),
            issued_report: report
                .try_clone_payload_v1(payload_receipt)
                .map_err(|error| {
                    observed_report_resource_error_v1(observer, Some(report.pass()), error)
                })?,
            submitted_report: report,
        })
    }

    fn accept_at_v1(
        &self,
        position: usize,
        context: &Context,
        function: &FuncOp,
        bound: &BoundProductionAnalysisReportV1,
        analyses: &mut PlironAnalysisManagerV1,
        observer: ReportObservationV1<'_, '_, '_>,
    ) -> Result<ProductionAnalysisStageValidationV1, ProductionAnalysisReportValidationErrorV1>
    {
        self.require_subject_handles(context, function, position)?;
        if !self.preservation.same_custody(&bound.checkpoint_token) {
            return Err(
                ProductionAnalysisReportValidationErrorV1::CounterfeitOrCrossSessionSeal {
                    position,
                },
            );
        }
        let expected = PRODUCTION_PLIRON_PASS_CONTRACTS_V1
            .get(position)
            .ok_or(
                ProductionAnalysisReportValidationErrorV1::StageOrderMismatch {
                    position,
                    expected: KernelCheckPassKindV1::SemanticRefinement,
                    observed: bound.submitted_report.pass(),
                },
            )?
            .pass();
        let issued_position = bound.checkpoint_token.position();
        if issued_position < position {
            return Err(ProductionAnalysisReportValidationErrorV1::ReplayedReport {
                issued_position,
                current_position: position,
            });
        }
        if issued_position != position || bound.checkpoint_token.pass() != expected {
            return Err(
                ProductionAnalysisReportValidationErrorV1::StageOrderMismatch {
                    position,
                    expected,
                    observed: bound.checkpoint_token.pass(),
                },
            );
        }
        if bound.context_address != self.context_address {
            return Err(ProductionAnalysisReportValidationErrorV1::CrossContextReport { position });
        }
        if bound.function != self.function {
            return Err(
                ProductionAnalysisReportValidationErrorV1::CrossFunctionReport { position },
            );
        }
        if bound.submitted_checkpoint.position != issued_position
            || bound.submitted_checkpoint.pass != bound.checkpoint_token.pass()
            || bound.submitted_checkpoint.identity != bound.checkpoint_token.identity()
            || bound.submitted_checkpoint.mutation_epoch != bound.checkpoint_token.mutation_epoch()
        {
            return Err(
                ProductionAnalysisReportValidationErrorV1::CheckpointMetadataTampered { position },
            );
        }
        if bound.implementation != implementation_for(expected)
            || bound.implementation.pass() != expected
        {
            return Err(
                ProductionAnalysisReportValidationErrorV1::ImplementationTampered { position },
            );
        }
        if bound.configuration != self.expected_configuration(expected) {
            return Err(
                ProductionAnalysisReportValidationErrorV1::ConfigurationTampered { position },
            );
        }
        if bound.issued_report != bound.submitted_report {
            return Err(
                ProductionAnalysisReportValidationErrorV1::ReportPayloadTampered { position },
            );
        }
        let observed = bound.submitted_report.pass();
        if observed != expected {
            return Err(
                ProductionAnalysisReportValidationErrorV1::StageOrderMismatch {
                    position,
                    expected,
                    observed,
                },
            );
        }
        if bound.claimed_status != bound.submitted_report.status() {
            return Err(
                ProductionAnalysisReportValidationErrorV1::ReportStatusTampered { position },
            );
        }
        let witness =
            issue_and_validate_production_analysis_witness_v1(
                context,
                function,
                bound.submitted_checkpoint,
                bound.implementation,
                bound.configuration.clone(),
                bound
                    .submitted_report
                    .try_clone_payload_v1(bound.payload_receipt)
                    .map_err(|error| {
                        observed_report_resource_error_v1(observer, Some(expected), error)
                    })?,
                (analyses, observer),
            )
            .map_err(|error| {
                ProductionAnalysisReportValidationErrorV1::WitnessValidation { position, error }
            })?;
        Ok(ProductionAnalysisStageValidationV1 {
            checkpoint: bound.submitted_checkpoint,
            implementation: bound.implementation,
            configuration: bound.configuration.clone(),
            analysis_status: bound.claimed_status,
            witness,
        })
    }

    fn require_subject_handles(
        &self,
        context: &Context,
        function: &FuncOp,
        position: usize,
    ) -> Result<(), ProductionAnalysisReportValidationErrorV1> {
        if context as *const Context as usize != self.context_address {
            return Err(ProductionAnalysisReportValidationErrorV1::CrossContextReport { position });
        }
        if function.get_operation() != self.function {
            return Err(
                ProductionAnalysisReportValidationErrorV1::CrossFunctionReport { position },
            );
        }
        Ok(())
    }
}

impl ProductionAnalysisReportValidationSessionV1<'_> {
    #[cfg(test)]
    fn issue(
        &self,
        context: &Context,
        function: &FuncOp,
        checkpoint_token: PlironPassCheckpointTokenV1,
        report: CapturedProductionAnalysisReportV1,
    ) -> Result<BoundProductionAnalysisReportV1, ProductionAnalysisReportValidationErrorV1> {
        let receipt = ProductionAnalysisReportPayloadReceiptV1::fallback(report.pass(), 0);
        self.issue_with_payload_receipt_v1(
            context,
            function,
            checkpoint_token,
            report,
            receipt,
            None,
        )
    }

    #[cfg(test)]
    fn accept(
        &mut self,
        context: &Context,
        function: &FuncOp,
        bound: &BoundProductionAnalysisReportV1,
    ) -> Result<(), ProductionAnalysisReportValidationErrorV1> {
        let mut analyses = PlironAnalysisManagerV1::new(function);
        self.accept_with_resource_upper_bound_v1(
            context,
            function,
            bound,
            ProductionAnalysisResourceUpperBoundV1::default(),
            &mut analyses,
            None,
        )
    }

    pub(crate) const fn last_stage_resource_upper_bound_v1(
        &self,
    ) -> Option<ProductionAnalysisResourceUpperBoundV1> {
        self.last_stage_resource_upper_bound
    }

    pub(crate) const fn setup_resource_upper_bound_v1(
        &self,
    ) -> ProductionAnalysisResourceUpperBoundV1 {
        self.setup_resource_upper_bound
    }

    fn finish(
        self,
        preservation: &PlironPassPreservationReportV1,
    ) -> Result<ProductionAnalysisReportValidationV1, ProductionAnalysisReportValidationErrorV1>
    {
        if self.next != PRODUCTION_ANALYSIS_REPORT_COUNT_V1 {
            let pass = PRODUCTION_PLIRON_PASS_CONTRACTS_V1[self.next].pass();
            return Err(ProductionAnalysisReportValidationErrorV1::OmittedReport {
                position: self.next,
                pass,
            });
        }
        let manifest_matches = self.preservation.same_report_custody(preservation)
            && self.preservation.input_identity() == preservation.input_identity()
            && self.preservation.input_mutation_epoch() == preservation.input_mutation_epoch()
            && preservation.certificates().len() == PRODUCTION_ANALYSIS_REPORT_COUNT_V1
            && preservation.is_exact_identity()
            && self
                .stages
                .iter()
                .zip(preservation.certificates())
                .enumerate()
                .all(|(position, (stage, certificate))| {
                    stage.checkpoint.position == position
                        && stage.checkpoint.pass == certificate.pass()
                        && stage.checkpoint.identity == certificate.identity()
                        && stage.checkpoint.mutation_epoch == certificate.mutation_epoch()
                })
            && preservation
                .certificates()
                .iter()
                .zip(PRODUCTION_PLIRON_PASS_CONTRACTS_V1)
                .all(|(certificate, contract)| certificate.pass() == contract.pass());
        if !manifest_matches {
            return Err(
                ProductionAnalysisReportValidationErrorV1::PreservationManifestInconsistent,
            );
        }
        Ok(ProductionAnalysisReportValidationV1 {
            stages: self.stages,
        })
    }
}

#[cfg(test)]
pub(crate) fn begin_production_analysis_report_validation_with_resource_limits_v1<'a>(
    context: &'a Context,
    function: &'a FuncOp,
    atomic_target: Option<&PlironAtomicTargetContextV1>,
    preservation: PlironPassValidationHandleV1,
    input_census: ProductionAnalysisInputCensusV1,
    limits: ProductionAnalysisResourceLimitsV1,
) -> Result<
    ProductionAnalysisReportValidationSessionV1<'a>,
    ProductionAnalysisReportValidationErrorV1,
> {
    begin_production_analysis_report_validation_with_observation_v1(
        context,
        function,
        atomic_target,
        preservation,
        input_census,
        limits,
        None,
    )
}

pub(crate) fn begin_production_analysis_report_validation_with_observation_v1<'a>(
    context: &'a Context,
    function: &'a FuncOp,
    atomic_target: Option<&PlironAtomicTargetContextV1>,
    preservation: PlironPassValidationHandleV1,
    input_census: ProductionAnalysisInputCensusV1,
    limits: ProductionAnalysisResourceLimitsV1,
    observer: ReportObservationV1<'_, '_, '_>,
) -> Result<
    ProductionAnalysisReportValidationSessionV1<'a>,
    ProductionAnalysisReportValidationErrorV1,
> {
    ProductionAnalysisReportValidationSessionV1::new_with_observation_v1(
        context,
        function,
        atomic_target,
        preservation,
        input_census,
        limits,
        observer,
    )
}

#[cfg(test)]
pub(crate) fn begin_production_analysis_report_validation_v1<'a>(
    context: &'a Context,
    function: &'a FuncOp,
    atomic_target: Option<&PlironAtomicTargetContextV1>,
    preservation: PlironPassValidationHandleV1,
) -> ProductionAnalysisReportValidationSessionV1<'a> {
    begin_production_analysis_report_validation_with_resource_limits_v1(
        context,
        function,
        atomic_target,
        preservation,
        ProductionAnalysisInputCensusV1::default(),
        ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
    )
    .expect("test report-validation setup fits the production hard ceiling")
}

pub(crate) trait SealedProductionAnalysisReportV1: Clone {
    fn pass(&self) -> KernelCheckPassKindV1;
    #[cfg(test)]
    fn payload_receipt_v1(
        &self,
        limits: ProductionAnalysisResourceLimitsV1,
    ) -> Result<ProductionAnalysisReportPayloadReceiptV1, ProductionAnalysisResourceLimitV1> {
        self.payload_receipt_with_observation_v1(limits, None)
    }
    fn payload_receipt_with_observation_v1(
        &self,
        limits: ProductionAnalysisResourceLimitsV1,
        observer: super::pliron_report_payload_receipt::PayloadObservationV1<'_, '_, '_>,
    ) -> Result<ProductionAnalysisReportPayloadReceiptV1, ProductionAnalysisResourceLimitV1>;
    fn try_capture_for_sealed_validation_v1(
        &self,
        receipt: ProductionAnalysisReportPayloadReceiptV1,
    ) -> Result<CapturedProductionAnalysisReportV1, ProductionAnalysisResourceLimitV1>;
}

macro_rules! impl_sealed_report {
    ($report:ty, $variant:ident, $pass:ident) => {
        impl SealedProductionAnalysisReportV1 for $report {
            fn pass(&self) -> KernelCheckPassKindV1 {
                KernelCheckPassKindV1::$pass
            }

            fn payload_receipt_with_observation_v1(
                &self,
                limits: ProductionAnalysisResourceLimitsV1,
                observer: super::pliron_report_payload_receipt::PayloadObservationV1<'_, '_, '_>,
            ) -> Result<ProductionAnalysisReportPayloadReceiptV1, ProductionAnalysisResourceLimitV1>
            {
                match observer {
                    None => self.validation_payload_receipt_v1(limits),
                    Some(observer) => {
                        self.validation_payload_receipt_with_observation_v1(limits, Some(observer))
                    }
                }
            }

            fn try_capture_for_sealed_validation_v1(
                &self,
                receipt: ProductionAnalysisReportPayloadReceiptV1,
            ) -> Result<CapturedProductionAnalysisReportV1, ProductionAnalysisResourceLimitV1> {
                receipt.require_pass(self.pass())?;
                Ok(CapturedProductionAnalysisReportV1::$variant(
                    if receipt.is_exact() {
                        self.try_clone_validation_payload_v1()?
                    } else {
                        self.clone()
                    },
                ))
            }
        }
    };
}

macro_rules! impl_conservative_sealed_report {
    ($report:ty, $variant:ident, $pass:ident) => {
        impl SealedProductionAnalysisReportV1 for $report {
            fn pass(&self) -> KernelCheckPassKindV1 {
                KernelCheckPassKindV1::$pass
            }

            fn payload_receipt_with_observation_v1(
                &self,
                _limits: ProductionAnalysisResourceLimitsV1,
                _observer: super::pliron_report_payload_receipt::PayloadObservationV1<'_, '_, '_>,
            ) -> Result<ProductionAnalysisReportPayloadReceiptV1, ProductionAnalysisResourceLimitV1>
            {
                Ok(ProductionAnalysisReportPayloadReceiptV1::fallback(
                    self.pass(),
                    0,
                ))
            }

            fn try_capture_for_sealed_validation_v1(
                &self,
                receipt: ProductionAnalysisReportPayloadReceiptV1,
            ) -> Result<CapturedProductionAnalysisReportV1, ProductionAnalysisResourceLimitV1> {
                receipt.require_pass(self.pass())?;
                if receipt.is_exact() {
                    return Err(payload_limit_v1("unsupported exact report payload"));
                }
                Ok(CapturedProductionAnalysisReportV1::$variant(self.clone()))
            }
        }
    };
}

impl_sealed_report!(PlironTensorLayoutReportV1, TensorLayout, TensorLayout);
impl_sealed_report!(RankedBoundsReportV1, Bounds, MemoryBounds);
impl_sealed_report!(PlironAtomicLegalityReportV1, Atomic, AtomicLegality);
impl_sealed_report!(RankedRaceReportV1, Race, RaceFreedom);
impl_conservative_sealed_report!(
    HierarchicalOwnershipReportV1,
    Ownership,
    HierarchicalOwnership
);
impl_sealed_report!(PlironBarrierReportV1, Barrier, BarrierConvergence);
impl_conservative_sealed_report!(PlironPipelineProtocolReportV1, Pipeline, PipelineProtocol);
impl_sealed_report!(PlironWorkgroupMemoryReportV1, Workgroup, WorkgroupMemory);
impl_sealed_report!(
    PlironSemanticRefinementReportV1,
    Semantic,
    SemanticRefinement
);

// Borrowed input grouping only; the session still authenticates the endpoint.
pub(crate) struct ProductionAnalysisReportEndpointV1<'a> {
    pub(crate) context: &'a Context,
    pub(crate) function: &'a FuncOp,
}

impl<'a> ProductionAnalysisReportValidationSessionV1<'a> {
    #[cfg(test)]
    pub(crate) fn record_with_resource_limits_v1<R: SealedProductionAnalysisReportV1>(
        &mut self,
        endpoint: ProductionAnalysisReportEndpointV1<'_>,
        checkpoint: PlironPassCheckpointTokenV1,
        report: &R,
        producing_phase_upper_bound: ProductionAnalysisResourceUpperBoundV1,
        limits: ProductionAnalysisResourceLimitsV1,
        analyses: &mut PlironAnalysisManagerV1,
    ) -> Result<(), ProductionAnalysisReportValidationErrorV1> {
        self.record_with_observation_v1(
            endpoint,
            checkpoint,
            report,
            producing_phase_upper_bound,
            limits,
            (analyses, None),
        )
    }

    pub(crate) fn record_with_observation_v1<R: SealedProductionAnalysisReportV1>(
        &mut self,
        endpoint: ProductionAnalysisReportEndpointV1<'_>,
        checkpoint: PlironPassCheckpointTokenV1,
        report: &R,
        producing_phase_upper_bound: ProductionAnalysisResourceUpperBoundV1,
        limits: ProductionAnalysisResourceLimitsV1,
        observed_analyses: (
            &mut PlironAnalysisManagerV1,
            ReportObservationV1<'_, '_, '_>,
        ),
    ) -> Result<(), ProductionAnalysisReportValidationErrorV1> {
        let (analyses, observer) = observed_analyses;
        with_report_observation_v1(observer, |observer| {
            let ProductionAnalysisReportEndpointV1 { context, function } = endpoint;
            let configuration_items = match &self.atomic_configuration {
                ProductionAnalysisConfigurationV1::AtomicTarget { capabilities } => {
                    capabilities.len()
                }
                ProductionAnalysisConfigurationV1::FixedByImplementation
                | ProductionAnalysisConfigurationV1::AtomicTargetAgnostic => 0,
            };
            let stage_error =
                |error| observed_report_resource_error_v1(observer, Some(report.pass()), error);
            let payload_receipt = report
                .payload_receipt_with_observation_v1(limits, observer)
                .map_err(stage_error)?;
            let resource_upper_bound = report_validation_stage_resource_upper_bound_v1(
                producing_phase_upper_bound,
                payload_receipt,
                configuration_items,
                report.pass(),
                self.input_census,
                limits,
            )
            .map_err(stage_error)?;
            super::pliron_pipeline::invocation_receipt_v1::require_observed_v1(
                limits,
                ProductionAnalysisResourcePhaseV1::ReportValidation,
                Ok(resource_upper_bound),
                observer,
            )
            .map_err(stage_error)?;
            let bound = self.issue_with_payload_receipt_v1(
                context,
                function,
                checkpoint,
                report
                    .try_capture_for_sealed_validation_v1(payload_receipt)
                    .map_err(stage_error)?,
                payload_receipt,
                observer,
            )?;
            self.accept_with_resource_upper_bound_v1(
                context,
                function,
                &bound,
                resource_upper_bound,
                analyses,
                observer,
            )
        })
    }

    #[cfg(test)]
    pub(crate) fn record<R: SealedProductionAnalysisReportV1>(
        &mut self,
        context: &Context,
        function: &FuncOp,
        checkpoint: PlironPassCheckpointTokenV1,
        report: &R,
    ) -> Result<(), ProductionAnalysisReportValidationErrorV1> {
        let mut analyses = PlironAnalysisManagerV1::new(function);
        let payload_receipt = report
            .payload_receipt_v1(ProductionAnalysisResourceLimitsV1::production_hard_ceiling())
            .map_err(|error| report_stage_resource_error_v1(report.pass(), error))?;
        let producing_phase_upper_bound = ProductionAnalysisResourceUpperBoundV1::checked_phase(
            ProductionAnalysisResourcePhaseV1::ReportValidation,
            1,
            payload_receipt.owner_storage_for_test_v1(),
            0,
        )
        .map_err(report_resource_error_v1)?;
        self.record_with_resource_limits_v1(
            ProductionAnalysisReportEndpointV1 { context, function },
            checkpoint,
            report,
            producing_phase_upper_bound,
            ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
            &mut analyses,
        )
    }

    #[cfg(test)]
    pub(crate) fn finish_validation(
        self,
        preservation: &PlironPassPreservationReportV1,
    ) -> Result<ProductionAnalysisReportValidationV1, ProductionAnalysisReportValidationErrorV1>
    {
        self.finish_validation_with_observation_v1(preservation, None)
    }

    pub(crate) fn finish_validation_with_observation_v1(
        self,
        preservation: &PlironPassPreservationReportV1,
        observer: ReportObservationV1<'_, '_, '_>,
    ) -> Result<ProductionAnalysisReportValidationV1, ProductionAnalysisReportValidationErrorV1>
    {
        with_report_observation_v1(observer, |_| self.finish(preservation))
    }
}

fn report_resource_error_v1(
    error: ProductionAnalysisResourceLimitV1,
) -> ProductionAnalysisReportValidationErrorV1 {
    ProductionAnalysisReportValidationErrorV1::ResourceLimit {
        producing_pass: None,
        resource: error.resource,
    }
}

fn report_stage_resource_error_v1(
    producing_pass: KernelCheckPassKindV1,
    error: ProductionAnalysisResourceLimitV1,
) -> ProductionAnalysisReportValidationErrorV1 {
    ProductionAnalysisReportValidationErrorV1::ResourceLimit {
        producing_pass: Some(producing_pass),
        resource: error.resource,
    }
}

fn report_validation_setup_resource_upper_bound_v1(
    configuration_items: usize,
) -> Result<ProductionAnalysisResourceUpperBoundV1, ProductionAnalysisResourceLimitV1> {
    const FIXED_SESSION_INITIALIZATION_WORK_V1: usize = 8;
    const FIXED_SESSION_STORAGE_V1: usize = 1;

    let phase = ProductionAnalysisResourcePhaseV1::ReportValidation;
    let overflow = |resource| ProductionAnalysisResourceLimitV1 { phase, resource };
    // Fixed work initializes custody, subject handles, configuration tag,
    // progress counter, stage roster, setup receipt, latest-stage receipt, and
    // the invariant borrow. Each target capability is then copied exactly once.
    let work_upper_bound = configuration_items
        .checked_add(FIXED_SESSION_INITIALIZATION_WORK_V1)
        .ok_or_else(|| overflow("report validation setup work upper bound"))?;
    // All nine stage slots are reserved up front and coexist with the copied
    // target capability roster and the fixed session owner.
    let retained_storage_upper_bound = configuration_items
        .checked_add(PRODUCTION_ANALYSIS_REPORT_COUNT_V1)
        .and_then(|storage| storage.checked_add(FIXED_SESSION_STORAGE_V1))
        .ok_or_else(|| overflow("report validation setup storage upper bound"))?;
    ProductionAnalysisResourceUpperBoundV1::checked_phase(
        phase,
        work_upper_bound,
        retained_storage_upper_bound,
        0,
    )
}

fn report_validation_stage_resource_upper_bound_v1(
    producing_phase_upper_bound: ProductionAnalysisResourceUpperBoundV1,
    payload_receipt: ProductionAnalysisReportPayloadReceiptV1,
    configuration_items: usize,
    pass: KernelCheckPassKindV1,
    input_census: ProductionAnalysisInputCensusV1,
    limits: ProductionAnalysisResourceLimitsV1,
) -> Result<ProductionAnalysisResourceUpperBoundV1, ProductionAnalysisResourceLimitV1> {
    const CONFIGURATION_WORK_STREAMS_V1: usize = 6;
    const FIXED_CHECKPOINT_AND_WITNESS_FIELDS_V1: usize = 32;
    const TEMPORARY_REPORT_PAYLOAD_OWNERS_V1: usize = 2;
    const TEMPORARY_CONFIGURATION_OWNERS_V1: usize = 3;

    let phase = ProductionAnalysisResourcePhaseV1::ReportValidation;
    let overflow = |resource| ProductionAnalysisResourceLimitV1 { phase, resource };
    let (payload_work, report_items) = payload_receipt.payload_bounds(
        pass,
        producing_phase_upper_bound.retained_storage_upper_bound(),
    )?;
    // The producer-owned report and caches remain separately reserved. Only
    // validation's actual clone owners use the exact payload receipt.
    let work_upper_bound = Some(payload_work)
        .and_then(|work| {
            configuration_items
                .checked_mul(CONFIGURATION_WORK_STREAMS_V1)
                .and_then(|configuration| work.checked_add(configuration))
        })
        .and_then(|work| work.checked_add(FIXED_CHECKPOINT_AND_WITNESS_FIELDS_V1))
        .ok_or_else(|| overflow("report validation work upper bound"))?;
    // One report-shaped witness and two configuration copies (the stage and
    // its witness) survive in the stage result. During validation two
    // additional report payloads and three configuration copies can coexist.
    let retained_storage_upper_bound = configuration_items
        .checked_mul(2)
        .and_then(|configuration| report_items.checked_add(configuration))
        .and_then(|storage| storage.checked_add(1))
        .ok_or_else(|| overflow("report validation retained storage upper bound"))?;
    let temporary_storage_upper_bound = report_items
        .checked_mul(TEMPORARY_REPORT_PAYLOAD_OWNERS_V1)
        .and_then(|storage| {
            configuration_items
                .checked_mul(TEMPORARY_CONFIGURATION_OWNERS_V1)
                .and_then(|configuration| storage.checked_add(configuration))
        })
        .and_then(|storage| storage.checked_add(1))
        .ok_or_else(|| overflow("report validation temporary storage upper bound"))?;
    let report_bound = ProductionAnalysisResourceUpperBoundV1::checked_phase(
        phase,
        work_upper_bound,
        retained_storage_upper_bound,
        temporary_storage_upper_bound,
    )?;
    let witness_bound =
        preflight_production_analysis_witness_resource_upper_bound_v1(pass, input_census, limits)?;
    report_bound.checked_with_nested_sequence_retain(&[witness_bound], phase)
}
