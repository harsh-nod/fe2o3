use core::mem::{align_of, size_of};
use fe2o3_aql::{
    AMD_SIGNAL_ALIGNMENT_V1, AMD_SIGNAL_BYTES_V1, AMD_SIGNAL_KIND_USER_V1,
    AMD_SIGNAL_VALUE_COMPLETE_V1, AMD_SIGNAL_VALUE_PENDING_V1,
    AQL_BATCH_RESERVATION_MODEL_MANIFEST_SHA256_V1, AQL_BATCH_RESERVATION_MODEL_MANIFEST_V1,
    AQL_DISPATCH_ABI_SCHEMA_MANIFEST_SHA256_BYTES_V1, AQL_DISPATCH_ABI_SCHEMA_MANIFEST_SHA256_V1,
    AQL_DISPATCH_ABI_SCHEMA_MANIFEST_V1, AQL_FIXED_BATCH_MODEL_MANIFEST_SHA256_V2,
    AQL_FIXED_BATCH_MODEL_MANIFEST_V2, AQL_KERNEL_DISPATCH_PACKET_BYTES_V1,
    AQL_MAX_BATCH_PACKETS_V1, AQL_MAX_FIXED_BATCH_PACKETS_V2,
    AQL_SYSTEM_SCOPED_KERNEL_DISPATCH_HEADER_V1, AmdBusyCompletionSignalV1,
    AqlAddressObservationError, AqlCompletionObservationV1, AqlDispatchGeometryV1,
    AqlDispatchOrderingV1, AqlDispatchPacketError, AqlGeometryError, AqlKernelDispatchPacketV1,
    AqlPacketBatchPublicationTargetV1, AqlPacketPublicationTargetV1,
    AqlPreparedKernelDispatchBatchErrorV1, AqlPreparedKernelDispatchBatchV1,
    AqlPreparedKernelDispatchBatchV2, AqlPreparedKernelDispatchV1, AqlRingCapacityError,
    AqlRingCapacityV1, AqlRingReservationError, AqlSingleProducerRingModelV1, ObservedGpuAddressV1,
    classify_acquired_completion_value_v1, encode_pending_completion_signal_bytes_v1,
    initialize_pending_completion_signal_bytes_v1,
};
use sha2::{Digest, Sha256};

#[test]
fn schema_digest_is_frozen() {
    let digest = Sha256::digest(AQL_DISPATCH_ABI_SCHEMA_MANIFEST_V1);
    assert_eq!(hex(&digest), AQL_DISPATCH_ABI_SCHEMA_MANIFEST_SHA256_V1);
    assert_eq!(
        digest.as_slice(),
        AQL_DISPATCH_ABI_SCHEMA_MANIFEST_SHA256_BYTES_V1
    );
}

#[test]
fn fixed_batch_v2_is_additive_and_frozen() {
    let digest = Sha256::digest(AQL_FIXED_BATCH_MODEL_MANIFEST_V2);
    assert_eq!(hex(&digest), AQL_FIXED_BATCH_MODEL_MANIFEST_SHA256_V2);
    assert_eq!(AQL_MAX_BATCH_PACKETS_V1, 256);
    assert_eq!(AQL_MAX_FIXED_BATCH_PACKETS_V2, 8192);
    assert!(
        AQL_FIXED_BATCH_MODEL_MANIFEST_V2.contains(AQL_BATCH_RESERVATION_MODEL_MANIFEST_SHA256_V1)
    );
}

#[test]
fn batch_reservation_model_digest_is_frozen_without_changing_the_wire_abi() {
    let digest = Sha256::digest(AQL_BATCH_RESERVATION_MODEL_MANIFEST_V1);
    assert_eq!(hex(&digest), AQL_BATCH_RESERVATION_MODEL_MANIFEST_SHA256_V1);

    assert_eq!(AQL_MAX_BATCH_PACKETS_V1, 256);
    assert_eq!(AQL_KERNEL_DISPATCH_PACKET_BYTES_V1, 64);
    assert_eq!(AQL_SYSTEM_SCOPED_KERNEL_DISPATCH_HEADER_V1, 0x1402);
    assert_eq!(
        AQL_DISPATCH_ABI_SCHEMA_MANIFEST_SHA256_V1,
        "82fbd7cf0b6c8647dce3f9b11e4f13a2dadfe3423509f769a4bc6cc87bb7acd0"
    );
}

