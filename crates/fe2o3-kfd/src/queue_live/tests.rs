#[path = "../tests/dependency_source_failure_tests.rs"]
mod dependency_source_failure_tests;
#[path = "../tests/dependency_source_publication_tests.rs"]
mod dependency_source_publication_tests;
#[path = "../tests/runtime_completion_tests.rs"]
mod runtime_completion_tests;
#[path = "../tests/runtime_publication_tests.rs"]
mod runtime_publication_tests;

use super::*;

pub(super) fn test_queue_key(queue: u64, generation: u64) -> QueueKeyV1 {
    QueueKeyV1 {
        vm: fe2o3_runtime_model::VmKeyV1 {
            device: fe2o3_runtime_model::DeviceKeyV1 {
                physical: fe2o3_runtime_model::PhysicalDeviceIdV1(7),
                generation: fe2o3_runtime_model::DeviceGenerationV1(11),
            },
            id: fe2o3_runtime_model::VmIdV1(13),
        },
        id: QueueInstanceIdV1(queue),
        generation: QueueGenerationV1(generation),
    }
}

fn persistent_prepared_custody_fixture(
    direction: Gfx942PersistentSdmaDirectionV1,
    id: u64,
) -> (
    PersistentSdmaPreparedCustodyV1,
    Gfx942SdmaCopyRequestV1,
    Gfx942SdmaCopyTicketV1,
) {
    let (allocation, prepared, request, host_binding) = persistent_restore_fixture(direction, id);
    let ticket = crate::sdma::persistent_sdma_ticket_for_test(
        allocation.attachment.queue,
        allocation.attachment.native_queue_id,
    );
    (
        PersistentSdmaPreparedCustodyV1 {
            allocation,
            prepared,
            planned_ticket: ticket,
            host_binding,
            direction,
            host_offset: 8,
            device_offset: 16,
            copy_bytes: 32,
        },
        request,
        ticket,
    )
}

fn persistent_published_custody_fixture(
    direction: Gfx942PersistentSdmaDirectionV1,
    id: u64,
) -> (Gfx942PersistentSdmaSubmissionV1, Gfx942SdmaCopyRequestV1) {
    let (custody, request, ticket) = persistent_prepared_custody_fixture(direction, id);
    let transition = transition_persistent_sdma_publication_v1(
        custody,
        PersistentSdmaPublicationObservationV1::Confirmed(ticket),
        true,
        true,
    );
    let PersistentSdmaPublicationTransitionV1::Published(submission) = transition else {
        panic!("exact confirmed publication must publish")
    };
    (submission, request)
}

fn prepare_restored_persistent_custody(
    mut allocation: Gfx942QueuePersistentAllocationV1,
    host: Gfx942SdmaBufferV1,
    dependency: Option<&Gfx942PersistentDependencyFrontierV1>,
) -> (
    PersistentSdmaPreparedCustodyV1,
    Gfx942SdmaCopyRequestV1,
    Gfx942SdmaCopyTicketV1,
) {
    let direction = allocation.direction();
    let operation = match direction {
        Gfx942PersistentSdmaDirectionV1::HostToDevice => {
            Gfx942PersistentOperationV1::LocalSdmaDestination
        }
        Gfx942PersistentSdmaDirectionV1::DeviceToHost => {
            Gfx942PersistentOperationV1::LocalSdmaSource
        }
    };
    let reserved = allocation
        .owner
        .reserve(
            Gfx942PersistentUseRequestV1::new(operation, 16, 32).unwrap(),
            dependency,
        )
        .unwrap();
    let prepared = allocation.owner.prepare(reserved).unwrap();
    let lease = allocation.owner.detach_local_native_for_sdma().unwrap();
    let device = Gfx942SdmaBufferV1::from_bridge_parts(
        Gfx942SdmaBufferStorageV1::Device(lease),
        allocation.attachment.queue,
        allocation.attachment.pool_generation,
        allocation.attachment.logical_bytes,
    );
    let request = persistent_sdma_request(direction, host, 8, device, 16, 32);
    let host_binding = match direction {
        Gfx942PersistentSdmaDirectionV1::HostToDevice => {
            Gfx942PersistentSdmaHostBindingV1::capture(&request.source, allocation.attachment.queue)
        }
        Gfx942PersistentSdmaDirectionV1::DeviceToHost => {
            Gfx942PersistentSdmaHostBindingV1::capture(
                &request.destination,
                allocation.attachment.queue,
            )
        }
    };
    let ticket = crate::sdma::persistent_sdma_ticket_for_test(
        allocation.attachment.queue,
        allocation.attachment.native_queue_id,
    );
    (
        PersistentSdmaPreparedCustodyV1 {
            allocation,
            prepared,
            planned_ticket: ticket,
            host_binding,
            direction,
            host_offset: 8,
            device_offset: 16,
            copy_bytes: 32,
        },
        request,
        ticket,
    )
}

fn completed_persistent_request(request: Gfx942SdmaCopyRequestV1) -> Gfx942SdmaCompletedCopyV1 {
    let Gfx942SdmaCopyRequestV1 {
        source,
        source_offset,
        destination,
        destination_offset,
        copy_bytes,
    } = request;
    Gfx942SdmaCompletedCopyV1 {
        source,
        source_offset,
        destination,
        destination_offset,
        copy_bytes,
    }
}

