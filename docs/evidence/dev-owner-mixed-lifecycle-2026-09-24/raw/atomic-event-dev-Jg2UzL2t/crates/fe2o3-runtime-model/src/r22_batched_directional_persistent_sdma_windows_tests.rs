use super::*;
use alloc::vec;

const TRANSFER_ID: u64 = 41;

fn device() -> DeviceKeyV1 {
    DeviceKeyV1 {
        physical: PhysicalDeviceIdV1(0x221),
        generation: DeviceGenerationV1(22),
    }
}

fn vm() -> VmKeyV1 {
    VmKeyV1 {
        device: device(),
        id: VmIdV1(23),
    }
}

fn allocation() -> R18NativeAllocationKeyV1 {
    let allocation = MemoryAllocationKeyV1 {
        vm: vm(),
        id: AllocationIdV1(24),
        generation: AllocationGenerationV1(25),
    };
    R18NativeAllocationKeyV1 {
        owner: R17PersistentAllocationOwnerIdV1(26),
        allocation,
        mapping: MemoryMappingKeyV1 {
            allocation,
            id: MappingIdV1(27),
        },
    }
}

fn pair() -> R19DirectionalQueuePairV1 {
    R19DirectionalQueuePairV1 {
        parent_queue: QueueKeyV1 {
            vm: vm(),
            id: QueueInstanceIdV1(28),
            generation: QueueGenerationV1(29),
        },
        pair_occurrence: 30,
        device_to_host: R19DirectionalChildQueueV1 {
            native_queue_id: 3,
            engine_id: R18_LOCAL_SDMA_DEVICE_TO_HOST_ENGINE_V1,
        },
        host_to_device: R19DirectionalChildQueueV1 {
            native_queue_id: 4,
            engine_id: R18_LOCAL_SDMA_HOST_TO_DEVICE_ENGINE_V1,
        },
    }
}

fn binding(byte_len: u64) -> R21SeamBindingV1 {
    R21SeamBindingV1 {
        allocation: allocation(),
        pair: pair(),
        attachment_generation: 31,
        pool_generation: 32,
        logical_byte_len: byte_len,
        physical_byte_len: byte_len,
        host_storage_id: 33,
        host_storage_generation: 34,
    }
}

fn host(byte_len: u64) -> R18HostBufferKeyV1 {
    R18HostBufferKeyV1 {
        session_id: 35,
        id: 33,
        generation: 34,
        byte_len,
        coherence: MemoryCoherenceV1::HostCoherent,
    }
}

fn request(byte_len: u64, direction: R18LocalSdmaDirectionV1) -> R20CopyRequestV1 {
    let host = R20CopyEndpointV1::Host {
        buffer: host(byte_len),
        offset: 0,
    };
    let device = R20CopyEndpointV1::Device {
        allocation: allocation(),
        offset: 0,
    };
    match direction {
        R18LocalSdmaDirectionV1::HostToDevice => R20CopyRequestV1 {
            transfer_id: TRANSFER_ID,
            source: host,
            destination: device,
            byte_len,
        },
        R18LocalSdmaDirectionV1::DeviceToHost => R20CopyRequestV1 {
            transfer_id: TRANSFER_ID,
            source: device,
            destination: host,
            byte_len,
        },
    }
}

fn model(byte_len: u64) -> R22BatchedDirectionalPersistentSdmaWindowsV1 {
    R22BatchedDirectionalPersistentSdmaWindowsV1::new_model_only(binding(byte_len)).unwrap()
}

fn begin(
    model: &mut R22BatchedDirectionalPersistentSdmaWindowsV1,
    byte_len: u64,
    direction: R18LocalSdmaDirectionV1,
) {
    assert_eq!(
        model.begin_model_only(request(byte_len, direction), vec![]),
        Ok(R22WindowClassificationV1::Applied)
    );
}

fn prepare(model: &mut R22BatchedDirectionalPersistentSdmaWindowsV1) -> R22WindowPlanV1 {
    match model.prepare_window_model_only(&[]).unwrap() {
        R22WindowClassificationV1::Prepared(plan) => plan,
        outcome => panic!("unexpected preparation: {outcome:?}"),
    }
}

