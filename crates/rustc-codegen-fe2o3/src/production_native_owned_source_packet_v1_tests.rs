//! Receipt arithmetic only; the dummy payload is never a typed source proof.
use super::*;

#[test]
fn packet_receipt_retains_actual_capacity_and_opaque_proof_receipt() {
    let mut native_module = Vec::with_capacity(97);
    native_module.extend_from_slice(b"inert bookkeeping payload");
    let capacity = native_module.capacity();
    let mut source_packet = Vec::with_capacity(139);
    source_packet.extend_from_slice(b"inert full source packet");
    let packet_capacity = source_packet.capacity();
    let proof_storage = size_of::<usize>() + 71;
    let retained = size_of::<PreparedNativeSourceProofPacketV1<usize>>() - size_of::<usize>()
        + proof_storage
        + capacity
        + packet_capacity;
    let packet = PreparedNativeSourceProofPacketV1::from_parts(packet::NativeSourcePacketPartsV1 {
        proof: 11usize,
        native_module,
        source_packet,
        retained,
    })
    .unwrap();
    assert_eq!(*packet.proof(), 11);
    assert_eq!(packet.proof_storage, proof_storage);
    assert_eq!(packet.retained_storage().unwrap(), retained);
    assert_eq!(packet.original_native_module.capacity(), capacity);
    assert_eq!(packet.source_packet.capacity(), packet_capacity);
    assert_eq!(packet.source_packet(), b"inert full source packet");
}

#[test]
fn packet_receipt_refuses_underflow_small_proof_and_overflow() {
    for retained in [
        0,
        packet_header::<usize>().unwrap() + size_of::<usize>() - 1,
    ] {
        assert!(matches!(
            PreparedNativeSourceProofPacketV1::from_parts(packet::NativeSourcePacketPartsV1 {
                proof: 11usize,
                native_module: Vec::new(),
                source_packet: Vec::new(),
                retained,
            }),
            Err(E::Resource(Resource::Accounting))
        ));
    }
    let packet = PreparedNativeSourceProofPacketV1 {
        proof: 11usize,
        original_native_module: Vec::new(),
        source_packet: Vec::new(),
        proof_storage: usize::MAX,
    };
    assert!(matches!(
        packet.retained_storage(),
        Err(E::Resource(Resource::Arithmetic))
    ));
    assert!(matches!(
        roster_wrapper_storage::<_, PreparedNativeSourceLineageV1>(&packet),
        Err(E::Resource(Resource::Arithmetic))
    ));
}