pub(super) fn persistent_compute_cancellation_test_session(
    queue: QueueKeyV1,
    persistent_compute: Option<PersistentComputeAttachmentV1>,
    release: Option<(u64, Vec<Gfx942FixedDispatchDataV1>)>,
) -> ComputeAqlQueueSessionV1 {
    ComputeAqlQueueSessionV1 {
        dispatch_capacity: Gfx942FixedDispatchCapacityV1::default(),
        engine: None,
        key: queue,
        compute_lane_session: queue,
        doorbell: None,
        submission: None,
        completion_signals: None,
        completion_owner: QueueOwnerSlotV1(Some(
            CompletionSignalArenaOwnerV1::for_persistent_compute_cancellation_test(queue),
        )),
        dependency_owner: QueueOwnerSlotV1(Some(
            ComputeDependencySessionOwnerV1::new(queue.id.0).unwrap(),
        )),
        terminal_dependency: None,
        dispatch: None,
        unpublished_dispatch: UnpublishedDispatchStateV1::default(),
        detached_data_count: 0,
        detached_dispatch_generation: None,
        detached_data_identities: Vec::new(),
        detached_next_insertion_index: None,
        persistent_compute: persistent_compute
            .map(BoundedPersistentComputeAttachmentV1::from_single),
        persistent_compute_test_release: release,
        next_persistent_compute_generation: 2,
        exception: None,
        sdma: None,
        striped_sdma: None,
        xgmi_attachment: None,
        sdma_outstanding_buffers: 0,
        sdma_pool_free: Vec::new(),
        sdma_pool_trim: None,
        sdma_allocation: None,
        sdma_promotion: None,
        sdma_demotion: None,
        initialized_storage_promotion: None,
        sdma_synchronous: None,
        sdma_recycle: None,
        auxiliary_release: None,
        sdma_pool_reuse_count: 0,
        sdma_device_pool: SdmaDevicePoolConfigurationV1::default(),
        sdma_host_pool_limits: None,
        terminal_poisoned: false,
        observation: ComputeAqlQueueObservationV1 {
            queue_id: 0,
            ring_bytes: 0,
            doorbell_slice_bytes: 0,
            doorbell_byte_offset: 0,
            event_id: 0,
            cwsr_shadow_pages: 0,
        },
        auxiliary_compute_lanes: Vec::new(),
    }
}

fn persistent_compute_gate_test_allocation_v1(
    queue: QueueKeyV1,
    id: u64,
) -> Gfx942DirectionalQueuePersistentAllocationV1 {
    let (mut device, _host) = crate::sdma::persistent_sdma_buffers_for_test(queue, id);
    device.set_logical_bytes(device.physical_bytes());
    let pair = admit_persistent_directional_sdma_pair_v1(Gfx942DirectionalSdmaQueueObservationV1 {
        host_to_device: Gfx942SdmaQueueObservationV1 {
            queue_id: 17,
            ring_bytes: crate::sdma::GFX942_SDMA_RING_BYTES_V1,
            maximum_in_flight: crate::sdma::GFX942_SDMA_MAX_IN_FLIGHT_V1 as u16,
            engine_index: Some(crate::sdma::GFX942_SDMA_H2D_ENGINE_INDEX_V1),
        },
        device_to_host: Gfx942SdmaQueueObservationV1 {
            queue_id: 23,
            ring_bytes: crate::sdma::GFX942_SDMA_RING_BYTES_V1,
            maximum_in_flight: crate::sdma::GFX942_SDMA_MAX_IN_FLIGHT_V1 as u16,
            engine_index: Some(crate::sdma::GFX942_SDMA_D2H_ENGINE_INDEX_V1),
        },
        admitted_engine_count: 2,
        admitted_queues_per_engine: 8,
    })
    .unwrap();
    let (allocation, outstanding) =
        promote_directional_persistent_sdma_custody_v1(device, pair, 2).unwrap();
    assert_eq!(outstanding, 2);
    allocation
}

fn persistent_compute_gate_test_entry_v1(
    queue: QueueKeyV1,
    id: u64,
    effect: Gfx942PersistentComputeEffectV1,
) -> PersistentComputeAttachmentEntryV1 {
    let mut allocation = persistent_compute_gate_test_allocation_v1(queue, id);
    let storage_identity = allocation
        .owner
        .local_native_for_sdma()
        .expect("gate fixture retains native storage")
        .storage_identity();
    allocation
        .owner
        .quarantine_for_caller_reported_currentness_loss();
    PersistentComputeAttachmentEntryV1 {
        allocation,
        initialization: PersistentComputeInitializationV1::AuthenticatedH2d([id as u8; 32]),
        state: PersistentComputeUseStateV1::Quarantined,
        storage_identity: Some(storage_identity),
        effect,
    }
}

