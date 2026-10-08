use super::*;
use fe2o3_resource_accounting::{
    ResourceKindV1, ResourceVectorV1, host_metadata_table_payload_bytes_v1,
};

const LARGE: usize = GFX942_INDEPENDENT_FILL_ARENA2048_SLOTS_V1;

impl DispatchResourceOwnerV1 {
    pub(in crate::queue) fn primary_assert_independent2048_roster_v1(&self) {
        assert_eq!(self.packets.len(), LARGE);
        assert_eq!(self.data.len(), 1);
        assert_eq!(self.code.len(), 1);
        assert_eq!(self.data_premises[0].valid_bytes, 16_380);
        assert_eq!(
            self.data_premises[0].effect,
            Some(DeviceDataEffectV1::WriteOnly)
        );
        assert!(
            self.packets
                .iter()
                .all(|p| p.ordering == AqlDispatchOrderingV1::Independent)
        );
    }
}

fn account(bytes: u64, records: usize) -> ResourceCreditAccountV1 {
    ResourceCreditAccountV1::new(
        ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, bytes),
        records,
    )
    .unwrap()
}

fn packet() -> Gfx942FixedDispatchPacketV1 {
    let mut bytes = vec![0; 272];
    bytes[8..16].copy_from_slice(&1u64.to_le_bytes());
    Gfx942FixedDispatchPacketV1::new_with_ordering(
        0,
        AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap(),
        AqlDispatchOrderingV1::Independent,
        0,
        bytes.into_boxed_slice(),
        Box::new([Gfx942DispatchBufferBindingV1::new(0, 0, 0, 4)]),
    )
    .require_conditional_fill_v1()
}

#[test]
fn independent2048_packet_payload_is_prepaid_and_original1024_charge_is_unchanged() {
    let original_bytes =
        host_metadata_table_payload_bytes_v1::<Gfx942FixedDispatchPacketV1>(SLOTS).unwrap();
    let large_bytes =
        host_metadata_table_payload_bytes_v1::<Gfx942FixedDispatchPacketV1>(LARGE).unwrap();
    assert_eq!(large_bytes, 2 * original_bytes);
    let original = account(original_bytes, 1);
    let packets = Gfx942NativeFillArenaPacketsV1::try_new(&original, |_| packet()).unwrap();
    assert_eq!(packets.packets().len(), SLOTS);
    assert_eq!(
        original
            .usage()
            .used
            .get(ResourceKindV1::ControlResidentBytes),
        original_bytes
    );
    drop(packets);
    let short = account(large_bytes - 1, 1);
    assert!(
        Gfx942IndependentFillArena2048PacketsV1::try_new(&short, |_| panic!(
            "initializer before full charge"
        ))
        .is_err()
    );
    assert_eq!(short.usage().used, ResourceVectorV1::ZERO);
    let exact = account(large_bytes, 1);
    let mut next = 0;
    let packets = Gfx942IndependentFillArena2048PacketsV1::try_new(&exact, |index| {
        assert_eq!(index, next);
        next += 1;
        assert_eq!(
            exact.usage().used.get(ResourceKindV1::ControlResidentBytes),
            large_bytes
        );
        packet()
    })
    .unwrap();
    assert_eq!(next, LARGE);
    assert_eq!(packets.packets().len(), LARGE);
    let pointer = packets.packets().as_ptr();
    let moved = std::hint::black_box(packets);
    assert_eq!(moved.packets().as_ptr(), pointer);
    drop(moved);
    assert_eq!(exact.usage().used, ResourceVectorV1::ZERO);
    assert_eq!(exact.usage().retained_records, 0);
}

#[test]
fn independent2048_partial_inert_initialization_unwind_closes_only_its_charge() {
    let bytes = host_metadata_table_payload_bytes_v1::<Gfx942FixedDispatchPacketV1>(LARGE).unwrap();
    let original = account(bytes, 1);
    let mut next = 0;
    let refused = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = Gfx942IndependentFillArena2048PacketsV1::try_new(&original, |index| {
            assert_eq!(index, next);
            next += 1;
            assert!(index != 1537, "injected inert packet refusal");
            packet()
        });
    }));
    assert!(refused.is_err());
    assert_eq!(next, 1538);
    assert_eq!(original.usage().used, ResourceVectorV1::ZERO);
    assert_eq!(original.usage().retained_records, 0);
}

