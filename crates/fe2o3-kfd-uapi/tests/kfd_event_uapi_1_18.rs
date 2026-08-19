use core::mem::{align_of, size_of};

use fe2o3_kfd_uapi::*;
use sha2::{Digest, Sha256};

#[test]
fn event_schema_composes_without_mutating_frozen_parents() {
    assert_eq!(
        KFD_EVENT_QUEUE_EXCEPTION_SCHEMA_ID,
        "linux-kfd-event-and-queue-exception-1.18-gfx942-v1"
    );
    assert_eq!(
        KFD_EVENT_PARENT_SCHEMA_BINDINGS,
        [
            (
                KFD_UAPI_SCHEMA_ID,
                "e4aad5d8e3177ea6d70298adab7741c377cb091373553ce689f3525e7514d9b4"
            ),
            (
                KFD_MEMORY_LIFECYCLE_SCHEMA_ID,
                "e2d6987b7c8e61a405b2f775d5d004f458a096241459e4cfdf90bd4497f4d58a"
            ),
            (
                KFD_AQL_QUEUE_LIFECYCLE_SCHEMA_ID,
                "b11f3c8c766dd25394350646e35269e10c8a33acb98f74cba2a82e95fa185c4e"
            ),
            (
                KFD_GFX942_QUEUE_RESOURCE_SCHEMA_ID,
                "63753a9c0dcef0f69e0235b95b44fe6ce22cb5b0d1df6f60a971a5ed28f15904"
            ),
        ]
    );
    assert_eq!(
        KFD_UAPI_SCHEMA_MANIFEST_SHA256,
        KFD_EVENT_PARENT_SCHEMA_BINDINGS[0].1
    );
    assert_eq!(
        KFD_MEMORY_LIFECYCLE_SCHEMA_MANIFEST_SHA256,
        KFD_EVENT_PARENT_SCHEMA_BINDINGS[1].1
    );
    assert_eq!(
        KFD_AQL_QUEUE_LIFECYCLE_SCHEMA_MANIFEST_SHA256,
        KFD_EVENT_PARENT_SCHEMA_BINDINGS[2].1
    );
    assert_eq!(
        KFD_GFX942_QUEUE_RESOURCE_SCHEMA_MANIFEST_SHA256,
        KFD_EVENT_PARENT_SCHEMA_BINDINGS[3].1
    );
    for (schema_id, digest) in KFD_EVENT_PARENT_SCHEMA_BINDINGS {
        assert!(KFD_EVENT_QUEUE_EXCEPTION_SCHEMA_MANIFEST.contains(schema_id));
        assert!(KFD_EVENT_QUEUE_EXCEPTION_SCHEMA_MANIFEST.contains(digest));
    }

    let digest = Sha256::digest(KFD_EVENT_QUEUE_EXCEPTION_SCHEMA_MANIFEST);
    assert_eq!(hex(&digest), KFD_EVENT_QUEUE_EXCEPTION_SCHEMA_SHA256);
    assert_eq!(&digest[..], &KFD_EVENT_QUEUE_EXCEPTION_SCHEMA_SHA256_BYTES);
}

