use alloc::{vec, vec::Vec};

use super::*;

#[derive(Clone, Copy)]
struct Fixture {
    queue: QueueKeyV1,
    code: LoadedCodeKeyV1,
    kernargs: [MappingKeyV1; 3],
    signals: [MappingKeyV1; 3],
    data: [MappingKeyV1; 3],
}

#[derive(Debug)]
struct Alpha;

#[derive(Debug)]
struct Beta;

fn digest(seed: u8) -> IdentityDigestV1 {
    IdentityDigestV1::from_untrusted_bytes([seed; IDENTITY_DIGEST_BYTES_V1])
}

fn advance(state: RuntimeStateV1, transition: RuntimeTransitionV1) -> RuntimeStateV1 {
    state.next(transition).unwrap()
}

fn live_fixture(physical: u64) -> (RuntimeStateV1, Fixture) {
    let device = DeviceKeyV1 {
        physical: PhysicalDeviceIdV1(physical),
        generation: DeviceGenerationV1(1),
    };
    let vm = VmKeyV1 {
        device,
        id: VmIdV1(physical),
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
                gpu_va: physical * 0x1000_0000 + 0x10_0000 + index * 0x1_0000,
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
            code,
            kernargs: [mappings[2], mappings[5], mappings[8]],
            signals: [mappings[3], mappings[6], mappings[9]],
            data: [mappings[4], mappings[7], mappings[10]],
        },
    )
}

fn stream(state: RuntimeStateV1, fixture: Fixture, seed: u8) -> ModelAsyncStreamV1 {
    let registry = registry(state, fixture, seed);
    ModelAsyncStreamV1::new_model_only(registry, digest(seed.wrapping_add(1))).unwrap()
}

