// Frozen from db5dd95e182ca7c90a00e3e98d234710c90c6264; only method name/visibility changes.
use super::*;

impl ContextProducerReadJournalV1 {
    pub(crate) fn baseline_release_producer_reads_v1(
        &mut self,
        consumer: ContextWriterKeyV1,
        references: &[ContextProducerReadReferenceV1],
        evidence: &ContextReadQuiescenceEvidenceV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        use ContextVersionJournalErrorV1 as E;
        if evidence.consumer != consumer {
            return Err(E::SettlementEvidenceMismatch);
        }
        if references.is_empty() {
            return Err(E::RosterCapacity);
        }
        if self
            .free
            .len()
            .checked_add(references.len())
            .is_none_or(|count| count > self.reservations.len() || count > self.free.capacity())
        {
            return Err(E::InvalidState);
        }
        let mut previous = None;
        let mut group = 0;
        for reference in references {
            if reference.consumer != consumer {
                return Err(E::InvalidReference);
            }
            let request = self.lookup_producer_read(*reference)?;
            let key = (read_key(&request), reference.incarnation);
            if previous.is_some_and(|prior| prior >= key) {
                return Err(E::NonCanonicalRoster);
            }
            group = if previous.is_some_and(|prior: ((u64, u64, u64), u64)| prior.0.0 == key.0.0) {
                group + 1
            } else {
                1
            };
            if self.counts[request.read.allocation.slot] < group {
                return Err(E::InvalidState);
            }
            previous = Some(key);
        }
        for reference in references {
            let entry = self.reservations[reference.slot]
                .take()
                .expect("preflighted reservation");
            self.counts[entry.request.read.allocation.slot] -= 1;
            self.free.push(reference.slot);
        }
        Ok(())
    }
}
