//! One actual full-stage producer of the existing inert history container.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKirTransitionReceiptErrorV1, InertCanonicalKirTransitionReceiptV1,
    encode_canonical_kir_occurrence_row_bytes_v1,
};
use fe2o3_kernel_opt::{
    CanonicalPolicy4ExecutionReceiptErrorV1, CanonicalPolicy6ContinuationClaimsV1,
    CanonicalPolicy8HistoryEncodingInputsV1, CanonicalPolicy8HistoryErrorV1,
    InertCanonicalPolicy8HistoryV1, encode_checked_canonical_policy4_execution_receipt_v1,
    encode_inert_policy8_history_v1,
};
use std::panic::{AssertUnwindSafe, catch_unwind};

#[derive(Debug)]
pub(crate) enum Policy8HistoryExportErrorV1 {
    Policy4(CanonicalPolicy4ExecutionReceiptErrorV1),
    Transition(CanonicalKirTransitionReceiptErrorV1),
    History(CanonicalPolicy8HistoryErrorV1),
    #[cfg(test)]
    DecodedInputs(Box<fe2o3_kernel_opt::CanonicalPolicy8HistoryInputsErrorV1>),
    #[cfg(test)]
    DecodedSemantics(Box<fe2o3_kernel_opt::CanonicalPolicy8CompositionErrorV1>),
    Panicked,
}
impl fmt::Display for Policy8HistoryExportErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Policy8 actual history export: {self:?}")
    }
}
impl std::error::Error for Policy8HistoryExportErrorV1 {}

fn export_error(value: Policy8HistoryExportErrorV1) -> ProductionPipelineError {
    error(CheckedOutputPolicy8StageErrorV1::History(Box::new(value)))
}

// Only this closed scope refunds new local reservations. Successful output is
// transferred unreserved; no consumer callback or retained reservation escapes.
fn export_scope<'work, T>(
    required: usize,
    budget: &mut Budget<'work>,
    run: impl FnOnce(&mut Budget<'work>) -> Result8<T>,
) -> Result8<T> {
    if budget.storage() < required {
        return Err(resource(Resource::Accounting));
    }
    let floor = budget.storage();
    let slot = budget as *const Budget<'_> as usize;
    let ledger = budget.work_ledger_identity_v1();
    let mut payloads = [None, None];
    let mut result = match catch_unwind(AssertUnwindSafe(|| run(budget))) {
        Ok(result) => result,
        Err(payload) => {
            payloads[0] = Some(payload);
            Err(export_error(Policy8HistoryExportErrorV1::Panicked))
        }
    };
    let cleanup = if slot != budget as *const Budget<'_> as usize
        || ledger != budget.work_ledger_identity_v1()
    {
        Err(Resource::Accounting)
    } else {
        budget
            .storage()
            .checked_sub(floor)
            .ok_or(Resource::Accounting)
            .and_then(|bytes| budget.release_storage(bytes))
    };
    if let Err(error) = cleanup {
        let rejected = std::mem::replace(&mut result, Err(resource(error)));
        payloads[1] = catch_unwind(AssertUnwindSafe(|| drop(rejected))).err();
    }
    drop(payloads);
    result
}

