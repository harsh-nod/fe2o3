use crate::production_analysis::{
    ConditionalPipelineOutcomeV1, ConditionalPipelineReportV1, PipelineErrorV1,
    invocation_receipt_v1::{
        InvocationObservationV1, InvocationReceiptFailureV1, InvocationReceiptV1,
    },
    run_conditional_production_checks_with_observation_v1,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as CanonicalBudget;
use std::cell::{Cell, RefCell};

type CanonicalAccountV1<'a, 'work> = RefCell<&'a mut CanonicalBudget<'work>>;

fn canonical_limit_v1(phase: Phase) -> ProductionAnalysisResourceLimitV1 {
    ProductionAnalysisResourceLimitV1 {
        phase,
        resource: "original canonical conditional ledger",
    }
}

// The constructor is private to the consuming pending owner. A fresh pipeline
// must still compare its initial snapshot and epoch against this retained input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ConditionalPendingSubjectV1 {
    owner: ContextIdentity,
    stage: StageIdentityV1,
    root: RootIdentityV1,
    epoch: u64,
}

pub(crate) struct ConditionalPipelineSubjectV1<'a> {
    pending: &'a ProductionConditionalRankedAnalysisV1,
    recipe: &'a ProductionRankedKernelV1,
    function: FuncOp,
    census: crate::production_analysis::ProductionAnalysisInputCensusV1,
    occurrences: Vec<crate::production_analysis::conditional_execution_v1::OccurrenceV1>,
}

impl ConditionalPipelineSubjectV1<'_> {
    pub(crate) fn pending_subject(&self) -> ConditionalPendingSubjectV1 {
        ConditionalPendingSubjectV1 {
            owner: self.pending._stage.owner,
            stage: self.pending._stage.identity,
            root: self.pending._root.identity,
            epoch: self.epoch(),
        }
    }

    pub(crate) fn context(&self) -> &pliron::context::Context {
        &self.pending._session.inner.context
    }

    pub(crate) fn function(&self) -> &FuncOp {
        &self.function
    }

    pub(crate) fn recipe(&self) -> &ProductionRankedKernelV1 {
        self.recipe
    }

    pub(crate) fn census(&self) -> crate::production_analysis::ProductionAnalysisInputCensusV1 {
        self.census
    }

    pub(crate) fn epoch(&self) -> u64 {
        self.pending.analysis._mutation_epoch
    }

    pub(crate) fn identity(&self) -> &BuiltIdentityV1 {
        &self.pending.analysis._identity
    }

    pub(crate) fn retained_identity_storage_upper_bound(&self) -> usize {
        self.pending
            .analysis
            ._analyses
            .resource_upper_bound()
            .retained_storage_upper_bound()
    }

    pub(crate) fn occurrences(
        &self,
    ) -> &[crate::production_analysis::conditional_execution_v1::OccurrenceV1] {
        &self.occurrences
    }
}