#[test]
fn packet_and_signal_layouts_are_exact() {
    assert_eq!(
        size_of::<AqlKernelDispatchPacketV1>(),
        AQL_KERNEL_DISPATCH_PACKET_BYTES_V1
    );
    assert_eq!(align_of::<AqlKernelDispatchPacketV1>(), 8);
    assert_eq!(size_of::<AmdBusyCompletionSignalV1>(), AMD_SIGNAL_BYTES_V1);
    assert_eq!(
        align_of::<AmdBusyCompletionSignalV1>(),
        AMD_SIGNAL_ALIGNMENT_V1
    );
}

#[test]
fn exact_unpublished_packet_and_final_header() {
    let geometry = AqlDispatchGeometryV1::new([1024, 1, 1], [64, 1, 1]).unwrap();
    let prepared = AqlKernelDispatchPacketV1::new_unpublished(
        geometry,
        0,
        4096,
        ObservedGpuAddressV1::new(0x1000).unwrap(),
        ObservedGpuAddressV1::new(0x2080).unwrap(),
        16,
        ObservedGpuAddressV1::new(0x3000).unwrap(),
    )
    .unwrap();

    let mut target = CaptureTarget::default();
    prepared.publish_with(&mut target).unwrap();
    let packet = target.unpublished.unwrap();
    assert_eq!(&packet[0..4], &0x0001_0001_u32.to_le_bytes());
    assert_eq!(&packet[32..40], &0x1000_u64.to_le_bytes());
    assert_eq!(&packet[40..48], &0x2080_u64.to_le_bytes());
    assert_eq!(&packet[56..64], &0x3000_u64.to_le_bytes());
    assert_eq!(target.publication_header, Some(0x1402));
    assert_eq!(AQL_SYSTEM_SCOPED_KERNEL_DISPATCH_HEADER_V1, 0x1402);
}

#[test]
fn prepared_publication_keeps_each_setup_dimension_paired() {
    for dimensions in 1_u16..=3 {
        let grid = match dimensions {
            1 => [64, 1, 1],
            2 => [64, 2, 1],
            _ => [64, 2, 3],
        };
        let prepared = AqlKernelDispatchPacketV1::new_unpublished(
            AqlDispatchGeometryV1::new(grid, [64, 1, 1]).unwrap(),
            0,
            0,
            ObservedGpuAddressV1::new(0x1000).unwrap(),
            ObservedGpuAddressV1::new(0x2000).unwrap(),
            16,
            ObservedGpuAddressV1::new(0x3000).unwrap(),
        )
        .unwrap();
        let mut target = CaptureTarget::default();
        prepared.publish_with(&mut target).unwrap();
        let body = target.unpublished.unwrap();
        assert_eq!(
            u32::from_le_bytes(body[0..4].try_into().unwrap()),
            (u32::from(dimensions) << 16) | 1
        );
        assert_eq!(target.publication_header, Some(0x1402));
    }
}

#[test]
fn prepared_batch_writes_every_invalid_body_before_any_release_header() {
    let batch =
        AqlPreparedKernelDispatchBatchV1::<4>::try_from_packets(core::array::from_fn(|_| {
            prepared_packet()
        }))
        .unwrap();
    assert_eq!(batch.packet_count(), 4);
    let mut target = BatchCaptureTarget::default();
    batch.publish_with(&mut target).unwrap();
    assert_eq!(
        target.events,
        [
            BatchEvent::Body(0),
            BatchEvent::Body(1),
            BatchEvent::Body(2),
            BatchEvent::Body(3),
            BatchEvent::Header(0, 0x1402),
            BatchEvent::Header(1, 0x1402),
            BatchEvent::Header(2, 0x1402),
            BatchEvent::Header(3, 0x1402),
        ]
    );
}

