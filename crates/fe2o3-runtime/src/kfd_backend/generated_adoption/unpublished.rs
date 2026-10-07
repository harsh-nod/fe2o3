//! Read-only original-owner boundary; generic retirement also allows Recycled.

use super::*;

impl KfdRuntimeBackendV1 {
    pub(crate) fn generated_data_unpublished_v1(
        &self,
        plan: &GeneratedShellPlanV1,
        submission: Option<u64>,
    ) -> bool {
        if self.require_live().is_err()
            || !self.validate_generated_shell_records_v1(plan)
            || !self.generated_lease_matches_v1(plan)
        {
            return false;
        }
        let native = self.generated_shells[&plan.key]
            .native
            .as_ref()
            .expect("original lease retains native owner");
        if native.phase != PhaseV1::Adopted {
            return false;
        }
        match (submission, native.submission.as_ref()) {
            (None, None) => self.generated_shell_has_no_submission_v1(plan),
            (Some(id), Some(owner)) => {
                self.generated_submission_owner_matches_v1(id, plan) && owner.receipt.issue_ready()
            }
            _ => false,
        }
    }
}
