use alloc::vec;

use super::*;

const MIB: u64 = 1024 * 1024;

fn device() -> DeviceKeyV1 {
    DeviceKeyV1 {
        physical: PhysicalDeviceIdV1(0x100),
        generation: DeviceGenerationV1(3),
    }
}

fn vm() -> VmKeyV1 {
    VmKeyV1 {
        device: device(),
        id: VmIdV1(11),
    }
}

fn queue() -> QueueKeyV1 {
    QueueKeyV1 {
        vm: vm(),
        id: QueueInstanceIdV1(21),
        generation: QueueGenerationV1(22),
    }
}

fn pair() -> R19DirectionalQueuePairV1 {
    R19DirectionalQueuePairV1 {
        parent_queue: queue(),
        pair_occurrence: 23,
        device_to_host: R19DirectionalChildQueueV1 {
            native_queue_id: 0,
            engine_id: R18_LOCAL_SDMA_DEVICE_TO_HOST_ENGINE_V1,
        },
        host_to_device: R19DirectionalChildQueueV1 {
            native_queue_id: 7,
            engine_id: R18_LOCAL_SDMA_HOST_TO_DEVICE_ENGINE_V1,
        },
    }
}

fn local_allocation() -> R18LocalPersistentAllocationAdmissionV1 {
    let key = MemoryAllocationKeyV1 {
        vm: vm(),
        id: AllocationIdV1(12),
        generation: AllocationGenerationV1(13),
    };
    R18LocalPersistentAllocationAdmissionV1 {
        owner: R17PersistentAllocationOwnerIdV1(1),
        allocation: MemoryAllocationRecordV1 {
            key,
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
        },
        mapping: MemoryMappingRecordV1 {
            key: MemoryMappingKeyV1 {
                allocation: key,
                id: MappingIdV1(16),
            },
            target_devices: vec![device()],
            access: MemoryAccessV1::ReadWrite,
            mapped_start: 0,
            mapped_end: 1,
            state: MemoryMappingStateV1::Mapped,
        },
        device: device(),
    }
}

fn admission() -> R19DirectionalAdmissionV1 {
    R19DirectionalAdmissionV1 {
        allocation: local_allocation(),
        pair: pair(),
        pool_generation: 31,
        logical_byte_len: 7 * MIB + 17,
        physical_byte_len: 8 * MIB,
    }
}

fn adapter() -> R19DirectionalPersistentLocalSdmaAdapterV1 {
    R19DirectionalPersistentLocalSdmaAdapterV1::new_model_only(admission()).unwrap()
}

fn host() -> R18HostBufferKeyV1 {
    R18HostBufferKeyV1 {
        session_id: 41,
        id: 42,
        generation: 43,
        byte_len: 2 * MIB,
        coherence: MemoryCoherenceV1::HostCoherent,
    }
}

fn range(offset: u64, byte_len: u64) -> R18ByteRangeV1 {
    R18ByteRangeV1 {
        byte_offset: offset,
        byte_len,
    }
}

fn ticket(
    adapter: &R19DirectionalPersistentLocalSdmaAdapterV1,
    direction: R18LocalSdmaDirectionV1,
    generation: u32,
) -> R18PlannedSdmaTicketV1 {
    R18PlannedSdmaTicketV1 {
        owner: adapter.pair().parent_queue,
        queue_id: adapter.pair().child(direction).native_queue_id,
        slot: (generation % u32::from(R18_SDMA_RING_SLOT_COUNT_V1)) as u16,
        generation,
    }
}

fn prepare(
    adapter: &mut R19DirectionalPersistentLocalSdmaAdapterV1,
    direction: R18LocalSdmaDirectionV1,
    generation: u32,
) -> R19DirectionalTransferLeaseV1 {
    let ticket = ticket(adapter, direction, generation);
    adapter
        .prepare_model_only(direction, range(64, 256), host(), range(128, 256), ticket)
        .unwrap()
}

fn publish(
    adapter: &mut R19DirectionalPersistentLocalSdmaAdapterV1,
    lease: R19DirectionalTransferLeaseV1,
) -> R19DirectionalTransferLeaseV1 {
    let binding = lease.binding();
    match lease
        .resolve_publication_model_only(
            adapter,
            R19DirectionalPublicationObservationV1 {
                binding,
                resolution: R18PublicationResolutionV1::Confirmed,
            },
        )
        .unwrap()
    {
        R19DirectionalPublicationOutcomeV1::Published(lease) => lease,
        _ => panic!("publication must confirm"),
    }
}