fn publish(
    model: &mut R22BatchedDirectionalPersistentSdmaWindowsV1,
    plan: &R22WindowPlanV1,
) -> R22WindowCompletionMetadataV1 {
    match model
        .resolve_publication_model_only(plan, R22WindowPublicationDispositionV1::Confirmed)
        .unwrap()
    {
        R22WindowClassificationV1::Published(metadata) => metadata,
        outcome => panic!("unexpected publication: {outcome:?}"),
    }
}

fn complete_and_retire(
    model: &mut R22BatchedDirectionalPersistentSdmaWindowsV1,
    status: R18SdmaTerminalStatusV1,
) -> R22WindowClassificationV1 {
    let plan = prepare(model);
    let metadata = publish(model, &plan);
    let frontier = match model
        .poll_window_model_only(
            R22WindowPollDispositionV1::Terminal(status),
            Some(&metadata),
        )
        .unwrap()
    {
        R22WindowClassificationV1::FrontierPending(frontier) => frontier,
        outcome => panic!("unexpected completion: {outcome:?}"),
    };
    model.retire_window_model_only(&frontier).unwrap()
}

#[test]
fn idle_r19_snapshot_derives_exact_device_ready_binding() {
    let snapshot = R19DirectionalSnapshotV1 {
        allocation: allocation(),
        pair: pair(),
        attachment_generation: 31,
        pool_generation: 32,
        logical_byte_len: 4096,
        physical_byte_len: 4096,
        current: true,
        phase: None,
        location: R19DirectionalLocationV1::PersistentAllocation,
        live_ticket: None,
        pending_frontier: None,
        settled_transfer_count: 9,
    };
    let model = R22BatchedDirectionalPersistentSdmaWindowsV1::from_idle_r19_snapshot_model_only(
        snapshot, 33, 34,
    )
    .unwrap();
    let state = model.snapshot();
    assert_eq!(state.phase, R22WindowPhaseV1::DeviceReady);
    assert_eq!(state.custody, Some(R22WindowCustodyKindV1::Device));
    assert_eq!(state.authority_count, 1);
    assert_eq!(state.aggregate_lease_count, 0);

    let mut busy = snapshot;
    busy.phase = Some(R19DirectionalPhaseV1::Published);
    assert!(matches!(
        R22BatchedDirectionalPersistentSdmaWindowsV1::from_idle_r19_snapshot_model_only(
            busy, 33, 34
        ),
        Err(R22WindowErrorV1::InvalidBinding)
    ));
}

#[test]
fn unsupported_or_substituted_endpoint_is_preflight_atomic() {
    let mut model = model(4096);
    let before = model.snapshot();
    let host = R20CopyEndpointV1::Host {
        buffer: host(4096),
        offset: 0,
    };
    assert_eq!(
        model.begin_model_only(
            R20CopyRequestV1 {
                transfer_id: TRANSFER_ID,
                source: host,
                destination: host,
                byte_len: 4096,
            },
            vec![],
        ),
        Err(R22WindowErrorV1::InvalidRequest)
    );
    assert_eq!(model.snapshot(), before);

    let mut substituted = request(4096, R18LocalSdmaDirectionV1::HostToDevice);
    if let R20CopyEndpointV1::Host { buffer, .. } = &mut substituted.source {
        buffer.generation += 1;
    }
    assert_eq!(
        model.begin_model_only(substituted, vec![]),
        Err(R22WindowErrorV1::InvalidRequest)
    );
    assert_eq!(model.snapshot(), before);
}

#[test]
fn dependency_identity_and_pending_are_preparation_atomic() {
    let dependency = R20DependencyV1 {
        event_id: 51,
        generation: 52,
    };
    let mut model = model(4096);
    model
        .begin_model_only(
            request(4096, R18LocalSdmaDirectionV1::HostToDevice),
            vec![dependency],
        )
        .unwrap();
    let before = model.snapshot();
    assert_eq!(
        model.prepare_window_model_only(&[R20DependencyObservationV1 {
            dependency: R20DependencyV1 {
                generation: 53,
                ..dependency
            },
            status: R20DependencyStatusV1::Satisfied,
        }]),
        Err(R22WindowErrorV1::DependencyMismatch)
    );
    assert_eq!(model.snapshot(), before);
    assert_eq!(
        model.prepare_window_model_only(&[R20DependencyObservationV1 {
            dependency,
            status: R20DependencyStatusV1::Pending,
        }]),
        Ok(R22WindowClassificationV1::DependencyPending)
    );
    assert_eq!(model.snapshot(), before);
}

