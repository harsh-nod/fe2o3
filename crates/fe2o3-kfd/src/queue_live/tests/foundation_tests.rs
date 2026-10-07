use super::*;

#[test]
fn every_live_foundation_mutation_and_moved_owner_uses_the_unwind_envelope() {
    let source = crate::queue::live_production_source_for_tests_v1();
    let production = source.split("#[cfg(test)]\nmod tests").next().unwrap();
    assert_eq!(
        production
            .matches("restore_model_ownership_for_live_mutation()")
            .count(),
        1
    );
    assert_eq!(
        production
            .matches("retake_model_ownership_after_live_mutation(loan)")
            .count(),
        1
    );

    let auxiliary = production
        .split("pub fn create_auxiliary_compute_lane_with_fixed_dispatch")
        .nth(1)
        .unwrap()
        .split("pub fn destroy_auxiliary_compute_lane_v1")
        .next()
        .unwrap();
    assert!(auxiliary.contains("construction_auxiliary::construct_auxiliary_compute_lane_v1("));
    let construction = include_str!("../../queue_live/construction_auxiliary.rs");
    let envelope = construction
        .find("with_preparation_custody(|memory|")
        .unwrap();
    assert!(
        include_str!("../../queue_live/construction_auxiliary/parent.rs")
            .contains("self.with_live_queue_memory_model_custody(work)")
    );
    let callback = construction.find("root.prepare_dispatch(").unwrap();
    assert!(envelope < callback);
    let preparation = construction
        .split("pub(super) fn prepare_dispatch(")
        .nth(1)
        .unwrap()
        .split("fn prepare(")
        .next()
        .unwrap();
    let capture = preparation
        .find("capture_returned_preparation_v1(memory, &mut self.data, prepare_data)")
        .unwrap();
    let retained = preparation.find("self.preparation = Some(").unwrap();
    assert!(capture < retained);

    for wrapper in [
        "fn with_sdma_owner_memory<R>",
        "fn with_striped_sdma_owner_memory<R>",
    ] {
        let body = production
            .split(wrapper)
            .nth(1)
            .unwrap()
            .split("\n    fn ")
            .next()
            .unwrap();
        assert!(body.contains("std::panic::catch_unwind"));
        assert!(body.contains("self.with_live_queue_memory_model"));
        assert!(body.contains("std::panic::resume_unwind(payload)"));
    }
}

#[test]
fn destroyed_queue_observation_counts_base_and_optional_sdma_resources() {
    let without_sdma = destroyed_queue_observation(17);
    assert_eq!(without_sdma.queue_id(), 17);
    assert_eq!(
        without_sdma.released_resources(),
        GFX942_DESTROYED_QUEUE_RELEASED_RESOURCE_COUNT_V1
    );

    let with_sdma = destroyed_queue_observation_with_additional_resources(17, 3);
    assert_eq!(with_sdma.queue_id(), 17);
    assert_eq!(
        with_sdma.released_resources(),
        GFX942_DESTROYED_QUEUE_RELEASED_RESOURCE_COUNT_V1 + 3
    );
}

#[test]
fn persistent_sdma_request_restoration_is_exact_in_both_directions() {
    for (ordinal, direction) in [
        Gfx942PersistentSdmaDirectionV1::HostToDevice,
        Gfx942PersistentSdmaDirectionV1::DeviceToHost,
    ]
    .into_iter()
    .enumerate()
    {
        let (allocation, prepared, request, host_binding) =
            persistent_restore_fixture(direction, 80 + ordinal as u64);
        let (mut allocation, host) = match restore_persistent_sdma_request(
            allocation,
            direction,
            8,
            16,
            32,
            host_binding,
            request,
        ) {
            Ok(restored) => restored,
            Err(_) => panic!("exact persistent request must restore"),
        };
        assert!(allocation.owner.local_native_is_attached_for_sdma());
        assert_eq!(host.kind(), Gfx942SdmaBufferKindV1::HostVisibleCoherent);
        allocation.owner.cancel_prepared(prepared).unwrap();
    }
}