impl ProductionConditionalRankedAnalysisV1 {
    #[cfg(test)]
    fn prepare_pipeline_subject_v1(
        &self,
        resources: &mut ProductionAnalysisResourceContractV1,
    ) -> Result<ConditionalPipelineSubjectV1<'_>, ProductionSessionErrorV1> {
        self.prepare_pipeline_subject_with_budget_v1(resources, None)
    }

    fn prepare_pipeline_subject_with_budget_v1(
        &self,
        resources: &mut ProductionAnalysisResourceContractV1,
        canonical: Option<&CanonicalAccountV1<'_, '_>>,
    ) -> Result<ConditionalPipelineSubjectV1<'_>, ProductionSessionErrorV1> {
        use crate::production_analysis::conditional_execution_v1::OccurrenceV1;
        let phase = Phase::PipelineVerification;
        let failure = || {
            resource(ProductionAnalysisResourceLimitV1 {
                phase,
                resource: "conditional input work or storage",
            })
        };
        let retained = self.analysis._analyses.resource_upper_bound();
        let prefix = resources.cumulative();
        if prefix.work_upper_bound() < retained.work_upper_bound()
            || prefix.retained_storage_upper_bound() < retained.retained_storage_upper_bound()
            || prefix.peak_storage_upper_bound() < retained.peak_storage_upper_bound()
        {
            return Err(resource(ProductionAnalysisResourceLimitV1 {
                phase,
                resource: "conditional pending owner floor",
            }));
        }
        let rows = self.analysis.payload.rows();
        let requests = &self.analysis.selections;
        let work = rows
            .len()
            .checked_add(requests.len())
            .and_then(|n| n.checked_mul(requests.len()))
            .and_then(|n| n.checked_add(rows.len()))
            .and_then(|n| n.checked_add(1))
            .and_then(|n| n.checked_mul(64))
            .ok_or_else(failure)?;
        let storage = requests
            .len()
            .checked_mul(std::mem::size_of::<OccurrenceV1>())
            .and_then(|n| n.checked_add(std::mem::size_of::<ConditionalPipelineSubjectV1<'_>>()))
            .and_then(|n| {
                n.checked_add(std::mem::size_of::<ProductionConditionalPipelineAnalysisV1>())
            })
            .ok_or_else(failure)?;
        if let Some(account) = canonical {
            let mut budget = account.borrow_mut();
            budget
                .charge_work(work)
                .map_err(|_| resource(canonical_limit_v1(phase)))?;
            budget
                .reserve_storage(storage)
                .map_err(|_| resource(canonical_limit_v1(phase)))?;
        }
        resources
            .admit_retained(
                phase,
                Bound::checked_phase(phase, work, storage, 0).map_err(resource)?,
            )
            .map_err(resource)?;

        let recipe = self.kernel()?;
        let record = self
            ._session
            .constructed_roots
            .get(&self._stage.identity)
            .ok_or(ProductionSessionErrorV1::StaleStage)?;
        let function = FuncOp::from_operation(
            record
                .ranked_function
                .ok_or(ProductionSessionErrorV1::WrongConstructionKind)?,
        );
        let census = self
            .analysis
            ._analyses
            .input_census()
            .ok_or(ProductionSessionErrorV1::RankedGraphChanged)?;
        if requests.is_empty()
            || requests.len() > crate::MAX_HIERARCHICAL_OWNERSHIP_CONTRACTS_V1
            || record.ownership_occurrences.len() != rows.len()
            || record.ownership_occurrences.len() != census.ownership_contracts
            || record
                .ownership_occurrences
                .iter()
                .zip(rows)
                .any(|(occurrence, row)| {
                    occurrence.operation != row.operation()
                        || occurrence.view != row.view()
                        || row.selected() != requests.contains(&occurrence.site)
                })
            || self
                ._session
                .inner
                .context
                .ir_mutation_attempt_epoch()
                .map_err(|_| ProductionSessionErrorV1::RankedGraphChanged)?
                .value()
                != self.analysis._mutation_epoch
        {
            return Err(ProductionSessionErrorV1::RankedGraphChanged);
        }
        let exact = match canonical {
            Some(account) => {
                middle_end_evidence_v4::derive_exact_ranked_graph_identity_with_canonical_budget_v1(
                    recipe,
                    resources,
                    &mut account.borrow_mut(),
                )
            }
            None => middle_end_evidence_v4::derive_exact_ranked_graph_identity_with_resources_v1(
                recipe, resources,
            ),
        }
        .map(ProductionExactGraphIdentityV1)
        .map_err(resource)?;
        if Some(exact) != record.exact_graph_identity {
            return Err(ProductionSessionErrorV1::RankedGraphChanged);
        }

        // Also reached by require_current_graph_v1, on the caller's original
        // canonical account. Exact recipe identity alone does not replay reads.
        let replay =
            super::conditional_read_occurrences_v1::replay_bound_v1(census).map_err(resource)?;
        if let Some(account) = canonical {
            account
                .borrow_mut()
                .charge_work(replay.work_upper_bound())
                .map_err(|_| resource(canonical_limit_v1(phase)))?;
        }
        resources.admit_retained(phase, replay).map_err(resource)?;
        super::conditional_read_occurrences_v1::replay_read_occurrences_v1(
            &self._session.inner.context,
            &function,
            recipe,
            &record.read_occurrences,
            self.analysis._mutation_epoch,
            census,
        )?;

        let mut occurrences = Vec::new();
        occurrences
            .try_reserve_exact(requests.len())
            .map_err(|_| failure())?;
        if occurrences.capacity() != requests.len() {
            return Err(failure());
        }
        for (index, request) in requests.iter().enumerate() {
            if requests[..index].contains(request) {
                return Err(selection(
                    index,
                    "duplicate conditional pipeline occurrence",
                ));
            }
            let original = record
                .ownership_occurrences
                .iter()
                .find(|occurrence| occurrence.site == *request)
                .ok_or_else(|| {
                    selection(index, "missing original conditional pipeline occurrence")
                })?;
            occurrences.push((original.site, original.operation, original.view));
        }
        Ok(ConditionalPipelineSubjectV1 {
            pending: self,
            recipe,
            function,
            census,
            occurrences,
        })
    }
}

