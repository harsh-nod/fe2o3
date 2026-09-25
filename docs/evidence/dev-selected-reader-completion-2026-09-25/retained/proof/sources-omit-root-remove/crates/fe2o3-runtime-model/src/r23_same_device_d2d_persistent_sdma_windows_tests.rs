use super::*;
use alloc::{vec, vec::Vec};

const TRANSFER_ID: u64 = 2301;
const ALLOCATION_BYTES: u64 = R17_PERSISTENT_NATIVE_ALLOCATION_BYTES_V1;

fn device() -> DeviceKeyV1 {
    DeviceKeyV1 {
        physical: PhysicalDeviceIdV1(23),
        generation: DeviceGenerationV1(24),
    }
}

fn vm() -> VmKeyV1 {
    VmKeyV1 {
        device: device(),
        id: VmIdV1(25),
    }
}

fn allocation(owner: u64, id: u64, generation: u64, mapping: u64) -> R18NativeAllocationKeyV1 {
    let allocation = MemoryAllocationKeyV1 {
        vm: vm(),
        id: AllocationIdV1(id),
        generation: AllocationGenerationV1(generation),
    };
    R18NativeAllocationKeyV1 {
        owner: R17PersistentAllocationOwnerIdV1(owner),
        allocation,
        mapping: MemoryMappingKeyV1 {
            allocation,
            id: MappingIdV1(mapping),
        },
    }
}

fn allocation_binding(
    owner: u64,
    id: u64,
    generation: u64,
    mapping: u64,
    backing: u64,
    gpu_va: u64,
) -> R23D2dAllocationBindingV1 {
    R23D2dAllocationBindingV1 {
        allocation: allocation(owner, id, generation, mapping),
        attachment_generation: generation + 100,
        pool_generation: generation + 200,
        backing_identity: backing,
        logical_byte_len: ALLOCATION_BYTES,
        physical_byte_len: ALLOCATION_BYTES,
        mapped_gpu_va: GpuVaRangeV1 {
            base: gpu_va,
            byte_len: ALLOCATION_BYTES,
        },
    }
}

fn binding() -> R23D2dBindingV1 {
    R23D2dBindingV1 {
        source: allocation_binding(31, 32, 33, 34, 35, 0x1000_0000),
        destination: allocation_binding(41, 42, 43, 44, 45, 0x3000_0000),
        queue: R18LocalSdmaQueueOccurrenceV1 {
            logical_queue: QueueKeyV1 {
                vm: vm(),
                id: QueueInstanceIdV1(51),
                generation: QueueGenerationV1(52),
            },
            native_queue_id: 5,
            occurrence: 53,
            engine_id: 0,
        },
    }
}

fn request(byte_len: u64) -> R23D2dCopyRequestV1 {
    R23D2dCopyRequestV1 {
        transfer_id: TRANSFER_ID,
        source_range: R18ByteRangeV1 {
            byte_offset: 0,
            byte_len,
        },
        destination_range: R18ByteRangeV1 {
            byte_offset: 0,
            byte_len,
        },
        byte_len,
    }
}

fn new_model() -> R23SameDeviceD2dPersistentSdmaWindowsV1 {
    R23SameDeviceD2dPersistentSdmaWindowsV1::new_model_only(binding()).unwrap()
}

fn begin(model: &mut R23SameDeviceD2dPersistentSdmaWindowsV1, byte_len: u64) {
    assert_eq!(
        model.begin_model_only(request(byte_len), vec![]),
        Ok(R23D2dClassificationV1::Applied)
    );
}

fn prepare(model: &mut R23SameDeviceD2dPersistentSdmaWindowsV1) -> R23D2dWindowPlanV1 {
    match model.prepare_window_model_only(&[]).unwrap() {
        R23D2dClassificationV1::Prepared(plan) => plan,
        other => panic!("unexpected preparation: {other:?}"),
    }
}

