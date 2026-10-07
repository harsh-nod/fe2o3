//! Deferred reads bind writer identities, never speculative content versions.

use super::*;

include!("read_resolution_body.rs");
include!("read_query_bodies.rs");
#[cfg(test)]
include!("read_resolution_baseline.rs");
#[cfg(test)]
include!("read_query_baseline.rs");

#[cfg(test)]
#[path = "read_query_tests.rs"]
mod query_tests;

macro_rules! queued_read_rust_expr {
    ($body:block) => {
        $body
    };
}

macro_rules! context_queued_read_declarations_v1 {
    ($($items:tt)*) => { $($items)* };
}

include!("read_declarations.rs");

fn queued_read_reference_same_v1(
    left: ContextQueuedProducerReadReferenceV1,
    right: ContextQueuedProducerReadReferenceV1,
) -> bool {
    queued_read_reference_same_body_v1!(left, right)
}
impl ContextQueuedWriterJournalV1 {
    pub fn retained_queued_read_count(&self) -> usize {
        self.queued_reads.len() - self.free_reads.len()
    }

    pub(super) fn validate_total_read_capacity(&self, count: usize) -> Result<(), Error> {
        if count > self.remaining_read_slots() {
            return Err(Error::MemberCapacity);
        }
        Ok(())
    }

    pub fn validate_queued_producer_read_capacity(&self, count: usize) -> Result<(), Error> {
        self.ensure_usable()?;
        self.validate_total_read_capacity(count)?;
        if self.next_read_incarnation == 0
            || self
                .next_read_incarnation
                .checked_add(count as u64)
                .is_none()
        {
            return Err(Error::EpochExhausted);
        }
        Ok(())
    }

    pub fn validate_queued_producer_read(
        &self,
        request: &ContextQueuedProducerReadV1,
    ) -> Result<(), Error> {
        self.validate_queued_read_shape(request)?;
        if !matches!(
            self.queued_writer_status(request.producer)?,
            Some(ContextQueuedWriterStatusV1::Waiting | ContextQueuedWriterStatusV1::Ready)
        ) {
            return Err(Error::AllocationBusy);
        }
        Ok(())
    }

    fn validate_queued_read_shape(
        &self,
        request: &ContextQueuedProducerReadV1,
    ) -> Result<(), Error> {
        self.destination(request.allocation)?;
        if request.byte_len == 0
            || request
                .byte_offset
                .checked_add(request.byte_len)
                .is_none_or(|end| end > request.allocation.byte_extent)
        {
            return Err(Error::InvalidExtent);
        }
        if request.producer.key.kind != ContextWriterKindV1::Submission
            || self.latest_writer(request.allocation.allocation)? != Some(request.producer)
        {
            return Err(Error::AllocationBusy);
        }
        Ok(())
    }

    fn read_entry(&self, slot: usize) -> Result<Reservation, Error> {
        queued_read_entry_body_v1!(self, slot)
    }

    fn validate_read_links(&self, entry: Reservation) -> Result<(), Error> {
        queued_read_links_body_v1!(self, entry)
    }

    fn inspect_queued_read(
        &self,
        reference: ContextQueuedProducerReadReferenceV1,
    ) -> Result<Reservation, Error> {
        queued_read_inspect_body_v1!(self, reference)
    }

    pub fn lookup_queued_producer_read(
        &self,
        reference: ContextQueuedProducerReadReferenceV1,
    ) -> Result<ContextQueuedProducerReadV1, Error> {
        queued_read_lookup_body_v1!(self, reference)
    }

    pub fn queued_producer_read_status(
        &self,
        reference: ContextQueuedProducerReadReferenceV1,
    ) -> Result<ContextProducerReadStatusV1, Error> {
        queued_read_status_body_v1!(self, reference)
    }

