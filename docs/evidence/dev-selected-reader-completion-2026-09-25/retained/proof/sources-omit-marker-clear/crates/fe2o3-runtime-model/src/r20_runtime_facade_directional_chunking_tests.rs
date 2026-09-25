use alloc::vec;

use super::*;

const MIB: u64 = 1024 * 1024;

fn device() -> DeviceKeyV1 {
    DeviceKeyV1 {
        physical: PhysicalDeviceIdV1(0x200),
        generation: DeviceGenerationV1(9),
    }
}

fn vm() -> VmKeyV1 {
    VmKeyV1 {
        device: device(),
        id: VmIdV1(70),
    }
}

fn queue() -> QueueKeyV1 {
    QueueKeyV1 {
        vm: vm(),
        id: QueueInstanceIdV1(71),
        generation: QueueGenerationV1(72),
    }
}

fn admission(byte_len: u64) -> R19DirectionalAdmissionV1 {
    let allocation = MemoryAllocationKeyV1 {
        vm: vm(),
        id: AllocationIdV1(73),
        generation: AllocationGenerationV1(74),
    };
    R19DirectionalAdmissionV1 {
        allocation: R18LocalPersistentAllocationAdmissionV1 {
            owner: R17PersistentAllocationOwnerIdV1(75),
            allocation: MemoryAllocationRecordV1 {
                key: allocation,
                reservation: VaReservationKeyV1 {
                    vm: vm(),
                    id: VaReservationIdV1(76),
                },
                handle: UntrustedAllocationHandleObservationV1(77),
                spec: MemoryAllocationSpecV1 {
                    byte_len,
                    alignment: MEMORY_PAGE_BYTES_V1,
                    kind: MemoryKindV1::DeviceLocal,
                    coherence: MemoryCoherenceV1::ExplicitVisibility,
                },
                state: MemoryAllocationStateV1::Live,
            },
            mapping: MemoryMappingRecordV1 {
                key: MemoryMappingKeyV1 {
                    allocation,
                    id: MappingIdV1(78),
                },
                target_devices: vec![device()],
                access: MemoryAccessV1::ReadWrite,
                mapped_start: 0,
                mapped_end: 1,
                state: MemoryMappingStateV1::Mapped,
            },
            device: device(),
        },
        pair: R19DirectionalQueuePairV1 {
            parent_queue: queue(),
            pair_occurrence: 79,
            device_to_host: R19DirectionalChildQueueV1 {
                native_queue_id: 3,
                engine_id: R18_LOCAL_SDMA_DEVICE_TO_HOST_ENGINE_V1,
            },
            host_to_device: R19DirectionalChildQueueV1 {
                native_queue_id: 4,
                engine_id: R18_LOCAL_SDMA_HOST_TO_DEVICE_ENGINE_V1,
            },
        },
        pool_generation: 80,
        logical_byte_len: byte_len,
        physical_byte_len: byte_len,
    }
}

fn facade(byte_len: u64) -> R20RuntimeFacadeDirectionalChunkingV1 {
    R20RuntimeFacadeDirectionalChunkingV1::new_model_only(
        R19DirectionalPersistentLocalSdmaAdapterV1::new_model_only(admission(byte_len)).unwrap(),
    )
}

fn allocation(byte_len: u64) -> R18NativeAllocationKeyV1 {
    let facade = facade(byte_len);
    facade.snapshot().adapter.unwrap().allocation
}

fn host(byte_len: u64) -> R18HostBufferKeyV1 {
    R18HostBufferKeyV1 {
        session_id: 81,
        id: 82,
        generation: 83,
        byte_len,
        coherence: MemoryCoherenceV1::HostCoherent,
    }
}