#[test]
fn plans_one_two_and_sixty_three_exact_contiguous_packets() {
    for (byte_len, expected_packets) in [
        (1, 1),
        (R18_SDMA_MAX_LINEAR_COPY_BYTES_V1 + 1, 2),
        (R22_SDMA_WINDOW_MAX_BYTES_V1, 63),
    ] {
        let mut model = model(byte_len);
        begin(&mut model, byte_len, R18LocalSdmaDirectionV1::HostToDevice);
        let plan = prepare(&mut model);
        assert_eq!(plan.packets.len(), expected_packets);
        assert!(plan.packets.len() <= R22_SDMA_WINDOW_MAX_PACKETS_V1);
        assert_eq!(plan.byte_len, byte_len);
        let mut next = 0;
        for (index, packet) in plan.packets.iter().enumerate() {
            assert_eq!(usize::from(packet.packet_index), index);
            assert_eq!(packet.transfer_offset, next);
            assert_eq!(packet.device_range.byte_offset, next);
            assert_eq!(packet.host_range.byte_offset, next);
            assert_eq!(packet.device_range.byte_len, packet.host_range.byte_len);
            assert!(packet.device_range.byte_len <= R18_SDMA_MAX_LINEAR_COPY_BYTES_V1);
            next += packet.device_range.byte_len;
        }
        assert_eq!(next, byte_len);
    }
}

#[test]
fn ticket_roster_uses_exact_directional_child_and_unique_wrapping_slots() {
    let total = R22_SDMA_WINDOW_MAX_BYTES_V1 + R18_SDMA_MAX_LINEAR_COPY_BYTES_V1 + 2048;
    let mut model = model(total);
    begin(&mut model, total, R18LocalSdmaDirectionV1::DeviceToHost);
    let first = prepare(&mut model);
    assert_eq!(first.packets.len(), 63);
    for (index, packet) in first.packets.iter().enumerate() {
        assert_eq!(
            packet.ticket.queue_id,
            pair().device_to_host.native_queue_id
        );
        assert_eq!(usize::from(packet.ticket.slot), index);
        assert_eq!(packet.ticket.generation, 1);
    }
    let metadata = publish(&mut model, &first);
    let frontier = match model
        .poll_window_model_only(
            R22WindowPollDispositionV1::Terminal(R18SdmaTerminalStatusV1::Succeeded),
            Some(&metadata),
        )
        .unwrap()
    {
        R22WindowClassificationV1::FrontierPending(frontier) => frontier,
        outcome => panic!("unexpected completion: {outcome:?}"),
    };
    model.retire_window_model_only(&frontier).unwrap();
    let second = prepare(&mut model);
    assert_eq!(second.packets.len(), 2);
    assert_eq!(second.packets[0].ticket.slot, 63);
    assert_eq!(second.packets[1].ticket.slot, 0);
    assert_eq!(second.packets[0].ticket.generation, 1);
    assert_eq!(second.packets[1].ticket.generation, 2);
    assert_ne!(second.packets[0].ticket.slot, second.packets[1].ticket.slot);
}

#[test]
fn preparation_and_retry_restore_the_exact_ready_state() {
    let mut model = model(R18_SDMA_MAX_LINEAR_COPY_BYTES_V1 + 1);
    begin(
        &mut model,
        R18_SDMA_MAX_LINEAR_COPY_BYTES_V1 + 1,
        R18LocalSdmaDirectionV1::HostToDevice,
    );
    let ready = model.snapshot();
    let plan = prepare(&mut model);
    let prepared = model.snapshot();
    assert_eq!(prepared.aggregate_lease_count, 1);
    assert_eq!(prepared.published_windows, 0);
    assert_eq!(prepared.write_pointer_publications, 0);
    assert_eq!(prepared.doorbell_publications, 0);
    assert_eq!(
        model.resolve_publication_model_only(
            &plan,
            R22WindowPublicationDispositionV1::RetryableBeforeQueueCustody,
        ),
        Ok(R22WindowClassificationV1::Retryable)
    );
    assert_eq!(model.snapshot(), ready);
}