#[test]
fn persistent_sdma_request_restoration_rejects_generation_substitution() {
    let direction = Gfx942PersistentSdmaDirectionV1::HostToDevice;
    let (mut allocation, prepared, request, host_binding) =
        persistent_restore_fixture(direction, 82);
    allocation.attachment.pool_generation += 1;
    let (mut allocation, request) =
        restore_persistent_sdma_request(allocation, direction, 8, 16, 32, host_binding, request)
            .unwrap_err();
    assert!(!allocation.owner.local_native_is_attached_for_sdma());
    let (source, destination) = request.into_buffers();
    assert_eq!(source.kind(), Gfx942SdmaBufferKindV1::HostVisibleCoherent);
    assert_eq!(destination.kind(), Gfx942SdmaBufferKindV1::DeviceLocal);
    allocation.owner.cancel_prepared(prepared).unwrap();
}

#[test]
fn recoverable_persistent_publication_restores_exact_owners_without_teardown() {
    let direction = Gfx942PersistentSdmaDirectionV1::HostToDevice;
    let (custody, request, _ticket) = persistent_prepared_custody_fixture(direction, 83);
    let expected_request = custody.prepared.request();
    let expected_attachment = custody.allocation.attachment;
    let expected_host = request.source.storage_identity();
    let transition = transition_persistent_sdma_publication_v1(
        custody,
        PersistentSdmaPublicationObservationV1::Recoverable(request),
        true,
        true,
    );
    let PersistentSdmaPublicationTransitionV1::Retryable { allocation, host } = transition else {
        panic!("clean lower rejection must remain retryable without queue teardown")
    };
    assert_eq!(allocation.attachment, expected_attachment);
    assert_eq!(host.storage_identity(), expected_host);
    assert!(allocation.owner.local_native_is_attached_for_sdma());
    assert_eq!(allocation.owner.live_use_count(), 0);
    assert_eq!(allocation.owner.quarantine_reason(), None);
    assert_eq!(
        expected_request,
        Gfx942PersistentUseRequestV1::new(
            Gfx942PersistentOperationV1::LocalSdmaDestination,
            16,
            32,
        )
        .unwrap()
    );
}

#[test]
fn retained_persistent_publication_quarantines_directly_from_prepared() {
    let (custody, _request, ticket) =
        persistent_prepared_custody_fixture(Gfx942PersistentSdmaDirectionV1::DeviceToHost, 84);
    let sequence = custody.prepared.sequence();
    let transition = transition_persistent_sdma_publication_v1(
        custody,
        PersistentSdmaPublicationObservationV1::Retained(ticket),
        true,
        false,
    );
    let PersistentSdmaPublicationTransitionV1::ProcessTeardown(custody) = transition else {
        panic!("retained lower publication must require process teardown")
    };
    assert_eq!(custody.sequence(), Some(sequence));
    assert_eq!(
        custody.stage(),
        crate::Gfx942PersistentSdmaTerminalStageV1::PreparedQueueRetained
    );
    let Gfx942PersistentSdmaTerminalStateV1::PreparedQueueRetained {
        allocation,
        ticket: retained_ticket,
    } = custody.state
    else {
        unreachable!()
    };
    assert_eq!(retained_ticket, ticket);
    assert_eq!(
        allocation.owner.quarantine_reason(),
        Some(Gfx942PersistentQuarantineReasonV1::CallerReportedPublicationIndeterminate)
    );
}

#[test]
fn confirmed_persistent_publication_records_the_exact_ticket() {
    let (custody, _request, ticket) =
        persistent_prepared_custody_fixture(Gfx942PersistentSdmaDirectionV1::HostToDevice, 85);
    let sequence = custody.prepared.sequence();
    let expected_request = custody.prepared.request();
    let transition = transition_persistent_sdma_publication_v1(
        custody,
        PersistentSdmaPublicationObservationV1::Confirmed(ticket),
        true,
        true,
    );
    let PersistentSdmaPublicationTransitionV1::Published(submission) = transition else {
        panic!("confirmed exact publication must produce published custody")
    };
    assert_eq!(submission.ticket, ticket);
    assert_eq!(submission.published.sequence(), sequence);
    assert_eq!(submission.request(), expected_request);
    assert_eq!(submission.allocation.owner.live_use_count(), 1);
    assert_eq!(submission.allocation.owner.quarantine_reason(), None);
}

