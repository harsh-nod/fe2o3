use super::*;
use crate::persistent_allocation::{Gfx942PersistentAccessV1, Gfx942PersistentUseErrorV1};
use crate::persistent_directional_sdma::{
    admit_persistent_directional_sdma_pair_v1, promote_directional_persistent_sdma_custody_v1,
};
use crate::sdma::Gfx942SdmaBufferStorageV1;
use crate::sdma::{
    GFX942_SDMA_D2H_ENGINE_INDEX_V1, GFX942_SDMA_H2D_ENGINE_INDEX_V1, GFX942_SDMA_MAX_IN_FLIGHT_V1,
    GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1, GFX942_SDMA_RING_BYTES_V1,
    Gfx942DirectionalSdmaQueueObservationV1, Gfx942SdmaQueueObservationV1,
    persistent_sdma_buffers_for_test, persistent_sdma_ticket_coordinates_for_test,
    persistent_sdma_window_packet_count,
};
use fe2o3_runtime_model::{
    DeviceGenerationV1, DeviceKeyV1, PhysicalDeviceIdV1, QueueGenerationV1, QueueInstanceIdV1,
    QueueKeyV1, VmIdV1, VmKeyV1,
};
use sha2::{Digest, Sha256};

#[test]
fn paired_lease_contradictions_abort_without_unwinding_custody() {
    const MODE: &str = "FE2O3_PRIVATE_SDMA_PAIR_CONTRADICTION";
    if let Ok(mode) = std::env::var(MODE) {
        rustix::process::setrlimit(
            rustix::process::Resource::Core,
            rustix::process::Rlimit {
                current: Some(0),
                maximum: Some(0),
            },
        )
        .unwrap();
        struct MustNotUnwind;
        impl Drop for MustNotUnwind {
            fn drop(&mut self) {
                std::process::exit(91);
            }
        }
        let _custody = MustNotUnwind;
        match mode.as_str() {
            "prepared" => {
                let (mut prepared, request, _) = prepared_fixture(1);
                std::mem::swap(
                    &mut prepared.source_prepared,
                    &mut prepared.destination_prepared,
                );
                let _ = transition_same_device_persistent_sdma_window_publication_v1(
                    prepared,
                    SameDevicePersistentSdmaWindowPublicationObservationV1::Recoverable(request),
                    false,
                    false,
                );
            }
            "published" => {
                let (mut published, _request) = published_fixture(1);
                std::mem::swap(
                    &mut published.source_published,
                    &mut published.destination_published,
                );
                let _ = transition_same_device_persistent_sdma_window_completion_v1(
                    published,
                    SameDevicePersistentSdmaWindowCompletionObservationV1::QueueRetained,
                    true,
                );
            }
            _ => std::process::exit(92),
        }
        std::process::exit(93);
    }

    use std::io::Read;
    use std::os::unix::process::ExitStatusExt;
    for mode in ["prepared", "published"] {
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "persistent_same_device_sdma::tests::paired_lease_contradictions_abort_without_unwinding_custody",
                "--nocapture",
            ])
            .env(MODE, mode)
            .env("RUST_BACKTRACE", "0")
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if std::time::Instant::now() >= deadline {
                child.kill().unwrap();
                let _ = child.wait();
                panic!("private paired-lease contradiction child timed out");
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        };
        assert_eq!(status.signal(), Some(libc::SIGABRT), "{mode}: {status}");
        let mut stderr = String::new();
        child
            .stderr
            .take()
            .unwrap()
            .take(16 * 1024)
            .read_to_string(&mut stderr)
            .unwrap();
        assert!(!stderr.contains("panicked at"), "{mode}: {stderr}");
    }
}

fn queue_key() -> QueueKeyV1 {
    QueueKeyV1 {
        vm: VmKeyV1 {
            device: DeviceKeyV1 {
                physical: PhysicalDeviceIdV1(7),
                generation: DeviceGenerationV1(1),
            },
            id: VmIdV1(1),
        },
        id: QueueInstanceIdV1(3),
        generation: QueueGenerationV1(1),
    }
}

fn queue_observation(queue_id: u32, engine_index: u32) -> Gfx942SdmaQueueObservationV1 {
    Gfx942SdmaQueueObservationV1 {
        queue_id,
        ring_bytes: GFX942_SDMA_RING_BYTES_V1,
        maximum_in_flight: GFX942_SDMA_MAX_IN_FLIGHT_V1 as u16,
        engine_index: Some(engine_index),
    }
}

