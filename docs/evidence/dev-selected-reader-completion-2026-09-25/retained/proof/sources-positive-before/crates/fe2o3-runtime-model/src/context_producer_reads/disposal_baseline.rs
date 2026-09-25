// Frozen at c0f0766d3fb3c5a588dcfb1a6c1355f9efb73cb8; only method names and visibility are redirected.
use super::*;

impl ContextProducerReadJournalV1 {
    pub(crate) fn baseline_validate_unknown_disposal_v1(
        &self,
        writer: ContextWriterReferenceV1,
        members: &[ContextAllocationWriteV1],
    ) -> Result<(), ContextVersionJournalErrorV1> {
        self.require_unread(members.iter().map(|member| member.allocation))?;
        self.stable
            .baseline_validate_unknown_disposal_v1(writer, members)
    }

    pub(crate) fn baseline_dispose_unknown_v1(
        &mut self,
        writer: ContextWriterReferenceV1,
        evidence: &ContextWriterDisposalEvidenceV1<'_>,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        self.baseline_validate_unknown_disposal_v1(writer, evidence.allocations)?;
        self.stable.baseline_dispose_unknown_v1(writer, evidence)
    }
}
