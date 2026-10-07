//! Context-scoped recorder configuration/readback; no raw backend or native access.
use super::*;
use crate::{
    KfdGeneratedCopyCoexistenceFailureV1 as Failure, KfdGeneratedCopyCoexistenceWitnessV1,
    KfdRuntimeBackendV1,
};

impl RuntimeContextV1<KfdRuntimeBackendV1> {
    /// Arms the bounded generated/copy receipt qualification on this live context.
    /// Call after warming independent buffers and before submitting either operation.
    /// No publication, poll, result, or execution authority is created.
    pub fn arm_generated_copy_coexistence_qualification_v1(&mut self) -> Result<(), Failure> {
        self.require_live()
            .map_err(|_| Failure::ContextUnavailable)?;
        self.invoke_journal_backend_v1(|backend| {
            Ok(backend.arm_generated_copy_coexistence_qualification_v1())
        })
        .map_err(|_| Failure::ContextUnavailable)?
    }

    /// Takes only historical receipt-coexistence evidence. Successful completion,
    /// outputs, canaries, cleanup, and refunds remain independent obligations.
    pub fn take_generated_copy_coexistence_qualification_v1(
        &mut self,
    ) -> Result<KfdGeneratedCopyCoexistenceWitnessV1, Failure> {
        self.require_live()
            .map_err(|_| Failure::ContextUnavailable)?;
        self.invoke_journal_backend_v1(|backend| {
            Ok(backend.take_generated_copy_coexistence_qualification_v1())
        })
        .map_err(|_| Failure::ContextUnavailable)?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kfd_backend::counted_allocations_for_test_v1 as counted;

    #[test]
    fn context_qualification_is_allocation_free_and_cannot_rearm_or_invent_evidence() {
        let mut context =
            RuntimeContextV1::open(KfdRuntimeBackendV1::mock_worker_v3_generated_only_v1())
                .unwrap();
        let (result, allocations) =
            counted(|| context.arm_generated_copy_coexistence_qualification_v1());
        assert_eq!(result, Ok(()));
        assert_eq!(allocations, 0);
        assert_eq!(
            context.take_generated_copy_coexistence_qualification_v1(),
            Err(Failure::MissingPublication)
        );
        assert_eq!(
            context.arm_generated_copy_coexistence_qualification_v1(),
            Err(Failure::AlreadyArmed)
        );
        assert!(!context.is_terminal());
    }

    #[test]
    fn terminal_context_refuses_before_recording() {
        let mut context =
            RuntimeContextV1::open(KfdRuntimeBackendV1::mock_worker_v3_generated_only_v1())
                .unwrap();
        context.terminal = true;
        let arm = context.arm_generated_copy_coexistence_qualification_v1();
        let take = context.take_generated_copy_coexistence_qualification_v1();
        context.terminal = false;
        assert_eq!(arm, Err(Failure::ContextUnavailable));
        assert_eq!(take, Err(Failure::ContextUnavailable));
    }
}
