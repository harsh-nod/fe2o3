use super::*;
use crate::production::{ConditionalPendingSubjectV1, ConditionalPipelineSubjectV1};
use crate::production_analysis::conditional_bounds_v1;
use crate::production_analysis::pliron_pipeline::{
    BoundConditionalInvocationV1, ProducedConditionalPayloadV1 as PayloadV1,
    ProducedConditionalStageV1,
};
#[cfg(test)]
use crate::production_analysis::{conditional_execution_v1 as own, conditional_semantic_v1 as sem};

type Bound = ProductionAnalysisResourceUpperBoundV1;
type Limit = ProductionAnalysisResourceLimitV1;
type Phase = ProductionAnalysisResourcePhaseV1;
type Manager = PlironAnalysisManagerV1;

fn with_observation_v1<T>(
    observer: ReportObservationV1<'_, '_, '_>,
    run: impl FnOnce(ReportObservationV1<'_, '_, '_>) -> Result<T, ErrorV1>,
) -> Result<T, ErrorV1> {
    let result = match observer {
        None => run(None),
        Some(observer) => observer.with_projection(&Ok, |nested| run(Some(nested))),
    };
    if let (Some(observer), Err(ErrorV1::Resource(error))) = (observer, &result) {
        observer.deny(*error);
    }
    result
}

fn admit_with_observation_v1(
    analyses: &mut Manager,
    bound: Bound,
    observer: ReportObservationV1<'_, '_, '_>,
) -> Result<(), ErrorV1> {
    if let Some(observer) = observer {
        observer.require(
            analyses.remaining_resource_limits(Phase::ReportValidation)?,
            Phase::ReportValidation,
            Ok(bound),
        )?;
    }
    analyses.admit_retained_resource_upper_bound(Phase::ReportValidation, bound)?;
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ImplementationV1 {
    ConditionalBoundsV1,
    ConditionalOwnershipV1,
    ConditionalSemanticV1,
}

impl ImplementationV1 {
    fn at(position: usize) -> Option<Self> {
        match position {
            1 => Some(Self::ConditionalBoundsV1),
            4 => Some(Self::ConditionalOwnershipV1),
            8 => Some(Self::ConditionalSemanticV1),
            _ => None,
        }
    }
}

impl PayloadV1 {
    fn implementation(&self) -> ImplementationV1 {
        match self {
            Self::Bounds(_) => ImplementationV1::ConditionalBoundsV1,
            Self::Ownership(_) => ImplementationV1::ConditionalOwnershipV1,
            Self::Semantic(_) => ImplementationV1::ConditionalSemanticV1,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
struct LeafV1 {
    checkpoint: ProductionAnalysisCheckpointV1,
    implementation: ImplementationV1,
    subject: ConditionalPendingSubjectV1,
    payload: PayloadV1,
}

#[derive(Debug, Eq, PartialEq)]
enum StageV1 {
    Ordinary(ProductionAnalysisStageValidationV1),
    Conditional(LeafV1),
}

impl StageV1 {
    fn checkpoint(&self) -> ProductionAnalysisCheckpointV1 {
        match self {
            Self::Ordinary(stage) => stage.checkpoint,
            Self::Conditional(stage) => stage.checkpoint,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum ErrorV1 {
    Resource(Limit),
    Ordinary(ProductionAnalysisReportValidationErrorV1),
    Subject,
    Family { position: usize },
    Order { expected: usize, observed: usize },
    Custody,
    Epoch,
    Checkpoint,
    Ledger,
    Manifest,
}

impl From<Limit> for ErrorV1 {
    fn from(error: Limit) -> Self {
        Self::Resource(error)
    }
}

impl From<ProductionAnalysisReportValidationErrorV1> for ErrorV1 {
    fn from(error: ProductionAnalysisReportValidationErrorV1) -> Self {
        Self::Ordinary(error)
    }
}

// Slots own actual conditional payloads once. They are neither ordinary report
// certificates nor independently checked conditional equivalence witnesses.
pub(crate) struct SessionV1<'a> {
    input: &'a ConditionalPipelineSubjectV1<'a>,
    preservation: PlironPassValidationHandleV1,
    atomic_configuration: ProductionAnalysisConfigurationV1,
    stages: Vec<Option<StageV1>>,
    next: usize,
    manager_address: usize,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ReportV1 {
    subject: ConditionalPendingSubjectV1,
    stages: Vec<Option<StageV1>>,
}

// The payload stays in slot 1 of the same live validation session. This borrow
// cannot be constructed from an ordinary clean report or copied digest bundle.
pub(crate) struct BoundsDependencyV1<'a> {
    input: &'a ConditionalPipelineSubjectV1<'a>,
    preservation: &'a PlironPassValidationHandleV1,
    checkpoint: &'a ProductionAnalysisCheckpointV1,
    manager_address: usize,
    report: &'a conditional_bounds_v1::ReportV1,
}

impl BoundsDependencyV1<'_> {
    pub(crate) fn is_clean(&self) -> bool {
        self.report.is_clean()
    }

    pub(crate) fn reads(
        &self,
    ) -> Option<&[crate::production_analysis::ConditionalLiveReadBoundV1]> {
        self.input.conditional_reads()
    }

    pub(crate) fn validate_v1(
        &self,
        input: &ConditionalPipelineSubjectV1<'_>,
        analyses: &Manager,
    ) -> Result<(), ErrorV1> {
        if !std::ptr::eq(input, self.input) {
            return Err(ErrorV1::Subject);
        }
        self.matches_endpoint_v1(
            input.context(),
            input.function(),
            input.census(),
            input.epoch(),
            analyses,
        )
    }

    pub(crate) fn matches_endpoint_v1(
        &self,
        context: &Context,
        function: &FuncOp,
        census: ProductionAnalysisInputCensusV1,
        epoch: u64,
        analyses: &Manager,
    ) -> Result<(), ErrorV1> {
        if self.manager_address != analyses as *const Manager as usize {
            return Err(ErrorV1::Ledger);
        }
        if !std::ptr::eq(context, self.input.context())
            || function.get_operation() != self.input.function().get_operation()
            || census != self.input.census()
            || analyses.input_census() != Some(census)
        {
            return Err(ErrorV1::Subject);
        }
        if epoch != self.input.epoch() || !current_epoch(self.input) {
            return Err(ErrorV1::Epoch);
        }
        if self.checkpoint.position != 1
            || self.checkpoint.pass != KernelCheckPassKindV1::MemoryBounds
            || self.checkpoint.identity != self.preservation.input_identity()
            || self.checkpoint.mutation_epoch != epoch
            || self.preservation.input_mutation_epoch() != epoch
            || !self.report.is_clean()
            || !self.report.matches_subject(self.input)
        {
            return Err(ErrorV1::Checkpoint);
        }
        Ok(())
    }
}

impl ReportV1 {
    pub(crate) fn typed_root_commitments_v1(&self) -> Option<&[[u64; 4]]> {
        match self.stages.get(8)?.as_ref()? {
            StageV1::Conditional(LeafV1 {
                payload: PayloadV1::Semantic(report),
                ..
            }) if report.is_clean() => Some(report.typed_root_commitments()),
            _ => None,
        }
    }
}

impl<'a> SessionV1<'a> {
    pub(crate) fn borrow_bounds_with_observation_v1(
        &self,
        analyses: &mut Manager,
        observer: ReportObservationV1<'_, '_, '_>,
    ) -> Result<(BoundsDependencyV1<'_>, Bound), ErrorV1> {
        with_observation_v1(observer, |observer| {
            if self.manager_address != analyses as *const Manager as usize {
                return Err(ErrorV1::Ledger);
            }
            let bound = bounds_dependency_work_v1(self.input)?;
            admit_with_observation_v1(analyses, bound, observer)?;
            let dependency = self.bounds_dependency_v1()?;
            dependency.validate_v1(self.input, analyses)?;
            Ok((dependency, bound))
        })
    }

    fn bounds_dependency_v1(&self) -> Result<BoundsDependencyV1<'_>, ErrorV1> {
        let Some(StageV1::Conditional(leaf)) = self.stages.get(1).and_then(Option::as_ref) else {
            return Err(ErrorV1::Checkpoint);
        };
        let PayloadV1::Bounds(report) = &leaf.payload else {
            return Err(ErrorV1::Family { position: 1 });
        };
        if self.next < 2
            || leaf.implementation != ImplementationV1::ConditionalBoundsV1
            || leaf.subject != self.input.pending_subject()
        {
            return Err(ErrorV1::Subject);
        }
        Ok(BoundsDependencyV1 {
            input: self.input,
            preservation: &self.preservation,
            checkpoint: &leaf.checkpoint,
            manager_address: self.manager_address,
            report,
        })
    }

    // Initial exact-byte comparison is performed by the shared pipeline before
    // this constructor; the retained input is never replaced with a fresh epoch.
    pub(crate) fn begin_with_observation_v1(
        binding: BoundConditionalInvocationV1<'a>,
        atomic_target: Option<&PlironAtomicTargetContextV1>,
        analyses: &mut Manager,
        observer: ReportObservationV1<'_, '_, '_>,
    ) -> Result<(Self, Bound), ErrorV1> {
        with_observation_v1(observer, |observer| {
            let (input, preservation, manager_address) = binding.into_parts();
            if manager_address != analyses as *const Manager as usize {
                return Err(ErrorV1::Ledger);
            }
            let count = atomic_target.map_or(0, |target| target.capabilities().len());
            let allocation = count
                .checked_mul(std::mem::size_of::<PlironAtomicTargetCapabilityV1>())
                .and_then(|n| n.checked_add(std::mem::size_of::<Self>()))
                .and_then(|n| {
                    PRODUCTION_ANALYSIS_REPORT_COUNT_V1
                        .checked_mul(std::mem::size_of::<Option<StageV1>>())
                        .and_then(|slots| n.checked_add(slots))
                })
                .ok_or_else(|| resource("conditional validation setup storage"))?;
            let work = count
                .checked_add(32)
                .ok_or_else(|| resource("conditional validation setup work"))?;
            let bound = Bound::checked_phase(Phase::ReportValidation, work, allocation, 0)?;
            admit_with_observation_v1(analyses, bound, observer)?;
            if analyses.input_census() != Some(input.census()) {
                return Err(ErrorV1::Subject);
            }
            if preservation.input_mutation_epoch() != input.epoch() || !current_epoch(input) {
                return Err(ErrorV1::Epoch);
            }
            let atomic_configuration = match atomic_target {
                None => ProductionAnalysisConfigurationV1::AtomicTargetAgnostic,
                Some(target) => {
                    let mut capabilities = Vec::new();
                    capabilities
                        .try_reserve_exact(count)
                        .map_err(|_| resource("conditional target capability allocation"))?;
                    if capabilities.capacity() != count {
                        return Err(resource("conditional target capability capacity").into());
                    }
                    capabilities.extend(target.capabilities().iter().copied());
                    ProductionAnalysisConfigurationV1::AtomicTarget { capabilities }
                }
            };
            let mut stages = Vec::new();
            stages
                .try_reserve_exact(PRODUCTION_ANALYSIS_REPORT_COUNT_V1)
                .map_err(|_| resource("conditional validation slot allocation"))?;
            if stages.capacity() != PRODUCTION_ANALYSIS_REPORT_COUNT_V1 {
                return Err(resource("conditional validation slot capacity").into());
            }
            stages.resize_with(PRODUCTION_ANALYSIS_REPORT_COUNT_V1, || None);
            Ok((
                Self {
                    input,
                    preservation,
                    atomic_configuration,
                    stages,
                    next: 0,
                    manager_address,
                },
                bound,
            ))
        })
    }

    fn subject(&self) -> ReportValidationSubjectV1<'_> {
        ReportValidationSubjectV1 {
            preservation: &self.preservation,
            context_address: self.input.context() as *const Context as usize,
            function: self.input.function().get_operation(),
            atomic_configuration: &self.atomic_configuration,
        }
    }

    fn header(
        &self,
        token: &PlironPassCheckpointTokenV1,
        analyses: &mut Manager,
        observer: ReportObservationV1<'_, '_, '_>,
    ) -> Result<(ProductionAnalysisCheckpointV1, Bound), ErrorV1> {
        if self.manager_address != analyses as *const Manager as usize {
            return Err(ErrorV1::Ledger);
        }
        let bound = Bound::checked_phase(Phase::ReportValidation, 32, 0, 0)?;
        admit_with_observation_v1(analyses, bound, observer)?;
        if analyses.input_census() != Some(self.input.census()) {
            return Err(ErrorV1::Subject);
        }
        if !self.preservation.same_custody(token) {
            return Err(ErrorV1::Custody);
        }
        if token.position() != self.next || self.next >= self.stages.len() {
            return Err(ErrorV1::Order {
                expected: self.next,
                observed: token.position(),
            });
        }
        if self.stages[self.next].is_some() {
            return Err(ErrorV1::Checkpoint);
        }
        if token.pass() != PRODUCTION_PLIRON_PASS_CONTRACTS_V1[self.next].pass()
            || token.identity() != self.preservation.input_identity()
        {
            return Err(ErrorV1::Checkpoint);
        }
        if token.mutation_epoch() != self.input.epoch()
            || self.preservation.input_mutation_epoch() != self.input.epoch()
            || !current_epoch(self.input)
        {
            return Err(ErrorV1::Epoch);
        }
        Ok((
            ProductionAnalysisCheckpointV1 {
                position: token.position(),
                pass: token.pass(),
                identity: token.identity(),
                mutation_epoch: token.mutation_epoch(),
            },
            bound,
        ))
    }

    pub(crate) fn record_ordinary_with_observation_v1<R: SealedProductionAnalysisReportV1>(
        &mut self,
        token: PlironPassCheckpointTokenV1,
        report: &R,
        producing: Bound,
        analyses: &mut Manager,
        observer: ReportObservationV1<'_, '_, '_>,
    ) -> Result<Option<Bound>, ErrorV1> {
        with_observation_v1(observer, |observer| {
            let (_, header) = self.header(&token, analyses, observer)?;
            if ImplementationV1::at(self.next).is_some() {
                return Err(ErrorV1::Family {
                    position: self.next,
                });
            }
            let count = match &self.atomic_configuration {
                ProductionAnalysisConfigurationV1::AtomicTarget { capabilities } => {
                    capabilities.len()
                }
                _ => 0,
            };
            let limits = analyses.remaining_resource_limits(Phase::ReportValidation)?;
            let run = |observer: ReportObservationV1<'_, '_, '_>| -> Result<Bound, ErrorV1> {
                let receipt = report.payload_receipt_with_observation_v1(limits, observer)?;
                let bound = report_validation_stage_resource_upper_bound_v1(
                    producing,
                    receipt,
                    count,
                    report.pass(),
                    self.input.census(),
                    limits,
                )?;
                admit_with_observation_v1(analyses, bound, observer)?;
                let subject = self.subject();
                let captured = subject.issue_at_v1(
                    self.next,
                    self.input.context(),
                    self.input.function(),
                    token,
                    report.try_capture_for_sealed_validation_v1(receipt)?,
                    (receipt, observer),
                )?;
                let stage = subject.accept_at_v1(
                    self.next,
                    self.input.context(),
                    self.input.function(),
                    &captured,
                    analyses,
                    observer,
                )?;
                self.stages[self.next] = Some(StageV1::Ordinary(stage));
                self.next += 1;
                Ok(bound)
            };
            match observer {
                None => run(None).map(|_| None),
                Some(observer) => observer.with_projection(
                    &|local| header.checked_then_retain(local, Phase::ReportValidation),
                    |nested| {
                        let bound = run(Some(nested))?;
                        // Transfer metadata is needed only by the observing caller.
                        header
                            .checked_then_retain(bound, Phase::ReportValidation)
                            .map(Some)
                            .map_err(Into::into)
                    },
                ),
            }
        })
    }

    pub(crate) fn record_produced_with_observation_v1(
        &mut self,
        produced: ProducedConditionalStageV1,
        analyses: &mut Manager,
        observer: ReportObservationV1<'_, '_, '_>,
    ) -> Result<Bound, ErrorV1> {
        with_observation_v1(observer, |observer| {
            let (subject, manager_address, token, payload) = produced.into_parts();
            if subject != self.input.pending_subject() {
                return Err(ErrorV1::Subject);
            }
            if manager_address != self.manager_address {
                return Err(ErrorV1::Ledger);
            }
            let (checkpoint, bound) = self.header(&token, analyses, observer)?;
            let implementation = payload.implementation();
            if ImplementationV1::at(self.next) != Some(implementation) {
                return Err(ErrorV1::Family {
                    position: self.next,
                });
            }
            let mut bound = bound;
            if let PayloadV1::Bounds(report) = &payload {
                let query = bounds_dependency_work_v1(self.input)?;
                let mut check = |observer: ReportObservationV1<'_, '_, '_>| -> Result<(), ErrorV1> {
                    admit_with_observation_v1(analyses, query, observer)?;
                    if !report.is_clean() || !report.matches_subject(self.input) {
                        return Err(ErrorV1::Subject);
                    }
                    Ok(())
                };
                match observer {
                    None => check(None)?,
                    Some(observer) => observer.with_projection(
                        &|local| bound.checked_then_retain(local, Phase::ReportValidation),
                        |nested| check(Some(nested)),
                    )?,
                }
                bound = bound.checked_then_retain(query, Phase::ReportValidation)?;
            }
            // The producer's reservation retains the payload; setup retains the
            // slot backing. Moving between the two introduces no allocation.
            self.stages[self.next] = Some(StageV1::Conditional(LeafV1 {
                checkpoint,
                implementation,
                subject: self.input.pending_subject(),
                payload,
            }));
            self.next += 1;
            Ok(bound)
        })
    }

    pub(crate) fn finish_with_observation_v1(
        self,
        preservation: &PlironPassPreservationReportV1,
        analyses: &mut Manager,
        observer: ReportObservationV1<'_, '_, '_>,
    ) -> Result<(ReportV1, Bound), ErrorV1> {
        with_observation_v1(observer, |observer| {
            if self.manager_address != analyses as *const Manager as usize {
                return Err(ErrorV1::Ledger);
            }
            if analyses.input_census() != Some(self.input.census()) {
                return Err(ErrorV1::Subject);
            }
            let bound = Bound::checked_phase(
                Phase::ReportValidation,
                32 * PRODUCTION_ANALYSIS_REPORT_COUNT_V1,
                0,
                0,
            )?;
            admit_with_observation_v1(analyses, bound, observer)?;
            if self.stages.len() != PRODUCTION_ANALYSIS_REPORT_COUNT_V1
                || self.next != self.stages.len()
                || !current_epoch(self.input)
                || !self.preservation.same_report_custody(preservation)
                || self.preservation.input_identity() != preservation.input_identity()
                || preservation.input_mutation_epoch() != self.input.epoch()
                || !preservation.is_exact_identity()
                || preservation.certificates().len() != self.stages.len()
            {
                return Err(ErrorV1::Manifest);
            }
            for (position, (stage, certificate)) in self
                .stages
                .iter()
                .zip(preservation.certificates())
                .enumerate()
            {
                let stage = stage.as_ref().ok_or(ErrorV1::Manifest)?;
                let checkpoint = stage.checkpoint();
                if checkpoint.position != position
                    || checkpoint.pass != certificate.pass()
                    || checkpoint.identity != certificate.identity()
                    || checkpoint.mutation_epoch != certificate.mutation_epoch()
                    || certificate.pass() != PRODUCTION_PLIRON_PASS_CONTRACTS_V1[position].pass()
                {
                    return Err(ErrorV1::Manifest);
                }
                match (stage, ImplementationV1::at(position)) {
                    (StageV1::Ordinary(_), None) => {}
                    (StageV1::Conditional(leaf), Some(expected))
                        if leaf.implementation == expected
                            && leaf.payload.implementation() == expected
                            && leaf.subject == self.input.pending_subject() => {}
                    _ => return Err(ErrorV1::Family { position }),
                }
            }
            Ok((
                ReportV1 {
                    subject: self.input.pending_subject(),
                    stages: self.stages,
                },
                bound,
            ))
        })
    }
}

fn bounds_dependency_work_v1(input: &ConditionalPipelineSubjectV1<'_>) -> Result<Bound, ErrorV1> {
    let work = input
        .census()
        .operands
        .checked_add(input.conditional_reads().map_or(0, <[_]>::len))
        .and_then(|n| n.checked_add(input.occurrences().len()))
        .and_then(|n| n.checked_mul(8))
        .and_then(|n| n.checked_add(std::mem::size_of::<ProductionAnalysisInputCensusV1>()))
        .and_then(|n| n.checked_add(128))
        .ok_or_else(|| resource("conditional bounds dependency work"))?;
    Ok(Bound::checked_phase(Phase::ReportValidation, work, 0, 0)?)
}

fn current_epoch(input: &ConditionalPipelineSubjectV1<'_>) -> bool {
    input
        .context()
        .ir_mutation_attempt_epoch()
        .ok()
        .map(|epoch| epoch.value())
        == Some(input.epoch())
}

fn resource(resource: &'static str) -> Limit {
    Limit {
        phase: Phase::ReportValidation,
        resource,
    }
}

#[cfg(test)]
mod test_helpers {
    use super::*;

    impl<'a> SessionV1<'a> {
        #[cfg(test)]
        pub(crate) fn test_swap_ownership_v1(&mut self, payload: &mut PayloadV1) {
            let Some(StageV1::Conditional(leaf)) = self.stages[4].as_mut() else {
                panic!("missing ownership")
            };
            std::mem::swap(&mut leaf.payload, payload);
        }
    }

    impl ReportV1 {
        #[cfg(all(test, feature = "internal-proof-staging"))]
        pub(crate) fn test_flip_checked_exit_v1(&mut self) {
            let Some(StageV1::Conditional(leaf)) = self.stages[4].as_mut() else {
                panic!("missing ownership")
            };
            let PayloadV1::Ownership(report) = &mut leaf.payload else {
                panic!("wrong payload")
            };
            let own::SelectedResultV1::Checked(facts) = &mut report.selected[0].result else {
                panic!("unchecked row")
            };
            facts.normal_exits[0] ^= 1;
        }

        #[cfg(test)]
        pub(crate) fn ownership(&self) -> &own::ReportV1 {
            let Some(StageV1::Conditional(LeafV1 {
                payload: PayloadV1::Ownership(report),
                ..
            })) = &self.stages[4]
            else {
                unreachable!("validated ownership slot")
            };
            report
        }

        #[cfg(test)]
        pub(crate) fn semantics(&self) -> &sem::ReportV1 {
            let Some(StageV1::Conditional(LeafV1 {
                payload: PayloadV1::Semantic(report),
                ..
            })) = &self.stages[8]
            else {
                unreachable!("validated semantic slot")
            };
            report
        }

        #[cfg(test)]
        pub(crate) const fn independently_validated(&self) -> bool {
            false
        }
    }
}