fn publish(model: &mut R23SameDeviceD2dPersistentSdmaWindowsV1, plan: &R23D2dWindowPlanV1) {
    assert_eq!(
        model.resolve_publication_model_only(plan, R23D2dPublicationDispositionV1::Confirmed),
        Ok(R23D2dClassificationV1::Published(plan.clone()))
    );
}

fn complete(
    model: &mut R23SameDeviceD2dPersistentSdmaWindowsV1,
    plan: &R23D2dWindowPlanV1,
) -> R23D2dFrontierKeyV1 {
    let completion = r23_exact_completion_metadata_v1(plan);
    match model
        .poll_window_model_only(R23D2dPollDispositionV1::Completed, Some(&completion))
        .unwrap()
    {
        R23D2dClassificationV1::FrontierPending(frontier) => frontier,
        other => panic!("unexpected completion: {other:?}"),
    }
}

#[test]
fn valid_pair_starts_with_two_exact_move_only_authorities() {
    let state = new_model().snapshot();
    assert_eq!(state.phase, R23D2dPhaseV1::DevicePairReady);
    assert_eq!(state.custody, Some(R23D2dCustodyKindV1::Device));
    assert_eq!(state.source_authority_count, 1);
    assert_eq!(state.destination_authority_count, 1);
    assert_eq!(state.source_read_lease_count, 0);
    assert_eq!(state.destination_write_lease_count, 0);
    assert!(state.current);
}

#[test]
fn currentness_loss_before_admission_enters_valid_absorbing_quarantine() {
    let mut model = new_model();
    assert!(matches!(
        model.lose_currentness_model_only(),
        Ok(R23D2dClassificationV1::Quarantined(_))
    ));
    let state = model.snapshot();
    assert_eq!(state.phase, R23D2dPhaseV1::Quarantined);
    assert_eq!(state.custody, Some(R23D2dCustodyKindV1::Quarantined));
    assert_eq!(state.source_authority_count, 1);
    assert_eq!(state.destination_authority_count, 1);
    assert_eq!(state.source_read_lease_count, 0);
    assert_eq!(state.destination_write_lease_count, 0);
    assert!(state.target_retained);
    assert!(!state.current);
    assert_eq!(
        model.release_terminal_model_only(0),
        Err(R23D2dErrorV1::Quarantined)
    );
}

#[test]
fn owner_allocation_mapping_and_backing_aliases_are_rejected() {
    let good = binding();
    let mut cases = vec![good; 4];
    cases[0].destination.allocation.owner = good.source.allocation.owner;
    cases[1].destination.allocation.allocation = good.source.allocation.allocation;
    cases[1].destination.allocation.mapping.allocation = good.source.allocation.allocation;
    cases[2].destination.allocation.mapping = good.source.allocation.mapping;
    cases[3].destination.backing_identity = good.source.backing_identity;
    for candidate in cases {
        assert!(matches!(
            R23SameDeviceD2dPersistentSdmaWindowsV1::new_model_only(candidate),
            Err(R23D2dErrorV1::InvalidBinding)
        ));
    }
}

#[test]
fn cross_vm_cross_device_and_queue_substitution_are_rejected() {
    let mut candidate = binding();
    candidate.destination.allocation.allocation.vm.id.0 += 1;
    candidate.destination.allocation.mapping.allocation =
        candidate.destination.allocation.allocation;
    assert!(R23SameDeviceD2dPersistentSdmaWindowsV1::new_model_only(candidate).is_err());

    let mut candidate = binding();
    candidate
        .destination
        .allocation
        .allocation
        .vm
        .device
        .generation
        .0 += 1;
    candidate.destination.allocation.mapping.allocation =
        candidate.destination.allocation.allocation;
    assert!(R23SameDeviceD2dPersistentSdmaWindowsV1::new_model_only(candidate).is_err());

    let mut candidate = binding();
    candidate.queue.logical_queue.vm.id.0 += 1;
    assert!(R23SameDeviceD2dPersistentSdmaWindowsV1::new_model_only(candidate).is_err());

    let mut candidate = binding();
    candidate.queue.engine_id = 1;
    assert!(R23SameDeviceD2dPersistentSdmaWindowsV1::new_model_only(candidate).is_err());
}