/// Two fresh graph-analysis runs, retained with their original pending owner.
/// This is not an independently proved theorem or a lowering/launch capability.
/// Construction, source/CPU joins and physical-address premises remain separate.
///
/// ```compile_fail
/// use fe2o3_pliron::{ProductionConditionalPipelineAnalysisV1, ProductionRankedKernelLoweringInputV1};
/// fn promote(value: ProductionConditionalPipelineAnalysisV1) -> ProductionRankedKernelLoweringInputV1 {
///     value.into()
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_pliron::ProductionConditionalPipelineAnalysisV1;
/// fn duplicate(value: ProductionConditionalPipelineAnalysisV1) { let _ = value.clone(); }
/// ```
///
/// ```compile_fail
/// use fe2o3_pliron::{ProductionConditionalPipelineAnalysisV1, ProductionConditionalRankedAnalysisV1};
/// fn recover(value: ProductionConditionalPipelineAnalysisV1) -> ProductionConditionalRankedAnalysisV1 {
///     value.pending
/// }
/// ```
pub struct ProductionConditionalPipelineAnalysisV1 {
    // Rust drops fields in declaration order: every graph-relative owner must
    // leave before the pending owner destroys the original arena/session.
    first: Option<ConditionalPipelineOutcomeV1>,
    replay: Option<ConditionalPipelineOutcomeV1>,
    input: Option<RetainedConditionalInputV1>,
    observations: [Option<ConditionalInvocationHistoryV1>; 2],
    resources: Bound,
    caught_panic: bool,
    pending: ProductionConditionalRankedAnalysisV1,
}

struct RetainedConditionalInputV1 {
    _function: FuncOp,
    _census: crate::production_analysis::ProductionAnalysisInputCensusV1,
    _occurrences: Vec<crate::production_analysis::conditional_execution_v1::OccurrenceV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ConditionalInvocationHistoryV1 {
    floor: Bound,
    observation: InvocationObservationV1,
}

impl ProductionConditionalPipelineAnalysisV1 {
    pub(in crate::production) fn into_pending_analysis_v1(
        self,
    ) -> ProductionConditionalRankedAnalysisV1 {
        let Self {
            first,
            replay,
            input,
            observations,
            pending,
            ..
        } = self;
        drop((first, replay, input, observations));
        pending
    }

    pub fn kernel(&self) -> Result<&ProductionRankedKernelV1, ProductionSessionErrorV1> {
        self.pending.kernel()
    }

    /// Borrows the unchanged original diagnostics and recipe owner. This does
    /// not move it out, recover its session, or grant mutation access.
    pub fn pending_analysis(&self) -> &ProductionConditionalRankedAnalysisV1 {
        &self.pending
    }

    pub fn retained_analysis_storage_v1(&self) -> usize {
        self.resources.retained_storage_upper_bound()
    }

