use super::*;

#[test]
fn synchronous_single_path_fuses_publication_and_bounded_completion_scope() {
    let live = crate::queue::live_production_source_for_tests_v1();
    let fused = live
        .split("pub fn execute_synchronous_directional_persistent_sdma_copy_for_v1")
        .nth(1)
        .unwrap()
        .split("pub fn poll_directional_persistent_sdma_copy_v1")
        .next()
        .unwrap();
    assert!(fused.contains("admit_directional_persistent_sdma_request_v1"));
    assert_eq!(
        fused.matches("sdma_synchronous::execute_in_place").count(),
        1
    );
    let driver = include_str!("../../queue_live/sdma_synchronous.rs");
    let opening_scope = driver
        .split("fn opening(&mut self)")
        .nth(2)
        .unwrap()
        .split("fn loan(&mut self)")
        .next()
        .unwrap();
    assert_eq!(
        opening_scope
            .matches("execute_live_model_custody_v1")
            .count(),
        1
    );
    assert_eq!(
        opening_scope
            .matches("check_queue_operational_currentness")
            .count(),
        1
    );
    let execution = driver
        .split("pub(super) fn execute_in_place")
        .nth(1)
        .unwrap()
        .split("impl SdmaSynchronousContextV1")
        .next()
        .unwrap();
    assert_eq!(
        execution.matches("execute_live_model_custody_v1").count(),
        1
    );
    let installed = execution.find("*context.root() = Some").unwrap();
    let opening = execution.find("context.opening()").unwrap();
    let preparation_envelope = execution.find(".prepare_request()").unwrap();
    let loan = execution.find("execute_live_model_custody_v1").unwrap();
    assert!(installed < opening && opening < preparation_envelope && preparation_envelope < loan);
    let run = driver
        .split("pub(super) fn run_in_place")
        .nth(1)
        .unwrap()
        .split("pub(super) trait SdmaSynchronousContextV1")
        .next()
        .unwrap();
    let preparation = run.find("prepare_directional_single_in_place").unwrap();
    let prepublication = run
        .find("if let Err(error) = memory.check_queue_operational_currentness()")
        .unwrap();
    let publication = run.find("owner.publish_single_in_place").unwrap();
    let wait = run
        .find("wait_for_in_current_scope_with_final_currentness")
        .unwrap();
    let ticket_mismatch = run.find("ticket != planned").unwrap();
    assert!(preparation < prepublication);
    assert!(prepublication < publication);
    assert!(publication < wait);
    assert!(!driver.contains("wait_directional_persistent_sdma_copy_for_v1"));
    assert!(!driver.contains("submit_directional_persistent_sdma_copy_v1"));
    assert!(
        run[publication..ticket_mismatch].contains("memory.check_queue_operational_currentness()")
    );
    assert!(run[ticket_mismatch..wait].contains("memory.check_queue_operational_currentness()"));
    let completed = run
        .find("SingleSdmaWaitInCurrentScopeV1::Completed(completed)")
        .unwrap();
    let rooted = run[completed..]
        .find("root.data = Some(SingleSdmaCopyCustodyV1::Completed(completed))")
        .unwrap();
    let outcome = run[completed..].find("OutcomeV1::Published").unwrap();
    assert!(rooted < outcome);

    let sdma = include_str!("../../sdma.rs");
    let failure_close = sdma
        .split("fn close_single_sdma_wait_failure_currentness(")
        .nth(1)
        .unwrap()
        .split("impl PreparedSdmaBatchV1")
        .next()
        .unwrap();
    assert_eq!(
        failure_close
            .matches("check_queue_operational_currentness")
            .count(),
        1
    );
    let lower = sdma
        .split("fn wait_for_in_current_scope_with_final_currentness(")
        .nth(1)
        .unwrap()
        .split("pub(crate) fn wait_many_for(")
        .next()
        .unwrap();
    assert_eq!(
        lower.matches("check_queue_operational_currentness").count(),
        2
    );
    assert_eq!(
        lower
            .matches("close_single_sdma_wait_failure_currentness")
            .count(),
        5
    );
    let final_currentness = lower.rfind("check_queue_operational_currentness").unwrap();
    let record_retirement = lower.find(".take()").unwrap();
    assert!(final_currentness < record_retirement);
}

