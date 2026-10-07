// Frozen at 269bb2c3d57b1221ee5e8871bf935d61dbacdc27; only names and visibility are redirected.
use super::*;

impl ContextProducerReadJournalV1 {
    pub(crate) fn baseline_settle_success_v1(
        &mut self,
        writer: ContextWriterReferenceV1,
        evidence: &ContextWriterSuccessEvidenceV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        self.stable.baseline_settle_success_v1(writer, evidence)
    }

    pub(crate) fn baseline_settle_no_effect_v1(
        &mut self,
        writer: ContextWriterReferenceV1,
        evidence: &ContextWriterNoEffectEvidenceV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        self.stable.baseline_settle_no_effect_v1(writer, evidence)
    }
}