fn request(
    byte_len: u64,
    direction: R18LocalSdmaDirectionV1,
    transfer_id: u64,
) -> R20CopyRequestV1 {
    let h = R20CopyEndpointV1::Host {
        buffer: host(byte_len),
        offset: 0,
    };
    let d = R20CopyEndpointV1::Device {
        allocation: allocation(byte_len),
        offset: 0,
    };
    match direction {
        R18LocalSdmaDirectionV1::HostToDevice => R20CopyRequestV1 {
            transfer_id,
            source: h,
            destination: d,
            byte_len,
        },
        R18LocalSdmaDirectionV1::DeviceToHost => R20CopyRequestV1 {
            transfer_id,
            source: d,
            destination: h,
            byte_len,
        },
    }
}

fn flush_confirmed(facade: &mut R20RuntimeFacadeDirectionalChunkingV1) -> R20FlushOutcomeV1 {
    facade
        .flush_model_only(&[], R20PublicationDispositionV1::Confirmed)
        .unwrap()
}

fn finish_transfer(facade: &mut R20RuntimeFacadeDirectionalChunkingV1) -> R20CompletionRecordV1 {
    loop {
        flush_confirmed(facade);
        match facade
            .poll_model_only(R20PollObservationV1::Succeeded)
            .unwrap()
        {
            R20PollOutcomeV1::ReadyContinuation { .. } => {}
            R20PollOutcomeV1::Completed(completion) => return completion,
            outcome => panic!("unexpected outcome: {outcome:?}"),
        }
    }
}

#[test]
fn unsupported_host_host_and_device_device_are_preflight_mutation_free() {
    let byte_len = 4096;
    let mut facade = facade(byte_len);
    let before = facade.snapshot();
    let h = R20CopyEndpointV1::Host {
        buffer: host(byte_len),
        offset: 0,
    };
    let d = R20CopyEndpointV1::Device {
        allocation: allocation(byte_len),
        offset: 0,
    };
    for (source, destination) in [(h, h), (d, d)] {
        assert_eq!(
            facade.enqueue_model_only(
                R20CopyRequestV1 {
                    transfer_id: 1,
                    source,
                    destination,
                    byte_len
                },
                vec![],
            ),
            Err(R20FacadeErrorV1::UnsupportedCopy)
        );
        assert_eq!(facade.snapshot(), before);
    }
}

#[test]
fn dependency_identity_and_satisfaction_gate_publication_without_mutation() {
    let byte_len = 4096;
    let mut facade = facade(byte_len);
    let dependency = R20DependencyV1 {
        event_id: 91,
        generation: 92,
    };
    facade
        .enqueue_model_only(
            request(byte_len, R18LocalSdmaDirectionV1::HostToDevice, 1),
            vec![dependency],
        )
        .unwrap();
    let before = facade.snapshot();
    assert_eq!(
        facade.flush_model_only(
            &[R20DependencyObservationV1 {
                dependency: R20DependencyV1 {
                    generation: 93,
                    ..dependency
                },
                status: R20DependencyStatusV1::Satisfied
            }],
            R20PublicationDispositionV1::Confirmed
        ),
        Err(R20FacadeErrorV1::DependencyMismatch)
    );
    assert_eq!(facade.snapshot(), before);
    assert_eq!(
        facade.flush_model_only(
            &[R20DependencyObservationV1 {
                dependency,
                status: R20DependencyStatusV1::Pending
            }],
            R20PublicationDispositionV1::Confirmed
        ),
        Ok(R20FlushOutcomeV1::DependencyPending)
    );
    assert_eq!(facade.snapshot(), before);
    assert!(matches!(
        facade.flush_model_only(
            &[R20DependencyObservationV1 {
                dependency,
                status: R20DependencyStatusV1::Satisfied
            }],
            R20PublicationDispositionV1::Confirmed
        ),
        Ok(R20FlushOutcomeV1::Published { .. })
    ));
}

