// Frozen from 455d3c62a03f1033b5c63e40634592a583e42484; only method names/visibility change.
use super::*;

impl ContextReadLeasedJournalV1 {
    pub(crate) fn baseline_lookup_read_v1(
        &self,
        reference: ContextReadLeaseReferenceV1,
    ) -> Result<ContextAllocationReadV1, ContextVersionJournalErrorV1> {
        let entry = self
            .leases
            .get(reference.slot)
            .copied()
            .flatten()
            .filter(|entry| entry.reference == reference)
            .ok_or(ContextVersionJournalErrorV1::InvalidReference)?;
        self.validate_read(&entry.request)?;
        Ok(entry.request)
    }

    pub(crate) fn baseline_release_reads_v1(
        &mut self,
        consumer: ContextWriterKeyV1,
        references: &[ContextReadLeaseReferenceV1],
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
            .free_reads
            .len()
            .checked_add(references.len())
            .is_none_or(|count| count > self.leases.len() || count > self.free_reads.capacity())
        {
            return Err(E::InvalidState);
        }
        let mut previous = None;
        let mut group = 0;
        for reference in references {
            if reference.consumer != consumer {
                return Err(E::InvalidReference);
            }
            let request = self.baseline_lookup_read_v1(*reference)?;
            let key = (read_key(&request), reference.incarnation);
            if previous.is_some_and(|prior| prior >= key) {
                return Err(E::NonCanonicalRoster);
            }
            group = if previous.is_some_and(|prior: ((u64, u64, u64), u64)| prior.0.0 == key.0.0) {
                group + 1
            } else {
                1
            };
            if self.readers[request.allocation.slot] < group {
                return Err(E::InvalidState);
            }
            previous = Some(key);
        }
        for reference in references {
            let entry = self.leases[reference.slot]
                .take()
                .expect("preflighted lease");
            self.readers[entry.request.allocation.slot] -= 1;
            self.free_reads.push(reference.slot);
        }
        Ok(())
    }
}
