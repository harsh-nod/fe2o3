use super::*;
use crate::{RuntimeResourceKindV1 as K, RuntimeResourceVectorV1};

#[derive(Debug, Default)]
pub(super) struct Fault {
    stage: Option<&'static str>,
    payload: Option<Box<u64>>,
    entries: usize,
    total_entries: usize,
    fail_at: usize,
}

impl Fault {
    pub(super) fn enter(&mut self, stage: &'static str) {
        self.total_entries += 1;
        if self.stage == Some(stage) {
            self.entries += 1;
            if self.entries >= self.fail_at
                && let Some(payload) = self.payload.take()
            {
                std::panic::panic_any(payload);
            }
        }
    }
}

fn configured(domain: bool) -> (RuntimeContextV1<MockBackend>, RuntimeDeviceIdV1) {
    let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
    let device = context.devices()[0].id();
    if domain {
        let root = allocation_admission_tests::shared_root(256, 8);
        context
            .configure_allocation_admission_in_domain_v1(device, &root, 256, 8)
            .unwrap();
    } else {
        context
            .configure_allocation_admission_v1(device, 256, 8)
            .unwrap();
    }
    assert!(context.versions.is_none());
    (context, device)
}

fn allocate(
    context: &mut RuntimeContextV1<MockBackend>,
    device: RuntimeDeviceIdV1,
    bytes: u64,
) -> RuntimeAllocationIdV1 {
    context
        .allocate(device, RuntimeMemoryKindV1::DeviceLocal, bytes, 8)
        .unwrap()
}

fn assert_sealed(
    context: &mut RuntimeContextV1<MockBackend>,
    device: RuntimeDeviceIdV1,
    allocations: usize,
) {
    assert!(context.is_terminal());
    let usage = context
        .allocation_admission_usage_v1(device)
        .unwrap()
        .unwrap();
    assert_eq!(
        usage.used,
        RuntimeResourceVectorV1::ZERO
            .with(K::RequestedAllocationBytes, 96)
            .with(K::AllocationRecords, 2)
    );
    assert_eq!(usage.retained_records, 0);
    assert_eq!(usage.quarantined_records, 2);
    assert_eq!(context.allocations.len(), allocations);
    let calls = context.backend.accounted_fault.total_entries;
    let cleanup = context.backend.cleanup_log.clone();
    assert!(context.create_stream(device).is_err());
    assert!(
        context
            .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 8, 8)
            .is_err()
    );
    assert!(!context.cleanup().is_complete());
    assert_eq!(context.backend.accounted_fault.total_entries, calls);
    assert_eq!(context.backend.cleanup_log, cleanup);
}

