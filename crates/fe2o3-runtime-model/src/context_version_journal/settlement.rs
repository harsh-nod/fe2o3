use super::*;

mod disposal;

#[cfg(test)]
#[path = "unknown_baseline.rs"]
mod unknown_baseline;

#[allow(unused_macros)]
#[macro_use]
mod unknown_templates {
    include!("unknown_wrapper_bodies.rs");
}

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
        journal_unknown_wrapper_body!(self, writer, retained::shared_retained_unknown_v1)
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
        settlement_scratch::shared_settlement_scratch_scan_v1(self, count)?;
        Ok((head, count))
    }

    fn settle_retained(
        &mut self,
        writer: ContextWriterReferenceV1,
        evidence: ContextWriterReferenceV1,
        success: bool,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        let (head, count) = self.preflight_settlement(writer, evidence)?;

        // All touched custody and return capacity are validated under this borrow.
        settlement_scratch::shared_settlement_scratch_stage_v1(self, head, count);
        settlement_commit::shared_settlement_commit_v1(self, writer, count, success);
        Ok(())
    }
}