#[test]
fn confirmed_persistent_publication_rejects_same_queue_ticket_substitution() {
    for (ordinal, slot, generation) in [(0_u64, 1_u16, 1_u32), (1, 0, 2)] {
        let (custody, _request, planned_ticket) = persistent_prepared_custody_fixture(
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            95 + ordinal,
        );
        let substituted_ticket = crate::sdma::persistent_sdma_ticket_coordinates_for_test(
            custody.allocation.attachment.queue,
            custody.allocation.attachment.native_queue_id,
            slot,
            generation,
        );
        assert_ne!(substituted_ticket, planned_ticket);
        let transition = transition_persistent_sdma_publication_v1(
            custody,
            PersistentSdmaPublicationObservationV1::Confirmed(substituted_ticket),
            true,
            true,
        );
        let PersistentSdmaPublicationTransitionV1::ProcessTeardown(custody) = transition else {
            panic!("same-queue ticket substitution must require process teardown")
        };
        assert_eq!(
            custody.stage(),
            crate::Gfx942PersistentSdmaTerminalStageV1::PublishedQueueRetained
        );
        let Gfx942PersistentSdmaTerminalStateV1::PublishedQueueRetained { allocation, ticket } =
            custody.state
        else {
            unreachable!()
        };
        assert_eq!(ticket, substituted_ticket);
        assert_eq!(
            allocation.owner.quarantine_reason(),
            Some(Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate)
        );
    }
}

#[test]
fn pending_and_timeout_keep_the_exact_published_submission() {
    let (pending, _request) =
        persistent_published_custody_fixture(Gfx942PersistentSdmaDirectionV1::HostToDevice, 86);
    let pending_ticket = pending.ticket;
    let pending_sequence = pending.published.sequence();
    let PersistentSdmaCompletionTransitionV1::Pending(pending) =
        transition_persistent_sdma_completion_v1(
            pending,
            PersistentSdmaCompletionObservationV1::Pending,
            true,
        )
    else {
        panic!("pending observation must preserve published custody")
    };
    assert_eq!(pending.ticket, pending_ticket);
    assert_eq!(pending.published.sequence(), pending_sequence);

    let (timeout, _request) =
        persistent_published_custody_fixture(Gfx942PersistentSdmaDirectionV1::DeviceToHost, 87);
    let timeout_ticket = timeout.ticket;
    let timeout_sequence = timeout.published.sequence();
    let PersistentSdmaCompletionTransitionV1::Timeout(timeout) =
        transition_persistent_sdma_completion_v1(
            timeout,
            PersistentSdmaCompletionObservationV1::Timeout,
            true,
        )
    else {
        panic!("timeout observation must preserve published custody")
    };
    assert_eq!(timeout.ticket, timeout_ticket);
    assert_eq!(timeout.published.sequence(), timeout_sequence);
    assert_eq!(timeout.allocation.owner.live_use_count(), 1);
}

#[test]
fn exact_persistent_completion_restores_both_owners_and_frontier() {
    let (submission, request) =
        persistent_published_custody_fixture(Gfx942PersistentSdmaDirectionV1::DeviceToHost, 88);
    let sequence = submission.published.sequence();
    let expected_attachment = submission.allocation.attachment;
    let expected_host = request.destination.storage_identity();
    let transition = transition_persistent_sdma_completion_v1(
        submission,
        PersistentSdmaCompletionObservationV1::Completed(completed_persistent_request(request)),
        true,
    );
    let PersistentSdmaCompletionTransitionV1::Completed(completed) = transition else {
        panic!("exact completion must restore and settle custody")
    };
    assert_eq!(
        completed.direction(),
        Gfx942PersistentSdmaDirectionV1::DeviceToHost
    );
    assert_eq!(completed.copy_bytes(), 32);
    let (allocation, host, frontier) = completed.into_parts();
    assert_eq!(allocation.attachment, expected_attachment);
    assert_eq!(host.storage_identity(), expected_host);
    assert!(allocation.owner.local_native_is_attached_for_sdma());
    assert_eq!(allocation.owner.live_use_count(), 0);
    assert_eq!(allocation.owner.retained_settled_use_count(), 1);
    assert_eq!(frontier.through_sequence(), sequence);
}

