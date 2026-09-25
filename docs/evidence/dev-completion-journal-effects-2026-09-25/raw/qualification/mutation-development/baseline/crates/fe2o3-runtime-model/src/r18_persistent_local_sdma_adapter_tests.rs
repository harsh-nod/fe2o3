use alloc::vec;

use super::*;

const MIB: u64 = 1024 * 1024;

fn device(physical: u64, generation: u64) -> DeviceKeyV1 {
    DeviceKeyV1 {
        physical: PhysicalDeviceIdV1(physical),
        generation: DeviceGenerationV1(generation),
    }
}

fn devices() -> [DeviceKeyV1; 2] {
    [device(0x100, 3), device(0x200, 4)]
}

fn vm() -> VmKeyV1 {
    VmKeyV1 {
        device: devices()[0],
        id: VmIdV1(11),
    }
}

fn logical_queue(id: u64, generation: u64) -> QueueKeyV1 {
    QueueKeyV1 {
        vm: vm(),
        id: QueueInstanceIdV1(id),
        generation: QueueGenerationV1(generation),
    }
}

fn queue(engine_id: u32) -> R18LocalSdmaQueueOccurrenceV1 {
    R18LocalSdmaQueueOccurrenceV1 {
        logical_queue: logical_queue(21, 22),
        native_queue_id: 0,
        occurrence: 23,
        engine_id,
    }
}

fn local_admission() -> R18LocalPersistentAllocationAdmissionV1 {
    let allocation_key = MemoryAllocationKeyV1 {
        vm: vm(),
        id: AllocationIdV1(12),
        generation: AllocationGenerationV1(13),
    };
    let allocation = MemoryAllocationRecordV1 {
        key: allocation_key,
        reservation: VaReservationKeyV1 {
            vm: vm(),
            id: VaReservationIdV1(14),
        },
        handle: UntrustedAllocationHandleObservationV1(15),
        spec: MemoryAllocationSpecV1 {
            byte_len: 8 * MIB,
            alignment: MEMORY_PAGE_BYTES_V1,
            kind: MemoryKindV1::DeviceLocal,
            coherence: MemoryCoherenceV1::ExplicitVisibility,
        },
        state: MemoryAllocationStateV1::Live,
    };
    let mapping = MemoryMappingRecordV1 {
        key: MemoryMappingKeyV1 {
            allocation: allocation_key,
            id: MappingIdV1(16),
        },
        target_devices: vec![devices()[0]],
        access: MemoryAccessV1::ReadWrite,
        mapped_start: 0,
        mapped_end: 1,
        state: MemoryMappingStateV1::Mapped,
    };
    R18LocalPersistentAllocationAdmissionV1 {
        owner: R17PersistentAllocationOwnerIdV1(1),
        allocation,
        mapping,
        device: devices()[0],
    }
}

fn adapter(engine_id: u32) -> R18PersistentLocalSdmaAdapterV1 {
    R18PersistentLocalSdmaAdapterV1::new_local_model_only(local_admission(), queue(engine_id))
        .unwrap()
}

fn host(id: u64, generation: u64) -> R18HostBufferKeyV1 {
    R18HostBufferKeyV1 {
        session_id: 30,
        id,
        generation,
        byte_len: 4 * MIB,
        coherence: MemoryCoherenceV1::HostCoherent,
    }
}

fn range(offset: u64, len: u64) -> R18ByteRangeV1 {
    R18ByteRangeV1 {
        byte_offset: offset,
        byte_len: len,
    }
}

fn ticket(
    queue: R18LocalSdmaQueueOccurrenceV1,
    slot: u16,
    generation: u32,
) -> R18PlannedSdmaTicketV1 {
    R18PlannedSdmaTicketV1 {
        owner: queue.logical_queue,
        queue_id: queue.native_queue_id,
        slot,
        generation,
    }
}

fn prepare(
    adapter: &mut R18PersistentLocalSdmaAdapterV1,
    direction: R18LocalSdmaDirectionV1,
) -> R18PreparedPersistentLocalSdmaLeaseV1 {
    let ticket = ticket(adapter.queue(), 0, 1);
    adapter
        .prepare_model_only(
            direction,
            range(64, 256),
            host(31, 32),
            range(128, 256),
            ticket,
        )
        .unwrap()
}

fn publish(
    adapter: &mut R18PersistentLocalSdmaAdapterV1,
    prepared: R18PreparedPersistentLocalSdmaLeaseV1,
) -> R18PublishedPersistentLocalSdmaLeaseV1 {
    let binding = prepared.binding();
    match prepared
        .resolve_publication_model_only(
            adapter,
            R18PublicationObservationV1 {
                binding,
                resolution: R18PublicationResolutionV1::Confirmed,
            },
        )
        .unwrap()
    {
        R18PublicationOutcomeV1::Published(lease) => lease,
        _ => panic!("confirmed publication must retain published custody"),
    }
}

fn complete(
    adapter: &mut R18PersistentLocalSdmaAdapterV1,
    published: R18PublishedPersistentLocalSdmaLeaseV1,
    status: R18SdmaTerminalStatusV1,
) -> R18CompletedPersistentLocalSdmaLeaseV1 {
    let binding = published.binding();
    match published
        .observe_model_only(
            adapter,
            R18CompletionObservationV1 {
                binding,
                resolution: R18CompletionResolutionV1::Terminal(status),
            },
        )
        .unwrap()
    {
        R18PublishedPollV1::Completed(lease) => lease,
        _ => panic!("terminal observation must retain completion custody"),
    }
}