#[test]
fn recoverable_publication_restores_exact_owners_in_both_directions() {
    for (index, direction) in [
        Gfx942PersistentSdmaDirectionV1::HostToDevice,
        Gfx942PersistentSdmaDirectionV1::DeviceToHost,
    ]
    .into_iter()
    .enumerate()
    {
        let (allocation, host) = promoted_fixture(20 + index as u64, 2048);
        let identity = allocation.attachment.storage_identity;
        let (prepared, request, _) = prepared_fixture(allocation, host, None, direction, 1);
        let DirectionalPersistentSdmaPublicationTransitionV1::Retryable { allocation, host } =
            transition_directional_persistent_sdma_publication_v1(
                prepared,
                DirectionalPersistentSdmaPublicationObservationV1::Recoverable(request),
                true,
                true,
            )
        else {
            panic!("clean lower rejection must be retryable")
        };
        assert_eq!(allocation.attachment.storage_identity, identity);
        assert_eq!(host.kind(), Gfx942SdmaBufferKindV1::HostVisibleCoherent);
        assert!(allocation.owner.local_native_is_attached_for_sdma());
    }
}

#[test]
fn retained_and_substituted_ticket_publications_are_terminal() {
    let (allocation, host) = promoted_fixture(30, 2048);
    let (prepared, _request, ticket) = prepared_fixture(
        allocation,
        host,
        None,
        Gfx942PersistentSdmaDirectionV1::HostToDevice,
        1,
    );
    let DirectionalPersistentSdmaPublicationTransitionV1::ProcessTeardown(custody) =
        transition_directional_persistent_sdma_publication_v1(
            prepared,
            DirectionalPersistentSdmaPublicationObservationV1::Retained(ticket),
            true,
            true,
        )
    else {
        panic!("retained lower custody must be terminal")
    };
    assert_eq!(
        custody.stage(),
        Gfx942DirectionalPersistentSdmaTerminalStageV1::PreparedQueueRetained
    );

    let (allocation, host) = promoted_fixture(31, 2048);
    let (prepared, _request, _ticket) = prepared_fixture(
        allocation,
        host,
        None,
        Gfx942PersistentSdmaDirectionV1::DeviceToHost,
        1,
    );
    let substituted = persistent_sdma_ticket_coordinates_for_test(queue_key(), 24, 1, 1);
    let DirectionalPersistentSdmaPublicationTransitionV1::ProcessTeardown(custody) =
        transition_directional_persistent_sdma_publication_v1(
            prepared,
            DirectionalPersistentSdmaPublicationObservationV1::Confirmed(substituted),
            true,
            true,
        )
    else {
        panic!("substituted full ticket must be terminal")
    };
    assert_eq!(
        custody.stage(),
        Gfx942DirectionalPersistentSdmaTerminalStageV1::PublishedQueueRetained
    );
}

#[test]
fn every_full_ticket_coordinate_is_authenticated() {
    for case in 0..4 {
        let (allocation, host) = promoted_fixture(32 + case, 2048);
        let (prepared, _request, _ticket) = prepared_fixture(
            allocation,
            host,
            None,
            Gfx942PersistentSdmaDirectionV1::DeviceToHost,
            1,
        );
        let substituted = match case {
            0 => {
                persistent_sdma_ticket_coordinates_for_test(queue_key_with_generation(2), 23, 1, 1)
            }
            1 => persistent_sdma_ticket_coordinates_for_test(queue_key(), 24, 1, 1),
            2 => persistent_sdma_ticket_coordinates_for_test(queue_key(), 23, 2, 1),
            3 => persistent_sdma_ticket_coordinates_for_test(queue_key(), 23, 1, 2),
            _ => unreachable!(),
        };
        let DirectionalPersistentSdmaPublicationTransitionV1::ProcessTeardown(custody) =
            transition_directional_persistent_sdma_publication_v1(
                prepared,
                DirectionalPersistentSdmaPublicationObservationV1::Confirmed(substituted),
                true,
                true,
            )
        else {
            panic!("ticket substitution case {case} must be terminal")
        };
        assert_eq!(
            custody.stage(),
            Gfx942DirectionalPersistentSdmaTerminalStageV1::PublishedQueueRetained
        );
    }
}