fn complete(
    adapter: &mut R19DirectionalPersistentLocalSdmaAdapterV1,
    lease: R19DirectionalTransferLeaseV1,
    status: R18SdmaTerminalStatusV1,
) -> R19DirectionalTransferLeaseV1 {
    let binding = lease.binding();
    match lease
        .observe_model_only(
            adapter,
            R19DirectionalCompletionObservationV1 {
                binding,
                resolution: R18CompletionResolutionV1::Terminal(status),
            },
        )
        .unwrap()
    {
        R19DirectionalPollV1::Completed(lease) => lease,
        _ => panic!("completion must be terminal"),
    }
}

fn restore(
    adapter: &mut R19DirectionalPersistentLocalSdmaAdapterV1,
    lease: R19DirectionalTransferLeaseV1,
    status: R18SdmaTerminalStatusV1,
) -> R19DirectionalTransferLeaseV1 {
    let binding = lease.binding();
    match lease
        .restore_model_only(
            adapter,
            R19DirectionalRestoreObservationV1 {
                binding,
                status,
                child_current: true,
            },
        )
        .unwrap()
    {
        R19DirectionalRestoreOutcomeV1::Restored(lease) => lease,
        _ => panic!("completion must restore"),
    }
}

fn settle(
    adapter: &mut R19DirectionalPersistentLocalSdmaAdapterV1,
    lease: R19DirectionalTransferLeaseV1,
) -> R19DirectionalSettledFrontierV1 {
    let completed = complete(adapter, lease, R18SdmaTerminalStatusV1::Succeeded);
    let restored = restore(adapter, completed, R18SdmaTerminalStatusV1::Succeeded);
    let binding = restored.binding();
    restored
        .settle_model_only(
            adapter,
            R19DirectionalSettlementObservationV1 {
                binding,
                status: R18SdmaTerminalStatusV1::Succeeded,
            },
        )
        .unwrap()
}

fn settle_and_retire(
    adapter: &mut R19DirectionalPersistentLocalSdmaAdapterV1,
    lease: R19DirectionalTransferLeaseV1,
) -> R19DirectionalFrontierRetirementReceiptV1 {
    let frontier = settle(adapter, lease);
    let key = frontier.key();
    frontier.retire_model_only(adapter, key).unwrap()
}

fn execute(
    adapter: &mut R19DirectionalPersistentLocalSdmaAdapterV1,
    direction: R18LocalSdmaDirectionV1,
    generation: u32,
) -> R19DirectionalFrontierRetirementReceiptV1 {
    let prepared = prepare(adapter, direction, generation);
    let published = publish(adapter, prepared);
    settle_and_retire(adapter, published)
}

#[test]
fn admission_binds_logical_and_page_rounded_physical_extents() {
    let adapter = adapter();
    let snapshot = adapter.snapshot();
    assert_eq!(snapshot.logical_byte_len, 7 * MIB + 17);
    assert_eq!(snapshot.physical_byte_len, 8 * MIB);
    assert!(snapshot.logical_byte_len <= snapshot.physical_byte_len);

    let mut invalid = admission();
    invalid.logical_byte_len = invalid.physical_byte_len + 1;
    assert_eq!(
        R19DirectionalPersistentLocalSdmaAdapterV1::new_model_only(invalid)
            .err()
            .unwrap()
            .error(),
        R19DirectionalErrorV1::InvalidAllocation
    );
}

#[test]
fn pair_requires_distinct_bounded_exact_engine_children() {
    let mut invalid = admission();
    invalid.pair.host_to_device.native_queue_id = 0;
    assert_eq!(
        R19DirectionalPersistentLocalSdmaAdapterV1::new_model_only(invalid)
            .err()
            .unwrap()
            .error(),
        R19DirectionalErrorV1::InvalidPair
    );
    let mut invalid = admission();
    invalid.pair.device_to_host.engine_id = 1;
    assert_eq!(
        R19DirectionalPersistentLocalSdmaAdapterV1::new_model_only(invalid)
            .err()
            .unwrap()
            .error(),
        R19DirectionalErrorV1::InvalidPair
    );
}

#[test]
fn each_direction_binds_child_access_and_endpoint_exactly() {
    for (direction, engine, access, endpoint) in [
        (
            R18LocalSdmaDirectionV1::DeviceToHost,
            0,
            R17PersistentAccessModeV1::Read,
            R18PersistentEndpointV1::Source,
        ),
        (
            R18LocalSdmaDirectionV1::HostToDevice,
            1,
            R17PersistentAccessModeV1::Write,
            R18PersistentEndpointV1::Destination,
        ),
    ] {
        let mut adapter = adapter();
        let lease = prepare(&mut adapter, direction, 1);
        let binding = lease.binding();
        assert_eq!(binding.child().engine_id, engine);
        assert_eq!(binding.persistent_access, access);
        assert_eq!(binding.persistent_endpoint, endpoint);
        assert_eq!(binding.ticket.queue_id, binding.child().native_queue_id);
    }
}