fn published_three_binding_test_entry_v1(
    queue: QueueKeyV1,
    id: u64,
    effect: Gfx942PersistentComputeEffectV1,
) -> PersistentComputeAttachmentEntryV1 {
    let mut allocation = persistent_compute_gate_test_allocation_v1(queue, id);
    let storage_identity = allocation
        .owner
        .local_native_for_sdma()
        .expect("published fixture retains native storage")
        .storage_identity();
    let operation = match effect {
        Gfx942PersistentComputeEffectV1::Read => Gfx942PersistentOperationV1::ComputeRead,
        Gfx942PersistentComputeEffectV1::Write => Gfx942PersistentOperationV1::ComputeWrite,
        Gfx942PersistentComputeEffectV1::ReadWrite => Gfx942PersistentOperationV1::ComputeReadWrite,
    };
    let reserved = allocation
        .owner
        .reserve(
            Gfx942PersistentUseRequestV1::new(operation, 0, allocation.byte_len()).unwrap(),
            None,
        )
        .unwrap();
    let prepared = allocation.owner.prepare(reserved).unwrap();
    let published = allocation.owner.publish(prepared).unwrap();
    PersistentComputeAttachmentEntryV1 {
        allocation,
        initialization: PersistentComputeInitializationV1::AuthenticatedH2d([id as u8; 32]),
        state: PersistentComputeUseStateV1::Published(published),
        storage_identity: Some(storage_identity),
        effect,
    }
}

pub(super) fn persistent_compute_gate_test_session_v1(
    queue: QueueKeyV1,
    binding_count: usize,
) -> ComputeAqlQueueSessionV1 {
    let binding = PersistentComputeBindingKeyV1 {
        queue,
        attachment_generation: 1,
    };
    let attachment = match binding_count {
        1 => {
            let entry = persistent_compute_gate_test_entry_v1(
                queue,
                0x5301,
                Gfx942PersistentComputeEffectV1::ReadWrite,
            );
            BoundedPersistentComputeAttachmentV1::from_single(PersistentComputeAttachmentV1 {
                allocation: entry.allocation,
                initialization: entry.initialization,
                state: entry.state,
                binding,
                storage_identity: entry
                    .storage_identity
                    .expect("single gate fixture retains storage identity"),
                effect: entry.effect,
                predecessor_dispatch_generation: None,
                terminal_custody: None,
            })
        }
        3 => BoundedPersistentComputeAttachmentV1::from_three(
            ThreeBindingPersistentComputeAttachmentV1 {
                entries: [
                    persistent_compute_gate_test_entry_v1(
                        queue,
                        0x5302,
                        Gfx942PersistentComputeEffectV1::Read,
                    ),
                    persistent_compute_gate_test_entry_v1(
                        queue,
                        0x5303,
                        Gfx942PersistentComputeEffectV1::Read,
                    ),
                    persistent_compute_gate_test_entry_v1(
                        queue,
                        0x5304,
                        Gfx942PersistentComputeEffectV1::Write,
                    ),
                ],
                binding,
                predecessor_dispatch_generation: None,
                terminal_custody: None,
            },
        ),
        _ => panic!("gate fixture supports exact N=1 or N=3"),
    };
    let mut session = persistent_compute_cancellation_test_session(queue, None, None);
    session.persistent_compute = Some(attachment);
    session
}

pub(super) fn compute_lane_state_for_multi_inflight_test(
    queue: QueueKeyV1,
) -> ComputeAqlQueueLaneStateV1 {
    ComputeAqlQueueLaneStateV1 {
        key: queue,
        doorbell: None,
        submission: None,
        completion_signals: None,
        completion_owner: QueueOwnerSlotV1(Some(
            CompletionSignalArenaOwnerV1::for_persistent_compute_cancellation_test(queue),
        )),
        dispatch: None,
        unpublished_dispatch: UnpublishedDispatchStateV1::default(),
        detached_data_count: 0,
        detached_dispatch_generation: None,
        detached_data_identities: Vec::new(),
        detached_next_insertion_index: None,
        exception: None,
        observation: ComputeAqlQueueObservationV1 {
            queue_id: 0,
            ring_bytes: 0,
            doorbell_slice_bytes: 0,
            doorbell_byte_offset: 0,
            event_id: 0,
            cwsr_shadow_pages: 0,
        },
    }
}

pub(super) fn prepared_persistent_compute_cancellation_fixture(
    queue: QueueKeyV1,
    id: u64,
    authenticated_sha256: Option<[u8; 32]>,
    predecessor_dispatch_generation: Option<u64>,
) -> (
    ComputeAqlQueueSessionV1,
    Gfx942PreparedPersistentComputeDispatchV1,
    crate::Gfx942DeviceMemoryIdentityV1,
) {
    prepared_persistent_compute_cancellation_fixture_with_initialization_v1(
        queue,
        id,
        authenticated_sha256,
        predecessor_dispatch_generation,
        true,
    )
}

