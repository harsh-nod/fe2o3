use alloc::{vec, vec::Vec};

use super::*;

fn digest(seed: u8) -> IdentityDigestV1 {
    IdentityDigestV1::from_untrusted_bytes([seed; IDENTITY_DIGEST_BYTES_V1])
}

fn device(generation: u64) -> DeviceKeyV1 {
    DeviceKeyV1 {
        physical: PhysicalDeviceIdV1(7),
        generation: DeviceGenerationV1(generation),
    }
}

fn vm(device: DeviceKeyV1, id: u64) -> VmKeyV1 {
    VmKeyV1 {
        device,
        id: VmIdV1(id),
    }
}
fn allocation(vm: VmKeyV1, id: u64) -> AllocationKeyV1 {
    AllocationKeyV1 {
        vm,
        id: AllocationIdV1(id),
    }
}
fn mapping(allocation: AllocationKeyV1, id: u64) -> MappingKeyV1 {
    MappingKeyV1 {
        allocation,
        id: MappingIdV1(id),
    }
}
fn code(vm: VmKeyV1, id: u64) -> LoadedCodeKeyV1 {
    LoadedCodeKeyV1 {
        vm,
        id: LoadedCodeIdV1(id),
    }
}
fn queue(vm: VmKeyV1, id: u64, generation: u64) -> QueueKeyV1 {
    QueueKeyV1 {
        vm,
        id: QueueInstanceIdV1(id),
        generation: QueueGenerationV1(generation),
    }
}
fn dispatch(queue: QueueKeyV1, id: u64) -> DispatchKeyV1 {
    DispatchKeyV1 {
        queue,
        id: DispatchIdV1(id),
    }
}
fn completion(dispatch: DispatchKeyV1, id: u64) -> CompletionKeyV1 {
    CompletionKeyV1 {
        dispatch,
        id: CompletionIdV1(id),
    }
}

#[derive(Clone, Copy)]
struct Fixture {
    device: DeviceKeyV1,
    vm: VmKeyV1,
    code_allocation: AllocationKeyV1,
    code_mapping: MappingKeyV1,
    ring_allocation: AllocationKeyV1,
    ring_mapping: MappingKeyV1,
    data_allocation: AllocationKeyV1,
    data_mapping: MappingKeyV1,
    code: LoadedCodeKeyV1,
    queue: QueueKeyV1,
}

fn advance(state: RuntimeStateV1, transition: RuntimeTransitionV1) -> RuntimeStateV1 {
    let state = state.next(transition).unwrap();
    state.validate_global_invariants().unwrap();
    state
}

fn live_fixture(queue_capacity: u32) -> (RuntimeStateV1, Fixture) {
    let device = device(1);
    let vm = vm(device, 10);
    let code_allocation = allocation(vm, 20);
    let code_mapping = mapping(code_allocation, 21);
    let ring_allocation = allocation(vm, 30);
    let ring_mapping = mapping(ring_allocation, 31);
    let data_allocation = allocation(vm, 40);
    let data_mapping = mapping(data_allocation, 41);
    let code = code(vm, 50);
    let queue = queue(vm, 60, 1);
    let mut state = RuntimeStateV1::new();
    state = advance(state, RuntimeTransitionV1::AddDevice { key: device });
    state = advance(state, RuntimeTransitionV1::CreateVm { key: vm });
    for (key, byte_len) in [
        (code_allocation, 4096),
        (
            ring_allocation,
            u64::from(queue_capacity) * AQL_PACKET_BYTES_V1,
        ),
        (data_allocation, 4096),
    ] {
        state = advance(state, RuntimeTransitionV1::Allocate { key, byte_len });
    }
    state = advance(
        state,
        RuntimeTransitionV1::Map {
            key: code_mapping,
            allocation_offset: 0,
            gpu_va: 0x10000,
            byte_len: 4096,
            access: MemoryAccessV1::ReadExecute,
        },
    );
    state = advance(
        state,
        RuntimeTransitionV1::Map {
            key: ring_mapping,
            allocation_offset: 0,
            gpu_va: 0x20000,
            byte_len: u64::from(queue_capacity) * AQL_PACKET_BYTES_V1,
            access: MemoryAccessV1::ReadWrite,
        },
    );
    state = advance(
        state,
        RuntimeTransitionV1::Map {
            key: data_mapping,
            allocation_offset: 0,
            gpu_va: 0x30000,
            byte_len: 4096,
            access: MemoryAccessV1::ReadWrite,
        },
    );
    state = advance(
        state,
        RuntimeTransitionV1::LoadCode {
            key: code,
            load_plan_id: CodeLoadPlanIdV1::from_untrusted_digest(digest(1)),
            artifact_id: RuntimeArtifactIdV1::from_untrusted_digest(digest(2)),
            executable_mapping: code_mapping,
            entry_offset: 128,
        },
    );
    state = advance(
        state,
        RuntimeTransitionV1::CreateQueue {
            key: queue,
            plan_id: QueuePlanIdV1::from_untrusted_digest(digest(3)),
            ring_mapping,
            capacity: queue_capacity,
        },
    );
    (
        state,
        Fixture {
            device,
            vm,
            code_allocation,
            code_mapping,
            ring_allocation,
            ring_mapping,
            data_allocation,
            data_mapping,
            code,
            queue,
        },
    )
}