#[test]
fn single_flight_and_frontier_retirement_gate_reuse() {
    let mut adapter = adapter();
    let lease = prepare(&mut adapter, R18LocalSdmaDirectionV1::HostToDevice, 1);
    assert_eq!(
        adapter
            .prepare_model_only(
                R18LocalSdmaDirectionV1::DeviceToHost,
                range(0, 64),
                host(),
                range(0, 64),
                R18PlannedSdmaTicketV1 {
                    owner: queue(),
                    queue_id: 0,
                    slot: 2,
                    generation: 2,
                },
            )
            .unwrap_err(),
        R19DirectionalErrorV1::Busy
    );
    let published = publish(&mut adapter, lease);
    let binding = published.binding();
    let completed = match published
        .observe_model_only(
            &mut adapter,
            R19DirectionalCompletionObservationV1 {
                binding,
                resolution: R18CompletionResolutionV1::Terminal(R18SdmaTerminalStatusV1::Succeeded),
            },
        )
        .unwrap()
    {
        R19DirectionalPollV1::Completed(lease) => lease,
        _ => unreachable!(),
    };
    let restored = match completed
        .restore_model_only(
            &mut adapter,
            R19DirectionalRestoreObservationV1 {
                binding,
                status: R18SdmaTerminalStatusV1::Succeeded,
                child_current: true,
            },
        )
        .unwrap()
    {
        R19DirectionalRestoreOutcomeV1::Restored(lease) => lease,
        _ => unreachable!(),
    };
    let frontier = restored
        .settle_model_only(
            &mut adapter,
            R19DirectionalSettlementObservationV1 {
                binding,
                status: R18SdmaTerminalStatusV1::Succeeded,
            },
        )
        .unwrap();
    assert_eq!(adapter.snapshot().pending_frontier, Some(frontier.key()));
    assert_eq!(
        adapter
            .prepare_model_only(
                R18LocalSdmaDirectionV1::DeviceToHost,
                range(0, 64),
                host(),
                range(0, 64),
                R18PlannedSdmaTicketV1 {
                    owner: queue(),
                    queue_id: 0,
                    slot: 2,
                    generation: 2,
                },
            )
            .unwrap_err(),
        R19DirectionalErrorV1::Busy
    );
}

#[test]
fn exact_retirement_clears_frontier_and_allows_any_successor() {
    let mut adapter = adapter();
    let retired = execute(&mut adapter, R18LocalSdmaDirectionV1::DeviceToHost, 1);
    assert_eq!(retired.retired_use_count, 1);
    assert_eq!(adapter.snapshot().pending_frontier, None);
    let next = prepare(&mut adapter, R18LocalSdmaDirectionV1::HostToDevice, 2);
    assert_eq!(
        next.binding().direction,
        R18LocalSdmaDirectionV1::HostToDevice
    );
}

#[test]
fn more_than_sixty_four_alternating_uses_recycle_the_r17_ledger() {
    let mut adapter = adapter();
    for generation in 1..=130 {
        let direction = if generation % 2 == 0 {
            R18LocalSdmaDirectionV1::HostToDevice
        } else {
            R18LocalSdmaDirectionV1::DeviceToHost
        };
        let retired = execute(&mut adapter, direction, generation);
        assert_eq!(retired.retired_use_count, 1);
    }
    assert_eq!(adapter.snapshot().settled_transfer_count, 130);
    assert!(adapter.active_persistent_use_record().is_none());
}

#[test]
fn repeated_same_direction_supports_chunk_continuation() {
    for direction in [
        R18LocalSdmaDirectionV1::HostToDevice,
        R18LocalSdmaDirectionV1::DeviceToHost,
    ] {
        let mut adapter = adapter();
        for generation in 1..=70 {
            let retired = execute(&mut adapter, direction, generation);
            assert_eq!(retired.retired_use_count, 1);
        }
        assert_eq!(adapter.snapshot().settled_transfer_count, 70);
    }
}