#[test]
fn c_layouts_and_ioctl_encodings_are_exact() {
    assert_eq!(
        (
            size_of::<KfdIoctlCreateEventArgsV1>(),
            align_of::<KfdIoctlCreateEventArgsV1>()
        ),
        (32, 8)
    );
    assert_eq!(
        (
            size_of::<KfdIoctlDestroyEventArgsV1>(),
            align_of::<KfdIoctlDestroyEventArgsV1>()
        ),
        (8, 4)
    );
    assert_eq!(
        (
            size_of::<KfdIoctlSetEventArgsV1>(),
            align_of::<KfdIoctlSetEventArgsV1>()
        ),
        (8, 4)
    );
    assert_eq!(
        (
            size_of::<KfdIoctlResetEventArgsV1>(),
            align_of::<KfdIoctlResetEventArgsV1>()
        ),
        (8, 4)
    );
    assert_eq!(
        (
            size_of::<KfdMemoryExceptionFailureV1>(),
            align_of::<KfdMemoryExceptionFailureV1>()
        ),
        (16, 4)
    );
    assert_eq!(
        (
            size_of::<KfdHsaMemoryExceptionDataV1>(),
            align_of::<KfdHsaMemoryExceptionDataV1>()
        ),
        (32, 8)
    );
    assert_eq!(
        (
            size_of::<KfdHsaHardwareExceptionDataV1>(),
            align_of::<KfdHsaHardwareExceptionDataV1>()
        ),
        (16, 4)
    );
    assert_eq!(
        (
            size_of::<KfdHsaSignalEventDataV1>(),
            align_of::<KfdHsaSignalEventDataV1>()
        ),
        (8, 8)
    );
    assert_eq!(
        (
            size_of::<KfdEventPayloadV1>(),
            align_of::<KfdEventPayloadV1>()
        ),
        (32, 8)
    );
    assert_eq!(
        (size_of::<KfdEventDataV1>(), align_of::<KfdEventDataV1>()),
        (48, 8)
    );
    assert_eq!(
        (
            size_of::<KfdIoctlWaitEventsArgsV1>(),
            align_of::<KfdIoctlWaitEventsArgsV1>()
        ),
        (24, 8)
    );
    assert_eq!(
        (
            size_of::<KfdContextSaveAreaHeaderV1>(),
            align_of::<KfdContextSaveAreaHeaderV1>()
        ),
        (40, 8)
    );

    assert_eq!(AMDKFD_IOC_CREATE_EVENT, 0xc020_4b08);
    assert_eq!(AMDKFD_IOC_DESTROY_EVENT, 0x4008_4b09);
    assert_eq!(AMDKFD_IOC_SET_EVENT, 0x4008_4b0a);
    assert_eq!(AMDKFD_IOC_RESET_EVENT, 0x4008_4b0b);
    assert_eq!(AMDKFD_IOC_WAIT_EVENTS, 0xc018_4b0c);
}

#[test]
fn event_types_and_signal_id_domain_fail_closed() {
    for value in 0..=8 {
        assert!(KfdEventTypeV1::from_wire(value).is_some());
    }
    assert_eq!(KfdEventTypeV1::from_wire(9), None);
    assert_eq!(KfdEventTypeV1::from_wire(u32::MAX), None);

    assert_eq!(KfdSignalEventIdV1::new(0), None);
    assert_eq!(KfdSignalEventIdV1::new(1).unwrap().get(), 1);
    assert_eq!(KfdSignalEventIdV1::new(4095).unwrap().get(), 4095);
    assert_eq!(KfdSignalEventIdV1::new(4096), None);
}

fn created_wire(id: u32) -> KfdIoctlCreateEventArgsV1 {
    KfdIoctlCreateEventArgsV1::from_untrusted_wire(
        KFD_EVENT_PAGE_MMAP_OFFSET,
        id,
        KFD_IOC_EVENT_SIGNAL,
        1,
        0,
        id,
        id,
    )
}

#[test]
fn queue_exception_signal_creation_binds_every_input_and_output() {
    let request = KfdIoctlCreateEventArgsV1::new_queue_exception_signal(None);
    assert_eq!(request.event_page_offset(), 0);
    assert_eq!(request.event_trigger_data(), 0);
    assert_eq!(request.event_type(), KFD_IOC_EVENT_SIGNAL);
    assert_eq!(request.auto_reset(), 1);
    assert_eq!(request.node_id(), 0);
    assert_eq!(request.event_id(), 0);
    assert_eq!(request.event_slot_index(), 0);
    let handle = KfdEventPageHandleObservationV1::new(0x1234_5000).unwrap();
    assert_eq!(
        KfdIoctlCreateEventArgsV1::new_queue_exception_signal(Some(handle)).event_page_offset(),
        handle.get()
    );
    assert_eq!(KfdEventPageHandleObservationV1::new(0), None);

    for id in [1, 4095] {
        let created = created_wire(id)
            .admit_queue_exception_signal_output()
            .unwrap();
        assert_eq!(created.id().get(), id);
        assert_eq!(created.trigger_data(), id);
        assert_eq!(created.slot_index(), id);
        assert_eq!(created.event_page_mmap_offset(), KFD_EVENT_PAGE_MMAP_OFFSET);
    }
}