fn restore(
    adapter: &mut R18PersistentLocalSdmaAdapterV1,
    completed: R18CompletedPersistentLocalSdmaLeaseV1,
) -> R18RestoredPersistentLocalSdmaLeaseV1 {
    let binding = completed.binding();
    let status = completed.status();
    match completed
        .restore_model_only(
            adapter,
            R18RestoreObservationV1 {
                binding,
                status,
                queue_current: true,
            },
        )
        .unwrap()
    {
        R18RestoreOutcomeV1::Restored(lease) => lease,
        _ => panic!("current queue must restore persistent custody"),
    }
}

fn settle(
    adapter: &mut R18PersistentLocalSdmaAdapterV1,
    restored: R18RestoredPersistentLocalSdmaLeaseV1,
    status: R18SdmaTerminalStatusV1,
) -> R18SettledFrontierV1 {
    let binding = restored.binding();
    restored
        .settle_model_only(adapter, R18SettlementObservationV1 { binding, status })
        .unwrap()
}

fn retire(
    adapter: &mut R18PersistentLocalSdmaAdapterV1,
    frontier: R18SettledFrontierV1,
) -> R18FrontierRetirementReceiptV1 {
    let observed = frontier.key();
    frontier.retire_model_only(adapter, observed).unwrap()
}

#[test]
fn host_to_device_fixes_write_destination_and_settles_before_release() {
    let mut adapter = adapter(R18_LOCAL_SDMA_HOST_TO_DEVICE_ENGINE_V1);
    let prepared = prepare(&mut adapter, R18LocalSdmaDirectionV1::HostToDevice);
    let binding = prepared.binding();
    assert_eq!(binding.persistent_access, R17PersistentAccessModeV1::Write);
    assert_eq!(
        binding.persistent_endpoint,
        R18PersistentEndpointV1::Destination
    );
    assert_eq!(binding.persistent_descriptor().range.byte_offset, 64);
    assert_eq!(binding.ticket.queue_id, 0);
    assert_eq!(
        adapter.snapshot().native_location,
        R18PersistentNativeLocationV1::PreparedBatch
    );
    assert_eq!(
        adapter.active_persistent_use_record().unwrap().phase,
        R17PersistentUsePhaseV1::Reserved
    );

    let published = publish(&mut adapter, prepared);
    assert_eq!(
        adapter.snapshot().native_location,
        R18PersistentNativeLocationV1::NativeQueue
    );
    assert_eq!(
        adapter.active_persistent_use_record().unwrap().phase,
        R17PersistentUsePhaseV1::Published
    );
    let completed = complete(&mut adapter, published, R18SdmaTerminalStatusV1::Succeeded);
    assert_eq!(
        adapter.snapshot().native_location,
        R18PersistentNativeLocationV1::CompletionBatch
    );
    assert_eq!(
        adapter.active_persistent_use_record().unwrap().phase,
        R17PersistentUsePhaseV1::Terminal
    );
    let restored = restore(&mut adapter, completed);
    assert_eq!(
        adapter.snapshot().active_phase,
        Some(R18PersistentLocalSdmaPhaseV1::Restored)
    );
    let frontier = settle(&mut adapter, restored, R18SdmaTerminalStatusV1::Succeeded);
    assert_eq!(
        frontier.key().persistent_frontier.through_use,
        binding.persistent_use.lease
    );
    assert_eq!(adapter.snapshot().settled_transfer_count, 1);
    assert_eq!(
        adapter.active_persistent_use_record().unwrap().phase,
        R17PersistentUsePhaseV1::Settled
    );
    assert_eq!(adapter.snapshot().pending_frontier, Some(frontier.key()));
    let retirement = retire(&mut adapter, frontier);
    assert_eq!(retirement.retired_use_count, 1);
    assert!(adapter.active_persistent_use_record().is_none());
    let release = adapter.release_model_only().unwrap();
    assert_eq!(release.completed_lease_count, 1);
}

#[test]
fn device_to_host_accepts_native_queue_zero_and_fixes_read_source() {
    let mut adapter = adapter(R18_LOCAL_SDMA_DEVICE_TO_HOST_ENGINE_V1);
    let prepared = prepare(&mut adapter, R18LocalSdmaDirectionV1::DeviceToHost);
    let binding = prepared.binding();
    assert_eq!(binding.queue.native_queue_id, 0);
    assert_eq!(binding.persistent_access, R17PersistentAccessModeV1::Read);
    assert_eq!(binding.persistent_endpoint, R18PersistentEndpointV1::Source);
    assert_eq!(
        binding.persistent_descriptor().class,
        R17PersistentUseClassV1::LocalSdma {
            device: devices()[0],
            queue: logical_queue(21, 22),
            engine_id: 0,
        }
    );
}

#[test]
fn direction_is_exactly_bound_to_targeted_engine() {
    let mut d2h = adapter(R18_LOCAL_SDMA_DEVICE_TO_HOST_ENGINE_V1);
    let d2h_ticket = ticket(d2h.queue(), 0, 1);
    assert_eq!(
        d2h.prepare_model_only(
            R18LocalSdmaDirectionV1::HostToDevice,
            range(0, 64),
            host(31, 32),
            range(0, 64),
            d2h_ticket,
        )
        .err(),
        Some(R18PersistentLocalSdmaErrorV1::WrongDirection)
    );
    let mut h2d = adapter(R18_LOCAL_SDMA_HOST_TO_DEVICE_ENGINE_V1);
    let h2d_ticket = ticket(h2d.queue(), 0, 1);
    assert_eq!(
        h2d.prepare_model_only(
            R18LocalSdmaDirectionV1::DeviceToHost,
            range(0, 64),
            host(31, 32),
            range(0, 64),
            h2d_ticket,
        )
        .err(),
        Some(R18PersistentLocalSdmaErrorV1::WrongDirection)
    );
}