#[test]
fn confirmed_window_has_one_pointer_and_one_doorbell_action() {
    let mut model = model(R22_SDMA_WINDOW_MAX_BYTES_V1);
    begin(
        &mut model,
        R22_SDMA_WINDOW_MAX_BYTES_V1,
        R18LocalSdmaDirectionV1::HostToDevice,
    );
    let plan = prepare(&mut model);
    publish(&mut model, &plan);
    let state = model.snapshot();
    assert_eq!(state.published_windows, 1);
    assert_eq!(state.published_packets, 63);
    assert_eq!(state.write_pointer_publications, 1);
    assert_eq!(state.doorbell_publications, 1);
    assert_eq!(state.aggregate_lease_count, 1);
    for generation in &state.slot_generations[..R22_SDMA_WINDOW_MAX_PACKETS_V1] {
        assert_eq!(*generation, 1);
    }
    assert_eq!(state.slot_generations[R22_SDMA_WINDOW_MAX_PACKETS_V1], 0);
}

#[test]
fn retained_publication_enters_single_authority_teardown() {
    let mut model = model(4096);
    begin(&mut model, 4096, R18LocalSdmaDirectionV1::HostToDevice);
    let plan = prepare(&mut model);
    assert_eq!(
        model.resolve_publication_model_only(
            &plan,
            R22WindowPublicationDispositionV1::RetainedAfterPacketWrite,
        ),
        Ok(R22WindowClassificationV1::ProcessTeardown {
            point: R22WindowFailurePointV1::Publication,
        })
    );
    let state = model.snapshot();
    assert_eq!(state.custody, Some(R22WindowCustodyKindV1::Opaque));
    assert_eq!(state.authority_count, 1);
    assert_eq!(state.aggregate_lease_count, 1);
    assert_eq!(
        model.release_submission_model_only(TRANSFER_ID),
        Err(R22WindowErrorV1::ProcessTeardown)
    );
}

#[test]
fn pending_and_timeout_preserve_whole_published_window() {
    let mut model = model(R18_SDMA_MAX_LINEAR_COPY_BYTES_V1 + 1);
    begin(
        &mut model,
        R18_SDMA_MAX_LINEAR_COPY_BYTES_V1 + 1,
        R18LocalSdmaDirectionV1::HostToDevice,
    );
    let plan = prepare(&mut model);
    publish(&mut model, &plan);
    let published = model.snapshot();
    assert_eq!(
        model.poll_window_model_only(R22WindowPollDispositionV1::Pending, None),
        Ok(R22WindowClassificationV1::Pending)
    );
    assert_eq!(model.snapshot(), published);
    assert_eq!(
        model.poll_window_model_only(R22WindowPollDispositionV1::TimedOut, None),
        Ok(R22WindowClassificationV1::TimedOut)
    );
    assert_eq!(model.snapshot(), published);
}

#[test]
fn partial_completion_never_releases_window_or_continuation() {
    let mut model = model(R18_SDMA_MAX_LINEAR_COPY_BYTES_V1 + 1);
    begin(
        &mut model,
        R18_SDMA_MAX_LINEAR_COPY_BYTES_V1 + 1,
        R18LocalSdmaDirectionV1::HostToDevice,
    );
    let plan = prepare(&mut model);
    publish(&mut model, &plan);
    assert_eq!(
        model.poll_window_model_only(
            R22WindowPollDispositionV1::Partial {
                completed_packets: 1,
            },
            None,
        ),
        Ok(R22WindowClassificationV1::Partial {
            completed_packets: 1,
        })
    );
    let state = model.snapshot();
    assert_eq!(state.phase, R22WindowPhaseV1::Published);
    assert_eq!(state.custody, Some(R22WindowCustodyKindV1::PublishedWindow));
    assert_eq!(state.observed_completed_packets, 1);
    assert_eq!(state.destination_dirty_through, 0);
    assert_eq!(
        model.prepare_window_model_only(&[]),
        Err(R22WindowErrorV1::InvalidPhase)
    );
}

#[test]
fn completion_metadata_substitution_fails_closed() {
    let mut model = model(4096);
    begin(&mut model, 4096, R18LocalSdmaDirectionV1::HostToDevice);
    let plan = prepare(&mut model);
    let mut metadata = publish(&mut model, &plan);
    metadata.plan.packets[0].ticket.generation += 1;
    assert_eq!(
        model.poll_window_model_only(
            R22WindowPollDispositionV1::Terminal(R18SdmaTerminalStatusV1::Succeeded),
            Some(&metadata),
        ),
        Ok(R22WindowClassificationV1::ProcessTeardown {
            point: R22WindowFailurePointV1::CompletionMetadata,
        })
    );
    assert_eq!(model.snapshot().authority_count, 1);
}

