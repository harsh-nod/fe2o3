use alloc::{vec, vec::Vec};

use super::*;

#[derive(Clone, Copy)]
struct Fixture {
    queue: QueueKeyV1,
    ring: MappingKeyV1,
    executable: MappingKeyV1,
    code: LoadedCodeKeyV1,
    kernargs: [MappingKeyV1; 3],
    signals: [MappingKeyV1; 3],
    data: [MappingKeyV1; 3],
}

fn digest(seed: u8) -> IdentityDigestV1 {
    IdentityDigestV1::from_untrusted_bytes([seed; IDENTITY_DIGEST_BYTES_V1])
}

fn advance(state: RuntimeStateV1, transition: RuntimeTransitionV1) -> RuntimeStateV1 {
    state.next(transition).unwrap()
}

fn live_fixture() -> (RuntimeStateV1, Fixture) {
    let device = DeviceKeyV1 {
        physical: PhysicalDeviceIdV1(1),
        generation: DeviceGenerationV1(1),
    };
    let vm = VmKeyV1 {
        device,
        id: VmIdV1(1),
    };
    let mut state = advance(
        RuntimeStateV1::new(),
        RuntimeTransitionV1::AddDevice { key: device },
    );
    state = advance(state, RuntimeTransitionV1::CreateVm { key: vm });

    let mut mappings = Vec::new();
    for index in 0_u64..11 {
        let allocation = AllocationKeyV1 {
            vm,
            id: AllocationIdV1(index + 1),
        };
        let mapping = MappingKeyV1 {
            allocation,
            id: MappingIdV1(index + 1),
        };
        state = advance(
            state,
            RuntimeTransitionV1::Allocate {
                key: allocation,
                byte_len: 4_096,
            },
        );
        state = advance(
            state,
            RuntimeTransitionV1::Map {
                key: mapping,
                allocation_offset: 0,
                gpu_va: 0x10_0000 + index * 0x1_0000,
                byte_len: 4_096,
                access: if index == 0 {
                    MemoryAccessV1::ReadExecute
                } else {
                    MemoryAccessV1::ReadWrite
                },
            },
        );
        mappings.push(mapping);
    }

    let code = LoadedCodeKeyV1 {
        vm,
        id: LoadedCodeIdV1(1),
    };
    state = advance(
        state,
        RuntimeTransitionV1::LoadCode {
            key: code,
            load_plan_id: CodeLoadPlanIdV1::from_untrusted_digest(digest(1)),
            artifact_id: RuntimeArtifactIdV1::from_untrusted_digest(digest(2)),
            executable_mapping: mappings[0],
            entry_offset: 64,
        },
    );
    let queue = QueueKeyV1 {
        vm,
        id: QueueInstanceIdV1(1),
        generation: QueueGenerationV1(1),
    };
    state = advance(
        state,
        RuntimeTransitionV1::CreateQueue {
            key: queue,
            plan_id: QueuePlanIdV1::from_untrusted_digest(digest(3)),
            ring_mapping: mappings[1],
            capacity: 4,
        },
    );
    (
        state,
        Fixture {
            queue,
            ring: mappings[1],
            executable: mappings[0],
            code,
            kernargs: [mappings[2], mappings[5], mappings[8]],
            signals: [mappings[3], mappings[6], mappings[9]],
            data: [mappings[4], mappings[7], mappings[10]],
        },
    )
}

fn resources(
    fixture: Fixture,
    index: usize,
    data_mapping: MappingKeyV1,
    access: MemoryAccessV1,
) -> AsyncOperationResourcesV1 {
    AsyncOperationResourcesV1::new(
        fixture.code,
        fixture.kernargs[index],
        fixture.signals[index],
        vec![DispatchResourceV1 {
            mapping: data_mapping,
            required_access: access,
        }],
    )
    .unwrap()
}

fn runtime_resources(resources: &AsyncOperationResourcesV1) -> Vec<DispatchResourceV1> {
    let mut result = vec![
        DispatchResourceV1 {
            mapping: resources.kernarg(),
            required_access: MemoryAccessV1::ReadWrite,
        },
        DispatchResourceV1 {
            mapping: resources.completion_signal(),
            required_access: MemoryAccessV1::ReadWrite,
        },
    ];
    result.extend_from_slice(resources.data());
    result.sort_unstable_by_key(|resource| resource.mapping);
    result
}