fn prepared_persistent_compute_cancellation_fixture_with_initialization_v1(
    queue: QueueKeyV1,
    id: u64,
    authenticated_sha256: Option<[u8; 32]>,
    predecessor_dispatch_generation: Option<u64>,
    fully_initialized: bool,
) -> (
    ComputeAqlQueueSessionV1,
    Gfx942PreparedPersistentComputeDispatchV1,
    crate::Gfx942DeviceMemoryIdentityV1,
) {
    let (mut device, _host) = crate::sdma::persistent_sdma_buffers_for_test(queue, id);
    device.set_logical_bytes(device.physical_bytes());
    let pair = admit_persistent_directional_sdma_pair_v1(Gfx942DirectionalSdmaQueueObservationV1 {
        host_to_device: Gfx942SdmaQueueObservationV1 {
            queue_id: 17,
            ring_bytes: crate::sdma::GFX942_SDMA_RING_BYTES_V1,
            maximum_in_flight: crate::sdma::GFX942_SDMA_MAX_IN_FLIGHT_V1 as u16,
            engine_index: Some(crate::sdma::GFX942_SDMA_H2D_ENGINE_INDEX_V1),
        },
        device_to_host: Gfx942SdmaQueueObservationV1 {
            queue_id: 23,
            ring_bytes: crate::sdma::GFX942_SDMA_RING_BYTES_V1,
            maximum_in_flight: crate::sdma::GFX942_SDMA_MAX_IN_FLIGHT_V1 as u16,
            engine_index: Some(crate::sdma::GFX942_SDMA_D2H_ENGINE_INDEX_V1),
        },
        admitted_engine_count: 2,
        admitted_queues_per_engine: 8,
    })
    .unwrap();
    let (mut allocation, outstanding) =
        promote_directional_persistent_sdma_custody_v1(device, pair, 2).unwrap();
    assert_eq!(outstanding, 2);
    let storage_identity = allocation
        .owner
        .local_native_for_sdma()
        .expect("promoted allocation retains local native custody")
        .storage_identity();
    let effect = if fully_initialized {
        Gfx942PersistentComputeEffectV1::ReadWrite
    } else {
        Gfx942PersistentComputeEffectV1::Write
    };
    let request = Gfx942PersistentUseRequestV1::new(
        if fully_initialized {
            Gfx942PersistentOperationV1::ComputeReadWrite
        } else {
            Gfx942PersistentOperationV1::ComputeWrite
        },
        0,
        allocation.byte_len(),
    )
    .unwrap();
    let reserved = allocation.owner.reserve(request, None).unwrap();
    let prepared = allocation.owner.prepare(reserved).unwrap();
    let lease = allocation
        .owner
        .detach_local_native_for_compute(&prepared)
        .unwrap();
    let data = if fully_initialized {
        Gfx942FixedDispatchDataV1::initialized_storage(lease)
    } else {
        Gfx942FixedDispatchDataV1::uninitialized(lease)
    };
    let binding = PersistentComputeBindingKeyV1 {
        queue,
        attachment_generation: 1,
    };
    let attachment = PersistentComputeAttachmentV1 {
        allocation,
        initialization: PersistentComputeInitializationV1::from_test_parts(
            authenticated_sha256,
            fully_initialized,
        ),
        state: PersistentComputeUseStateV1::Prepared(prepared),
        binding,
        storage_identity,
        effect,
        predecessor_dispatch_generation,
        terminal_custody: None,
    };
    let session = persistent_compute_cancellation_test_session(
        queue,
        Some(attachment),
        Some((predecessor_dispatch_generation.unwrap_or(0), vec![data])),
    );
    (
        session,
        Gfx942PreparedPersistentComputeDispatchV1 {
            binding,
            thread_affinity: PhantomData,
        },
        storage_identity,
    )
}

pub(super) fn prepared_three_binding_persistent_compute_cancellation_fixture_v1(
    queue: QueueKeyV1,
) -> (
    ComputeAqlQueueSessionV1,
    Gfx942PreparedThreeBindingPersistentComputeDispatchV1,
    [crate::Gfx942DeviceMemoryIdentityV1; 3],
    [[u8; 32]; 3],
) {
    let digests = [[0xa1; 32], [0xb2; 32], [0xc3; 32]];
    let effects = [
        Gfx942PersistentComputeEffectV1::Read,
        Gfx942PersistentComputeEffectV1::Read,
        Gfx942PersistentComputeEffectV1::Write,
    ];
    let make = |index: usize| {
        let mut allocation = persistent_compute_gate_test_allocation_v1(
            queue,
            0x5900 + u64::try_from(index).unwrap(),
        );
        let storage_identity = allocation
            .owner
            .local_native_for_sdma()
            .expect("prepared fixture retains native storage")
            .storage_identity();
        let operation = match effects[index] {
            Gfx942PersistentComputeEffectV1::Read => Gfx942PersistentOperationV1::ComputeRead,
            Gfx942PersistentComputeEffectV1::Write => Gfx942PersistentOperationV1::ComputeWrite,
            Gfx942PersistentComputeEffectV1::ReadWrite => {
                Gfx942PersistentOperationV1::ComputeReadWrite
            }
        };
        let request =
            Gfx942PersistentUseRequestV1::new(operation, 0, allocation.byte_len()).unwrap();
        let reserved = allocation.owner.reserve(request, None).unwrap();
        let prepared = allocation.owner.prepare(reserved).unwrap();
        let lease = allocation
            .owner
            .detach_local_native_for_compute(&prepared)
            .unwrap();
        (
            PersistentComputeAttachmentEntryV1 {
                allocation,
                initialization: PersistentComputeInitializationV1::AuthenticatedH2d(digests[index]),
                state: PersistentComputeUseStateV1::Prepared(prepared),
                storage_identity: Some(storage_identity),
                effect: effects[index],
            },
            Gfx942FixedDispatchDataV1::initialized_storage(lease),
            storage_identity,
        )
    };
    let [
        (entry_a, data_a, identity_a),
        (entry_b, data_b, identity_b),
        (entry_c, data_c, identity_c),
    ] = [0, 1, 2].map(make);
    let binding = PersistentComputeBindingKeyV1 {
        queue,
        attachment_generation: 1,
    };
    let mut session = persistent_compute_cancellation_test_session(
        queue,
        None,
        Some((7, vec![data_a, data_b, data_c])),
    );
    session.set_three_binding_persistent_compute_attachment_v1(
        ThreeBindingPersistentComputeAttachmentV1 {
            entries: [entry_a, entry_b, entry_c],
            binding,
            predecessor_dispatch_generation: Some(7),
            terminal_custody: None,
        },
    );
    (
        session,
        Gfx942PreparedThreeBindingPersistentComputeDispatchV1 {
            binding,
            thread_affinity: PhantomData,
        },
        [identity_a, identity_b, identity_c],
        digests,
    )
}

