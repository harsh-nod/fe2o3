use super::*;
use crate::production_analysis::conditional_execution_v1 as own;
use crate::production_analysis::pliron_effect_refinement::conditional_v1 as effect;
use pliron::op::Op;

use crate::production_analysis::invocation_receipt_v1::AdditionalObservationV1;

struct ModeV1<'a, 'b, 'o, 'p, 'r>(
    effect::PreparedV1<'a>,
    AdditionalObservationV1<'o, 'p, 'r>,
    &'b own::BoundsSourceV1<'b>,
);

impl SemanticModeV1 for ModeV1<'_, '_, '_, '_, '_> {
    type Effect = effect::ReportV1;
    fn effect(
        self,
        execution: SemanticExecutionV1<'_>,
        observer: SemanticObserverV1<'_, '_, '_>,
    ) -> Self::Effect {
        let SemanticExecutionV1 {
            context,
            function,
            analyses,
        } = execution;
        effect::run_preadmitted_with_bounds_v1(
            context,
            function,
            analyses,
            self.0,
            self.2,
            (observer, self.1),
        )
    }
    fn selected_view(&self, view: pliron::value::Value) -> bool {
        self.0
            .input()
            .occurrences()
            .iter()
            .any(|occurrence| occurrence.2 == view)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ReportV1 {
    body: SemanticBodyV1<effect::ReportV1>,
    input_mismatch: bool,
    bounds_failure: Option<own::BoundsFailureV1>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ErrorV1 {
    pub(crate) report: ReportV1,
}

impl ReportV1 {
    pub(crate) fn typed_root_commitments(&self) -> &[[u64; 4]] {
        &self.body.typed_root_commitments
    }
    pub(crate) fn is_clean(&self) -> bool {
        !self.input_mismatch
            && self.bounds_failure.is_none()
            && self.body.findings.is_empty()
            && self.body.progress.is_clean()
            && matches!(&self.body.effect_refinement, EffectRunV1::Executed(effect) if effect.is_clean())
    }

    #[cfg(all(test, feature = "internal-proof-staging"))]
    pub(crate) fn test_failed_progress(&self, expected: &PlironProgressReportV1) {
        assert!(!expected.is_clean());
        assert!(!self.input_mismatch);
        assert!(self.bounds_failure.is_none());
        assert_eq!(&self.body.progress, expected);
        assert!(self.body.findings.is_empty());
        assert!(matches!(self.body.effect_refinement, EffectRunV1::NotRun));
        assert!(!self.is_clean());
    }
}

pub(crate) fn preflight_v1(
    census: ProductionAnalysisInputCensusV1,
    selections: usize,
) -> Result<ProductionAnalysisResourceUpperBoundV1, ProductionAnalysisResourceLimitV1> {
    let work = checked_semantic_sum_v1(&[
        own::BOUNDS_DEPENDENCY_VALIDATION_WORK_V1,
        checked_semantic_product_v1(own::BOUNDS_DEPENDENCY_OBLIGATION_WORK_V1, census.operands)?,
        checked_semantic_product_v1(
            own::BOUNDS_DEPENDENCY_ROSTER_WORK_V1,
            checked_semantic_sum_v1(&[census.operations, census.ownership_contracts])?,
        )?,
        checked_semantic_product_v1(
            128,
            checked_semantic_product_v1(
                checked_semantic_sum_v1(&[census.semantic_refinement_contracts, 1])?,
                checked_semantic_sum_v1(&[selections, 1])?,
            )?,
        )?,
    ])?;
    ProductionAnalysisResourceUpperBoundV1::checked_phase(
        ProductionAnalysisResourcePhaseV1::SemanticRefinement,
        work,
        std::mem::size_of::<ReportV1>(),
        0,
    )
}

#[cfg(test)]
pub(crate) fn run_after_progress_preadmitted_v1(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
    progress: PlironProgressReportV1,
    prepared: effect::PreparedV1<'_>,
) -> ReportV1 {
    run_after_progress_with_observation_v1(context, function, analyses, progress, prepared, None)
}

#[cfg(test)]
pub(crate) fn run_after_progress_with_observation_v1(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
    progress: PlironProgressReportV1,
    prepared: effect::PreparedV1<'_>,
    observer: SemanticObserverV1<'_, '_, '_>,
) -> ReportV1 {
    run_after_progress_with_admissions_v1(
        context,
        function,
        analyses,
        progress,
        prepared,
        (observer, None),
    )
}

#[cfg(test)]
fn run_after_progress_with_admissions_v1(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
    progress: PlironProgressReportV1,
    prepared: effect::PreparedV1<'_>,
    observations: (
        SemanticObserverV1<'_, '_, '_>,
        AdditionalObservationV1<'_, '_, '_>,
    ),
) -> ReportV1 {
    run_after_progress_with_bounds_v1(
        context,
        function,
        analyses,
        progress,
        prepared,
        &own::BoundsSourceV1::Ordinary,
        observations,
    )
}

pub(crate) fn run_after_progress_with_bounds_v1(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
    progress: PlironProgressReportV1,
    prepared: effect::PreparedV1<'_>,
    bounds: &own::BoundsSourceV1<'_>,
    observations: (
        SemanticObserverV1<'_, '_, '_>,
        AdditionalObservationV1<'_, '_, '_>,
    ),
) -> ReportV1 {
    let (observer, additional) = observations;
    let run = || {
        let input = prepared.input();
        let census = input.census();
        let epoch = input.epoch();
        if !std::ptr::eq(context, input.context())
            || function.get_operation() != input.function().get_operation()
            || analyses.input_census() != Some(census)
            || context
                .ir_mutation_attempt_epoch()
                .ok()
                .map(|value| value.value())
                != Some(epoch)
        {
            return ReportV1 {
                body: semantic_early_v1(Vec::new(), progress),
                input_mismatch: true,
                bounds_failure: None,
            };
        }
        if let Err(error) = bounds.validate_v1(input, analyses) {
            return ReportV1 {
                body: semantic_early_v1(Vec::new(), progress),
                input_mismatch: false,
                bounds_failure: Some(error),
            };
        }
        if !progress.is_clean() {
            return ReportV1 {
                body: semantic_early_v1(Vec::new(), progress),
                input_mismatch: false,
                bounds_failure: None,
            };
        }
        let body = run_semantic_core_v1(
            context,
            function,
            analyses,
            progress,
            ModeV1(prepared, additional, bounds),
            observer,
        );
        ReportV1 {
            body,
            bounds_failure: None,
            input_mismatch: context
                .ir_mutation_attempt_epoch()
                .ok()
                .map(|value| value.value())
                != Some(epoch),
        }
    };
    match observer {
        None => run(),
        Some(observer) => observer.with_projection(&Ok, |_| run()),
    }
}

pub(crate) fn require_with_scoped_bounds_v1(
    input: crate::production_analysis::pliron_pass_contract::ScopedVerifiedProgressInputV1<'_>,
    analyses: &mut PlironAnalysisManagerV1,
    prepared: effect::PreparedV1<'_>,
    bounds: &own::BoundsSourceV1<'_>,
    observations: (
        SemanticObserverV1<'_, '_, '_>,
        AdditionalObservationV1<'_, '_, '_>,
    ),
) -> Result<
    Result<ReportV1, ErrorV1>,
    crate::production_analysis::pliron_pass_contract::PlironPassPreservationErrorV1,
> {
    let (observer, additional) = observations;
    let run = || {
        let scoped =
            crate::production_analysis::pliron_progress::run_pliron_progress_with_scoped_observation_v1(
                input, observer,
            )?;
        let report = run_after_progress_with_bounds_v1(
            scoped.context,
            scoped.function,
            analyses,
            scoped.report,
            prepared,
            bounds,
            (observer, additional),
        );
        Ok(if report.is_clean() {
            Ok(report)
        } else {
            Err(ErrorV1 { report })
        })
    };
    match observer {
        None => run(),
        Some(observer) => observer.with_projection(&Ok, |_| run()),
    }
}