fn identity(queue: QueueKeyV1, id: u64) -> (DispatchKeyV1, CompletionKeyV1) {
    let dispatch = DispatchKeyV1 {
        queue,
        id: DispatchIdV1(id),
    };
    (
        dispatch,
        CompletionKeyV1 {
            dispatch,
            id: CompletionIdV1(id + 1_000),
        },
    )
}

fn registry(
    state: RuntimeStateV1,
    fixture: Fixture,
    max_in_flight: usize,
    incarnation: u8,
) -> AsyncQueueRegistryV1 {
    AsyncQueueRegistryV1::new_model_only(state, digest(incarnation), fixture.queue, max_in_flight)
        .unwrap()
}

fn reserve(
    registry: &mut AsyncQueueRegistryV1,
    fixture: Fixture,
    index: usize,
    id: u64,
) -> AsyncReservedOperationTokenV1 {
    let (dispatch, completion) = identity(fixture.queue, id);
    registry
        .reserve_model_only(
            dispatch,
            completion,
            resources(
                fixture,
                index,
                fixture.data[index],
                MemoryAccessV1::ReadWrite,
            ),
        )
        .unwrap()
}

#[test]
fn submitted_operations_complete_out_of_order_and_slots_reuse_by_generation() {
    let (state, fixture) = live_fixture();
    let mut registry = registry(state, fixture, 2, 0x80);
    let first = reserve(&mut registry, fixture, 0, 10)
        .publish_model_only(&mut registry)
        .unwrap();
    let second = reserve(&mut registry, fixture, 1, 11)
        .publish_model_only(&mut registry)
        .unwrap();
    assert_eq!(registry.retained_operation_count(), 2);

    let second = match second
        .poll_model_only(&mut registry, AsyncCompletionObservationV1::Completed)
        .unwrap()
    {
        AsyncOperationPollV1::Completed(completed) => completed,
        other => panic!("unexpected poll result: {other:?}"),
    };
    assert_eq!(registry.available_slot_count(), 0);
    let released = second.recycle_model_only(&mut registry).unwrap();
    assert_eq!(
        released.outcome(),
        AsyncReleasedOperationOutcomeV1::RecycledAfterCompletion
    );

    let third = reserve(&mut registry, fixture, 2, 12);
    assert_eq!(
        third.binding().slot_index(),
        released.binding().slot_index()
    );
    assert_eq!(
        third.binding().slot_generation(),
        released.binding().slot_generation() + 1
    );
    let _third = third
        .cancel_before_publication_model_only(&mut registry)
        .unwrap();
    let first = match first
        .poll_model_only(&mut registry, AsyncCompletionObservationV1::Completed)
        .unwrap()
    {
        AsyncOperationPollV1::Completed(completed) => completed,
        other => panic!("unexpected poll result: {other:?}"),
    };
    first.recycle_model_only(&mut registry).unwrap();
    assert_eq!(registry.retained_operation_count(), 0);
    registry.validate_global_invariants().unwrap();
    registry.into_runtime_state().unwrap();
}

#[test]
fn timeout_and_post_publication_cancellation_retain_submitted_custody() {
    let (state, fixture) = live_fixture();
    let mut registry = registry(state, fixture, 1, 0x81);
    let submitted = reserve(&mut registry, fixture, 0, 20)
        .publish_model_only(&mut registry)
        .unwrap();
    let timed_out = submitted.observe_timeout_model_only(&mut registry).unwrap();
    assert_eq!(timed_out.observation_count(), 1);
    assert_eq!(registry.retained_operation_count(), 1);
    let (dispatch, completion) = identity(fixture.queue, 21);
    assert_eq!(
        registry
            .reserve_model_only(
                dispatch,
                completion,
                resources(fixture, 1, fixture.data[1], MemoryAccessV1::ReadWrite,),
            )
            .unwrap_err(),
        AsyncQueueErrorV1::QueueFull
    );

    let cancellation = timed_out
        .into_submitted()
        .request_cancellation_model_only(&mut registry)
        .unwrap();
    assert!(cancellation.is_first_request());
    assert!(registry.slots()[0].cancellation_requested());
    assert!(registry.into_runtime_state().is_err());
}

