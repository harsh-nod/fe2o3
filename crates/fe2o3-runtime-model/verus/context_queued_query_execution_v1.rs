// Actual complete owners and shared query bodies; no admission/native authority.
include!("context_producer_query_execution_v1.rs");
use vstd::prelude::verus as context_queued_writer_declarations_v1;
include!("../src/context_queued_writers/declarations.rs");
include!("../src/context_queued_writers/query_bodies.rs");
use reads::{ContextQueuedProducerReadReferenceV1, ContextQueuedProducerReadV1};
type Error = ContextVersionJournalErrorV1;

verus! {

fn queued_writer_key_same_v1(left: ContextWriterKeyV1, right: ContextWriterKeyV1) -> (result: bool)
    ensures result == (left == right),
{ retained_writer_key_body!(left, right) }

fn queued_writer_same_v1(left: ContextWriterReferenceV1, right: ContextWriterReferenceV1) -> (result: bool)
    ensures result == (left == right),
{ queued_writer_same_body_v1!(left, right) }

spec fn queued_usable_decision_v1(owner: ContextQueuedWriterJournalV1) -> Result<(), Error> {
    if owner.disposal_terminal { Err(Error::InvalidState) } else { Ok(()) }
}

spec fn queued_root_decision_v1(owner: ContextQueuedWriterJournalV1,
    writer: ContextWriterReferenceV1) -> Result<Root, Error> {
    if owner.disposal_terminal { Err(Error::InvalidState) }
    else {
        match writer_lookup_decision_v1(owner.inner.stable.journal, writer) {
            Err(error) => Err(error),
            Ok(_) => if writer.slot >= owner.roots@.len() { Err(Error::InvalidReference) }
                else { match owner.roots@[writer.slot as int] {
                    Some(root) => if root.writer == writer { Ok(root) } else { Err(Error::InvalidReference) },
                    None => Err(Error::InvalidReference),
                } },
        }
    }
}

spec fn queued_destination_decision_v1(owner: ContextQueuedWriterJournalV1,
    write: ContextAllocationWriteV1) -> Result<ContextAllocationStateV1, Error> {
    if owner.disposal_terminal { Err(Error::InvalidState) }
    else {
        match allocation_lookup_decision_v1(owner.inner.stable.journal, write.allocation) {
            Err(error) => Err(error),
            Ok(state) => if state.device != write.device { Err(Error::AllocationDeviceMismatch) }
                else if state.byte_extent != write.byte_extent { Err(Error::AllocationExtentMismatch) }
                else { Ok(state) },
        }
    }
}

spec fn queued_active_lookup_decision_v1(owner: ContextQueuedWriterJournalV1,
    reference: ContextProducerReadReferenceV1) -> Result<ContextProducerReadV1, Error> {
    producer_lookup_decision_v1(owner.inner, reference)
}

spec fn queued_active_status_decision_v1(owner: ContextQueuedWriterJournalV1,
    reference: ContextProducerReadReferenceV1) -> Result<ContextProducerReadStatusV1, Error> {
    if owner.disposal_terminal { Err(Error::InvalidState) }
    else { producer_query_decision_v1(owner.inner, reference) }
}

impl ContextQueuedWriterJournalV1 {
    fn ensure_usable(&self) -> (result: Result<(), Error>)
        ensures result == queued_usable_decision_v1(*self),
    { queued_ensure_usable_body_v1!(self) }

    fn root(&self, writer: ContextWriterReferenceV1) -> (result: Result<Root, Error>)
        ensures result == queued_root_decision_v1(*self, writer),
    {
        proof { reveal(inspection_stable_projection_v1); reveal(inspection_producer_projection_v1); }
        queued_root_body_v1!(self, writer)
    }

    fn destination(&self, write: ContextAllocationWriteV1) -> (result: Result<ContextAllocationStateV1, Error>)
        ensures result == queued_destination_decision_v1(*self, write),
    {
        proof { reveal(inspection_stable_projection_v1); reveal(inspection_producer_projection_v1); }
        queued_destination_body_v1!(self, write)
    }

    fn lookup_producer_read(&self, reference: ContextProducerReadReferenceV1) -> (result: Result<ContextProducerReadV1, Error>)
        ensures result == queued_active_lookup_decision_v1(*self, reference),
    { queued_active_lookup_body_v1!(self, reference) }

    fn producer_read_status(&self, reference: ContextProducerReadReferenceV1) -> (result: Result<ContextProducerReadStatusV1, Error>)
        ensures result == queued_active_status_decision_v1(*self, reference),
    { queued_active_status_body_v1!(self, reference) }
}

}

