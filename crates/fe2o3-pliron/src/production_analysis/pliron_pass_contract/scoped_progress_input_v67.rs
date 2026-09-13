use crate::production_analysis::pliron_ir_identity::LivePlironStructuralIdentityProviderV1;
use pliron::{
    builtin::ops::FuncOp,
    context::{Context, IrMutationAttemptEpochExhausted},
};

// Extra fixed visits: pass guard 3, pending epoch 1, three-field publication 3,
// callback dispatch 1, epoch query/comparison 2, and endpoint extraction 2.
const SCOPED_PROGRESS_SUCCESS_WORK_V1: usize = 12;
pub(crate) const SCOPED_PROGRESS_INPUT_STORAGE_V1: usize = 3;
const SCOPED_PROGRESS_EPOCH_DETAIL_V1: &str = "IR mutation-attempt epoch exhausted";
// Error discriminant, optional pass (two cells), and three-cell String owner.
const SCOPED_PROGRESS_ERROR_FIELDS_V1: usize = 6;

fn scoped_progress_input_resource_upper_bound_v1()
-> Result<ProductionAnalysisResourceUpperBoundV1, PlironPassPreservationErrorV1> {
    ProductionAnalysisResourceUpperBoundV1::checked_phase(
        ProductionAnalysisResourcePhaseV1::PassPreservation,
        SCOPED_PROGRESS_SUCCESS_WORK_V1
            + SCOPED_PROGRESS_ERROR_FIELDS_V1
            + SCOPED_PROGRESS_EPOCH_DETAIL_V1.len(),
        0,
        SCOPED_PROGRESS_INPUT_STORAGE_V1
            + SCOPED_PROGRESS_ERROR_FIELDS_V1
            + SCOPED_PROGRESS_EPOCH_DETAIL_V1.len(),
    )
    .map_err(preservation_resource_error_v1)
}

/// One callback-scoped use of the live session's most recent verified graph.
/// No digest, replacement endpoint, generic provider, or public constructor can
/// mint this input. The callback lifetime cannot escape its higher-ranked runner.
pub(crate) struct ScopedVerifiedProgressInputV1<'scope, const BARRIER: bool = false> {
    context: &'scope Context,
    function: &'scope FuncOp,
    mutation_epoch: u64,
}

fn require_scoped_progress_epoch_for_pass_v1(
    pass: KernelCheckPassKindV1,
    expected: u64,
    observed: Result<u64, IrMutationAttemptEpochExhausted>,
) -> Result<(), PlironPassPreservationErrorV1> {
    let pass = Some(pass);
    let observed =
        observed.map_err(
            |_error| PlironPassPreservationErrorV1::MutationEpochUnavailable {
                pass,
                detail: SCOPED_PROGRESS_EPOCH_DETAIL_V1.to_owned(),
            },
        )?;
    if observed != expected {
        return Err(PlironPassPreservationErrorV1::MutationAttempted {
            pass,
            before: expected,
            after: observed,
        });
    }
    Ok(())
}

#[cfg(test)]
fn require_scoped_progress_epoch_v1(
    expected: u64,
    observed: Result<u64, IrMutationAttemptEpochExhausted>,
) -> Result<(), PlironPassPreservationErrorV1> {
    require_scoped_progress_epoch_for_pass_v1(
        KernelCheckPassKindV1::SemanticRefinement,
        expected,
        observed,
    )
}

const fn scoped_progress_pass_v1<const BARRIER: bool>() -> KernelCheckPassKindV1 {
    if BARRIER {
        KernelCheckPassKindV1::BarrierConvergence
    } else {
        KernelCheckPassKindV1::SemanticRefinement
    }
}

impl<'scope, const BARRIER: bool> ScopedVerifiedProgressInputV1<'scope, BARRIER> {
    pub(crate) fn into_endpoints(
        self,
    ) -> Result<(&'scope Context, &'scope FuncOp), PlironPassPreservationErrorV1> {
        self.endpoints()
    }

    pub(crate) fn endpoints(
        &self,
    ) -> Result<(&'scope Context, &'scope FuncOp), PlironPassPreservationErrorV1> {
        let Self {
            context,
            function,
            mutation_epoch,
        } = *self;
        require_scoped_progress_epoch_for_pass_v1(
            scoped_progress_pass_v1::<BARRIER>(),
            mutation_epoch,
            context
                .ir_mutation_attempt_epoch()
                .map(|epoch| epoch.value()),
        )?;
        Ok((context, function))
    }
}