fn pair() -> crate::persistent_directional_sdma::Gfx942PersistentDirectionalSdmaPairV1 {
    admit_persistent_directional_sdma_pair_v1(Gfx942DirectionalSdmaQueueObservationV1 {
        host_to_device: queue_observation(17, GFX942_SDMA_H2D_ENGINE_INDEX_V1),
        device_to_host: queue_observation(23, GFX942_SDMA_D2H_ENGINE_INDEX_V1),
        admitted_engine_count: 2,
        admitted_queues_per_engine: 8,
    })
    .unwrap()
}

fn allocation(id: u64) -> Gfx942DirectionalQueuePersistentAllocationV1 {
    let (mut device, _) = persistent_sdma_buffers_for_test(queue_key(), id);
    device.set_logical_bytes(2048);
    let (allocation, outstanding) =
        promote_directional_persistent_sdma_custody_v1(device, pair(), 2).unwrap();
    assert_eq!(outstanding, 2);
    allocation
}

fn prepared_fixture(
    packet_count: usize,
) -> (
    SameDevicePersistentSdmaWindowPreparedCustodyV1,
    Gfx942SdmaCopyRequestV1,
    Vec<Gfx942SdmaCopyTicketV1>,
) {
    let mut source = allocation(10);
    let mut destination = allocation(20);
    let source_reserved = source
        .owner
        .reserve(same_device_source_use_request_v1(8, 32).unwrap(), None)
        .unwrap();
    let destination_reserved = destination
        .owner
        .reserve(
            same_device_destination_use_request_v1(16, 32).unwrap(),
            None,
        )
        .unwrap();
    let source_prepared = source.owner.prepare(source_reserved).unwrap();
    let destination_prepared = destination.owner.prepare(destination_reserved).unwrap();
    let (source_lease, destination_lease) =
        crate::persistent_allocation::detach_local_native_pair_for_sdma_v1(
            &mut source.owner,
            &mut destination.owner,
        )
        .unwrap();
    let source_buffer = Gfx942SdmaBufferV1::from_bridge_parts(
        Gfx942SdmaBufferStorageV1::Device(source_lease),
        source.attachment.queue,
        source.attachment.pool_generation,
        source.attachment.logical_bytes,
    );
    let destination_buffer = Gfx942SdmaBufferV1::from_bridge_parts(
        Gfx942SdmaBufferStorageV1::Device(destination_lease),
        destination.attachment.queue,
        destination.attachment.pool_generation,
        destination.attachment.logical_bytes,
    );
    let descriptor = same_device_persistent_sdma_descriptor_v1(8, 16, 32, packet_count);
    let request =
        same_device_persistent_sdma_request_v1(source_buffer, 8, destination_buffer, 16, 32);
    let tickets = (0..packet_count)
        .map(|index| {
            persistent_sdma_ticket_coordinates_for_test(
                queue_key(),
                pair().host_to_device_queue_id,
                index as u16,
                1,
            )
        })
        .collect::<Vec<_>>();
    (
        SameDevicePersistentSdmaWindowPreparedCustodyV1 {
            source,
            source_prepared,
            destination,
            destination_prepared,
            planned_tickets: tickets.clone(),
            descriptor,
        },
        request,
        tickets,
    )
}

fn published_fixture(
    packet_count: usize,
) -> (
    Gfx942SameDevicePersistentSdmaWindowSubmissionV1,
    Gfx942SdmaCopyRequestV1,
) {
    let (prepared, request, tickets) = prepared_fixture(packet_count);
    let SameDevicePersistentSdmaWindowPublicationTransitionV1::Published(submission) =
        transition_same_device_persistent_sdma_window_publication_v1(
            prepared,
            SameDevicePersistentSdmaWindowPublicationObservationV1::Confirmed(tickets),
            true,
            true,
        )
    else {
        unreachable!()
    };
    (submission, request)
}

#[test]
fn same_device_window_manifest_digest_is_frozen() {
    let digest = Sha256::digest(GFX942_SAME_DEVICE_PERSISTENT_SDMA_WINDOW_MANIFEST_V1);
    let rendered: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    assert_eq!(
        rendered,
        GFX942_SAME_DEVICE_PERSISTENT_SDMA_WINDOW_MANIFEST_SHA256_V1
    );
}

