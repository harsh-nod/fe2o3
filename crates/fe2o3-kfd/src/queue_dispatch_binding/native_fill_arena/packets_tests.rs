use super::*;
use fe2o3_resource_accounting::{
    ResourceKindV1, ResourceVectorV1, host_metadata_table_payload_bytes_v1,
};

fn packet(index: usize) -> Gfx942FixedDispatchPacketV1 {
    let mut bytes = vec![0; 272];
    bytes[8..16].copy_from_slice(&1u64.to_le_bytes());
    Gfx942FixedDispatchPacketV1::new(
        0,
        AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap(),
        0,
        bytes.into_boxed_slice(),
        Box::new([Gfx942DispatchBufferBindingV1::new(
            0,
            0,
            index as u64 * 4,
            4,
        )]),
    )
    .require_conditional_fill_v1()
}

fn account(bytes: u64) -> ResourceCreditAccountV1 {
    ResourceCreditAccountV1::new(
        ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, bytes),
        1,
    )
    .unwrap()
}

fn payload_bytes() -> u64 {
    host_metadata_table_payload_bytes_v1::<Gfx942FixedDispatchPacketV1>(SLOTS).unwrap()
}

#[test]
fn arena_packet_heap_precharge_refuses_before_initializer_and_retains_exact_payload() {
    let short = account(payload_bytes() - 1);
    assert!(
        Gfx942NativeFillArenaPacketsV1::try_new(&short, |_| {
            panic!("packet initialization preceded full header reservation")
        })
        .is_err()
    );
    assert_eq!(short.usage().used, ResourceVectorV1::ZERO);
    assert_eq!(short.usage().retained_records, 0);

    let exact = account(payload_bytes());
    let mut next = 0;
    let packets = Gfx942NativeFillArenaPacketsV1::try_new(&exact, |index| {
        assert_eq!(index, next);
        next += 1;
        assert_eq!(
            exact.usage().used.get(ResourceKindV1::ControlResidentBytes),
            payload_bytes()
        );
        packet(index)
    })
    .unwrap();
    assert_eq!(next, SLOTS);
    assert_eq!(exact.usage().retained_records, 1);
    let pointer = packets.packets().as_ptr();
    let moved = std::hint::black_box(packets);
    assert_eq!(moved.packets().as_ptr(), pointer);
    for (index, packet) in moved.packets().iter().enumerate() {
        assert_eq!(packet.buffers[0].data_byte_offset, index as u64 * 4);
    }
    drop(moved);
    assert_eq!(exact.usage().used, ResourceVectorV1::ZERO);
    assert_eq!(exact.usage().retained_records, 0);
}

#[test]
fn arena_packet_partial_initializer_unwind_refunds_only_effect_free_table() {
    let account = account(payload_bytes());
    let mut calls = 0;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = Gfx942NativeFillArenaPacketsV1::try_new(&account, |index| {
            calls += 1;
            assert_eq!(
                account
                    .usage()
                    .used
                    .get(ResourceKindV1::ControlResidentBytes),
                payload_bytes()
            );
            assert!(index != 517, "injected inert packet initializer panic");
            packet(index)
        });
    }));
    assert!(result.is_err());
    assert_eq!(calls, 518);
    assert_eq!(account.usage().used, ResourceVectorV1::ZERO);
    assert_eq!(account.usage().retained_records, 0);
}

#[test]
fn arena_packet_admission_and_preparation_headers_have_bounded_inline_size() {
    type Custody = FixedDispatchPreparationCustodyV1<SLOTS, Gfx942NativeFillArenaPacketsV1>;
    assert!(core::mem::size_of::<Gfx942NativeFillArenaPacketsV1>() <= 64);
    assert!(core::mem::size_of::<Gfx942NativeFillArenaInputsV1<'static>>() <= 128);
    assert!(core::mem::size_of::<Gfx942NativeFillArenaFailureV1<'static>>() <= 4096);
    assert!(core::mem::size_of::<Custody>() <= 16384);
    assert!(
        core::mem::size_of::<Custody>()
            < core::mem::size_of::<[Gfx942FixedDispatchPacketV1; SLOTS]>()
    );
}

#[test]
fn arena_local_placement_checks_every_original_before_mutation_and_restores_exact_headers() {
    let funded = account(payload_bytes());
    let mut packets = Gfx942NativeFillArenaPacketsV1::try_new(&funded, |_| packet(0)).unwrap();
    let pointer = packets.packets().as_ptr();
    packets.packets[1023].buffers[0].data_byte_offset = 4;
    assert!(packets.place_local_outputs(SLOTS as u64 * 4).is_err());
    assert!(
        packets.packets[..1023]
            .iter()
            .all(|packet| packet.buffers[0].data_byte_offset == 0)
    );
    assert_eq!(packets.packets[1023].buffers[0].data_byte_offset, 4);
    packets.packets[1023].buffers[0].data_byte_offset = 0;
    assert!(packets.place_local_outputs(SLOTS as u64 * 4 + 4).is_err());
    assert!(
        packets
            .packets
            .iter()
            .all(|packet| packet.buffers[0].data_byte_offset == 0)
    );
    packets.place_local_outputs(SLOTS as u64 * 4).unwrap();
    for (index, packet) in packets.packets.iter().enumerate() {
        assert_eq!(packet.buffers[0].data_byte_offset, index as u64 * 4);
    }
    packets.restore_local_outputs();
    assert!(
        packets
            .packets
            .iter()
            .all(|packet| packet.buffers[0].data_byte_offset == 0)
    );
    assert_eq!(packets.packets().as_ptr(), pointer);
    assert_eq!(funded.usage().retained_records, 1);
}