#[test]
fn exact_persistent_completion_rejects_same_queue_host_substitution() {
    let (submission, request) =
        persistent_published_custody_fixture(Gfx942PersistentSdmaDirectionV1::HostToDevice, 97);
    let queue = submission.allocation.attachment.queue;
    let Gfx942SdmaCopyRequestV1 {
        source: original_host,
        source_offset,
        destination: exact_device,
        destination_offset,
        copy_bytes,
    } = request;
    let (_unused_device, substituted_host) =
        crate::sdma::persistent_sdma_buffers_for_test(queue, 98);
    assert_ne!(
        original_host.storage_identity(),
        substituted_host.storage_identity()
    );
    let completed = Gfx942SdmaCompletedCopyV1 {
        source: substituted_host,
        source_offset,
        destination: exact_device,
        destination_offset,
        copy_bytes,
    };
    let transition = transition_persistent_sdma_completion_v1(
        submission,
        PersistentSdmaCompletionObservationV1::Completed(completed),
        true,
    );
    let PersistentSdmaCompletionTransitionV1::ProcessTeardown(custody) = transition else {
        panic!("same-queue host substitution must require process teardown")
    };
    assert_eq!(
        custody.stage(),
        crate::Gfx942PersistentSdmaTerminalStageV1::CompletedUnrestored
    );
    let Gfx942PersistentSdmaTerminalStateV1::CompletedUnrestored {
        allocation,
        completed: _,
    } = custody.state
    else {
        unreachable!()
    };
    assert_eq!(
        allocation.owner.quarantine_reason(),
        Some(Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate)
    );
}

#[test]
fn persistent_host_binding_rejects_same_storage_descriptor_substitution() {
    let queue = test_queue_key(21, 1);
    let (device, host) = crate::sdma::persistent_sdma_buffers_for_test(queue, 99);
    drop(device);
    let binding = Gfx942PersistentSdmaHostBindingV1::capture(&host, queue);
    let logical_bytes = host.requested_bytes();
    let (storage, owner, pool_generation, recovered_logical_bytes) = host.into_bridge_parts();
    assert_eq!(logical_bytes, recovered_logical_bytes);
    let substituted_generation =
        Gfx942SdmaBufferV1::from_bridge_parts(storage, owner, pool_generation + 1, logical_bytes);
    assert!(!binding.matches(&substituted_generation));

    let (device, host) = crate::sdma::persistent_sdma_buffers_for_test(queue, 100);
    drop(device);
    let binding = Gfx942PersistentSdmaHostBindingV1::capture(&host, queue);
    let (storage, owner, pool_generation, logical_bytes) = host.into_bridge_parts();
    let substituted_extent =
        Gfx942SdmaBufferV1::from_bridge_parts(storage, owner, pool_generation, logical_bytes / 2);
    assert!(!binding.matches(&substituted_extent));
}

#[test]
fn completed_observation_with_lost_currentness_is_terminal_and_unrestored() {
    let (submission, request) =
        persistent_published_custody_fixture(Gfx942PersistentSdmaDirectionV1::HostToDevice, 89);
    let sequence = submission.published.sequence();
    let transition = transition_persistent_sdma_completion_v1(
        submission,
        PersistentSdmaCompletionObservationV1::Completed(completed_persistent_request(request)),
        false,
    );
    let PersistentSdmaCompletionTransitionV1::ProcessTeardown(custody) = transition else {
        panic!("completion outside the currentness envelope must be terminal")
    };
    assert_eq!(custody.sequence(), Some(sequence));
    assert_eq!(
        custody.stage(),
        crate::Gfx942PersistentSdmaTerminalStageV1::CompletedUnrestored
    );
    let Gfx942PersistentSdmaTerminalStateV1::CompletedUnrestored {
        allocation,
        completed: _,
    } = custody.state
    else {
        unreachable!()
    };
    assert_eq!(
        allocation.owner.quarantine_reason(),
        Some(Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss)
    );
}

#[test]
fn persistent_demotion_advances_pool_generation_without_changing_buffer_debit() {
    let (custody, request, _ticket) =
        persistent_prepared_custody_fixture(Gfx942PersistentSdmaDirectionV1::HostToDevice, 90);
    let expected_identity = custody.allocation.attachment.storage_identity;
    let expected_generation = custody.allocation.attachment.pool_generation + 1;
    let transition = transition_persistent_sdma_publication_v1(
        custody,
        PersistentSdmaPublicationObservationV1::Recoverable(request),
        true,
        true,
    );
    let PersistentSdmaPublicationTransitionV1::Retryable {
        allocation,
        host: _,
    } = transition
    else {
        panic!("clean rejection must restore quiescent allocation custody")
    };
    let (buffer, outstanding_buffers) = demote_persistent_sdma_custody_v1(allocation, 2).unwrap();
    assert_eq!(outstanding_buffers, 2);
    assert_eq!(buffer.kind(), Gfx942SdmaBufferKindV1::DeviceLocal);
    assert_eq!(buffer.storage_identity(), expected_identity);
    assert_eq!(buffer.pool_generation(), expected_generation);
}

