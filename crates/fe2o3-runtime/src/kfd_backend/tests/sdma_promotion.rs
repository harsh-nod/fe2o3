use super::*;

#[test]
fn scripted_sdma_cross_driver_and_mixed_pair_mismatches_retain_without_consuming_fifo() {
    let mut left =
        ScriptedSdmaDriverV1::new([ScriptedSdmaStepV1::Promote(ScriptedFailureModeV1::Success)]);
    let right = ScriptedSdmaDriverV1::new([]);
    let foreign_buffer = right.test_host_owner(8);
    let failure = DirectionalSdmaOpsV1::Scripted(&mut left)
        .promote(foreign_buffer)
        .unwrap_err();
    assert!(matches!(
        &failure,
        SdmaTransitionFailureV1::ProcessTeardown {
            custody: SdmaTerminalCustodyV1::Scripted(_),
            ..
        }
    ));
    assert_eq!(left.remaining_steps(), 1);
    assert_eq!(left.live_owner_count(), 0);
    assert_eq!(right.live_owner_count(), 1);
    assert_eq!(right.unexpected_drops(), 0);

    let mut pair_driver = ScriptedSdmaDriverV1::new([scripted_submit_step_v1(
        Gfx942PersistentSdmaDirectionV1::HostToDevice,
        0,
        0,
        8,
        ScriptedFailureModeV1::Success,
    )]);
    let foreign_driver = ScriptedSdmaDriverV1::new([]);
    let host = pair_driver.test_host_owner(8);
    let device = foreign_driver.test_device_owner(8);
    let (device, host) = match (device, host) {
        (DirectionalSdmaDeviceOwnerV1::Scripted(device), SdmaBufferOwnerV1::Scripted(host)) => {
            (device, host)
        }
        _ => unreachable!("scripted factories return scripted owners"),
    };
    let failure = DirectionalSdmaOpsV1::Scripted(&mut pair_driver)
        .submit(
            DirectionalSdmaPairOwnerV1 {
                device: DirectionalSdmaDeviceOwnerV1::Scripted(device),
                host: SdmaBufferOwnerV1::Scripted(host),
            },
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            DirectionalSdmaRequestPlanV1::Single(DirectionalSdmaCopyRequestV1 {
                host_offset: 0,
                device_offset: 0,
                copy_bytes: 8,
            }),
        )
        .unwrap_err();
    assert!(matches!(
        &failure,
        SdmaTransitionFailureV1::ProcessTeardown {
            custody: SdmaTerminalCustodyV1::Scripted(_),
            ..
        }
    ));
    assert_eq!(pair_driver.remaining_steps(), 1);
    assert_eq!(pair_driver.live_owner_count(), 1);
    assert_eq!(foreign_driver.live_owner_count(), 1);
    assert_eq!(pair_driver.unexpected_drops(), 0);
    assert_eq!(foreign_driver.unexpected_drops(), 0);
}

#[test]
fn scripted_sdma_drop_still_aborts_with_live_or_terminal_custody() {
    use std::os::unix::process::ExitStatusExt;

    const CHILD: &str = "FE2O3_TEST_SCRIPTED_SDMA_ABORT_CHILD";
    if let Some(case) = std::env::var_os(CHILD) {
        if case == "live" {
            let (backend, _, _, _) = scripted_direct_backend_v1(8, []);
            drop(backend);
            std::process::exit(97);
        }
        let mut backend = KfdRuntimeBackendV1::mock();
        backend.scripted_sdma = Some(ScriptedSdmaDriverV1::new([]));
        backend.terminal = true;
        drop(backend);
        std::process::exit(97);
    }
    for case in ["terminal", "live"] {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "kfd_backend::tests::sdma_promotion::scripted_sdma_drop_still_aborts_with_live_or_terminal_custody",
                    "--nocapture",
                ])
                .env(CHILD, case)
                .status()
                .unwrap();
        assert_eq!(
            status.signal(),
            Some(6),
            "scripted Drop case {case} did not terminate through SIGABRT"
        );
    }
}

