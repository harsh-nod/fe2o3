// Frozen from 30aac7720c4a2eef34da3e3682ec5b74784ed61d; only method names/visibility change.
use super::*;

impl ContextProducerReadJournalV1 {
    pub(crate) fn baseline_acquire_reads_v1(
        &mut self,
        consumer: ContextWriterKeyV1,
        requests: &[ContextAllocationReadV1],
        output: &mut [Option<ContextReadLeaseReferenceV1>],
    ) -> Result<(), ContextVersionJournalErrorV1> {
        // Preserve stable-reader header error precedence before the shared budget.
        use ContextVersionJournalErrorV1 as E;
        if consumer.context_generation != self.context_generation() {
            return Err(E::ForeignContext);
        }
        if consumer.local == 0 || consumer.local == u64::MAX {
            return Err(E::InvalidWriterId);
        }
        if requests.is_empty() || requests.len() != output.len() {
            return Err(E::RosterCapacity);
        }
        if output.iter().any(Option::is_some) {
            return Err(E::InvalidState);
        }
        self.validate_read_capacity(requests.len())?;
        self.stable.acquire_reads(consumer, requests, output)
    }

    pub(crate) fn baseline_release_reads_v1(
        &mut self,
        consumer: ContextWriterKeyV1,
        references: &[ContextReadLeaseReferenceV1],
        evidence: &ContextReadQuiescenceEvidenceV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        self.stable.release_reads(consumer, references, evidence)
    }
}