#[test]
fn prepared_batch_retains_each_exact_dispatch_ordering_header() {
    assert_eq!(AqlDispatchOrderingV1::Independent.header(), 0x1402);
    assert_eq!(AqlDispatchOrderingV1::WaitForPrior.header(), 0x1502);
    assert_eq!(
        AqlDispatchOrderingV1::from_header(0x1402),
        Some(AqlDispatchOrderingV1::Independent)
    );
    assert_eq!(
        AqlDispatchOrderingV1::from_header(0x1502),
        Some(AqlDispatchOrderingV1::WaitForPrior)
    );
    assert_eq!(AqlDispatchOrderingV1::from_header(0x1503), None);

    let batch = AqlPreparedKernelDispatchBatchV1::<2>::try_from_packets([
        prepared_packet_with_ordering(AqlDispatchOrderingV1::Independent),
        prepared_packet_with_ordering(AqlDispatchOrderingV1::WaitForPrior),
    ])
    .unwrap();
    let mut target = BatchCaptureTarget::default();
    batch.publish_with(&mut target).unwrap();
    assert_eq!(
        target.events,
        [
            BatchEvent::Body(0),
            BatchEvent::Body(1),
            BatchEvent::Header(0, 0x1402),
            BatchEvent::Header(1, 0x1502),
        ]
    );
}

#[test]
fn prepared_batch_rejects_counts_outside_the_reviewed_bound() {
    assert_eq!(
        AqlPreparedKernelDispatchBatchV1::<0>::try_from_packets([]),
        Err(AqlPreparedKernelDispatchBatchErrorV1::ZeroPacketCount)
    );
    assert_eq!(
        AqlPreparedKernelDispatchBatchV1::<257>::try_from_packets(core::array::from_fn(|_| {
            prepared_packet()
        })),
        Err(
            AqlPreparedKernelDispatchBatchErrorV1::PacketCountExceedsReviewedMaximum {
                requested: 257,
                maximum: 256,
            }
        )
    );
}

#[test]
fn fixed_batch_v2_admits_8192_with_one_two_phase_publication() {
    let packets = (0..8192)
        .map(|_| prepared_packet())
        .collect::<Vec<_>>()
        .into_boxed_slice()
        .try_into()
        .unwrap();
    let batch = AqlPreparedKernelDispatchBatchV2::<8192>::try_from_boxed_packets(packets).unwrap();
    assert_eq!(batch.packet_count(), 8192);
    let mut target = BatchCaptureTarget::default();
    batch.publish_with(&mut target).unwrap();
    assert_eq!(target.events.len(), 16384);
    assert!(matches!(target.events[0], BatchEvent::Body(0)));
    assert!(matches!(target.events[8191], BatchEvent::Body(8191)));
    assert!(matches!(target.events[8192], BatchEvent::Header(0, 0x1402)));
    assert!(matches!(
        target.events[16383],
        BatchEvent::Header(8191, 0x1402)
    ));

    let over_bound = (0..8193)
        .map(|_| prepared_packet())
        .collect::<Vec<_>>()
        .into_boxed_slice()
        .try_into()
        .unwrap();
    assert_eq!(
        AqlPreparedKernelDispatchBatchV2::<8193>::try_from_boxed_packets(over_bound),
        Err(
            AqlPreparedKernelDispatchBatchErrorV1::PacketCountExceedsReviewedMaximum {
                requested: 8193,
                maximum: 8192,
            }
        )
    );
}