#[test]
fn ready_promotion_observation_is_monotonic_and_exhaustion_is_nonmutating() {
    let mut backend = KfdRuntimeBackendV1::mock();
    let role = Gfx942DeviceContentRoleV1::new([0x26; 32], 7).unwrap();
    let content = Gfx942DeviceContentDescriptorV1::new(role, 4096, [0x37; 32]).unwrap();
    let first = backend
        .observe_ready_promotion_performance_v1(content, Duration::from_nanos(11))
        .unwrap();
    let second = backend
        .observe_ready_promotion_performance_v1(content, Duration::from_nanos(13))
        .unwrap();
    assert_eq!(first.ordinal(), 0);
    assert_eq!(second.ordinal(), 1);
    assert_eq!(second.content_ordinal(), 7);
    assert_eq!(second.authenticated_bytes(), 4096);
    assert_eq!(second.authentication(), Duration::from_nanos(13));
    assert_eq!(backend.last_ready_promotion_performance_v1(), Some(second));

    backend.next_ready_promotion_ordinal = None;
    assert_eq!(
        backend.observe_ready_promotion_performance_v1(content, Duration::from_nanos(17)),
        None
    );
    assert_eq!(backend.last_ready_promotion_performance_v1(), Some(second));
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn scripted_sdma_promotion_retry_and_teardown_preserve_exact_custody() {
    let retry_steps = [
        ScriptedSdmaStepV1::Allocate {
            kind: ScriptedBufferKindV1::Device,
            byte_len: 8,
        },
        ScriptedSdmaStepV1::Promote(ScriptedFailureModeV1::Retryable),
        ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
    ];
    let mut retry = KfdRuntimeBackendV1::mock();
    retry.native_available = true;
    retry.scripted_sdma = Some(ScriptedSdmaDriverV1::new(retry_steps));
    assert!(matches!(
        retry.allocate_v1(7, RuntimeMemoryKindV1::DeviceLocal, 8, 8),
        Err(RuntimeBackendFailureV1::Quiescent(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Native
    ));
    assert!(retry.allocations.is_empty());
    let driver = retry.scripted_sdma.as_ref().unwrap();
    assert!(driver.is_exhausted());
    assert_eq!(driver.live_owner_count(), 0);
    assert_eq!(driver.unexpected_drops(), 0);
    retry.shutdown_native_v1().unwrap();

    let teardown_steps = [
        ScriptedSdmaStepV1::Allocate {
            kind: ScriptedBufferKindV1::Device,
            byte_len: 8,
        },
        ScriptedSdmaStepV1::Promote(ScriptedFailureModeV1::ProcessTeardown),
    ];
    let mut teardown = KfdRuntimeBackendV1::mock();
    teardown.native_available = true;
    teardown.scripted_sdma = Some(ScriptedSdmaDriverV1::new(teardown_steps));
    assert!(matches!(
        teardown.allocate_v1(7, RuntimeMemoryKindV1::DeviceLocal, 8, 8),
        Err(RuntimeBackendFailureV1::Terminal(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Terminal
    ));
    assert!(teardown.terminal);
    assert!(teardown.terminal_sdma_custody.is_some());
    let driver = teardown.scripted_sdma.as_ref().unwrap();
    assert!(driver.is_exhausted());
    assert_eq!(driver.live_owner_count(), 1);
    assert_eq!(driver.unexpected_drops(), 0);
    disarm_scripted_drop_after_inspection_v1(&mut teardown);
}

#[test]
fn scripted_sdma_demotion_and_recycle_recovery_are_retryable_without_loss() {
    let steps = [
        ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Retryable),
        ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
        ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Recovered),
        ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
    ];
    let (mut backend, stream, host, device) = scripted_direct_backend_v1(8, steps);
    backend.allocations.get_mut(&device).unwrap().sdma_backed = false;
    assert!(matches!(
        backend.release_allocation_v1(device),
        Err(RuntimeBackendFailureV1::Quiescent(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Native
    ));
    assert!(matches!(
        backend.allocations[&device].sdma_storage,
        KfdRuntimeSdmaStorageV1::Device(_)
    ));
    assert!(matches!(
        backend.release_allocation_v1(device),
        Err(RuntimeBackendFailureV1::Quiescent(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Native
    ));
    assert!(matches!(
        backend.allocations[&device].sdma_storage,
        KfdRuntimeSdmaStorageV1::DemotedDevice(_)
    ));
    backend.release_allocation_v1(device).unwrap();
    backend.release_allocation_v1(host).unwrap_err();
    // The final host recycle was deliberately not scripted: mismatch is
    // terminal and exact host custody is retained instead of disappearing.
    assert!(backend.terminal);
    assert!(backend.terminal_sdma_custody.is_some());
    assert_eq!(
        backend.scripted_sdma.as_ref().unwrap().live_owner_count(),
        1
    );
    let _ = stream;
    disarm_scripted_drop_after_inspection_v1(&mut backend);
}

#[test]
fn scripted_sdma_initial_submit_retry_is_conclusive_and_releasable() {
    let mut steps = vec![scripted_submit_step_v1(
        Gfx942PersistentSdmaDirectionV1::HostToDevice,
        0,
        0,
        8,
        ScriptedFailureModeV1::Retryable,
    )];
    steps.extend(scripted_release_steps_v1());
    let (mut backend, stream, host, device) = scripted_direct_backend_v1(8, steps);
    let (source, destination) = scripted_copy_regions_v1(host, device, 8);
    let submission = backend
        .copy_async_v1(stream, source, destination, &[])
        .unwrap();
    assert_eq!(
        backend.poll_v1(submission).unwrap(),
        BackendPollV1::Failed {
            code: COOPERATIVE_COPY_FAILURE_CODE_V1
        }
    );
    assert!(!backend.quiescent_sdma_submissions.contains(&submission));
    assert!(backend.active_sdma.is_empty());
    clean_scripted_direct_backend_v1(&mut backend, stream, host, device, Some(submission));
}

#[test]
fn scripted_full_h2d_seals_and_normalizes_same_device_identity_without_copy() {
    let byte_len = usize::try_from(HOST_VISIBLE_MEMORY_PAGE_BYTES_V1).unwrap();
    let mut steps = vec![
        ScriptedSdmaStepV1::Write {
            offset: 0,
            byte_len,
        },
        scripted_submit_step_v1(
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            0,
            0,
            u32::try_from(byte_len).unwrap(),
            ScriptedFailureModeV1::Success,
        ),
        ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
            direction: None,
            copy_bytes: None,
        }),
    ];
    steps.extend(scripted_release_steps_v1());
    let (mut backend, stream, host, device) = scripted_direct_backend_v1(byte_len, steps);
    let expected = vec![0x5a_u8; byte_len];
    backend.write_allocation_v1(host, 0, &expected).unwrap();
    let (source, destination) =
        scripted_copy_regions_v1(host, device, u64::try_from(byte_len).unwrap());
    let submission = backend
        .copy_async_v1(stream, source, destination, &[])
        .unwrap();
    assert_eq!(
        backend.poll_v1(submission).unwrap(),
        BackendPollV1::Succeeded
    );
    let destination = &backend.allocations[&device];
    let ready = destination
        .sdma_storage
        .persistent_compute_ready_facts_v1()
        .expect("full H2D must retain authenticated ready custody");
    assert_eq!(ready.logical_bytes, u64::try_from(byte_len).unwrap());
    assert_eq!(ready.physical_bytes, ready.logical_bytes);
    let expected_sha256: [u8; 32] = Sha256::digest(&expected).into();
    assert_eq!(ready.authenticated_sha256, expected_sha256);
    let promotion = backend.last_ready_promotion_performance_v1().unwrap();
    assert_eq!(promotion.ordinal(), 0);
    assert_eq!(promotion.content_ordinal(), 0);
    assert_eq!(promotion.authenticated_bytes(), ready.logical_bytes);
    assert_eq!(
        destination.sdma_storage.ready_promotion_performance_v1(),
        Some(promotion)
    );
    assert_eq!(destination.bytes.as_ref(), expected);
    assert_eq!(
        destination.content_sha256,
        Some(Sha256::digest(&expected).into())
    );
    assert!(!destination.sdma_shadow_dirty);
    assert_eq!(
        backend.scripted_sdma.as_ref().unwrap().live_owner_count(),
        2
    );

    let original_box = match &backend.allocations[&device].sdma_storage {
        KfdRuntimeSdmaStorageV1::H2dReady(ready) => ready.as_ref() as *const _ as usize,
        _ => unreachable!(),
    };
    let admission = PersistentFullRangeComputeAdmissionV1 {
        allocation: device,
        access: RuntimeAccessV1::Read,
        source: PersistentFullRangeComputeSourceV1::AuthenticatedH2d,
    };
    let (taken, extraction_allocations) =
        counted_allocations_for_test_v1(|| backend.take_persistent_compute_input_v1(admission, 73));
    let (ready_owner, restoration) = taken.unwrap();
    assert!(matches!(
        backend.allocations[&device].sdma_storage,
        KfdRuntimeSdmaStorageV1::ComputeInFlight(73)
    ));
    assert_eq!(
        backend.scripted_sdma.as_ref().unwrap().live_owner_count(),
        2
    );
    let (restored, restoration_allocations) = counted_allocations_for_test_v1(|| {
        backend.restore_persistent_bind_input_v1(ready_owner, restoration)
    });
    restored.unwrap();
    assert_eq!(extraction_allocations, 0);
    assert!(matches!(&backend.allocations[&device].sdma_storage,
            KfdRuntimeSdmaStorageV1::H2dReady(ready) if ready.as_ref() as *const _ as usize == original_box));
    assert_eq!(
        backend.allocations[&device]
            .sdma_storage
            .persistent_compute_ready_facts_v1(),
        Some(ready)
    );
    assert_eq!(
        backend.allocations[&device]
            .sdma_storage
            .ready_promotion_performance_v1(),
        Some(promotion)
    );

    backend.normalize_h2d_ready_v1(device).unwrap();
    let KfdRuntimeSdmaStorageV1::Device(owner) = &backend.allocations[&device].sdma_storage else {
        panic!("normalization must restore directional device custody")
    };
    assert_eq!(owner.scripted_bytes().unwrap(), expected);
    assert_eq!(
        backend.scripted_sdma.as_ref().unwrap().live_owner_count(),
        2
    );
    clean_scripted_direct_backend_v1(&mut backend, stream, host, device, Some(submission));
    assert_eq!(
        restoration_allocations, 0,
        "bind rejection restoration must not allocate"
    );
}

#[test]
fn scripted_recovered_ready_promotion_preserves_prior_observation_and_h2d_success() {
    let byte_len = usize::try_from(HOST_VISIBLE_MEMORY_PAGE_BYTES_V1).unwrap();
    let mut steps = vec![
        ScriptedSdmaStepV1::Write {
            offset: 0,
            byte_len,
        },
        scripted_submit_step_v1(
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            0,
            0,
            u32::try_from(byte_len).unwrap(),
            ScriptedFailureModeV1::Success,
        ),
        ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
            direction: None,
            copy_bytes: None,
        }),
        ScriptedSdmaStepV1::PromoteComputeReady(ScriptedFailureModeV1::Retryable),
    ];
    steps.extend(scripted_release_steps_v1());
    let (mut backend, stream, host, device) = scripted_direct_backend_v1(byte_len, steps);
    let prior_promotion = ready_promotion_observation_v1(60);
    backend.last_ready_promotion_performance = Some(prior_promotion);
    backend.next_ready_promotion_ordinal = Some(61);
    let expected = vec![0x4d; byte_len];
    backend.write_allocation_v1(host, 0, &expected).unwrap();
    let (source, destination) =
        scripted_copy_regions_v1(host, device, u64::try_from(byte_len).unwrap());
    let submission = backend
        .copy_async_v1(stream, source, destination, &[])
        .unwrap();

    assert_eq!(
        backend.poll_v1(submission).unwrap(),
        BackendPollV1::Succeeded
    );
    assert_eq!(
        backend.last_ready_promotion_performance_v1(),
        Some(prior_promotion)
    );
    assert_eq!(backend.next_ready_promotion_ordinal, Some(61));
    let KfdRuntimeSdmaStorageV1::Device(device_owner) = &backend.allocations[&device].sdma_storage
    else {
        panic!("recovered promotion must restore physical device custody")
    };
    assert_eq!(device_owner.scripted_bytes().unwrap(), expected);
    clean_scripted_direct_backend_v1(&mut backend, stream, host, device, Some(submission));
}

#[test]
fn scripted_terminal_before_ready_promotion_retains_completed_custody() {
    let byte_len = usize::try_from(HOST_VISIBLE_MEMORY_PAGE_BYTES_V1).unwrap();
    let steps = [
        ScriptedSdmaStepV1::Write {
            offset: 0,
            byte_len,
        },
        scripted_submit_step_v1(
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            0,
            0,
            u32::try_from(byte_len).unwrap(),
            ScriptedFailureModeV1::Success,
        ),
        ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
            direction: None,
            copy_bytes: None,
        }),
        ScriptedSdmaStepV1::PromoteComputeReady(ScriptedFailureModeV1::ProcessTeardown),
    ];
    let (mut backend, stream, host, device) = scripted_direct_backend_v1(byte_len, steps);
    let prior_promotion = ready_promotion_observation_v1(40);
    backend.last_ready_promotion_performance = Some(prior_promotion);
    backend.next_ready_promotion_ordinal = Some(41);
    backend
        .write_allocation_v1(host, 0, &vec![0x6b; byte_len])
        .unwrap();
    let (source, destination) =
        scripted_copy_regions_v1(host, device, u64::try_from(byte_len).unwrap());
    let submission = backend
        .copy_async_v1(stream, source, destination, &[])
        .unwrap();
    assert_eq!(backend.published_sdma_submissions, [submission]);
    assert!(backend.published_sdma_index_is_consistent_v1());

    assert!(matches!(
        backend.poll_v1(submission),
        Err(RuntimeBackendFailureV1::Terminal(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Terminal
    ));
    assert!(backend.terminal);
    assert!(backend.terminal_sdma_custody.is_some());
    assert_eq!(
        backend.last_ready_promotion_performance_v1(),
        Some(prior_promotion)
    );
    assert_eq!(backend.next_ready_promotion_ordinal, Some(41));
    assert!(matches!(
        backend.active_sdma[&submission].phase,
        ActiveSdmaPhaseV1::Quarantined
    ));
    assert!(backend.published_sdma_submissions.is_empty());
    assert!(backend.published_sdma_index_is_consistent_v1());
    assert!(matches!(
        backend.allocations[&host].sdma_storage,
        KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Async(actual))
            if actual == submission
    ));
    assert!(matches!(
        backend.allocations[&device].sdma_storage,
        KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Async(actual))
            if actual == submission
    ));
    let driver = backend.scripted_sdma.as_ref().unwrap();
    assert!(driver.is_exhausted());
    assert_eq!(driver.live_owner_count(), 2);
    assert_eq!(driver.unexpected_drops(), 0);
    disarm_scripted_drop_after_inspection_v1(&mut backend);
}

#[test]
fn scripted_terminal_receiver_retains_exact_foreign_ready_receipt() {
    let byte_len = usize::try_from(HOST_VISIBLE_MEMORY_PAGE_BYTES_V1).unwrap();
    let steps = [
        ScriptedSdmaStepV1::Write {
            offset: 0,
            byte_len,
        },
        scripted_submit_step_v1(
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            0,
            0,
            u32::try_from(byte_len).unwrap(),
            ScriptedFailureModeV1::Success,
        ),
        ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
            direction: None,
            copy_bytes: None,
        }),
        ScriptedSdmaStepV1::PromoteComputeReadyForeignQueueTerminal,
    ];
    let (mut backend, stream, host, device) = scripted_direct_backend_v1(byte_len, steps);
    backend
        .write_allocation_v1(host, 0, &vec![0x71; byte_len])
        .unwrap();
    let (source, destination) =
        scripted_copy_regions_v1(host, device, u64::try_from(byte_len).unwrap());
    let submission = backend
        .copy_async_v1(stream, source, destination, &[])
        .unwrap();

    assert!(matches!(
        backend.poll_v1(submission),
        Err(RuntimeBackendFailureV1::Terminal(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Terminal
    ));
    assert!(backend.terminal);
    assert!(matches!(
        backend.terminal_sdma_custody,
        Some(KfdRuntimeTerminalSdmaCustodyV1::Completed(
            DirectionalSdmaCompletedOwnerV1::Scripted(_)
        ))
    ));
    assert!(matches!(
        backend.active_sdma[&submission].phase,
        ActiveSdmaPhaseV1::Quarantined
    ));
    assert!(backend.published_sdma_submissions.is_empty());
    assert!(matches!(
        backend.allocations[&host].sdma_storage,
        KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Async(actual))
            if actual == submission
    ));
    assert!(matches!(
        backend.allocations[&device].sdma_storage,
        KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Async(actual))
            if actual == submission
    ));
    let driver = backend.scripted_sdma.as_ref().unwrap();
    assert!(driver.is_exhausted());
    assert_eq!(driver.live_owner_count(), 2);
    assert_eq!(driver.unexpected_drops(), 0);
    disarm_scripted_drop_after_inspection_v1(&mut backend);
}