#[test]
fn bounds_lengths_overflow_and_host_identity_are_checked() {
    let cases = [
        (range(0, 0), host(31, 32), range(0, 0)),
        (range(8 * MIB - 32, 64), host(31, 32), range(0, 64)),
        (range(u64::MAX - 1, 4), host(31, 32), range(0, 4)),
        (range(0, 64), host(31, 32), range(4 * MIB - 32, 64)),
        (range(0, 4), host(31, 32), range(u64::MAX - 1, 4)),
        (range(0, 64), host(31, 32), range(0, 32)),
    ];
    for (device_range, host, host_range) in cases {
        let mut adapter = adapter(R18_LOCAL_SDMA_HOST_TO_DEVICE_ENGINE_V1);
        let ticket = ticket(adapter.queue(), 0, 1);
        assert_eq!(
            adapter
                .prepare_model_only(
                    R18LocalSdmaDirectionV1::HostToDevice,
                    device_range,
                    host,
                    host_range,
                    ticket,
                )
                .err(),
            Some(R18PersistentLocalSdmaErrorV1::InvalidRange)
        );
    }
    for invalid_host in [
        host(0, 1),
        host(1, 0),
        R18HostBufferKeyV1 {
            id: 1,
            generation: 1,
            byte_len: 0,
            ..host(1, 1)
        },
        R18HostBufferKeyV1 {
            session_id: 0,
            ..host(1, 1)
        },
        R18HostBufferKeyV1 {
            coherence: MemoryCoherenceV1::ExplicitVisibility,
            ..host(1, 1)
        },
    ] {
        let mut adapter = adapter(R18_LOCAL_SDMA_HOST_TO_DEVICE_ENGINE_V1);
        let ticket = ticket(adapter.queue(), 0, 1);
        assert_eq!(
            adapter
                .prepare_model_only(
                    R18LocalSdmaDirectionV1::HostToDevice,
                    range(0, 64),
                    invalid_host,
                    range(0, 64),
                    ticket,
                )
                .err(),
            Some(R18PersistentLocalSdmaErrorV1::InvalidHostBuffer)
        );
    }
}

#[test]
fn packet_copy_limit_and_concrete_planned_ticket_shape_are_exact() {
    let mut exact = adapter(R18_LOCAL_SDMA_HOST_TO_DEVICE_ENGINE_V1);
    let exact_ticket = ticket(exact.queue(), R18_SDMA_RING_SLOT_COUNT_V1 - 1, u32::MAX);
    assert!(
        exact
            .prepare_model_only(
                R18LocalSdmaDirectionV1::HostToDevice,
                range(0, R18_SDMA_MAX_LINEAR_COPY_BYTES_V1),
                host(31, 32),
                range(0, R18_SDMA_MAX_LINEAR_COPY_BYTES_V1),
                exact_ticket,
            )
            .is_ok()
    );

    let mut too_large = adapter(R18_LOCAL_SDMA_HOST_TO_DEVICE_ENGINE_V1);
    let too_large_ticket = ticket(too_large.queue(), 0, 1);
    assert_eq!(
        too_large
            .prepare_model_only(
                R18LocalSdmaDirectionV1::HostToDevice,
                range(0, R18_SDMA_MAX_LINEAR_COPY_BYTES_V1 + 1),
                R18HostBufferKeyV1 {
                    byte_len: 8 * MIB,
                    ..host(31, 32)
                },
                range(0, R18_SDMA_MAX_LINEAR_COPY_BYTES_V1 + 1),
                too_large_ticket,
            )
            .err(),
        Some(R18PersistentLocalSdmaErrorV1::InvalidRange)
    );

    let canonical_queue = queue(R18_LOCAL_SDMA_HOST_TO_DEVICE_ENGINE_V1);
    let mut invalid_tickets = [
        ticket(canonical_queue, R18_SDMA_RING_SLOT_COUNT_V1, 1),
        ticket(canonical_queue, 0, 0),
        R18PlannedSdmaTicketV1 {
            owner: logical_queue(99, 22),
            ..ticket(canonical_queue, 0, 1)
        },
        R18PlannedSdmaTicketV1 {
            queue_id: 1,
            ..ticket(canonical_queue, 0, 1)
        },
    ];
    for invalid_ticket in &mut invalid_tickets {
        let mut adapter = adapter(R18_LOCAL_SDMA_HOST_TO_DEVICE_ENGINE_V1);
        assert_eq!(
            adapter
                .prepare_model_only(
                    R18LocalSdmaDirectionV1::HostToDevice,
                    range(0, 64),
                    host(31, 32),
                    range(0, 64),
                    *invalid_ticket,
                )
                .err(),
            Some(R18PersistentLocalSdmaErrorV1::StaleBinding)
        );
        assert_eq!(adapter.snapshot().active_phase, None);
    }
}