#[test]
fn same_device_window_packet_bound_is_exact() {
    let maximum = GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1;
    assert!(persistent_sdma_window_packet_count(0).is_err());
    assert_eq!(persistent_sdma_window_packet_count(1).unwrap(), 1);
    assert_eq!(
        persistent_sdma_window_packet_count(maximum * 63).unwrap(),
        63
    );
    assert!(persistent_sdma_window_packet_count(maximum * 63 + 1).is_err());
}

#[test]
fn terminal_same_device_admission_absorbs_invalid_geometry_after_affiliation() {
    let source = allocation(31);
    let destination = allocation(32);
    let source_identity = source.attachment.storage_identity;
    let destination_identity = destination.attachment.storage_identity;
    let failure = crate::queue::admit_same_device_persistent_sdma_window_input_v1(
        queue_key(),
        true,
        source,
        u64::MAX,
        destination,
        u64::MAX,
        0,
    )
    .expect_err("terminal custody must dominate invalid same-device geometry");
    let (_, custody) = failure.into_parts();
    let Gfx942SameDevicePersistentSdmaWindowSubmissionCustodyV1::ProcessTeardown(terminal) =
        custody
    else {
        panic!("self-owned terminal inputs must not return retryable custody")
    };
    assert_eq!(
        terminal.stage(),
        Gfx942SameDevicePersistentSdmaWindowTerminalStageV1::AdmissionRestored
    );
    assert_eq!(terminal.descriptor().copy_bytes(), 0);
    assert_eq!(terminal.descriptor().packet_count(), 0);
    let Gfx942SameDevicePersistentSdmaWindowTerminalStateV1::AdmissionRestored {
        source,
        destination,
    } = terminal.state
    else {
        unreachable!()
    };
    assert_eq!(source.attachment.storage_identity, source_identity);
    assert_eq!(
        destination.attachment.storage_identity,
        destination_identity
    );
}

#[test]
fn terminal_receiver_returns_foreign_same_device_inputs_exactly() {
    let source = allocation(33);
    let destination = allocation(34);
    let source_identity = source.attachment.storage_identity;
    let destination_identity = destination.attachment.storage_identity;
    let receiver = QueueKeyV1 {
        generation: QueueGenerationV1(2),
        ..queue_key()
    };
    let failure = crate::queue::admit_same_device_persistent_sdma_window_input_v1(
        receiver,
        true,
        source,
        0,
        destination,
        0,
        1,
    )
    .expect_err("foreign inputs must be returned before terminal absorption");
    let (_, custody) = failure.into_parts();
    let Gfx942SameDevicePersistentSdmaWindowSubmissionCustodyV1::Retryable {
        source,
        destination,
    } = custody
    else {
        panic!("foreign inputs must remain retryable on their producing queue")
    };
    assert_eq!(source.attachment.storage_identity, source_identity);
    assert_eq!(
        destination.attachment.storage_identity,
        destination_identity
    );
    let retry = crate::queue::admit_same_device_persistent_sdma_window_input_v1(
        queue_key(),
        false,
        source,
        0,
        destination,
        0,
        1,
    );
    assert!(retry.is_ok());
}

#[test]
fn same_device_leases_have_distinct_read_and_write_roles() {
    let (prepared, request, _) = prepared_fixture(3);
    assert_eq!(
        prepared.source_prepared.request().operation(),
        Gfx942PersistentOperationV1::LocalSdmaSource
    );
    assert_eq!(
        prepared.source_prepared.request().access(),
        Gfx942PersistentAccessV1::Read
    );
    assert_eq!(
        prepared.destination_prepared.request().operation(),
        Gfx942PersistentOperationV1::LocalSdmaDestination
    );
    assert_eq!(
        prepared.destination_prepared.request().access(),
        Gfx942PersistentAccessV1::Write
    );
    let SameDevicePersistentSdmaWindowPublicationTransitionV1::Retryable {
        source,
        destination,
    } = transition_same_device_persistent_sdma_window_publication_v1(
        prepared,
        SameDevicePersistentSdmaWindowPublicationObservationV1::Recoverable(request),
        true,
        true,
    )
    else {
        unreachable!()
    };
    assert_eq!(source.owner.live_use_count(), 0);
    assert_eq!(destination.owner.live_use_count(), 0);
}

