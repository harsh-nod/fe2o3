//! Exact paired private proof and actual producer report at each shared checkpoint.
use super::*;
use crate::kir_bridge_v1::NativePrivateInputV1;
use crate::kir_bridge_v1::canonical_ranked_v1::private_profile::NativeCanonicalPrivateAdmissionV1;
use crate::production_analysis::canonical_ranked_checks_v1::private::PrivateOperationKindV1;
use pliron::{builtin::op_interfaces::OneRegionInterface, linked_list::ContainsLinkedList};

#[path = "canonical_private_resources_v1.rs"]
mod resources;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PrivateStageCoverageV1 {
    position: usize,
    pass: KernelCheckPassKindV1,
    epoch: u64,
    identity: crate::PlironStructuralIdentityLabelV1,
    operations: usize,
    private_counts: [usize; 5],
}

pub(crate) struct PrivateCoverageV1 {
    stages: [PrivateStageCoverageV1; 9],
}

impl PrivateCoverageV1 {
    pub(crate) fn len(&self) -> usize {
        self.stages.len()
    }
}

/// The ordinary report is diagnostic; completion additionally owns all nine
/// private coverage/checkpoint joins. This is not ordinary attachment authority.
pub struct CanonicalPrivatePipelineReportV1 {
    pub(super) report: ProductionPlironPreloweringReportV2,
    pub(super) coverage: PrivateCoverageV1,
}
impl CanonicalPrivatePipelineReportV1 {
    pub fn reports(&self) -> &ProductionPlironPreloweringReportV2 {
        &self.report
    }
    pub fn paired_stage_count(&self) -> usize {
        self.coverage.len()
    }
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

pub(crate) struct CanonicalPrivatePipelineOutcomeV1 {
    pub(crate) report: CanonicalPrivatePipelineReportV1,
    pub(crate) resource_upper_bound: ProductionAnalysisResourceUpperBoundV1,
}

pub(super) struct SessionV1<'a, A: NativePrivateInputV1 = NativeCanonicalPrivateAdmissionV1<'a>> {
    input: &'a A,
    ordinary: ProductionAnalysisReportValidationSessionV1<'a>,
    manager: usize,
    identity: crate::PlironStructuralIdentityLabelV1,
    next: usize,
    pending: Option<PrivateStageCoverageV1>,
    stages: [Option<PrivateStageCoverageV1>; 9],
    setup: ProductionAnalysisResourceUpperBoundV1,
}

impl<'a, A: NativePrivateInputV1> SessionV1<'a, A> {
    pub(super) fn begin(
        input: &'a A,
        endpoint: (&'a Context, &'a FuncOp),
        atomic_target: Option<&PlironAtomicTargetContextV1>,
        preservation: crate::PlironPassValidationHandleV1,
        census: ProductionAnalysisInputCensusV1,
        limits: ProductionAnalysisResourceLimitsV1,
        analyses: &mut PlironAnalysisManagerV1,
        observer: PipelineObservationV1<'_, '_, '_>,
    ) -> Result<Self, PipelineErrorV1> {
        let phase = ProductionAnalysisResourcePhaseV1::ReportValidation;
        let header =
            resources::setup().map_err(|e| observed_pipeline_resource_error_v1(observer, e))?;
        invocation_receipt_v1::require_observed_v1(limits, phase, Ok(header), observer)
            .map_err(|e| observed_pipeline_resource_error_v1(observer, e))?;
        if !input.authenticate(endpoint.0, endpoint.1)
            || input.epoch() != preservation.input_mutation_epoch()
            || input.operation_count() != census.operations
        {
            return Err(PipelineErrorV1::CanonicalPrivateInput);
        }
        let identity = preservation.input_identity();
        let available = limits.remaining_after_retained(phase, header)?;
        let ordinary = with_pipeline_projection_v1(
            observer,
            &|local| header.checked_then_retain(local, phase),
            |nested| {
                begin_observed_report_validation_v1(
                    endpoint.0,
                    endpoint.1,
                    atomic_target,
                    preservation,
                    census,
                    available,
                    nested,
                )
            },
        )
        .map_err(ProductionPlironPreloweringErrorV2::ReportValidation)?;
        let setup = header.checked_then_retain(ordinary.setup_resource_upper_bound_v1(), phase)?;
        Ok(Self {
            input,
            ordinary,
            manager: analyses as *const _ as usize,
            identity,
            next: 0,
            pending: None,
            stages: [None; 9],
            setup,
        })
    }

    pub(super) fn setup(&self) -> ProductionAnalysisResourceUpperBoundV1 {
        self.setup
    }