#[test]
fn accounted_fail_stop_public_effect_panics_preserve_payload_and_all_local_credits() {
    for domain in [false, true] {
        for stage in [
            "create-stream",
            "destroy-stream",
            "allocate",
            "release-allocation",
            "write",
            "read",
            "load-module",
            "unload-module",
            "resolve-kernel",
            "submit",
            "poll",
            "wait",
            "release-submission",
            "record-event",
            "release-event",
            "copy",
            "peer-copy",
            "cancel",
            "flush",
        ] {
            let (mut context, device) = configured(domain);
            let other = context.devices()[1].id();
            let first = allocate(&mut context, device, 64);
            let second = allocate(&mut context, device, 32);
            let unconfigured = allocate(&mut context, other, 32);
            let stream = context.create_stream(device).unwrap();
            let module = context.load_module(device, b"object").unwrap();
            let kernel = context
                .resolve_kernel::<AddArguments>(module, "add")
                .unwrap();
            let arguments = AddArguments {
                allocation: first,
                scalar: 1,
            };
            let mut submission = context
                .launch(stream, &kernel, &arguments, geometry(), &[])
                .unwrap();
            let event = context.record_event(&submission).unwrap();
            if stage == "release-submission" {
                context
                    .wait(&mut submission, Duration::from_secs(1))
                    .unwrap();
                context.release_event(event).unwrap();
            }
            let payload = Box::new(0x1234_u64);
            let identity = &*payload as *const u64;
            context.backend.accounted_fault = Fault {
                stage: Some(stage),
                payload: Some(payload),
                entries: 0,
                ..Fault::default()
            };
            let region = |allocation, access| RuntimeMemoryRegionV1 {
                allocation,
                access,
                byte_offset: 0,
                byte_len: 16,
            };
            let result = catch_unwind(AssertUnwindSafe(|| {
                match stage {
                    "create-stream" => {
                        context.create_stream(other).unwrap();
                    }
                    "destroy-stream" => {
                        context.destroy_stream(stream).unwrap();
                    }
                    // Selected device/ID has no account; another device still owns credits.
                    "allocate" => {
                        allocate(&mut context, other, 8);
                    }
                    "release-allocation" => {
                        context.release_allocation(unconfigured).unwrap();
                    }
                    "write" => {
                        context.write_allocation(unconfigured, 0, &[1]).unwrap();
                    }
                    "read" => {
                        context.read_allocation(unconfigured, 0, &mut [0]).unwrap();
                    }
                    "load-module" => {
                        context.load_module(other, b"object").unwrap();
                    }
                    "unload-module" => {
                        context.unload_module(module).unwrap();
                    }
                    "resolve-kernel" => {
                        context
                            .resolve_kernel::<AddArguments>(module, "next")
                            .unwrap();
                    }
                    "submit" => {
                        context
                            .launch(stream, &kernel, &arguments, geometry(), &[])
                            .unwrap();
                    }
                    "poll" => {
                        context.poll(&mut submission).unwrap();
                    }
                    "wait" => {
                        context
                            .wait(&mut submission, Duration::from_secs(1))
                            .unwrap();
                    }
                    "release-submission" => {
                        context.release_submission(submission).unwrap();
                    }
                    "record-event" => {
                        context.record_event(&submission).unwrap();
                    }
                    "release-event" => {
                        context.release_event(event).unwrap();
                    }
                    "copy" => {
                        context
                            .copy_async(
                                stream,
                                region(first, RuntimeAccessV1::Read),
                                region(second, RuntimeAccessV1::Write),
                                &[],
                            )
                            .unwrap();
                    }
                    "peer-copy" => {
                        context
                            .peer_copy(
                                stream,
                                region(unconfigured, RuntimeAccessV1::Read),
                                region(second, RuntimeAccessV1::Write),
                                &[],
                            )
                            .unwrap();
                    }
                    "cancel" => {
                        context.cancel(&mut submission).unwrap();
                    }
                    "flush" => {
                        context.flush_stream(stream).unwrap();
                    }
                    _ => unreachable!(),
                }
            }));
            assert_eq!(
                context.backend.accounted_fault.entries, 1,
                "{domain}/{stage}"
            );
            let payload = result.expect_err(stage).downcast::<Box<u64>>().unwrap();
            assert_eq!(&**payload as *const u64, identity, "{domain}/{stage}");
            assert_sealed(&mut context, device, 3);
            assert_eq!(context.backend.memory.len(), 3);
        }
    }
}

#[test]
fn accounted_fail_stop_empty_accounts_guard_metadata_unwind_and_future_entry() {
    for domain in [false, true] {
        let (mut context, device) = configured(domain);
        let result = catch_unwind(AssertUnwindSafe(|| {
            context.guard_journal_unwind_v1(|_| panic!("metadata"))
        }));
        assert!(result.is_err());
        assert!(context.is_terminal());
        assert!(context.create_stream(device).is_err());
        assert_eq!(context.backend.next, 0);
    }
}

#[test]
fn accounted_fail_stop_terminal_and_protocol_errors_quarantine_without_journal() {
    for domain in [false, true] {
        for protocol in [false, true] {
            let (mut context, device) = configured(domain);
            allocate(&mut context, device, 64);
            allocate(&mut context, device, 32);
            if protocol {
                context.backend.handle_override = Some((MockHandleKind::Stream, 0));
                assert!(matches!(
                    context.create_stream(device),
                    Err(RuntimeErrorV1::BackendProtocol(_))
                ));
                assert_eq!(context.streams.len(), 1);
            } else {
                let stream = context.create_stream(device).unwrap();
                context.backend.flush_failure = MockFlushFailure::Terminal;
                assert!(matches!(
                    context.flush_stream(stream),
                    Err(RuntimeErrorV1::BackendTerminal(_))
                ));
            }
            assert_sealed(&mut context, device, 2);
        }
    }
}

