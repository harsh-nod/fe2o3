// Independent query baseline frozen from signed 3d4ee2d91.
// Missing count storage deliberately remains the old panicking path; tests exclude it.
impl ContextQueuedWriterJournalV1 {
    pub(super) fn baseline_ensure_usable(&self) -> Result<(), Error> {
        if self.disposal_terminal {
            Err(Error::InvalidState)
        } else {
            Ok(())
        }
    }

    fn baseline_root(&self, writer: ContextWriterReferenceV1) -> Result<Root, Error> {
        self.baseline_ensure_usable()?;
        self.inner.lookup_writer(writer)?;
        self.roots
            .get(writer.slot)
            .copied()
            .flatten()
            .filter(|root| root.writer == writer)
            .ok_or(Error::InvalidReference)
    }

    fn baseline_destination(
        &self,
        write: ContextAllocationWriteV1,
    ) -> Result<ContextAllocationStateV1, Error> {
        self.baseline_ensure_usable()?;
        let state = self.inner.lookup_allocation(write.allocation)?;
        if state.device != write.device {
            return Err(Error::AllocationDeviceMismatch);
        }
        if state.byte_extent != write.byte_extent {
            return Err(Error::AllocationExtentMismatch);
        }
        Ok(state)
    }

    fn baseline_read_entry(&self, slot: usize) -> Result<Reservation, Error> {
        self.queued_reads
            .get(slot)
            .copied()
            .flatten()
            .ok_or(Error::InvalidReference)
    }

    fn baseline_validate_read_links(&self, entry: Reservation) -> Result<(), Error> {
        if entry.status != ContextProducerReadStatusV1::Pending {
            return if entry.previous.is_none() && entry.next.is_none() {
                Ok(())
            } else {
                Err(Error::InvalidState)
            };
        }
        let root = self.baseline_root(entry.request.producer)?;
        if root.read_count == 0 || root.read_count > self.queued_reads.len() {
            return Err(Error::InvalidState);
        }
        if let Some(previous) = entry.previous {
            let previous = self.baseline_read_entry(previous)?;
            if previous.request.producer != entry.request.producer
                || previous.status != ContextProducerReadStatusV1::Pending
                || previous.next != Some(entry.reference.slot)
            {
                return Err(Error::InvalidState);
            }
        } else if root.read_head != Some(entry.reference.slot) {
            return Err(Error::InvalidState);
        }
        if let Some(next) = entry.next {
            let next = self.baseline_read_entry(next)?;
            if next.request.producer != entry.request.producer
                || next.status != ContextProducerReadStatusV1::Pending
                || next.previous != Some(entry.reference.slot)
            {
                return Err(Error::InvalidState);
            }
        }
        Ok(())
    }

    fn baseline_inspect_queued_read(
        &self,
        reference: ContextQueuedProducerReadReferenceV1,
    ) -> Result<Reservation, Error> {
        self.baseline_ensure_usable()?;
        let entry = self.baseline_read_entry(reference.slot)?;
        if entry.reference != reference {
            return Err(Error::InvalidReference);
        }
        let state = self.baseline_destination(entry.request.allocation)?;
        if self.read_counts[entry.request.allocation.allocation.slot] == 0 {
            return Err(Error::InvalidState);
        }
        self.baseline_validate_read_links(entry)?;
        if entry.status == ContextProducerReadStatusV1::Success {
            if entry.version != Some((state.attempt_epoch, state.content_lineage))
                || state.pending_writer.is_some()
                || state.attempt_epoch != state.content_lineage
            {
                return Err(Error::InvalidState);
            }
        } else if entry.version.is_some() {
            return Err(Error::InvalidState);
        }
        Ok(entry)
    }
}