fn prepare(state: RuntimeStateV1, fixture: Fixture, id: u64) -> (RuntimeStateV1, CompletionKeyV1) {
    let dispatch = dispatch(fixture.queue, id);
    let completion = completion(dispatch, 1000 + id);
    let state = advance(
        state,
        RuntimeTransitionV1::PrepareDispatch {
            key: dispatch,
            code: fixture.code,
            completion,
            resources: vec![DispatchResourceV1 {
                mapping: fixture.data_mapping,
                required_access: MemoryAccessV1::ReadWrite,
            }],
        },
    );
    (state, completion)
}

#[test]
fn normal_trace_binds_exact_completion_and_releases_bottom_up() {
    let (state, fixture) = live_fixture(2);
    let (state, completion) = prepare(state, fixture, 70);
    let state = advance(state, RuntimeTransitionV1::PublishDispatch { completion });
    let state = advance(state, RuntimeTransitionV1::ObserveCompletion { completion });
    assert_eq!(state.dispatches()[0].state, DispatchStateV1::Completed);
    assert_eq!(state.completions()[0].state, CompletionStateV1::Observed);

    let mut state = advance(state, RuntimeTransitionV1::UnloadCode { key: fixture.code });
    state = advance(
        state,
        RuntimeTransitionV1::ReleaseQueue { key: fixture.queue },
    );
    for key in [
        fixture.code_mapping,
        fixture.ring_mapping,
        fixture.data_mapping,
    ] {
        state = advance(state, RuntimeTransitionV1::Unmap { key });
    }
    for key in [
        fixture.code_allocation,
        fixture.ring_allocation,
        fixture.data_allocation,
    ] {
        state = advance(state, RuntimeTransitionV1::ReleaseAllocation { key });
    }
    state = advance(state, RuntimeTransitionV1::ReleaseVm { key: fixture.vm });
    state = advance(
        state,
        RuntimeTransitionV1::ReleaseDevice {
            key: fixture.device,
        },
    );
    assert_eq!(state.devices()[0].state, DeviceStateV1::Released);
}