#[test]
fn accounted_fail_stop_nonterminal_rejection_and_quiescent_disposal_remain_retryable() {
    for domain in [false, true] {
        for failure in [MockMemoryFailure::Rejected, MockMemoryFailure::Quiescent] {
            let (mut context, device) = configured(domain);
            let first = allocate(&mut context, device, 64);
            let second = allocate(&mut context, device, 32);
            let before = context
                .allocation_admission_usage_v1(device)
                .unwrap()
                .unwrap();
            context.backend.release_allocation_failure = failure;
            assert!(context.release_allocation(first).is_err());
            assert!(!context.is_terminal());
            assert_eq!(
                context
                    .allocation_admission_usage_v1(device)
                    .unwrap()
                    .unwrap(),
                before
            );
            context.release_allocation(first).unwrap();
            context.release_allocation(second).unwrap();
            assert!(context.cleanup().is_complete());
        }
    }
}

#[test]
fn accounted_fail_stop_no_live_credits_still_guards_public_entry_but_legacy_is_unchanged() {
    for domain in [false, true] {
        for disposed in [false, true] {
            let (mut context, device) = configured(domain);
            if disposed {
                let allocation = allocate(&mut context, device, 8);
                context.release_allocation(allocation).unwrap();
            }
            context.backend.accounted_fault = Fault {
                stage: Some("create-stream"),
                payload: Some(Box::new(1)),
                ..Fault::default()
            };
            assert!(catch_unwind(AssertUnwindSafe(|| context.create_stream(device))).is_err());
            assert!(context.is_terminal());
            assert_eq!(context.backend.accounted_fault.entries, 1);
            assert!(context.create_stream(device).is_err());
            assert_eq!(context.backend.accounted_fault.total_entries, 1);
            let usage = context
                .allocation_admission_usage_v1(device)
                .unwrap()
                .unwrap();
            assert_eq!(usage.used, RuntimeResourceVectorV1::ZERO);
        }
    }
    let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
    let device = context.devices()[0].id();
    context.backend.accounted_fault = Fault {
        stage: Some("create-stream"),
        payload: Some(Box::new(1)),
        ..Fault::default()
    };
    assert!(catch_unwind(AssertUnwindSafe(|| context.create_stream(device))).is_err());
    assert!(!context.is_terminal());
    context.create_stream(device).unwrap();
    assert_eq!(context.backend.accounted_fault.entries, 2);
    assert!(context.cleanup().is_complete());
}

#[test]
fn accounted_fail_stop_cleanup_refunds_only_confirmed_disposal_before_panic() {
    for domain in [false, true] {
        let (mut context, device) = configured(domain);
        let disposed = allocate(&mut context, device, 16);
        let first = allocate(&mut context, device, 64);
        let second = allocate(&mut context, device, 32);
        let disposed_handle = context.allocations[&disposed].backend_allocation;
        let first_handle = context.allocations[&first].backend_allocation;
        let second_handle = context.allocations[&second].backend_allocation;
        let payload = Box::new(17);
        let identity = &*payload as *const u64;
        context.backend.accounted_fault = Fault {
            stage: Some("release-allocation"),
            payload: Some(payload),
            fail_at: 2,
            ..Fault::default()
        };
        let result = catch_unwind(AssertUnwindSafe(|| context.cleanup()));
        let payload = result.unwrap_err().downcast::<Box<u64>>().unwrap();
        assert_eq!(&**payload as *const u64, identity);
        assert_eq!(context.backend.accounted_fault.entries, 2);
        assert!(!context.allocations.contains_key(&disposed));
        assert!(!context.backend.memory.contains_key(&disposed_handle));
        assert!(context.backend.memory.contains_key(&first_handle));
        assert!(context.backend.memory.contains_key(&second_handle));
        assert_sealed(&mut context, device, 2);
    }
}

