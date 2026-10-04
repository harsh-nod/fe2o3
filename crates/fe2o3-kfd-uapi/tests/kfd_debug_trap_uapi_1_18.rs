use core::mem::{align_of, size_of};

use fe2o3_kfd_uapi::*;
use sha2::{Digest, Sha256};

#[test]
fn debug_trap_schema_is_additive_and_digest_pinned() {
    assert_eq!(
        KFD_DEBUG_TRAP_SCHEMA_ID_V1,
        "linux-kfd-debug-trap-1.18-x86_64-le-v1"
    );
    assert!(KFD_DEBUG_TRAP_SCHEMA_MANIFEST_V1.contains(KFD_UAPI_SCHEMA_MANIFEST_SHA256));
    assert!(KFD_DEBUG_TRAP_SCHEMA_MANIFEST_V1.contains(KFD_UAPI_SOURCE_HEADER_SHA256));
    assert!(KFD_DEBUG_TRAP_SCHEMA_MANIFEST_V1.contains(KFD_DEBUG_TRAP_DRIVER_SOURCE_SHA256_V1));
    assert_eq!(
        hex(&Sha256::digest(KFD_DEBUG_TRAP_SCHEMA_MANIFEST_V1)),
        KFD_DEBUG_TRAP_SCHEMA_MANIFEST_SHA256_V1
    );
}

#[test]
fn debug_trap_layouts_and_request_are_exact() {
    assert_eq!(
        (
            size_of::<KfdIoctlRuntimeEnableArgsV1>(),
            align_of::<KfdIoctlRuntimeEnableArgsV1>()
        ),
        (16, 8)
    );
    assert_eq!(
        (
            size_of::<KfdRuntimeInfoV1>(),
            align_of::<KfdRuntimeInfoV1>()
        ),
        (16, 8)
    );
    assert_eq!(
        (
            size_of::<KfdDebugQueueSnapshotEntryV1>(),
            align_of::<KfdDebugQueueSnapshotEntryV1>()
        ),
        (64, 8)
    );
    assert_eq!(
        (
            size_of::<KfdDebugDeviceSnapshotEntryV1>(),
            align_of::<KfdDebugDeviceSnapshotEntryV1>()
        ),
        (120, 8)
    );
    assert_eq!(
        (
            size_of::<KfdDebugContextSaveAreaHeaderV1>(),
            align_of::<KfdDebugContextSaveAreaHeaderV1>()
        ),
        (40, 8)
    );
    assert_eq!(
        (
            size_of::<KfdIoctlDebugTrapArgsV1>(),
            align_of::<KfdIoctlDebugTrapArgsV1>()
        ),
        (32, 8)
    );
    assert_eq!(size_of::<KfdIoctlDebugTrapEnableArgsV1>(), 24);
    assert_eq!(size_of::<KfdIoctlDebugTrapSendRuntimeEventArgsV1>(), 16);
    assert_eq!(size_of::<KfdIoctlDebugTrapSetExceptionsArgsV1>(), 8);
    assert_eq!(size_of::<KfdIoctlDebugTrapLaunchOverrideArgsV1>(), 16);
    assert_eq!(size_of::<KfdIoctlDebugTrapLaunchModeArgsV1>(), 8);
    assert_eq!(size_of::<KfdIoctlDebugTrapSuspendQueuesArgsV1>(), 24);
    assert_eq!(size_of::<KfdIoctlDebugTrapResumeQueuesArgsV1>(), 16);
    assert_eq!(size_of::<KfdIoctlDebugTrapSetAddressWatchArgsV1>(), 24);
    assert_eq!(size_of::<KfdIoctlDebugTrapClearAddressWatchArgsV1>(), 8);
    assert_eq!(size_of::<KfdIoctlDebugTrapSetFlagsArgsV1>(), 8);
    assert_eq!(size_of::<KfdIoctlDebugTrapQueryEventArgsV1>(), 16);
    assert_eq!(size_of::<KfdIoctlDebugTrapQueryExceptionInfoArgsV1>(), 24);
    assert_eq!(size_of::<KfdIoctlDebugTrapQueueSnapshotArgsV1>(), 24);
    assert_eq!(size_of::<KfdIoctlDebugTrapDeviceSnapshotArgsV1>(), 24);
    assert_eq!(AMDKFD_IOC_DBG_TRAP, 0xc020_4b26);
    assert_eq!(AMDKFD_IOC_RUNTIME_ENABLE, 0xc010_4b25);
    assert_eq!(KFD_RUNTIME_DEBUG_MODE_ENABLE_MASK_V1, 1);
    assert_eq!(KFD_RUNTIME_DEBUG_MODE_TTMP_SAVE_MASK_V1, 2);
    assert!(
        KfdIoctlRuntimeEnableArgsV1::new_queue_exception_enable().is_exact_queue_exception_enable()
    );
}