#[test]
fn geometry_rejects_invalid_dimensions_and_bounds() {
    assert_eq!(
        AqlDispatchGeometryV1::new([0, 1, 1], [1, 1, 1]),
        Err(AqlGeometryError::ZeroGrid)
    );
    assert_eq!(
        AqlDispatchGeometryV1::new([1, 1, 1], [0, 1, 1]),
        Err(AqlGeometryError::ZeroWorkgroup)
    );
    assert_eq!(
        AqlDispatchGeometryV1::new([1, 1, 1], [u16::MAX as u32 + 1, 1, 1]),
        Err(AqlGeometryError::WorkgroupTooLarge)
    );
    assert_eq!(
        AqlDispatchGeometryV1::new([63, 1, 1], [64, 1, 1]),
        Err(AqlGeometryError::GridSmallerThanWorkgroup)
    );
}

#[test]
fn geometry_derives_exact_dimensions() {
    assert_eq!(
        AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1])
            .unwrap()
            .dimensions(),
        1
    );
    assert_eq!(
        AqlDispatchGeometryV1::new([64, 2, 1], [64, 2, 1])
            .unwrap()
            .dimensions(),
        2
    );
    assert_eq!(
        AqlDispatchGeometryV1::new([64, 2, 3], [64, 2, 1])
            .unwrap()
            .dimensions(),
        3
    );
}

#[test]
fn packet_rejects_address_substitution_and_misalignment() {
    let geometry = AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap();
    let valid = ObservedGpuAddressV1::new(0x1000).unwrap();
    assert_eq!(
        ObservedGpuAddressV1::new(0),
        Err(AqlAddressObservationError::Zero)
    );
    assert_eq!(
        AqlKernelDispatchPacketV1::new_unpublished(
            geometry,
            0,
            0,
            ObservedGpuAddressV1::new(0x1008).unwrap(),
            valid,
            16,
            valid,
        ),
        Err(AqlDispatchPacketError::KernelObject(
            AqlAddressObservationError::Misaligned
        ))
    );
    assert_eq!(
        AqlKernelDispatchPacketV1::new_unpublished(geometry, 0, 0, valid, valid, 3, valid,),
        Err(AqlDispatchPacketError::Kernarg(
            AqlAddressObservationError::InvalidRequiredAlignment
        ))
    );
}

#[test]
fn busy_signal_starts_pending() {
    let signal = AmdBusyCompletionSignalV1::new_pending();
    assert_eq!(
        signal.observe_acquire(),
        AqlCompletionObservationV1::Pending
    );
}

#[test]
fn pending_signal_byte_initializer_overwrites_every_byte_exactly() {
    let mut expected = [0_u8; AMD_SIGNAL_BYTES_V1];
    expected[0..8].copy_from_slice(&AMD_SIGNAL_KIND_USER_V1.to_le_bytes());
    expected[8..16].copy_from_slice(&AMD_SIGNAL_VALUE_PENDING_V1.to_le_bytes());

    let encoded = encode_pending_completion_signal_bytes_v1();
    assert_eq!(encoded, expected);
    assert_eq!(&encoded[0..8], &1_i64.to_le_bytes());
    assert_eq!(&encoded[8..16], &1_i64.to_le_bytes());
    assert!(encoded[16..].iter().all(|byte| *byte == 0));

    let mut destination = [0xa5_u8; AMD_SIGNAL_BYTES_V1];
    initialize_pending_completion_signal_bytes_v1(&mut destination);
    assert_eq!(destination, expected);
}

#[test]
fn pure_completion_classifier_is_exact_and_preserves_unexpected_values() {
    assert_eq!(AMD_SIGNAL_VALUE_PENDING_V1, 1);
    assert_eq!(AMD_SIGNAL_VALUE_COMPLETE_V1, 0);
    assert_eq!(
        classify_acquired_completion_value_v1(AMD_SIGNAL_VALUE_PENDING_V1),
        AqlCompletionObservationV1::Pending
    );
    assert_eq!(
        classify_acquired_completion_value_v1(AMD_SIGNAL_VALUE_COMPLETE_V1),
        AqlCompletionObservationV1::Completed
    );

    for unexpected in [i64::MIN, -7, -1, 2, i64::MAX] {
        assert_eq!(
            classify_acquired_completion_value_v1(unexpected),
            AqlCompletionObservationV1::Unexpected(unexpected)
        );
    }
}