    /// Rechecks the original final graph, occurrence bindings and mutation epoch.
    /// This neither reruns a transform nor upgrades ordinary report counters.
    pub fn require_current_graph_v1(
        &self,
        budget: &mut CanonicalBudget<'_>,
    ) -> Result<(), ProductionSessionErrorV1> {
        if self.caught_panic
            || self.first.is_none()
            || self.replay.is_some()
            || self.input.is_none()
            || self.observations.iter().any(Option::is_none)
        {
            return Err(ProductionSessionErrorV1::RankedGraphChanged);
        }
        let floor = budget.storage();
        let account = RefCell::new(budget);
        let result = catch_unwind(AssertUnwindSafe(|| {
            let mut resources = ProductionAnalysisResourceContractV1::new(
                self.pending._session.analysis_resource_limits(),
            );
            resources
                .admit_retained(Phase::PipelineVerification, self.resources)
                .map_err(resource)?;
            self.pending
                .prepare_pipeline_subject_with_budget_v1(&mut resources, Some(&account))
                .map(drop)
        }));
        let mut budget = account.borrow_mut();
        let released = budget
            .storage()
            .checked_sub(floor)
            .ok_or_else(|| resource(canonical_limit_v1(Phase::PipelineVerification)))?;
        budget
            .release_storage(released)
            .map_err(|_| resource(canonical_limit_v1(Phase::PipelineVerification)))?;
        match result {
            Ok(result) => result,
            Err(_) => Err(ProductionSessionErrorV1::RankedGraphChanged),
        }
    }
}

/// Keeps the original pending owner and any completed diagnostic reports alive.
/// There is no conversion back into a session or an executable lowering input.
pub struct ProductionConditionalPipelineErrorV1 {
    failure: ConditionalPipelineFailureV1,
    analysis: ProductionConditionalPipelineAnalysisV1,
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
enum ConditionalPipelineFailureV1 {
    Session(ProductionSessionErrorV1),
    Pipeline(PipelineErrorV1),
    Resource(ProductionAnalysisResourceLimitV1),
    Receipt(InvocationReceiptFailureV1),
    InvocationAccounting,
    ReplayResources,
    ReplayPayload,
    CaughtPanic,
}

impl From<ProductionAnalysisResourceLimitV1> for ConditionalPipelineFailureV1 {
    fn from(error: ProductionAnalysisResourceLimitV1) -> Self {
        Self::Resource(error)
    }
}

impl fmt::Debug for ProductionConditionalPipelineErrorV1 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("ProductionConditionalPipelineErrorV1")
            .field("failure", &self.failure)
            .field("resources", &self.analysis.resources)
            .field("observations", &self.analysis.observations)
            .field("caught_panic", &self.analysis.caught_panic)
            .finish_non_exhaustive()
    }
}

impl fmt::Display for ProductionConditionalPipelineErrorV1 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.failure {
            ConditionalPipelineFailureV1::Session(error) => error.fmt(out),
            ConditionalPipelineFailureV1::Pipeline(error) => error.fmt(out),
            ConditionalPipelineFailureV1::Receipt(error) => {
                write!(out, "conditional invocation receipt: {error:?}")
            }
            ConditionalPipelineFailureV1::Resource(error) => write!(
                out,
                "conditional pipeline {}: {}",
                error.phase.code(),
                error.resource
            ),
            other => write!(out, "conditional pipeline: {other:?}"),
        }
    }
}

impl Error for ProductionConditionalPipelineErrorV1 {}

impl ProductionConditionalPipelineErrorV1 {
    pub fn pending_analysis(&self) -> &ProductionConditionalRankedAnalysisV1 {
        &self.analysis.pending
    }
}

impl ProductionConditionalRankedAnalysisV1 {
    /// Consumes this frozen owner into a two-run conditional graph analysis.
    /// The mode, original bindings and atomic target come only from this owner.
    /// Neither report equality nor this graph-only API proves CPU equivalence.
    #[allow(clippy::result_large_err)]
    pub fn check_pipeline_v1(
        self,
    ) -> Result<ProductionConditionalPipelineAnalysisV1, ProductionConditionalPipelineErrorV1> {
        let limits = self._session.analysis_resource_limits();
        self.check_pipeline_with_limits_v1(limits)
    }

    /// Adds all new pipeline work to the original canonical ledger before it
    /// executes. The caller already retains this pending owner's storage floor.
    #[allow(clippy::result_large_err)]
    pub fn check_pipeline_with_budget_v1(
        self,
        budget: &mut CanonicalBudget<'_>,
    ) -> Result<ProductionConditionalPipelineAnalysisV1, ProductionConditionalPipelineErrorV1> {
        let limits = self._session.analysis_resource_limits();
        self.check_pipeline_accounted_v1(limits, Some(budget))
    }