#[test]
fn recoverable_prepublication_failure_restores_retryable_owner() {
    let mut adapter = adapter();
    let lease = prepare(&mut adapter, R18LocalSdmaDirectionV1::DeviceToHost, 1);
    let binding = lease.binding();
    match lease
        .resolve_publication_model_only(
            &mut adapter,
            R19DirectionalPublicationObservationV1 {
                binding,
                resolution: R18PublicationResolutionV1::RecoverableFailure {
                    point: R18PrepublicationFailurePointV1::BeforeQueueCustody,
                },
            },
        )
        .unwrap()
    {
        R19DirectionalPublicationOutcomeV1::Recovered(receipt) => {
            assert_eq!(receipt.binding, binding)
        }
        _ => unreachable!(),
    }
    assert_eq!(adapter.snapshot().phase, None);
    let _retry = prepare(&mut adapter, R18LocalSdmaDirectionV1::HostToDevice, 2);
}

#[test]
fn retained_publication_is_permanent_ticketed_quarantine() {
    let mut adapter = adapter();
    let lease = prepare(&mut adapter, R18LocalSdmaDirectionV1::HostToDevice, 1);
    let binding = lease.binding();
    let quarantined = match lease
        .resolve_publication_model_only(
            &mut adapter,
            R19DirectionalPublicationObservationV1 {
                binding,
                resolution: R18PublicationResolutionV1::IndeterminateRetention {
                    point: R18PrepublicationFailurePointV1::Doorbell,
                },
            },
        )
        .unwrap()
    {
        R19DirectionalPublicationOutcomeV1::Quarantined(lease) => lease,
        _ => unreachable!(),
    };
    assert_eq!(quarantined.live_ticket(), Some(binding.ticket));
    assert!(!adapter.snapshot().current);
    assert_eq!(
        adapter.release_model_only().unwrap_err().error(),
        R19DirectionalErrorV1::Quarantined
    );
}

#[test]
fn preparation_ambiguity_quarantines_without_claiming_a_ticket() {
    let mut adapter = adapter();
    let lease = prepare(&mut adapter, R18LocalSdmaDirectionV1::DeviceToHost, 1);
    let quarantined = adapter
        .quarantine_preparation_currentness_model_only(lease)
        .unwrap();
    assert_eq!(quarantined.live_ticket(), None);
    assert_eq!(
        quarantined.reason(),
        R19DirectionalQuarantineReasonV1::PreparationCurrentnessAmbiguous
    );
}

#[test]
fn pending_and_timeout_retain_the_exact_ticket_and_child_custody() {
    let mut adapter = adapter();
    let prepared = prepare(&mut adapter, R18LocalSdmaDirectionV1::DeviceToHost, 1);
    let published = publish(&mut adapter, prepared);
    let binding = published.binding();
    let pending = match published
        .observe_model_only(
            &mut adapter,
            R19DirectionalCompletionObservationV1 {
                binding,
                resolution: R18CompletionResolutionV1::Pending,
            },
        )
        .unwrap()
    {
        R19DirectionalPollV1::Pending(lease) => lease,
        _ => unreachable!(),
    };
    assert_eq!(adapter.snapshot().live_ticket, Some(binding.ticket));
    let timed_out = match pending
        .observe_model_only(
            &mut adapter,
            R19DirectionalCompletionObservationV1 {
                binding,
                resolution: R18CompletionResolutionV1::TimedOut,
            },
        )
        .unwrap()
    {
        R19DirectionalPollV1::TimedOut(lease) => lease,
        _ => unreachable!(),
    };
    assert_eq!(timed_out.binding(), binding);
    assert_eq!(adapter.snapshot().live_ticket, Some(binding.ticket));
}

#[test]
fn stale_completion_observation_is_atomic_and_retryable() {
    let mut adapter = adapter();
    let prepared = prepare(&mut adapter, R18LocalSdmaDirectionV1::HostToDevice, 1);
    let published = publish(&mut adapter, prepared);
    let binding = published.binding();
    let mut stale = binding;
    stale.ticket.generation += 1;
    let (error, published) = published
        .observe_model_only(
            &mut adapter,
            R19DirectionalCompletionObservationV1 {
                binding: stale,
                resolution: R18CompletionResolutionV1::Pending,
            },
        )
        .unwrap_err()
        .into_parts();
    assert_eq!(error, R19DirectionalErrorV1::ObservationMismatch);
    assert_eq!(published.binding(), binding);
    assert_eq!(
        adapter.snapshot().phase,
        Some(R19DirectionalPhaseV1::Published)
    );
}