fn registry(state: RuntimeStateV1, fixture: Fixture, seed: u8) -> AsyncQueueRegistryV1 {
    AsyncQueueRegistryV1::new_model_only(state, digest(seed), fixture.queue, 3).unwrap()
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

fn occurrence(queue: QueueKeyV1, id: u64) -> (DispatchKeyV1, CompletionKeyV1) {
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

fn lazy<K>(
    stream: &ModelAsyncStreamV1,
    fixture: Fixture,
    index: usize,
    id: u64,
    dependencies: Vec<AsyncEventDependencyV1>,
) -> LazyTypedAsyncOperationV1<K> {
    let (dispatch, completion) = occurrence(fixture.queue, id);
    LazyTypedAsyncOperationV1::new_model_only(
        TypedAsyncKernelV1::new_model_only(digest(0x60 + index as u8)).unwrap(),
        stream,
        dispatch,
        completion,
        resources(
            fixture,
            index,
            fixture.data[index],
            MemoryAccessV1::ReadWrite,
        ),
        dependencies,
    )
    .unwrap()
}

fn complete<K: core::fmt::Debug>(
    operation: SubmittedTypedAsyncOperationV1<K>,
    stream: &mut ModelAsyncStreamV1,
) -> CompletedTypedAsyncOperationV1<K> {
    match operation
        .poll_model_only(stream, AsyncCompletionObservationV1::Completed)
        .unwrap()
    {
        TypedAsyncOperationPollV1::Completed(completed) => completed,
        other => panic!("unexpected typed poll result: {other:?}"),
    }
}

#[test]
fn lazy_submission_event_dependency_and_completion_are_exactly_sequenced() {
    let (state, fixture) = live_fixture(1);
    let mut stream = stream(state, fixture, 0x20);
    let first = lazy::<Alpha>(&stream, fixture, 0, 10, Vec::new());
    assert_eq!(stream.retained_operation_count(), 0);
    assert_eq!(stream.next_sequence(), 1);

    let first = first.reserve_model_only(&mut stream).unwrap();
    assert_eq!(first.identity().stream_sequence(), 1);
    assert_eq!(stream.retained_operation_count(), 1);
    let first = first.publish_model_only(&mut stream).unwrap();
    let timeout = first.observe_timeout_model_only(&mut stream).unwrap();
    assert_eq!(timeout.observation_count(), 1);
    assert_eq!(stream.retained_operation_count(), 1);
    let cancellation = timeout
        .into_submitted()
        .request_cancellation_model_only(&mut stream)
        .unwrap();
    assert!(cancellation.is_first_request());
    assert_eq!(stream.retained_operation_count(), 1);
    let first = match cancellation
        .into_submitted()
        .poll_model_only(&mut stream, AsyncCompletionObservationV1::Pending)
        .unwrap()
    {
        TypedAsyncOperationPollV1::Pending(pending) => pending,
        other => panic!("unexpected typed poll result: {other:?}"),
    };
    let first = complete(first, &mut stream);
    let event = first.event();
    assert_eq!(event.operation().stream_sequence(), 1);
    assert_eq!(stream.retained_operation_count(), 1);
    let receipt = first.recycle_model_only(&mut stream).unwrap();
    assert_eq!(receipt.event().operation(), event.operation());
    assert_eq!(stream.retained_operation_count(), 0);

    let second = lazy::<Beta>(&stream, fixture, 1, 11, vec![event.as_dependency()]);
    let second = second.reserve_model_only(&mut stream).unwrap();
    assert_eq!(second.identity().stream_sequence(), 2);
    assert_eq!(second.dependencies(), &[event.as_dependency()]);
    let second = second.publish_model_only(&mut stream).unwrap();
    complete(second, &mut stream)
        .recycle_model_only(&mut stream)
        .unwrap();
    assert_eq!(stream.next_sequence(), 3);
    stream.validate_global_invariants().unwrap();
}

#[test]
fn multiple_reservations_enforce_publication_completion_and_recycle_order() {
    let (state, fixture) = live_fixture(1);
    let mut stream = stream(state, fixture, 0x28);
    let first = lazy::<Alpha>(&stream, fixture, 0, 15, Vec::new())
        .reserve_model_only(&mut stream)
        .unwrap();
    let second = lazy::<Beta>(&stream, fixture, 1, 16, Vec::new())
        .reserve_model_only(&mut stream)
        .unwrap();
    assert_eq!(first.identity().stream_sequence(), 1);
    assert_eq!(second.identity().stream_sequence(), 2);
    assert_eq!(stream.next_publication_sequence(), 1);

    let failure = second.publish_model_only(&mut stream).unwrap_err();
    assert_eq!(
        failure.error(),
        TypedAsyncErrorV1::StreamPublicationOrderingMismatch
    );
    let second = failure.into_retained();
    assert_eq!(stream.next_publication_sequence(), 1);
    let failure = second
        .cancel_before_publication_model_only(&mut stream)
        .unwrap_err();
    assert_eq!(
        failure.error(),
        TypedAsyncErrorV1::StreamPublicationOrderingMismatch
    );
    let second = failure.into_retained();

    let first = first.publish_model_only(&mut stream).unwrap();
    let second = second.publish_model_only(&mut stream).unwrap();
    assert_eq!(stream.next_publication_sequence(), 3);
    let failure = second
        .poll_model_only(&mut stream, AsyncCompletionObservationV1::Completed)
        .unwrap_err();
    assert_eq!(
        failure.error(),
        TypedAsyncErrorV1::StreamCompletionOrderingMismatch
    );
    let second = failure.into_retained();
    assert_eq!(stream.next_completion_sequence(), 1);
    let failure = second
        .poll_model_only(
            &mut stream,
            AsyncCompletionObservationV1::Indeterminate(
                AsyncIndeterminateReasonV1::ObservationUnavailable,
            ),
        )
        .unwrap_err();
    assert_eq!(
        failure.error(),
        TypedAsyncErrorV1::StreamCompletionOrderingMismatch
    );
    let second = failure.into_retained();

    let first = complete(first, &mut stream);
    let second = complete(second, &mut stream);
    assert_eq!(stream.next_completion_sequence(), 3);
    let failure = second.recycle_model_only(&mut stream).unwrap_err();
    assert_eq!(
        failure.error(),
        TypedAsyncErrorV1::StreamRecycleOrderingMismatch
    );
    let second = failure.into_retained();
    assert_eq!(stream.next_recycle_sequence(), 1);

    first.recycle_model_only(&mut stream).unwrap();
    second.recycle_model_only(&mut stream).unwrap();
    assert_eq!(stream.next_recycle_sequence(), 3);
    assert_eq!(stream.retained_operation_count(), 0);
    stream.validate_global_invariants().unwrap();
}

#[test]
fn prepublication_cancellation_is_skipped_by_later_ordering_frontiers() {
    let (state, fixture) = live_fixture(1);
    let mut stream = stream(state, fixture, 0x2c);
    let first = lazy::<Alpha>(&stream, fixture, 0, 17, Vec::new())
        .reserve_model_only(&mut stream)
        .unwrap()
        .publish_model_only(&mut stream)
        .unwrap();
    lazy::<Beta>(&stream, fixture, 1, 18, Vec::new())
        .reserve_model_only(&mut stream)
        .unwrap()
        .cancel_before_publication_model_only(&mut stream)
        .unwrap();
    assert_eq!(stream.next_publication_sequence(), 3);
    assert_eq!(stream.next_completion_sequence(), 1);
    assert_eq!(stream.next_recycle_sequence(), 1);

    let first = complete(first, &mut stream);
    assert_eq!(stream.next_completion_sequence(), 3);
    first.recycle_model_only(&mut stream).unwrap();
    assert_eq!(stream.next_recycle_sequence(), 3);

    complete(
        lazy::<Alpha>(&stream, fixture, 2, 19, Vec::new())
            .reserve_model_only(&mut stream)
            .unwrap()
            .publish_model_only(&mut stream)
            .unwrap(),
        &mut stream,
    )
    .recycle_model_only(&mut stream)
    .unwrap();
    assert_eq!(stream.next_recycle_sequence(), 4);
}

#[test]
fn cancellation_before_publication_releases_but_never_reuses_sequence() {
    let (state, fixture) = live_fixture(1);
    let mut stream = stream(state, fixture, 0x30);
    let first = lazy::<Alpha>(&stream, fixture, 0, 20, Vec::new())
        .reserve_model_only(&mut stream)
        .unwrap();
    assert_eq!(first.identity().stream_sequence(), 1);
    first
        .cancel_before_publication_model_only(&mut stream)
        .unwrap();
    assert_eq!(stream.retained_operation_count(), 0);

    let second = lazy::<Alpha>(&stream, fixture, 1, 21, Vec::new())
        .reserve_model_only(&mut stream)
        .unwrap();
    assert_eq!(second.identity().stream_sequence(), 2);
    second
        .cancel_before_publication_model_only(&mut stream)
        .unwrap();
}

#[test]
fn indeterminate_outcome_retains_resources_and_has_no_release_route() {
    let (state, fixture) = live_fixture(1);
    let mut stream = stream(state, fixture, 0x40);
    let submitted = lazy::<Alpha>(&stream, fixture, 0, 30, Vec::new())
        .reserve_model_only(&mut stream)
        .unwrap()
        .publish_model_only(&mut stream)
        .unwrap();
    let quarantined = match submitted
        .poll_model_only(
            &mut stream,
            AsyncCompletionObservationV1::Indeterminate(AsyncIndeterminateReasonV1::QueueFault),
        )
        .unwrap()
    {
        TypedAsyncOperationPollV1::Indeterminate(quarantined) => quarantined,
        other => panic!("unexpected typed poll result: {other:?}"),
    };
    assert_eq!(quarantined.reason(), AsyncIndeterminateReasonV1::QueueFault);
    assert_eq!(quarantined.resources().data()[0].mapping, fixture.data[0]);
    drop(quarantined);
    assert_eq!(stream.retained_operation_count(), 1);
    assert!(stream.into_runtime_state().is_err());
}

#[test]
fn dropping_submitted_custody_is_not_cancellation_or_release() {
    let (state, fixture) = live_fixture(1);
    let mut stream = stream(state, fixture, 0x48);
    let submitted = lazy::<Alpha>(&stream, fixture, 0, 35, Vec::new())
        .reserve_model_only(&mut stream)
        .unwrap()
        .publish_model_only(&mut stream)
        .unwrap();
    drop(submitted);
    assert_eq!(stream.retained_operation_count(), 1);
    assert_eq!(stream.available_slot_count(), 2);
    assert!(stream.into_runtime_state().is_err());
}

#[test]
fn stream_substitution_returns_the_original_lazy_custody() {
    let (state, fixture) = live_fixture(1);
    let mut first = stream(state.clone(), fixture, 0x50);
    let mut second = stream(state, fixture, 0x52);
    let operation = lazy::<Alpha>(&first, fixture, 0, 40, Vec::new());
    let failure = operation.reserve_model_only(&mut second).unwrap_err();
    assert_eq!(failure.error(), TypedAsyncErrorV1::StreamMismatch);
    assert_eq!(second.retained_operation_count(), 0);
    let operation = failure.into_retained();
    operation
        .reserve_model_only(&mut first)
        .unwrap()
        .cancel_before_publication_model_only(&mut first)
        .unwrap();
}

#[test]
fn alias_rejection_is_failure_atomic_and_returns_lazy_custody() {
    let (state, fixture) = live_fixture(1);
    let mut stream = stream(state, fixture, 0x60);
    let first = lazy::<Alpha>(&stream, fixture, 0, 50, Vec::new())
        .reserve_model_only(&mut stream)
        .unwrap();
    let (dispatch, completion) = occurrence(fixture.queue, 51);
    let second = LazyTypedAsyncOperationV1::<Beta>::new_model_only(
        TypedAsyncKernelV1::new_model_only(digest(0x70)).unwrap(),
        &stream,
        dispatch,
        completion,
        resources(fixture, 1, fixture.data[0], MemoryAccessV1::ReadWrite),
        Vec::new(),
    )
    .unwrap();
    let failure = second.reserve_model_only(&mut stream).unwrap_err();
    assert_eq!(
        failure.error(),
        TypedAsyncErrorV1::Queue(AsyncQueueErrorV1::ResourceConflict)
    );
    assert_eq!(stream.retained_operation_count(), 1);
    let second = failure.into_retained();
    first
        .cancel_before_publication_model_only(&mut stream)
        .unwrap();
    second
        .reserve_model_only(&mut stream)
        .unwrap()
        .cancel_before_publication_model_only(&mut stream)
        .unwrap();
}

#[test]
fn dependencies_reject_duplicate_and_cross_context_events() {
    let (first_state, first_fixture) = live_fixture(1);
    let mut first_stream = stream(first_state, first_fixture, 0x70);
    let event = complete(
        lazy::<Alpha>(&first_stream, first_fixture, 0, 60, Vec::new())
            .reserve_model_only(&mut first_stream)
            .unwrap()
            .publish_model_only(&mut first_stream)
            .unwrap(),
        &mut first_stream,
    )
    .event();

    let (dispatch, completion) = occurrence(first_fixture.queue, 61);
    assert_eq!(
        LazyTypedAsyncOperationV1::<Beta>::new_model_only(
            TypedAsyncKernelV1::new_model_only(digest(0x71)).unwrap(),
            &first_stream,
            dispatch,
            completion,
            resources(
                first_fixture,
                1,
                first_fixture.data[1],
                MemoryAccessV1::Read,
            ),
            vec![event.as_dependency(), event.as_dependency()],
        )
        .unwrap_err(),
        TypedAsyncErrorV1::DuplicateDependency
    );

    let (second_state, second_fixture) = live_fixture(2);
    let second_stream = stream(second_state, second_fixture, 0x72);
    let (dispatch, completion) = occurrence(second_fixture.queue, 62);
    assert_eq!(
        LazyTypedAsyncOperationV1::<Beta>::new_model_only(
            TypedAsyncKernelV1::new_model_only(digest(0x73)).unwrap(),
            &second_stream,
            dispatch,
            completion,
            resources(
                second_fixture,
                0,
                second_fixture.data[0],
                MemoryAccessV1::Read,
            ),
            vec![event.as_dependency()],
        )
        .unwrap_err(),
        TypedAsyncErrorV1::DependencyContextMismatch
    );
}

#[test]
fn invalid_stream_incarnation_returns_registry_without_mutation() {
    let (state, fixture) = live_fixture(1);
    let registry =
        AsyncQueueRegistryV1::new_model_only(state, digest(0x7a), fixture.queue, 1).unwrap();
    let error = ModelAsyncStreamV1::new_model_only(
        registry,
        IdentityDigestV1::from_untrusted_bytes([0; IDENTITY_DIGEST_BYTES_V1]),
    )
    .unwrap_err();
    assert_eq!(error.error(), TypedAsyncErrorV1::InvalidStreamIncarnation);
    assert_eq!(error.into_registry().retained_operation_count(), 0);
}

#[test]
fn stream_creation_rejects_reserved_raw_registry_and_returns_exact_custody() {
    let (state, fixture) = live_fixture(1);
    let mut registry = registry(state, fixture, 0x80);
    let (dispatch, completion) = occurrence(fixture.queue, 80);
    let reserved = registry
        .reserve_model_only(
            dispatch,
            completion,
            resources(fixture, 0, fixture.data[0], MemoryAccessV1::ReadWrite),
        )
        .unwrap();
    let binding = reserved.binding();

    let error = ModelAsyncStreamV1::new_model_only(registry, digest(0x81)).unwrap_err();
    assert_eq!(
        error.error(),
        TypedAsyncErrorV1::Queue(AsyncQueueErrorV1::QueueAlreadyRetainsOperations)
    );
    let mut registry = error.into_registry();
    assert_eq!(
        registry.slots()[binding.slot_index() as usize].binding(),
        Some(binding)
    );
    reserved
        .cancel_before_publication_model_only(&mut registry)
        .unwrap();
    assert_eq!(registry.retained_operation_count(), 0);
}

#[test]
fn stream_creation_rejects_submitted_raw_registry_and_returns_exact_custody() {
    let (state, fixture) = live_fixture(1);
    let mut registry = registry(state, fixture, 0x82);
    let (dispatch, completion) = occurrence(fixture.queue, 81);
    let submitted = registry
        .reserve_model_only(
            dispatch,
            completion,
            resources(fixture, 0, fixture.data[0], MemoryAccessV1::ReadWrite),
        )
        .unwrap()
        .publish_model_only(&mut registry)
        .unwrap();
    let binding = submitted.binding();

    let error = ModelAsyncStreamV1::new_model_only(registry, digest(0x83)).unwrap_err();
    assert_eq!(
        error.error(),
        TypedAsyncErrorV1::Queue(AsyncQueueErrorV1::QueueAlreadyRetainsOperations)
    );
    let mut registry = error.into_registry();
    assert_eq!(
        registry.slots()[binding.slot_index() as usize].binding(),
        Some(binding)
    );
    let completed = match submitted
        .poll_model_only(&mut registry, AsyncCompletionObservationV1::Completed)
        .unwrap()
    {
        AsyncOperationPollV1::Completed(completed) => completed,
        other => panic!("unexpected raw poll result: {other:?}"),
    };
    completed.recycle_model_only(&mut registry).unwrap();
    assert_eq!(registry.retained_operation_count(), 0);
}

#[test]
fn stream_creation_rejects_completed_raw_registry_and_returns_exact_custody() {
    let (state, fixture) = live_fixture(1);
    let mut registry = registry(state, fixture, 0x84);
    let (dispatch, completion) = occurrence(fixture.queue, 82);
    let submitted = registry
        .reserve_model_only(
            dispatch,
            completion,
            resources(fixture, 0, fixture.data[0], MemoryAccessV1::ReadWrite),
        )
        .unwrap()
        .publish_model_only(&mut registry)
        .unwrap();
    let completed = match submitted
        .poll_model_only(&mut registry, AsyncCompletionObservationV1::Completed)
        .unwrap()
    {
        AsyncOperationPollV1::Completed(completed) => completed,
        other => panic!("unexpected raw poll result: {other:?}"),
    };
    let binding = completed.binding();

    let error = ModelAsyncStreamV1::new_model_only(registry, digest(0x85)).unwrap_err();
    assert_eq!(
        error.error(),
        TypedAsyncErrorV1::Queue(AsyncQueueErrorV1::QueueAlreadyRetainsOperations)
    );
    let mut registry = error.into_registry();
    assert_eq!(
        registry.slots()[binding.slot_index() as usize].binding(),
        Some(binding)
    );
    completed.recycle_model_only(&mut registry).unwrap();
    assert_eq!(registry.retained_operation_count(), 0);
}