// Full-stage verification is performed by the sole production caller. The
// constructed-artifact tests call only this core and cannot fabricate bindings.
fn encode_actual_history(
    artifacts: &PreparedPolicy8ArtifactsV1,
    budget: &mut Budget<'_>,
) -> Result8<InertCanonicalPolicy8HistoryV1> {
    // Fixed borrowed role/record assembly and receipt bookkeeping, no graph scan.
    budget.charge_work(128).map_err(resource)?;
    let (bound, checked) = artifacts.admitted.history().portable_prefix();
    let p5 = checked.intermediate_policy5();
    let p4 = p5.intermediate_policy4();
    let policy4 = encode_checked_canonical_policy4_execution_receipt_v1(bound, p4, budget)
        .map_err(|e| export_error(Policy8HistoryExportErrorV1::Policy4(e)))?;
    budget
        .reserve_storage(policy4.storage().retained_storage())
        .map_err(resource)?;
    let (transition, transition_storage) =
        InertCanonicalKirTransitionReceiptV1::from_candidate_with_budget(
            p5.owner().canonical().identity(),
            checked.owner().canonical().identity(),
            checked.continuation().occurrences().candidate(),
            budget,
        )
        .map_err(|e| export_error(Policy8HistoryExportErrorV1::Transition(e)))?;
    budget
        .reserve_storage(transition_storage.retained_storage())
        .map_err(resource)?;
    let wire = {
        let authenticated = artifacts.check_portable_execution_relation_v1(
            policy4.canonical_bytes(),
            p5.execution().canonical_bytes(),
            p5.load_forwarding_rows(),
            CanonicalPolicy6ContinuationClaimsV1 {
                composition_record: checked.execution().canonical_bytes(),
                integer_record: checked.continuation().execution().canonical_bytes(),
                transition_wire: transition.canonical_bytes(),
            },
            artifacts.prefix_execution().canonical_bytes(),
            budget,
        )?;
        budget
            .reserve_storage(authenticated.retained_storage())
            .map_err(resource)?;
        let (tail, tail_storage) = encode_canonical_kir_occurrence_row_bytes_v1(
            authenticated.continuation().claims().occurrences,
            budget,
        )
        .map_err(|e| export_error(Policy8HistoryExportErrorV1::Transition(e)))?;
        budget
            .reserve_storage(tail_storage.retained_storage())
            .map_err(resource)?;
        let (wire, storage) = encode_inert_policy8_history_v1(
            CanonicalPolicy8HistoryEncodingInputsV1 {
                roles: [
                    bound,
                    p4.intermediate_policy3().owner(),
                    p4.owner(),
                    p5.owner(),
                    checked.owner(),
                    authenticated.continuation().input(),
                    artifacts.output(),
                ],
                policy4_wire: policy4.canonical_bytes(),
                policy5_record: p5.execution().canonical_bytes(),
                load_rows: p5.load_forwarding_rows(),
                integer_record: checked.continuation().execution().canonical_bytes(),
                transition_wire: transition.canonical_bytes(),
                policy7_record: artifacts.prefix_execution().canonical_bytes(),
                tail_rows: &tail,
            },
            budget,
        )
        .map_err(|e| export_error(Policy8HistoryExportErrorV1::History(e)))?;
        budget
            .reserve_storage(storage.retained_storage())
            .map_err(resource)?;
        // Temporary backing drops while its complete receipts remain prepaid.
        drop(tail);
        wire
    };
    drop(transition);
    drop(policy4);
    Ok(wire)
}

impl CheckedOutputTargetProductionCompilationPolicy8V1 {
    /// Exports only the existing inert history of this genuine retained stage.
    /// The full surviving stage floor and existing collector/root/source/native
    /// verification precede encoding. No graph, profile, record, row or J is a
    /// caller input; all are borrowed from one actual immutable prefix and K.
    ///
    /// Existing P4/O-I/neutral encoders and actual-stage portable authentication
    /// run unchanged. Their repeated source/formal/native replay and inherited
    /// allocation domains remain explicit; this is not deduplicated verification
    /// or an RSS bound. New temporary and output receipts stay fully reserved
    /// until their backing drops or the result transfers. Failure restores only
    /// the intact incoming floor on the same Work, retaining work/peak/denials.
    /// Existing diagnostic error boxing and panic payload allocation remain
    /// outside the retained-data accounting contract.
    ///
    /// The returned owner is UNRESERVED; reserve its storage before further
    /// controlled allocation. It remains inert on return and on readmission:
    /// no producer seal, signed source proof, publication, default or launch
    /// authority is serialized. No owner layout or existing protocol changes.
    pub(crate) fn export_portable_history_v1(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result8<InertCanonicalPolicy8HistoryV1> {
        export_scope(self.retained_floor, budget, |budget| {
            self.verify_equivalence(budget)?;
            encode_actual_history(&self.artifacts, budget)
        })
    }
}

#[cfg(test)]
#[path = "production_policy8_history_v1_tests.rs"]
pub(crate) mod tests;