#[test]
fn scripted_live_foreign_ready_receipt_is_terminal_with_exact_custody() {
    let byte_len = usize::try_from(HOST_VISIBLE_MEMORY_PAGE_BYTES_V1).unwrap();
    let steps = [
        ScriptedSdmaStepV1::Write {
            offset: 0,
            byte_len,
        },
        scripted_submit_step_v1(
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            0,
            0,
            u32::try_from(byte_len).unwrap(),
            ScriptedFailureModeV1::Success,
        ),
        ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
            direction: None,
            copy_bytes: None,
        }),
        ScriptedSdmaStepV1::PromoteComputeReadyForeignQueue,
    ];
    let (mut backend, stream, host, device) = scripted_direct_backend_v1(byte_len, steps);
    let prior_promotion = ready_promotion_observation_v1(50);
    backend.last_ready_promotion_performance = Some(prior_promotion);
    backend.next_ready_promotion_ordinal = Some(51);
    backend
        .write_allocation_v1(host, 0, &vec![0x7c; byte_len])
        .unwrap();
    let (source, destination) =
        scripted_copy_regions_v1(host, device, u64::try_from(byte_len).unwrap());
    let submission = backend
        .copy_async_v1(stream, source, destination, &[])
        .unwrap();

    assert!(matches!(
        backend.poll_v1(submission),
        Err(RuntimeBackendFailureV1::Terminal(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Terminal
    ));
    assert!(backend.terminal);
    assert_eq!(
        backend.last_ready_promotion_performance_v1(),
        Some(prior_promotion)
    );
    assert_eq!(backend.next_ready_promotion_ordinal, Some(51));
    assert!(matches!(
        backend.terminal_sdma_custody,
        Some(KfdRuntimeTerminalSdmaCustodyV1::Completed(
            DirectionalSdmaCompletedOwnerV1::Scripted(_)
        ))
    ));
    assert!(matches!(
        backend.active_sdma[&submission].phase,
        ActiveSdmaPhaseV1::Quarantined
    ));
    assert!(backend.published_sdma_submissions.is_empty());
    assert!(backend.published_sdma_index_is_consistent_v1());
    assert!(matches!(
        backend.allocations[&host].sdma_storage,
        KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Async(actual))
            if actual == submission
    ));
    assert!(matches!(
        backend.allocations[&device].sdma_storage,
        KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Async(actual))
            if actual == submission
    ));
    let driver = backend.scripted_sdma.as_ref().unwrap();
    assert!(driver.is_exhausted());
    assert_eq!(driver.live_owner_count(), 2);
    assert_eq!(driver.unexpected_drops(), 0);
    let _ = stream;
    disarm_scripted_drop_after_inspection_v1(&mut backend);
}
