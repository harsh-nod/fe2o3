//! Nominal conditional MemoryBounds output. Only the consuming pipeline's
//! privately constructed subject authenticates the descriptive read roster.

use super::*;
use crate::production::ConditionalPipelineSubjectV1;
use crate::production_analysis::{
    conditional_execution_v1::OccurrenceV1,
    pliron_pipeline::invocation_receipt_v1::AdditionalObservationV1,
    pliron_ranked_coverage_v1::{
        LiveReadBoundV1, RuleFactsV1, RuleRefusalV1, check_conditional_live_rule_in_phase_v1,
        preflight_live_rule_with_inputs_v1,
    },
};
use pliron::context::Ptr;
use std::mem::size_of;

type Census = ProductionAnalysisInputCensusV1;
type Bound = ProductionAnalysisResourceUpperBoundV1;
type Limit = ProductionAnalysisResourceLimitV1;
type Limits = ProductionAnalysisResourceLimitsV1;
type Phase = ProductionAnalysisResourcePhaseV1;
type Manager = PlironAnalysisManagerV1;
type PipelineObservationV1<'o, 'p, 'r> = RankedBoundsObserverV1<'o, 'p, 'r>;
type ObservationsV1<'o, 'p, 'r> = (
    PipelineObservationV1<'o, 'p, 'r>,
    AdditionalObservationV1<'o, 'p, 'r>,
);
type CoverageV1 = (OccurrenceV1, RuleFactsV1);

// Capture retains at most one inline failure. Detail fields borrow static text;
// no formatted diagnostics or per-query failure collection is allocated here.
const _: () = assert!(size_of::<Option<PresburgerFailureV1>>() <= 64);

#[derive(Debug, Eq, PartialEq)]
pub(super) struct ReadObligationV1 {
    pub(super) finding: usize,
    pub(super) block: usize,
    pub(super) operation: usize,
    pub(super) live_operation: Ptr<Operation>,
    pub(super) view: Value,
    pub(super) index: Value,
    pub(super) extent: Value,
    pub(super) read: Option<usize>,
}

pub(super) struct ReadCaptureV1 {
    obligations: Vec<ReadObligationV1>,
    terminal: Option<PresburgerFailureV1>,
    resource_denied: bool,
    pub(super) counterexample: bool,
}

impl ReadCaptureV1 {
    fn new(census: Census) -> Result<Self, Limit> {
        Ok(Self {
            obligations: reserved(obligation_capacity(census))?,
            terminal: None,
            resource_denied: false,
            counterexample: false,
        })
    }

    pub(super) fn failure(&mut self, failure: &PresburgerFailureV1) {
        self.resource_denied |= matches!(failure, PresburgerFailureV1::ResourceLimit { .. });
        if !matches!(failure, PresburgerFailureV1::Unsupported { .. }) {
            self.terminal.get_or_insert_with(|| failure.clone());
        }
    }

    pub(super) fn push(
        &mut self,
        obligation: ReadObligationV1,
    ) -> Result<(), RankedBoundsFindingV1> {
        if self.obligations.len() == self.obligations.capacity() {
            return Err(RankedBoundsFindingV1::ResourceLimitExceeded {
                resource: "conditional typed read obligations",
                limit: self.obligations.capacity(),
                actual: self.obligations.len().saturating_add(1),
            });
        }
        self.obligations.push(obligation);
        Ok(())
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum FailureV1 {
    Resource(Limit),
    Subject,
    ReadRoster,
    TerminalFinding {
        finding: usize,
    },
    ReadAssociation {
        finding: usize,
    },
    Presburger(PresburgerFailureV1),
    Counterexample,
    Coverage {
        selection: usize,
        refusal: RuleRefusalV1,
    },
}

impl From<Limit> for FailureV1 {
    fn from(error: Limit) -> Self {
        Self::Resource(error)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ErrorV1 {
    pub(crate) failure: FailureV1,
    ordinary: Option<RankedBoundsReportV1>,
}

impl ErrorV1 {
    #[cfg(test)]
    pub(crate) fn ordinary_report(&self) -> Option<&RankedBoundsReportV1> {
        self.ordinary.as_ref()
    }
}

impl fmt::Display for ErrorV1 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "conditional memory bounds: {:?}", self.failure)
    }
}

impl std::error::Error for ErrorV1 {}

/// Private fields and no Clone/Copy: custody moves to report validation once.
/// Manager identity belongs to the validation session's dependency token.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ReportV1 {
    context_address: usize,
    function: Ptr<Operation>,
    epoch: u64,
    census: Census,
    ordinary: RankedBoundsReportV1,
    reads: Option<Vec<LiveReadBoundV1>>,
    obligations: Vec<ReadObligationV1>,
    coverage: Vec<CoverageV1>,
    resources: Bound,
}

impl ReportV1 {
    #[cfg(test)]
    pub(crate) fn ordinary_report(&self) -> &RankedBoundsReportV1 {
        &self.ordinary
    }

    pub(crate) fn reads(&self) -> Option<&[LiveReadBoundV1]> {
        self.reads.as_deref()
    }