#[test]
fn published_and_ambiguous_dispatches_fail_closed() {
    let (state, fixture) = live_fixture(1);
    let (state, completion) = prepare(state, fixture, 70);
    let state = advance(state, RuntimeTransitionV1::PublishDispatch { completion });
    for transition in [
        RuntimeTransitionV1::Unmap {
            key: fixture.data_mapping,
        },
        RuntimeTransitionV1::UnloadCode { key: fixture.code },
        RuntimeTransitionV1::ReleaseQueue { key: fixture.queue },
    ] {
        assert!(matches!(
            state.next(transition),
            Err(TransitionErrorV1::ResourceInUse(_))
        ));
    }

    let state = advance(
        state,
        RuntimeTransitionV1::MarkDispatchAmbiguous { completion },
    );
    assert!(matches!(
        state.next(RuntimeTransitionV1::ObserveCompletion { completion }),
        Err(TransitionErrorV1::IllegalState(_))
    ));
    assert!(matches!(
        state.next(RuntimeTransitionV1::SettleAfterQuiescence { completion }),
        Err(TransitionErrorV1::NotQuiescent(_))
    ));
    let state = advance(
        state,
        RuntimeTransitionV1::BeginQueueFailure { key: fixture.queue },
    );
    assert!(matches!(
        state.next(RuntimeTransitionV1::ReleaseQueue { key: fixture.queue }),
        Err(TransitionErrorV1::NotQuiescent(_))
    ));
    let state = advance(
        state,
        RuntimeTransitionV1::EstablishQueueQuiescence { key: fixture.queue },
    );
    let state = advance(
        state,
        RuntimeTransitionV1::SettleAfterQuiescence { completion },
    );
    assert_eq!(
        state.dispatches()[0].state,
        DispatchStateV1::FailedQuiescent
    );
    advance(
        state,
        RuntimeTransitionV1::ReleaseQueue { key: fixture.queue },
    );
}

#[test]
fn completion_identity_is_not_interchangeable() {
    let (state, fixture) = live_fixture(2);
    let (state, exact) = prepare(state, fixture, 70);
    let wrong = CompletionKeyV1 {
        dispatch: exact.dispatch,
        id: CompletionIdV1(exact.id.0 + 1),
    };
    assert_eq!(
        state.next(RuntimeTransitionV1::PublishDispatch { completion: wrong }),
        Err(TransitionErrorV1::CompletionMismatch(exact.dispatch))
    );

    let other_dispatch = dispatch(fixture.queue, 71);
    let substituted = CompletionKeyV1 {
        dispatch: other_dispatch,
        id: exact.id,
    };
    assert!(matches!(
        state.next(RuntimeTransitionV1::ObserveCompletion {
            completion: substituted
        }),
        Err(TransitionErrorV1::NotFound(_))
    ));
    let state = advance(
        state,
        RuntimeTransitionV1::AbortPrepared { completion: exact },
    );
    assert_eq!(
        state.completions()[0].state,
        CompletionStateV1::CancelledBeforePublication
    );
}

#[test]
fn cross_device_and_stale_generation_substitution_are_rejected() {
    let (state, fixture) = live_fixture(2);
    let other_device = DeviceKeyV1 {
        physical: PhysicalDeviceIdV1(8),
        generation: DeviceGenerationV1(1),
    };
    let other_vm = vm(other_device, 10);
    let state = advance(state, RuntimeTransitionV1::AddDevice { key: other_device });
    let mut state = advance(state, RuntimeTransitionV1::CreateVm { key: other_vm });
    let foreign_ring_allocation = allocation(other_vm, 90);
    let foreign_ring_mapping = mapping(foreign_ring_allocation, 91);
    let foreign_queue = queue(other_vm, 92, 1);
    state = advance(
        state,
        RuntimeTransitionV1::Allocate {
            key: foreign_ring_allocation,
            byte_len: 2 * AQL_PACKET_BYTES_V1,
        },
    );
    state = advance(
        state,
        RuntimeTransitionV1::Map {
            key: foreign_ring_mapping,
            allocation_offset: 0,
            gpu_va: 0x40000,
            byte_len: 2 * AQL_PACKET_BYTES_V1,
            access: MemoryAccessV1::ReadWrite,
        },
    );
    state = advance(
        state,
        RuntimeTransitionV1::CreateQueue {
            key: foreign_queue,
            plan_id: QueuePlanIdV1::from_untrusted_digest(digest(99)),
            ring_mapping: foreign_ring_mapping,
            capacity: 2,
        },
    );
    let foreign_dispatch = dispatch(foreign_queue, 70);
    let foreign_completion = completion(foreign_dispatch, 100);
    assert_eq!(
        state.next(RuntimeTransitionV1::PrepareDispatch {
            key: foreign_dispatch,
            code: fixture.code,
            completion: foreign_completion,
            resources: Vec::new()
        }),
        Err(TransitionErrorV1::BindingMismatch(RecordRefV1::Dispatch(
            foreign_dispatch,
        )))
    );

    assert!(matches!(
        state.next(RuntimeTransitionV1::AddDevice { key: device(2) }),
        Err(TransitionErrorV1::ResourceInUse(_))
    ));
    assert_eq!(
        state.next(RuntimeTransitionV1::CreateVm {
            key: vm(device(2), 99)
        }),
        Err(TransitionErrorV1::NotFound(RecordRefV1::Device(device(2))))
    );
}

