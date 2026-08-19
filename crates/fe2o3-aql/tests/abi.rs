use core::mem::{align_of, size_of};
use fe2o3_aql::{
    AMD_SIGNAL_ALIGNMENT_V1, AMD_SIGNAL_BYTES_V1, AQL_DISPATCH_ABI_SCHEMA_MANIFEST_SHA256_BYTES_V1,
    AQL_DISPATCH_ABI_SCHEMA_MANIFEST_SHA256_V1, AQL_DISPATCH_ABI_SCHEMA_MANIFEST_V1,
    AQL_KERNEL_DISPATCH_PACKET_BYTES_V1, AQL_SYSTEM_SCOPED_KERNEL_DISPATCH_HEADER_V1,
    AmdBusyCompletionSignalV1, AqlAddressObservationError, AqlCompletionObservationV1,
    AqlDispatchGeometryV1, AqlDispatchPacketError, AqlGeometryError, AqlKernelDispatchPacketV1,
    ObservedGpuAddressV1,
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
fn exact_unpublished_packet_and_publication_word() {
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

    let packet = prepared.packet();
    assert!(packet.is_unpublished());
    assert_eq!(packet.kernel_object(), 0x1000);
    assert_eq!(packet.kernarg_address(), 0x2080);
    assert_eq!(packet.completion_signal(), 0x3000);
    assert_eq!(prepared.publication_word(), 0x0001_1402);
    assert_eq!(AQL_SYSTEM_SCOPED_KERNEL_DISPATCH_HEADER_V1, 0x1402);
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

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        value.push(char::from(DIGITS[usize::from(*byte >> 4)]));
        value.push(char::from(DIGITS[usize::from(*byte & 0x0f)]));
    }
    value
}