#[test]
fn completion_currentness_ambiguity_quarantines_the_live_ticket() {
    let mut adapter = adapter();
    let prepared = prepare(&mut adapter, R18LocalSdmaDirectionV1::HostToDevice, 1);
    let published = publish(&mut adapter, prepared);
    let binding = published.binding();
    let quarantined = match published
        .observe_model_only(
            &mut adapter,
            R19DirectionalCompletionObservationV1 {
                binding,
                resolution: R18CompletionResolutionV1::CurrentnessAmbiguous,
            },
        )
        .unwrap()
    {
        R19DirectionalPollV1::Quarantined(lease) => lease,
        _ => unreachable!(),
    };
    assert_eq!(quarantined.live_ticket(), Some(binding.ticket));
    assert!(!adapter.snapshot().current);
}

#[test]
fn stale_frontier_retirement_retains_adapter_and_frontier() {
    let mut adapter = adapter();
    let prepared = prepare(&mut adapter, R18LocalSdmaDirectionV1::DeviceToHost, 1);
    let published = publish(&mut adapter, prepared);
    let binding = published.binding();
    let completed = match published
        .observe_model_only(
            &mut adapter,
            R19DirectionalCompletionObservationV1 {
                binding,
                resolution: R18CompletionResolutionV1::Terminal(R18SdmaTerminalStatusV1::Succeeded),
            },
        )
        .unwrap()
    {
        R19DirectionalPollV1::Completed(lease) => lease,
        _ => unreachable!(),
    };
    let restored = match completed
        .restore_model_only(
            &mut adapter,
            R19DirectionalRestoreObservationV1 {
                binding,
                status: R18SdmaTerminalStatusV1::Succeeded,
                child_current: true,
            },
        )
        .unwrap()
    {
        R19DirectionalRestoreOutcomeV1::Restored(lease) => lease,
        _ => unreachable!(),
    };
    let frontier = restored
        .settle_model_only(
            &mut adapter,
            R19DirectionalSettlementObservationV1 {
                binding,
                status: R18SdmaTerminalStatusV1::Succeeded,
            },
        )
        .unwrap();
    let mut stale = frontier.key();
    stale.persistent_frontier.generation += 1;
    let (error, frontier) = frontier
        .retire_model_only(&mut adapter, stale)
        .unwrap_err()
        .into_parts();
    assert_eq!(error, R19DirectionalErrorV1::ObservationMismatch);
    assert_eq!(adapter.snapshot().pending_frontier, Some(frontier.key()));
}

#[test]
fn cross_incarnation_frontier_cannot_retire_an_equal_adapter() {
    let mut first = adapter();
    let first_prepared = prepare(&mut first, R18LocalSdmaDirectionV1::DeviceToHost, 1);
    let first_published = publish(&mut first, first_prepared);
    let first_frontier = settle(&mut first, first_published);

    let mut second = adapter();
    let second_prepared = prepare(&mut second, R18LocalSdmaDirectionV1::DeviceToHost, 1);
    let second_published = publish(&mut second, second_prepared);
    let second_frontier = settle(&mut second, second_published);
    assert_eq!(first_frontier.key(), second_frontier.key());

    let key = first_frontier.key();
    let (error, first_frontier) = first_frontier
        .retire_model_only(&mut second, key)
        .unwrap_err()
        .into_parts();
    assert_eq!(error, R19DirectionalErrorV1::WrongAdapter);
    assert_eq!(
        first.snapshot().pending_frontier,
        Some(first_frontier.key())
    );
    assert_eq!(
        second.snapshot().pending_frontier,
        Some(second_frontier.key())
    );
}

#[test]
fn idle_rebind_changes_attachment_and_starts_a_fresh_frontier_chain() {
    let mut adapter = adapter();
    let retired = execute(&mut adapter, R18LocalSdmaDirectionV1::DeviceToHost, 1);
    assert_eq!(retired.retired_use_count, 1);
    let mut replacement = pair();
    replacement.pair_occurrence += 1;
    replacement.device_to_host.native_queue_id = 8;
    let receipt = adapter.rebind_pair_model_only(replacement).unwrap();
    assert_eq!(receipt.attachment_generation, 2);
    let next = prepare(&mut adapter, R18LocalSdmaDirectionV1::DeviceToHost, 2);
    assert_eq!(next.binding().attachment_generation, 2);
}