#[test]
fn demote_repromote_rejects_frontier_aba_for_the_same_native_allocation() {
    let (submission, request) =
        persistent_published_custody_fixture(Gfx942PersistentSdmaDirectionV1::HostToDevice, 101);
    let PersistentSdmaCompletionTransitionV1::Completed(completed) =
        transition_persistent_sdma_completion_v1(
            submission,
            PersistentSdmaCompletionObservationV1::Completed(completed_persistent_request(request)),
            true,
        )
    else {
        unreachable!()
    };
    let (allocation, host, old_frontier) = completed.into_parts();
    let old_sequence = old_frontier.through_sequence();
    let old_identity = allocation.attachment.storage_identity;
    let old_pool_generation = allocation.attachment.pool_generation;
    let (device, outstanding_buffers) = demote_persistent_sdma_custody_v1(allocation, 2).unwrap();
    assert_eq!(outstanding_buffers, 2);
    assert_eq!(device.storage_identity(), old_identity);
    assert_eq!(device.pool_generation(), old_pool_generation + 1);

    let allocation = promote_persistent_sdma_custody_v1(
        device,
        17,
        Gfx942PersistentSdmaDirectionV1::HostToDevice.engine_index(),
    )
    .expect("the same device buffer must re-promote");
    assert_eq!(allocation.attachment.storage_identity, old_identity);
    let (prepared, request, ticket) = prepare_restored_persistent_custody(allocation, host, None);
    let PersistentSdmaPublicationTransitionV1::Published(submission) =
        transition_persistent_sdma_publication_v1(
            prepared,
            PersistentSdmaPublicationObservationV1::Confirmed(ticket),
            true,
            true,
        )
    else {
        unreachable!()
    };
    let PersistentSdmaCompletionTransitionV1::Completed(completed) =
        transition_persistent_sdma_completion_v1(
            submission,
            PersistentSdmaCompletionObservationV1::Completed(completed_persistent_request(request)),
            true,
        )
    else {
        unreachable!()
    };
    let (mut allocation, _host, new_frontier) = completed.into_parts();
    assert_eq!(new_frontier.through_sequence(), old_sequence);

    let rejected = allocation
        .owner
        .reserve(
            Gfx942PersistentUseRequestV1::new(
                Gfx942PersistentOperationV1::LocalSdmaDestination,
                16,
                32,
            )
            .unwrap(),
            Some(&old_frontier),
        )
        .expect_err("an old incarnation frontier cannot order new history");
    assert_eq!(
        rejected.error(),
        Gfx942PersistentUseErrorV1::StaleOrSubstitutedDependency
    );
    let failure = allocation
        .retire_settled_frontier_v1(old_frontier)
        .expect_err("an old incarnation frontier cannot retire new history");
    let (allocation, _returned_old_frontier) = failure.into_parts();
    let allocation = allocation
        .retire_settled_frontier_v1(new_frontier)
        .expect("the exact new incarnation frontier must retire new history");
    assert_eq!(allocation.owner.retained_settled_use_count(), 0);
}

#[test]
fn frontier_retirement_supports_more_than_the_bounded_ledger_sequential_uses() {
    let (mut prepared, mut request, mut ticket) =
        persistent_prepared_custody_fixture(Gfx942PersistentSdmaDirectionV1::HostToDevice, 91);
    let expected_attachment = prepared.allocation.attachment;
    for cycle in 0..(crate::GFX942_MAX_PERSISTENT_ALLOCATION_USES_V1 + 2) {
        let publication = transition_persistent_sdma_publication_v1(
            prepared,
            PersistentSdmaPublicationObservationV1::Confirmed(ticket),
            true,
            true,
        );
        let PersistentSdmaPublicationTransitionV1::Published(submission) = publication else {
            panic!("cycle {cycle} must publish")
        };
        let completion = transition_persistent_sdma_completion_v1(
            submission,
            PersistentSdmaCompletionObservationV1::Completed(completed_persistent_request(request)),
            true,
        );
        let PersistentSdmaCompletionTransitionV1::Completed(completed) = completion else {
            panic!("cycle {cycle} must complete")
        };
        let (allocation, host, frontier) = completed.into_parts();
        assert_eq!(allocation.owner.retained_settled_use_count(), 1);
        let allocation = allocation
            .retire_settled_frontier_v1(frontier)
            .unwrap_or_else(|_| panic!("cycle {cycle} exact frontier must retire"));
        assert_eq!(allocation.owner.retained_settled_use_count(), 0);
        assert_eq!(allocation.attachment, expected_attachment);
        if cycle + 1 == crate::GFX942_MAX_PERSISTENT_ALLOCATION_USES_V1 + 2 {
            break;
        }
        (prepared, request, ticket) = prepare_restored_persistent_custody(allocation, host, None);
    }
}