#[test]
fn exception_codes_and_masks_fail_closed() {
    for code in [
        1, 2, 3, 4, 5, 6, 16, 17, 18, 19, 20, 21, 22, 23, 30, 31, 32, 33, 34, 35, 36, 48, 49,
    ] {
        let code = KfdDebugTrapExceptionCodeV1::from_wire(code).unwrap();
        assert!(KfdDebugExceptionMaskV1::ALL.contains(code));
    }
    for invalid in [0, 7, 15, 24, 29, 37, 47, 50, u32::MAX] {
        assert_eq!(KfdDebugTrapExceptionCodeV1::from_wire(invalid), None);
    }
    assert!(KfdDebugExceptionMaskV1::new(KFD_DEBUG_TRAP_ALL_EXCEPTION_MASK_V1).is_some());
    assert!(KfdDebugExceptionMaskV1::new(1_u64 << 63).is_none());
    assert!(KfdDebugLaunchOverrideMaskV1::new(KFD_DBG_TRAP_ALL_LAUNCH_OVERRIDE_MASK_V1).is_some());
    assert!(KfdDebugLaunchOverrideMaskV1::new(1 << 29).is_none());
    assert!(KfdDebugTrapFlagsV1::new(3).is_some());
    assert!(KfdDebugTrapFlagsV1::new(4).is_none());
}

#[test]
fn union_free_wire_constructors_match_little_endian_c_fields() {
    let enable = KfdIoctlDebugTrapArgsV1::enable(
        91,
        KfdDebugExceptionMaskV1::from_code(KfdDebugTrapExceptionCodeV1::QueueWaveTrap),
        0x1122_3344_5566_7788,
        16,
        23,
    );
    assert_eq!(enable.pid(), 91);
    assert_eq!(
        enable.operation_raw(),
        KfdDebugTrapOperationV1::Enable as u32
    );
    assert_eq!(
        enable.payload_words(),
        [2, 0x1122_3344_5566_7788, 16 | (23_u64 << 32)]
    );

    let watch = KfdIoctlDebugTrapArgsV1::set_address_watch(
        91,
        0x1000,
        KfdDebugTrapAddressWatchModeV1::All,
        0xffff_f000,
        77,
    );
    assert_eq!(
        watch.payload_words(),
        [0x1000, 3 | (0xffff_f000_u64 << 32), 77]
    );

    let returned = KfdIoctlDebugTrapArgsV1::from_untrusted_wire(
        91,
        KfdDebugTrapOperationV1::QueryDebugEvent as u32,
        [2, 77 | (9_u64 << 32), 0],
    );
    assert_eq!(returned.returned_event_mask().unwrap().bits(), 2);
    assert_eq!(returned.returned_event_gpu_id(), 77);
    assert_eq!(returned.returned_event_queue_id(), 9);
}

#[test]
fn snapshot_constructors_pin_entry_stride() {
    let queues =
        KfdIoctlDebugTrapArgsV1::queue_snapshot(7, KfdDebugExceptionMaskV1::NONE, 0x1000, 4);
    assert_eq!(queues.payload_words(), [0, 0x1000, 4 | (64_u64 << 32)]);
    let devices =
        KfdIoctlDebugTrapArgsV1::device_snapshot(7, KfdDebugExceptionMaskV1::NONE, 0x2000, 8);
    assert_eq!(devices.payload_words(), [0, 0x2000, 8 | (120_u64 << 32)]);
}

fn hex(bytes: &[u8]) -> String {
    use core::fmt::Write;
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut output, "{byte:02x}").unwrap();
    }
    output
}