#[test]
fn same_device_pending_timeout_completion_and_retirement_are_paired() {
    let (submission, request) = published_fixture(3);
    let SameDevicePersistentSdmaWindowCompletionTransitionV1::Pending(submission) =
        transition_same_device_persistent_sdma_window_completion_v1(
            submission,
            SameDevicePersistentSdmaWindowCompletionObservationV1::Pending,
            true,
        )
    else {
        unreachable!()
    };
    let SameDevicePersistentSdmaWindowCompletionTransitionV1::Timeout(submission) =
        transition_same_device_persistent_sdma_window_completion_v1(
            submission,
            SameDevicePersistentSdmaWindowCompletionObservationV1::Timeout,
            true,
        )
    else {
        unreachable!()
    };
    let completed_lower = CompletedPersistentSdmaWindowV1 {
        request,
        packet_count: 3,
    };
    let SameDevicePersistentSdmaWindowCompletionTransitionV1::Completed(completed) =
        transition_same_device_persistent_sdma_window_completion_v1(
            submission,
            SameDevicePersistentSdmaWindowCompletionObservationV1::Completed(completed_lower),
            true,
        )
    else {
        unreachable!()
    };
    assert_eq!(
        completed.descriptor(),
        same_device_persistent_sdma_descriptor_v1(8, 16, 32, 3)
    );
    let pair = match completed.retire_settled_frontiers_v1() {
        Ok(pair) => pair,
        Err(_) => panic!("exact paired frontier retirement must succeed"),
    };
    let (source, destination) = pair.into_parts();
    assert_eq!(source.owner.retained_settled_use_count(), 0);
    assert_eq!(destination.owner.retained_settled_use_count(), 0);
}

#[test]
fn same_device_completion_coordinate_substitution_is_terminal() {
    let (submission, mut request) = published_fixture(3);
    request.destination_offset += 1;
    let transition = transition_same_device_persistent_sdma_window_completion_v1(
        submission,
        SameDevicePersistentSdmaWindowCompletionObservationV1::Completed(
            CompletedPersistentSdmaWindowV1 {
                request,
                packet_count: 3,
            },
        ),
        true,
    );
    let SameDevicePersistentSdmaWindowCompletionTransitionV1::ProcessTeardown(custody) = transition
    else {
        panic!("coordinate substitution must be terminal")
    };
    assert_eq!(
        custody.stage(),
        Gfx942SameDevicePersistentSdmaWindowTerminalStageV1::CompletedUnrestored
    );
}

#[test]
fn same_device_completion_source_destination_substitution_is_terminal() {
    let (submission, mut request) = published_fixture(2);
    std::mem::swap(&mut request.source, &mut request.destination);
    std::mem::swap(&mut request.source_offset, &mut request.destination_offset);
    let transition = transition_same_device_persistent_sdma_window_completion_v1(
        submission,
        SameDevicePersistentSdmaWindowCompletionObservationV1::Completed(
            CompletedPersistentSdmaWindowV1 {
                request,
                packet_count: 2,
            },
        ),
        true,
    );
    assert!(matches!(
        transition,
        SameDevicePersistentSdmaWindowCompletionTransitionV1::ProcessTeardown(_)
    ));
}

#[test]
fn same_device_swapped_frontiers_do_not_partially_retire() {
    let (submission, request) = published_fixture(2);
    let transition = transition_same_device_persistent_sdma_window_completion_v1(
        submission,
        SameDevicePersistentSdmaWindowCompletionObservationV1::Completed(
            CompletedPersistentSdmaWindowV1 {
                request,
                packet_count: 2,
            },
        ),
        true,
    );
    let SameDevicePersistentSdmaWindowCompletionTransitionV1::Completed(completed) = transition
    else {
        unreachable!()
    };
    let Gfx942SameDevicePersistentSdmaWindowCompletedV1 {
        source,
        source_frontier,
        destination,
        destination_frontier,
        descriptor,
    } = completed;
    let substituted = Gfx942SameDevicePersistentSdmaWindowCompletedV1 {
        source,
        source_frontier: destination_frontier,
        destination,
        destination_frontier: source_frontier,
        descriptor,
    };
    let failure = match substituted.retire_settled_frontiers_v1() {
        Ok(_) => panic!("swapped frontiers must not retire"),
        Err(failure) => failure,
    };
    let Gfx942SameDevicePersistentSdmaWindowCompletedV1 {
        source,
        destination,
        ..
    } = failure.into_completed();
    assert_eq!(source.owner.retained_settled_use_count(), 1);
    assert_eq!(destination.owner.retained_settled_use_count(), 1);
}