#[test]
fn single_flight_and_release_are_gated_until_exact_settlement() {
    let mut adapter = adapter(R18_LOCAL_SDMA_HOST_TO_DEVICE_ENGINE_V1);
    let prepared = prepare(&mut adapter, R18LocalSdmaDirectionV1::HostToDevice);
    let next_ticket = ticket(adapter.queue(), 1, 1);
    assert_eq!(
        adapter
            .prepare_model_only(
                R18LocalSdmaDirectionV1::HostToDevice,
                range(0, 64),
                host(41, 42),
                range(0, 64),
                next_ticket,
            )
            .err(),
        Some(R18PersistentLocalSdmaErrorV1::Busy)
    );
    let failure = adapter.release_model_only().err().unwrap();
    assert_eq!(failure.error(), R18PersistentLocalSdmaErrorV1::Busy);
    let (_, mut adapter) = failure.into_parts();
    let published = publish(&mut adapter, prepared);
    let completed = complete(&mut adapter, published, R18SdmaTerminalStatusV1::Succeeded);
    let restored = restore(&mut adapter, completed);
    let failure = adapter.release_model_only().err().unwrap();
    assert_eq!(failure.error(), R18PersistentLocalSdmaErrorV1::Busy);
    let (_, mut adapter) = failure.into_parts();
    let frontier = settle(&mut adapter, restored, R18SdmaTerminalStatusV1::Succeeded);
    let failure = adapter.release_model_only().err().unwrap();
    assert_eq!(failure.error(), R18PersistentLocalSdmaErrorV1::Busy);
    let (_, mut adapter) = failure.into_parts();
    retire(&mut adapter, frontier);
    assert!(adapter.release_model_only().is_ok());
}

#[test]
fn only_before_queue_custody_is_recoverable_and_ticket_is_not_reused() {
    let mut adapter = adapter(R18_LOCAL_SDMA_HOST_TO_DEVICE_ENGINE_V1);
    let prepared = prepare(&mut adapter, R18LocalSdmaDirectionV1::HostToDevice);
    let first = prepared.binding();
    match prepared
        .resolve_publication_model_only(
            &mut adapter,
            R18PublicationObservationV1 {
                binding: first,
                resolution: R18PublicationResolutionV1::RecoverableFailure {
                    point: R18PrepublicationFailurePointV1::BeforeQueueCustody,
                },
            },
        )
        .unwrap()
    {
        R18PublicationOutcomeV1::Restored(receipt) => assert_eq!(receipt.binding, first),
        _ => panic!("recoverable failure must restore ownership"),
    }
    assert_eq!(
        adapter.snapshot().native_location,
        R18PersistentNativeLocationV1::PersistentAllocation
    );
    let second = prepare(&mut adapter, R18LocalSdmaDirectionV1::HostToDevice);
    assert!(
        second.binding().persistent_use.lease.generation > first.persistent_use.lease.generation
    );

    let binding = second.binding();
    let failure = second
        .resolve_publication_model_only(
            &mut adapter,
            R18PublicationObservationV1 {
                binding,
                resolution: R18PublicationResolutionV1::RecoverableFailure {
                    point: R18PrepublicationFailurePointV1::PacketWrite,
                },
            },
        )
        .err()
        .unwrap();
    assert_eq!(
        failure.error(),
        R18PersistentLocalSdmaErrorV1::IllegalFailureClassification
    );
    assert_eq!(
        adapter.snapshot().active_phase,
        Some(R18PersistentLocalSdmaPhaseV1::Prepared)
    );
}

#[test]
fn every_post_custody_prepublication_failure_quarantines_without_fake_publication() {
    let points = [
        R18PrepublicationFailurePointV1::CompletionReset,
        R18PrepublicationFailurePointV1::RingReservation,
        R18PrepublicationFailurePointV1::PacketWrite,
        R18PrepublicationFailurePointV1::WritePointer,
        R18PrepublicationFailurePointV1::Doorbell,
    ];
    for point in points {
        let mut adapter = adapter(R18_LOCAL_SDMA_HOST_TO_DEVICE_ENGINE_V1);
        let prepared = prepare(&mut adapter, R18LocalSdmaDirectionV1::HostToDevice);
        let binding = prepared.binding();
        let quarantine = match prepared
            .resolve_publication_model_only(
                &mut adapter,
                R18PublicationObservationV1 {
                    binding,
                    resolution: R18PublicationResolutionV1::IndeterminateRetention { point },
                },
            )
            .unwrap()
        {
            R18PublicationOutcomeV1::Quarantined(lease) => lease,
            _ => panic!("retained ambiguity must quarantine directly"),
        };
        assert_eq!(
            quarantine.reason(),
            R18QuarantineReasonV1::PublicationIndeterminate(point)
        );
        assert_eq!(
            adapter.snapshot().active_phase,
            Some(R18PersistentLocalSdmaPhaseV1::Quarantined)
        );
        assert_eq!(
            adapter.snapshot().native_location,
            R18PersistentNativeLocationV1::Quarantine
        );
    }
}