#[test]
fn device_set_is_bounded_and_generations_advance_only_after_release() {
    let first = device(1);
    let mut state = advance(
        RuntimeStateV1::new(),
        RuntimeTransitionV1::AddDevice { key: first },
    );
    assert!(matches!(
        state.next(RuntimeTransitionV1::AddDevice { key: device(2) }),
        Err(TransitionErrorV1::ResourceInUse(_))
    ));
    state = advance(state, RuntimeTransitionV1::ReleaseDevice { key: first });
    let second = device(2);
    state = advance(state, RuntimeTransitionV1::AddDevice { key: second });
    state = advance(state, RuntimeTransitionV1::ReleaseDevice { key: second });
    assert_eq!(
        state.next(RuntimeTransitionV1::AddDevice { key: device(0) }),
        Err(TransitionErrorV1::GenerationNotMonotonic(device(0)))
    );

    let mut full = RuntimeStateV1::new();
    for physical in 0..MAX_DEVICES_V1 as u64 {
        let key = DeviceKeyV1 {
            physical: PhysicalDeviceIdV1(physical),
            generation: DeviceGenerationV1(1),
        };
        full = advance(full, RuntimeTransitionV1::AddDevice { key });
    }
    let overflow = DeviceKeyV1 {
        physical: PhysicalDeviceIdV1(MAX_DEVICES_V1 as u64),
        generation: DeviceGenerationV1(1),
    };
    assert_eq!(
        full.next(RuntimeTransitionV1::AddDevice { key: overflow }),
        Err(TransitionErrorV1::CapacityExceeded {
            kind: RecordKindV1::Device,
            maximum: MAX_DEVICES_V1,
        })
    );
}

#[test]
fn mapping_ranges_permissions_and_canonical_resources_are_checked() {
    let (state, fixture) = live_fixture(2);
    let overlap_allocation = allocation(fixture.vm, 90);
    let overlap_mapping = mapping(overlap_allocation, 91);
    let state = advance(
        state,
        RuntimeTransitionV1::Allocate {
            key: overlap_allocation,
            byte_len: 128,
        },
    );
    assert_eq!(
        state.next(RuntimeTransitionV1::Map {
            key: overlap_mapping,
            allocation_offset: 0,
            gpu_va: 0x30010,
            byte_len: 64,
            access: MemoryAccessV1::Read
        }),
        Err(TransitionErrorV1::AddressConflict(overlap_mapping))
    );
    assert!(matches!(
        state.next(RuntimeTransitionV1::Map {
            key: overlap_mapping,
            allocation_offset: 100,
            gpu_va: u64::MAX - 10,
            byte_len: 64,
            access: MemoryAccessV1::Read
        }),
        Err(TransitionErrorV1::InvalidRange(_))
    ));

    let dispatch = dispatch(fixture.queue, 70);
    let completion = completion(dispatch, 170);
    let duplicate_resources = vec![
        DispatchResourceV1 {
            mapping: fixture.data_mapping,
            required_access: MemoryAccessV1::Read,
        },
        DispatchResourceV1 {
            mapping: fixture.data_mapping,
            required_access: MemoryAccessV1::ReadWrite,
        },
    ];
    assert_eq!(
        state.next(RuntimeTransitionV1::PrepareDispatch {
            key: dispatch,
            code: fixture.code,
            completion,
            resources: duplicate_resources
        }),
        Err(TransitionErrorV1::NonCanonicalResources(dispatch))
    );
}