    pub(super) fn prepare(
        &mut self,
        phase: ProductionAnalysisResourcePhaseV1,
        analyses: &mut PlironAnalysisManagerV1,
        observer: PipelineObservationV1<'_, '_, '_>,
    ) -> Result<ProductionAnalysisResourceUpperBoundV1, PipelineErrorV1> {
        let bound = resources::stage(self.input, phase)
            .map_err(|e| observed_pipeline_resource_error_v1(observer, e))?;
        resources::admit(analyses, phase, bound, observer)?;
        if self.next >= 9
            || self.pending.is_some()
            || self.manager != analyses as *const _ as usize
            || !self
                .input
                .authenticate(self.input.context(), self.input.function())
        {
            return Err(PipelineErrorV1::CanonicalPrivateInput);
        }
        let pass = PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2[self.next];
        let expected_phase = match pass {
            KernelCheckPassKindV1::TensorLayout => ProductionAnalysisResourcePhaseV1::TensorLayout,
            KernelCheckPassKindV1::MemoryBounds => ProductionAnalysisResourcePhaseV1::MemoryBounds,
            KernelCheckPassKindV1::AtomicLegality => {
                ProductionAnalysisResourcePhaseV1::AtomicLegality
            }
            KernelCheckPassKindV1::RaceFreedom => ProductionAnalysisResourcePhaseV1::RaceFreedom,
            KernelCheckPassKindV1::HierarchicalOwnership => {
                ProductionAnalysisResourcePhaseV1::HierarchicalOwnership
            }
            KernelCheckPassKindV1::BarrierConvergence => {
                ProductionAnalysisResourcePhaseV1::BarrierConvergence
            }
            KernelCheckPassKindV1::PipelineProtocol => {
                ProductionAnalysisResourcePhaseV1::PipelineProtocol
            }
            KernelCheckPassKindV1::WorkgroupMemory => {
                ProductionAnalysisResourcePhaseV1::WorkgroupMemory
            }
            KernelCheckPassKindV1::SemanticRefinement => {
                ProductionAnalysisResourcePhaseV1::SemanticRefinement
            }
            _ => return Err(PipelineErrorV1::CanonicalPrivateInput),
        };
        if phase != expected_phase {
            return Err(PipelineErrorV1::CanonicalPrivateInput);
        }
        let mut counts = [0usize; 5];
        for block in self
            .input
            .function()
            .get_region(self.input.context())
            .deref(self.input.context())
            .iter(self.input.context())
        {
            for operation in block.deref(self.input.context()).iter(self.input.context()) {
                let kind = self
                    .input
                    .operation(self.input.context(), operation)
                    .ok_or(PipelineErrorV1::CanonicalPrivateInput)?;
                let slot = match kind {
                    PrivateOperationKindV1::Scalar => None,
                    PrivateOperationKindV1::Allocate => Some(0),
                    PrivateOperationKindV1::Address => Some(1),
                    PrivateOperationKindV1::Read => Some(2),
                    PrivateOperationKindV1::Write => Some(3),
                    PrivateOperationKindV1::Call => Some(4),
                    // These are not NativeData. The same sealed occurrence
                    // query checks both members of every terminal pair here,
                    // at each real stage. Calls occupy the existing call slot;
                    // ends, like other terminators, are covered by total rows.
                    PrivateOperationKindV1::TrapCall => Some(4),
                    PrivateOperationKindV1::TrapEnd
                    | PrivateOperationKindV1::LifecycleV18
                    | PrivateOperationKindV1::UnreachableV18 => None,
                };
                if let Some(slot) = slot {
                    counts[slot] += 1;
                }
            }
        }
        // Every position consumes the same sealed whole-language/callee proof,
        // not absence of findings in the ordinary reader. Bounds also consumes
        // these occurrences through its explicit private classifier.
        self.pending = Some(PrivateStageCoverageV1 {
            position: self.next,
            pass,
            epoch: self.input.epoch(),
            identity: self.identity,
            operations: self.input.operation_count(),
            private_counts: counts,
        });
        #[cfg(test)]
        tests::inject(self.next, &mut self.pending);
        let coverage = self.pending.ok_or(PipelineErrorV1::CanonicalPrivateInput)?;
        if coverage.position != self.next
            || coverage.pass != pass
            || coverage.epoch != self.input.epoch()
            || coverage.identity != self.identity
            || coverage.operations != self.input.operation_count()
            || coverage.private_counts != counts
        {
            return Err(PipelineErrorV1::CanonicalPrivateInput);
        }
        Ok(bound)
    }