#[test]
fn indeterminate_observation_quarantines_without_release_or_reuse() {
    let (state, fixture) = live_fixture();
    let mut registry = registry(state, fixture, 1, 0x82);
    let submitted = reserve(&mut registry, fixture, 0, 30)
        .publish_model_only(&mut registry)
        .unwrap();
    let quarantined = match submitted
        .poll_model_only(
            &mut registry,
            AsyncCompletionObservationV1::Indeterminate(
                AsyncIndeterminateReasonV1::DeviceCurrentnessLost,
            ),
        )
        .unwrap()
    {
        AsyncOperationPollV1::Indeterminate(quarantined) => quarantined,
        other => panic!("unexpected poll result: {other:?}"),
    };
    assert_eq!(
        quarantined.reason(),
        AsyncIndeterminateReasonV1::DeviceCurrentnessLost
    );
    assert_eq!(
        registry.slots()[0].phase(),
        AsyncQueueSlotPhaseV1::Indeterminate
    );
    assert_eq!(registry.available_slot_count(), 0);
    assert!(registry.into_runtime_state().is_err());
}

#[test]
fn resource_conflicts_reject_but_shared_reads_are_compatible() {
    let (state, fixture) = live_fixture();
    let mut registry = registry(state, fixture, 3, 0x83);
    let (first_dispatch, first_completion) = identity(fixture.queue, 40);
    let _first = registry
        .reserve_model_only(
            first_dispatch,
            first_completion,
            resources(fixture, 0, fixture.data[0], MemoryAccessV1::Read),
        )
        .unwrap();
    let (second_dispatch, second_completion) = identity(fixture.queue, 41);
    let _second = registry
        .reserve_model_only(
            second_dispatch,
            second_completion,
            resources(fixture, 1, fixture.data[0], MemoryAccessV1::Read),
        )
        .unwrap();
    let (third_dispatch, third_completion) = identity(fixture.queue, 42);
    assert_eq!(
        registry
            .reserve_model_only(
                third_dispatch,
                third_completion,
                resources(fixture, 2, fixture.data[0], MemoryAccessV1::ReadWrite),
            )
            .unwrap_err(),
        AsyncQueueErrorV1::ResourceConflict
    );
    assert_eq!(registry.retained_operation_count(), 2);
    registry.validate_global_invariants().unwrap();
}

#[test]
fn operation_resources_reject_executable_data_and_role_collisions() {
    let (_, fixture) = live_fixture();
    assert_eq!(
        AsyncOperationResourcesV1::new(
            fixture.code,
            fixture.kernargs[0],
            fixture.signals[0],
            vec![DispatchResourceV1 {
                mapping: fixture.data[0],
                required_access: MemoryAccessV1::ReadExecute,
            }],
        )
        .unwrap_err(),
        AsyncQueueErrorV1::InvalidDataAccess
    );
    assert_eq!(
        AsyncOperationResourcesV1::new(
            fixture.code,
            fixture.kernargs[0],
            fixture.kernargs[0],
            Vec::new(),
        )
        .unwrap_err(),
        AsyncQueueErrorV1::ResourceRoleCollision
    );
}

#[test]
fn distinct_declared_registry_incarnations_reject_token_replay() {
    let (state, fixture) = live_fixture();
    let mut first = registry(state.clone(), fixture, 1, 0x84);
    let mut second = registry(state, fixture, 1, 0x85);
    let token = reserve(&mut first, fixture, 0, 50);
    let second_token = reserve(&mut second, fixture, 0, 50);
    let failure = token.publish_model_only(&mut second).unwrap_err();
    assert_eq!(failure.error(), &AsyncQueueErrorV1::TokenMismatch);
    let token = failure.into_retained();
    token
        .cancel_before_publication_model_only(&mut first)
        .unwrap();
    second_token
        .cancel_before_publication_model_only(&mut second)
        .unwrap();
}