#[test]
fn stale_frontier_retirement_returns_exact_custody_and_current_frontier_still_retires() {
    let (first, first_request) =
        persistent_published_custody_fixture(Gfx942PersistentSdmaDirectionV1::HostToDevice, 92);
    let PersistentSdmaCompletionTransitionV1::Completed(first) =
        transition_persistent_sdma_completion_v1(
            first,
            PersistentSdmaCompletionObservationV1::Completed(completed_persistent_request(
                first_request,
            )),
            true,
        )
    else {
        unreachable!()
    };
    let (allocation, host, stale_frontier) = first.into_parts();
    let stale_sequence = stale_frontier.through_sequence();
    let (second, second_request, second_ticket) =
        prepare_restored_persistent_custody(allocation, host, Some(&stale_frontier));
    let PersistentSdmaPublicationTransitionV1::Published(second) =
        transition_persistent_sdma_publication_v1(
            second,
            PersistentSdmaPublicationObservationV1::Confirmed(second_ticket),
            true,
            true,
        )
    else {
        unreachable!()
    };
    let PersistentSdmaCompletionTransitionV1::Completed(second) =
        transition_persistent_sdma_completion_v1(
            second,
            PersistentSdmaCompletionObservationV1::Completed(completed_persistent_request(
                second_request,
            )),
            true,
        )
    else {
        unreachable!()
    };
    let (allocation, _host, current_frontier) = second.into_parts();
    assert!(current_frontier.through_sequence() > stale_sequence);
    let failure = allocation
        .retire_settled_frontier_v1(stale_frontier)
        .expect_err("stale frontier must return both move-only inputs");
    let (allocation, returned_stale) = failure.into_parts();
    assert_eq!(returned_stale.through_sequence(), stale_sequence);
    assert_eq!(allocation.owner.retained_settled_use_count(), 2);
    let allocation = allocation
        .retire_settled_frontier_v1(current_frontier)
        .expect("exact latest frontier retires all settled history");
    assert_eq!(allocation.owner.retained_settled_use_count(), 0);
}

#[test]
fn substituted_frontier_retirement_returns_both_allocation_and_frontier() {
    fn complete_one(
        direction: Gfx942PersistentSdmaDirectionV1,
        id: u64,
    ) -> (
        Gfx942QueuePersistentAllocationV1,
        Gfx942PersistentDependencyFrontierV1,
    ) {
        let (submission, request) = persistent_published_custody_fixture(direction, id);
        let PersistentSdmaCompletionTransitionV1::Completed(completed) =
            transition_persistent_sdma_completion_v1(
                submission,
                PersistentSdmaCompletionObservationV1::Completed(completed_persistent_request(
                    request,
                )),
                true,
            )
        else {
            unreachable!()
        };
        let (allocation, _host, frontier) = completed.into_parts();
        (allocation, frontier)
    }

    let (allocation_a, frontier_a) =
        complete_one(Gfx942PersistentSdmaDirectionV1::HostToDevice, 93);
    let (allocation_b, frontier_b) =
        complete_one(Gfx942PersistentSdmaDirectionV1::HostToDevice, 94);
    let frontier_b_sequence = frontier_b.through_sequence();
    let failure = allocation_a
        .retire_settled_frontier_v1(frontier_b)
        .expect_err("another allocation's frontier must be rejected");
    let (allocation_a, returned_frontier_b) = failure.into_parts();
    assert_eq!(returned_frontier_b.through_sequence(), frontier_b_sequence);
    let _allocation_a = allocation_a
        .retire_settled_frontier_v1(frontier_a)
        .expect("allocation A still accepts its exact frontier");
    let _allocation_b = allocation_b
        .retire_settled_frontier_v1(returned_frontier_b)
        .expect("allocation B accepts its returned exact frontier");
}

