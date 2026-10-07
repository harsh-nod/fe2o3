// Frozen from 4c237366268542e1a19542b25c00f7b58d9f24a8; only method names/visibility change.
use super::*;

impl ContextProducerReadJournalV1 {
    pub(crate) fn baseline_retained_producer_read_count_v1(&self) -> usize {
        self.reservations.len() - self.free.len()
    }

    pub(crate) fn baseline_retained_read_count_v1(&self) -> usize {
        self.stable.baseline_retained_read_count_v1()
            + self.baseline_retained_producer_read_count_v1()
    }

    pub(crate) fn baseline_remaining_read_slots_v1(&self) -> usize {
        self.reservations.len() - self.baseline_retained_read_count_v1()
    }

    pub(crate) fn baseline_validate_read_capacity_v1(
        &self,
        count: usize,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        if count > self.baseline_remaining_read_slots_v1() {
            return Err(ContextVersionJournalErrorV1::MemberCapacity);
        }
        self.stable.baseline_validate_read_capacity_v1(count)
    }

    pub(crate) fn baseline_validate_producer_read_capacity_v1(
        &self,
        count: usize,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        if count > self.baseline_remaining_read_slots_v1() {
            return Err(ContextVersionJournalErrorV1::MemberCapacity);
        }
        if self.next_incarnation == 0 || self.next_incarnation.checked_add(count as u64).is_none() {
            return Err(ContextVersionJournalErrorV1::EpochExhausted);
        }
        Ok(())
    }

    pub(crate) fn baseline_acquire_producer_reads_v1(
        &mut self,
        consumer: ContextWriterKeyV1,
        requests: &[ContextProducerReadV1],
        output: &mut [Option<ContextProducerReadReferenceV1>],
    ) -> Result<(), ContextVersionJournalErrorV1> {
        use ContextVersionJournalErrorV1 as E;
        if consumer.context_generation != self.context_generation() {
            return Err(E::ForeignContext);
        }
        if consumer.local == 0
            || consumer.local == u64::MAX
            || consumer.kind != ContextWriterKindV1::Submission
        {
            return Err(E::InvalidWriterId);
        }
        if requests.is_empty() || requests.len() != output.len() {
            return Err(E::RosterCapacity);
        }
        if output.iter().any(Option::is_some) {
            return Err(E::InvalidState);
        }
        self.baseline_validate_producer_read_capacity_v1(requests.len())?;
        let mut previous = None;
        let mut group = 0;
        for (index, request) in requests.iter().enumerate() {
            self.validate_producer_read(request)?;
            if request.producer.key.local >= consumer.local {
                return Err(E::InvalidWriterId);
            }
            let key = read_key(request);
            if previous.is_some_and(|prior| prior >= key) {
                return Err(E::NonCanonicalRoster);
            }
            group = if previous.is_some_and(|prior: (u64, u64, u64)| prior.0 == key.0) {
                group + 1
            } else {
                1
            };
            if self.counts[request.read.allocation.slot]
                .checked_add(group)
                .is_none_or(|count| count > self.reservations.len())
            {
                return Err(E::InvalidState);
            }
            let slot = self.free[self.free.len() - index - 1];
            if self.reservations.get(slot) != Some(&None) {
                return Err(E::InvalidState);
            }
            previous = Some(key);
        }
        for (index, request) in requests.iter().enumerate() {
            let slot = self.free.pop().expect("preflighted free reservation");
            let reference = ContextProducerReadReferenceV1 {
                slot,
                incarnation: self.next_incarnation + index as u64,
                consumer,
            };
            self.reservations[slot] = Some(ReservationV1 {
                reference,
                request: *request,
            });
            self.counts[request.read.allocation.slot] += 1;
            output[index] = Some(reference);
        }
        self.next_incarnation += requests.len() as u64;
        Ok(())
    }
}