#[test]
fn demote_advances_pool_generation_and_repromotion_rejects_old_incarnation() {
    let mut adapter = adapter();
    let retired = execute(&mut adapter, R18LocalSdmaDirectionV1::HostToDevice, 1);
    let demoted = match adapter.demote_model_only() {
        Ok(demoted) => demoted,
        Err(_) => panic!("idle adapter must demote"),
    };
    assert_eq!(demoted.pool_generation(), 32);
    let mut promoted = demoted.promote_model_only(pair()).unwrap();
    assert_eq!(promoted.snapshot().pool_generation, 32);
    assert_ne!(
        retired.frontier.pool_generation,
        promoted.snapshot().pool_generation
    );
    let next = prepare(&mut promoted, R18LocalSdmaDirectionV1::HostToDevice, 2);
    assert_eq!(next.binding().pool_generation, 32);
    let published = publish(&mut promoted, next);
    let current_frontier = settle(&mut promoted, published);
    let (error, current_frontier) = current_frontier
        .retire_model_only(&mut promoted, retired.frontier)
        .unwrap_err()
        .into_parts();
    assert_eq!(error, R19DirectionalErrorV1::ObservationMismatch);
    assert_eq!(
        promoted.snapshot().pending_frontier,
        Some(current_frontier.key())
    );
}

#[test]
fn logical_extent_copy_and_ticket_limits_are_enforced() {
    let mut adapter = adapter();
    let direction = R18LocalSdmaDirectionV1::DeviceToHost;
    let oversized = range(adapter.snapshot().logical_byte_len - 32, 64);
    assert_eq!(
        adapter
            .prepare_model_only(
                direction,
                oversized,
                host(),
                range(0, 64),
                ticket(&adapter, direction, 1),
            )
            .unwrap_err(),
        R19DirectionalErrorV1::InvalidRange
    );
    let mut stale_ticket = ticket(&adapter, direction, 1);
    stale_ticket.queue_id = adapter.pair().host_to_device.native_queue_id;
    assert_eq!(
        adapter
            .prepare_model_only(direction, range(0, 64), host(), range(0, 64), stale_ticket,)
            .unwrap_err(),
        R19DirectionalErrorV1::StaleBinding
    );
}

#[test]
fn failed_terminal_status_is_exactly_restored_settled_and_retired() {
    let mut adapter = adapter();
    let prepared = prepare(&mut adapter, R18LocalSdmaDirectionV1::DeviceToHost, 1);
    let published = publish(&mut adapter, prepared);
    let status = R18SdmaTerminalStatusV1::Failed { code: -17 };
    let completed = complete(&mut adapter, published, status);
    let binding = completed.binding();
    let restored = restore(&mut adapter, completed, status);
    let frontier = restored
        .settle_model_only(
            &mut adapter,
            R19DirectionalSettlementObservationV1 { binding, status },
        )
        .unwrap();
    let key = frontier.key();
    assert_eq!(
        frontier
            .retire_model_only(&mut adapter, key)
            .unwrap()
            .retired_use_count,
        1
    );
}

#[test]
fn restore_currentness_ambiguity_is_permanent_ticketed_quarantine() {
    let mut adapter = adapter();
    let prepared = prepare(&mut adapter, R18LocalSdmaDirectionV1::HostToDevice, 1);
    let published = publish(&mut adapter, prepared);
    let completed = complete(&mut adapter, published, R18SdmaTerminalStatusV1::Succeeded);
    let binding = completed.binding();
    let quarantined = match completed
        .restore_model_only(
            &mut adapter,
            R19DirectionalRestoreObservationV1 {
                binding,
                status: R18SdmaTerminalStatusV1::Succeeded,
                child_current: false,
            },
        )
        .unwrap()
    {
        R19DirectionalRestoreOutcomeV1::Quarantined(lease) => lease,
        _ => unreachable!(),
    };
    assert_eq!(quarantined.live_ticket(), Some(binding.ticket));
    assert_eq!(
        quarantined.reason(),
        R19DirectionalQuarantineReasonV1::RestoreCurrentnessAmbiguous
    );
    assert!(!adapter.snapshot().current);
}

#[test]
fn illegal_publication_classifications_are_atomic_and_retryable() {
    let mut first = adapter();
    let prepared = prepare(&mut first, R18LocalSdmaDirectionV1::DeviceToHost, 1);
    let binding = prepared.binding();
    let (error, prepared) = prepared
        .resolve_publication_model_only(
            &mut first,
            R19DirectionalPublicationObservationV1 {
                binding,
                resolution: R18PublicationResolutionV1::RecoverableFailure {
                    point: R18PrepublicationFailurePointV1::Doorbell,
                },
            },
        )
        .unwrap_err()
        .into_parts();
    assert_eq!(error, R19DirectionalErrorV1::IllegalFailureClassification);
    assert_eq!(
        first.snapshot().phase,
        Some(R19DirectionalPhaseV1::Prepared)
    );
    let published = publish(&mut first, prepared);
    assert_eq!(published.binding(), binding);

    let mut second = adapter();
    let prepared = prepare(&mut second, R18LocalSdmaDirectionV1::HostToDevice, 1);
    let binding = prepared.binding();
    let (error, prepared) = prepared
        .resolve_publication_model_only(
            &mut second,
            R19DirectionalPublicationObservationV1 {
                binding,
                resolution: R18PublicationResolutionV1::IndeterminateRetention {
                    point: R18PrepublicationFailurePointV1::BeforeQueueCustody,
                },
            },
        )
        .unwrap_err()
        .into_parts();
    assert_eq!(error, R19DirectionalErrorV1::IllegalFailureClassification);
    assert_eq!(prepared.binding(), binding);
    assert_eq!(
        second.snapshot().phase,
        Some(R19DirectionalPhaseV1::Prepared)
    );
}