#[test]
fn queue_ring_is_rejected_in_every_operation_role() {
    let (state, fixture) = live_fixture();
    let mut registry = registry(state, fixture, 1, 0x86);
    let cases = [
        AsyncOperationResourcesV1::new(
            fixture.code,
            fixture.ring,
            fixture.signals[0],
            vec![DispatchResourceV1 {
                mapping: fixture.data[0],
                required_access: MemoryAccessV1::Read,
            }],
        )
        .unwrap(),
        AsyncOperationResourcesV1::new(
            fixture.code,
            fixture.kernargs[0],
            fixture.ring,
            vec![DispatchResourceV1 {
                mapping: fixture.data[0],
                required_access: MemoryAccessV1::Read,
            }],
        )
        .unwrap(),
        AsyncOperationResourcesV1::new(
            fixture.code,
            fixture.kernargs[0],
            fixture.signals[0],
            vec![DispatchResourceV1 {
                mapping: fixture.ring,
                required_access: MemoryAccessV1::ReadWrite,
            }],
        )
        .unwrap(),
    ];
    for (offset, resources) in cases.into_iter().enumerate() {
        let (dispatch, completion) = identity(fixture.queue, 60 + offset as u64);
        assert_eq!(
            registry
                .reserve_model_only(dispatch, completion, resources)
                .unwrap_err(),
            AsyncQueueErrorV1::QueueInfrastructureCollision
        );
    }
}

#[test]
fn retained_dispatch_on_another_queue_blocks_conflicting_resources() {
    let (mut state, fixture) = live_fixture();
    let ring_allocation = AllocationKeyV1 {
        vm: fixture.queue.vm,
        id: AllocationIdV1(100),
    };
    let ring_mapping = MappingKeyV1 {
        allocation: ring_allocation,
        id: MappingIdV1(100),
    };
    state = advance(
        state,
        RuntimeTransitionV1::Allocate {
            key: ring_allocation,
            byte_len: 4_096,
        },
    );
    state = advance(
        state,
        RuntimeTransitionV1::Map {
            key: ring_mapping,
            allocation_offset: 0,
            gpu_va: 0x80_0000,
            byte_len: 4_096,
            access: MemoryAccessV1::ReadWrite,
        },
    );
    let other_queue = QueueKeyV1 {
        vm: fixture.queue.vm,
        id: QueueInstanceIdV1(2),
        generation: QueueGenerationV1(1),
    };
    state = advance(
        state,
        RuntimeTransitionV1::CreateQueue {
            key: other_queue,
            plan_id: QueuePlanIdV1::from_untrusted_digest(digest(0x40)),
            ring_mapping,
            capacity: 4,
        },
    );
    let (dispatch, completion) = identity(other_queue, 70);
    state = advance(
        state,
        RuntimeTransitionV1::PrepareDispatch {
            key: dispatch,
            code: fixture.code,
            completion,
            resources: runtime_resources(&resources(
                fixture,
                0,
                fixture.data[0],
                MemoryAccessV1::ReadWrite,
            )),
        },
    );
    state = advance(state, RuntimeTransitionV1::PublishDispatch { completion });

    let mut registry = registry(state, fixture, 1, 0x87);
    let (candidate, candidate_completion) = identity(fixture.queue, 71);
    assert_eq!(
        registry
            .reserve_model_only(
                candidate,
                candidate_completion,
                resources(fixture, 1, fixture.data[0], MemoryAccessV1::ReadWrite,),
            )
            .unwrap_err(),
        AsyncQueueErrorV1::ResourceConflict
    );
}