#[test]
fn recovered_postpublication_h2d_is_quiescent_and_retains_dirty_extent() {
    let mut model = model(4096);
    begin(&mut model, 4096, R18LocalSdmaDirectionV1::HostToDevice);
    let plan = prepare(&mut model);
    publish(&mut model, &plan);
    let expected = R22WindowQuiescentRecordV1 {
        transfer_id: TRANSFER_ID,
        completed_bytes: 0,
        total_bytes: 4096,
        possibly_mutated_through: 4096,
        host_possibly_mutated_through: 0,
    };
    assert_eq!(
        model.poll_window_model_only(R22WindowPollDispositionV1::RecoveredWithoutTerminal, None,),
        Ok(R22WindowClassificationV1::QuiescentWithoutResult(expected))
    );
    let state = model.snapshot();
    assert_eq!(state.custody, Some(R22WindowCustodyKindV1::Device));
    assert_eq!(state.quiescent, Some(expected));
    assert!(state.target_retained);
}

#[test]
fn recovered_postpublication_d2h_preserves_possible_host_mutation() {
    let mut model = model(4096);
    begin(&mut model, 4096, R18LocalSdmaDirectionV1::DeviceToHost);
    let plan = prepare(&mut model);
    publish(&mut model, &plan);
    match model
        .poll_window_model_only(R22WindowPollDispositionV1::RecoveredWithoutTerminal, None)
        .unwrap()
    {
        R22WindowClassificationV1::QuiescentWithoutResult(record) => {
            assert_eq!(record.completed_bytes, 0);
            assert_eq!(record.possibly_mutated_through, 4096);
            assert_eq!(record.host_possibly_mutated_through, 4096);
        }
        outcome => panic!("unexpected recovery: {outcome:?}"),
    }
    assert_eq!(model.snapshot().host_dirty_through, 0);
    assert_eq!(model.snapshot().host_possibly_mutated_through, 4096);
}

#[test]
fn exact_frontier_is_required_for_retirement() {
    let mut model = model(4096);
    begin(&mut model, 4096, R18LocalSdmaDirectionV1::HostToDevice);
    let plan = prepare(&mut model);
    let metadata = publish(&mut model, &plan);
    let mut frontier = match model
        .poll_window_model_only(
            R22WindowPollDispositionV1::Terminal(R18SdmaTerminalStatusV1::Succeeded),
            Some(&metadata),
        )
        .unwrap()
    {
        R22WindowClassificationV1::FrontierPending(frontier) => frontier,
        outcome => panic!("unexpected completion: {outcome:?}"),
    };
    frontier.plan.lease.pool_generation += 1;
    assert_eq!(
        model.retire_window_model_only(&frontier),
        Ok(R22WindowClassificationV1::ProcessTeardown {
            point: R22WindowFailurePointV1::Retirement,
        })
    );
    assert_eq!(
        model.snapshot().custody,
        Some(R22WindowCustodyKindV1::Opaque)
    );
}

#[test]
fn continuation_is_visible_only_after_exact_frontier_retirement() {
    let total = R22_SDMA_WINDOW_MAX_BYTES_V1 + 2048;
    let mut model = model(total);
    begin(&mut model, total, R18LocalSdmaDirectionV1::HostToDevice);
    let plan = prepare(&mut model);
    let metadata = publish(&mut model, &plan);
    let frontier = match model
        .poll_window_model_only(
            R22WindowPollDispositionV1::Terminal(R18SdmaTerminalStatusV1::Succeeded),
            Some(&metadata),
        )
        .unwrap()
    {
        R22WindowClassificationV1::FrontierPending(frontier) => frontier,
        outcome => panic!("unexpected completion: {outcome:?}"),
    };
    assert_eq!(
        model.prepare_window_model_only(&[]),
        Err(R22WindowErrorV1::InvalidPhase)
    );
    assert_eq!(
        model.retire_window_model_only(&frontier),
        Ok(R22WindowClassificationV1::ReadyContinuation {
            completed_bytes: R22_SDMA_WINDOW_MAX_BYTES_V1,
        })
    );
    assert_eq!(model.snapshot().retired_windows, 1);
    assert_eq!(model.snapshot().aggregate_lease_count, 0);
    assert!(matches!(
        model.prepare_window_model_only(&[]),
        Ok(R22WindowClassificationV1::Prepared(_))
    ));
}