#[test]
fn pending_timeout_resume_terminal_restore_and_settle_keep_custody() {
    let mut adapter = adapter(R18_LOCAL_SDMA_HOST_TO_DEVICE_ENGINE_V1);
    let prepared = prepare(&mut adapter, R18LocalSdmaDirectionV1::HostToDevice);
    let published = publish(&mut adapter, prepared);
    let binding = published.binding();
    let published = match published
        .observe_model_only(
            &mut adapter,
            R18CompletionObservationV1 {
                binding,
                resolution: R18CompletionResolutionV1::Pending,
            },
        )
        .unwrap()
    {
        R18PublishedPollV1::Pending(lease) => lease,
        _ => panic!("pending observation must preserve published custody"),
    };
    let timed_out = match published
        .observe_model_only(
            &mut adapter,
            R18CompletionObservationV1 {
                binding,
                resolution: R18CompletionResolutionV1::TimedOut,
            },
        )
        .unwrap()
    {
        R18PublishedPollV1::TimedOut(lease) => lease,
        _ => panic!("timeout must preserve native queue custody"),
    };
    assert_eq!(
        adapter.snapshot().native_location,
        R18PersistentNativeLocationV1::NativeQueue
    );
    let timed_out = match timed_out
        .observe_model_only(
            &mut adapter,
            R18CompletionObservationV1 {
                binding,
                resolution: R18CompletionResolutionV1::Pending,
            },
        )
        .unwrap()
    {
        R18TimedOutPollV1::TimedOut(lease) => lease,
        _ => panic!("pending after timeout remains timed out"),
    };
    let completed = match timed_out
        .observe_model_only(
            &mut adapter,
            R18CompletionObservationV1 {
                binding,
                resolution: R18CompletionResolutionV1::Terminal(R18SdmaTerminalStatusV1::Succeeded),
            },
        )
        .unwrap()
    {
        R18TimedOutPollV1::Completed(lease) => lease,
        _ => panic!("terminal resume must return completion custody"),
    };
    let restored = restore(&mut adapter, completed);
    let frontier = settle(&mut adapter, restored, R18SdmaTerminalStatusV1::Succeeded);
    retire(&mut adapter, frontier);
}

#[test]
fn exact_binding_rejects_ticket_host_allocation_and_queue_substitution() {
    let mut adapter = adapter(R18_LOCAL_SDMA_HOST_TO_DEVICE_ENGINE_V1);
    let prepared = prepare(&mut adapter, R18LocalSdmaDirectionV1::HostToDevice);
    let expected = prepared.binding();
    let mut substitutions = vec![];
    let mut changed = expected;
    changed.ticket.generation += 1;
    substitutions.push(changed);
    changed = expected;
    changed.ticket.slot += 1;
    substitutions.push(changed);
    changed = expected;
    changed.ticket.owner.generation.0 += 1;
    substitutions.push(changed);
    changed = expected;
    changed.host.generation += 1;
    substitutions.push(changed);
    changed = expected;
    changed.host.session_id += 1;
    substitutions.push(changed);
    changed = expected;
    changed.host.coherence = MemoryCoherenceV1::ExplicitVisibility;
    substitutions.push(changed);
    changed = expected;
    changed.allocation.allocation.generation.0 += 1;
    substitutions.push(changed);
    changed = expected;
    changed.queue.logical_queue.generation.0 += 1;
    substitutions.push(changed);
    changed = expected;
    changed.queue.occurrence += 1;
    substitutions.push(changed);
    changed = expected;
    changed.queue.native_queue_id += 1;
    substitutions.push(changed);
    changed = expected;
    changed.attachment_generation += 1;
    substitutions.push(changed);

    let mut retained = prepared;
    for binding in substitutions {
        let failure = retained
            .resolve_publication_model_only(
                &mut adapter,
                R18PublicationObservationV1 {
                    binding,
                    resolution: R18PublicationResolutionV1::Confirmed,
                },
            )
            .err()
            .unwrap();
        assert_eq!(
            failure.error(),
            R18PersistentLocalSdmaErrorV1::ObservationMismatch
        );
        retained = failure.into_parts().1;
    }
    assert_eq!(retained.binding(), expected);
}

#[test]
fn queue_rebind_increments_attachment_and_rejects_aba_observation() {
    let mut adapter = adapter(R18_LOCAL_SDMA_HOST_TO_DEVICE_ENGINE_V1);
    let first = prepare(&mut adapter, R18LocalSdmaDirectionV1::HostToDevice);
    let stale = first.binding();
    match first
        .resolve_publication_model_only(
            &mut adapter,
            R18PublicationObservationV1 {
                binding: stale,
                resolution: R18PublicationResolutionV1::RecoverableFailure {
                    point: R18PrepublicationFailurePointV1::BeforeQueueCustody,
                },
            },
        )
        .unwrap()
    {
        R18PublicationOutcomeV1::Restored(_) => {}
        _ => panic!("expected restoration"),
    }
    let replacement = R18LocalSdmaQueueOccurrenceV1 {
        logical_queue: logical_queue(21, 23),
        native_queue_id: 0,
        occurrence: 24,
        engine_id: R18_LOCAL_SDMA_HOST_TO_DEVICE_ENGINE_V1,
    };
    let receipt = adapter.rebind_queue_model_only(replacement).unwrap();
    assert_eq!(receipt.attachment_generation, 2);
    let current = prepare(&mut adapter, R18LocalSdmaDirectionV1::HostToDevice);
    let failure = current
        .resolve_publication_model_only(
            &mut adapter,
            R18PublicationObservationV1 {
                binding: stale,
                resolution: R18PublicationResolutionV1::Confirmed,
            },
        )
        .err()
        .unwrap();
    assert_eq!(
        failure.error(),
        R18PersistentLocalSdmaErrorV1::ObservationMismatch
    );
}