#[test]
fn queue_exception_signal_creation_rejects_hostile_drift() {
    use KfdCreateSignalAdmissionErrorV1 as E;
    assert_eq!(
        KfdIoctlCreateEventArgsV1::from_untrusted_wire(0, 1, 0, 1, 0, 1, 1)
            .admit_queue_exception_signal_output(),
        Err(E::EventPageOffset)
    );
    assert_eq!(
        KfdIoctlCreateEventArgsV1::from_untrusted_wire(
            KFD_EVENT_PAGE_MMAP_OFFSET,
            1,
            1,
            1,
            0,
            1,
            1
        )
        .admit_queue_exception_signal_output(),
        Err(E::EventType)
    );
    assert_eq!(
        KfdIoctlCreateEventArgsV1::from_untrusted_wire(
            KFD_EVENT_PAGE_MMAP_OFFSET,
            1,
            0,
            0,
            0,
            1,
            1
        )
        .admit_queue_exception_signal_output(),
        Err(E::AutoReset)
    );
    assert_eq!(
        KfdIoctlCreateEventArgsV1::from_untrusted_wire(
            KFD_EVENT_PAGE_MMAP_OFFSET,
            1,
            0,
            1,
            1,
            1,
            1
        )
        .admit_queue_exception_signal_output(),
        Err(E::NodeId)
    );
    assert_eq!(
        created_wire(0).admit_queue_exception_signal_output(),
        Err(E::EventId)
    );
    assert_eq!(
        created_wire(4096).admit_queue_exception_signal_output(),
        Err(E::EventId)
    );
    assert_eq!(
        KfdIoctlCreateEventArgsV1::from_untrusted_wire(
            KFD_EVENT_PAGE_MMAP_OFFSET,
            2,
            0,
            1,
            0,
            1,
            1
        )
        .admit_queue_exception_signal_output(),
        Err(E::TriggerData)
    );
    assert_eq!(
        KfdIoctlCreateEventArgsV1::from_untrusted_wire(
            KFD_EVENT_PAGE_MMAP_OFFSET,
            1,
            0,
            1,
            0,
            1,
            2
        )
        .admit_queue_exception_signal_output(),
        Err(E::SlotIndex)
    );
}

#[test]
fn id_requests_and_signal_event_data_zero_reserved_fields() {
    let id = KfdSignalEventIdV1::new(7).unwrap();
    let destroy = KfdIoctlDestroyEventArgsV1::new(id);
    let set = KfdIoctlSetEventArgsV1::new(id);
    let reset = KfdIoctlResetEventArgsV1::new(id);
    assert_eq!((destroy.event_id(), destroy.pad()), (7, 0));
    assert_eq!((set.event_id(), set.pad()), (7, 0));
    assert_eq!((reset.event_id(), reset.pad()), (7, 0));

    let data = KfdEventDataV1::new_signal(id, 23);
    assert_eq!(data.payload().words(), [23, 0, 0, 0]);
    assert_eq!(data.extension_address(), 0);
    assert_eq!(data.event_id(), 7);
    assert_eq!(data.pad(), 0);
}

#[test]
fn opaque_event_data_addresses_are_bounded_and_aligned() {
    assert_eq!(KfdEventDataArrayAddressV1::new(0, 1), None);
    assert_eq!(KfdEventDataArrayAddressV1::new(0x1001, 1), None);
    assert_eq!(KfdEventDataArrayAddressV1::new(0x1000, 0), None);
    assert_eq!(KfdEventDataArrayAddressV1::new(u64::MAX - 7, 1), None);
    assert_eq!(
        KfdEventDataArrayAddressV1::new(0x1000, 1).unwrap().get(),
        0x1000
    );

    assert_eq!(KfdQueueExceptionPayloadAddressV1::new(0), None);
    assert_eq!(KfdQueueExceptionPayloadAddressV1::new(0x1004), None);
    assert_eq!(KfdQueueExceptionPayloadAddressV1::new(u64::MAX - 7), None);
    assert_eq!(
        KfdQueueExceptionPayloadAddressV1::new(0x2000)
            .unwrap()
            .get(),
        0x2000
    );
}