#[test]
fn auxiliary_destroy_accepts_fully_released_detached_lane() {
    let queue = test_queue_key(201, 1);
    for insertion in [None, Some(0)] {
        let mut lane = compute_lane_state_for_multi_inflight_test(queue);
        lane.detached_dispatch_generation = Some(64);
        lane.detached_next_insertion_index = insertion;
        let mut state = Some(lane);
        let released = take_after_auxiliary_destroy_preflight_v1(
            &mut state,
            preflight_auxiliary_compute_lane_destroy_v1,
        )
        .expect("completed detached lane has no remaining data custody");
        assert!(state.is_none());
        assert!(released.dispatch.is_none());
        assert_eq!(released.detached_dispatch_generation, Some(64));
        assert_eq!(released.detached_next_insertion_index, insertion);
    }
}

#[test]
fn auxiliary_destroy_rejects_live_or_malformed_detached_ledger_without_taking_custody() {
    let queue = test_queue_key(202, 1);
    let (device, _host) = crate::sdma::persistent_sdma_buffers_for_test(queue, 0x6200);
    let Gfx942SdmaBufferStorageIdentityV1::Device(identity) = device.storage_identity() else {
        unreachable!("device fixture retains device storage")
    };
    let identity = Gfx942FixedDispatchStorageIdentityV1::DeviceUninitialized(identity);
    for (generation, count, identities, insertion) in [
        (None, 0, vec![], None),
        (Some(64), 1, vec![identity], None),
        (Some(64), 1, vec![], None),
        (Some(64), 0, vec![identity], Some(0)),
        (Some(64), 0, vec![], Some(1)),
        (Some(0), 0, vec![], None),
    ] {
        let mut lane = compute_lane_state_for_multi_inflight_test(queue);
        lane.detached_dispatch_generation = generation;
        lane.detached_data_count = count;
        lane.detached_data_identities = identities.clone();
        lane.detached_next_insertion_index = insertion;
        let completion_before = lane.completion_owner.state_snapshot_for_test();
        let mut state = Some(lane);
        assert!(
            take_after_auxiliary_destroy_preflight_v1(
                &mut state,
                preflight_auxiliary_compute_lane_destroy_v1,
            )
            .is_err()
        );
        let retained = state.as_ref().expect("rejected preflight retains the lane");
        assert_eq!(retained.key, queue);
        assert_eq!(retained.detached_dispatch_generation, generation);
        assert_eq!(retained.detached_data_count, count);
        assert_eq!(retained.detached_data_identities, identities);
        assert_eq!(retained.detached_next_insertion_index, insertion);
        assert_eq!(
            retained.completion_owner.state_snapshot_for_test(),
            completion_before
        );
    }
}

#[test]
fn auxiliary_destroy_rejects_live_completion_then_accepts_exact_cancellation() {
    let queue = test_queue_key(203, 1);
    let mut lane = compute_lane_state_for_multi_inflight_test(queue);
    lane.detached_dispatch_generation = Some(64);
    lane.detached_next_insertion_index = Some(0);
    let bound = lane
        .completion_owner
        .bind_batch([test_completion_template(queue, 65)])
        .unwrap();
    let (_, retention) = bound.into_parts();
    let before = lane.completion_owner.state_snapshot_for_test();
    let mut state = Some(lane);
    assert!(matches!(
        take_after_auxiliary_destroy_preflight_v1(
            &mut state,
            preflight_auxiliary_compute_lane_destroy_v1,
        ),
        Err(ComputeAqlQueueSessionErrorV1::Completion(_))
    ));
    let retained = state
        .as_mut()
        .expect("live completion retains lane custody");
    assert_eq!(retained.completion_owner.state_snapshot_for_test(), before);
    retained.completion_owner.cancel_bound(retention).unwrap();
    assert!(
        take_after_auxiliary_destroy_preflight_v1(
            &mut state,
            preflight_auxiliary_compute_lane_destroy_v1,
        )
        .is_ok()
    );
    assert!(state.is_none());
}

#[test]
fn auxiliary_destroy_preflight_rejection_preserves_custody_for_retry() {
    struct TestLane {
        leased: bool,
    }

    let mut state = Some(TestLane { leased: true });
    let rejected = take_after_auxiliary_destroy_preflight_v1(&mut state, |lane| {
        if lane.leased {
            Err(ComputeAqlQueueSessionErrorV1::Contract(
                "injected live completion lease",
            ))
        } else {
            Ok(())
        }
    });
    assert!(matches!(
        rejected,
        Err(ComputeAqlQueueSessionErrorV1::Contract(
            "injected live completion lease"
        ))
    ));
    assert!(state.as_ref().is_some_and(|lane| lane.leased));

    state.as_mut().unwrap().leased = false;
    let released = take_after_auxiliary_destroy_preflight_v1(&mut state, |_| Ok(())).unwrap();
    assert!(!released.leased);
    assert!(state.is_none());
}