    /// Complete all preflights before either owner commits. All three families
    /// share R slots; the queued tail is allocation-free and cannot reject.
    /// Validation sorts Q producer references and visits each distinct producer's
    /// destination roster once, rather than once per queued read.
    pub fn acquire_mixed_reads_with_queued(
        &mut self,
        consumer: ContextWriterKeyV1,
        stable: (
            &[ContextAllocationReadV1],
            &mut [Option<ContextReadLeaseReferenceV1>],
        ),
        active: (
            &[ContextProducerReadV1],
            &mut [Option<ContextProducerReadReferenceV1>],
        ),
        queued: (
            &[ContextQueuedProducerReadV1],
            &mut [Option<ContextQueuedProducerReadReferenceV1>],
        ),
    ) -> Result<(), Error> {
        self.ensure_usable()?;
        self.inner
            .preflight_acquire_mixed_reads_v1(consumer, stable.0, stable.1, active.0, active.1)?;
        let count = stable
            .0
            .len()
            .checked_add(active.0.len())
            .and_then(|n| n.checked_add(queued.0.len()))
            .ok_or(Error::MemberCapacity)?;
        self.validate_total_read_capacity(count)?;
        for read in stable.0 {
            self.require_unqueued(read.allocation)?;
        }
        for read in active.0 {
            self.require_unqueued(read.read.allocation)?;
        }
        if queued.0.len() != queued.1.len() {
            return Err(Error::RosterCapacity);
        }
        if queued.1.iter().any(Option::is_some) {
            return Err(Error::InvalidState);
        }
        self.validate_queued_producer_read_capacity(queued.0.len())?;
        // Validate each complete producer roster once, independent of read fanout.
        self.read_producer_scratch.clear();
        self.read_producer_scratch
            .extend(queued.0.iter().map(|request| request.producer));
        self.read_producer_scratch.sort_unstable_by_key(|writer| {
            (writer.key.context_generation, writer.key.local, writer.slot)
        });
        self.read_producer_scratch.dedup();
        let result = (|| {
            for &producer in &self.read_producer_scratch {
                if !matches!(
                    self.queued_writer_status(producer)?,
                    Some(ContextQueuedWriterStatusV1::Waiting | ContextQueuedWriterStatusV1::Ready)
                ) {
                    return Err(Error::AllocationBusy);
                }
            }
            let mut previous = None;
            let mut group = 0usize;
            for (ordinal, request) in queued.0.iter().enumerate() {
                self.validate_queued_read_shape(request)?;
                if request.producer.key.local >= consumer.local {
                    return Err(Error::InvalidWriterId);
                }
                let key = (
                    request.allocation.allocation.key.local,
                    request.byte_offset,
                    request.byte_len,
                );
                if previous.is_some_and(|prior| prior >= key) {
                    return Err(Error::NonCanonicalRoster);
                }
                group = if previous.is_some_and(|prior: (u64, u64, u64)| prior.0 == key.0) {
                    group + 1
                } else {
                    1
                };
                if self.read_counts[request.allocation.allocation.slot]
                    .checked_add(group)
                    .is_none_or(|n| n > self.queued_reads.len())
                {
                    return Err(Error::InvalidState);
                }
                previous = Some(key);
                let root = self.root(request.producer)?;
                if root
                    .read_count
                    .checked_add(queued.0.len())
                    .is_none_or(|n| n > self.queued_reads.len())
                {
                    return Err(Error::InvalidState);
                }
                if let Some(head) = root.read_head {
                    let entry = self.read_entry(head)?;
                    if entry.request.producer != request.producer || entry.previous.is_some() {
                        return Err(Error::InvalidState);
                    }
                    self.validate_read_links(entry)?;
                } else if root.read_count != 0 {
                    return Err(Error::InvalidState);
                }
                let slot = self.free_reads[self.free_reads.len() - ordinal - 1];
                if self.queued_reads.get(slot).is_none_or(Option::is_some) {
                    return Err(Error::InvalidState);
                }
            }
            Ok(())
        })();
        self.read_producer_scratch.clear();
        result?;
        self.inner
            .acquire_mixed_reads(consumer, stable.0, stable.1, active.0, active.1)?;
        self.disposal_terminal = true;
        for (request, output) in queued.0.iter().zip(queued.1) {
            let slot = self.free_reads.pop().expect("preflighted queued read slot");
            let reference = ContextQueuedProducerReadReferenceV1 {
                slot,
                incarnation: self.next_read_incarnation,
                consumer,
            };
            let root = self.roots[request.producer.slot]
                .as_mut()
                .expect("preflighted producer root");
            if let Some(head) = root.read_head {
                self.queued_reads[head]
                    .as_mut()
                    .expect("preflighted read head")
                    .previous = Some(slot);
            }
            self.queued_reads[slot] = Some(Reservation {
                reference,
                request: *request,
                status: ContextProducerReadStatusV1::Pending,
                version: None,
                previous: None,
                next: root.read_head,
            });
            root.read_head = Some(slot);
            root.read_count += 1;
            self.read_counts[request.allocation.allocation.slot] += 1;
            self.next_read_incarnation += 1;
            *output = Some(reference);
        }
        self.disposal_terminal = false;
        Ok(())
    }

