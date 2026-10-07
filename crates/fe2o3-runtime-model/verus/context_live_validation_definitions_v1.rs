use vstd::prelude::verus as context_live_validation_declarations_v1;
include!("../../fe2o3-runtime/src/context/versions/live_validation_declarations.rs");

structural_eq!(AllocationPhaseV1);

verus! {
struct Versions<V> {
    journal: ContextQueuedWriterJournalV1,
    phases: Vec<Option<AllocationPhaseV1>>,
    unread: V,
}

#[verifier::opaque]
spec fn live_phase_decision(journal: ContextQueuedWriterJournalV1,
    phases: Seq<Option<AllocationPhaseV1>>, reference: ContextAllocationReferenceV1,
    phase: AllocationPhaseV1) -> Result<(), ContextVersionJournalErrorV1> {
    match allocation_lookup_decision_v1(journal.inner.stable.journal, reference) {
        Err(error) => Err(error),
        Ok(_) => if reference.slot >= phases.len()
            || phases[reference.slot as int] != Some(phase) {
            Err(ContextVersionJournalErrorV1::InvalidState)
        } else { Ok(()) },
    }
}

#[verifier::opaque]
spec fn live_allocation_decision(journal: ContextQueuedWriterJournalV1,
    phases: Seq<Option<AllocationPhaseV1>>, id: RuntimeAllocationIdV1,
    record: AllocationRecordV1) -> Result<ContextAllocationReferenceV1, ContextVersionJournalErrorV1> {
    match record.journal {
        None => Err(ContextVersionJournalErrorV1::InvalidState),
        Some(reference) => match live_phase_decision(journal, phases, reference, AllocationPhaseV1::Live) {
            Err(error) => Err(error),
            Ok(()) => match allocation_lookup_decision_v1(journal.inner.stable.journal, reference) {
                Err(error) => Err(error),
                Ok(actual) => if reference.key != (ContextAllocationKeyV1 {
                    context_generation: id.context_generation, local: id.local,
                }) || actual.device != (ContextJournalDeviceKeyV1 {
                    context_generation: record.device.context_generation, local: record.device.local,
                }) || actual.byte_extent != record.byte_len {
                    Err(ContextVersionJournalErrorV1::InvalidAllocationReference)
                } else { Ok(reference) },
            },
        },
    }
}

impl ContextQueuedWriterJournalV1 {
    fn lookup_allocation(&self, allocation: ContextAllocationReferenceV1)
        -> (out: Result<ContextAllocationStateV1, ContextVersionJournalErrorV1>)
        ensures out == allocation_lookup_decision_v1(self.inner.stable.journal, allocation),
    {
        proof { reveal(inspection_stable_projection_v1); reveal(inspection_producer_projection_v1); }
        queued_allocation_lookup_body_v1!(self, allocation)
    }
}

impl<V> Versions<V> {
    fn validate_phase(&self, reference: ContextAllocationReferenceV1, phase: AllocationPhaseV1)
        -> (out: Result<(), ContextVersionJournalErrorV1>)
        ensures out == live_phase_decision(self.journal, self.phases@, reference, phase),
    {
        proof { reveal(live_phase_decision); }
        context_validate_phase_body_v1!(self, reference, phase)
    }

    fn validate_live(&self, id: RuntimeAllocationIdV1, record: &AllocationRecordV1)
        -> (out: Result<ContextAllocationReferenceV1, ContextVersionJournalErrorV1>)
        ensures out == live_allocation_decision(self.journal, self.phases@, id, *record),
    {
        proof { reveal(live_allocation_decision); }
        context_validate_live_body_v1!(self, id, record)
    }
}
}
