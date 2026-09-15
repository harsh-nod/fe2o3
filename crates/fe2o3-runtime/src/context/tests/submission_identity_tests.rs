use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

type Context = RuntimeContextV1<MockBackend>;
type Submission = RuntimeSubmissionV1<AddArguments>;

#[derive(Default)]
struct CallbackCounts {
    called: AtomicUsize,
    dropped: AtomicUsize,
}

struct CallbackProbe(Arc<CallbackCounts>);

impl CallbackProbe {
    fn complete(self, _status: RuntimeCompletionStatusV1) {
        self.0.called.fetch_add(1, Ordering::SeqCst);
    }
}

impl Drop for CallbackProbe {
    fn drop(&mut self) {
        self.0.dropped.fetch_add(1, Ordering::SeqCst);
    }
}

fn counts(probe: &CallbackCounts) -> (usize, usize) {
    (
        probe.called.load(Ordering::SeqCst),
        probe.dropped.load(Ordering::SeqCst),
    )
}

fn sorted<K: Ord, V>(items: impl IntoIterator<Item = (K, V)>) -> Vec<(K, V)> {
    let mut items: Vec<_> = items.into_iter().collect();
    items.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    items
}

fn set(items: &HashSet<u64>) -> Vec<u64> {
    let mut items: Vec<_> = items.iter().copied().collect();
    items.sort_unstable();
    items
}

#[derive(Debug, Eq, PartialEq)]
struct TokenSnapshot {
    id: RuntimeSubmissionIdV1,
    backend: u64,
    stream: RuntimeStreamIdV1,
    device: RuntimeDeviceIdV1,
    completion: Option<RuntimePollV1>,
    peer: Option<PeerTransferMechanismV1>,
}

fn token_snapshot<A>(token: &RuntimeSubmissionV1<A>) -> TokenSnapshot {
    TokenSnapshot {
        id: token.id,
        backend: token.backend_submission,
        stream: token.stream,
        device: token.device,
        completion: token.completion,
        peer: token.peer_transfer,
    }
}

// Private test snapshots model stale or substituted tokens without adding a public Clone API.
fn copy_token<A>(token: &RuntimeSubmissionV1<A>) -> RuntimeSubmissionV1<A> {
    RuntimeSubmissionV1 {
        id: token.id,
        backend_submission: token.backend_submission,
        stream: token.stream,
        device: token.device,
        completion: token.completion,
        peer_transfer: token.peer_transfer,
        marker: PhantomData,
    }
}

type StreamSnapshot = (u64, RuntimeDeviceIdV1, Option<u64>, Option<u64>);
type AllocationSnapshot = (u64, RuntimeDeviceIdV1, RuntimeMemoryKindV1, u64);
type ModuleSnapshot = (u64, RuntimeDeviceIdV1, [u8; 32]);
type EventSnapshot = (u64, RuntimeDeviceIdV1, RuntimeSubmissionIdV1);
type SubmissionSnapshot = (
    u64,
    RuntimeStreamIdV1,
    RuntimeDeviceIdV1,
    bool,
    RuntimeCompletionStatusV1,
);
type CallbackStorageSnapshot = (usize, usize, Vec<usize>);
type MemorySnapshot = (Vec<u8>, usize, usize);

#[derive(Debug, Eq, PartialEq)]
struct BackendSnapshot {
    next: u64,
    counts: [usize; 8],
    memory: Vec<(u64, MemorySnapshot)>,
    polls: Vec<(u64, u8)>,
    cleanup: Vec<(MockCleanupKind, u64)>,
    cleanup_capacity: usize,
    failure: (
        MockMemoryFailure,
        MockMemoryFailure,
        MockCleanupFailure,
        MockFlushFailure,
        MockWaitFailure,
    ),
    handle_override: Option<(MockHandleKind, u64)>,
    flags: (bool, bool),
    device_lengths: (usize, usize),
    capabilities: RuntimeExecutionCapabilitiesV1,
    waited_submission: Option<u64>,
    drained_submission: Option<u64>,
    cancelled_submission: Option<u64>,
    recorded_event: Option<(u64, u64)>,
    flushed_stream: Option<u64>,
    geometry: Option<RuntimeLaunchGeometryV1>,
    atomic: Option<RuntimeAtomicLaunchContractV1>,
    collective: Option<RuntimeCollectiveLaunchContractV1>,
    wait_observation: Option<BackendPollV1>,
    deadlines: Vec<Instant>,
    storage: [usize; 3],
}