#[test]
fn injected_lower_failures_restore_each_move_only_custody_token() {
    let mut primary_adapter = adapter();
    let prepared = prepare(
        &mut primary_adapter,
        R18LocalSdmaDirectionV1::HostToDevice,
        1,
    );
    let binding = prepared.binding();
    primary_adapter.inject_lower_failure_once(R19InjectedLowerFailurePointV1::Publish);
    let (error, prepared) = prepared
        .resolve_publication_model_only(
            &mut primary_adapter,
            R19DirectionalPublicationObservationV1 {
                binding,
                resolution: R18PublicationResolutionV1::Confirmed,
            },
        )
        .unwrap_err()
        .into_parts();
    assert_eq!(error, R19DirectionalErrorV1::InvariantViolation);
    assert_eq!(
        primary_adapter
            .active_persistent_use_record()
            .unwrap()
            .phase,
        R17PersistentUsePhaseV1::Reserved
    );
    let published = publish(&mut primary_adapter, prepared);

    primary_adapter.inject_lower_failure_once(R19InjectedLowerFailurePointV1::Observe);
    let (error, published) = published
        .observe_model_only(
            &mut primary_adapter,
            R19DirectionalCompletionObservationV1 {
                binding,
                resolution: R18CompletionResolutionV1::Pending,
            },
        )
        .unwrap_err()
        .into_parts();
    assert_eq!(error, R19DirectionalErrorV1::InvariantViolation);
    assert_eq!(
        primary_adapter
            .active_persistent_use_record()
            .unwrap()
            .phase,
        R17PersistentUsePhaseV1::Published
    );
    let completed = complete(
        &mut primary_adapter,
        published,
        R18SdmaTerminalStatusV1::Succeeded,
    );
    let restored = restore(
        &mut primary_adapter,
        completed,
        R18SdmaTerminalStatusV1::Succeeded,
    );
    primary_adapter.inject_lower_failure_once(R19InjectedLowerFailurePointV1::Settle);
    let (error, restored) = restored
        .settle_model_only(
            &mut primary_adapter,
            R19DirectionalSettlementObservationV1 {
                binding,
                status: R18SdmaTerminalStatusV1::Succeeded,
            },
        )
        .unwrap_err()
        .into_parts();
    assert_eq!(error, R19DirectionalErrorV1::InvariantViolation);
    assert_eq!(
        primary_adapter
            .active_persistent_use_record()
            .unwrap()
            .phase,
        R17PersistentUsePhaseV1::Terminal
    );
    let frontier = restored
        .settle_model_only(
            &mut primary_adapter,
            R19DirectionalSettlementObservationV1 {
                binding,
                status: R18SdmaTerminalStatusV1::Succeeded,
            },
        )
        .unwrap();
    let key = frontier.key();
    primary_adapter.inject_lower_failure_once(R19InjectedLowerFailurePointV1::Retire);
    let (error, frontier) = frontier
        .retire_model_only(&mut primary_adapter, key)
        .unwrap_err()
        .into_parts();
    assert_eq!(error, R19DirectionalErrorV1::InvariantViolation);
    assert_eq!(primary_adapter.snapshot().pending_frontier, Some(key));
    assert_eq!(
        frontier
            .retire_model_only(&mut primary_adapter, key)
            .unwrap()
            .retired_use_count,
        1
    );

    let mut cancel_adapter = adapter();
    let prepared = prepare(
        &mut cancel_adapter,
        R18LocalSdmaDirectionV1::DeviceToHost,
        1,
    );
    let binding = prepared.binding();
    cancel_adapter.inject_lower_failure_once(R19InjectedLowerFailurePointV1::Cancel);
    let (error, prepared) = prepared
        .resolve_publication_model_only(
            &mut cancel_adapter,
            R19DirectionalPublicationObservationV1 {
                binding,
                resolution: R18PublicationResolutionV1::RecoverableFailure {
                    point: R18PrepublicationFailurePointV1::BeforeQueueCustody,
                },
            },
        )
        .unwrap_err()
        .into_parts();
    assert_eq!(error, R19DirectionalErrorV1::InvariantViolation);
    assert_eq!(
        cancel_adapter.active_persistent_use_record().unwrap().phase,
        R17PersistentUsePhaseV1::Reserved
    );
    assert!(matches!(
        prepared
            .resolve_publication_model_only(
                &mut cancel_adapter,
                R19DirectionalPublicationObservationV1 {
                    binding,
                    resolution: R18PublicationResolutionV1::RecoverableFailure {
                        point: R18PrepublicationFailurePointV1::BeforeQueueCustody,
                    },
                },
            )
            .unwrap(),
        R19DirectionalPublicationOutcomeV1::Recovered(_)
    ));

    let mut quarantine_adapter = adapter();
    let prepared = prepare(
        &mut quarantine_adapter,
        R18LocalSdmaDirectionV1::HostToDevice,
        1,
    );
    quarantine_adapter.inject_lower_failure_once(R19InjectedLowerFailurePointV1::Quarantine);
    let (error, prepared) = quarantine_adapter
        .quarantine_preparation_currentness_model_only(prepared)
        .unwrap_err()
        .into_parts();
    assert_eq!(error, R19DirectionalErrorV1::InvariantViolation);
    assert_eq!(
        quarantine_adapter
            .active_persistent_use_record()
            .unwrap()
            .phase,
        R17PersistentUsePhaseV1::Reserved
    );
    assert_eq!(
        quarantine_adapter
            .quarantine_preparation_currentness_model_only(prepared)
            .unwrap()
            .live_ticket(),
        None
    );

    let mut restore_adapter = adapter();
    let prepared = prepare(
        &mut restore_adapter,
        R18LocalSdmaDirectionV1::HostToDevice,
        1,
    );
    let published = publish(&mut restore_adapter, prepared);
    let completed = complete(
        &mut restore_adapter,
        published,
        R18SdmaTerminalStatusV1::Failed { code: -17 },
    );
    let binding = completed.binding();
    restore_adapter.inject_lower_failure_once(R19InjectedLowerFailurePointV1::RestoreCurrentness);
    let (error, completed) = completed
        .restore_model_only(
            &mut restore_adapter,
            R19DirectionalRestoreObservationV1 {
                binding,
                status: R18SdmaTerminalStatusV1::Failed { code: -17 },
                child_current: false,
            },
        )
        .unwrap_err()
        .into_parts();
    assert_eq!(error, R19DirectionalErrorV1::InvariantViolation);
    assert_eq!(
        restore_adapter
            .active_persistent_use_record()
            .unwrap()
            .phase,
        R17PersistentUsePhaseV1::Terminal
    );
    assert!(matches!(
        completed
            .restore_model_only(
                &mut restore_adapter,
                R19DirectionalRestoreObservationV1 {
                    binding,
                    status: R18SdmaTerminalStatusV1::Failed { code: -17 },
                    child_current: false,
                },
            )
            .unwrap(),
        R19DirectionalRestoreOutcomeV1::Quarantined(_)
    ));
}