fn prepared_publication_identity_v1(
    state: &PersistentComputeUseStateV1,
) -> crate::persistent_allocation::PersistentUseIdentityForTestV1 {
    let PersistentComputeUseStateV1::Prepared(lease) = state else {
        panic!("prepared publication lease required");
    };
    lease.cancellation_identity_for_test()
}

struct TestPersistentLedgerEntryV1 {
    owner: Gfx942PersistentDeviceAllocationV1,
    state: PersistentComputeUseStateV1,
}

impl PersistentComputeLedgerEntryV1 for TestPersistentLedgerEntryV1 {
    fn owner_and_state_v1(
        &mut self,
    ) -> (
        &mut Gfx942PersistentDeviceAllocationV1,
        &mut PersistentComputeUseStateV1,
    ) {
        (&mut self.owner, &mut self.state)
    }
}

fn prepared_test_persistent_ledger_entry_v1(id: u64) -> TestPersistentLedgerEntryV1 {
    let mut owner = Gfx942PersistentDeviceAllocationV1::from_local_mapping(
        crate::shared_memory::local_mapping_for_persistent_sdma_test(id),
    );
    let request = Gfx942PersistentUseRequestV1::new(
        Gfx942PersistentOperationV1::ComputeReadWrite,
        0,
        owner.byte_len(),
    )
    .unwrap();
    let reserved = owner.reserve(request, None).unwrap();
    let prepared = owner.prepare(reserved).unwrap();
    TestPersistentLedgerEntryV1 {
        owner,
        state: PersistentComputeUseStateV1::Prepared(prepared),
    }
}

#[derive(Clone, Copy)]
struct BarrierSnapshotInput {
    packet_count: u16,
    write: u64,
    read: u64,
    header: u16,
    setup: u16,
    kind: i64,
    signal: Gfx942TimeoutSignalObservationV1,
    reason: u64,
}

impl BarrierSnapshotInput {
    fn valid(read: u64) -> Self {
        Self {
            packet_count: 1,
            write: 1,
            read,
            header: fe2o3_aql::AQL_SYSTEM_SCOPED_BARRIER_AND_HEADER_V1,
            setup: 0,
            kind: fe2o3_aql::AMD_SIGNAL_KIND_USER_V1,
            signal: Gfx942TimeoutSignalObservationV1::Completed,
            reason: 0,
        }
    }

    fn observation(self) -> Gfx942TimeoutExecutionObservationV1 {
        Gfx942TimeoutExecutionObservationV1::new(
            self.packet_count,
            self.write,
            self.read,
            self.header,
            self.setup,
            self.kind,
            self.signal,
            self.reason,
        )
    }
}

fn detached_preflight(
    dispatch_attached: bool,
    data_count: usize,
    generation: Option<u64>,
    identity_count: usize,
    returned_count: usize,
    identity_mismatch: Option<usize>,
) -> DetachedReturningDestroyPreflightV1 {
    DetachedReturningDestroyPreflightV1 {
        dispatch_attached,
        detached_data_count: data_count,
        detached_dispatch_generation: generation,
        detached_identity_count: identity_count,
        returned_data_count: returned_count,
        identity_mismatch,
    }
}

