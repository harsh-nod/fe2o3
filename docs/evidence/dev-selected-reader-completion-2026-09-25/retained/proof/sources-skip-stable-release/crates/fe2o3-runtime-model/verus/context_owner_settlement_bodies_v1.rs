verus! {

fn settlement_scratch_access_v1(journal: &JournalContentsV1) {}
fn settlement_commit_access_v1(journal: &JournalContentsV1) {}

impl SettlementReturnStorageV1 {
    fn check(&self, count: usize) -> (result: Result<(), ReadErrorV1>)
        ensures result == shared_settlement_storage_decision_v1(*self, count),
    {
        settlement_return_admission_body!(self, count, ReadErrorV1::InvalidState)
    }
}

fn shared_settlement_scratch_scan_v1(journal: &JournalContentsV1, count: usize)
    -> (result: Result<(), ReadErrorV1>)
    requires count <= journal.scratch@.len(),
    ensures result == settlement_scratch_scan_v1(*journal, count, 0),
{
    settlement_scratch_scan_body!(verus_exec_expr, journal, count, index, [
        invariant index <= count, count <= journal.scratch@.len(),
            settlement_scratch_scan_v1(*journal, count, 0)
                == settlement_scratch_scan_v1(*journal, count, index as nat),
        decreases count - index,
    ])
}

fn shared_settlement_scratch_stage_v1(journal: &mut JournalContentsV1, initial: Option<usize>, count: usize)
    requires settlement_storage_ready_v1(*old(journal), initial, count),
    ensures begin_stage_frame_v1(*old(journal), *final(journal)),
        final(journal).scratch@ == settlement_scratch_v1(*old(journal), initial, count, count as nat, 0),
{
    let ghost before = *journal;
    proof { assert(journal.scratch@ =~= settlement_scratch_v1(before, initial, count, 0, 0)); }
    settlement_scratch_stage_body!(verus_exec_expr, journal, initial, count, head, index, [
        invariant index <= count, before == *old(journal), settlement_storage_ready_v1(before, initial, count),
            begin_stage_frame_v1(before, *journal), head == settlement_cursor_v1(before, initial, index as nat),
            journal.scratch@.len() == before.scratch@.len(),
            index < count ==> settlement_plan_ready_v1(before, initial, index as nat),
            forall|i: int| 0 <= i < journal.scratch@.len() ==> (#[trigger] journal.scratch@[i])
                == settlement_scratch_v1(before, initial, count, index as nat, 0)[i],
        decreases count - index,
    ]);
    proof { assert(journal.scratch@ =~= settlement_scratch_v1(before, initial, count, count as nat, 0)); }
}

fn shared_settlement_commit_v1(journal: &mut JournalContentsV1, writer: WriterReferenceV1,
    count: usize, success: bool, head: Option<usize>, Ghost(before): Ghost<JournalContentsV1>)
    requires settlement_storage_ready_v1(before, head, count), writer.slot < before.writers@.len(),
        begin_stage_frame_v1(before, *old(journal)),
        old(journal).scratch@ == settlement_scratch_v1(before, head, count, count as nat, 0),
    ensures settlement_raw_success_relation_v1(before, *final(journal), writer, head, count, success),
{
    proof {
        settlement_prefix_shapes_v1(before, head, count, 0, success);
        assert(journal.member_free@ =~= before.member_free@ + settlement_slots_v1(before, head, 0));
        if count > 0 { assert(journal.scratch@[0] == Some(settlement_plan_at_v1(before, head, 0))); }
    }
    settlement_commit_body!(verus_exec_expr, journal, writer, count, success, index, [
        invariant index <= count, settlement_storage_ready_v1(before, head, count), writer.slot < before.writers@.len(),
            settlement_commit_frame_v1(before, *journal), journal.writers == before.writers, journal.free == before.free,
            forall|i: int| 0 <= i < journal.scratch@.len() ==> (#[trigger] journal.scratch@[i])
                == settlement_scratch_v1(before, head, count, count as nat, index as nat)[i],
            index < count ==> journal.scratch@[index as int] == Some(settlement_plan_at_v1(before, head, index as nat)),
            journal.allocations@ =~= settlement_allocations_prefix_v1(before, head, success, index as nat),
            journal.members@ =~= settlement_members_prefix_v1(before, head, index as nat),
            journal.member_free@ =~= before.member_free@ + settlement_slots_v1(before, head, index as nat),
            journal.members@.len() == before.members@.len(), journal.allocations@.len() == before.allocations@.len(),
            journal.scratch@.len() == before.scratch@.len(),
            forall|a: int| 0 <= a < before.allocations@.len() ==>
                (#[trigger] journal.allocations@[a]).is_some() == before.allocations@[a].is_some(),
            index < count ==> settlement_plan_ready_v1(before, head, index as nat),
        decreases count - index,
    ]);
    proof { assert(journal.scratch@ =~= before.scratch@); }
}

impl ContextVersionJournalV1 {
    // Observations do not assert physical allocation, authentication, or unwind behavior.
    fn preflight_settlement_observed_v1(&self, writer: WriterReferenceV1, evidence: WriterReferenceV1,
        free_storage: usize, member_free_storage: usize)
        -> (result: Result<(Option<usize>, usize), ReadErrorV1>)
        ensures result == settlement_preflight_decision_v1(*self, writer, evidence, free_storage, member_free_storage),
    {
        settlement_preflight_body!(verus_exec_expr, self, writer, evidence,
            free_storage, member_free_storage, shared_retained_header_v1,
            shared_retained_writer_key_v1, shared_retained_chain_v1, shared_settlement_scratch_scan_v1)
    }

    fn settle_retained_observed_v1(&mut self, writer: WriterReferenceV1, evidence: WriterReferenceV1, success: bool,
        free_storage: usize, member_free_storage: usize) -> (result: Result<(), ReadErrorV1>)
        ensures settlement_execution_relation_v1(*old(self), *final(self), writer, evidence,
            free_storage, member_free_storage, success, result),
    {
        settlement_execute_body!(verus_exec_expr, self, writer, evidence, success, head, count,
            preflight_settlement_observed_v1, [, free_storage, member_free_storage],
            shared_settlement_scratch_stage_v1, shared_settlement_commit_v1, [, head, Ghost(before)],
            [let ghost before = *self;],
            [proof { settlement_preflight_ready_v1(before, writer, evidence, free_storage, member_free_storage, head, count); }])
    }

    fn settle_success_observed_v1(&mut self, writer: WriterReferenceV1, evidence: &ContextWriterSuccessEvidenceV1,
        free_storage: usize, member_free_storage: usize) -> (result: Result<(), ReadErrorV1>)
        ensures settlement_execution_relation_v1(*old(self), *final(self), writer, evidence.writer,
            free_storage, member_free_storage, true, result),
    {
        settlement_outcome_body!(self, settle_retained_observed_v1, writer, evidence, true, [, free_storage, member_free_storage])
    }

    fn settle_no_effect_observed_v1(&mut self, writer: WriterReferenceV1, evidence: &ContextWriterNoEffectEvidenceV1,
        free_storage: usize, member_free_storage: usize) -> (result: Result<(), ReadErrorV1>)
        ensures settlement_execution_relation_v1(*old(self), *final(self), writer, evidence.writer,
            free_storage, member_free_storage, false, result),
    {
        settlement_outcome_body!(self, settle_retained_observed_v1, writer, evidence, false, [, free_storage, member_free_storage])
    }
}

impl ContextReadLeasedJournalV1 {
    fn settle_success_observed_v1(&mut self, writer: WriterReferenceV1, evidence: &ContextWriterSuccessEvidenceV1,
        free_storage: usize, member_free_storage: usize) -> (result: Result<(), ReadErrorV1>)
        ensures settlement_execution_relation_v1(old(self).journal, final(self).journal, writer, evidence.writer,
            free_storage, member_free_storage, true, result),
            owner_writer_stable_frame_v1(*old(self), *final(self)),
            result.is_err() ==> *final(self) == *old(self),
    {
        settlement_owner_forward_body!(self, journal, settle_success_observed_v1, writer, evidence, [, free_storage, member_free_storage])
    }

    fn settle_no_effect_observed_v1(&mut self, writer: WriterReferenceV1, evidence: &ContextWriterNoEffectEvidenceV1,
        free_storage: usize, member_free_storage: usize) -> (result: Result<(), ReadErrorV1>)
        ensures settlement_execution_relation_v1(old(self).journal, final(self).journal, writer, evidence.writer,
            free_storage, member_free_storage, false, result),
            owner_writer_stable_frame_v1(*old(self), *final(self)),
            result.is_err() ==> *final(self) == *old(self),
    {
        settlement_owner_forward_body!(self, journal, settle_no_effect_observed_v1, writer, evidence, [, free_storage, member_free_storage])
    }
}

impl ContextProducerReadJournalV1 {
    fn settle_success_observed_v1(&mut self, writer: WriterReferenceV1, evidence: &ContextWriterSuccessEvidenceV1,
        free_storage: usize, member_free_storage: usize) -> (result: Result<(), ReadErrorV1>)
        ensures settlement_execution_relation_v1(old(self).stable.journal, final(self).stable.journal, writer, evidence.writer,
            free_storage, member_free_storage, true, result),
            owner_writer_producer_frame_v1(*old(self), *final(self)),
            result.is_err() ==> *final(self) == *old(self),
    {
        settlement_owner_forward_body!(self, stable, settle_success_observed_v1, writer, evidence, [, free_storage, member_free_storage])
    }

    fn settle_no_effect_observed_v1(&mut self, writer: WriterReferenceV1, evidence: &ContextWriterNoEffectEvidenceV1,
        free_storage: usize, member_free_storage: usize) -> (result: Result<(), ReadErrorV1>)
        ensures settlement_execution_relation_v1(old(self).stable.journal, final(self).stable.journal, writer, evidence.writer,
            free_storage, member_free_storage, false, result),
            owner_writer_producer_frame_v1(*old(self), *final(self)),
            result.is_err() ==> *final(self) == *old(self),
    {
        settlement_owner_forward_body!(self, stable, settle_no_effect_observed_v1, writer, evidence, [, free_storage, member_free_storage])
    }
}

}