#[test]
fn identical_numeric_reconstruction_cannot_use_foreign_transition_token() {
    let mut first_adapter = adapter(R18_LOCAL_SDMA_HOST_TO_DEVICE_ENGINE_V1);
    let foreign = prepare(&mut first_adapter, R18LocalSdmaDirectionV1::HostToDevice);
    let binding = foreign.binding();
    let mut reconstructed = adapter(R18_LOCAL_SDMA_HOST_TO_DEVICE_ENGINE_V1);
    let _local = prepare(&mut reconstructed, R18LocalSdmaDirectionV1::HostToDevice);
    assert_eq!(
        reconstructed.snapshot().allocation,
        first_adapter.snapshot().allocation
    );
    let failure = foreign
        .resolve_publication_model_only(
            &mut reconstructed,
            R18PublicationObservationV1 {
                binding,
                resolution: R18PublicationResolutionV1::Confirmed,
            },
        )
        .err()
        .unwrap();
    assert_eq!(failure.error(), R18PersistentLocalSdmaErrorV1::WrongAdapter);
}

#[test]
fn mismatched_terminal_restore_and_settlement_observations_retain_state() {
    let mut adapter = adapter(R18_LOCAL_SDMA_HOST_TO_DEVICE_ENGINE_V1);
    let prepared = prepare(&mut adapter, R18LocalSdmaDirectionV1::HostToDevice);
    let published = publish(&mut adapter, prepared);
    let completed = complete(&mut adapter, published, R18SdmaTerminalStatusV1::Succeeded);
    let binding = completed.binding();
    let failure = completed
        .restore_model_only(
            &mut adapter,
            R18RestoreObservationV1 {
                binding,
                status: R18SdmaTerminalStatusV1::Failed { code: -1 },
                queue_current: true,
            },
        )
        .err()
        .unwrap();
    assert_eq!(
        failure.error(),
        R18PersistentLocalSdmaErrorV1::ObservationMismatch
    );
    let completed = failure.into_parts().1;
    let restored = restore(&mut adapter, completed);
    let failure = restored
        .settle_model_only(
            &mut adapter,
            R18SettlementObservationV1 {
                binding,
                status: R18SdmaTerminalStatusV1::Failed { code: -2 },
            },
        )
        .err()
        .unwrap();
    assert_eq!(
        failure.error(),
        R18PersistentLocalSdmaErrorV1::ObservationMismatch
    );
    assert_eq!(
        adapter.snapshot().active_phase,
        Some(R18PersistentLocalSdmaPhaseV1::Restored)
    );
}

#[test]
fn currentness_ambiguity_quarantines_at_preparation_completion_and_restore() {
    let mut before_publish = adapter(R18_LOCAL_SDMA_HOST_TO_DEVICE_ENGINE_V1);
    let prepared = prepare(&mut before_publish, R18LocalSdmaDirectionV1::HostToDevice);
    let binding = prepared.binding();
    match prepared
        .resolve_publication_model_only(
            &mut before_publish,
            R18PublicationObservationV1 {
                binding,
                resolution: R18PublicationResolutionV1::CurrentnessAmbiguous,
            },
        )
        .unwrap()
    {
        R18PublicationOutcomeV1::Quarantined(lease) => {
            assert_eq!(
                lease.reason(),
                R18QuarantineReasonV1::QueueCurrentnessAmbiguous
            )
        }
        _ => panic!("ambiguous currentness must quarantine"),
    }

    let mut during_completion = adapter(R18_LOCAL_SDMA_HOST_TO_DEVICE_ENGINE_V1);
    let prepared = prepare(
        &mut during_completion,
        R18LocalSdmaDirectionV1::HostToDevice,
    );
    let published = publish(&mut during_completion, prepared);
    let binding = published.binding();
    match published
        .observe_model_only(
            &mut during_completion,
            R18CompletionObservationV1 {
                binding,
                resolution: R18CompletionResolutionV1::CurrentnessAmbiguous,
            },
        )
        .unwrap()
    {
        R18PublishedPollV1::Quarantined(lease) => assert_eq!(
            lease.reason(),
            R18QuarantineReasonV1::CompletionCurrentnessAmbiguous
        ),
        _ => panic!("ambiguous completion currentness must quarantine"),
    }

    let mut during_restore = adapter(R18_LOCAL_SDMA_HOST_TO_DEVICE_ENGINE_V1);
    let prepared = prepare(&mut during_restore, R18LocalSdmaDirectionV1::HostToDevice);
    let published = publish(&mut during_restore, prepared);
    let completed = complete(
        &mut during_restore,
        published,
        R18SdmaTerminalStatusV1::Failed { code: 7 },
    );
    let binding = completed.binding();
    match completed
        .restore_model_only(
            &mut during_restore,
            R18RestoreObservationV1 {
                binding,
                status: R18SdmaTerminalStatusV1::Failed { code: 7 },
                queue_current: false,
            },
        )
        .unwrap()
    {
        R18RestoreOutcomeV1::Quarantined(lease) => assert_eq!(
            lease.reason(),
            R18QuarantineReasonV1::RestoreCurrentnessAmbiguous
        ),
        _ => panic!("ambiguous restore currentness must quarantine"),
    }
}