#[test]
fn same_device_retained_and_ticket_substitution_quarantine_both_owners() {
    let (prepared, _, tickets) = prepared_fixture(3);
    let transition = transition_same_device_persistent_sdma_window_publication_v1(
        prepared,
        SameDevicePersistentSdmaWindowPublicationObservationV1::Retained(tickets),
        true,
        true,
    );
    let SameDevicePersistentSdmaWindowPublicationTransitionV1::ProcessTeardown(custody) =
        transition
    else {
        panic!("retained publication must be terminal")
    };
    assert_eq!(
        custody.stage(),
        Gfx942SameDevicePersistentSdmaWindowTerminalStageV1::PreparedQueueRetained
    );
    let Gfx942SameDevicePersistentSdmaWindowTerminalStateV1::PreparedQueueRetained {
        source,
        destination,
        ..
    } = custody.state
    else {
        unreachable!()
    };
    assert_eq!(
        source.owner.quarantine_reason(),
        Some(Gfx942PersistentQuarantineReasonV1::CallerReportedPublicationIndeterminate)
    );
    assert_eq!(
        destination.owner.quarantine_reason(),
        Some(Gfx942PersistentQuarantineReasonV1::CallerReportedPublicationIndeterminate)
    );

    let (prepared, _, mut tickets) = prepared_fixture(3);
    tickets[1] = persistent_sdma_ticket_coordinates_for_test(queue_key(), 17, 1, 2);
    let transition = transition_same_device_persistent_sdma_window_publication_v1(
        prepared,
        SameDevicePersistentSdmaWindowPublicationObservationV1::Confirmed(tickets),
        true,
        true,
    );
    let SameDevicePersistentSdmaWindowPublicationTransitionV1::ProcessTeardown(custody) =
        transition
    else {
        panic!("ticket substitution must be terminal")
    };
    assert_eq!(
        custody.stage(),
        Gfx942SameDevicePersistentSdmaWindowTerminalStageV1::PublishedQueueRetained
    );
    let Gfx942SameDevicePersistentSdmaWindowTerminalStateV1::PublishedQueueRetained {
        source,
        destination,
        ..
    } = custody.state
    else {
        unreachable!()
    };
    assert_eq!(
        source.owner.quarantine_reason(),
        Some(Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate)
    );
    assert_eq!(
        destination.owner.quarantine_reason(),
        Some(Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate)
    );
}

#[test]
fn same_device_identical_storage_is_rejected_before_pair_publication() {
    let mut source = allocation(40);
    let mut destination = allocation(40);
    let source_reserved = source
        .owner
        .reserve(same_device_source_use_request_v1(0, 32).unwrap(), None)
        .unwrap();
    let destination_reserved = destination
        .owner
        .reserve(
            same_device_destination_use_request_v1(64, 32).unwrap(),
            None,
        )
        .unwrap();
    let source_prepared = source.owner.prepare(source_reserved).unwrap();
    let destination_prepared = destination.owner.prepare(destination_reserved).unwrap();
    let failure = publish_local_sdma_pair_v1(
        &mut source.owner,
        source_prepared,
        &mut destination.owner,
        destination_prepared,
    )
    .expect_err("identical storage identity must be rejected");
    assert_eq!(
        failure.error,
        Gfx942PersistentUseErrorV1::WrongOwnerOrGeneration
    );
    assert_eq!(failure.source.request().range().offset(), 0);
    assert_eq!(failure.destination.request().range().offset(), 64);
}

#[test]
fn same_device_lower_selector_stays_additive() {
    let source = include_str!("../sdma.rs");
    let selector = source
        .split("fn prepare_same_device_persistent_window_recoverable")
        .nth(1)
        .expect("same-device selector")
        .split("pub(crate) fn submit")
        .next()
        .unwrap();
    assert!(selector.contains("GFX942_SDMA_H2D_OWNER_SLOT_V1"));
    assert!(selector.contains("prepare_persistent_window_recoverable"));
    let directional = source
        .split("fn owner_for_copy")
        .nth(1)
        .unwrap()
        .split("fn owner_for_requests")
        .next()
        .unwrap();
    assert!(directional.contains("admits only H2D or D2H copies"));
}
