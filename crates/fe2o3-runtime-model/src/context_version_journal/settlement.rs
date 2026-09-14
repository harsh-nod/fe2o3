use super::*;

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
        let (head, count, unknown) = self.retained_header(writer, true)?;
        self.validate_retained_chain(writer, head, count)?;
        if !unknown {
            self.store_slot(
                writer.slot,
                Some(WriterEntryV1::Unknown {
                    key: writer.key,
                    head,
                    count,
                }),
            );
        }
        Ok(())
    }

    fn retained_header(
        &self,
        writer: ContextWriterReferenceV1,
        allow_unknown: bool,
    ) -> Result<(Option<usize>, usize, bool), ContextVersionJournalErrorV1> {
        let (key, head, count, unknown) = match self.read_slot(writer.slot).copied().flatten() {
            Some(WriterEntryV1::Pending { key, head, count }) => (key, head, count, false),
            Some(WriterEntryV1::Unknown { key, head, count }) if allow_unknown => {
                (key, head, count, true)
            }
            _ => return Err(ContextVersionJournalErrorV1::InvalidReference),
        };
        if key != writer.key || key.context_generation != self.context_generation {
            return Err(ContextVersionJournalErrorV1::InvalidReference);
        }
        Ok((head, count, unknown))
    }

    fn validate_retained_chain(
        &self,
        writer: ContextWriterReferenceV1,
        mut head: Option<usize>,
        count: usize,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        if count > self.allocation_capacity || (count == 0) != head.is_none() {
            return Err(ContextVersionJournalErrorV1::InvalidState);
        }
        let mut previous_key = None;
        for _ in 0..count {
            let slot = head.ok_or(ContextVersionJournalErrorV1::InvalidState)?;
            self.count_indexed_access();
            let member = self
                .members
                .get(slot)
                .and_then(Option::as_ref)
                .ok_or(ContextVersionJournalErrorV1::InvalidState)?;
            if member.writer != writer
                || previous_key.is_some_and(|key| key >= member.allocation.key)
            {
                return Err(ContextVersionJournalErrorV1::InvalidState);
            }
            let allocation = self
                .exact_allocation(member.allocation)
                .map_err(|_| ContextVersionJournalErrorV1::InvalidState)?;
            if allocation.pending_member != Some(slot)
                || allocation.attempt_epoch != member.attempt_epoch
                || allocation.content_lineage != member.prior_lineage
                || member.prior_lineage >= member.attempt_epoch
            {
                return Err(ContextVersionJournalErrorV1::InvalidState);
            }
            previous_key = Some(member.allocation.key);
            head = member.next;
        }
        if head.is_some() {
            return Err(ContextVersionJournalErrorV1::InvalidState);
        }
        Ok(())
    }

    fn settle_retained(
        &mut self,
        writer: ContextWriterReferenceV1,
        evidence: ContextWriterReferenceV1,
        success: bool,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        let (mut head, count, _) = self.retained_header(writer, false)?;
        if evidence != writer {
            return Err(ContextVersionJournalErrorV1::SettlementEvidenceMismatch);
        }
        self.validate_retained_chain(writer, head, count)?;
        let writer_returns = self
            .free
            .len()
            .checked_add(1)
            .ok_or(ContextVersionJournalErrorV1::InvalidState)?;
        let member_returns = self
            .member_free
            .len()
            .checked_add(count)
            .ok_or(ContextVersionJournalErrorV1::InvalidState)?;
        if writer_returns > self.writer_capacity
            || writer_returns > self.free.capacity()
            || member_returns > self.allocation_capacity
            || member_returns > self.member_free.capacity()
            || count > self.scratch.len()
        {
            return Err(ContextVersionJournalErrorV1::InvalidState);
        }
        for index in 0..count {
            self.count_indexed_access();
            if self.scratch[index].is_some() {
                return Err(ContextVersionJournalErrorV1::InvalidState);
            }
        }

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
