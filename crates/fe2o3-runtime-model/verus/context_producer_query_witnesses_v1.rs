verus! {

// Synthetic raw lifecycle states, not acquisition/settlement reachability.
// Deliberately malformed free/count storage is irrelevant to these read-only queries.
#[verifier::spinoff_prover]
fn producer_query_raw_witness_v1(phase: u8, invalid_old_slot: bool) -> (result: bool)
    requires phase < 4,
    ensures result,
{
    let producer = WriterReferenceV1 { slot: if phase < 2 || !invalid_old_slot { 0 } else { usize::MAX },
        key: WriterKeyV1 { context_generation: 7, local: 10, kind: WriterKindV1::Submission } };
    let model_producer = logical::WriterReferenceV1 { slot: if phase < 2 || !invalid_old_slot { 0 } else { usize::MAX },
        key: logical::WriterKeyV1 { context_generation: 7, local: 10, kind: logical::WriterKindV1::Submission } };
    let allocation = AllocationReferenceV1 { slot: 0, key: AllocationKeyV1 { context_generation: 7, local: 1 } };
    let model_allocation = logical::AllocationReferenceV1 { slot: 0,
        key: logical::AllocationKeyV1 { context_generation: 7, local: 1 } };
    let device = ContextJournalDeviceKeyV1 { context_generation: 7, local: 2 };
    let model_device = logical::DeviceKeyV1 { context_generation: 7, local: 2 };
    let request = ContextProducerReadV1 { producer, read: ContextAllocationReadV1 {
        allocation, device, byte_extent: 16, byte_offset: 3, byte_len: 13, attempt_epoch: 9, content_lineage: 5 } };
    let model_request = logical::ProducerReadV1 { producer: model_producer, read: logical::AllocationReadV1 {
        allocation: model_allocation, device: model_device, byte_extent: 16, byte_offset: 3, byte_len: 13,
        attempt_epoch: 9, content_lineage: 5 } };
    let reference = ContextProducerReadReferenceV1 { slot: 0, incarnation: 1,
        consumer: WriterKeyV1 { context_generation: 7, local: 20, kind: WriterKindV1::Submission } };
    let model_reference = logical::ProducerReadReferenceV1 { slot: 0, incarnation: 1,
        consumer: logical::WriterKeyV1 { context_generation: 7, local: 20, kind: logical::WriterKindV1::Submission } };
    let replacement = WriterKeyV1 { local: 99, ..producer.key };
    let model_replacement = logical::WriterKeyV1 { local: 99, ..model_producer.key };
    let writer = if phase == 0 { WriterEntryV1::Pending { key: producer.key, head: Some(usize::MAX), count: usize::MAX } }
        else if phase == 1 { WriterEntryV1::Unknown { key: producer.key, head: Some(usize::MAX), count: usize::MAX } }
        else { WriterEntryV1::Reserved(replacement) };
    let model_writer = if phase == 0 { logical::WriterEntryV1::Pending { key: model_producer.key, head: Some(usize::MAX), count: usize::MAX } }
        else if phase == 1 { logical::WriterEntryV1::Unknown { key: model_producer.key, head: Some(usize::MAX), count: usize::MAX } }
        else { logical::WriterEntryV1::Reserved(model_replacement) };
    let journal = JournalContentsV1 { context_generation: 7, allocation_capacity: 1, writer_capacity: 1,
        registration_watermark: 20, reserved_count: 0, writers: vec![Some(writer)], free: vec![0usize],
        allocations: vec![Some(AllocationEntryV1 { key: allocation.key, device, byte_extent: 16, attempt_epoch: 9,
            content_lineage: if phase == 2 { 9 } else { 5 }, pending_member: if phase < 2 { Some(0) } else { None } })],
        allocation_free: vec![], members: vec![Some(MemberEntryV1 { writer: producer, allocation,
            prior_lineage: 5, attempt_epoch: 9, next: None })], member_free: vec![0usize], scratch: vec![None] };
    let model_journal = logical::JournalContentsV1 { context_generation: 7, allocation_capacity: 1, writer_capacity: 1,
        registration_watermark: 20, reserved_count: 0, writers: vec![Some(model_writer)], free: vec![0usize],
        allocations: vec![Some(logical::AllocationEntryV1 { key: model_allocation.key, device: model_device,
            byte_extent: 16, attempt_epoch: 9, content_lineage: if phase == 2 { 9 } else { 5 },
            pending_member: if phase < 2 { Some(0) } else { None } })], allocation_free: vec![],
        members: vec![Some(logical::MemberEntryV1 { writer: model_producer, allocation: model_allocation,
            prior_lineage: 5, attempt_epoch: 9, next: None })], member_free: vec![0usize], scratch: vec![None] };
    let mut actual = ContextProducerReadJournalV1 {
        stable: ContextReadLeasedJournalV1 { journal, leases: vec![None], free_reads: vec![0usize],
            readers: vec![0usize], next_incarnation: 1 },
        reservations: vec![Some(ReservationV1 { reference, request })], free: vec![0usize, 0],
        counts: vec![usize::MAX], next_incarnation: 0 };
    let mut model = logical::ProducerReadContentsV1 {
        stable: logical::ReadContentsV1 { journal: model_journal, leases: vec![None], free_reads: vec![0usize],
            readers: vec![0usize], next_incarnation: 1 },
        reservations: vec![Some(logical::ProducerReservationV1 { reference: model_reference, request: model_request })],
        free: vec![0usize, 0], counts: vec![usize::MAX], next_incarnation: 0 };
    proof {
        assert(journal_view(actual.stable.journal).writers =~= model.stable.journal.writers@);
        assert(journal_view(actual.stable.journal).allocations =~= model.stable.journal.allocations@);
        assert(journal_view(actual.stable.journal).members =~= model.stable.journal.members@);
        assert(journal_view(actual.stable.journal).scratch =~= model.stable.journal.scratch@);
        assert(actual.stable.journal.free@ =~= model.stable.journal.free@);
        assert(actual.stable.journal.allocation_free@ =~= model.stable.journal.allocation_free@);
        assert(actual.stable.journal.member_free@ =~= model.stable.journal.member_free@);
        assert(actual.stable.leases@.map(|_i, entry| read_lease_slot_view(entry)) =~= model.stable.leases@);
        assert(actual.stable.free_reads@ =~= model.stable.free_reads@);
        assert(actual.stable.readers@ =~= model.stable.readers@);
        assert(actual.reservations@.map(|_i, entry| producer_reservation_slot_view(entry)) =~= model.reservations@);
        assert(actual.free@ =~= model.free@);
        assert(actual.counts@ =~= model.counts@);
    }
    let expected = if phase == 0 { ContextProducerReadStatusV1::Pending }
        else if phase == 1 { ContextProducerReadStatusV1::Unknown }
        else if phase == 2 { ContextProducerReadStatusV1::Success }
        else { ContextProducerReadStatusV1::NoEffect };
    let ghost before = actual;
    let status = producer_status_historical_exec_v1(&actual, &model, &request, model_request);
    assert(status.0 == Ok(expected));
    let validation = producer_validate_historical_exec_v1(&actual, &model, &request, model_request);
    assert(validation.0 == if phase == 0 { Ok(()) } else { Err(ReadErrorV1::AllocationBusy) });
    let lookup = producer_lookup_historical_exec_v1(&actual, &model, reference, model_reference);
    assert(lookup.0 == Ok(request));
    let query = producer_query_historical_exec_v1(&actual, &model, reference, model_reference);
    assert(query.0 == Ok(expected));
    assert(actual == before);

    let bad_request = ContextProducerReadV1 { read: ContextAllocationReadV1 {
        device: ContextJournalDeviceKeyV1 { local: 3, ..device }, ..request.read }, ..request };
    let model_bad_request = logical::ProducerReadV1 { read: logical::AllocationReadV1 {
        device: logical::DeviceKeyV1 { local: 3, ..model_device }, ..model_request.read }, ..model_request };
    actual.reservations.set(0, Some(ReservationV1 { reference, request: bad_request }));
    model.reservations.set(0, Some(logical::ProducerReservationV1 { reference: model_reference, request: model_bad_request }));
    proof { assert(actual.reservations@.map(|_i, entry| producer_reservation_slot_view(entry)) =~= model.reservations@); }
    let stale = ContextProducerReadReferenceV1 { incarnation: 2, ..reference };
    let model_stale = logical::ProducerReadReferenceV1 { incarnation: 2, ..model_reference };
    let query = producer_query_historical_exec_v1(&actual, &model, stale, model_stale);
    assert(query.0 == Err(ReadErrorV1::InvalidReference));
    let query = producer_query_historical_exec_v1(&actual, &model, reference, model_reference);
    assert(query.0 == Err(ReadErrorV1::AllocationDeviceMismatch));
    true
}

}