#[test]
fn ring_capacity_and_reservation_are_exact() {
    assert_eq!(
        AqlRingCapacityV1::from_ring_bytes(2048),
        Err(AqlRingCapacityError::BelowMinimum)
    );
    assert_eq!(
        AqlRingCapacityV1::from_ring_bytes(6144),
        Err(AqlRingCapacityError::NotPowerOfTwo)
    );
    let capacity = AqlRingCapacityV1::from_ring_bytes(4096).unwrap();
    assert_eq!(capacity.packets(), 64);
    let mut model = AqlSingleProducerRingModelV1::new(capacity, 65, 64).unwrap();
    let reservation = model.reserve_one(64).unwrap();
    assert_eq!(reservation.packet_id(), 65);
    assert_eq!(reservation.slot_index(), 1);
    assert_eq!(reservation.observed_read(), 64);
    assert_eq!(reservation.next_write(), 66);
    assert_eq!(model.write(), 66);
    assert_eq!(model.last_read(), 64);
}

#[test]
fn ring_reservation_fails_closed_on_counter_anomalies() {
    let capacity = AqlRingCapacityV1::from_ring_bytes(4096).unwrap();
    assert_eq!(
        AqlSingleProducerRingModelV1::new(capacity, 1, 2),
        Err(AqlRingReservationError::ReadAfterWrite)
    );
    assert_eq!(
        AqlSingleProducerRingModelV1::new(capacity, 65, 0),
        Err(AqlRingReservationError::CounterDistanceExceedsCapacity)
    );
    let mut full = AqlSingleProducerRingModelV1::new(capacity, 64, 0).unwrap();
    assert_eq!(full.reserve_one(0), Err(AqlRingReservationError::Full));
    let mut exhausted = AqlSingleProducerRingModelV1::new(capacity, u64::MAX, u64::MAX).unwrap();
    assert_eq!(
        exhausted.reserve_one(u64::MAX),
        Err(AqlRingReservationError::WriteCounterExhausted)
    );
    let mut regressed = AqlSingleProducerRingModelV1::new(capacity, 10, 5).unwrap();
    regressed.reserve_one(5).unwrap();
    assert_eq!(
        regressed.reserve_one(4),
        Err(AqlRingReservationError::ReadRegressed)
    );
}

#[test]
fn a_full_window_uses_each_slot_once() {
    let capacity = AqlRingCapacityV1::from_ring_bytes(4096).unwrap();
    let mut model = AqlSingleProducerRingModelV1::new(capacity, 0, 0).unwrap();
    for write in 0_u64..64 {
        let reservation = model.reserve_one(0).unwrap();
        assert_eq!(reservation.slot_index(), write as u32);
    }
    assert_eq!(model.reserve_one(0), Err(AqlRingReservationError::Full));
}

#[test]
fn one_hundred_thousand_completed_reservations_wrap_slots_exactly() {
    let capacity = AqlRingCapacityV1::from_ring_bytes(4096).unwrap();
    let mut model = AqlSingleProducerRingModelV1::new(capacity, 0, 0).unwrap();
    for expected in 0_u64..100_000 {
        let reservation = model.reserve_one(expected).unwrap();
        assert_eq!(reservation.packet_id(), expected);
        assert_eq!(reservation.slot_index(), (expected & 63) as u32);
    }
    assert_eq!(model.write(), 100_000);
}

