use super::*;

mod disposal;

impl ContextVersionJournalV1 {
    /// Settles a complete retained writer using an inert success premise.
    pub fn settle_success(
        &mut self,
        writer: ContextWriterReferenceV1,
        evidence: &ContextWriterSuccessEvidenceV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        self.settle_retained(writer, evidence.writer, true)
    }

    /// Preserves burned attempt epochs and prior lineage; no authority is minted.
    pub fn settle_no_effect(
        &mut self,
        writer: ContextWriterReferenceV1,
        evidence: &ContextWriterNoEffectEvidenceV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        self.settle_retained(writer, evidence.writer, false)
    }

    /// Retains the complete writer chain until separately authorized recovery.
    pub fn mark_unknown(
        &mut self,
        writer: ContextWriterReferenceV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        retained::shared_retained_unknown_v1(self, writer)
    }

    fn retained_header(
        &self,
        writer: ContextWriterReferenceV1,
        allow_unknown: bool,
    ) -> Result<(Option<usize>, usize, bool), ContextVersionJournalErrorV1> {
        retained::shared_retained_header_v1(self, writer, allow_unknown)
    }

    fn validate_retained_chain(
        &self,
        writer: ContextWriterReferenceV1,
        head: Option<usize>,
        count: usize,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        retained::shared_retained_chain_v1(self, writer, head, count)
    }

    pub(super) fn preflight_settlement(
        &self,
        writer: ContextWriterReferenceV1,
        evidence: ContextWriterReferenceV1,
    ) -> Result<(Option<usize>, usize), ContextVersionJournalErrorV1> {
        let (head, count, _) = self.retained_header(writer, false)?;
        if evidence != writer {
            return Err(ContextVersionJournalErrorV1::SettlementEvidenceMismatch);
        }
        self.validate_retained_chain(writer, head, count)?;
        SettlementReturnStorageV1 {
            writer_free_len: self.free.len(),
            member_free_len: self.member_free.len(),
            writer_limit: self.writer_capacity,
            writer_storage: self.free.capacity(),
            member_limit: self.allocation_capacity,
            member_storage: self.member_free.capacity(),
            scratch_len: self.scratch.len(),
        }
        .check(count)?;
        for index in 0..count {
            self.count_indexed_access();
            if self.scratch[index].is_some() {
                return Err(ContextVersionJournalErrorV1::InvalidState);
            }
        }
        Ok((head, count))
    }

    fn settle_retained(
        &mut self,
        writer: ContextWriterReferenceV1,
        evidence: ContextWriterReferenceV1,
        success: bool,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        let (mut head, count) = self.preflight_settlement(writer, evidence)?;

        // All touched custody and return capacity are validated under this borrow.
        for index in 0..count {
            let slot = head.expect("validated complete retained chain");
            self.count_indexed_access();
            let member = self.members[slot].expect("validated retained member");
            self.store_plan(
                index,
                BeginMemberPlanV1 {
                    member_slot: slot,
                    allocation: member.allocation,
                    prior_lineage: member.prior_lineage,
                    attempt_epoch: member.attempt_epoch,
                },
            );
            head = member.next;
        }
        for index in 0..count {
            self.count_indexed_access();
            let plan = self.scratch[index]
                .take()
                .expect("complete settlement plan");
            self.count_indexed_access();
            let allocation = self.allocations[plan.allocation.slot]
                .as_mut()
                .expect("validated exact retained allocation");
            if success {
                allocation.content_lineage = plan.attempt_epoch;
            }
            allocation.pending_member = None;
            self.count_indexed_access();
            self.members[plan.member_slot] = None;
            self.count_indexed_access();
            self.member_free.push(plan.member_slot);
        }
        self.store_slot(writer.slot, None);
        self.push_free(writer.slot);
        Ok(())
    }
}
