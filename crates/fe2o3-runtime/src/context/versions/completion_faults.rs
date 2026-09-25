//! Test-only faults at the adapter boundary, not simulated journal internals.

use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::context) enum CompletionJournalStageV1 {
    Stable,
    Producer,
    Writer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::context) enum CompletionJournalPointV1 {
    BeforeEffect,
    AfterEffect,
}

pub(in crate::context) enum CompletionJournalFailureV1 {
    Error,
    Panic(Box<dyn core::any::Any + Send>),
}

pub(super) struct CompletionJournalFaultV1 {
    id: RuntimeSubmissionIdV1,
    stage: CompletionJournalStageV1,
    point: CompletionJournalPointV1,
    failure: CompletionJournalFailureV1,
}

impl ContextVersionsV1 {
    pub(in crate::context) fn inject_completion_fault_for_test_v1(
        &mut self,
        id: RuntimeSubmissionIdV1,
        stage: CompletionJournalStageV1,
        point: CompletionJournalPointV1,
        failure: CompletionJournalFailureV1,
    ) {
        assert!(self.completion_fault.is_none());
        assert!(
            point != CompletionJournalPointV1::AfterEffect
                || matches!(failure, CompletionJournalFailureV1::Panic(_)),
            "a journal error is not a post-commit result"
        );
        self.completion_fault = Some(CompletionJournalFaultV1 {
            id,
            stage,
            point,
            failure,
        });
    }

    pub(super) fn completion_boundary_for_test_v1(
        &mut self,
        id: RuntimeSubmissionIdV1,
        stage: CompletionJournalStageV1,
        point: CompletionJournalPointV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        if !self
            .completion_fault
            .as_ref()
            .is_some_and(|fault| fault.id == id && fault.stage == stage && fault.point == point)
        {
            return Ok(());
        }
        match self.completion_fault.take().unwrap().failure {
            CompletionJournalFailureV1::Error => Err(ContextVersionJournalErrorV1::InvalidState),
            CompletionJournalFailureV1::Panic(payload) => std::panic::resume_unwind(payload),
        }
    }

    pub(in crate::context) fn completion_fault_pending_for_test_v1(&self) -> bool {
        self.completion_fault.is_some()
    }

    pub(in crate::context) fn completion_roots_for_test_v1(
        &self,
        id: RuntimeSubmissionIdV1,
    ) -> (bool, bool, bool) {
        (
            self.submission_readers.contains_key(&id),
            self.producer_readers.contains_key(&id),
            self.submission_writers.contains_key(&id),
        )
    }
}