#[test]
fn failed_and_quiescent_dependencies_settle_exact_retained_targets() {
    let byte_len = 4096;
    let dependency = R20DependencyV1 {
        event_id: 91,
        generation: 92,
    };
    for (id, status) in [
        (1, R20DependencyStatusV1::Failed),
        (2, R20DependencyStatusV1::QuiescentWithoutResult),
    ] {
        let mut facade = facade(byte_len);
        facade
            .enqueue_model_only(
                request(byte_len, R18LocalSdmaDirectionV1::HostToDevice, id),
                vec![dependency],
            )
            .unwrap();
        assert_eq!(facade.snapshot().retained_targets, vec![id]);
        assert_eq!(
            facade.flush_model_only(
                &[R20DependencyObservationV1 { dependency, status }],
                R20PublicationDispositionV1::Confirmed,
            ),
            Ok(R20FlushOutcomeV1::Quiescent)
        );
        assert!(facade.snapshot().active.is_none());
        match status {
            R20DependencyStatusV1::Failed => assert_eq!(
                facade.poll_submission_model_only(id),
                Ok(R20PollOutcomeV1::Completed(R20CompletionRecordV1 {
                    transfer_id: id,
                    succeeded: false,
                    failure_code: Some(-2),
                    completed_bytes: 0,
                }))
            ),
            R20DependencyStatusV1::QuiescentWithoutResult => assert!(matches!(
                facade.poll_submission_model_only(id),
                Ok(R20PollOutcomeV1::QuiescentWithoutResult(_))
            )),
            _ => unreachable!(),
        }
        facade.release_submission_model_only(id).unwrap();
        assert!(facade.snapshot().retained_targets.is_empty());
        assert_eq!(
            facade.poll_submission_model_only(id),
            Err(R20FacadeErrorV1::InvalidTransfer)
        );
    }
}

#[test]
fn h2d_and_d2h_bind_exact_storage_roles_and_destination_dirty_ranges() {
    for direction in [
        R18LocalSdmaDirectionV1::HostToDevice,
        R18LocalSdmaDirectionV1::DeviceToHost,
    ] {
        let byte_len = 8192;
        let mut facade = facade(byte_len);
        let req = request(byte_len, direction, 1);
        facade.enqueue_model_only(req, vec![]).unwrap();
        flush_confirmed(&mut facade);
        let published = facade.snapshot().active.unwrap();
        assert_eq!(published.direction, direction);
        assert_eq!(published.source, req.source);
        assert_eq!(published.destination, req.destination);
        let completion = facade
            .poll_model_only(R20PollObservationV1::Succeeded)
            .unwrap();
        assert_eq!(
            completion,
            R20PollOutcomeV1::Completed(R20CompletionRecordV1 {
                transfer_id: 1,
                succeeded: true,
                failure_code: None,
                completed_bytes: byte_len
            })
        );
        assert_eq!(
            facade.snapshot().destination_dirty,
            vec![R20DestinationDirtyV1 {
                transfer_id: 1,
                destination: req.destination,
                byte_offset: 0,
                byte_len
            }]
        );
    }
}

#[test]
fn multi_packet_offsets_are_exact_and_poll_never_publishes_continuation() {
    let byte_len = R18_SDMA_MAX_LINEAR_COPY_BYTES_V1 * 2 + 37;
    let mut facade = facade(byte_len.div_ceil(4096) * 4096);
    let allocation = facade.snapshot().adapter.unwrap().allocation;
    let req = R20CopyRequestV1 {
        transfer_id: 1,
        source: R20CopyEndpointV1::Host {
            buffer: host(byte_len),
            offset: 0,
        },
        destination: R20CopyEndpointV1::Device {
            allocation,
            offset: 0,
        },
        byte_len,
    };
    facade.enqueue_model_only(req, vec![]).unwrap();
    for (offset, size) in [
        (0, R18_SDMA_MAX_LINEAR_COPY_BYTES_V1),
        (
            R18_SDMA_MAX_LINEAR_COPY_BYTES_V1,
            R18_SDMA_MAX_LINEAR_COPY_BYTES_V1,
        ),
        (R18_SDMA_MAX_LINEAR_COPY_BYTES_V1 * 2, 37),
    ] {
        flush_confirmed(&mut facade);
        let active = facade.snapshot().active.unwrap();
        assert_eq!(active.packet_offset, Some(offset));
        assert_eq!(active.packet_byte_len, Some(size));
        let outcome = facade
            .poll_model_only(R20PollObservationV1::Succeeded)
            .unwrap();
        if offset + size < byte_len {
            assert_eq!(
                outcome,
                R20PollOutcomeV1::ReadyContinuation {
                    completed_bytes: offset + size
                }
            );
            let snapshot = facade.snapshot();
            assert_eq!(snapshot.phase, R20FacadePacketPhaseV1::Ready);
            assert_eq!(snapshot.active.unwrap().packet_offset, None);
        }
    }
    assert_eq!(facade.snapshot().destination_dirty.len(), 3);
}

