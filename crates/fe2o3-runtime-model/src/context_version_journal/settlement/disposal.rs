use super::*;

impl ContextVersionJournalV1 {
    /// Preflights the complete Unknown roster without authorizing native effects.
    /// Partial disposal must remain retained by the production owner until every
    /// member is confirmed gone. This model never clears a successful prefix.
    pub fn validate_unknown_disposal(
        &self,
        writer: ContextWriterReferenceV1,
        canonical: &[ContextAllocationWriteV1],
    ) -> Result<(), ContextVersionJournalErrorV1> {
        self.unknown_disposal_plan(writer, canonical).map(|_| ())
    }

    /// Retires an exact Unknown writer and all its allocations atomically.
    /// No allocation becomes available, and no successful lineage is published.
    /// Writer issuance history remains burned. Callers must not reuse disposed
    /// allocation keys. This operation is O(k), with no allocation or callback;
    /// partition preservation assumes a valid prestate.
    pub fn dispose_unknown(
        &mut self,
        writer: ContextWriterReferenceV1,
        evidence: &ContextWriterDisposalEvidenceV1<'_>,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        self.retained_header(writer, true)?;
        if evidence.writer != writer {
            return Err(ContextVersionJournalErrorV1::SettlementEvidenceMismatch);
        }
        let (mut head, count) = self.unknown_disposal_plan(writer, evidence.allocations)?;
        for index in 0..count {
            let slot = head.expect("validated complete Unknown chain");
            let member = self.members[slot].expect("validated Unknown member");
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
        // Disposal removes membership and allocation together. It does not pass
        // through a state that could be mistaken for recovered content.
        for index in 0..count {
            let plan = self.scratch[index].take().expect("complete disposal plan");
            self.allocations[plan.allocation.slot] = None;
            self.allocation_free.push(plan.allocation.slot);
            self.members[plan.member_slot] = None;
            self.member_free.push(plan.member_slot);
        }
        self.store_slot(writer.slot, None);
        self.push_free(writer.slot);
        Ok(())
    }

    fn unknown_disposal_plan(
        &self,
        writer: ContextWriterReferenceV1,
        canonical: &[ContextAllocationWriteV1],
    ) -> Result<(Option<usize>, usize), ContextVersionJournalErrorV1> {
        use ContextVersionJournalErrorV1 as E;
        let (head, count, unknown) = self.retained_header(writer, true)?;
        if !unknown {
            return Err(E::InvalidState);
        }
        if canonical.len() != count {
            return Err(E::SettlementEvidenceMismatch);
        }
        self.validate_retained_chain(writer, head, count)?;
        let mut cursor = head;
        for destination in canonical {
            let slot = cursor.ok_or(E::InvalidState)?;
            let member = self.members[slot].ok_or(E::InvalidState)?;
            let allocation = self.exact_allocation(member.allocation)?;
            if destination.allocation != member.allocation
                || destination.device != allocation.device
                || destination.byte_extent != allocation.byte_extent
            {
                return Err(E::SettlementEvidenceMismatch);
            }
            cursor = member.next;
        }
        let writer_returns = self.free.len().checked_add(1).ok_or(E::InvalidState)?;
        let member_returns = self
            .member_free
            .len()
            .checked_add(count)
            .ok_or(E::InvalidState)?;
        let allocation_returns = self
            .allocation_free
            .len()
            .checked_add(count)
            .ok_or(E::InvalidState)?;
        if writer_returns > self.writer_capacity
            || writer_returns > self.free.capacity()
            || member_returns > self.allocation_capacity
            || member_returns > self.member_free.capacity()
            || allocation_returns > self.allocation_capacity
            || allocation_returns > self.allocation_free.capacity()
            || count > self.scratch.len()
            || self.scratch[..count].iter().any(Option::is_some)
        {
            return Err(E::InvalidState);
        }
        Ok((head, count))
    }
}