#[test]
fn batch_reservation_wraps_in_order_with_distinct_slots() {
    let capacity = AqlRingCapacityV1::from_ring_bytes(4096).unwrap();
    let mut model = AqlSingleProducerRingModelV1::new(capacity, 62, 62).unwrap();
    let reservation = model.reserve_batch(62, 4).unwrap();

    assert_eq!(reservation.first_packet_id(), 62);
    assert_eq!(reservation.packet_count(), 4);
    assert_eq!(reservation.observed_read(), 62);
    assert_eq!(reservation.next_write(), 66);
    assert_eq!(reservation.last_packet_id(), 65);
    assert_eq!(
        reservation
            .entries()
            .map(|entry| (entry.packet_id(), entry.slot_index()))
            .collect::<Vec<_>>(),
        [(62, 62), (63, 63), (64, 0), (65, 1)]
    );
    assert_eq!(reservation.entry(0).unwrap().slot_index(), 62);
    assert_eq!(reservation.entry(3).unwrap().slot_index(), 1);
    assert_eq!(reservation.entry(4), None);
    assert_eq!(model.write(), 66);
    assert_eq!(model.last_read(), 62);
}

#[test]
fn batch_count_and_space_rejections_leave_both_counters_unchanged() {
    let minimum = AqlRingCapacityV1::from_ring_bytes(4096).unwrap();
    let larger = AqlRingCapacityV1::from_ring_bytes(32768).unwrap();

    assert_batch_failure_unchanged(
        AqlSingleProducerRingModelV1::new(minimum, 0, 0).unwrap(),
        0,
        0,
        AqlRingReservationError::ZeroPacketCount,
    );
    assert_batch_failure_unchanged(
        AqlSingleProducerRingModelV1::new(larger, 0, 0).unwrap(),
        0,
        AQL_MAX_BATCH_PACKETS_V1 + 1,
        AqlRingReservationError::PacketCountExceedsReviewedMaximum {
            requested: 257,
            maximum: 256,
        },
    );
    assert_batch_failure_unchanged(
        AqlSingleProducerRingModelV1::new(minimum, 0, 0).unwrap(),
        0,
        65,
        AqlRingReservationError::PacketCountExceedsRingCapacity {
            requested: 65,
            capacity: 64,
        },
    );
    assert_batch_failure_unchanged(
        AqlSingleProducerRingModelV1::new(minimum, 64, 0).unwrap(),
        0,
        1,
        AqlRingReservationError::Full,
    );
    assert_batch_failure_unchanged(
        AqlSingleProducerRingModelV1::new(minimum, 63, 0).unwrap(),
        0,
        2,
        AqlRingReservationError::InsufficientSpace {
            requested: 2,
            available: 1,
        },
    );

    // Even a newer read observation is not retained when the batch fails.
    assert_batch_failure_unchanged(
        AqlSingleProducerRingModelV1::new(minimum, 63, 0).unwrap(),
        5,
        7,
        AqlRingReservationError::InsufficientSpace {
            requested: 7,
            available: 6,
        },
    );
}

#[test]
fn batch_counter_rejections_leave_both_counters_unchanged() {
    let capacity = AqlRingCapacityV1::from_ring_bytes(4096).unwrap();
    assert_batch_failure_unchanged(
        AqlSingleProducerRingModelV1::new(capacity, 10, 5).unwrap(),
        4,
        1,
        AqlRingReservationError::ReadRegressed,
    );
    assert_batch_failure_unchanged(
        AqlSingleProducerRingModelV1::new(capacity, 10, 5).unwrap(),
        11,
        1,
        AqlRingReservationError::ReadAfterWrite,
    );
    assert_batch_failure_unchanged(
        AqlSingleProducerRingModelV1::new(capacity, u64::MAX - 1, u64::MAX - 1).unwrap(),
        u64::MAX - 1,
        2,
        AqlRingReservationError::WriteCounterExhausted,
    );
}

