use core::mem::{align_of, size_of};
use fe2o3_aql::{
    AMD_SIGNAL_ALIGNMENT_V1, AMD_SIGNAL_BYTES_V1, AMD_SIGNAL_KIND_USER_V1,
    AMD_SIGNAL_VALUE_COMPLETE_V1, AMD_SIGNAL_VALUE_PENDING_V1,
    AQL_DISPATCH_ABI_SCHEMA_MANIFEST_SHA256_BYTES_V1, AQL_DISPATCH_ABI_SCHEMA_MANIFEST_SHA256_V1,
    AQL_DISPATCH_ABI_SCHEMA_MANIFEST_V1, AQL_KERNEL_DISPATCH_PACKET_BYTES_V1,
    AQL_SYSTEM_SCOPED_KERNEL_DISPATCH_HEADER_V1, AmdBusyCompletionSignalV1,
    AqlAddressObservationError, AqlCompletionObservationV1, AqlDispatchGeometryV1,
    AqlDispatchPacketError, AqlGeometryError, AqlKernelDispatchPacketV1,
    AqlPacketPublicationTargetV1, AqlRingCapacityError, AqlRingCapacityV1, AqlRingReservationError,
    AqlSingleProducerRingModelV1, ObservedGpuAddressV1, classify_acquired_completion_value_v1,
    encode_pending_completion_signal_bytes_v1, initialize_pending_completion_signal_bytes_v1,
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

#[derive(Default)]
struct CaptureTarget {
    unpublished: Option<[u8; AQL_KERNEL_DISPATCH_PACKET_BYTES_V1]>,
    publication_header: Option<u16>,
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

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        value.push(char::from(DIGITS[usize::from(*byte >> 4)]));
        value.push(char::from(DIGITS[usize::from(*byte & 0x0f)]));
    }
    value
}