mod reads {
    use super::*;
    use vstd::prelude::verus as context_queued_read_declarations_v1;
    include!("../src/context_queued_writers/read_declarations.rs");
    include!("../src/context_queued_writers/read_query_bodies.rs");

    verus! {

    fn queued_read_reference_same_v1(left: ContextQueuedProducerReadReferenceV1,
        right: ContextQueuedProducerReadReferenceV1) -> (result: bool)
        ensures result == (left == right),
    { queued_read_reference_same_body_v1!(left, right) }

    spec fn entry_decision_v1(owner: ContextQueuedWriterJournalV1, slot: usize) -> Result<Reservation, Error> {
        if slot >= owner.queued_reads@.len() { Err(Error::InvalidReference) }
        else { match owner.queued_reads@[slot as int] {
            Some(entry) => Ok(entry), None => Err(Error::InvalidReference),
        } }
    }

    spec fn next_decision_v1(owner: ContextQueuedWriterJournalV1, entry: Reservation) -> Result<(), Error> {
        match entry.next {
            None => Ok(()),
            Some(slot) => match entry_decision_v1(owner, slot) {
                Err(error) => Err(error),
                Ok(next) => if next.request.producer != entry.request.producer
                    || next.status != ContextProducerReadStatusV1::Pending
                    || next.previous != Some(entry.reference.slot) { Err(Error::InvalidState) }
                    else { Ok(()) },
            },
        }
    }

    spec fn previous_decision_v1(owner: ContextQueuedWriterJournalV1, root: Root,
        entry: Reservation) -> Result<(), Error> {
        match entry.previous {
            None => if root.read_head == Some(entry.reference.slot) { Ok(()) } else { Err(Error::InvalidState) },
            Some(slot) => match entry_decision_v1(owner, slot) {
                Err(error) => Err(error),
                Ok(previous) => if previous.request.producer != entry.request.producer
                    || previous.status != ContextProducerReadStatusV1::Pending
                    || previous.next != Some(entry.reference.slot) { Err(Error::InvalidState) }
                    else { Ok(()) },
            },
        }
    }

    spec fn links_decision_v1(owner: ContextQueuedWriterJournalV1, entry: Reservation) -> Result<(), Error> {
        if entry.status != ContextProducerReadStatusV1::Pending {
            if entry.previous.is_none() && entry.next.is_none() { Ok(()) } else { Err(Error::InvalidState) }
        } else {
            match queued_root_decision_v1(owner, entry.request.producer) {
                Err(error) => Err(error),
                Ok(root) => if root.read_count == 0 || root.read_count > owner.queued_reads@.len() {
                    Err(Error::InvalidState)
                } else { match previous_decision_v1(owner, root, entry) {
                    Err(error) => Err(error), Ok(()) => next_decision_v1(owner, entry),
                } },
            }
        }
    }

    spec fn version_decision_v1(entry: Reservation, state: ContextAllocationStateV1) -> Result<Reservation, Error> {
        if entry.status == ContextProducerReadStatusV1::Success {
            if entry.version != Some((state.attempt_epoch, state.content_lineage))
                || state.pending_writer.is_some() || state.attempt_epoch != state.content_lineage {
                Err(Error::InvalidState)
            } else { Ok(entry) }
        } else if entry.version.is_some() { Err(Error::InvalidState) }
        else { Ok(entry) }
    }

    spec fn retained_decision_v1(owner: ContextQueuedWriterJournalV1, entry: Reservation,
        state: ContextAllocationStateV1) -> Result<Reservation, Error> {
        let slot = entry.request.allocation.allocation.slot;
        if slot >= owner.read_counts@.len() { Err(Error::InvalidReference) }
        else if owner.read_counts@[slot as int] == 0 { Err(Error::InvalidState) }
        else { match links_decision_v1(owner, entry) {
            Err(error) => Err(error), Ok(()) => version_decision_v1(entry, state),
        } }
    }

    spec fn inspect_decision_v1(owner: ContextQueuedWriterJournalV1,
        reference: ContextQueuedProducerReadReferenceV1) -> Result<Reservation, Error> {
        if owner.disposal_terminal { Err(Error::InvalidState) }
        else { match entry_decision_v1(owner, reference.slot) {
            Err(error) => Err(error),
            Ok(entry) => if entry.reference != reference { Err(Error::InvalidReference) }
                else { match queued_destination_decision_v1(owner, entry.request.allocation) {
                    Err(error) => Err(error), Ok(state) => retained_decision_v1(owner, entry, state),
                } },
        } }
    }

    pub(super) closed spec fn lookup_decision_v1(owner: ContextQueuedWriterJournalV1,
        reference: ContextQueuedProducerReadReferenceV1) -> Result<ContextQueuedProducerReadV1, Error> {
        match inspect_decision_v1(owner, reference) { Err(error) => Err(error), Ok(entry) => Ok(entry.request) }
    }

    pub(super) closed spec fn status_decision_v1(owner: ContextQueuedWriterJournalV1,
        reference: ContextQueuedProducerReadReferenceV1) -> Result<ContextProducerReadStatusV1, Error> {
        match inspect_decision_v1(owner, reference) { Err(error) => Err(error), Ok(entry) => Ok(entry.status) }
    }

    impl ContextQueuedWriterJournalV1 {
        fn read_entry(&self, slot: usize) -> (result: Result<Reservation, Error>)
            ensures result == entry_decision_v1(*self, slot),
        { queued_read_entry_body_v1!(self, slot) }

        fn validate_read_links(&self, entry: Reservation) -> (result: Result<(), Error>)
            ensures result == links_decision_v1(*self, entry),
        { queued_read_links_body_v1!(self, entry) }

        fn inspect_queued_read(&self, reference: ContextQueuedProducerReadReferenceV1) -> (result: Result<Reservation, Error>)
            ensures result == inspect_decision_v1(*self, reference),
        { queued_read_inspect_body_v1!(self, reference) }

        pub(super) fn lookup_queued_producer_read(&self, reference: ContextQueuedProducerReadReferenceV1) -> (result: Result<ContextQueuedProducerReadV1, Error>)
            ensures result == lookup_decision_v1(*self, reference),
        { queued_read_lookup_body_v1!(self, reference) }

        pub(super) fn queued_producer_read_status(&self, reference: ContextQueuedProducerReadReferenceV1) -> (result: Result<ContextProducerReadStatusV1, Error>)
            ensures result == status_decision_v1(*self, reference),
        { queued_read_status_body_v1!(self, reference) }
    }

    }
}

verus! {

type QueuedQueryResultsV1 = (
    Result<ContextProducerReadV1, Error>, Result<ContextProducerReadStatusV1, Error>,
    Result<ContextQueuedProducerReadV1, Error>, Result<ContextProducerReadStatusV1, Error>,
);

fn queued_queries_preserve_full_owner_v1(owner: &mut ContextQueuedWriterJournalV1,
    active: ContextProducerReadReferenceV1, queued: ContextQueuedProducerReadReferenceV1)
    -> (result: QueuedQueryResultsV1)
    ensures
        *final(owner) == *old(owner),
        result.0 == queued_active_lookup_decision_v1(*old(owner), active),
        result.1 == queued_active_status_decision_v1(*old(owner), active),
        result.2 == reads::lookup_decision_v1(*old(owner), queued),
        result.3 == reads::status_decision_v1(*old(owner), queued),
{
    (owner.lookup_producer_read(active), owner.producer_read_status(active),
        owner.lookup_queued_producer_read(queued), owner.queued_producer_read_status(queued))
}

}
