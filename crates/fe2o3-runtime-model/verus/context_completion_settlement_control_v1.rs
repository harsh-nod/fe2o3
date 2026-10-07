use vstd::prelude::*;

include!("../../fe2o3-runtime/src/context/completion_settlement_body.rs");

verus! {

// Opaque argument/result labels. This harness proves call control flow, not
// Context storage, journal effects, callbacks, backend observations or unwind.
#[derive(PartialEq, Eq, Clone, Copy)]
struct ScriptV1 {
    submission: u64,
    outcome: u64,
    status: u64,
    returned: u64,
    error: u64,
    fail_at: u8,
}

#[derive(PartialEq, Eq, Clone, Copy)]
struct FailureV1 {
    stage: u8,
    code: u64,
}

struct SettlementProbeV1 {
    script: ScriptV1,
    attempted: u8,
    completed: u8,
    writer_seen: Option<u64>,
    publication_seen: Option<u64>,
}

spec fn ready_v1(state: SettlementProbeV1, stage: u8) -> bool {
    &&& 1 <= stage <= 4
    &&& state.script.fail_at <= 4
    &&& state.attempted == stage - 1
    &&& state.completed == stage - 1
}

spec fn after_v1(before: SettlementProbeV1, stage: u8,
    writer_seen: Option<u64>, publication_seen: Option<u64>) -> SettlementProbeV1 {
    SettlementProbeV1 {
        script: before.script,
        attempted: stage,
        completed: if before.script.fail_at == stage { (stage - 1) as u8 } else { stage },
        writer_seen,
        publication_seen,
    }
}

spec fn step_result_v1(script: ScriptV1, stage: u8) -> Result<(), FailureV1> {
    if script.fail_at == stage { Err(FailureV1 { stage, code: script.error }) } else { Ok(()) }
}

impl SettlementProbeV1 {
    fn record_v1(&mut self, stage: u8, writer_seen: Option<u64>, publication_seen: Option<u64>)
        -> (result: Result<(), FailureV1>)
        requires ready_v1(*old(self), stage),
        ensures *final(self) == after_v1(*old(self), stage, writer_seen, publication_seen),
            result == step_result_v1(old(self).script, stage),
    {
        self.attempted = stage;
        self.writer_seen = writer_seen;
        self.publication_seen = publication_seen;
        if self.script.fail_at == stage {
            Err(FailureV1 { stage, code: self.script.error })
        } else {
            self.completed = stage;
            Ok(())
        }
    }

    fn release_submission_inputs_v1(&mut self, submission: u64) -> (result: Result<(), FailureV1>)
        requires ready_v1(*old(self), 1), submission == old(self).script.submission,
        ensures *final(self) == after_v1(*old(self), 1, old(self).writer_seen, old(self).publication_seen),
            result == step_result_v1(old(self).script, 1),
    {
        self.record_v1(1, self.writer_seen, self.publication_seen)
    }

    fn settle_submission_writer_v1(&mut self, submission: u64, outcome: u64)
        -> (result: Result<(), FailureV1>)
        requires ready_v1(*old(self), 2), submission == old(self).script.submission,
            outcome == old(self).script.outcome,
        ensures *final(self) == after_v1(*old(self), 2, Some(outcome), old(self).publication_seen),
            result == step_result_v1(old(self).script, 2),
    {
        self.record_v1(2, Some(outcome), self.publication_seen)
    }

    fn release_operation_dependencies_v1(&mut self, submission: u64) -> (result: Result<(), FailureV1>)
        requires ready_v1(*old(self), 3), submission == old(self).script.submission,
        ensures *final(self) == after_v1(*old(self), 3, old(self).writer_seen, old(self).publication_seen),
            result == step_result_v1(old(self).script, 3),
    {
        self.record_v1(3, self.writer_seen, self.publication_seen)
    }

    fn publish_submission_status_v1(&mut self, submission: u64, status: u64)
        -> (result: Result<u64, FailureV1>)
        requires ready_v1(*old(self), 4), submission == old(self).script.submission,
            status == old(self).script.status,
        ensures *final(self) == after_v1(*old(self), 4, old(self).writer_seen, Some(status)),
            result == if old(self).script.fail_at == 4 {
                Err(FailureV1 { stage: 4, code: old(self).script.error })
            } else { Ok(old(self).script.returned) },
    {
        match self.record_v1(4, self.writer_seen, Some(status)) {
            Ok(()) => Ok(self.script.returned),
            Err(error) => Err(error),
        }
    }

    fn settle_v1(&mut self, submission: u64, status: u64, outcome: u64)
        -> (result: Result<u64, FailureV1>)
        requires ready_v1(*old(self), 1), old(self).writer_seen.is_none(),
            old(self).publication_seen.is_none(), submission == old(self).script.submission,
            status == old(self).script.status, outcome == old(self).script.outcome,
        ensures
            final(self).script == old(self).script,
            final(self).attempted == if old(self).script.fail_at == 0 { 4 } else { old(self).script.fail_at },
            final(self).completed == if old(self).script.fail_at == 0 { 4 } else { old(self).script.fail_at - 1 },
            final(self).writer_seen == if old(self).script.fail_at == 1 { None } else { Some(outcome) },
            final(self).publication_seen == if old(self).script.fail_at == 0 || old(self).script.fail_at == 4 {
                Some(status)
            } else { None },
            result == if old(self).script.fail_at == 0 { Ok(old(self).script.returned) }
                else { Err(FailureV1 { stage: old(self).script.fail_at, code: old(self).script.error }) },
    {
        completion_settlement_execution_body!(verus_exec_expr, self, submission, status, outcome)
    }
}

}