#[test]
fn mapped_overlap_and_wrapping_extents_are_rejected() {
    let mut overlap = binding();
    overlap.destination.mapped_gpu_va.base = overlap.source.mapped_gpu_va.base + 4096;
    assert!(R23SameDeviceD2dPersistentSdmaWindowsV1::new_model_only(overlap).is_err());

    let mut wrapping = binding();
    wrapping.destination.mapped_gpu_va.base = u64::MAX - 3;
    assert!(R23SameDeviceD2dPersistentSdmaWindowsV1::new_model_only(wrapping).is_err());
}

#[test]
fn invalid_copy_ranges_are_preflight_atomic() {
    let mut model = new_model();
    let before = model.snapshot();
    let mut invalid = request(4096);
    invalid.source_range.byte_len += 1;
    assert_eq!(
        model.begin_model_only(invalid, vec![]),
        Err(R23D2dErrorV1::InvalidRequest)
    );
    assert_eq!(model.snapshot(), before);

    let mut invalid = request(8192);
    invalid.destination_range.byte_offset = ALLOCATION_BYTES - 4096;
    assert_eq!(
        model.begin_model_only(invalid, vec![]),
        Err(R23D2dErrorV1::InvalidRequest)
    );
    assert_eq!(model.snapshot(), before);
}

#[test]
fn dependency_pending_and_identity_mismatch_are_preparation_atomic() {
    let dependency = R20DependencyV1 {
        event_id: 61,
        generation: 62,
    };
    let mut model = new_model();
    model
        .begin_model_only(request(4096), vec![dependency])
        .unwrap();
    let before = model.snapshot();
    let pending = [R20DependencyObservationV1 {
        dependency,
        status: R20DependencyStatusV1::Pending,
    }];
    assert_eq!(
        model.prepare_window_model_only(&pending),
        Ok(R23D2dClassificationV1::DependencyPending)
    );
    assert_eq!(model.snapshot(), before);
    let substituted = [R20DependencyObservationV1 {
        dependency: R20DependencyV1 {
            generation: 63,
            ..dependency
        },
        status: R20DependencyStatusV1::Satisfied,
    }];
    assert_eq!(
        model.prepare_window_model_only(&substituted),
        Err(R23D2dErrorV1::DependencyMismatch)
    );
    assert_eq!(model.snapshot(), before);
}

#[test]
fn planned_leases_have_exact_read_write_roles_and_window_ranges() {
    let mut model = new_model();
    begin(&mut model, 8192);
    let plan = prepare(&mut model);
    assert_eq!(plan.leases.source_read.role, R23D2dLeaseRoleV1::SourceRead);
    assert_eq!(
        plan.leases.destination_write.role,
        R23D2dLeaseRoleV1::DestinationWrite
    );
    assert_eq!(
        plan.leases.source_read.allocation,
        binding().source.allocation
    );
    assert_eq!(
        plan.leases.destination_write.allocation,
        binding().destination.allocation
    );
    assert_eq!(plan.leases.source_read.range, request(8192).source_range);
    assert_eq!(
        plan.leases.destination_write.range,
        request(8192).destination_range
    );
    let state = model.snapshot();
    assert_eq!(state.source_read_lease_count, 1);
    assert_eq!(state.destination_write_lease_count, 1);
}