    /// O(census.operands); consumers charge the typed association traversal.
    pub(crate) fn is_clean(&self) -> bool {
        self.ordinary.findings().len() == self.obligations.len()
            && self
                .obligations
                .iter()
                .enumerate()
                .all(|(index, row)| row.finding == index && row.read.is_some())
    }

    /// Direct payload equality, O(reads + coverage), with no digest substitute.
    pub(crate) fn matches_subject(&self, input: &ConditionalPipelineSubjectV1<'_>) -> bool {
        self.context_address == input.context() as *const Context as usize
            && self.function == input.function().get_operation()
            && self.epoch == input.epoch()
            && current_epoch(input.context()) == Some(self.epoch)
            && self.census == input.census()
            && self.reads() == input.conditional_reads()
            && (self.reads.is_none()
                || self
                    .coverage
                    .iter()
                    .map(|row| row.0)
                    .eq(input.occurrences().iter().copied()))
    }

    #[cfg(test)]
    pub(crate) fn resource_upper_bound(&self) -> Bound {
        self.resources
    }
}

fn resource(name: &'static str) -> Limit {
    ranked_bounds_resource_error_v1(name)
}

fn sum(xs: &[usize]) -> Result<usize, Limit> {
    checked_ranked_bounds_sum_v1(xs, "conditional bounds arithmetic")
}

fn mul(a: usize, b: usize) -> Result<usize, Limit> {
    checked_ranked_bounds_product_v1(a, b, "conditional bounds arithmetic")
}

fn obligation_capacity(census: Census) -> usize {
    census.operands.min(MAX_RANKED_BOUNDS_FINDINGS)
}

fn reserved<T>(count: usize) -> Result<Vec<T>, Limit> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| resource("conditional bounds allocation"))?;
    if values.capacity() != count {
        return Err(resource("conditional bounds allocation capacity"));
    }
    Ok(values)
}

/// Fixed stage admission. Hold the ordinary producer's peak through the live
/// replays. Each replay separately admits its MemoryBounds query through the
/// caller's additional projection, which already includes this stage bound.
pub(crate) fn preflight_v1(
    census: Census,
    read_count: usize,
    limits: Limits,
) -> Result<Bound, Limit> {
    if read_count > census.operations {
        return Err(resource("conditional read roster exceeds operation census"));
    }
    let ordinary = preflight_ranked_bounds_resource_upper_bound_v1(census, limits)?;
    let findings = census
        .blocks
        .checked_add(census.operands)
        .ok_or_else(|| resource("conditional bounds arithmetic"))?
        .min(MAX_RANKED_BOUNDS_FINDINGS);
    let work = mul(
        128,
        sum(&[
            1,
            census.blocks,
            census.operations,
            census.operands,
            census.ownership_contracts,
            read_count,
            obligation_capacity(census),
            mul(findings, sum(&[read_count, 1])?)?,
            mul(read_count, sum(&[read_count, 1])?)?,
        ])?,
    )?;
    let retained = sum(&[
        size_of::<ReportV1>(),
        size_of::<ErrorV1>(),
        size_of::<ReadCaptureV1>(),
        mul(obligation_capacity(census), size_of::<ReadObligationV1>())?,
        mul(read_count, size_of::<LiveReadBoundV1>())?,
        mul(census.ownership_contracts, size_of::<CoverageV1>())?,
    ])?;
    let fixed = Bound::checked_phase(Phase::MemoryBounds, work, retained, 0)?;
    let joined = fixed.checked_then_retain(ordinary, Phase::MemoryBounds)?;
    let held = Bound::checked_phase(
        Phase::MemoryBounds,
        joined.work_upper_bound(),
        joined.peak_storage_upper_bound(),
        0,
    )?;
    limits.require(Phase::MemoryBounds, held)
}

fn current_epoch(context: &Context) -> Option<u64> {
    context
        .ir_mutation_attempt_epoch()
        .ok()
        .map(|epoch| epoch.value())
}

// This raw descriptor has no external constructor or callable producer API.
// Tests exercise the same implementation without manufacturing a source proof.
struct InputV1<'a> {
    context: &'a Context,
    function: &'a FuncOp,
    census: Census,
    epoch: u64,
    reads: Option<&'a [LiveReadBoundV1]>,
    occurrences: &'a [OccurrenceV1],
}

#[allow(clippy::result_large_err)]
pub(crate) fn run_preadmitted_with_observation_v1(
    input: &ConditionalPipelineSubjectV1<'_>,
    analyses: &mut Manager,
    observations: ObservationsV1<'_, '_, '_>,
) -> Result<ReportV1, ErrorV1> {
    run_observed(
        InputV1 {
            context: input.context(),
            function: input.function(),
            census: input.census(),
            epoch: input.epoch(),
            reads: input.conditional_reads(),
            occurrences: input.occurrences(),
        },
        analyses,
        observations,
    )
}