#[test]
fn full_256_mib_transfer_requires_65_exact_r19_lifecycles() {
    let byte_len = 256 * MIB;
    let mut facade = facade(byte_len);
    facade
        .enqueue_model_only(
            request(byte_len, R18LocalSdmaDirectionV1::HostToDevice, 1),
            vec![],
        )
        .unwrap();
    let completion = finish_transfer(&mut facade);
    assert_eq!(completion.completed_bytes, byte_len);
    assert_eq!(facade.snapshot().destination_dirty.len(), 65);
    assert_eq!(
        facade.snapshot().adapter.unwrap().settled_transfer_count,
        65
    );
}

#[test]
fn exact_retirement_allows_repeated_and_mixed_direction_transfers() {
    let byte_len = 4096;
    let mut facade = facade(byte_len);
    for (id, direction) in [
        (1, R18LocalSdmaDirectionV1::HostToDevice),
        (2, R18LocalSdmaDirectionV1::HostToDevice),
        (3, R18LocalSdmaDirectionV1::DeviceToHost),
        (4, R18LocalSdmaDirectionV1::DeviceToHost),
    ] {
        facade
            .enqueue_model_only(request(byte_len, direction, id), vec![])
            .unwrap();
        assert!(finish_transfer(&mut facade).succeeded);
        assert_eq!(facade.snapshot().adapter.unwrap().pending_frontier, None);
    }
    assert_eq!(facade.snapshot().completions.len(), 4);
}

#[test]
fn pending_and_timeout_retain_exact_ticket_and_packet() {
    let byte_len = 4096;
    let mut facade = facade(byte_len);
    facade
        .enqueue_model_only(
            request(byte_len, R18LocalSdmaDirectionV1::DeviceToHost, 1),
            vec![],
        )
        .unwrap();
    flush_confirmed(&mut facade);
    let published = facade.snapshot();
    assert_eq!(
        facade.poll_model_only(R20PollObservationV1::Pending),
        Ok(R20PollOutcomeV1::Pending)
    );
    assert_eq!(facade.snapshot(), published);
    assert_eq!(
        facade.poll_model_only(R20PollObservationV1::TimedOut),
        Ok(R20PollOutcomeV1::TimedOut)
    );
    assert_eq!(
        facade.snapshot().active.unwrap().ticket,
        published.active.unwrap().ticket
    );
    assert!(matches!(
        facade.poll_model_only(R20PollObservationV1::Succeeded),
        Ok(R20PollOutcomeV1::Completed(_))
    ));
}

#[test]
fn cancellation_is_only_legal_before_any_completed_byte() {
    let byte_len = R18_SDMA_MAX_LINEAR_COPY_BYTES_V1 + 1;
    let physical = byte_len.div_ceil(4096) * 4096;
    let mut facade = facade(physical);
    let allocation = facade.snapshot().adapter.unwrap().allocation;
    let req = R20CopyRequestV1 {
        transfer_id: 1,
        source: R20CopyEndpointV1::Host {
            buffer: host(byte_len),
            offset: 0,
        },
        destination: R20CopyEndpointV1::Device {
            allocation,
            offset: 0,
        },
        byte_len,
    };
    facade.enqueue_model_only(req, vec![]).unwrap();
    assert_eq!(facade.cancel_model_only(), Ok(1));
    facade.enqueue_model_only(req, vec![]).unwrap();
    flush_confirmed(&mut facade);
    assert_eq!(facade.cancel_model_only(), Err(R20FacadeErrorV1::TooLate));
    assert!(matches!(
        facade.poll_model_only(R20PollObservationV1::Succeeded),
        Ok(R20PollOutcomeV1::ReadyContinuation { .. })
    ));
    assert_eq!(facade.cancel_model_only(), Err(R20FacadeErrorV1::TooLate));
}