#[test]
fn combined_and_standalone_striped_owners_route_without_aliasing() {
    let live = crate::queue::live_production_source_for_tests_v1()
        .split("#[cfg(test)]\nmod tests")
        .next()
        .unwrap();
    let combined = live
        .split("pub fn enable_gfx942_directional_and_striped_sdma_copy_engines_v1")
        .nth(1)
        .unwrap()
        .split("pub fn allocate_sdma_host_buffer")
        .next()
        .unwrap();
    let create = combined
        .find("with_sdma_queue_creation_custody_v1")
        .unwrap();
    assert!(create < combined.find("self.sdma = Some(directional)").unwrap());
    assert!(create < combined.find("self.striped_sdma = Some(striped)").unwrap());
    assert!(combined.contains("combined_striped_sdma_queue_count_is_admitted"));

    let routing = live
        .split("fn with_striped_sdma_owner_memory<R>")
        .nth(1)
        .unwrap()
        .split("fn with_device_buffer_pool")
        .next()
        .unwrap();
    assert!(routing.contains("let separate = self.striped_sdma.is_some()"));
    assert!(routing.contains("self.striped_sdma.take()"));
    assert!(routing.contains("self.sdma.take()"));
    assert!(routing.contains("self.striped_sdma = Some(owner)"));
    assert!(routing.contains("self.sdma = Some(owner)"));
}

#[test]
fn session_owned_queue_id_roster_rejects_primary_auxiliary_and_sdma_collisions() {
    let mut roster = Vec::with_capacity(4);
    assert!(push_unique_queue_id_v1(&mut roster, 7));
    assert!(push_unique_queue_id_v1(&mut roster, 11));
    assert!(!push_unique_queue_id_v1(&mut roster, 7));
    assert_eq!(roster, [7, 11]);

    assert!(queue_id_collides_with_session_owned_roster_v1(
        7, 7, false, false
    ));
    assert!(queue_id_collides_with_session_owned_roster_v1(
        13, 7, true, false
    ));
    assert!(queue_id_collides_with_session_owned_roster_v1(
        13, 7, false, true
    ));
    assert!(!queue_id_collides_with_session_owned_roster_v1(
        13, 7, false, false
    ));

    let auxiliary = include_str!("../../queue_live/construction_auxiliary.rs");
    let auxiliary_finish = auxiliary
        .split("fn create_and_install(")
        .nth(1)
        .unwrap()
        .split("pub(super) fn construct_auxiliary_compute_lane_v1")
        .next()
        .unwrap();
    let recover_queue_id = auxiliary_finish
        .find("recover_native_queue_id(engine, key)")
        .unwrap();
    let collision = auxiliary_finish
        .find("session_owned_queue_id_is_retained_v1(queue_id)")
        .unwrap();
    let install = auxiliary_finish
        .find("install_auxiliary_compute_lane_slot_v1")
        .unwrap();
    assert!(recover_queue_id < collision && collision < install);
}

#[test]
fn combined_teardown_destroys_and_releases_both_owner_sets_in_order() {
    let live = crate::queue::live_production_source_for_tests_v1()
        .split("#[cfg(test)]\nmod tests")
        .next()
        .unwrap();
    let destroy = live
        .split("fn destroy_queue_and_event")
        .nth(1)
        .unwrap()
        .split("fn complete_destroy<T>")
        .next()
        .unwrap();
    let striped_destroy = destroy.find("striped_sdma.destroy_queue").unwrap();
    let directional_destroy = destroy.find("sdma.destroy_queue").unwrap();
    let compute_destroy = destroy.find("engine.destroy(self.key)").unwrap();
    assert!(striped_destroy < directional_destroy);
    assert!(directional_destroy < compute_destroy);

    let release = live
        .split("fn complete_destroy<T>")
        .nth(1)
        .unwrap()
        .split("fn require_sdma_enabled")
        .next()
        .unwrap();
    assert!(release.contains("saturating_add"));
    let striped_release = release.find("if let Some(striped_sdma)").unwrap();
    let directional_release = release.find("if let Some(sdma)").unwrap();
    assert!(striped_release < directional_release);
}
