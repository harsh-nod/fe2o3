// Frozen at 269bb2c3d57b1221ee5e8871bf935d61dbacdc27; only names and visibility are redirected.
use super::*;

impl ContextVersionJournalV1 {
    pub(crate) fn baseline_settle_success_v1(
        &mut self,
        writer: ContextWriterReferenceV1,
        evidence: &ContextWriterSuccessEvidenceV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        self.baseline_settle_retained_v1(writer, evidence.writer, true)
    }

    pub(crate) fn baseline_settle_no_effect_v1(
        &mut self,
        writer: ContextWriterReferenceV1,
        evidence: &ContextWriterNoEffectEvidenceV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        self.baseline_settle_retained_v1(writer, evidence.writer, false)
    }

    pub(crate) fn baseline_preflight_settlement_v1(
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

    pub(crate) fn baseline_settle_retained_v1(
        &mut self,
        writer: ContextWriterReferenceV1,
        evidence: ContextWriterReferenceV1,
        success: bool,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        let (head, count) = self.baseline_preflight_settlement_v1(writer, evidence)?;

        // All touched custody and return capacity are validated under this borrow.
        settlement_scratch::shared_settlement_scratch_stage_v1(self, head, count);
        settlement_commit::shared_settlement_commit_v1(self, writer, count, success);
        Ok(())
    }
}