#[test]
fn independent2048_storage_exact_payload_and_record_capacity_are_not_replenished() {
    let funded = account(64 << 20, LARGE + 3);
    let storage = Gfx942IndependentFillArena2048StorageV1::preallocate(funded.clone()).unwrap();
    let bytes = funded
        .usage()
        .used
        .get(ResourceKindV1::ControlResidentBytes);
    assert_eq!(funded.usage().retained_records, LARGE + 3);
    assert_eq!(storage.0.capacity(), ArenaCapacityV1::Independent2048);
    for (limit, records) in [(bytes - 1, LARGE + 3), (bytes, LARGE + 2)] {
        let short = account(limit, records);
        assert!(Gfx942IndependentFillArena2048StorageV1::preallocate(short.clone()).is_err());
        assert_eq!(short.usage().used, ResourceVectorV1::ZERO);
        assert_eq!(short.usage().retained_records, 0);
    }
    let exact = account(bytes, LARGE + 3);
    let exact_storage =
        Gfx942IndependentFillArena2048StorageV1::preallocate(exact.clone()).unwrap();
    assert_eq!(
        exact.usage().used.get(ResourceKindV1::ControlResidentBytes),
        bytes
    );
    drop(exact_storage);
    drop(storage);
    assert_eq!(exact.usage().used, ResourceVectorV1::ZERO);
    assert_eq!(funded.usage().used, ResourceVectorV1::ZERO);
}

#[test]
fn independent2048_profile_is_closed_and_original_inline_bounds_are_preserved() {
    assert!(ArenaCapacityV1::Original1024.permits(ArenaOrderV1::Ordered));
    assert!(ArenaCapacityV1::Original1024.permits(ArenaOrderV1::IndependentDisjointWriteOnly));
    assert!(!ArenaCapacityV1::Independent2048.permits(ArenaOrderV1::Ordered));
    assert!(ArenaCapacityV1::Independent2048.permits(ArenaOrderV1::IndependentDisjointWriteOnly));
    assert!(core::mem::size_of::<Gfx942IndependentFillArena2048PacketsV1>() <= 64);
    assert!(core::mem::size_of::<Gfx942IndependentFillArena2048InputsV1<'static>>() <= 128);
    assert!(
        core::mem::size_of::<
            FixedDispatchPreparationCustodyV1<LARGE, Gfx942NativeFillArenaPacketsV1>,
        >() <= 16384
    );
    assert!(validate_fixed_batch_ring::<LARGE>(64 * 1024).is_err());
    validate_fixed_batch_ring::<LARGE>(128 * 1024).unwrap();
    validate_fixed_batch_ring::<SLOTS>(64 * 1024).unwrap();
}

#[test]
fn independent2048_local_placement_preserves_original_headers_before_full_validation() {
    let bytes = host_metadata_table_payload_bytes_v1::<Gfx942FixedDispatchPacketV1>(LARGE).unwrap();
    let funded = account(bytes, 1);
    let mut packets =
        Gfx942IndependentFillArena2048PacketsV1::try_new(&funded, |_| packet()).unwrap();
    let pointer = packets.packets().as_ptr();
    assert!(packets.0.place_local_outputs(LARGE as u64 * 4 + 4).is_err());
    assert!(
        packets
            .packets()
            .iter()
            .all(|p| p.buffers[0].data_byte_offset == 0)
    );
    packets.0.place_local_outputs(LARGE as u64 * 4).unwrap();
    for (index, p) in packets.packets().iter().enumerate() {
        assert_eq!(p.buffers[0].data_byte_offset, index as u64 * 4);
    }
    packets.0.restore_local_outputs();
    assert!(
        packets
            .packets()
            .iter()
            .all(|p| p.buffers[0].data_byte_offset == 0)
    );
    assert_eq!(packets.packets().as_ptr(), pointer);
    assert_eq!(funded.usage().retained_records, 1);
}