#[test]
fn local_admission_rejects_mapping_substitution_and_queue_boundaries() {
    let invalid_queues = [
        R18LocalSdmaQueueOccurrenceV1 {
            occurrence: 0,
            ..queue(0)
        },
        R18LocalSdmaQueueOccurrenceV1 {
            engine_id: 2,
            ..queue(0)
        },
        R18LocalSdmaQueueOccurrenceV1 {
            logical_queue: QueueKeyV1 {
                vm: VmKeyV1 {
                    id: VmIdV1(99),
                    ..vm()
                },
                ..logical_queue(21, 22)
            },
            ..queue(0)
        },
        R18LocalSdmaQueueOccurrenceV1 {
            native_queue_id: R18_KFD_PROCESS_QUEUE_ID_LIMIT_V1,
            ..queue(0)
        },
    ];
    for invalid_queue in invalid_queues {
        let admission = local_admission();
        let retained = admission.clone();
        let failure =
            R18PersistentLocalSdmaAdapterV1::new_local_model_only(admission, invalid_queue)
                .err()
                .unwrap();
        assert_eq!(failure.error(), R18PersistentLocalSdmaErrorV1::InvalidQueue);
        assert_eq!(failure.into_parts().1, retained);
    }

    let mut upper = queue(0);
    upper.native_queue_id = R18_KFD_PROCESS_QUEUE_ID_LIMIT_V1 - 1;
    assert!(
        R18PersistentLocalSdmaAdapterV1::new_local_model_only(local_admission(), upper).is_ok()
    );

    let mut nonlocal = local_admission();
    nonlocal.mapping.target_devices = devices().to_vec();
    nonlocal.mapping.mapped_end = 2;
    let retained = nonlocal.clone();
    let failure = R18PersistentLocalSdmaAdapterV1::new_local_model_only(nonlocal, queue(0))
        .err()
        .unwrap();
    assert_eq!(
        failure.error(),
        R18PersistentLocalSdmaErrorV1::InvalidAllocation
    );
    assert_eq!(failure.into_parts().1, retained);

    let mut wrong_device = local_admission();
    wrong_device.device = devices()[1];
    assert_eq!(
        R18PersistentLocalSdmaAdapterV1::new_local_model_only(wrong_device, queue(0))
            .err()
            .unwrap()
            .error(),
        R18PersistentLocalSdmaErrorV1::InvalidAllocation
    );
}

#[test]
fn indeterminate_before_queue_custody_is_rejected_without_state_change() {
    let mut adapter = adapter(R18_LOCAL_SDMA_HOST_TO_DEVICE_ENGINE_V1);
    let prepared = prepare(&mut adapter, R18LocalSdmaDirectionV1::HostToDevice);
    let binding = prepared.binding();
    let failure = prepared
        .resolve_publication_model_only(
            &mut adapter,
            R18PublicationObservationV1 {
                binding,
                resolution: R18PublicationResolutionV1::IndeterminateRetention {
                    point: R18PrepublicationFailurePointV1::BeforeQueueCustody,
                },
            },
        )
        .err()
        .unwrap();
    assert_eq!(
        failure.error(),
        R18PersistentLocalSdmaErrorV1::IllegalFailureClassification
    );
    assert_eq!(
        adapter.snapshot().native_location,
        R18PersistentNativeLocationV1::PreparedBatch
    );
}

#[test]
fn failed_terminal_can_restore_and_ticket_aba_cannot_complete_reused_binding() {
    let mut adapter = adapter(R18_LOCAL_SDMA_DEVICE_TO_HOST_ENGINE_V1);
    let first_prepared = prepare(&mut adapter, R18LocalSdmaDirectionV1::DeviceToHost);
    let first_binding = first_prepared.binding();
    let first_published = publish(&mut adapter, first_prepared);
    let first_completed = complete(
        &mut adapter,
        first_published,
        R18SdmaTerminalStatusV1::Failed { code: -5 },
    );
    let first_restored = restore(&mut adapter, first_completed);
    let frontier = settle(
        &mut adapter,
        first_restored,
        R18SdmaTerminalStatusV1::Failed { code: -5 },
    );
    retire(&mut adapter, frontier);

    let second_prepared = prepare(&mut adapter, R18LocalSdmaDirectionV1::DeviceToHost);
    let second_binding = second_prepared.binding();
    assert_eq!(second_binding.ticket, first_binding.ticket);
    assert!(
        second_binding.persistent_use.lease.generation
            > first_binding.persistent_use.lease.generation
    );
    let second_published = publish(&mut adapter, second_prepared);
    let failure = second_published
        .observe_model_only(
            &mut adapter,
            R18CompletionObservationV1 {
                binding: first_binding,
                resolution: R18CompletionResolutionV1::Terminal(R18SdmaTerminalStatusV1::Succeeded),
            },
        )
        .err()
        .unwrap();
    assert_eq!(
        failure.error(),
        R18PersistentLocalSdmaErrorV1::ObservationMismatch
    );
    assert_eq!(failure.into_parts().1.binding(), second_binding);
    assert_eq!(
        adapter.snapshot().active_phase,
        Some(R18PersistentLocalSdmaPhaseV1::Published)
    );
}