#[test]
fn retained_cross_queue_writer_cannot_alias_candidate_executable() {
    let (mut state, fixture) = live_fixture();
    let executable_alias = MappingKeyV1 {
        allocation: fixture.executable.allocation,
        id: MappingIdV1(97),
    };
    state = advance(
        state,
        RuntimeTransitionV1::Map {
            key: executable_alias,
            allocation_offset: 0,
            gpu_va: 0xb0_0000,
            byte_len: 4_096,
            access: MemoryAccessV1::ReadWrite,
        },
    );
    let ring_allocation = AllocationKeyV1 {
        vm: fixture.queue.vm,
        id: AllocationIdV1(101),
    };
    let ring_mapping = MappingKeyV1 {
        allocation: ring_allocation,
        id: MappingIdV1(101),
    };
    state = advance(
        state,
        RuntimeTransitionV1::Allocate {
            key: ring_allocation,
            byte_len: 4_096,
        },
    );
    state = advance(
        state,
        RuntimeTransitionV1::Map {
            key: ring_mapping,
            allocation_offset: 0,
            gpu_va: 0xc0_0000,
            byte_len: 4_096,
            access: MemoryAccessV1::ReadWrite,
        },
    );
    let other_queue = QueueKeyV1 {
        vm: fixture.queue.vm,
        id: QueueInstanceIdV1(3),
        generation: QueueGenerationV1(1),
    };
    state = advance(
        state,
        RuntimeTransitionV1::CreateQueue {
            key: other_queue,
            plan_id: QueuePlanIdV1::from_untrusted_digest(digest(0x41)),
            ring_mapping,
            capacity: 4,
        },
    );
    let (dispatch, completion) = identity(other_queue, 75);
    let external = AsyncOperationResourcesV1::new(
        fixture.code,
        fixture.kernargs[0],
        fixture.signals[0],
        vec![DispatchResourceV1 {
            mapping: executable_alias,
            required_access: MemoryAccessV1::ReadWrite,
        }],
    )
    .unwrap();
    state = advance(
        state,
        RuntimeTransitionV1::PrepareDispatch {
            key: dispatch,
            code: fixture.code,
            completion,
            resources: runtime_resources(&external),
        },
    );
    state = advance(state, RuntimeTransitionV1::PublishDispatch { completion });

    let mut registry = registry(state, fixture, 1, 0x8a);
    let (candidate, candidate_completion) = identity(fixture.queue, 76);
    assert_eq!(
        registry
            .reserve_model_only(
                candidate,
                candidate_completion,
                resources(fixture, 1, fixture.data[1], MemoryAccessV1::Read),
            )
            .unwrap_err(),
        AsyncQueueErrorV1::ResourceConflict
    );
}

#[test]
fn overlapping_mappings_cannot_bypass_roles_or_write_conflicts() {
    let (mut state, fixture) = live_fixture();
    let alias = MappingKeyV1 {
        allocation: fixture.data[0].allocation,
        id: MappingIdV1(99),
    };
    state = advance(
        state,
        RuntimeTransitionV1::Map {
            key: alias,
            allocation_offset: 0,
            gpu_va: 0x90_0000,
            byte_len: 4_096,
            access: MemoryAccessV1::ReadWrite,
        },
    );
    let mut registry = registry(state, fixture, 2, 0x88);
    let (role_dispatch, role_completion) = identity(fixture.queue, 80);
    let aliased_roles = AsyncOperationResourcesV1::new(
        fixture.code,
        fixture.data[0],
        fixture.signals[0],
        vec![DispatchResourceV1 {
            mapping: alias,
            required_access: MemoryAccessV1::Read,
        }],
    )
    .unwrap();
    assert_eq!(
        registry
            .reserve_model_only(role_dispatch, role_completion, aliased_roles)
            .unwrap_err(),
        AsyncQueueErrorV1::ResourceRoleCollision
    );

    let (first_dispatch, first_completion) = identity(fixture.queue, 81);
    let _first = registry
        .reserve_model_only(
            first_dispatch,
            first_completion,
            resources(fixture, 0, fixture.data[0], MemoryAccessV1::ReadWrite),
        )
        .unwrap();
    let (second_dispatch, second_completion) = identity(fixture.queue, 82);
    assert_eq!(
        registry
            .reserve_model_only(
                second_dispatch,
                second_completion,
                resources(fixture, 1, alias, MemoryAccessV1::Read),
            )
            .unwrap_err(),
        AsyncQueueErrorV1::ResourceConflict
    );
}

#[test]
fn writable_alias_of_live_executable_storage_is_rejected() {
    let (mut state, fixture) = live_fixture();
    let alias = MappingKeyV1 {
        allocation: fixture.executable.allocation,
        id: MappingIdV1(98),
    };
    state = advance(
        state,
        RuntimeTransitionV1::Map {
            key: alias,
            allocation_offset: 0,
            gpu_va: 0xa0_0000,
            byte_len: 4_096,
            access: MemoryAccessV1::ReadWrite,
        },
    );
    let mut registry = registry(state, fixture, 1, 0x89);
    let (dispatch, completion) = identity(fixture.queue, 90);
    assert_eq!(
        registry
            .reserve_model_only(
                dispatch,
                completion,
                resources(fixture, 0, alias, MemoryAccessV1::ReadWrite),
            )
            .unwrap_err(),
        AsyncQueueErrorV1::ExecutableStorageCollision
    );
}