#[test]
fn one_two_and_sixty_three_packets_pair_exact_contiguous_ranges() {
    let max = R18_SDMA_MAX_LINEAR_COPY_BYTES_V1;
    for (bytes, count) in [(1, 1), (max + 1, 2), (R23_D2D_WINDOW_MAX_BYTES_V1, 63)] {
        let mut model = new_model();
        begin(&mut model, bytes);
        let plan = prepare(&mut model);
        assert_eq!(plan.packets.len(), count);
        assert_eq!(plan.byte_len, bytes);
        for (index, packet) in plan.packets.iter().enumerate() {
            assert_eq!(usize::from(packet.packet_index), index);
            assert_eq!(
                packet.source_range.byte_len,
                packet.destination_range.byte_len
            );
            assert_eq!(
                packet.source_range.byte_offset,
                plan.leases.source_read.range.byte_offset + packet.transfer_offset
            );
            assert_eq!(
                packet.destination_range.byte_offset,
                plan.leases.destination_write.range.byte_offset + packet.transfer_offset
            );
        }
        assert_eq!(
            plan.packets
                .iter()
                .map(|packet| packet.source_range.byte_len)
                .sum::<u64>(),
            bytes
        );
    }
}

#[test]
fn tickets_bind_queue_and_use_independent_generations_after_wrap() {
    let mut model = new_model();
    begin(
        &mut model,
        R23_D2D_WINDOW_MAX_BYTES_V1 + R18_SDMA_MAX_LINEAR_COPY_BYTES_V1 + 1,
    );
    let first = prepare(&mut model);
    assert_eq!(first.packets.len(), 63);
    assert!(first.packets.iter().enumerate().all(|(index, packet)| {
        usize::from(packet.ticket.slot) == index
            && packet.ticket.generation == 1
            && packet.ticket.owner == binding().queue.logical_queue
            && packet.ticket.queue_id == binding().queue.native_queue_id
    }));
    publish(&mut model, &first);
    let frontier = complete(&mut model, &first);
    assert!(matches!(
        model.retire_window_model_only(&frontier),
        Ok(R23D2dClassificationV1::ReadyContinuation { .. })
    ));
    let second = prepare(&mut model);
    assert_eq!(second.packets[0].ticket.slot, 63);
    assert_eq!(second.packets[0].ticket.generation, 1);
    assert_eq!(second.packets[1].ticket.slot, 0);
    assert_eq!(second.packets[1].ticket.generation, 2);
    publish(&mut model, &second);

    model
        .poll_window_model_only(
            R23D2dPollDispositionV1::Completed,
            Some(&r23_exact_completion_metadata_v1(&second)),
        )
        .unwrap();
    let state = model.snapshot();
    assert_eq!(state.slot_generations[0], 2);
    assert_eq!(state.slot_generations[63], 1);
}

#[test]
fn clean_publication_retry_restores_both_authorities_after_consuming_reservations() {
    let mut model = new_model();
    begin(&mut model, 4096);
    let ready = model.snapshot();
    let plan = prepare(&mut model);
    assert_eq!(
        model.resolve_publication_model_only(
            &plan,
            R23D2dPublicationDispositionV1::RetryableBeforeQueueCustody,
        ),
        Ok(R23D2dClassificationV1::Retryable)
    );
    let restored = model.snapshot();
    assert_eq!(restored.phase, R23D2dPhaseV1::Ready);
    assert_eq!(restored.custody, Some(R23D2dCustodyKindV1::Ready));
    assert_eq!(restored.source_authority_count, 1);
    assert_eq!(restored.destination_authority_count, 1);
    assert_eq!(restored.source_read_lease_count, 0);
    assert_eq!(restored.slot_generations, ready.slot_generations);
    assert_eq!(restored.published_windows, ready.published_windows);
    assert_eq!(
        restored.source_next_use_generation,
        ready.source_next_use_generation + 1
    );
    assert_eq!(
        restored.destination_next_use_generation,
        ready.destination_next_use_generation + 1
    );
}