#[derive(Clone, Copy, Debug)]
enum InjectedPostHandoffFailureV1 {
    EventCreation,
    ShadowInstallation,
    ShadowInitialization,
    ResourceSealing,
    ResourceMapping,
    ModelTransfer,
    EngineAdmission,
    SubmissionModel,
    QueueCreateNoEffect,
    QueueCreateIndeterminate,
    RuntimeQueueTransition,
    CreateOutputs,
    NativeQueueId,
    SessionComposition,
    PreDoorbellCurrentness,
    DoorbellMapping,
    PostDoorbellCurrentness,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RetainedReplayInjectedStageV1 {
    MappedFacts,
    Detach,
    AuthenticatedConstruction,
    Retain,
    FinalAudit,
}

struct RetainedReplayScriptV1 {
    fail: Option<RetainedReplayInjectedStageV1>,
    panic: bool,
    trace: Vec<RetainedReplayInjectedStageV1>,
}

impl RetainedReplayScriptV1 {
    fn observe(
        &mut self,
        stage: RetainedReplayInjectedStageV1,
    ) -> Result<(), RetainedReplayInjectedStageV1> {
        self.trace.push(stage);
        if self.fail == Some(stage) {
            if self.panic {
                std::panic::panic_any(stage);
            }
            return Err(stage);
        }
        Ok(())
    }
}

struct RetainedReplayScriptRequestV1(u64);
struct RetainedReplayScriptStorageV1(u64);
struct RetainedReplayScriptDataV1(u64);
struct RetainedReplayScriptAttachedV1(u64);

type RetainedReplayScriptOutcomeV1 = PersistentRetainedControlReplayPipelineOutcomeV1<
    RetainedReplayScriptRequestV1,
    RetainedReplayScriptStorageV1,
    RetainedReplayScriptDataV1,
    RetainedReplayScriptAttachedV1,
    RetainedReplayInjectedStageV1,
>;
type RetainedReplayScriptCustodyV1 = PersistentRetainedControlReplayPipelineCustodyV1<
    RetainedReplayScriptRequestV1,
    RetainedReplayScriptStorageV1,
    RetainedReplayScriptDataV1,
    RetainedReplayScriptAttachedV1,
>;

fn execute_retained_replay_script_v1(
    script: &mut RetainedReplayScriptV1,
    phases: &mut RetainedReplayScriptCustodyV1,
) -> Result<(), RetainedReplayInjectedStageV1> {
    use PersistentRetainedControlReplayPipelineCustodyV1 as Phase;
    execute_persistent_retained_control_replay_pipeline_v1(
        script,
        phases,
        |script, phases| {
            let Phase::Input(request) = phases else {
                panic!("input phase")
            };
            assert_eq!(request.0, 0x35);
            script.observe(RetainedReplayInjectedStageV1::MappedFacts)
        },
        |script, phases| {
            let Phase::Input(request) = phases else {
                panic!("input phase")
            };
            assert_eq!(request.0, 0x35);
            script.observe(RetainedReplayInjectedStageV1::Detach)?;
            let Phase::Input(request) = core::mem::replace(phases, Phase::Empty) else {
                unreachable!()
            };
            *phases = Phase::Storage(RetainedReplayScriptStorageV1(request.0));
            Ok(())
        },
        |script, phases| {
            let Phase::Storage(storage) = phases else {
                panic!("storage phase")
            };
            assert_eq!(storage.0, 0x35);
            script.observe(RetainedReplayInjectedStageV1::AuthenticatedConstruction)?;
            let Phase::Storage(storage) = core::mem::replace(phases, Phase::Empty) else {
                unreachable!()
            };
            *phases = Phase::Data(RetainedReplayScriptDataV1(storage.0));
            Ok(())
        },
        |script, phases| {
            let Phase::Data(data) = phases else {
                panic!("data phase")
            };
            assert_eq!(data.0, 0x35);
            script.observe(RetainedReplayInjectedStageV1::Retain)?;
            let Phase::Data(data) = core::mem::replace(phases, Phase::Empty) else {
                unreachable!()
            };
            *phases = Phase::Attached(RetainedReplayScriptAttachedV1(data.0));
            Ok(())
        },
        |script, phases| {
            let Phase::Attached(attached) = phases else {
                panic!("attached phase")
            };
            assert_eq!(attached.0, 0x35);
            script.observe(RetainedReplayInjectedStageV1::FinalAudit)
        },
    )
}

fn run_retained_replay_script_v1(
    fail: Option<RetainedReplayInjectedStageV1>,
) -> (
    RetainedReplayScriptOutcomeV1,
    Vec<RetainedReplayInjectedStageV1>,
) {
    let mut script = RetainedReplayScriptV1 {
        fail,
        panic: false,
        trace: Vec::new(),
    };
    let mut phases = RetainedReplayScriptCustodyV1::Input(RetainedReplayScriptRequestV1(0x35));
    let result = execute_retained_replay_script_v1(&mut script, &mut phases);
    (phases.into_outcome(result), script.trace)
}

fn retained_replay_prepared_owner_fixture_v1(
    queue: QueueKeyV1,
    id: u64,
) -> (
    Gfx942DirectionalQueuePersistentAllocationV1,
    Gfx942PersistentUseLeaseV1<Gfx942PersistentPreparedV1>,
) {
    let (mut session, prepared, _) =
        prepared_persistent_compute_cancellation_fixture(queue, id, None, Some(7));
    let input = session
        .cancel_prepared_directional_persistent_fixed_dispatch_v1(prepared)
        .expect("fixture cancellation restores attached replay input");
    let (mut allocation, initialization) = input.into_parts();
    let _ = initialization.authenticated_sha256();
    let _ = initialization.is_fully_initialized();
    let request = Gfx942PersistentUseRequestV1::new(
        Gfx942PersistentOperationV1::ComputeReadWrite,
        0,
        allocation.byte_len(),
    )
    .unwrap();
    let reserved = allocation.owner.reserve(request, None).unwrap();
    let prepared = allocation.owner.prepare(reserved).unwrap();
    (allocation, prepared)
}

fn persistent_bind_prepared_entries_for_test(
    count: usize,
) -> Vec<PersistentComputeAttachmentEntryV1> {
    (0..count)
        .map(|index| {
            let (allocation, prepared) = retained_replay_prepared_owner_fixture_v1(
                test_queue_key(293, 1),
                0x9200 + index as u64,
            );
            let storage_identity = Some(
                allocation
                    .owner
                    .local_native_for_sdma()
                    .unwrap()
                    .storage_identity(),
            );
            PersistentComputeAttachmentEntryV1 {
                allocation,
                initialization: PersistentComputeInitializationV1::AfterDispatch,
                state: PersistentComputeUseStateV1::Prepared(prepared),
                storage_identity,
                effect: Gfx942PersistentComputeEffectV1::ReadWrite,
            }
        })
        .collect()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CompletionRecycleScriptV1 {
    Pending,
    Ready,
    PublishedStateFailure,
    DispatchGenerationFailure,
    CompletionObservationFailure,
    DispatchCompletionFailure,
    AllocationCompletionFailure,
    SignalGenerationFailure,
    SignalResetFailure,
    ClosingCurrentnessFailure,
    RecycleCurrentnessFailure,
    RecycleInfrastructureFailure,
    DispatchRecycleFailure,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CompletionRecycleScriptCustodyV1 {
    Published(u64),
    Completed(u64),
    Recycled(u64),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CompletionRecycleScriptFailureV1 {
    point: CompletionRecycleScriptV1,
    custody: CompletionRecycleScriptCustodyV1,
}

struct CompletionRecycleScriptStateV1 {
    script: CompletionRecycleScriptV1,
    trace: Vec<&'static str>,
}

type CompletionRecycleScriptResultV1 = Result<
    PersistentComputePollAndRecycleTransitionV1<u64, u64, u64>,
    PersistentComputePollAndRecycleTransitionFailureV1<
        CompletionRecycleScriptFailureV1,
        CompletionRecycleScriptFailureV1,
    >,
>;

fn execute_completion_recycle_script_v1(
    script: CompletionRecycleScriptV1,
) -> (CompletionRecycleScriptResultV1, Vec<&'static str>) {
    const CUSTODY_ID: u64 = 73;
    let mut state = CompletionRecycleScriptStateV1 {
        script,
        trace: Vec::new(),
    };
    let result = execute_persistent_compute_poll_and_recycle_v1(
        &mut state,
        |state| match state.script {
            CompletionRecycleScriptV1::PublishedStateFailure => {
                state.trace.push("published-state-failure");
                Err(CompletionRecycleScriptFailureV1 {
                    point: state.script,
                    custody: CompletionRecycleScriptCustodyV1::Published(CUSTODY_ID),
                })
            }
            CompletionRecycleScriptV1::DispatchGenerationFailure => {
                state.trace.push("dispatch-generation-failure");
                Err(CompletionRecycleScriptFailureV1 {
                    point: state.script,
                    custody: CompletionRecycleScriptCustodyV1::Published(CUSTODY_ID),
                })
            }
            CompletionRecycleScriptV1::CompletionObservationFailure => {
                state
                    .trace
                    .extend(["check-a", "acquire", "observation-failure"]);
                Err(CompletionRecycleScriptFailureV1 {
                    point: state.script,
                    custody: CompletionRecycleScriptCustodyV1::Published(CUSTODY_ID),
                })
            }
            CompletionRecycleScriptV1::DispatchCompletionFailure => {
                state.trace.extend([
                    "check-a",
                    "acquire",
                    "check-b",
                    "dispatch-completion-failure",
                ]);
                Err(CompletionRecycleScriptFailureV1 {
                    point: state.script,
                    custody: CompletionRecycleScriptCustodyV1::Completed(CUSTODY_ID),
                })
            }
            CompletionRecycleScriptV1::AllocationCompletionFailure => {
                state.trace.extend([
                    "check-a",
                    "acquire",
                    "check-b",
                    "dispatch-completed",
                    "allocation-completion-failure",
                ]);
                Err(CompletionRecycleScriptFailureV1 {
                    point: state.script,
                    custody: CompletionRecycleScriptCustodyV1::Completed(CUSTODY_ID),
                })
            }
            CompletionRecycleScriptV1::Pending => {
                state.trace.extend(["check-a", "acquire", "check-b"]);
                Ok(PersistentComputePollTransitionV1::Pending(CUSTODY_ID))
            }
            _ => {
                state.trace.extend([
                    "check-a",
                    "acquire",
                    "check-b",
                    "dispatch-completed",
                    "allocation-completed",
                ]);
                Ok(PersistentComputePollTransitionV1::Ready(CUSTODY_ID))
            }
        },
        |state| {
            state.trace.push("midpoint");
            101
        },
        |state, completed| match state.script {
            CompletionRecycleScriptV1::Ready => {
                state.trace.extend([
                    "reset",
                    "check-c",
                    "dispatch-recycled",
                    "attachment-recycled",
                ]);
                Ok(completed)
            }
            CompletionRecycleScriptV1::SignalGenerationFailure => {
                state.trace.push("signal-generation-failure");
                Err(CompletionRecycleScriptFailureV1 {
                    point: state.script,
                    custody: CompletionRecycleScriptCustodyV1::Completed(completed),
                })
            }
            CompletionRecycleScriptV1::SignalResetFailure => {
                state.trace.push("reset-failure");
                Err(CompletionRecycleScriptFailureV1 {
                    point: state.script,
                    custody: CompletionRecycleScriptCustodyV1::Completed(completed),
                })
            }
            CompletionRecycleScriptV1::ClosingCurrentnessFailure => {
                state.trace.extend(["reset", "check-c-failure"]);
                Err(CompletionRecycleScriptFailureV1 {
                    point: state.script,
                    custody: CompletionRecycleScriptCustodyV1::Completed(completed),
                })
            }
            CompletionRecycleScriptV1::RecycleCurrentnessFailure => {
                state.trace.push("recycle-currentness-failure");
                Err(CompletionRecycleScriptFailureV1 {
                    point: state.script,
                    custody: CompletionRecycleScriptCustodyV1::Completed(completed),
                })
            }
            CompletionRecycleScriptV1::RecycleInfrastructureFailure => {
                state.trace.push("recycle-infrastructure-failure");
                Err(CompletionRecycleScriptFailureV1 {
                    point: state.script,
                    custody: CompletionRecycleScriptCustodyV1::Completed(completed),
                })
            }
            CompletionRecycleScriptV1::DispatchRecycleFailure => {
                state
                    .trace
                    .extend(["reset", "check-c", "dispatch-recycle-failure"]);
                Err(CompletionRecycleScriptFailureV1 {
                    point: state.script,
                    custody: CompletionRecycleScriptCustodyV1::Recycled(completed),
                })
            }
            _ => unreachable!("poll-stage scripts never reach recycle"),
        },
    );
    (result, state.trace)
}

struct CompletionWaitRecycleScriptStateV1 {
    scripts: std::collections::VecDeque<CompletionRecycleScriptV1>,
    trace: Vec<&'static str>,
    pending_boundaries: u64,
    timeout_after_pending_boundaries: u64,
}

struct CompletionWaitPendingV1(u64);

type CompletionWaitRecycleScriptResultV1 = Result<
    PersistentComputeWaitAndRecycleTransitionV1<CompletionWaitPendingV1, u64, u64>,
    PersistentComputePollAndRecycleTransitionFailureV1<
        CompletionRecycleScriptFailureV1,
        CompletionRecycleScriptFailureV1,
    >,
>;

fn execute_completion_wait_recycle_scripts_v1(
    pending: CompletionWaitPendingV1,
    scripts: impl IntoIterator<Item = CompletionRecycleScriptV1>,
    timeout_after_pending_boundaries: u64,
) -> (CompletionWaitRecycleScriptResultV1, Vec<&'static str>) {
    let mut state = CompletionWaitRecycleScriptStateV1 {
        scripts: scripts.into_iter().collect(),
        trace: Vec::new(),
        pending_boundaries: 0,
        timeout_after_pending_boundaries,
    };
    let result = execute_persistent_compute_wait_and_recycle_v1(
        &mut state,
        pending,
        |state, pending| {
            assert_eq!(pending.0, 73);
            let script = state
                .scripts
                .pop_front()
                .expect("wait script retains one outcome per observation");
            let (result, trace) = execute_completion_recycle_script_v1(script);
            state.trace.extend(trace);
            result.map(|transition| match transition {
                PersistentComputePollAndRecycleTransitionV1::Pending(pending) => {
                    PersistentComputePollAndRecycleTransitionV1::Pending(CompletionWaitPendingV1(
                        pending,
                    ))
                }
                PersistentComputePollAndRecycleTransitionV1::Recycled {
                    recycled,
                    completion_observed_at,
                } => PersistentComputePollAndRecycleTransitionV1::Recycled {
                    recycled,
                    completion_observed_at,
                },
            })
        },
        |state| {
            state.pending_boundaries += 1;
            state.pending_boundaries >= state.timeout_after_pending_boundaries
        },
    );
    (result, state.trace)
}

#[path = "tests/completion_tests.rs"]
mod completion_tests;
#[path = "tests/construction_tests.rs"]
mod construction_tests;
#[path = "tests/foundation_tests.rs"]
mod foundation_tests;
#[path = "tests/lane_tests.rs"]
mod lane_tests;
#[path = "tests/quarantine_tests.rs"]
mod quarantine_tests;
#[path = "tests/replay_tests.rs"]
mod replay_tests;
#[path = "tests/scope_tests.rs"]
mod scope_tests;
#[path = "tests/three_binding_tests.rs"]
mod three_binding_tests;

#[path = "tests/memory_fixture.rs"]
mod memory_fixture;
use memory_fixture::persistent_restore_fixture;
use memory_fixture::public_sdma_pool_buffer_for_test;
use memory_fixture::test_completion_template;