#[test]
fn retryable_publication_settles_failed_or_partial_quiescent_custody() {
    let byte_len = R18_SDMA_MAX_LINEAR_COPY_BYTES_V1 + 1;
    let physical = byte_len.div_ceil(4096) * 4096;
    let mut facade = facade(physical);
    let allocation = facade.snapshot().adapter.unwrap().allocation;
    facade
        .enqueue_model_only(
            R20CopyRequestV1 {
                transfer_id: 1,
                source: R20CopyEndpointV1::Host {
                    buffer: host(byte_len),
                    offset: 0,
                },
                destination: R20CopyEndpointV1::Device {
                    allocation,
                    offset: 0,
                },
                byte_len,
            },
            vec![],
        )
        .unwrap();
    assert_eq!(
        facade.flush_model_only(
            &[],
            R20PublicationDispositionV1::RetryableBeforeQueueCustody
        ),
        Ok(R20FlushOutcomeV1::Quiescent)
    );
    assert_eq!(facade.snapshot().phase, R20FacadePacketPhaseV1::Completed);
    assert_eq!(
        facade.poll_submission_model_only(1),
        Ok(R20PollOutcomeV1::Completed(R20CompletionRecordV1 {
            transfer_id: 1,
            succeeded: false,
            failure_code: Some(-1),
            completed_bytes: 0
        }))
    );
    facade.release_submission_model_only(1).unwrap();
    facade
        .enqueue_model_only(
            R20CopyRequestV1 {
                transfer_id: 1,
                source: R20CopyEndpointV1::Host {
                    buffer: host(byte_len),
                    offset: 0,
                },
                destination: R20CopyEndpointV1::Device {
                    allocation,
                    offset: 0,
                },
                byte_len,
            },
            vec![],
        )
        .unwrap();
    flush_confirmed(&mut facade);
    facade
        .poll_model_only(R20PollObservationV1::Succeeded)
        .unwrap();
    assert_eq!(
        facade.flush_model_only(
            &[],
            R20PublicationDispositionV1::RetryableBeforeQueueCustody
        ),
        Ok(R20FlushOutcomeV1::Quiescent)
    );
    let snapshot = facade.snapshot();
    assert_eq!(
        snapshot.phase,
        R20FacadePacketPhaseV1::QuiescentWithoutResult
    );
    assert!(snapshot.active.is_none());
    assert_eq!(
        snapshot.quiescent_without_result,
        Some(R20QuiescentWithoutResultV1 {
            transfer_id: 1,
            completed_bytes: R18_SDMA_MAX_LINEAR_COPY_BYTES_V1,
            total_bytes: byte_len,
        })
    );
    assert_eq!(
        facade.poll_submission_model_only(2),
        Err(R20FacadeErrorV1::InvalidTransfer)
    );
    let quiescent = R20QuiescentWithoutResultV1 {
        transfer_id: 1,
        completed_bytes: R18_SDMA_MAX_LINEAR_COPY_BYTES_V1,
        total_bytes: byte_len,
    };
    assert_eq!(
        facade.poll_submission_model_only(1),
        Ok(R20PollOutcomeV1::QuiescentWithoutResult(quiescent))
    );
    assert_eq!(
        facade.poll_submission_model_only(1),
        Ok(R20PollOutcomeV1::QuiescentWithoutResult(quiescent))
    );
    assert_eq!(
        facade
            .release_quiescent_model_only(1)
            .unwrap()
            .completed_bytes,
        R18_SDMA_MAX_LINEAR_COPY_BYTES_V1
    );
    assert_eq!(facade.snapshot().phase, R20FacadePacketPhaseV1::Idle);
}