    #[allow(clippy::result_large_err)]
    fn check_pipeline_with_limits_v1(
        self,
        limits: ProductionAnalysisResourceLimitsV1,
    ) -> Result<ProductionConditionalPipelineAnalysisV1, ProductionConditionalPipelineErrorV1> {
        self.check_pipeline_accounted_v1(limits, None)
    }

    #[allow(clippy::result_large_err)]
    fn check_pipeline_accounted_v1(
        self,
        limits: ProductionAnalysisResourceLimitsV1,
        canonical: Option<&mut CanonicalBudget<'_>>,
    ) -> Result<ProductionConditionalPipelineAnalysisV1, ProductionConditionalPipelineErrorV1> {
        let canonical_floor = canonical.as_ref().map(|budget| budget.storage());
        let canonical = canonical.map(RefCell::new);
        #[cfg(all(test, feature = "internal-proof-staging"))]
        let replay_fault = replay_test_v1::take();
        let retained = self.analysis._analyses.resource_upper_bound();
        let mut analysis = ProductionConditionalPipelineAnalysisV1 {
            first: None,
            replay: None,
            input: None,
            observations: [None; 2],
            resources: retained,
            caught_panic: false,
            pending: self,
        };
        let mut resources = ProductionAnalysisResourceContractV1::new(limits);
        let mut admitted_pending = false;
        let result = catch_unwind(AssertUnwindSafe(
            || -> Result<(), ConditionalPipelineFailureV1> {
                let phase = Phase::PipelineVerification;
                if canonical_floor
                    .is_some_and(|floor| floor < retained.retained_storage_upper_bound())
                {
                    return Err(canonical_limit_v1(phase).into());
                }
                resources.admit_retained(phase, retained)?;
                admitted_pending = true;
                let input = analysis
                    .pending
                    .prepare_pipeline_subject_with_budget_v1(&mut resources, canonical.as_ref())
                    .map_err(ConditionalPipelineFailureV1::Session)?;
                run_owned_conditional_invocation_v1(
                    &input,
                    &mut resources,
                    limits,
                    Bound::default(),
                    &mut analysis.first,
                    &mut analysis.observations[0],
                    canonical.as_ref(),
                    #[cfg(all(test, feature = "internal-proof-staging"))]
                    None,
                )?;
                let first = analysis
                    .first
                    .as_ref()
                    .ok_or(ConditionalPipelineFailureV1::InvocationAccounting)?;
                let comparison =
                    conditional_report_comparison_bound_v1(first.resource_upper_bound)?;
                // Reserve prospective replay/comparison capacity without recording
                // either as executed work. Each real invocation supplies its history.
                limits.require(
                    phase,
                    resources
                        .cumulative()
                        .checked_then_retain(first.resource_upper_bound, phase)?
                        .checked_then_retain(comparison, phase)?,
                )?;
                run_owned_conditional_invocation_v1(
                    &input,
                    &mut resources,
                    limits,
                    comparison,
                    &mut analysis.replay,
                    &mut analysis.observations[1],
                    canonical.as_ref(),
                    #[cfg(all(test, feature = "internal-proof-staging"))]
                    replay_fault.map(|fault| (fault, first.resource_upper_bound)),
                )?;
                #[cfg(all(test, feature = "internal-proof-staging"))]
                match replay_fault {
                    Some(replay_test_v1::Fault::Resources) => {
                        let replay = analysis.replay.as_mut().unwrap();
                        replay.resource_upper_bound = replay
                            .resource_upper_bound
                            .checked_then_retain(Bound::checked_phase(phase, 1, 0, 0)?, phase)?;
                    }
                    Some(replay_test_v1::Fault::Payload) => {
                        let replay = analysis.replay.take().unwrap();
                        analysis.replay = Some(ConditionalPipelineOutcomeV1 {
                            report: replay.report.test_payload_inequality_v1(&first.report),
                            resource_upper_bound: replay.resource_upper_bound,
                        });
                    }
                    _ => {}
                }
                // Both reports are alive here. Admission precedes even resource Eq.
                if let Some(account) = &canonical {
                    account
                        .borrow_mut()
                        .charge_work(comparison.work_upper_bound())
                        .map_err(|_| canonical_limit_v1(phase))?;
                }
                resources.admit_retained(phase, comparison)?;
                let replay = analysis
                    .replay
                    .as_ref()
                    .ok_or(ConditionalPipelineFailureV1::InvocationAccounting)?;
                if replay.resource_upper_bound != first.resource_upper_bound {
                    return Err(ConditionalPipelineFailureV1::ReplayResources);
                }
                if replay.report != first.report {
                    return Err(ConditionalPipelineFailureV1::ReplayPayload);
                }
                let released = replay.resource_upper_bound.retained_storage_upper_bound();
                drop(analysis.replay.take());
                resources.admit_replacement(phase, released, Bound::default())?;
                analysis.input = Some(RetainedConditionalInputV1 {
                    _function: input.function,
                    _census: input.census,
                    _occurrences: input.occurrences,
                });
                Ok(())
            },
        ));
        // Preparation updates this ledger as it admits A; no phase is recovered
        // by subtracting cumulative work or historical peaks after an error.
        if admitted_pending {
            analysis.resources = resources.cumulative();
        }
        analysis.caught_panic = result.is_err()
            || analysis
                .observations
                .iter()
                .flatten()
                .any(|history| history.observation.caught_panic);
        if analysis.caught_panic {
            analysis.pending._session.poisoned = true;
        }
        let result = match (result, canonical.as_ref(), canonical_floor) {
            (result, Some(account), Some(floor)) => {
                let mut budget = account.borrow_mut();
                let settle = budget
                    .storage()
                    .checked_sub(floor)
                    .and_then(|held| {
                        analysis
                            .resources
                            .retained_storage_upper_bound()
                            .checked_sub(retained.retained_storage_upper_bound())
                            .and_then(|needed| held.checked_sub(needed))
                    })
                    .ok_or_else(|| canonical_limit_v1(Phase::PipelineVerification))
                    .and_then(|release| {
                        budget
                            .release_storage(release)
                            .map_err(|_| canonical_limit_v1(Phase::PipelineVerification))
                    });
                match settle {
                    Ok(()) => result,
                    Err(error) => Ok(Err(error.into())),
                }
            }
            (result, _, _) => result,
        };
        match result {
            Ok(Ok(())) => Ok(analysis),
            Ok(Err(failure)) => Err(ProductionConditionalPipelineErrorV1 { failure, analysis }),
            Err(_) => Err(ProductionConditionalPipelineErrorV1 {
                failure: ConditionalPipelineFailureV1::CaughtPanic,
                analysis,
            }),
        }
    }
}