    pub(super) fn validate_attached_reads(&self, root: Root) -> Result<(), Error> {
        if root.read_count > self.queued_reads.len() {
            return Err(Error::InvalidState);
        }
        let mut next = root.read_head;
        let mut previous = None;
        for _ in 0..root.read_count {
            let slot = next.ok_or(Error::InvalidState)?;
            let entry = self.read_entry(slot)?;
            if entry.reference.slot != slot
                || entry.previous != previous
                || entry.request.producer != root.writer
                || entry.status != ContextProducerReadStatusV1::Pending
            {
                return Err(Error::InvalidState);
            }
            self.inspect_queued_read(entry.reference)?;
            previous = next;
            next = entry.next;
        }
        if next.is_some() {
            return Err(Error::InvalidState);
        }
        Ok(())
    }

    pub(super) fn resolve_reads(&mut self, root: Root, status: ContextProducerReadStatusV1) {
        queued_read_resolution_body!(
            queued_read_rust_expr,
            self,
            root,
            status,
            next,
            index,
            slot,
            entry,
            state,
            retained,
            [],
            [],
            [],
            [],
            [],
            []
        );
    }

    pub fn release_queued_producer_reads(
        &mut self,
        consumer: ContextWriterKeyV1,
        references: &[ContextQueuedProducerReadReferenceV1],
        evidence: &ContextReadQuiescenceEvidenceV1,
    ) -> Result<(), Error> {
        self.ensure_usable()?;
        if consumer != evidence.consumer {
            return Err(Error::SettlementEvidenceMismatch);
        }
        if references.is_empty() {
            return Err(Error::RosterCapacity);
        }
        if self
            .free_reads
            .len()
            .checked_add(references.len())
            .is_none_or(|n| n > self.queued_reads.len() || n > self.free_reads.capacity())
        {
            return Err(Error::InvalidState);
        }
        let mut previous = None;
        let mut group = 0;
        for &reference in references {
            if reference.consumer != consumer {
                return Err(Error::InvalidReference);
            }
            let entry = self.inspect_queued_read(reference)?;
            let key = (
                entry.request.allocation.allocation.key.local,
                entry.request.byte_offset,
                entry.request.byte_len,
                reference.incarnation,
            );
            if previous.is_some_and(|prior| prior >= key) {
                return Err(Error::NonCanonicalRoster);
            }
            group = if previous.is_some_and(|prior: (u64, u64, u64, u64)| prior.0 == key.0) {
                group + 1
            } else {
                1
            };
            if self.read_counts[entry.request.allocation.allocation.slot] < group {
                return Err(Error::InvalidState);
            }
            previous = Some(key);
        }
        self.disposal_terminal = true;
        for reference in references {
            let entry = self.queued_reads[reference.slot]
                .take()
                .expect("preflighted read release");
            if entry.status == ContextProducerReadStatusV1::Pending {
                if let Some(previous) = entry.previous {
                    self.queued_reads[previous]
                        .as_mut()
                        .expect("current previous read")
                        .next = entry.next;
                } else {
                    self.roots[entry.request.producer.slot]
                        .as_mut()
                        .expect("retained producer")
                        .read_head = entry.next;
                }
                if let Some(next) = entry.next {
                    self.queued_reads[next]
                        .as_mut()
                        .expect("current next read")
                        .previous = entry.previous;
                }
                self.roots[entry.request.producer.slot]
                    .as_mut()
                    .expect("retained producer")
                    .read_count -= 1;
            }
            self.read_counts[entry.request.allocation.allocation.slot] -= 1;
            self.free_reads.push(reference.slot);
        }
        self.disposal_terminal = false;
        Ok(())
    }

    pub(super) fn has_queued_consumer_reads_matching(
        &self,
        selected: impl Fn(ContextWriterKeyV1) -> bool,
    ) -> bool {
        self.retained_queued_read_count() != 0
            && self
                .queued_reads
                .iter()
                .flatten()
                .any(|entry| selected(entry.reference.consumer))
    }
}