#[test]
fn destination_reservation_failure_consumes_only_source_generation() {
    let mut model = new_model();
    begin(&mut model, 4096);
    let ready = model.snapshot();
    assert_eq!(
        model.prepare_window_with_reservation_model_only(
            &[],
            R23D2dReservationDispositionV1::DestinationRejectedAfterSourceReserved,
        ),
        Ok(R23D2dClassificationV1::Retryable)
    );
    let restored = model.snapshot();
    assert_eq!(restored.phase, R23D2dPhaseV1::Ready);
    assert_eq!(restored.custody, Some(R23D2dCustodyKindV1::Ready));
    assert_eq!(restored.source_authority_count, 1);
    assert_eq!(restored.destination_authority_count, 1);
    assert_eq!(restored.source_read_lease_count, 0);
    assert_eq!(restored.destination_write_lease_count, 0);
    assert_eq!(
        restored.source_next_use_generation,
        ready.source_next_use_generation + 1
    );
    assert_eq!(
        restored.destination_next_use_generation,
        ready.destination_next_use_generation
    );
}

#[test]
fn confirmed_window_has_one_pointer_and_doorbell_and_only_possible_mutation() {
    let mut model = new_model();
    begin(&mut model, R18_SDMA_MAX_LINEAR_COPY_BYTES_V1 + 1);
    let plan = prepare(&mut model);
    publish(&mut model, &plan);
    let state = model.snapshot();
    assert_eq!(state.published_windows, 1);
    assert_eq!(state.published_packets, 2);
    assert_eq!(state.write_pointer_publications, 1);
    assert_eq!(state.doorbell_publications, 1);
    assert_eq!(state.destination_possibly_mutated_through, plan.byte_len);
    assert_eq!(state.destination_dirty_through, 0);
}

#[test]
fn retained_or_substituted_publication_is_absorbing_quarantine() {
    for substitute in [false, true] {
        let mut model = new_model();
        begin(&mut model, 4096);
        let mut plan = prepare(&mut model);
        let disposition = if substitute {
            plan.transfer_id += 1;
            R23D2dPublicationDispositionV1::Confirmed
        } else {
            R23D2dPublicationDispositionV1::RetainedAfterPacketWrite
        };
        assert!(matches!(
            model.resolve_publication_model_only(&plan, disposition),
            Ok(R23D2dClassificationV1::Quarantined(_))
        ));
        let state = model.snapshot();
        assert_eq!(state.phase, R23D2dPhaseV1::Quarantined);
        assert_eq!(state.source_authority_count, 1);
        assert_eq!(state.destination_authority_count, 1);
        assert_eq!(state.source_read_lease_count, 1);
        assert_eq!(
            model.release_terminal_model_only(TRANSFER_ID),
            Err(R23D2dErrorV1::Quarantined)
        );
    }
}

#[test]
fn pending_and_timeout_repoll_preserve_exact_published_pair() {
    let mut model = new_model();
    begin(&mut model, 8192);
    let plan = prepare(&mut model);
    publish(&mut model, &plan);
    let published = model.snapshot();
    assert_eq!(
        model.poll_window_model_only(R23D2dPollDispositionV1::Pending, None),
        Ok(R23D2dClassificationV1::Pending)
    );
    assert_eq!(model.snapshot(), published);
    assert_eq!(
        model.poll_window_model_only(R23D2dPollDispositionV1::TimedOut, None),
        Ok(R23D2dClassificationV1::TimedOut)
    );
    let timed_out = model.snapshot();
    assert_eq!(timed_out.phase, R23D2dPhaseV1::TimedOut);
    assert_eq!(timed_out.custody, Some(R23D2dCustodyKindV1::Published));
    assert_eq!(timed_out.source_read_lease_count, 1);
    assert_eq!(timed_out.destination_write_lease_count, 1);
    let frontier = complete(&mut model, &plan);
    assert_eq!(frontier.completion.aggregate_bytes, 8192);
}