impl PlironPassContractSessionV1<LivePlironStructuralIdentityProviderV1<'_>> {
    pub(crate) fn run_scoped_semantic_refinement_with_resource_limits_v1<T, E>(
        &mut self,
        limits: impl Into<ProductionAnalysisReplacementLimitsV1>,
        execute: impl for<'scope> FnOnce(
            ScopedVerifiedProgressInputV1<'scope>,
        ) -> Result<Result<T, E>, PlironPassPreservationErrorV1>,
    ) -> Result<Result<T, E>, PlironPassPreservationErrorV1> {
        self.run_scoped_progress_pass_with_resource_limits_v1::<false, T, E>(limits, execute)
    }

    pub(crate) fn run_scoped_barrier_with_resource_limits_v1<T, E>(
        &mut self,
        limits: impl Into<ProductionAnalysisReplacementLimitsV1>,
        execute: impl for<'scope> FnOnce(
            ScopedVerifiedProgressInputV1<'scope, true>,
        ) -> Result<Result<T, E>, PlironPassPreservationErrorV1>,
    ) -> Result<Result<T, E>, PlironPassPreservationErrorV1> {
        self.run_scoped_progress_pass_with_resource_limits_v1::<true, T, E>(limits, execute)
    }

    // The two named entry points bind the pass at compile time; no caller can
    // mint an input for an arbitrary pass or replace its verified endpoints.
    fn run_scoped_progress_pass_with_resource_limits_v1<const BARRIER: bool, T, E>(
        &mut self,
        limits: impl Into<ProductionAnalysisReplacementLimitsV1>,
        execute: impl for<'scope> FnOnce(
            ScopedVerifiedProgressInputV1<'scope, BARRIER>,
        ) -> Result<Result<T, E>, PlironPassPreservationErrorV1>,
    ) -> Result<Result<T, E>, PlironPassPreservationErrorV1> {
        let scope_bound = scoped_progress_input_resource_upper_bound_v1()?;
        self.run_scoped_analysis_with_resource_limits_v1(
            scoped_progress_pass_v1::<BARRIER>(),
            scope_bound,
            limits,
            |context, function, mutation_epoch, _snapshot| {
                execute(ScopedVerifiedProgressInputV1 {
                    context,
                    function,
                    mutation_epoch,
                })
            },
        )
    }

    fn run_scoped_analysis_with_resource_limits_v1<T, E>(
        &mut self,
        pass: KernelCheckPassKindV1,
        scope_bound: ProductionAnalysisResourceUpperBoundV1,
        limits: impl Into<ProductionAnalysisReplacementLimitsV1>,
        execute: impl for<'scope> FnOnce(
            &'scope Context,
            &'scope FuncOp,
            u64,
            &'scope crate::production_analysis::pliron_ir_identity::BuiltIdentityV1,
        ) -> Result<Result<T, E>, PlironPassPreservationErrorV1>,
    ) -> Result<Result<T, E>, PlironPassPreservationErrorV1> {
        let limits = limits.into();
        let input_limits = limits
            .input
            .remaining_after_retained(
                ProductionAnalysisResourcePhaseV1::PassPreservation,
                scope_bound,
            )
            .map_err(preservation_resource_error_v1)?;
        let preservation_limits = limits
            .output
            .remaining_after_retained(
                ProductionAnalysisResourcePhaseV1::PassPreservation,
                scope_bound,
            )
            .map_err(preservation_resource_error_v1)?;
        self.require_pass_can_begin(pass)?;
        self.begin_pass_with_resource_limits_v1(pass, false, input_limits)?;
        let pending =
            self.pending
                .as_ref()
                .ok_or(PlironPassPreservationErrorV1::InvalidSessionState {
                    detail: "the scoped analysis pass has no active checkpoint",
                })?;
        let result = {
            let (context, function) = self.provider.scoped_endpoints_v1();
            catch_unwind(AssertUnwindSafe(|| {
                execute(context, function, pending.mutation_epoch, &pending.before)
            }))
        };
        match result {
            Err(_) => Err(PlironPassPreservationErrorV1::AnalysisPanicked { pass }),
            Ok(Err(error)) => Err(error),
            Ok(Ok(result)) => {
                // The callback and all scoped inputs end before the unchanged
                // post-pass capture, including an ordinary analysis Err result.
                self.end_pass_with_resource_limits_v1(pass, preservation_limits)?;
                let checkpoint = self.last_checkpoint_resource_upper_bound.ok_or(
                    PlironPassPreservationErrorV1::InvalidSessionState {
                        detail: "the scoped analysis pass has no completed resource checkpoint",
                    },
                )?;
                let combined = scope_bound
                    .checked_then_retain(
                        checkpoint,
                        ProductionAnalysisResourcePhaseV1::PassPreservation,
                    )
                    .map_err(preservation_resource_error_v1)?;
                limits
                    .output
                    .require(
                        ProductionAnalysisResourcePhaseV1::PassPreservation,
                        combined,
                    )
                    .map_err(preservation_resource_error_v1)?;
                self.last_checkpoint_resource_upper_bound = Some(combined);
                Ok(result)
            }
        }
    }
}
