// Full ordered owner contents; no construction of an allocated historical Vec.
verus! {

spec fn allocation_read_view(value: ContextAllocationReadV1) -> logical::AllocationReadV1 {
    logical::AllocationReadV1 { allocation: allocation_reference_view(value.allocation),
        device: device_key_view(value.device), byte_extent: value.byte_extent, byte_offset: 0,
        byte_len: value.byte_len, attempt_epoch: value.attempt_epoch, content_lineage: value.content_lineage }
}

spec fn read_reference_view(value: ContextReadLeaseReferenceV1) -> logical::ReadReferenceV1 {
    logical::ReadReferenceV1 { slot: value.slot, incarnation: value.incarnation, consumer: writer_key_view(value.consumer) }
}

spec fn read_lease_slot_view(value: Option<ReadLeaseV1>) -> Option<logical::ReadLeaseV1> {
    match value {
        None => None,
        Some(entry) => Some(logical::ReadLeaseV1 { reference: read_reference_view(entry.reference), request: allocation_read_view(entry.request) }),
    }
}

spec fn producer_read_view(value: ContextProducerReadV1) -> logical::ProducerReadV1 {
    logical::ProducerReadV1 { read: allocation_read_view(value.read), producer: writer_reference_view(value.producer) }
}

spec fn producer_reference_view(value: ContextProducerReadReferenceV1) -> logical::ProducerReadReferenceV1 {
    logical::ProducerReadReferenceV1 { slot: value.slot, incarnation: value.incarnation, consumer: writer_key_view(value.consumer) }
}

spec fn producer_reservation_slot_view(value: Option<ReservationV1>) -> Option<logical::ProducerReservationV1> {
    match value {
        None => None,
        Some(entry) => Some(logical::ProducerReservationV1 { reference: producer_reference_view(entry.reference), request: producer_read_view(entry.request) }),
    }
}

spec fn allocation_read_from(value: logical::AllocationReadV1) -> ContextAllocationReadV1 {
    ContextAllocationReadV1 { allocation: allocation_reference_from(value.allocation),
        device: device_key_from(value.device), byte_extent: value.byte_extent, byte_offset: value.byte_offset,
        byte_len: value.byte_len, attempt_epoch: value.attempt_epoch, content_lineage: value.content_lineage }
}

spec fn read_reference_from(value: logical::ReadReferenceV1) -> ContextReadLeaseReferenceV1 {
    ContextReadLeaseReferenceV1 { slot: value.slot, incarnation: value.incarnation, consumer: writer_key_from(value.consumer) }
}

spec fn read_lease_slot_from(value: Option<logical::ReadLeaseV1>) -> Option<ReadLeaseV1> {
    match value {
        None => None,
        Some(entry) => Some(ReadLeaseV1 { reference: read_reference_from(entry.reference), request: allocation_read_from(entry.request) }),
    }
}

spec fn producer_read_from(value: logical::ProducerReadV1) -> ContextProducerReadV1 {
    ContextProducerReadV1 { read: allocation_read_from(value.read), producer: writer_reference_from(value.producer) }
}

spec fn producer_reference_from(value: logical::ProducerReadReferenceV1) -> ContextProducerReadReferenceV1 {
    ContextProducerReadReferenceV1 { slot: value.slot, incarnation: value.incarnation, consumer: writer_key_from(value.consumer) }
}

spec fn producer_reservation_slot_from(value: Option<logical::ProducerReservationV1>) -> Option<ReservationV1> {
    match value {
        None => None,
        Some(entry) => Some(ReservationV1 { reference: producer_reference_from(entry.reference), request: producer_read_from(entry.request) }),
    }
}

proof fn allocation_read_round_trip(value: ContextAllocationReadV1, model: logical::AllocationReadV1)
    ensures allocation_read_from(allocation_read_view(value)) == value, allocation_read_view(allocation_read_from(model)) == model,
{
    allocation_reference_round_trip(value.allocation, model.allocation);
    device_key_round_trip(value.device, model.device);
}

proof fn read_reference_round_trip(value: ContextReadLeaseReferenceV1, model: logical::ReadReferenceV1)
    ensures read_reference_from(read_reference_view(value)) == value, read_reference_view(read_reference_from(model)) == model,
{
    writer_key_round_trip(value.consumer, model.consumer);
}

proof fn read_lease_slot_round_trip(value: Option<ReadLeaseV1>, model: Option<logical::ReadLeaseV1>)
    ensures read_lease_slot_from(read_lease_slot_view(value)) == value, read_lease_slot_view(read_lease_slot_from(model)) == model,
{
    if let Some(entry) = value {
        read_reference_round_trip(entry.reference, read_reference_view(entry.reference));
        allocation_read_round_trip(entry.request, allocation_read_view(entry.request));
    }
    if let Some(entry) = model {
        read_reference_round_trip(read_reference_from(entry.reference), entry.reference);
        allocation_read_round_trip(allocation_read_from(entry.request), entry.request);
    }
}

proof fn producer_read_round_trip(value: ContextProducerReadV1, model: logical::ProducerReadV1)
    ensures producer_read_from(producer_read_view(value)) == value, producer_read_view(producer_read_from(model)) == model,
{
    allocation_read_round_trip(value.read, model.read);
    writer_reference_round_trip(value.producer, model.producer);
}

proof fn producer_reference_round_trip(value: ContextProducerReadReferenceV1, model: logical::ProducerReadReferenceV1)
    ensures producer_reference_from(producer_reference_view(value)) == value, producer_reference_view(producer_reference_from(model)) == model,
{
    writer_key_round_trip(value.consumer, model.consumer);
}

proof fn producer_reservation_slot_round_trip(value: Option<ReservationV1>, model: Option<logical::ProducerReservationV1>)
    ensures producer_reservation_slot_from(producer_reservation_slot_view(value)) == value,
        producer_reservation_slot_view(producer_reservation_slot_from(model)) == model,
{
    if let Some(entry) = value {
        producer_reference_round_trip(entry.reference, producer_reference_view(entry.reference));
        producer_read_round_trip(entry.request, producer_read_view(entry.request));
    }
    if let Some(entry) = model {
        producer_reference_round_trip(producer_reference_from(entry.reference), entry.reference);
        producer_read_round_trip(producer_read_from(entry.request), entry.request);
    }
}

spec fn stable_represents(actual: ContextReadLeasedJournalV1, model: logical::ReadContentsV1) -> bool {
    &&& represents(actual.journal, model.journal)
    &&& actual.leases@.map(|_i, entry| read_lease_slot_view(entry)) == model.leases@
    &&& actual.free_reads@ == model.free_reads@
    &&& actual.readers@ == model.readers@
    &&& actual.next_incarnation == model.next_incarnation
}

spec fn producer_represents(actual: ContextProducerReadJournalV1, model: logical::ProducerReadContentsV1) -> bool {
    &&& stable_represents(actual.stable, model.stable)
    &&& actual.reservations@.map(|_i, entry| producer_reservation_slot_view(entry)) == model.reservations@
    &&& actual.free@ == model.free@
    &&& actual.counts@ == model.counts@
    &&& actual.next_incarnation == model.next_incarnation
}

spec fn lookup_result_from(model: logical::JournalContentsV1, result: Result<logical::AllocationEntryV1, logical::ReadErrorV1>)
    -> Result<ContextAllocationStateV1, ReadErrorV1>
{
    match result {
        Err(error) => Err(read_error_embed(error)),
        Ok(entry) => Ok(ContextAllocationStateV1 { device: device_key_from(entry.device),
            byte_extent: entry.byte_extent, attempt_epoch: entry.attempt_epoch, content_lineage: entry.content_lineage,
            pending_writer: match entry.pending_member {
                None => None,
                Some(slot) => Some(writer_reference_from(model.members@[slot as int].unwrap().writer)),
            } }),
    }
}

}