#[test]
fn prepared_ticket_must_name_the_selected_child_and_valid_slot_generation() {
    for case in 0..3 {
        let (allocation, host) = promoted_fixture(36 + case, 2048);
        let (mut prepared, _request, _ticket) = prepared_fixture(
            allocation,
            host,
            None,
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            1,
        );
        prepared.planned_ticket = match case {
            0 => persistent_sdma_ticket_coordinates_for_test(queue_key(), 23, 1, 1),
            1 => persistent_sdma_ticket_coordinates_for_test(queue_key(), 17, 64, 1),
            2 => persistent_sdma_ticket_coordinates_for_test(queue_key(), 17, 1, 0),
            _ => unreachable!(),
        };
        let substituted = prepared.planned_ticket;
        let DirectionalPersistentSdmaPublicationTransitionV1::ProcessTeardown(custody) =
            transition_directional_persistent_sdma_publication_v1(
                prepared,
                DirectionalPersistentSdmaPublicationObservationV1::Confirmed(substituted),
                true,
                true,
            )
        else {
            panic!("invalid prepared ticket case {case} must be terminal")
        };
        assert_eq!(
            custody.stage(),
            Gfx942DirectionalPersistentSdmaTerminalStageV1::PublishedQueueRetained
        );
    }
}

#[test]
fn pending_timeout_and_exact_completion_preserve_direction_and_size() {
    let (submission, request) =
        published_fixture(40, Gfx942PersistentSdmaDirectionV1::DeviceToHost);
    let DirectionalPersistentSdmaCompletionTransitionV1::Pending(submission) =
        transition_directional_persistent_sdma_completion_v1(
            submission,
            DirectionalPersistentSdmaCompletionObservationV1::Pending,
            true,
        )
    else {
        unreachable!()
    };
    let DirectionalPersistentSdmaCompletionTransitionV1::Timeout(submission) =
        transition_directional_persistent_sdma_completion_v1(
            submission,
            DirectionalPersistentSdmaCompletionObservationV1::Timeout,
            true,
        )
    else {
        unreachable!()
    };
    assert_eq!(
        submission.direction(),
        Gfx942PersistentSdmaDirectionV1::DeviceToHost
    );
    assert_eq!(submission.copy_bytes(), 32);
    let DirectionalPersistentSdmaCompletionTransitionV1::Completed(completed) =
        transition_directional_persistent_sdma_completion_v1(
            submission,
            DirectionalPersistentSdmaCompletionObservationV1::Completed(completed_request(request)),
            true,
        )
    else {
        unreachable!()
    };
    assert_eq!(
        completed.direction(),
        Gfx942PersistentSdmaDirectionV1::DeviceToHost
    );
    assert_eq!(completed.copy_bytes(), 32);
    let completed = completed.into_single_packet_window_v1();
    assert_eq!(completed.packet_count(), 1);
    assert_eq!(completed.host_offset(), 8);
    assert_eq!(completed.device_offset(), 16);
    assert_eq!(completed.copy_bytes(), 32);
}

#[test]
fn next_use_requires_exact_frontier_retirement_not_dependency_chaining() {
    let (submission, request) =
        published_fixture(45, Gfx942PersistentSdmaDirectionV1::HostToDevice);
    let DirectionalPersistentSdmaCompletionTransitionV1::Completed(completed) =
        transition_directional_persistent_sdma_completion_v1(
            submission,
            DirectionalPersistentSdmaCompletionObservationV1::Completed(completed_request(request)),
            true,
        )
    else {
        unreachable!()
    };
    let (mut allocation, host, frontier) = completed.into_parts();
    let error = allocation
        .owner
        .reserve(
            Gfx942PersistentUseRequestV1::new(
                Gfx942PersistentOperationV1::LocalSdmaDestination,
                16,
                32,
            )
            .unwrap(),
            None,
        )
        .expect_err("an unretired overlapping frontier must block the next use");
    assert_eq!(
        error.error(),
        Gfx942PersistentUseErrorV1::DependencyRequired
    );
    let allocation = allocation
        .retire_settled_frontier_v1(frontier)
        .expect("the exact frontier must retire");
    let (_prepared, _request, _ticket) = prepared_fixture(
        allocation,
        host,
        None,
        Gfx942PersistentSdmaDirectionV1::HostToDevice,
        2,
    );
}

#[test]
fn demotion_requires_exact_frontier_retirement() {
    let (submission, request) =
        published_fixture(46, Gfx942PersistentSdmaDirectionV1::DeviceToHost);
    let DirectionalPersistentSdmaCompletionTransitionV1::Completed(completed) =
        transition_directional_persistent_sdma_completion_v1(
            submission,
            DirectionalPersistentSdmaCompletionObservationV1::Completed(completed_request(request)),
            true,
        )
    else {
        unreachable!()
    };
    let (allocation, _host, frontier) = completed.into_parts();
    let (error, allocation) = demote_directional_persistent_sdma_custody_v1(allocation, 2)
        .expect_err("an unretired frontier must block demotion");
    assert_eq!(error, Gfx942PersistentUseErrorV1::OutstandingUses);
    let allocation = allocation
        .retire_settled_frontier_v1(frontier)
        .expect("the exact frontier must retire");
    let (buffer, outstanding) = demote_directional_persistent_sdma_custody_v1(allocation, 2)
        .expect("retired custody must demote");
    assert_eq!(buffer.kind(), Gfx942SdmaBufferKindV1::DeviceLocal);
    assert_eq!(outstanding, 2);
}

