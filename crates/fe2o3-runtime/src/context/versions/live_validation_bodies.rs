macro_rules! context_allocation_enrollment_body_v1 {
    ($id:ident, $device:ident, $byte_extent:ident) => {
        ContextAllocationEnrollmentV1 {
            key: ContextAllocationKeyV1 {
                context_generation: $id.context_generation,
                local: $id.local,
            },
            device: ContextJournalDeviceKeyV1 {
                context_generation: $device.context_generation,
                local: $device.local,
            },
            byte_extent: $byte_extent,
        }
    };
}

macro_rules! context_validate_phase_body_v1 {
    ($this:ident, $reference:ident, $phase:ident) => {{
        $this.journal.lookup_allocation($reference)?;
        if $this.phases.get($reference.slot) != Some(&Some($phase)) {
            return Err(ContextVersionJournalErrorV1::InvalidState);
        }
        Ok(())
    }};
}

macro_rules! context_validate_live_body_v1 {
    ($this:ident, $id:ident, $record:ident) => {{
        let reference = $record
            .journal
            .ok_or(ContextVersionJournalErrorV1::InvalidState)?;
        $this.validate_phase(reference, AllocationPhaseV1::Live)?;
        let expected = enrollment($id, $record.device, $record.byte_len);
        let actual = $this.journal.lookup_allocation(reference)?;
        if reference.key != expected.key
            || actual.device != expected.device
            || actual.byte_extent != expected.byte_extent
        {
            return Err(ContextVersionJournalErrorV1::InvalidAllocationReference);
        }
        Ok(reference)
    }};
}