    pub(super) fn record<R: SealedProductionAnalysisReportV1>(
        &mut self,
        endpoint: (&Context, &FuncOp),
        checkpoint: crate::PlironPassCheckpointTokenV1,
        report: &R,
        stage: ProductionAnalysisStageV1,
        analyses: &mut PlironAnalysisManagerV1,
        observer: PipelineObservationV1<'_, '_, '_>,
    ) -> Result<Option<ProductionAnalysisResourceUpperBoundV1>, PipelineErrorV1> {
        let phase = ProductionAnalysisResourcePhaseV1::ReportValidation;
        let header =
            resources::record().map_err(|e| observed_pipeline_resource_error_v1(observer, e))?;
        resources::admit(analyses, phase, header, observer)?;
        let coverage = self
            .pending
            .take()
            .ok_or(PipelineErrorV1::CanonicalPrivateInput)?;
        if self.manager != analyses as *const _ as usize
            || !self.input.authenticate(endpoint.0, endpoint.1)
            || coverage.position != self.next
            || checkpoint.position() != self.next
            || checkpoint.pass() != coverage.pass
            || stage.pass != coverage.pass
            || checkpoint.identity() != coverage.identity
            || checkpoint.mutation_epoch() != coverage.epoch
            || self.stages[self.next].is_some()
        {
            return Err(PipelineErrorV1::CanonicalPrivateInput);
        }
        let limits = analyses.remaining_resource_limits(phase)?;
        with_pipeline_projection_v1(
            observer,
            &|local| header.checked_then_retain(local, phase),
            |nested| {
                self.ordinary.record_with_observation_v1(
                    ProductionAnalysisReportEndpointV1 {
                        context: endpoint.0,
                        function: endpoint.1,
                    },
                    checkpoint,
                    report,
                    stage.producing_phase_upper_bound,
                    limits,
                    (analyses, nested),
                )
            },
        )
        .map_err(ProductionPlironPreloweringErrorV2::ReportValidation)?;
        let bound = self
            .ordinary
            .last_stage_resource_upper_bound_v1()
            .ok_or(PipelineErrorV1::CanonicalPrivateInput)?;
        analyses.admit_retained_resource_upper_bound(phase, bound)?;
        self.stages[self.next] = Some(coverage);
        self.next += 1;
        let total = header.checked_then_retain(bound, phase)?;
        Ok(observer.map(|_| total))
    }

    pub(super) fn finish(
        self,
        preservation: &PlironPassPreservationReportV1,
        analyses: &mut PlironAnalysisManagerV1,
        observer: PipelineObservationV1<'_, '_, '_>,
    ) -> Result<
        (
            ProductionAnalysisReportValidationV1,
            PrivateCoverageV1,
            ProductionAnalysisResourceUpperBoundV1,
        ),
        PipelineErrorV1,
    > {
        let phase = ProductionAnalysisResourcePhaseV1::ReportValidation;
        let bound =
            resources::finish().map_err(|e| observed_pipeline_resource_error_v1(observer, e))?;
        resources::admit(analyses, phase, bound, observer)?;
        if self.next != 9
            || self.pending.is_some()
            || self.stages.iter().any(Option::is_none)
            || self.manager != analyses as *const _ as usize
            || !self
                .input
                .authenticate(self.input.context(), self.input.function())
        {
            return Err(PipelineErrorV1::CanonicalPrivateInput);
        }
        for (position, coverage) in self.stages.iter().enumerate() {
            let coverage = coverage.ok_or(PipelineErrorV1::CanonicalPrivateInput)?;
            if coverage.position != position
                || coverage.pass != PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2[position]
                || coverage.epoch != self.input.epoch()
                || coverage.identity != self.identity
                || coverage.operations != self.input.operation_count()
            {
                return Err(PipelineErrorV1::CanonicalPrivateInput);
            }
        }
        let validation = self
            .ordinary
            .finish_validation_with_observation_v1(preservation, observer)
            .map_err(ProductionPlironPreloweringErrorV2::ReportValidation)?;
        let stages = self
            .stages
            .map(|stage| stage.expect("all nine paired stages checked"));
        Ok((validation, PrivateCoverageV1 { stages }, bound))
    }
}

pub(crate) fn run(
    input: &NativeCanonicalPrivateAdmissionV1<'_>,
    limits: ProductionAnalysisResourceLimitsV1,
    receipt: Option<&mut invocation_receipt_v1::InvocationReceiptV1>,
) -> Result<CanonicalPrivatePipelineOutcomeV1, PipelineErrorV1> {
    match run_shared_production_checks_v1(
        input.context(),
        input.function(),
        None,
        None,
        limits,
        (PipelineFamilyV1::CanonicalPrivate(input), receipt),
        #[cfg(test)]
        None,
    )? {
        PipelineOutcomeV1::CanonicalPrivate(outcome) => Ok(outcome),
        _ => Err(PipelineErrorV1::CanonicalPrivateInput),
    }
}

pub(crate) fn run_v18(
    input: &crate::kir_bridge_v1::NativeCanonicalPrivateAdmissionV18<'_>,
    limits: ProductionAnalysisResourceLimitsV1,
    receipt: Option<&mut invocation_receipt_v1::InvocationReceiptV1>,
) -> Result<CanonicalPrivatePipelineOutcomeV1, PipelineErrorV1> {
    match run_shared_production_checks_v1(
        input.context(),
        input.function(),
        None,
        None,
        limits,
        (PipelineFamilyV1::CanonicalPrivateV18(input), receipt),
        #[cfg(test)]
        None,
    )? {
        PipelineOutcomeV1::CanonicalPrivate(outcome) => Ok(outcome),
        _ => Err(PipelineErrorV1::CanonicalPrivateInput),
    }
}

#[cfg(test)]
#[path = "../canonical_private_pipeline_v1_tests.rs"]
mod tests;