#[allow(clippy::result_large_err)]
fn run_observed(
    input: InputV1<'_>,
    analyses: &mut Manager,
    observations: ObservationsV1<'_, '_, '_>,
) -> Result<ReportV1, ErrorV1> {
    let (observer, additional) = observations;
    let result = match observer {
        None => run_inner(input, analyses, observations),
        Some(observer) => observer.with_projection(&Ok, |nested| {
            run_inner(input, analyses, (Some(nested), additional))
        }),
    };
    if let Err(ErrorV1 {
        failure: FailureV1::Resource(error),
        ..
    }) = &result
    {
        if let Some(observer) = observer {
            observer.deny(*error);
        }
        if let Some((observer, _)) = additional {
            observer.deny(*error);
        }
    }
    result
}

#[allow(clippy::result_large_err)]
fn run_inner(
    input: InputV1<'_>,
    analyses: &mut Manager,
    observations: ObservationsV1<'_, '_, '_>,
) -> Result<ReportV1, ErrorV1> {
    let (observer, additional) = observations;
    let mut ordinary = None;
    let result = (|| -> Result<ReportV1, FailureV1> {
        if analyses.input_census() != Some(input.census)
            || current_epoch(input.context) != Some(input.epoch)
            || input.occurrences.len() > input.census.ownership_contracts
        {
            return Err(FailureV1::Subject);
        }
        let count = input.reads.map_or(0, <[LiveReadBoundV1]>::len);
        let mut resources = preflight_v1(input.census, count, Limits::new(usize::MAX, usize::MAX))?;
        let mut capture = ReadCaptureV1::new(input.census)?;
        ordinary = Some(run_pliron_ranked_bounds_with_capture_v1(
            input.context,
            input.function,
            analyses,
            observer,
            input.reads.is_some().then_some(&mut capture),
        ));
        if capture.resource_denied {
            return Err(resource("Presburger query work limit").into());
        }
        if let Some(failure) = capture.terminal.take() {
            return Err(FailureV1::Presburger(failure));
        }
        if capture.counterexample {
            return Err(FailureV1::Counterexample);
        }
        associate(
            ordinary.as_ref().unwrap(),
            &mut capture.obligations,
            input.reads,
        )?;

        let mut coverage = reserved(input.census.ownership_contracts)?;
        let mut reads = input.reads.map(|rows| reserved(rows.len())).transpose()?;
        if let Some(rows) = input.reads {
            if input.occurrences.is_empty() {
                return Err(FailureV1::ReadRoster);
            }
            let inventory = analyses
                .function_inventory_handle()
                .map_err(|_| FailureV1::Subject)?;
            for (selection, occurrence) in input.occurrences.iter().copied().enumerate() {
                let facts = check_conditional_live_rule_in_phase_v1(
                    (input.context, input.function),
                    inventory.as_ref(),
                    input.census,
                    input.epoch,
                    (occurrence.1, occurrence.2),
                    Some(rows),
                    analyses,
                    additional,
                    Phase::MemoryBounds,
                )?
                .map_err(|refusal| FailureV1::Coverage { selection, refusal })?;
                coverage.push((occurrence, facts));
                let local =
                    preflight_live_rule_with_inputs_v1(input.census, count, Phase::MemoryBounds)?;
                resources = resources.checked_then_retain(local, Phase::MemoryBounds)?;
            }
            reads.as_mut().unwrap().extend_from_slice(rows);
        }
        if current_epoch(input.context) != Some(input.epoch) {
            return Err(FailureV1::Subject);
        }
        Ok(ReportV1 {
            context_address: input.context as *const Context as usize,
            function: input.function.get_operation(),
            epoch: input.epoch,
            census: input.census,
            ordinary: ordinary.take().unwrap(),
            reads,
            obligations: capture.obligations,
            coverage,
            resources,
        })
    })();
    result.map_err(|failure| ErrorV1 { failure, ordinary })
}

fn associate(
    report: &RankedBoundsReportV1,
    obligations: &mut [ReadObligationV1],
    reads: Option<&[LiveReadBoundV1]>,
) -> Result<(), FailureV1> {
    for (finding, value) in report.findings().iter().enumerate() {
        let RankedBoundsFindingV1::UnprovedBound {
            block,
            operation,
            access: AccessKindAttr::Read,
            dimension: 0,
            ..
        } = value
        else {
            return Err(FailureV1::TerminalFinding { finding });
        };
        let reads = reads.ok_or(FailureV1::ReadRoster)?;
        let row = obligations
            .get_mut(finding)
            .filter(|row| {
                row.finding == finding && row.block == *block && row.operation == *operation
            })
            .ok_or(FailureV1::ReadAssociation { finding })?;
        let mut matched = None;
        for (index, read) in reads.iter().enumerate() {
            if read.operation == row.live_operation {
                if matched.replace(index).is_some()
                    || read.view != row.view
                    || read.index != row.index
                    || read.extent != row.extent
                {
                    return Err(FailureV1::ReadAssociation { finding });
                }
            }
        }
        row.read = Some(matched.ok_or(FailureV1::ReadAssociation { finding })?);
    }
    if obligations.len() != report.findings().len() {
        return Err(FailureV1::ReadRoster);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