#[test]
fn accounted_fail_stop_quarantine_leaves_unattached_external_credits_unchanged() {
    for domain in [false, true] {
        let (mut context, device) = configured(domain);
        allocate(&mut context, device, 64);
        allocate(&mut context, device, 32);
        let retained = context
            .allocation_admission
            .reserve(device, 8)
            .unwrap()
            .unwrap();
        let reserved = context
            .allocation_admission
            .prepare_roster(device, &[16])
            .unwrap()
            .unwrap();
        context.backend.accounted_fault = Fault {
            stage: Some("create-stream"),
            payload: Some(Box::new(1)),
            ..Fault::default()
        };
        assert!(catch_unwind(AssertUnwindSafe(|| context.create_stream(device))).is_err());
        let usage = context
            .allocation_admission_usage_v1(device)
            .unwrap()
            .unwrap();
        assert_eq!(
            usage.used,
            RuntimeResourceVectorV1::ZERO
                .with(K::RequestedAllocationBytes, 120)
                .with(K::AllocationRecords, 4)
        );
        assert_eq!(usage.reserved_records, 1);
        assert_eq!(usage.retained_records, 1);
        assert_eq!(usage.quarantined_records, 2);
        retained.release_after_rejection().unwrap();
        drop(reserved);
        assert_sealed(&mut context, device, 2);
    }
}

impl RuntimeOwnedShutdownBackendV1 for MockBackend {
    fn shutdown_owned_v1(&mut self) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.accounted_fault.enter("owned-shutdown");
        mock_memory_failure_v1(core::mem::take(&mut self.shutdown_failure))
    }
}

#[test]
fn accounted_fail_stop_owned_shutdown_seals_only_uncertain_terminal_outcomes() {
    for domain in [false, true] {
        for failure in [
            MockMemoryFailure::Panic,
            MockMemoryFailure::Terminal,
            MockMemoryFailure::Rejected,
            MockMemoryFailure::Quiescent,
        ] {
            let (mut context, device) = configured(domain);
            let allocation = allocate(&mut context, device, 8);
            context.release_allocation(allocation).unwrap();
            assert!(context.cleanup().is_complete());
            context.backend.shutdown_failure = failure;
            let result = catch_unwind(AssertUnwindSafe(|| context.shutdown_owned_backend_v1()));
            if failure == MockMemoryFailure::Panic {
                assert!(result.is_err());
            } else {
                let error = result.unwrap().unwrap_err();
                assert!(matches!(
                    (failure, error),
                    (
                        MockMemoryFailure::Terminal,
                        RuntimeBackendFailureV1::Terminal(_)
                    ) | (
                        MockMemoryFailure::Rejected,
                        RuntimeBackendFailureV1::Rejected(_)
                    ) | (
                        MockMemoryFailure::Quiescent,
                        RuntimeBackendFailureV1::Quiescent(_)
                    )
                ));
            }
            assert_eq!(
                context.is_terminal(),
                matches!(
                    failure,
                    MockMemoryFailure::Panic | MockMemoryFailure::Terminal
                )
            );
            let usage = context
                .allocation_admission_usage_v1(device)
                .unwrap()
                .unwrap();
            assert_eq!(usage.used, RuntimeResourceVectorV1::ZERO);
        }
    }
}

#[test]
fn accounted_fail_stop_uncertain_allocation_retains_attempt_and_previous_handles() {
    for domain in [false, true] {
        for failure in [MockMemoryFailure::Panic, MockMemoryFailure::Terminal] {
            let (mut context, device) = configured(domain);
            allocate(&mut context, device, 64);
            allocate(&mut context, device, 32);
            context.backend.allocation_failure = failure;
            let result = catch_unwind(AssertUnwindSafe(|| {
                context.allocate(device, RuntimeMemoryKindV1::DeviceLocal, 8, 8)
            }));
            if failure == MockMemoryFailure::Panic {
                assert!(result.is_err());
            } else {
                assert!(matches!(
                    result.unwrap(),
                    Err(RuntimeErrorV1::BackendTerminal(_))
                ));
            }
            assert!(context.is_terminal());
            let usage = context
                .allocation_admission_usage_v1(device)
                .unwrap()
                .unwrap();
            assert_eq!(
                usage.used,
                RuntimeResourceVectorV1::ZERO
                    .with(K::RequestedAllocationBytes, 104)
                    .with(K::AllocationRecords, 3)
            );
            assert_eq!(usage.quarantined_records, 3);
            assert_eq!(usage.retained_records, 0);
            assert_eq!(context.allocations.len(), 2);
            assert_eq!(context.backend.memory.len(), 3);
        }
    }
}