#[test]
fn queue_capacity_counts_prepared_published_and_ambiguous_work() {
    let (state, fixture) = live_fixture(1);
    let (state, first) = prepare(state, fixture, 70);
    let second_dispatch = dispatch(fixture.queue, 71);
    let second_completion = completion(second_dispatch, 171);
    let second = RuntimeTransitionV1::PrepareDispatch {
        key: second_dispatch,
        code: fixture.code,
        completion: second_completion,
        resources: Vec::new(),
    };
    assert_eq!(
        state.next(second.clone()),
        Err(TransitionErrorV1::QueueFull(fixture.queue))
    );
    let state = advance(
        state,
        RuntimeTransitionV1::PublishDispatch { completion: first },
    );
    assert_eq!(
        state.next(second.clone()),
        Err(TransitionErrorV1::QueueFull(fixture.queue))
    );
    let state = advance(
        state,
        RuntimeTransitionV1::MarkDispatchAmbiguous { completion: first },
    );
    assert_eq!(
        state.next(second.clone()),
        Err(TransitionErrorV1::QueueFull(fixture.queue))
    );
    let state = advance(
        state,
        RuntimeTransitionV1::BeginQueueFailure { key: fixture.queue },
    );
    let state = advance(
        state,
        RuntimeTransitionV1::EstablishQueueQuiescence { key: fixture.queue },
    );
    let state = advance(
        state,
        RuntimeTransitionV1::SettleAfterQuiescence { completion: first },
    );
    assert!(matches!(
        state.next(second),
        Err(TransitionErrorV1::IllegalState(_))
    ));
}

#[test]
fn rejected_transitions_are_failure_atomic_and_invariants_detect_corruption() {
    let (state, fixture) = live_fixture(2);
    let before = state.clone();
    assert!(
        state
            .next(RuntimeTransitionV1::ReleaseAllocation {
                key: fixture.data_allocation
            })
            .is_err()
    );
    assert_eq!(state, before);

    let (mut state, completion) = prepare(state, fixture, 70);
    state
        .mappings
        .iter_mut()
        .find(|record| record.key == fixture.data_mapping)
        .unwrap()
        .state = ResourceStateV1::Released;
    assert_eq!(
        state.validate_global_invariants(),
        Err(InvariantViolationV1::EarlyRelease(RecordRefV1::Mapping(
            fixture.data_mapping
        )))
    );
    assert!(matches!(
        state.next(RuntimeTransitionV1::PublishDispatch { completion }),
        Err(TransitionErrorV1::SourceInvariant(_))
    ));
}

#[test]
fn transition_batches_are_atomic_when_a_late_transition_fails() {
    let first = device(1);
    let state = RuntimeStateV1::new();
    assert_eq!(
        state.next_all([
            RuntimeTransitionV1::AddDevice { key: first },
            RuntimeTransitionV1::AddDevice { key: first },
        ]),
        Err(TransitionErrorV1::AlreadyExists(RecordRefV1::Device(first)))
    );
    assert!(state.devices().is_empty());

    let advanced = state
        .next_all([
            RuntimeTransitionV1::AddDevice { key: first },
            RuntimeTransitionV1::CreateVm { key: vm(first, 10) },
        ])
        .unwrap();
    assert_eq!(advanced.devices().len(), 1);
    assert_eq!(advanced.vms().len(), 1);
    advanced.validate_global_invariants().unwrap();
}

#[test]
fn identity_types_are_domain_distinct_and_explicitly_untrusted() {
    let bytes = digest(9);
    let model = RuntimeModelIdV1::from_untrusted_digest(bytes);
    let artifact = RuntimeArtifactIdV1::from_untrusted_digest(bytes);
    assert_eq!(model.digest().as_bytes(), artifact.digest().as_bytes());
    assert_eq!(RUNTIME_IDENTITY_SCHEMA_VERSION_V1, 1);
    assert_eq!(RUNTIME_STATE_SCHEMA_VERSION_V1, 1);
}