#[derive(Debug, Eq, PartialEq)]
struct Snapshot {
    identity: (u64, u64),
    terminal: bool,
    graph: (Option<ContextGraphReservationV1>, bool),
    devices: Vec<RuntimeDeviceV1>,
    streams: Vec<(RuntimeStreamIdV1, StreamSnapshot)>,
    allocations: Vec<(RuntimeAllocationIdV1, AllocationSnapshot)>,
    modules: Vec<(RuntimeModuleIdV1, ModuleSnapshot)>,
    kernels: Vec<(u64, RuntimeModuleIdV1)>,
    events: Vec<(RuntimeEventIdV1, EventSnapshot)>,
    submissions: Vec<(RuntimeSubmissionIdV1, SubmissionSnapshot)>,
    backend_handles: [Vec<u64>; 5],
    callbacks: Vec<(RuntimeSubmissionIdV1, CallbackStorageSnapshot)>,
    callback_counts: (usize, u64),
    credits: Vec<Option<crate::RuntimeResourceCreditUsageV1>>,
    capacities: [usize; 12],
    probes: Vec<(usize, usize)>,
    backend: BackendSnapshot,
}

fn snapshot(context: &Context, probes: &[Arc<CallbackCounts>]) -> Snapshot {
    let b = &context.backend;
    Snapshot {
        identity: (context.context_generation, context.next_identity),
        terminal: context.terminal,
        graph: (context.graph_reservation, context.graph_issue_closed),
        devices: context.devices.clone(),
        streams: sorted(context.streams.iter().map(|(id, r)| {
            (
                *id,
                (r.backend_stream, r.device, r.unpublished, r.generated),
            )
        })),
        allocations: sorted(
            context
                .allocations
                .iter()
                .map(|(id, r)| (*id, (r.backend_allocation, r.device, r.kind, r.byte_len))),
        ),
        modules: sorted(
            context
                .modules
                .iter()
                .map(|(id, r)| (*id, (r.backend_module, r.device, r.image_sha256))),
        ),
        kernels: sorted(context.kernels.iter().map(|(id, r)| (*id, r.module))),
        events: sorted(
            context
                .events
                .iter()
                .map(|(id, r)| (*id, (r.backend_event, r.device, r.submission))),
        ),
        submissions: sorted(context.submissions.iter().map(|(id, r)| {
            (
                *id,
                (
                    r.backend_submission,
                    r.stream,
                    r.device,
                    r.quiescent,
                    r.status,
                ),
            )
        })),
        backend_handles: [
            set(&context.backend_streams),
            set(&context.backend_allocations),
            set(&context.backend_modules),
            set(&context.backend_events),
            set(&context.backend_submissions),
        ],
        callbacks: sorted(context.completion_callbacks.iter().map(|(id, callbacks)| {
            (
                *id,
                (
                    callbacks.capacity(),
                    callbacks.as_ptr() as usize,
                    callbacks
                        .iter()
                        .map(|c| c.as_ref() as *const _ as *const () as usize)
                        .collect(),
                ),
            )
        })),
        callback_counts: (
            context.completion_callback_count,
            context.completion_callback_panic_count,
        ),
        credits: context
            .devices
            .iter()
            .map(|d| context.allocation_admission_usage_v1(d.id()).unwrap())
            .collect(),
        capacities: [
            context.devices.capacity(),
            context.streams.capacity(),
            context.allocations.capacity(),
            context.modules.capacity(),
            context.kernels.capacity(),
            context.events.capacity(),
            context.submissions.capacity(),
            context.backend_streams.capacity(),
            context.backend_allocations.capacity(),
            context.backend_modules.capacity(),
            context.backend_events.capacity(),
            context.backend_submissions.capacity(),
        ],
        probes: probes.iter().map(|p| counts(p)).collect(),
        backend: BackendSnapshot {
            next: b.next,
            counts: [
                b.allocation_calls,
                b.submit_count,
                b.poll_call_count,
                b.wait_call_count,
                b.cancel_call_count,
                b.flush_call_count,
                b.last_dependency_count,
                context.completion_callbacks.capacity(),
            ],
            memory: sorted(b.memory.iter().map(|(id, bytes)| {
                (
                    *id,
                    (bytes.clone(), bytes.as_ptr() as usize, bytes.capacity()),
                )
            })),
            polls: sorted(b.polls.iter().map(|(id, n)| (*id, *n))),
            cleanup: b.cleanup_log.clone(),
            cleanup_capacity: b.cleanup_log.capacity(),
            failure: (
                b.allocation_failure,
                b.release_allocation_failure,
                b.cleanup_failure,
                b.flush_failure,
                b.first_wait_failure,
            ),
            handle_override: b.handle_override,
            flags: (b.terminal_on_submit, b.cancel_before_publication),
            device_lengths: (b.device_name_len, b.device_target_len),
            capabilities: b.execution_capabilities,
            waited_submission: b.last_waited_submission,
            drained_submission: b.last_drained_submission,
            cancelled_submission: b.last_cancelled_submission,
            recorded_event: b.last_recorded_event,
            flushed_stream: b.last_flushed_stream,
            geometry: b.last_launch_geometry,
            atomic: b.last_atomic_contract,
            collective: b.last_collective_contract,
            wait_observation: b.wait_observation,
            deadlines: b.wait_deadlines.clone(),
            storage: [
                b.memory.capacity(),
                b.polls.capacity(),
                b.wait_deadlines.capacity(),
            ],
        },
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Ingress {
    Poll,
    Wait,
    Query,
    Callback,
    Release,
    Event,
    Cancel,
    Drain,
}

const INGRESSES: [Ingress; 8] = [
    Ingress::Poll,
    Ingress::Wait,
    Ingress::Query,
    Ingress::Callback,
    Ingress::Release,
    Ingress::Event,
    Ingress::Cancel,
    Ingress::Drain,
];

#[derive(Clone, Copy, Debug)]
enum Coordinate {
    Context,
    Logical,
    Backend,
    Stream,
    Device,
}

const COORDINATES: [Coordinate; 5] = [
    Coordinate::Context,
    Coordinate::Logical,
    Coordinate::Backend,
    Coordinate::Stream,
    Coordinate::Device,
];

struct Fixture {
    context: Context,
    foreign: Context,
    target: Submission,
    neighbor: Submission,
    probes: [Arc<CallbackCounts>; 2],
}

impl Fixture {
    fn new(completed: bool) -> Self {
        let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
        let foreign = RuntimeContextV1::open(MockBackend::default()).unwrap();
        let probes = [
            Arc::new(CallbackCounts::default()),
            Arc::new(CallbackCounts::default()),
        ];
        let mut submissions = Vec::new();
        for (index, probe) in probes.iter().enumerate() {
            let device = context.devices()[index].id();
            context
                .configure_allocation_admission_v1(device, 256, 4)
                .unwrap();
            let stream = context.create_stream(device).unwrap();
            let allocation = context
                .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
                .unwrap();
            let module = context
                .load_module(device, b"identity-test-module")
                .unwrap();
            let kernel = context
                .resolve_kernel::<AddArguments>(module, "add")
                .unwrap();
            let submission = context
                .launch(
                    stream,
                    &kernel,
                    &AddArguments {
                        allocation,
                        scalar: 7,
                    },
                    geometry(),
                    &[],
                )
                .unwrap();
            let callback = CallbackProbe(Arc::clone(probe));
            context
                .on_completion(&submission, move |status| callback.complete(status))
                .unwrap();
            submissions.push(submission);
        }
        let neighbor = submissions.pop().unwrap();
        let target = submissions.pop().unwrap();
        if completed {
            let event = context.record_event(&target).unwrap();
            assert_eq!(
                context.wait_event(event, Duration::from_secs(1)).unwrap(),
                RuntimeCompletionStatusV1::Succeeded
            );
            context.release_event(event).unwrap();
            assert_eq!(target.completion, None);
            assert_eq!(
                context.query_submission(&target),
                Ok(RuntimeCompletionStatusV1::Succeeded)
            );
            assert_eq!(counts(&probes[0]), (1, 1));
        }
        Self {
            context,
            foreign,
            target,
            neighbor,
            probes,
        }
    }

    fn substitute(&self, coordinate: Coordinate) -> Submission {
        let mut token = copy_token(&self.target);
        match coordinate {
            Coordinate::Context => token.id.context_generation = self.foreign.context_generation,
            Coordinate::Logical => token.id.local = self.neighbor.id.local,
            Coordinate::Backend => token.backend_submission = self.neighbor.backend_submission,
            Coordinate::Stream => token.stream = self.neighbor.stream,
            Coordinate::Device => token.device = self.neighbor.device,
        }
        token
    }

    fn cleanup(mut self) {
        assert!(self.context.cleanup().is_complete());
        assert!(self.foreign.cleanup().is_complete());
    }
}

#[derive(Debug, Eq, PartialEq)]
enum Observation {
    Poll(RuntimePollV1),
    Query(RuntimeCompletionStatusV1),
    Registered,
    Released,
    Event(RuntimeEventIdV1),
    Cancel(RuntimeCancellationV1),
}

fn enter<A>(
    context: &mut Context,
    token: &mut Option<RuntimeSubmissionV1<A>>,
    ingress: Ingress,
    probe: &Arc<CallbackCounts>,
) -> Result<Observation, RuntimeErrorV1<MockError>> {
    if ingress == Ingress::Release {
        return match context.release_submission(token.take().unwrap()) {
            Ok(()) => Ok(Observation::Released),
            Err(failure) => {
                let (returned, error) = failure.into_parts();
                *token = Some(returned);
                Err(error)
            }
        };
    }
    let token = token.as_mut().unwrap();
    match ingress {
        Ingress::Poll => context.poll(token).map(Observation::Poll),
        Ingress::Wait => context
            .wait(token, Duration::from_secs(1))
            .map(Observation::Poll),
        Ingress::Query => context
            .query_submission(token)
            .map(Observation::Query)
            .map_err(RuntimeErrorV1::Validation),
        Ingress::Callback => {
            let callback = CallbackProbe(Arc::clone(probe));
            context
                .on_completion(token, move |status| callback.complete(status))
                .map(|()| Observation::Registered)
        }
        Ingress::Event => context.record_event(token).map(Observation::Event),
        Ingress::Cancel => context.cancel(token).map(Observation::Cancel),
        Ingress::Drain => context
            .drain(token, Instant::now() + Duration::from_secs(30))
            .map(Observation::Poll),
        Ingress::Release => unreachable!(),
    }
}

fn reject(
    fixture: &mut Fixture,
    token: Option<Submission>,
    ingress: Ingress,
    expected: RuntimeValidationErrorV1,
) {
    let foreign = snapshot(&fixture.foreign, &[]);
    let target = token_snapshot(&fixture.target);
    let neighbor = token_snapshot(&fixture.neighbor);
    reject_context(
        &mut fixture.context,
        &fixture.probes,
        token,
        ingress,
        expected,
    );
    assert_eq!(snapshot(&fixture.foreign, &[]), foreign);
    assert_eq!(token_snapshot(&fixture.target), target);
    assert_eq!(token_snapshot(&fixture.neighbor), neighbor);
}

fn reject_context<A>(
    context: &mut Context,
    probes: &[Arc<CallbackCounts>],
    mut token: Option<RuntimeSubmissionV1<A>>,
    ingress: Ingress,
    expected: RuntimeValidationErrorV1,
) {
    let before_token = token_snapshot(token.as_ref().unwrap());
    let before = snapshot(context, probes);
    let incoming = Arc::new(CallbackCounts::default());
    let result = enter(context, &mut token, ingress, &incoming);
    assert!(
        matches!(result, Err(RuntimeErrorV1::Validation(error)) if error == expected),
        "{ingress:?}: {result:?}"
    );
    assert_eq!(
        token_snapshot(token.as_ref().unwrap()),
        before_token,
        "supplied token changed"
    );
    assert_eq!(
        snapshot(context, probes),
        before,
        "retained owners or backend changed"
    );
    assert_eq!(
        counts(&incoming),
        (0, usize::from(ingress == Ingress::Callback))
    );
}

fn reject_deadlines_at_context_gate<A>(
    context: &mut Context,
    probes: &[Arc<CallbackCounts>],
    supplied: &RuntimeSubmissionV1<A>,
    expected: RuntimeValidationErrorV1,
) {
    let before = snapshot(context, probes);
    let before_token = token_snapshot(supplied);
    let mut token = copy_token(supplied);
    let wait = context.wait(&mut token, Duration::MAX);
    assert!(matches!(wait, Err(RuntimeErrorV1::Validation(error)) if error == expected));
    assert_eq!(token_snapshot(&token), before_token);
    assert_eq!(snapshot(context, probes), before);
    let drain = context.drain(&mut token, Instant::now());
    assert!(matches!(drain, Err(RuntimeErrorV1::Validation(error)) if error == expected));
    assert_eq!(token_snapshot(&token), before_token);
    assert_eq!(snapshot(context, probes), before);
    assert_eq!(token_snapshot(supplied), before_token);
}

#[test]
fn every_ingress_rejects_each_coordinate_for_pending_and_event_completed_records() {
    let mut cases = 0;
    for completed in [false, true] {
        for ingress in INGRESSES {
            for coordinate in COORDINATES {
                let mut fixture = Fixture::new(completed);
                let token = fixture.substitute(coordinate);
                reject(
                    &mut fixture,
                    Some(token),
                    ingress,
                    RuntimeValidationErrorV1::UnknownSubmission,
                );
                fixture.cleanup();
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 80);
}

#[test]
fn exact_tokens_reach_each_ingress_with_state_appropriate_results() {
    for completed in [false, true] {
        for ingress in INGRESSES {
            let mut fixture = Fixture::new(completed);
            let before = snapshot(&fixture.context, &fixture.probes);
            let mut token = Some(copy_token(&fixture.target));
            let incoming = Arc::new(CallbackCounts::default());
            let mut returned_event = None;
            let result = enter(&mut fixture.context, &mut token, ingress, &incoming);
            if ingress == Ingress::Release && !completed {
                assert!(matches!(
                    result,
                    Err(RuntimeErrorV1::Validation(
                        RuntimeValidationErrorV1::SubmissionPending
                    ))
                ));
                assert_eq!(
                    token_snapshot(token.as_ref().unwrap()),
                    token_snapshot(&fixture.target)
                );
            } else {
                let observed = result.unwrap();
                match ingress {
                    Ingress::Poll => assert_eq!(
                        observed,
                        Observation::Poll(if completed {
                            RuntimePollV1::Succeeded
                        } else {
                            RuntimePollV1::Pending
                        })
                    ),
                    Ingress::Wait | Ingress::Drain => {
                        assert_eq!(observed, Observation::Poll(RuntimePollV1::Succeeded))
                    }
                    Ingress::Query => assert_eq!(
                        observed,
                        Observation::Query(if completed {
                            RuntimeCompletionStatusV1::Succeeded
                        } else {
                            RuntimeCompletionStatusV1::Pending
                        })
                    ),
                    Ingress::Callback => {
                        assert_eq!(observed, Observation::Registered);
                        assert_eq!(counts(&incoming), if completed { (1, 1) } else { (0, 0) });
                    }
                    Ingress::Release => {
                        assert_eq!(observed, Observation::Released);
                        assert!(token.is_none());
                    }
                    Ingress::Event => {
                        let Observation::Event(id) = observed else {
                            panic!("event observation required: {observed:?}");
                        };
                        returned_event = Some(id);
                    }
                    Ingress::Cancel => {
                        assert_eq!(
                            observed,
                            Observation::Cancel(RuntimeCancellationV1::TooLate)
                        );
                        assert_eq!(
                            fixture.context.backend.cancel_call_count,
                            usize::from(!completed)
                        );
                    }
                }
            }
            let context = &fixture.context;
            if let Some(token) = token.as_ref() {
                let mut expected = token_snapshot(&fixture.target);
                if matches!(ingress, Ingress::Wait | Ingress::Drain)
                    || (ingress == Ingress::Poll && completed)
                {
                    expected.completion = Some(RuntimePollV1::Succeeded);
                }
                assert_eq!(token_snapshot(token), expected);
            }
            let id = fixture.target.id;
            let backend_id = fixture.target.backend_submission;
            let after = snapshot(context, &fixture.probes);
            match ingress {
                Ingress::Query => assert_eq!(after, before),
                Ingress::Poll => {
                    let mut expected = before;
                    if !completed {
                        expected.backend.counts[2] += 1;
                        expected
                            .backend
                            .polls
                            .iter_mut()
                            .find(|(key, _)| *key == backend_id)
                            .unwrap()
                            .1 += 1;
                    }
                    assert_eq!(after, expected);
                    assert_eq!(
                        token.as_ref().unwrap().completion,
                        completed.then_some(RuntimePollV1::Succeeded)
                    );
                }
                Ingress::Wait | Ingress::Drain => {
                    assert_eq!(
                        context.backend.wait_call_count,
                        before.backend.counts[3] + usize::from(!completed)
                    );
                    assert_eq!(context.backend.poll_call_count, before.backend.counts[2]);
                    assert_eq!(
                        after.backend.waited_submission,
                        if completed {
                            before.backend.waited_submission
                        } else {
                            Some(backend_id)
                        },
                        "{ingress:?}: exact backend wait submission"
                    );
                    assert_eq!(
                        after.backend.drained_submission,
                        if ingress == Ingress::Drain && !completed {
                            Some(backend_id)
                        } else {
                            before.backend.drained_submission
                        },
                        "{ingress:?}: exact backend drain entry"
                    );
                    assert_eq!(
                        token.as_ref().unwrap().completion,
                        Some(RuntimePollV1::Succeeded)
                    );
                    assert_eq!(
                        context.submissions[&id].status,
                        RuntimeCompletionStatusV1::Succeeded
                    );
                    assert!(context.submissions[&id].quiescent);
                    assert!(!context.completion_callbacks.contains_key(&id));
                    assert_eq!(context.completion_callback_count, 1);
                    assert_eq!(counts(&fixture.probes[0]), (1, 1));
                    if completed {
                        assert_eq!(after, before);
                    }
                }
                Ingress::Callback => {
                    assert_eq!(after.backend, before.backend);
                    assert_eq!(after.submissions, before.submissions);
                    assert_eq!(
                        context.completion_callback_count,
                        before.callback_counts.0 + usize::from(!completed)
                    );
                    if !completed {
                        assert_eq!(context.completion_callbacks[&id].len(), 2);
                    }
                }
                Ingress::Release if completed => {
                    assert!(!context.submissions.contains_key(&id));
                    assert!(!context.backend_submissions.contains(&backend_id));
                    assert!(!context.backend.polls.contains_key(&backend_id));
                    let mut expected_cleanup = before.backend.cleanup;
                    expected_cleanup.push((MockCleanupKind::Submission, backend_id));
                    assert_eq!(context.backend.cleanup_log, expected_cleanup);
                    assert_eq!(context.submissions.len(), 1);
                }
                Ingress::Release => assert_eq!(after, before),
                Ingress::Event => {
                    assert_eq!(
                        after.backend.recorded_event,
                        Some((
                            context.streams[&fixture.target.stream].backend_stream,
                            backend_id
                        )),
                        "exact backend event stream and submission"
                    );
                    let event = &context.events[&returned_event.unwrap()];
                    assert_eq!(event.submission, id);
                    assert_eq!(event.device, fixture.target.device);
                    assert_eq!(event.backend_event, before.backend.next + 1);
                    assert!(context.backend_events.contains(&event.backend_event));
                    assert_eq!(context.events.len(), before.events.len() + 1);
                    assert_eq!(context.next_identity, before.identity.1 + 1);
                    assert_eq!(after.submissions, before.submissions);
                    assert_eq!(after.callbacks, before.callbacks);
                }
                Ingress::Cancel => {
                    let mut expected = before;
                    expected.backend.counts[4] += usize::from(!completed);
                    if !completed {
                        expected.backend.cancelled_submission = Some(backend_id);
                    }
                    assert_eq!(after, expected);
                }
            }
            assert_eq!(
                context.query_submission(&fixture.neighbor),
                Ok(RuntimeCompletionStatusV1::Pending)
            );
            assert_eq!(counts(&fixture.probes[1]), (0, 0));
            let probes = fixture.probes.clone();
            fixture.cleanup();
            assert_eq!(counts(&probes[0]), (1, 1));
            assert_eq!(counts(&probes[1]), (1, 1));
            assert_eq!(
                counts(&incoming),
                if ingress == Ingress::Callback {
                    (1, 1)
                } else {
                    (0, 0)
                }
            );
        }
    }
}

#[test]
fn cached_success_never_bypasses_any_identity_coordinate() {
    let mut cases = 0;
    for ingress in INGRESSES {
        for coordinate in COORDINATES {
            let mut fixture = Fixture::new(false);
            assert_eq!(
                fixture
                    .context
                    .wait(&mut fixture.target, Duration::from_secs(1))
                    .unwrap(),
                RuntimePollV1::Succeeded
            );
            assert_eq!(fixture.target.completion, Some(RuntimePollV1::Succeeded));
            let token = fixture.substitute(coordinate);
            reject(
                &mut fixture,
                Some(token),
                ingress,
                RuntimeValidationErrorV1::UnknownSubmission,
            );
            fixture.cleanup();
            cases += 1;
        }
    }
    assert_eq!(cases, 40);
}

#[test]
fn released_cached_token_is_rejected_at_every_ingress() {
    for ingress in INGRESSES {
        let mut fixture = Fixture::new(false);
        fixture
            .context
            .wait(&mut fixture.target, Duration::from_secs(1))
            .unwrap();
        fixture
            .context
            .release_submission(copy_token(&fixture.target))
            .unwrap();
        assert!(!fixture.context.submissions.contains_key(&fixture.target.id));
        let stale = copy_token(&fixture.target);
        reject(
            &mut fixture,
            Some(stale),
            ingress,
            RuntimeValidationErrorV1::UnknownSubmission,
        );
        fixture.cleanup();
    }
}

#[test]
fn real_backend_id_reuse_does_not_revive_a_released_cached_token() {
    for ingress in INGRESSES {
        let mut fixture = Fixture::new(false);
        fixture
            .context
            .wait(&mut fixture.target, Duration::from_secs(1))
            .unwrap();
        fixture
            .context
            .release_submission(copy_token(&fixture.target))
            .unwrap();
        let device = fixture.target.device;
        let context = &mut fixture.context;
        let allocation = *context
            .allocations
            .iter()
            .find(|(_, record)| record.device == device)
            .unwrap()
            .0;
        let module = *context
            .modules
            .iter()
            .find(|(_, record)| record.device == device)
            .unwrap()
            .0;
        let kernel = context
            .resolve_kernel::<AddArguments>(module, "add")
            .unwrap();
        context.backend.handle_override = Some((
            MockHandleKind::Submission,
            fixture.target.backend_submission,
        ));
        let mut replacement = context
            .launch(
                fixture.target.stream,
                &kernel,
                &AddArguments {
                    allocation,
                    scalar: 9,
                },
                geometry(),
                &[],
            )
            .unwrap();
        assert_eq!(
            replacement.backend_submission,
            fixture.target.backend_submission
        );
        assert_ne!(replacement.id, fixture.target.id);
        assert_eq!(replacement.completion, None);
        assert!(context.backend.handle_override.is_none());
        let callback = CallbackProbe(Arc::clone(&fixture.probes[0]));
        context
            .on_completion(&replacement, move |status| callback.complete(status))
            .unwrap();
        let before_replacement = token_snapshot(&replacement);
        let stale = copy_token(&fixture.target);
        reject(
            &mut fixture,
            Some(stale),
            ingress,
            RuntimeValidationErrorV1::UnknownSubmission,
        );
        assert_eq!(token_snapshot(&replacement), before_replacement);
        assert_eq!(counts(&fixture.probes[0]), (1, 1));
        assert_eq!(
            fixture.context.query_submission(&replacement),
            Ok(RuntimeCompletionStatusV1::Pending)
        );
        assert_eq!(
            fixture
                .context
                .wait(&mut replacement, Duration::from_secs(1))
                .unwrap(),
            RuntimePollV1::Succeeded
        );
        assert_eq!(counts(&fixture.probes[0]), (2, 2));
        fixture.context.release_submission(replacement).unwrap();
        fixture.cleanup();
    }
}

#[test]
fn destroyed_stream_keeps_identity_validation_and_retained_query_semantics() {
    for ingress in INGRESSES {
        for coordinate in COORDINATES {
            let mut fixture = Fixture::new(false);
            fixture
                .context
                .destroy_stream(fixture.target.stream)
                .unwrap();
            assert_eq!(
                fixture.context.query_submission(&fixture.target),
                Ok(RuntimeCompletionStatusV1::QuiescentWithoutResult)
            );
            let token = fixture.substitute(coordinate);
            reject(
                &mut fixture,
                Some(token),
                ingress,
                RuntimeValidationErrorV1::UnknownSubmission,
            );
            fixture.cleanup();
        }
        let mut fixture = Fixture::new(false);
        fixture
            .context
            .destroy_stream(fixture.target.stream)
            .unwrap();
        let token = copy_token(&fixture.target);
        if matches!(ingress, Ingress::Poll | Ingress::Wait | Ingress::Event) {
            reject(
                &mut fixture,
                Some(token),
                ingress,
                RuntimeValidationErrorV1::UnknownStream,
            );
        } else {
            let before = snapshot(&fixture.context, &fixture.probes);
            let incoming = Arc::new(CallbackCounts::default());
            let mut token = Some(token);
            let observed = enter(&mut fixture.context, &mut token, ingress, &incoming).unwrap();
            match ingress {
                Ingress::Query => assert_eq!(
                    observed,
                    Observation::Query(RuntimeCompletionStatusV1::QuiescentWithoutResult)
                ),
                Ingress::Callback => {
                    assert_eq!(observed, Observation::Registered);
                    assert_eq!(counts(&incoming), (1, 1));
                }
                Ingress::Cancel => assert_eq!(
                    observed,
                    Observation::Cancel(RuntimeCancellationV1::TooLate)
                ),
                Ingress::Drain => {
                    let quiescent = RuntimePollV1::Failed {
                        code: RUNTIME_QUIESCENT_WITHOUT_RESULT_CODE_V1,
                    };
                    assert_eq!(observed, Observation::Poll(quiescent));
                    assert_eq!(token.as_ref().unwrap().completion, Some(quiescent));
                }
                Ingress::Release => {
                    assert_eq!(observed, Observation::Released);
                    assert!(token.is_none());
                    assert!(!fixture.context.submissions.contains_key(&fixture.target.id));
                    assert_eq!(
                        fixture.context.backend.cleanup_log.last(),
                        Some(&(
                            MockCleanupKind::Submission,
                            fixture.target.backend_submission
                        ))
                    );
                }
                _ => unreachable!(),
            }
            if ingress != Ingress::Release {
                assert_eq!(snapshot(&fixture.context, &fixture.probes), before);
            }
        }
        fixture.cleanup();
    }
}

#[test]
fn wait_validates_identity_before_deadline_but_drain_checks_expiration_first() {
    for completed in [false, true] {
        for coordinate in COORDINATES {
            let mut fixture = Fixture::new(completed);
            let mut token = fixture.substitute(coordinate);
            let before_token = token_snapshot(&token);
            let before = snapshot(&fixture.context, &fixture.probes);
            assert!(matches!(
                fixture.context.wait(&mut token, Duration::MAX),
                Err(RuntimeErrorV1::Validation(
                    RuntimeValidationErrorV1::UnknownSubmission
                ))
            ));
            assert_eq!(token_snapshot(&token), before_token);
            assert_eq!(snapshot(&fixture.context, &fixture.probes), before);
            assert!(matches!(
                fixture.context.drain(&mut token, Instant::now()),
                Err(RuntimeErrorV1::Validation(
                    RuntimeValidationErrorV1::InvalidDeadline
                ))
            ));
            assert_eq!(token_snapshot(&token), before_token);
            assert_eq!(snapshot(&fixture.context, &fixture.probes), before);
            fixture.cleanup();
        }
        let mut fixture = Fixture::new(completed);
        let before = snapshot(&fixture.context, &fixture.probes);
        let mut token = copy_token(&fixture.target);
        let result = fixture.context.wait(&mut token, Duration::MAX);
        if completed {
            assert_eq!(result.unwrap(), RuntimePollV1::Succeeded);
            assert_eq!(token.completion, Some(RuntimePollV1::Succeeded));
        } else {
            assert!(matches!(
                result,
                Err(RuntimeErrorV1::Validation(
                    RuntimeValidationErrorV1::InvalidDeadline
                ))
            ));
            assert_eq!(token.completion, None);
        }
        assert_eq!(snapshot(&fixture.context, &fixture.probes), before);
        let before_token = token_snapshot(&token);
        assert!(matches!(
            fixture.context.drain(&mut token, Instant::now()),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::InvalidDeadline
            ))
        ));
        assert_eq!(token_snapshot(&token), before_token);
        assert_eq!(snapshot(&fixture.context, &fixture.probes), before);
        fixture.cleanup();
    }
}

#[test]
fn actual_terminal_backend_failure_precedes_ingress_identity_except_pure_query() {
    for completed in [false, true] {
        for ingress in INGRESSES {
            for coordinate in COORDINATES.into_iter().map(Some).chain([None]) {
                let mut fixture = Fixture::new(completed);
                fixture.context.backend.flush_failure = MockFlushFailure::Terminal;
                assert!(matches!(
                    fixture.context.flush_stream(fixture.target.stream),
                    Err(RuntimeErrorV1::BackendTerminal(_))
                ));
                assert!(fixture.context.is_terminal());
                let token = coordinate
                    .map_or_else(|| copy_token(&fixture.target), |c| fixture.substitute(c));
                if ingress == Ingress::Wait {
                    reject_deadlines_at_context_gate(
                        &mut fixture.context,
                        &fixture.probes,
                        &token,
                        RuntimeValidationErrorV1::ContextTerminal,
                    );
                }
                if ingress == Ingress::Query && coordinate.is_none() {
                    let before = snapshot(&fixture.context, &fixture.probes);
                    assert_eq!(
                        fixture.context.query_submission(&token),
                        Ok(if completed {
                            RuntimeCompletionStatusV1::Succeeded
                        } else {
                            RuntimeCompletionStatusV1::Pending
                        })
                    );
                    assert_eq!(snapshot(&fixture.context, &fixture.probes), before);
                } else {
                    reject(
                        &mut fixture,
                        Some(token),
                        ingress,
                        if ingress == Ingress::Query {
                            RuntimeValidationErrorV1::UnknownSubmission
                        } else {
                            RuntimeValidationErrorV1::ContextTerminal
                        },
                    );
                }
                let before = snapshot(&fixture.context, &fixture.probes);
                let report = fixture.context.cleanup();
                assert!(report.is_terminal());
                assert!(!report.is_complete());
                assert_eq!(snapshot(&fixture.context, &fixture.probes), before);
                assert!(fixture.foreign.cleanup().is_complete());
                let probes = fixture.probes.clone();
                drop(fixture);
                assert_eq!(counts(&probes[0]), (usize::from(completed), 1));
                assert_eq!(counts(&probes[1]), (0, 1));
            }
        }
    }
}

#[test]
fn genuine_graph_reservation_preserves_query_and_public_ingress_precedence() {
    for terminal in [false, true] {
        for ingress in INGRESSES {
            for foreign_token in [false, true] {
                let (mut context, stream, allocation, kernel) = context_with_launch_prerequisites();
                let mut foreign = RuntimeContextV1::open(MockBackend::default()).unwrap();
                let args = AddArguments {
                    allocation,
                    scalar: 3,
                };
                let action = context
                    .prepare_graph_launch_v1(
                        stream,
                        &kernel,
                        &args.encode_explicit_kernarg_v1(),
                        &args.bindings_v1(),
                        geometry(),
                    )
                    .unwrap();
                let reservation = context.reserve_graph_v1(1).unwrap();
                let mut submission = context.submit_graph_action_v1(reservation, action).unwrap();
                assert_eq!(context.submissions.len(), 1);
                assert_eq!(context.backend.submit_count, 1);
                if terminal {
                    context.backend.flush_failure = MockFlushFailure::Terminal;
                    assert!(matches!(
                        context.flush_with_graph_access_v1(stream, Some(reservation)),
                        Err(RuntimeErrorV1::BackendTerminal(_))
                    ));
                }
                let mut token = copy_token(&submission);
                if foreign_token {
                    token.id.context_generation = foreign.context_generation;
                }
                if ingress == Ingress::Wait {
                    reject_deadlines_at_context_gate(
                        &mut context,
                        &[],
                        &token,
                        if terminal {
                            RuntimeValidationErrorV1::ContextTerminal
                        } else {
                            RuntimeValidationErrorV1::ContextReserved
                        },
                    );
                }
                if ingress == Ingress::Query && !foreign_token {
                    let before = snapshot(&context, &[]);
                    assert_eq!(
                        context.query_submission(&token),
                        Ok(RuntimeCompletionStatusV1::Pending)
                    );
                    assert_eq!(snapshot(&context, &[]), before);
                } else {
                    reject_context(
                        &mut context,
                        &[],
                        Some(token),
                        ingress,
                        if ingress == Ingress::Query {
                            RuntimeValidationErrorV1::UnknownSubmission
                        } else if terminal {
                            RuntimeValidationErrorV1::ContextTerminal
                        } else {
                            RuntimeValidationErrorV1::ContextReserved
                        },
                    );
                }
                if terminal {
                    let before = snapshot(&context, &[]);
                    let report = context.cleanup();
                    assert!(report.is_terminal());
                    assert!(report.is_graph_reserved());
                    assert_eq!(snapshot(&context, &[]), before);
                } else {
                    assert_eq!(
                        context
                            .poll_with_graph_access_v1(&mut submission, Some(reservation))
                            .unwrap(),
                        RuntimePollV1::Pending
                    );
                    assert_eq!(
                        context
                            .poll_with_graph_access_v1(&mut submission, Some(reservation))
                            .unwrap(),
                        RuntimePollV1::Succeeded
                    );
                    context
                        .release_graph_submission_v1(reservation, &submission)
                        .unwrap();
                    context.close_graph_issue_v1(reservation).unwrap();
                    context.release_graph_v1(reservation).unwrap();
                    assert!(context.cleanup().is_complete());
                }
                assert!(foreign.cleanup().is_complete());
            }
        }
    }
}
