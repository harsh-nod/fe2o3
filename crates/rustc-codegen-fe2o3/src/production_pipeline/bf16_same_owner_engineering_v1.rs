//! Consuming compiler-private engineering continuation of the real source owner.
//! No default selector, artifact extraction, publication or launch conversion.
use super::*;
use fe2o3_hsaco_finalize::{PinnedWorkerV1, WorkerExecutionLimitsV1, WorkerOutputConstraintsV1};
use std::time::Instant;

impl Bf16SameOwnerHandoffV1 {
    /// Execute only the closed engineering API, while both original accounts
    /// and the intact source/descriptor/handoff remain live. The sole result is
    /// unit after the owning stage and then its materialization account drop.
    /// Runtime image/loader custody and descendant containment remain external
    /// root duties; this compiler-private method issues no execution lease.
    #[allow(dead_code)]
    pub(crate) fn observe_engineering_and_drop_v1(
        self,
        requested_return: [u8; 4],
        worker: &PinnedWorkerV1,
        output: WorkerOutputConstraintsV1,
        limits: WorkerExecutionLimitsV1,
        deadline: Instant,
    ) -> Result<(), Box<ProductionPipelineError>> {
        self.phase
            .try_map(|mut stage, budget| {
                budget
                    .check_prior_denials_v1()
                    .map_err(materialization_resource_error_v29)?;
                let ledger = budget.work_ledger_identity_v1();
                let before = (budget.storage(), budget.work(), budget.peak_storage());
                let attempted: Result<(), ProductionPipelineError> =
                    stage.handoff.observe_same_owner_engineering_v1(
                        requested_return,
                        &stage.bindings.typed_descriptor_roots,
                        stage.bindings.rustc_target.profile(),
                        worker,
                        output,
                        limits,
                        deadline,
                    );
                // This is the SAME source owner, not an extracted engine value.
                // On errors too, source/descriptor/engine state dies before the
                // original outer account. No counter or credit is reset.
                drop(stage);
                let clean = budget.check_prior_denials_v1();
                if budget.work_ledger_identity_v1() != ledger
                    || (budget.storage(), budget.work(), budget.peak_storage()) != before
                {
                    drop(attempted);
                    clean.map_err(materialization_resource_error_v29)?;
                    return Err(Box::new(materialization_resource_error_v29(
                        Resource::Accounting,
                    )));
                }
                clean.map_err(materialization_resource_error_v29)?;
                attempted.map_err(Box::new)
            })?
            .finish_copy();
        Ok(())
    }

    /// Genuine-source negative controls only. No replacement owner/Worker is
    /// constructed; the closed entry guard refuses any unexpected engine call.
    #[cfg(test)]
    pub(crate) fn exercise_engineering_refusals_and_drop_for_test_v1(
        self,
        requested_return: [u8; 4],
        worker: &PinnedWorkerV1,
        output_bytes: u64,
        limits: WorkerExecutionLimitsV1,
        deadline: Instant,
    ) -> Result<(), Box<ProductionPipelineError>> {
        self.phase
            .try_map(|mut stage, budget| {
                budget
                    .check_prior_denials_v1()
                    .map_err(materialization_resource_error_v29)?;
                let ledger = budget.work_ledger_identity_v1();
                let before = (budget.storage(), budget.work(), budget.peak_storage());
                let attempted: Result<(), ProductionPipelineError> = stage
                    .handoff
                    .exercise_same_owner_engineering_refusals_for_test_v1(
                        requested_return,
                        &stage.bindings.typed_descriptor_roots,
                        stage.bindings.rustc_target.profile(),
                        worker,
                        output_bytes,
                        limits,
                        deadline,
                    );
                drop(stage);
                let clean = budget.check_prior_denials_v1();
                if budget.work_ledger_identity_v1() != ledger
                    || (budget.storage(), budget.work(), budget.peak_storage()) != before
                {
                    drop(attempted);
                    clean.map_err(materialization_resource_error_v29)?;
                    return Err(Box::new(materialization_resource_error_v29(
                        Resource::Accounting,
                    )));
                }
                clean.map_err(materialization_resource_error_v29)?;
                attempted.map_err(Box::new)
            })?
            .finish_copy();
        Ok(())
    }
}
