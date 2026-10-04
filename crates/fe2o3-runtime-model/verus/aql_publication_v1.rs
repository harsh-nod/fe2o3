use vstd::prelude::*;

verus! {

pub open spec fn packet_bytes_v1() -> nat {
    64
}

pub open spec fn max_ring_packets_v1() -> nat {
    33_554_432
}

pub open spec fn max_u64_v1() -> nat {
    0xffffffffffffffff
}

pub open spec fn invalid_packet_type_v1() -> u8 {
    1u8
}

pub open spec fn final_header_low_v1() -> u8 {
    0x02u8
}

pub open spec fn final_header_high_v1() -> u8 {
    0x14u8
}

pub open spec fn index_in_range_v1(index: int, start: nat, size: nat) -> bool {
    start as int <= index && index < (start + size) as int
}

pub open spec fn canonical_unpublished_packet_v1(
    packet: Seq<u8>,
    dimensions: nat,
) -> bool {
    &&& packet.len() == packet_bytes_v1()
    &&& 1 <= dimensions <= 3
    &&& packet[0] == invalid_packet_type_v1()
    &&& packet[1] == 0u8
    &&& packet[2] == dimensions as u8
    &&& packet[3] == 0u8
}

pub open spec fn canonical_ring_frame_v1(
    ring: Seq<u8>,
    capacity: nat,
    slot: nat,
) -> bool {
    &&& 0 < capacity <= max_ring_packets_v1()
    &&& slot < capacity
    &&& ring.len() == capacity * packet_bytes_v1()
}

pub open spec fn slot_start_v1(slot: nat) -> nat {
    slot * packet_bytes_v1()
}

pub open spec fn little_endian_u32_v1(bytes: Seq<u8>, start: nat) -> nat {
    (bytes[start as int] as nat)
        + (bytes[start as int + 1] as nat) * 0x100
        + (bytes[start as int + 2] as nat) * 0x1_0000
        + (bytes[start as int + 3] as nat) * 0x100_0000
}

pub open spec fn copy_unpublished_body_v1(
    source: Seq<u8>,
    before: Seq<u8>,
    slot: nat,
) -> Seq<u8> {
    let start = slot_start_v1(slot);
    Seq::new(before.len(), |index: int|
        if index_in_range_v1(index, start, packet_bytes_v1()) {
            source[index - start as int]
        } else {
            before[index]
        }
    )
}

pub open spec fn publish_invariant_header_release_u32_v1(
    copied: Seq<u8>,
    slot: nat,
) -> Seq<u8> {
    let start = slot_start_v1(slot);
    Seq::new(copied.len(), |index: int|
        if index == start as int {
            final_header_low_v1()
        } else if index == start as int + 1 {
            final_header_high_v1()
        } else if index == start as int + 2 {
            copied[start as int + 2]
        } else if index == start as int + 3 {
            copied[start as int + 3]
        } else {
            copied[index]
        }
    )
}

pub open spec fn published_ring_v1(
    source: Seq<u8>,
    before: Seq<u8>,
    slot: nat,
) -> Seq<u8> {
    publish_invariant_header_release_u32_v1(
        copy_unpublished_body_v1(source, before, slot),
        slot,
    )
}

pub proof fn invalid_body_copy_is_exact_and_framed_v1(
    source: Seq<u8>,
    before: Seq<u8>,
    capacity: nat,
    slot: nat,
    dimensions: nat,
)
    requires
        canonical_unpublished_packet_v1(source, dimensions),
        canonical_ring_frame_v1(before, capacity, slot),
    ensures
        copy_unpublished_body_v1(source, before, slot).len() == before.len(),
        forall|index: int| 0 <= index < before.len() ==>
            if index_in_range_v1(index, slot_start_v1(slot), packet_bytes_v1()) {
                #[trigger] copy_unpublished_body_v1(source, before, slot)[index]
                    == source[index - slot_start_v1(slot) as int]
            } else {
                #[trigger] copy_unpublished_body_v1(source, before, slot)[index]
                    == before[index]
            },
        copy_unpublished_body_v1(source, before, slot)[slot_start_v1(slot) as int]
            == invalid_packet_type_v1(),
        copy_unpublished_body_v1(source, before, slot)[slot_start_v1(slot) as int + 2]
            == dimensions as u8,
{
}

pub proof fn release_u32_is_exact_framed_and_preserves_setup_v1(
    copied: Seq<u8>,
    capacity: nat,
    slot: nat,
)
    requires
        canonical_ring_frame_v1(copied, capacity, slot),
    ensures
        publish_invariant_header_release_u32_v1(copied, slot).len() == copied.len(),
        publish_invariant_header_release_u32_v1(copied, slot)[slot_start_v1(slot) as int]
            == final_header_low_v1(),
        publish_invariant_header_release_u32_v1(copied, slot)[slot_start_v1(slot) as int + 1]
            == final_header_high_v1(),
        publish_invariant_header_release_u32_v1(copied, slot)[slot_start_v1(slot) as int + 2]
            == copied[slot_start_v1(slot) as int + 2],
        publish_invariant_header_release_u32_v1(copied, slot)[slot_start_v1(slot) as int + 3]
            == copied[slot_start_v1(slot) as int + 3],
        little_endian_u32_v1(
            publish_invariant_header_release_u32_v1(copied, slot),
            slot_start_v1(slot),
        ) == 0x1402
            + (copied[slot_start_v1(slot) as int + 2] as nat) * 0x1_0000
            + (copied[slot_start_v1(slot) as int + 3] as nat) * 0x100_0000,
        forall|index: int| 0 <= index < copied.len() ==>
            !index_in_range_v1(index, slot_start_v1(slot), 4) ==>
                #[trigger] publish_invariant_header_release_u32_v1(copied, slot)[index]
                    == copied[index],
{
}

pub proof fn canonical_invalid_then_release_transition_v1(
    source: Seq<u8>,
    before: Seq<u8>,
    capacity: nat,
    slot: nat,
    dimensions: nat,
)
    requires
        canonical_unpublished_packet_v1(source, dimensions),
        canonical_ring_frame_v1(before, capacity, slot),
    ensures
        copy_unpublished_body_v1(source, before, slot)[slot_start_v1(slot) as int]
            == invalid_packet_type_v1(),
        published_ring_v1(source, before, slot)[slot_start_v1(slot) as int]
            == final_header_low_v1(),
        published_ring_v1(source, before, slot)[slot_start_v1(slot) as int + 1]
            == final_header_high_v1(),
        published_ring_v1(source, before, slot)[slot_start_v1(slot) as int + 2]
            == dimensions as u8,
        published_ring_v1(source, before, slot)[slot_start_v1(slot) as int + 3]
            == 0u8,
        little_endian_u32_v1(
            published_ring_v1(source, before, slot),
            slot_start_v1(slot),
        ) == 0x1402 + dimensions * 0x1_0000,
        forall|index: int| 0 <= index < before.len() ==>
            if index_in_range_v1(index, slot_start_v1(slot), packet_bytes_v1()) {
                if index == slot_start_v1(slot) as int {
                    #[trigger] published_ring_v1(source, before, slot)[index]
                        == final_header_low_v1()
                } else if index == slot_start_v1(slot) as int + 1 {
                    #[trigger] published_ring_v1(source, before, slot)[index]
                        == final_header_high_v1()
                } else {
                    #[trigger] published_ring_v1(source, before, slot)[index]
                        == source[index - slot_start_v1(slot) as int]
                }
            } else {
                #[trigger] published_ring_v1(source, before, slot)[index]
                    == before[index]
            },
{
    invalid_body_copy_is_exact_and_framed_v1(
        source,
        before,
        capacity,
        slot,
        dimensions,
    );
    release_u32_is_exact_framed_and_preserves_setup_v1(
        copy_unpublished_body_v1(source, before, slot),
        capacity,
        slot,
    );
}

pub open spec fn completion_signal_bytes_v1() -> nat {
    64
}

pub open spec fn pending_completion_signal_image_v1() -> Seq<u8> {
    Seq::new(completion_signal_bytes_v1(), |index: int|
        if index == 0 || index == 8 {
            1u8
        } else {
            0u8
        }
    )
}

#[derive(PartialEq, Eq)]
pub enum AqlCompletionObservationV1 {
    Pending,
    Completed,
    Unexpected { value: i64 },
}

pub open spec fn classify_acquired_completion_value_v1(
    value: i64,
) -> AqlCompletionObservationV1 {
    if value == 1 {
        AqlCompletionObservationV1::Pending
    } else if value == 0 {
        AqlCompletionObservationV1::Completed
    } else {
        AqlCompletionObservationV1::Unexpected { value }
    }
}

pub proof fn pending_completion_signal_image_is_exact_v1()
    ensures
        pending_completion_signal_image_v1().len() == completion_signal_bytes_v1(),
        forall|index: int| 0 <= index < completion_signal_bytes_v1() ==>
            #[trigger] pending_completion_signal_image_v1()[index]
                == if index == 0 || index == 8 { 1u8 } else { 0u8 },
        pending_completion_signal_image_v1()[0] == 1u8,
        pending_completion_signal_image_v1()[8] == 1u8,
        forall|index: int| 16 <= index < completion_signal_bytes_v1() ==>
            #[trigger] pending_completion_signal_image_v1()[index] == 0u8,
{
}

pub proof fn completion_observation_classifier_is_exact_v1(value: i64)
    ensures
        value == 1 ==>
            classify_acquired_completion_value_v1(value)
                == AqlCompletionObservationV1::Pending,
        value == 0 ==>
            classify_acquired_completion_value_v1(value)
                == AqlCompletionObservationV1::Completed,
        value != 1 && value != 0 ==>
            classify_acquired_completion_value_v1(value)
                == (AqlCompletionObservationV1::Unexpected { value }),
{
}

#[derive(PartialEq, Eq)]
pub struct AqlRingStateV1 {
    pub capacity: nat,
    pub write: nat,
    pub last_read: nat,
}

#[derive(PartialEq, Eq)]
pub struct AqlReservationV1 {
    pub packet_id: nat,
    pub slot: nat,
    pub observed_read: nat,
    pub next_write: nat,
}

#[derive(PartialEq, Eq)]
pub enum AqlReservationRejectionV1 {
    ReadRegressed,
    ReadAfterWrite,
    DistanceExceedsCapacity,
    Full,
    WriteExhausted,
}

#[derive(PartialEq, Eq)]
pub enum AqlReservationOutcomeV1 {
    Accepted {
        after: AqlRingStateV1,
        reservation: AqlReservationV1,
    },
    Rejected {
        state: AqlRingStateV1,
        reason: AqlReservationRejectionV1,
    },
}

pub open spec fn canonical_ring_state_v1(state: AqlRingStateV1) -> bool {
    &&& 0 < state.capacity <= max_ring_packets_v1()
    &&& state.last_read <= state.write <= max_u64_v1()
    &&& state.write - state.last_read <= state.capacity
}

pub open spec fn reserve_one_v1(
    state: AqlRingStateV1,
    observed_read: nat,
) -> AqlReservationOutcomeV1 {
    if observed_read < state.last_read {
        AqlReservationOutcomeV1::Rejected {
            state,
            reason: AqlReservationRejectionV1::ReadRegressed,
        }
    } else if observed_read > state.write {
        AqlReservationOutcomeV1::Rejected {
            state,
            reason: AqlReservationRejectionV1::ReadAfterWrite,
        }
    } else if state.write - observed_read > state.capacity {
        AqlReservationOutcomeV1::Rejected {
            state,
            reason: AqlReservationRejectionV1::DistanceExceedsCapacity,
        }
    } else if state.write - observed_read == state.capacity {
        AqlReservationOutcomeV1::Rejected {
            state,
            reason: AqlReservationRejectionV1::Full,
        }
    } else if state.write == max_u64_v1() {
        AqlReservationOutcomeV1::Rejected {
            state,
            reason: AqlReservationRejectionV1::WriteExhausted,
        }
    } else {
        AqlReservationOutcomeV1::Accepted {
            after: AqlRingStateV1 {
                capacity: state.capacity,
                write: state.write + 1,
                last_read: observed_read,
            },
            reservation: AqlReservationV1 {
                packet_id: state.write,
                slot: state.write % state.capacity,
                observed_read,
                next_write: state.write + 1,
            },
        }
    }
}

pub open spec fn accepted_reservation_v1(
    before: AqlRingStateV1,
    observed_read: nat,
    after: AqlRingStateV1,
    reservation: AqlReservationV1,
) -> bool {
    reserve_one_v1(before, observed_read)
        == (AqlReservationOutcomeV1::Accepted { after, reservation })
}

pub open spec fn outcome_is_rejected_v1(outcome: AqlReservationOutcomeV1) -> bool {
    match outcome {
        AqlReservationOutcomeV1::Rejected { .. } => true,
        _ => false,
    }
}

pub open spec fn outcome_state_v1(
    outcome: AqlReservationOutcomeV1,
) -> AqlRingStateV1 {
    match outcome {
        AqlReservationOutcomeV1::Accepted { after, .. } => after,
        AqlReservationOutcomeV1::Rejected { state, .. } => state,
    }
}

pub proof fn accepted_reservation_advances_exactly_once_v1(
    before: AqlRingStateV1,
    observed_read: nat,
    after: AqlRingStateV1,
    reservation: AqlReservationV1,
)
    requires
        canonical_ring_state_v1(before),
        accepted_reservation_v1(before, observed_read, after, reservation),
    ensures
        after.capacity == before.capacity,
        after.write == before.write + 1,
        after.last_read == observed_read,
        reservation.packet_id == before.write,
        reservation.slot == before.write % before.capacity,
        reservation.observed_read == observed_read,
        reservation.next_write == after.write,
        canonical_ring_state_v1(after),
{
}

pub proof fn two_accepted_reservations_form_one_linear_chain_v1(
    initial: AqlRingStateV1,
    first_read: nat,
    middle: AqlRingStateV1,
    first: AqlReservationV1,
    second_read: nat,
    final_state: AqlRingStateV1,
    second: AqlReservationV1,
)
    requires
        canonical_ring_state_v1(initial),
        accepted_reservation_v1(initial, first_read, middle, first),
        accepted_reservation_v1(middle, second_read, final_state, second),
    ensures
        first.packet_id == initial.write,
        second.packet_id == first.packet_id + 1,
        second.packet_id != first.packet_id,
        middle.write == initial.write + 1,
        final_state.write == initial.write + 2,
{
    accepted_reservation_advances_exactly_once_v1(
        initial,
        first_read,
        middle,
        first,
    );
    accepted_reservation_advances_exactly_once_v1(
        middle,
        second_read,
        final_state,
        second,
    );
}

pub proof fn accepted_observation_preserves_nondecreasing_read_v1(
    before: AqlRingStateV1,
    observed_read: nat,
    after: AqlRingStateV1,
    reservation: AqlReservationV1,
)
    requires
        canonical_ring_state_v1(before),
        accepted_reservation_v1(before, observed_read, after, reservation),
    ensures
        before.last_read <= observed_read,
        after.last_read == observed_read,
        before.last_read <= after.last_read,
{
}

pub proof fn full_ring_rejects_without_reservation_v1(
    state: AqlRingStateV1,
    observed_read: nat,
)
    requires
        canonical_ring_state_v1(state),
        state.last_read <= observed_read <= state.write,
        state.write - observed_read == state.capacity,
    ensures
        reserve_one_v1(state, observed_read)
            == (AqlReservationOutcomeV1::Rejected {
                state,
                reason: AqlReservationRejectionV1::Full,
            }),
{
}

pub proof fn every_rejected_reservation_preserves_state_v1(
    state: AqlRingStateV1,
    observed_read: nat,
)
    ensures
        outcome_is_rejected_v1(reserve_one_v1(state, observed_read)) ==>
            outcome_state_v1(reserve_one_v1(state, observed_read)) == state,
{
}

pub open spec fn witness_packet_v1() -> Seq<u8> {
    Seq::new(packet_bytes_v1(), |index: int|
        if index == 0 {
            invalid_packet_type_v1()
        } else if index == 1 {
            0u8
        } else if index == 2 {
            2u8
        } else if index == 3 {
            0u8
        } else {
            0xa5u8
        }
    )
}

pub open spec fn witness_ring_v1() -> Seq<u8> {
    Seq::new(2 * packet_bytes_v1(), |_index: int| 0x7bu8)
}

pub open spec fn witness_state_v1() -> AqlRingStateV1 {
    AqlRingStateV1 {
        capacity: 4,
        write: 1,
        last_read: 0,
    }
}

pub open spec fn witness_after_v1() -> AqlRingStateV1 {
    AqlRingStateV1 {
        capacity: 4,
        write: 2,
        last_read: 0,
    }
}

pub open spec fn witness_reservation_v1() -> AqlReservationV1 {
    AqlReservationV1 {
        packet_id: 1,
        slot: 1,
        observed_read: 0,
        next_write: 2,
    }
}

pub open spec fn witness_final_state_v1() -> AqlRingStateV1 {
    AqlRingStateV1 {
        capacity: 4,
        write: 3,
        last_read: 1,
    }
}

pub open spec fn witness_second_reservation_v1() -> AqlReservationV1 {
    AqlReservationV1 {
        packet_id: 2,
        slot: 2,
        observed_read: 1,
        next_write: 3,
    }
}

pub open spec fn witness_full_state_v1() -> AqlRingStateV1 {
    AqlRingStateV1 {
        capacity: 4,
        write: 4,
        last_read: 0,
    }
}

pub proof fn aql_publication_and_reservation_model_is_inhabited_v1()
    ensures
        canonical_unpublished_packet_v1(witness_packet_v1(), 2),
        canonical_ring_frame_v1(witness_ring_v1(), 2, 1),
        witness_packet_v1()[4] == 0xa5u8,
        published_ring_v1(witness_packet_v1(), witness_ring_v1(), 1)[68]
            == 0xa5u8,
        published_ring_v1(witness_packet_v1(), witness_ring_v1(), 1)[0]
            == 0x7bu8,
        little_endian_u32_v1(
            published_ring_v1(witness_packet_v1(), witness_ring_v1(), 1),
            64,
        ) == 0x0002_1402,
        pending_completion_signal_image_v1().len() == 64,
        pending_completion_signal_image_v1()[0] == 1u8,
        pending_completion_signal_image_v1()[8] == 1u8,
        pending_completion_signal_image_v1()[1] == 0u8,
        pending_completion_signal_image_v1()[16] == 0u8,
        classify_acquired_completion_value_v1(1) == AqlCompletionObservationV1::Pending,
        classify_acquired_completion_value_v1(0) == AqlCompletionObservationV1::Completed,
        classify_acquired_completion_value_v1(-7i64)
            == (AqlCompletionObservationV1::Unexpected { value: -7i64 }),
        canonical_ring_state_v1(witness_state_v1()),
        accepted_reservation_v1(
            witness_state_v1(),
            0,
            witness_after_v1(),
            witness_reservation_v1(),
        ),
        accepted_reservation_v1(
            witness_after_v1(),
            1,
            witness_final_state_v1(),
            witness_second_reservation_v1(),
        ),
        canonical_ring_state_v1(witness_full_state_v1()),
        reserve_one_v1(witness_full_state_v1(), 0)
            == (AqlReservationOutcomeV1::Rejected {
                state: witness_full_state_v1(),
                reason: AqlReservationRejectionV1::Full,
            }),
{
    pending_completion_signal_image_is_exact_v1();
    completion_observation_classifier_is_exact_v1(1);
    completion_observation_classifier_is_exact_v1(0);
    completion_observation_classifier_is_exact_v1(-7i64);
    canonical_invalid_then_release_transition_v1(
        witness_packet_v1(),
        witness_ring_v1(),
        2,
        1,
        2,
    );
    two_accepted_reservations_form_one_linear_chain_v1(
        witness_state_v1(),
        0,
        witness_after_v1(),
        witness_reservation_v1(),
        1,
        witness_final_state_v1(),
        witness_second_reservation_v1(),
    );
    full_ring_rejects_without_reservation_v1(witness_full_state_v1(), 0);
}

} // verus!