#[test]
fn one_event_wait_admits_only_bound_success_results() {
    let address = KfdEventDataArrayAddressV1::new(0x1000, 1).unwrap();
    let request = KfdIoctlWaitEventsArgsV1::new_one_signal(address, 250);
    assert_eq!(
        (
            request.events_address(),
            request.event_count(),
            request.wait_for_all(),
            request.timeout_ms(),
            request.wait_result()
        ),
        (0x1000, 1, 1, 250, KFD_IOC_WAIT_RESULT_FAIL)
    );

    for (wire, expected) in [
        (0, KfdWaitResultV1::Complete),
        (1, KfdWaitResultV1::Timeout),
    ] {
        let output = KfdIoctlWaitEventsArgsV1::from_untrusted_wire(0x1000, 1, 1, 250, wire);
        assert_eq!(output.admit_successful_result(address, 250), Ok(expected));
    }

    use KfdWaitAdmissionErrorV1 as E;
    assert_eq!(
        KfdIoctlWaitEventsArgsV1::from_untrusted_wire(0x2000, 1, 1, 250, 0)
            .admit_successful_result(address, 250),
        Err(E::EventsAddress)
    );
    assert_eq!(
        KfdIoctlWaitEventsArgsV1::from_untrusted_wire(0x1000, 2, 1, 250, 0)
            .admit_successful_result(address, 250),
        Err(E::EventCount)
    );
    assert_eq!(
        KfdIoctlWaitEventsArgsV1::from_untrusted_wire(0x1000, 1, 0, 250, 0)
            .admit_successful_result(address, 250),
        Err(E::WaitMode)
    );
    assert_eq!(
        KfdIoctlWaitEventsArgsV1::from_untrusted_wire(0x1000, 1, 1, 251, 0)
            .admit_successful_result(address, 250),
        Err(E::Timeout)
    );
    assert_eq!(
        KfdIoctlWaitEventsArgsV1::from_untrusted_wire(0x1000, 1, 1, 250, 2)
            .admit_successful_result(address, 250),
        Err(E::KernelFailure)
    );
    assert_eq!(
        KfdIoctlWaitEventsArgsV1::from_untrusted_wire(0x1000, 1, 1, 250, 3)
            .admit_successful_result(address, 250),
        Err(E::UnknownResult)
    );
}

#[test]
fn context_header_and_queue_reason_are_fail_closed() {
    let payload = KfdQueueExceptionPayloadAddressV1::new(0x2000).unwrap();
    let event = KfdSignalEventIdV1::new(9).unwrap();
    let header =
        KfdContextSaveAreaHeaderV1::new_queue_exception(0x1000, 0x2000, payload, event).unwrap();
    assert_eq!(header.wave_state_words(), [0; 4]);
    assert_eq!(
        (header.debug_offset(), header.debug_size()),
        (0x1000, 0x2000)
    );
    assert_eq!(header.error_payload_address(), 0x2000);
    assert_eq!(header.error_event_id(), 9);
    assert_eq!(header.reserved(), 0);

    assert_eq!(
        KfdContextSaveAreaHeaderV1::new_queue_exception(1, 64, payload, event),
        Err(KfdContextSaveHeaderErrorV1::DebugOffsetAlignment)
    );
    assert_eq!(
        KfdContextSaveAreaHeaderV1::new_queue_exception(64, 1, payload, event),
        Err(KfdContextSaveHeaderErrorV1::DebugSizeAlignment)
    );
    assert_eq!(
        KfdContextSaveAreaHeaderV1::new_queue_exception(u32::MAX - 63, 64, payload, event),
        Err(KfdContextSaveHeaderErrorV1::DebugRangeOverflow)
    );

    assert_eq!(KFD_QUEUE_EXCEPTION_MASK, 0x0000_0000_607f_803f);
    assert!(
        KfdQueueExceptionReasonV1::from_untrusted_wire(0)
            .unwrap()
            .is_empty()
    );
    let all = KfdQueueExceptionReasonV1::from_untrusted_wire(KFD_QUEUE_EXCEPTION_MASK).unwrap();
    for code in KFD_QUEUE_EXCEPTION_CODES {
        let typed = KfdQueueExceptionCodeV1::from_wire(code).unwrap();
        assert!(all.contains_code(code));
        assert!(all.contains(typed));
        assert_eq!(typed.mask(), 1_u64 << (code - 1));
    }
    assert_eq!(KfdQueueExceptionCodeV1::from_wire(7), None);
    assert!(!all.contains_code(0));
    assert!(!all.contains_code(64));
    assert_eq!(KfdQueueExceptionReasonV1::from_untrusted_wire(1 << 6), None);
    assert_eq!(
        KfdQueueExceptionReasonV1::from_untrusted_wire(1 << 63),
        None
    );
}

#[test]
fn event_page_constants_match_signal_event_contract() {
    assert_eq!(KFD_EVENT_PAGE_MMAP_OFFSET, 0x8000_0000_0000_0000);
    assert_eq!(KFD_EVENT_PAGE_SLOT_COUNT, 4096);
    assert_eq!(KFD_EVENT_PAGE_BYTES, 32768);
    assert_eq!(KFD_EVENT_SLOT_UNSIGNALED, u64::MAX);
    assert_eq!(KFD_EVENT_TIMEOUT_IMMEDIATE, 0);
    assert_eq!(KFD_EVENT_TIMEOUT_INFINITE, u32::MAX);
}

fn hex(bytes: &[u8]) -> String {
    use core::fmt::Write;
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut output, "{byte:02x}").unwrap();
    }
    output
}