#[test]
fn minimum_ring_admission_boundaries_are_exhaustive() {
    let capacity = AqlRingCapacityV1::from_ring_bytes(4096).unwrap();
    for occupancy in 0_u64..=64 {
        for requested in 1_u32..=64 {
            let read = 1_000_u64;
            let write = read + occupancy;
            let mut model = AqlSingleProducerRingModelV1::new(capacity, write, read).unwrap();
            let result = model.reserve_batch(read, requested);
            let available = 64 - occupancy;

            if u64::from(requested) <= available {
                let reservation = result.unwrap();
                assert_eq!(reservation.first_packet_id(), write);
                assert_eq!(reservation.packet_count(), requested);
                assert_eq!(reservation.next_write(), write + u64::from(requested));
                assert_eq!(model.write(), write + u64::from(requested));
                assert_eq!(model.last_read(), read);
            } else if available == 0 {
                assert_eq!(result, Err(AqlRingReservationError::Full));
                assert_eq!(model.write(), write);
                assert_eq!(model.last_read(), read);
            } else {
                assert_eq!(
                    result,
                    Err(AqlRingReservationError::InsufficientSpace {
                        requested,
                        available: available as u32,
                    })
                );
                assert_eq!(model.write(), write);
                assert_eq!(model.last_read(), read);
            }
        }
    }
}

#[test]
fn every_reviewed_batch_count_and_wrap_phase_has_distinct_ordered_slots() {
    let capacity = AqlRingCapacityV1::from_ring_bytes(32768).unwrap();
    assert_eq!(capacity.packets(), 512);

    for first_slot in 0_u64..512 {
        for packet_count in 1_u32..=AQL_MAX_BATCH_PACKETS_V1 {
            let mut model =
                AqlSingleProducerRingModelV1::new(capacity, first_slot, first_slot).unwrap();
            let reservation = model.reserve_batch(first_slot, packet_count).unwrap();
            let mut seen = [false; 512];
            let mut entries = reservation.entries();
            assert_eq!(entries.len(), packet_count as usize);

            for batch_index in 0..packet_count {
                let entry = entries.next().unwrap();
                let expected_packet = first_slot + u64::from(batch_index);
                let expected_slot = (expected_packet & 511) as u32;
                assert_eq!(entry.packet_id(), expected_packet);
                assert_eq!(entry.slot_index(), expected_slot);
                assert!(!seen[expected_slot as usize]);
                seen[expected_slot as usize] = true;
                assert_eq!(entries.len(), (packet_count - batch_index - 1) as usize);
            }
            assert_eq!(entries.next(), None);
        }
    }
}

#[test]
fn maximum_batch_and_last_nonoverflowing_counter_are_admitted() {
    let capacity = AqlRingCapacityV1::from_ring_bytes(32768).unwrap();
    let mut maximum = AqlSingleProducerRingModelV1::new(capacity, 0, 0).unwrap();
    let reservation = maximum.reserve_batch(0, AQL_MAX_BATCH_PACKETS_V1).unwrap();
    assert_eq!(reservation.packet_count(), 256);
    assert_eq!(reservation.entries().len(), 256);

    let ring = AqlRingCapacityV1::from_ring_bytes(4096).unwrap();
    let mut last = AqlSingleProducerRingModelV1::new(ring, u64::MAX - 2, u64::MAX - 2).unwrap();
    let reservation = last.reserve_batch(u64::MAX - 2, 2).unwrap();
    assert_eq!(reservation.first_packet_id(), u64::MAX - 2);
    assert_eq!(reservation.last_packet_id(), u64::MAX - 1);
    assert_eq!(reservation.next_write(), u64::MAX);
    assert_eq!(last.write(), u64::MAX);
}