fn conditional_report_comparison_bound_v1(
    first: Bound,
) -> Result<Bound, ProductionAnalysisResourceLimitV1> {
    let phase = Phase::PipelineVerification;
    // Derived Eq visits inline fields and retained dynamic payloads. F bounds
    // all of those owners; exact G==F is checked before payload Eq. No cloning
    // or dynamic comparison scratch is allocated by this fixed report family.
    let work = first
        .retained_storage_upper_bound()
        .checked_add(std::mem::size_of::<ConditionalPipelineReportV1>())
        .and_then(|n| n.checked_add(std::mem::size_of::<Bound>()))
        .and_then(|n| n.checked_mul(2))
        .and_then(|n| n.checked_add(1))
        .ok_or(ProductionAnalysisResourceLimitV1 {
            phase,
            resource: "conditional report comparison work",
        })?;
    Bound::checked_phase(phase, work, 0, 0)
}

#[allow(clippy::result_large_err)]
fn run_owned_conditional_invocation_v1(
    input: &ConditionalPipelineSubjectV1<'_>,
    resources: &mut ProductionAnalysisResourceContractV1,
    limits: ProductionAnalysisResourceLimitsV1,
    reserved_after: Bound,
    report: &mut Option<ConditionalPipelineOutcomeV1>,
    observation: &mut Option<ConditionalInvocationHistoryV1>,
    canonical: Option<&CanonicalAccountV1<'_, '_>>,
    #[cfg(all(test, feature = "internal-proof-staging"))] replay_fault: Option<(
        replay_test_v1::Fault,
        Bound,
    )>,
) -> Result<(), ConditionalPipelineFailureV1> {
    let phase = Phase::PipelineVerification;
    let floor = resources.cumulative();
    // Reserve later comparison in the local allowance, not accepted history.
    let local = limits
        .remaining_after_retained(phase, floor.checked_then_discard(reserved_after, phase)?)?;
    #[cfg(all(test, feature = "internal-proof-staging"))]
    let local = match replay_fault {
        Some((replay_test_v1::Fault::WorkShort, first)) => ProductionAnalysisResourceLimitsV1::new(
            local
                .max_work()
                .min(first.work_upper_bound().checked_sub(1).unwrap()),
            local.max_peak_storage(),
        ),
        _ => local,
    };
    let paid_work = Cell::new(0usize);
    let held_storage = Cell::new(0usize);
    let admit = |phase, prefix: Bound| {
        if let Some(account) = canonical {
            let mut budget = account.borrow_mut();
            let work = prefix
                .work_upper_bound()
                .checked_sub(paid_work.get())
                .ok_or_else(|| canonical_limit_v1(phase))?;
            budget
                .charge_work(work)
                .map_err(|_| canonical_limit_v1(phase))?;
            paid_work.set(prefix.work_upper_bound());
            let storage = prefix
                .peak_storage_upper_bound()
                .checked_sub(held_storage.get())
                .ok_or_else(|| canonical_limit_v1(phase))?;
            budget
                .reserve_storage(storage)
                .map_err(|_| canonical_limit_v1(phase))?;
            held_storage.set(prefix.peak_storage_upper_bound());
        }
        Ok(())
    };
    let mut receipt = InvocationReceiptV1::with_admission(floor, limits, &admit)?;
    let result = catch_unwind(AssertUnwindSafe(|| {
        #[cfg(all(test, feature = "internal-proof-staging"))]
        if matches!(replay_fault, Some((replay_test_v1::Fault::Panic, _))) {
            crate::production_analysis::panic_next_production_analysis_for_test_v1();
        }
        run_conditional_production_checks_with_observation_v1(
            input,
            input.pending._session.atomic_target.as_ref(),
            None,
            local,
            Some(&mut receipt),
        )
    }));
    let state = receipt.snapshot();
    *observation = Some(ConditionalInvocationHistoryV1 {
        floor,
        observation: state,
    });
    // Merge accepted history once, including failed/panicking invocations.
    // The returned manager bound is an equality check, not another admission.
    resources.admit_retained(phase, state.current)?;
    match result {
        Ok(Ok(outcome)) => *report = Some(outcome),
        Ok(Err(error)) => return Err(ConditionalPipelineFailureV1::Pipeline(error)),
        Err(_) => return Err(ConditionalPipelineFailureV1::CaughtPanic),
    }
    let complete = receipt
        .complete()
        .map_err(ConditionalPipelineFailureV1::Receipt)?;
    if report
        .as_ref()
        .is_none_or(|outcome| outcome.resource_upper_bound != complete)
    {
        return Err(ConditionalPipelineFailureV1::InvocationAccounting);
    }
    Ok(())
}

#[cfg(all(test, feature = "internal-proof-staging"))]
mod replay_test_v1 {
    use std::{cell::Cell, marker::PhantomData, rc::Rc};

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(super) enum Fault {
        WorkShort,
        Panic,
        Resources,
        Payload,
    }

    std::thread_local! {
        static SLOT: Cell<(bool, Option<Fault>)> = const { Cell::new((false, None)) };
    }

    pub(super) struct Scope(PhantomData<Rc<()>>);

    pub(super) fn install(fault: Fault) -> Scope {
        SLOT.with(|slot| {
            assert_eq!(slot.get(), (false, None));
            slot.set((true, Some(fault)));
        });
        Scope(PhantomData)
    }

    pub(super) fn take() -> Option<Fault> {
        SLOT.with(|slot| {
            let (active, fault) = slot.get();
            slot.set((active, None));
            fault
        })
    }

    impl Drop for Scope {
        fn drop(&mut self) {
            SLOT.with(|slot| slot.set((false, None)));
        }
    }
}