#[test]
fn incomplete_aggregate_never_creates_frontier_dirty_bytes_or_continuation() {
    let mut model = new_model();
    begin(&mut model, R18_SDMA_MAX_LINEAR_COPY_BYTES_V1 + 1);
    let plan = prepare(&mut model);
    publish(&mut model, &plan);
    assert_eq!(
        model.poll_window_model_only(
            R23D2dPollDispositionV1::Incomplete {
                completed_packets: 1,
            },
            None,
        ),
        Ok(R23D2dClassificationV1::Incomplete {
            completed_packets: 1,
        })
    );
    let state = model.snapshot();
    assert_eq!(state.phase, R23D2dPhaseV1::Published);
    assert_eq!(state.custody, Some(R23D2dCustodyKindV1::Published));
    assert_eq!(state.destination_dirty_through, 0);
    assert_eq!(state.retired_windows, 0);
}

#[test]
fn aggregate_completion_requires_exact_ticket_roster_and_byte_count() {
    let mut model = new_model();
    begin(&mut model, 8192);
    let plan = prepare(&mut model);
    publish(&mut model, &plan);
    let mut substituted = r23_exact_completion_metadata_v1(&plan);
    substituted.aggregate_bytes -= 1;
    assert!(matches!(
        model.poll_window_model_only(R23D2dPollDispositionV1::Completed, Some(&substituted),),
        Ok(R23D2dClassificationV1::Quarantined(_))
    ));

    let mut model = new_model();
    begin(&mut model, 8192);
    let plan = prepare(&mut model);
    publish(&mut model, &plan);
    let mut substituted = r23_exact_completion_metadata_v1(&plan);
    substituted.completions[0].completion_value += 1;
    assert!(matches!(
        model.poll_window_model_only(R23D2dPollDispositionV1::Completed, Some(&substituted),),
        Ok(R23D2dClassificationV1::Quarantined(_))
    ));
}

#[test]
fn authenticated_success_is_not_dirty_until_full_frontier_retirement() {
    let mut model = new_model();
    begin(&mut model, 8192);
    let plan = prepare(&mut model);
    publish(&mut model, &plan);
    let frontier = complete(&mut model, &plan);
    let pending = model.snapshot();
    assert_eq!(pending.phase, R23D2dPhaseV1::FrontierPending);
    assert_eq!(pending.destination_dirty_through, 0);
    assert_eq!(pending.observed_completed_packets, plan.packets.len());
    assert!(matches!(
        model.retire_window_model_only(&frontier),
        Ok(R23D2dClassificationV1::Completed(
            R23D2dCompletionRecordV1 {
                succeeded: true,
                completed_bytes: 8192,
                destination_dirty_through: 8192,
                ..
            }
        ))
    ));
}

#[test]
fn native_execution_failure_enters_absorbing_quarantine() {
    let mut model = new_model();
    begin(&mut model, 8192);
    let plan = prepare(&mut model);
    publish(&mut model, &plan);
    assert!(matches!(
        model.poll_window_model_only(
            R23D2dPollDispositionV1::Indeterminate(
                R23D2dQuarantineReasonV1::CompletionIndeterminate,
            ),
            None,
        ),
        Ok(R23D2dClassificationV1::Quarantined(_))
    ));
    let state = model.snapshot();
    assert_eq!(state.phase, R23D2dPhaseV1::Quarantined);
    assert_eq!(state.source_read_lease_count, 1);
    assert_eq!(state.destination_write_lease_count, 1);
    assert_eq!(state.destination_dirty_through, 0);
    assert_eq!(state.destination_possibly_mutated_through, 8192);
}