#[test]
fn active_and_frontier_custody_gate_rebind_demote_and_release() {
    let mut active_adapter = adapter();
    let prepared = prepare(
        &mut active_adapter,
        R18LocalSdmaDirectionV1::DeviceToHost,
        1,
    );
    assert_eq!(
        active_adapter.rebind_pair_model_only(pair()).unwrap_err(),
        R19DirectionalErrorV1::Busy
    );
    let retained_adapter = match active_adapter.demote_model_only() {
        Ok(_) => panic!("active adapter must not demote"),
        Err(failure) => {
            assert_eq!(failure.error(), R19DirectionalErrorV1::Busy);
            failure.into_parts().1
        }
    };
    assert_eq!(
        retained_adapter.snapshot().phase,
        Some(R19DirectionalPhaseV1::Prepared)
    );
    drop(prepared);

    let mut adapter = adapter();
    let prepared = prepare(&mut adapter, R18LocalSdmaDirectionV1::HostToDevice, 1);
    let published = publish(&mut adapter, prepared);
    let frontier = settle(&mut adapter, published);
    let adapter = match adapter.release_model_only() {
        Ok(_) => panic!("pending frontier must block release"),
        Err(failure) => {
            assert_eq!(failure.error(), R19DirectionalErrorV1::Busy);
            failure.into_parts().1
        }
    };
    assert_eq!(adapter.snapshot().pending_frontier, Some(frontier.key()));
}