#[test]
fn closing_currentness_loss_is_terminal_before_and_after_completion() {
    let (allocation, host) = promoted_fixture(50, 2048);
    let (prepared, request, _) = prepared_fixture(
        allocation,
        host,
        None,
        Gfx942PersistentSdmaDirectionV1::HostToDevice,
        1,
    );
    let DirectionalPersistentSdmaPublicationTransitionV1::ProcessTeardown(custody) =
        transition_directional_persistent_sdma_publication_v1(
            prepared,
            DirectionalPersistentSdmaPublicationObservationV1::Recoverable(request),
            true,
            false,
        )
    else {
        unreachable!()
    };
    assert_eq!(
        custody.stage(),
        Gfx942DirectionalPersistentSdmaTerminalStageV1::PreparedRestored
    );

    let (submission, request) =
        published_fixture(51, Gfx942PersistentSdmaDirectionV1::HostToDevice);
    let DirectionalPersistentSdmaCompletionTransitionV1::ProcessTeardown(custody) =
        transition_directional_persistent_sdma_completion_v1(
            submission,
            DirectionalPersistentSdmaCompletionObservationV1::Completed(completed_request(request)),
            false,
        )
    else {
        unreachable!()
    };
    assert_eq!(
        custody.stage(),
        Gfx942DirectionalPersistentSdmaTerminalStageV1::CompletedUnrestored
    );
}

#[test]
fn host_and_range_substitution_are_terminal() {
    let (allocation, host) = promoted_fixture(60, 2048);
    let (prepared, mut request, _) = prepared_fixture(
        allocation,
        host,
        None,
        Gfx942PersistentSdmaDirectionV1::HostToDevice,
        1,
    );
    request.source_offset += 1;
    let DirectionalPersistentSdmaPublicationTransitionV1::ProcessTeardown(custody) =
        transition_directional_persistent_sdma_publication_v1(
            prepared,
            DirectionalPersistentSdmaPublicationObservationV1::Recoverable(request),
            true,
            true,
        )
    else {
        unreachable!()
    };
    assert_eq!(
        custody.stage(),
        Gfx942DirectionalPersistentSdmaTerminalStageV1::PreparedUnrestored
    );

    let (allocation, host) = promoted_fixture(61, 2048);
    let (prepared, request, _) = prepared_fixture(
        allocation,
        host,
        None,
        Gfx942PersistentSdmaDirectionV1::DeviceToHost,
        1,
    );
    let (_unused, foreign_host) = persistent_sdma_buffers_for_test(queue_key(), 999);
    let Gfx942SdmaCopyRequestV1 {
        source: device,
        source_offset,
        destination: _,
        destination_offset,
        copy_bytes,
    } = request;
    let substituted = Gfx942SdmaCopyRequestV1::new(
        device,
        source_offset,
        foreign_host,
        destination_offset,
        copy_bytes,
    );
    let DirectionalPersistentSdmaPublicationTransitionV1::ProcessTeardown(custody) =
        transition_directional_persistent_sdma_publication_v1(
            prepared,
            DirectionalPersistentSdmaPublicationObservationV1::Recoverable(substituted),
            true,
            true,
        )
    else {
        unreachable!()
    };
    assert_eq!(
        custody.stage(),
        Gfx942DirectionalPersistentSdmaTerminalStageV1::PreparedUnrestored
    );
}

#[test]
fn more_than_64_repeated_same_direction_uses_are_admitted() {
    exercise_sequential_directions(70, |_| Gfx942PersistentSdmaDirectionV1::HostToDevice);
    exercise_sequential_directions(71, |_| Gfx942PersistentSdmaDirectionV1::DeviceToHost);
}

#[test]
fn more_than_64_arbitrarily_alternating_directions_are_admitted() {
    exercise_sequential_directions(72, |cycle| {
        if cycle.is_multiple_of(3) {
            Gfx942PersistentSdmaDirectionV1::DeviceToHost
        } else {
            Gfx942PersistentSdmaDirectionV1::HostToDevice
        }
    });
}