#[test]
fn frontier_retirement_rejects_every_substitution_and_stale_replay() {
    let mut adapter = adapter(R18_LOCAL_SDMA_HOST_TO_DEVICE_ENGINE_V1);
    let prepared = prepare(&mut adapter, R18LocalSdmaDirectionV1::HostToDevice);
    let published = publish(&mut adapter, prepared);
    let completed = complete(&mut adapter, published, R18SdmaTerminalStatusV1::Succeeded);
    let restored = restore(&mut adapter, completed);
    let frontier = settle(&mut adapter, restored, R18SdmaTerminalStatusV1::Succeeded);
    let expected = frontier.key();
    let mut observations = vec![];
    let mut changed = expected;
    changed.persistent_frontier.generation += 1;
    observations.push(changed);
    changed = expected;
    changed.persistent_frontier.through_use.generation += 1;
    observations.push(changed);
    changed = expected;
    changed.allocation.allocation.generation.0 += 1;
    observations.push(changed);
    changed = expected;
    changed.queue.occurrence += 1;
    observations.push(changed);
    changed = expected;
    changed.attachment_generation += 1;
    observations.push(changed);

    let mut retained = frontier;
    for observed in observations {
        let failure = retained
            .retire_model_only(&mut adapter, observed)
            .err()
            .unwrap();
        assert_eq!(
            failure.error(),
            R18PersistentLocalSdmaErrorV1::ObservationMismatch
        );
        retained = failure.into_parts().1;
        assert_eq!(adapter.snapshot().pending_frontier, Some(expected));
    }
    retire(&mut adapter, retained);

    let prepared = prepare(&mut adapter, R18LocalSdmaDirectionV1::HostToDevice);
    let published = publish(&mut adapter, prepared);
    let completed = complete(&mut adapter, published, R18SdmaTerminalStatusV1::Succeeded);
    let restored = restore(&mut adapter, completed);
    let current = settle(&mut adapter, restored, R18SdmaTerminalStatusV1::Succeeded);
    let failure = current
        .retire_model_only(&mut adapter, expected)
        .err()
        .unwrap();
    assert_eq!(
        failure.error(),
        R18PersistentLocalSdmaErrorV1::ObservationMismatch
    );
    let current = failure.into_parts().1;
    assert!(current.key().persistent_frontier.generation > expected.persistent_frontier.generation);
    retire(&mut adapter, current);
}

#[test]
fn numerically_identical_foreign_frontier_is_rejected_by_private_incarnation() {
    let mut foreign_adapter = adapter(R18_LOCAL_SDMA_HOST_TO_DEVICE_ENGINE_V1);
    let foreign_prepared = prepare(&mut foreign_adapter, R18LocalSdmaDirectionV1::HostToDevice);
    let foreign_published = publish(&mut foreign_adapter, foreign_prepared);
    let foreign_completed = complete(
        &mut foreign_adapter,
        foreign_published,
        R18SdmaTerminalStatusV1::Succeeded,
    );
    let foreign_restored = restore(&mut foreign_adapter, foreign_completed);
    let foreign_frontier = settle(
        &mut foreign_adapter,
        foreign_restored,
        R18SdmaTerminalStatusV1::Succeeded,
    );

    let mut target_adapter = adapter(R18_LOCAL_SDMA_HOST_TO_DEVICE_ENGINE_V1);
    let target_prepared = prepare(&mut target_adapter, R18LocalSdmaDirectionV1::HostToDevice);
    let target_published = publish(&mut target_adapter, target_prepared);
    let target_completed = complete(
        &mut target_adapter,
        target_published,
        R18SdmaTerminalStatusV1::Succeeded,
    );
    let target_restored = restore(&mut target_adapter, target_completed);
    let target_frontier = settle(
        &mut target_adapter,
        target_restored,
        R18SdmaTerminalStatusV1::Succeeded,
    );

    let identical_observation = target_frontier.key();
    assert_eq!(foreign_frontier.key(), identical_observation);
    assert_eq!(
        foreign_adapter.snapshot().pending_frontier,
        target_adapter.snapshot().pending_frontier
    );

    let failure = foreign_frontier
        .retire_model_only(&mut target_adapter, identical_observation)
        .err()
        .unwrap();
    assert_eq!(failure.error(), R18PersistentLocalSdmaErrorV1::WrongAdapter);
    let foreign_frontier = failure.into_parts().1;
    assert_eq!(
        target_adapter.snapshot().pending_frontier,
        Some(identical_observation)
    );
    assert_eq!(
        foreign_adapter.snapshot().pending_frontier,
        Some(identical_observation)
    );

    assert_eq!(
        retire(&mut foreign_adapter, foreign_frontier).retired_use_count,
        1
    );
    assert_eq!(
        retire(&mut target_adapter, target_frontier).retired_use_count,
        1
    );
    assert!(foreign_adapter.release_model_only().is_ok());
    assert!(target_adapter.release_model_only().is_ok());
}

#[test]
fn exact_frontier_retirement_supports_more_than_sixty_four_sequential_uses() {
    let mut adapter = adapter(R18_LOCAL_SDMA_DEVICE_TO_HOST_ENGINE_V1);
    let mut previous_use_generation = 0;
    for _ in 0..65 {
        let prepared = prepare(&mut adapter, R18LocalSdmaDirectionV1::DeviceToHost);
        let use_generation = prepared.binding().persistent_use.lease.generation;
        assert!(use_generation > previous_use_generation);
        previous_use_generation = use_generation;
        let published = publish(&mut adapter, prepared);
        let completed = complete(&mut adapter, published, R18SdmaTerminalStatusV1::Succeeded);
        let restored = restore(&mut adapter, completed);
        let frontier = settle(&mut adapter, restored, R18SdmaTerminalStatusV1::Succeeded);
        assert_eq!(adapter.snapshot().pending_frontier, Some(frontier.key()));
        assert_eq!(retire(&mut adapter, frontier).retired_use_count, 1);
        assert_eq!(adapter.snapshot().pending_frontier, None);
    }
    assert_eq!(adapter.snapshot().settled_transfer_count, 65);
    let release = adapter.release_model_only().unwrap();
    assert_eq!(release.completed_lease_count, 65);
    assert_eq!(release.settled_transfer_count, 65);
}