#[test]
fn fixed_batch_v2_reservation_is_bounded_by_512k_ring_and_checked_arithmetic() {
    let capacity = AqlRingCapacityV1::from_ring_bytes(524_288).unwrap();
    let mut model = AqlSingleProducerRingModelV1::new(capacity, 777, 777).unwrap();
    assert_eq!(
        model.reserve_batch(777, 257).unwrap_err(),
        AqlRingReservationError::PacketCountExceedsReviewedMaximum {
            requested: 257,
            maximum: 256,
        }
    );
    let reservation = model
        .reserve_fixed_batch_v2(777, AQL_MAX_FIXED_BATCH_PACKETS_V2)
        .unwrap();
    assert_eq!(reservation.packet_count(), 8192);
    assert_eq!(reservation.first_packet_id(), 777);
    assert_eq!(reservation.last_packet_id(), 8968);
    assert_eq!(reservation.next_write(), 8969);
    assert_eq!(reservation.entries().len(), 8192);

    let mut too_many = AqlSingleProducerRingModelV1::new(capacity, 0, 0).unwrap();
    assert_eq!(
        too_many
            .reserve_fixed_batch_v2(0, AQL_MAX_FIXED_BATCH_PACKETS_V2 + 1)
            .unwrap_err(),
        AqlRingReservationError::PacketCountExceedsReviewedMaximum {
            requested: 8193,
            maximum: 8192,
        }
    );

    let mut exhausted =
        AqlSingleProducerRingModelV1::new(capacity, u64::MAX - 8191, u64::MAX - 8191).unwrap();
    let before = exhausted.write();
    assert_eq!(
        exhausted
            .reserve_fixed_batch_v2(u64::MAX - 8191, 8192)
            .unwrap_err(),
        AqlRingReservationError::WriteCounterExhausted
    );
    assert_eq!(exhausted.write(), before);
}

#[derive(Default)]
struct CaptureTarget {
    unpublished: Option<[u8; AQL_KERNEL_DISPATCH_PACKET_BYTES_V1]>,
    publication_header: Option<u16>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BatchEvent {
    Body(u32),
    Header(u32, u16),
}

#[derive(Default)]
struct BatchCaptureTarget {
    events: Vec<BatchEvent>,
}

impl AqlPacketBatchPublicationTargetV1 for BatchCaptureTarget {
    type Error = ();

    fn write_unpublished(
        &mut self,
        batch_index: u32,
        packet: &AqlKernelDispatchPacketV1,
    ) -> Result<(), Self::Error> {
        assert!(packet.is_unpublished());
        self.events.push(BatchEvent::Body(batch_index));
        Ok(())
    }

    fn publish_release_header(&mut self, batch_index: u32, header: u16) -> Result<(), Self::Error> {
        self.events.push(BatchEvent::Header(batch_index, header));
        Ok(())
    }
}

impl AqlPacketPublicationTargetV1 for CaptureTarget {
    type Error = ();

    fn write_unpublished(&mut self, packet: &AqlKernelDispatchPacketV1) -> Result<(), Self::Error> {
        self.unpublished = Some(packet.encode_unpublished_le());
        Ok(())
    }

    fn publish_release_header(&mut self, header: u16) -> Result<(), Self::Error> {
        self.publication_header = Some(header);
        Ok(())
    }
}

fn assert_batch_failure_unchanged(
    mut model: AqlSingleProducerRingModelV1,
    observed_read: u64,
    packet_count: u32,
    expected: AqlRingReservationError,
) {
    let before = (model.write(), model.last_read());
    assert_eq!(
        model.reserve_batch(observed_read, packet_count),
        Err(expected)
    );
    assert_eq!((model.write(), model.last_read()), before);
}

fn prepared_packet() -> AqlPreparedKernelDispatchV1 {
    prepared_packet_with_ordering(AqlDispatchOrderingV1::Independent)
}

fn prepared_packet_with_ordering(ordering: AqlDispatchOrderingV1) -> AqlPreparedKernelDispatchV1 {
    AqlKernelDispatchPacketV1::new_unpublished_with_ordering(
        AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap(),
        0,
        0,
        ObservedGpuAddressV1::new(0x1000).unwrap(),
        ObservedGpuAddressV1::new(0x2000).unwrap(),
        16,
        ObservedGpuAddressV1::new(0x3000).unwrap(),
        ordering,
    )
    .unwrap()
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        value.push(char::from(DIGITS[usize::from(*byte >> 4)]));
        value.push(char::from(DIGITS[usize::from(*byte & 0x0f)]));
    }
    value
}