#[test]
fn cancellation_is_only_before_any_window_progress() {
    let total = R22_SDMA_WINDOW_MAX_BYTES_V1 + 2048;
    let mut model = model(total);
    begin(&mut model, total, R18LocalSdmaDirectionV1::HostToDevice);
    assert_eq!(
        model.cancel_model_only(TRANSFER_ID),
        Ok(R22WindowClassificationV1::Released)
    );
    assert_eq!(model.snapshot().phase, R22WindowPhaseV1::DeviceReady);

    begin(&mut model, total, R18LocalSdmaDirectionV1::HostToDevice);
    complete_and_retire(&mut model, R18SdmaTerminalStatusV1::Succeeded);
    assert_eq!(
        model.cancel_model_only(TRANSFER_ID),
        Err(R22WindowErrorV1::InvalidTransfer)
    );
}

#[test]
fn full_256_mib_transfer_uses_two_windows_and_sixty_five_packets() {
    let total = 256 * 1024 * 1024;
    let mut model = model(total);
    begin(&mut model, total, R18LocalSdmaDirectionV1::HostToDevice);
    let first = prepare(&mut model);
    assert_eq!(first.packets.len(), 63);
    assert_eq!(first.byte_len, R22_SDMA_WINDOW_MAX_BYTES_V1);
    let first_metadata = publish(&mut model, &first);
    let first_frontier = match model
        .poll_window_model_only(
            R22WindowPollDispositionV1::Terminal(R18SdmaTerminalStatusV1::Succeeded),
            Some(&first_metadata),
        )
        .unwrap()
    {
        R22WindowClassificationV1::FrontierPending(frontier) => frontier,
        outcome => panic!("unexpected completion: {outcome:?}"),
    };
    model.retire_window_model_only(&first_frontier).unwrap();

    let second = prepare(&mut model);
    assert_eq!(second.packets.len(), 2);
    assert_eq!(second.packets[1].device_range.byte_len, 2048);
    let second_metadata = publish(&mut model, &second);
    let second_frontier = match model
        .poll_window_model_only(
            R22WindowPollDispositionV1::Terminal(R18SdmaTerminalStatusV1::Succeeded),
            Some(&second_metadata),
        )
        .unwrap()
    {
        R22WindowClassificationV1::FrontierPending(frontier) => frontier,
        outcome => panic!("unexpected completion: {outcome:?}"),
    };
    assert_eq!(
        model.retire_window_model_only(&second_frontier),
        Ok(R22WindowClassificationV1::Completed(
            R22WindowCompletionRecordV1 {
                transfer_id: TRANSFER_ID,
                succeeded: true,
                failure_code: None,
                completed_bytes: total,
            }
        ))
    );
    let state = model.snapshot();
    assert_eq!(state.published_windows, 2);
    assert_eq!(state.published_packets, 65);
    assert_eq!(state.write_pointer_publications, 2);
    assert_eq!(state.doorbell_publications, 2);
    assert_eq!(state.retired_windows, 2);
}

#[test]
fn failed_terminal_is_exact_and_release_allows_mixed_direction_reuse() {
    let mut model = model(4096);
    begin(&mut model, 4096, R18LocalSdmaDirectionV1::HostToDevice);
    assert_eq!(
        complete_and_retire(&mut model, R18SdmaTerminalStatusV1::Failed { code: -9 }),
        R22WindowClassificationV1::Completed(R22WindowCompletionRecordV1 {
            transfer_id: TRANSFER_ID,
            succeeded: false,
            failure_code: Some(-9),
            completed_bytes: 0,
        })
    );
    assert_eq!(
        model.poll_submission_model_only(TRANSFER_ID),
        Ok(R22WindowClassificationV1::Completed(
            R22WindowCompletionRecordV1 {
                transfer_id: TRANSFER_ID,
                succeeded: false,
                failure_code: Some(-9),
                completed_bytes: 0,
            }
        ))
    );
    model.release_submission_model_only(TRANSFER_ID).unwrap();
    begin(&mut model, 4096, R18LocalSdmaDirectionV1::DeviceToHost);
    assert_eq!(
        complete_and_retire(&mut model, R18SdmaTerminalStatusV1::Succeeded),
        R22WindowClassificationV1::Completed(R22WindowCompletionRecordV1 {
            transfer_id: TRANSFER_ID,
            succeeded: true,
            failure_code: None,
            completed_bytes: 4096,
        })
    );
    assert_eq!(model.snapshot().host_dirty_through, 4096);
}