#[test]
fn stale_frontier_quarantines_both_authorities_and_leases() {
    let mut model = new_model();
    begin(&mut model, 8192);
    let plan = prepare(&mut model);
    publish(&mut model, &plan);
    let mut frontier = complete(&mut model, &plan);
    frontier.completion.plan.transfer_id += 1;
    assert!(matches!(
        model.retire_window_model_only(&frontier),
        Ok(R23D2dClassificationV1::Quarantined(_))
    ));
    let state = model.snapshot();
    assert_eq!(state.source_authority_count, 1);
    assert_eq!(state.destination_authority_count, 1);
    assert_eq!(state.source_read_lease_count, 1);
    assert_eq!(state.destination_write_lease_count, 1);
}

#[test]
fn continuation_is_visible_only_after_complete_window_retirement() {
    let total = R23_D2D_WINDOW_MAX_BYTES_V1 + 4096;
    let mut model = new_model();
    begin(&mut model, total);
    let plan = prepare(&mut model);
    publish(&mut model, &plan);
    assert_eq!(model.snapshot().destination_dirty_through, 0);
    let frontier = complete(&mut model, &plan);
    assert_eq!(model.snapshot().destination_dirty_through, 0);
    assert_eq!(
        model.retire_window_model_only(&frontier),
        Ok(R23D2dClassificationV1::ReadyContinuation {
            completed_bytes: R23_D2D_WINDOW_MAX_BYTES_V1,
        })
    );
    assert_eq!(
        model.snapshot().destination_dirty_through,
        R23_D2D_WINDOW_MAX_BYTES_V1
    );
}

#[test]
fn full_256_mib_transfer_uses_two_windows_and_sixty_five_packets() {
    let mut model = new_model();
    begin(&mut model, ALLOCATION_BYTES);
    let mut counts = Vec::new();
    loop {
        let plan = prepare(&mut model);
        counts.push(plan.packets.len());
        publish(&mut model, &plan);
        let frontier = complete(&mut model, &plan);
        if matches!(
            model.retire_window_model_only(&frontier).unwrap(),
            R23D2dClassificationV1::Completed(_)
        ) {
            break;
        }
    }
    assert_eq!(counts, [63, 2]);
    let state = model.snapshot();
    assert_eq!(state.published_packets, 65);
    assert_eq!(state.write_pointer_publications, 2);
    assert_eq!(state.doorbell_publications, 2);
}

#[test]
fn cancellation_and_terminal_release_restore_pair_for_reuse() {
    let mut model = new_model();
    begin(&mut model, 4096);
    assert_eq!(
        model.cancel_model_only(),
        Ok(R23D2dClassificationV1::Cancelled)
    );
    assert_eq!(model.snapshot().phase, R23D2dPhaseV1::DevicePairReady);

    begin(&mut model, 4096);
    let plan = prepare(&mut model);
    publish(&mut model, &plan);
    let frontier = complete(&mut model, &plan);
    model.retire_window_model_only(&frontier).unwrap();
    assert_eq!(
        model.release_terminal_model_only(TRANSFER_ID),
        Ok(R23D2dClassificationV1::Released)
    );
    assert_eq!(model.snapshot().phase, R23D2dPhaseV1::DevicePairReady);
    begin(&mut model, 4096);
}

#[test]
fn explicit_indeterminate_or_currentness_loss_is_absorbing_quarantine() {
    let mut model = new_model();
    begin(&mut model, 4096);
    let plan = prepare(&mut model);
    publish(&mut model, &plan);
    assert!(matches!(
        model.poll_window_model_only(
            R23D2dPollDispositionV1::Indeterminate(
                R23D2dQuarantineReasonV1::CompletionIndeterminate,
            ),
            None,
        ),
        Ok(R23D2dClassificationV1::Quarantined(_))
    ));
    assert_eq!(
        model.poll_window_model_only(R23D2dPollDispositionV1::Pending, None),
        Err(R23D2dErrorV1::Quarantined)
    );

    let mut model = new_model();
    begin(&mut model, 4096);
    assert!(matches!(
        model.lose_currentness_model_only(),
        Ok(R23D2dClassificationV1::Quarantined(_))
    ));
    assert!(!model.snapshot().current);
}
