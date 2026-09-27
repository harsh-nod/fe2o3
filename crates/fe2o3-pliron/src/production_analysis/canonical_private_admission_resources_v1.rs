//! Private orchestration uses the existing cumulative analysis contract/receipt.
use super::*;
use crate::kir_bridge_v1::NativePrivateInputV1;
use crate::production_analysis::pliron_pipeline::{
    PipelineErrorV1,
    canonical_private_v1::CanonicalPrivatePipelineOutcomeV1,
};

fn snapshot(
    bound: Bound,
    receipt: InvocationObservationV1,
) -> CanonicalRankedPolicyResourceObservationV1 {
    CanonicalRankedPolicyResourceObservationV1 {
        work: bound.work_upper_bound(),
        retained: bound.retained_storage_upper_bound(),
        peak: bound.peak_storage_upper_bound(),
        first_denial: receipt
            .first_denial
            .map(|error| (error.phase, error.resource)),
        caught_panic: receipt.caught_panic,
    }
}

pub(in crate::production_analysis::canonical_ranked_checks_v1) struct PrivateAnalysisV1 {
    contract: Contract,
    limits: Limits,
    denial: Option<(Phase, &'static str)>,
    panicked: bool,
    pub(in crate::production_analysis::canonical_ranked_checks_v1) last: Option<CanonicalRankedPolicyHistoryV1>,
}
impl PrivateAnalysisV1 {
    pub(in crate::production_analysis::canonical_ranked_checks_v1) fn new(limits: Limits) -> Self {
        Self {
            contract: Contract::new(limits),
            limits,
            denial: None,
            panicked: false,
            last: None,
        }
    }

    fn denied(&mut self, error: Limit) -> Failure {
        self.denial.get_or_insert((error.phase, error.resource));
        error.into()
    }

    pub(in crate::production_analysis::canonical_ranked_checks_v1) fn observation(&self) -> CanonicalRankedPolicyResourceObservationV1 {
        let mut observed = snapshot(
            self.contract.cumulative(),
            InvocationObservationV1::default(),
        );
        observed.first_denial = self.denial;
        observed.caught_panic = self.panicked;
        observed
    }

    pub(in crate::production_analysis::canonical_ranked_checks_v1) fn invoke(
        &mut self,
        input: &impl NativePrivateInputV1,
    ) -> Result<CanonicalPrivatePipelineOutcomeV1, Failure> {
        let floor = self.contract.cumulative();
        let limits = self
            .contract
            .remaining(Phase::PipelineVerification)
            .map_err(|error| self.denied(error))?;
        let mut receipt =
            InvocationReceiptV1::new(floor, self.limits).map_err(|error| self.denied(error))?;
        let result = catch_unwind(AssertUnwindSafe(|| {
            input.run_fixed(limits, Some(&mut receipt))
        }));
        let observed = receipt.snapshot();
        self.last = Some(CanonicalRankedPolicyHistoryV1 {
            function: input.ordinal(),
            floor: snapshot(floor, InvocationObservationV1::default()),
            invocation: snapshot(observed.current, observed),
        });
        if let Some(error) = observed.first_denial {
            self.denial.get_or_insert((error.phase, error.resource));
        }
        self.panicked |= observed.caught_panic || result.is_err();
        self.contract
            .admit_retained(Phase::PipelineVerification, observed.current)
            .map_err(|error| self.denied(error))?;
        let outcome = match result {
            Ok(Ok(outcome)) => outcome,
            Ok(Err(PipelineErrorV1::Ordinary(cause))) => {
                return Err(Failure::Analysis {
                    function: input.ordinal(),
                    cause,
                });
            }
            Ok(Err(_)) => return Err(refuse(CanonicalPrivateRequirementV1::StageCoverage, None)),
            Err(payload) => {
                resources::discard(payload);
                return Err(Failure::Panicked);
            }
        };
        let bound = receipt
            .complete()
            .map_err(|_| Failure::InvocationAccounting)?;
        if outcome.resource_upper_bound != bound {
            return Err(Failure::InvocationAccounting);
        }
        Ok(outcome)
    }

    pub(in crate::production_analysis::canonical_ranked_checks_v1) fn release_reports(&mut self) -> Result<(), Failure> {
        let retained = self.contract.cumulative().retained_storage_upper_bound();
        self.contract
            .admit_replacement(Phase::PipelineVerification, retained, Bound::default())
            .map_err(|error| self.denied(error))
    }
}