#[test]
fn opaque_publication_and_currentness_failures_retain_process_teardown_custody() {
    for publication_opaque in [true, false] {
        let byte_len = 4096;
        let mut facade = facade(byte_len);
        facade
            .enqueue_model_only(
                request(byte_len, R18LocalSdmaDirectionV1::HostToDevice, 1),
                vec![],
            )
            .unwrap();
        if publication_opaque {
            assert_eq!(
                facade.flush_model_only(&[], R20PublicationDispositionV1::OpaqueAfterPacketWrite),
                Err(R20FacadeErrorV1::ProcessTeardown)
            );
        } else {
            flush_confirmed(&mut facade);
            assert_eq!(
                facade.poll_model_only(R20PollObservationV1::CurrentnessAmbiguous),
                Err(R20FacadeErrorV1::ProcessTeardown)
            );
        }
        assert_eq!(
            facade.snapshot().phase,
            R20FacadePacketPhaseV1::ProcessTeardown
        );
        assert_eq!(facade.terminal_custody_kind(), Some("quarantined"));
    }
}

#[test]
fn failed_packet_retires_frontier_without_dirtying_destination() {
    let byte_len = 4096;
    let mut facade = facade(byte_len);
    facade
        .enqueue_model_only(
            request(byte_len, R18LocalSdmaDirectionV1::HostToDevice, 1),
            vec![],
        )
        .unwrap();
    flush_confirmed(&mut facade);
    assert_eq!(
        facade.poll_model_only(R20PollObservationV1::Failed { code: -7 }),
        Ok(R20PollOutcomeV1::Completed(R20CompletionRecordV1 {
            transfer_id: 1,
            succeeded: false,
            failure_code: Some(-7),
            completed_bytes: 0
        }))
    );
    let snapshot = facade.snapshot();
    assert!(snapshot.destination_dirty.is_empty());
    assert_eq!(snapshot.adapter.unwrap().pending_frontier, None);
}

#[test]
fn successful_completion_remains_exactly_pollable_until_retain_release() {
    let byte_len = 4096;
    let mut facade = facade(byte_len);
    facade
        .enqueue_model_only(
            request(byte_len, R18LocalSdmaDirectionV1::HostToDevice, 1),
            vec![],
        )
        .unwrap();
    let completion = finish_transfer(&mut facade);
    assert_eq!(facade.snapshot().retained_targets, vec![1]);
    assert_eq!(
        facade.poll_submission_model_only(1),
        Ok(R20PollOutcomeV1::Completed(completion))
    );
    facade.release_submission_model_only(1).unwrap();
    assert!(facade.snapshot().retained_targets.is_empty());
    assert_eq!(
        facade.poll_submission_model_only(1),
        Err(R20FacadeErrorV1::InvalidTransfer)
    );
}

#[test]
fn logical_host_and_allocation_identity_bounds_are_enforced_before_mutation() {
    let byte_len = 8192;
    let mut facade = facade(byte_len);
    let before = facade.snapshot();
    let mut wrong = request(byte_len, R18LocalSdmaDirectionV1::HostToDevice, 1);
    if let R20CopyEndpointV1::Device {
        ref mut allocation, ..
    } = wrong.destination
    {
        allocation.owner = R17PersistentAllocationOwnerIdV1(999);
    }
    assert_eq!(
        facade.enqueue_model_only(wrong, vec![]),
        Err(R20FacadeErrorV1::InvalidEndpoint)
    );
    assert_eq!(facade.snapshot(), before);
    let mut too_large = request(byte_len, R18LocalSdmaDirectionV1::HostToDevice, 1);
    too_large.byte_len += 1;
    assert_eq!(
        facade.enqueue_model_only(too_large, vec![]),
        Err(R20FacadeErrorV1::InvalidRange)
    );
    assert_eq!(facade.snapshot(), before);
}
