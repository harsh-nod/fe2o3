use super::*;
use crate::queue_linux::primary_fixture::LocalRuntimeObservationV1;
use fe2o3_runtime_model::{
    ComputeAqlQueuePhaseV1 as Phase, ComputeAqlQueueRecordV1, QueueHistoryEntryV1,
    QueueHistoryEventKindV1 as Event, UntrustedQueueIdObservationV1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Case {
    BeforeCreate,
    AfterCreate,
    BeforeDoorbell,
    AfterDoorbell,
    RuntimeCreated,
    RecoverOutputs,
    RecoverId,
    EventId,
}

impl Case {
    fn callback(self) -> &'static str {
        match self {
            Self::RuntimeCreated => "runtime-created",
            Self::RecoverOutputs => "recover-outputs",
            Self::RecoverId => "recover-id",
            Self::EventId => "event-id",
            _ => "currentness",
        }
    }

    fn created(self) -> bool {
        self != Self::BeforeCreate
    }

    fn assembled(self) -> bool {
        matches!(self, Self::BeforeDoorbell | Self::AfterDoorbell)
    }

    fn recovered(self) -> bool {
        self.assembled() || matches!(self, Self::RecoverId | Self::EventId)
    }

    fn queue_live(self) -> bool {
        !matches!(
            self,
            Self::BeforeCreate | Self::AfterCreate | Self::RuntimeCreated
        )
    }

    fn quarantine(self, panic: bool) -> bool {
        self.callback() == "currentness" && !panic
    }
}

fn assert_recovery_prefix(scope: &Scope, case: Case, panic: bool) {
    assert_pair_custody(scope);
    let root = &scope.construction;
    let engine = &scope.primary.completed.as_ref().unwrap().engine;
    let key = root.key.unwrap();
    assert!(scope.lanes.is_empty());
    assert!(root.authority.is_none());
    assert!(
        root.resource_prefix
            .as_ref()
            .unwrap()
            .primary_fixture_identities_v1()
            .is_empty()
    );
    assert!(
        engine
            .resources
            .iter()
            .find(|r| r.key == key)
            .unwrap()
            .authority
            .is_some()
    );
    assert_eq!(engine.authority_poisoned, case.quarantine(panic));
    assert_eq!(
        engine.phase(key),
        Some(if case.quarantine(panic) {
            Phase::Ambiguous
        } else if case.created() {
            Phase::Active
        } else {
            Phase::Planned
        })
    );
    assert_eq!(engine.native_queue_id(key), case.created().then_some(8));
    let outputs = engine.create_outputs(key);
    assert_eq!(outputs.is_some(), case.created());
    assert_eq!(root.outputs, if case.recovered() { outputs } else { None });
    if let Some(outputs) = outputs {
        assert_eq!(
            trace().borrow().create_returns[1],
            (outputs.queue_id().value(), outputs.doorbell_offset().raw())
        );
    }
    assert_eq!(root.completed.is_some(), case.assembled());
    assert_eq!(root.dispatch.is_some(), !case.assembled());
    assert_eq!(root.completion.retained.is_some(), !case.assembled());
    assert_eq!(root.completion_owner.is_some(), !case.assembled());
    assert_eq!(root.submission.is_some(), !case.assembled());
    assert_eq!(root.runtime.is_some(), !case.assembled());
    assert_eq!(root.event.is_some(), !case.assembled());
    assert_eq!(
        root.published.is_some(),
        case.created() && !case.assembled()
    );
    assert_eq!(root.unpublished.is_some(), !case.created());
    assert!(root.creation_arm.is_some());
    if let Some(lane) = &root.completed {
        assert_eq!(lane.key, key);
        assert!(lane.dispatch.is_some() && lane.submission.is_some());
        assert!(lane.completion_signals.is_some() && lane.completion_owner.0.is_some());
        assert!(lane.exception.is_some());
        assert_eq!(lane.observation.queue_id, 8);
        assert_eq!(lane.observation.ring_bytes, 4096);
        assert_eq!(lane.doorbell.is_some(), case == Case::AfterDoorbell);
        let expected = if case == Case::AfterDoorbell {
            (
                fe2o3_kfd_uapi::KFD_GFX942_PROCESS_DOORBELL_SLICE_BYTES as usize,
                outputs.unwrap().doorbell_offset().in_process_byte_offset(),
            )
        } else {
            (0, 0)
        };
        assert_eq!(
            (
                lane.observation.doorbell_slice_bytes,
                lane.observation.doorbell_byte_offset
            ),
            expected
        );
    }
}

fn assert_history(
    scope: &Scope,
    history: &[QueueHistoryEntryV1],
    mut primary_record: ComputeAqlQueueRecordV1,
    case: Case,
    panic: bool,
) {
    let primary = scope.primary.completed.as_ref().unwrap();
    let engine = &primary.engine;
    let key = scope.construction.key.unwrap();
    let record = engine
        .model
        .queues()
        .iter()
        .find(|r| r.plan.queue == key)
        .unwrap();
    assert_eq!(record.configuration, record.plan.initial_configuration);
    assert!(record.pending_configuration.is_none());
    let mut expected = vec![(
        key,
        Event::PlanAdmitted,
        Phase::Planned,
        Phase::Planned,
        None,
        record.configuration,
    )];
    if case.created() {
        expected.extend([
            (
                key,
                Event::CreateBegan,
                Phase::Planned,
                Phase::CreatePending,
                None,
                record.configuration,
            ),
            (
                key,
                Event::CreateSucceeded,
                Phase::CreatePending,
                Phase::Active,
                Some(UntrustedQueueIdObservationV1(8)),
                record.configuration,
            ),
        ]);
    }
    if case.quarantine(panic) {
        expected.extend([
            (
                primary.key,
                Event::CurrentnessLost,
                primary_record.phase,
                Phase::Ambiguous,
                primary_record.queue_id,
                primary_record.configuration,
            ),
            (
                key,
                Event::CurrentnessLost,
                if case.created() {
                    Phase::Active
                } else {
                    Phase::Planned
                },
                Phase::Ambiguous,
                record.queue_id,
                record.configuration,
            ),
        ]);
        primary_record.phase = Phase::Ambiguous;
    }
    assert_eq!(
        engine
            .model
            .queues()
            .iter()
            .find(|r| r.plan.queue == primary.key),
        Some(&primary_record)
    );
    assert_eq!(&engine.model.history()[..history.len()], history);
    let expected: Vec<_> = expected
        .into_iter()
        .enumerate()
        .map(
            |(index, (queue, event, from, to, queue_id, configuration))| QueueHistoryEntryV1 {
                sequence: (history.len() + index + 1) as u64,
                queue,
                event,
                from,
                to,
                queue_id,
                configuration,
            },
        )
        .collect();
    assert_eq!(
        &engine.model.history()[history.len()..],
        expected,
        "{case:?}, panic={panic}"
    );
}

fn currentness_trace(external: bool) -> (Vec<&'static str>, [usize; 4]) {
    let gate = LocalGateV1::new();
    let mut resources = None;
    let mut auxiliary_start = 0;
    let (scope, result, trace) = prefix_case_with_oracle(
        external,
        |trace| {
            trace.borrow_mut().local_gate = Some(gate.clone());
            resources = Some(trace.borrow().local_resources.clone());
        },
        |_, trace| auxiliary_start = trace.borrow().calls.len(),
        |scope, failed| {
            assert!(!failed);
            assert_pair(scope);
        },
    );
    assert!(result.is_ok());
    assert_eq!(gate.observation(), (false, false));
    assert_eq!(
        gate.runtime_observation(),
        LocalRuntimeObservationV1::Enabled {
            opener_pid: std::process::id(),
            leases: 2,
        }
    );
    let calls = trace.borrow().calls.clone();
    let windows = [
        (["currentness", "publish", "create"], 0),
        (["create", "currentness", "runtime-created"], 1),
        (["event-id", "currentness", "doorbell"], 1),
        (["doorbell-observe", "currentness", "gate-finish"], 1),
    ];
    let positions = windows.map(|(window, currentness)| {
        let matches: Vec<_> = calls[auxiliary_start..]
            .windows(window.len())
            .enumerate()
            .filter_map(|(index, candidate)| {
                (candidate == window).then_some(auxiliary_start + index + currentness)
            })
            .collect();
        assert_eq!(matches.len(), 1, "unique auxiliary window {window:?}");
        matches[0]
    });
    let resources = resources.unwrap();
    assert_eq!(resources.live(), (2, 2, 2));
    drop(scope);
    assert_eq!(resources.live(), (0, 0, 0));
    assert_eq!(gate.observation(), (false, true));
    (calls, positions)
}

fn exercise(case: Case, panic: bool, external: bool, baseline: Option<(&[&'static str], usize)>) {
    let gate = LocalGateV1::new();
    let mut resources = None;
    let mut history = Vec::new();
    let mut primary_record = None;
    let mut occurrence = 0;
    let name = case.callback();
    let (scope, result, trace) = prefix_case_with_oracle(
        external,
        |trace| {
            trace.borrow_mut().local_gate = Some(gate.clone());
            resources = Some(trace.borrow().local_resources.clone());
        },
        |scope, trace| {
            let primary = scope.primary.completed.as_ref().unwrap();
            history = primary.engine.model.history().to_vec();
            primary_record = primary
                .engine
                .model
                .queues()
                .iter()
                .find(|r| r.plan.queue == primary.key)
                .copied();
            let mut t = trace.borrow_mut();
            let previous = t.calls.iter().filter(|&&c| c == name).count();
            occurrence = baseline.map_or(previous + 1, |(calls, position)| {
                calls[..=position].iter().filter(|&&c| c == name).count()
            });
            assert!(occurrence > previous);
            t.fault = Some((name, occurrence, panic));
        },
        |scope, failed| {
            assert!(failed, "{case:?}, panic={panic}, external={external}");
            assert_recovery_prefix(scope, case, panic);
        },
    );
    let payload = result.expect_err("every selected callback must fail");
    if panic {
        assert_eq!(
            payload.downcast_ref::<(&str, usize)>(),
            Some(&(name, occurrence))
        );
    } else {
        let error = payload
            .downcast_ref::<ComputeAqlQueueSessionErrorV1>()
            .unwrap();
        let ComputeAqlQueueSessionErrorV1::TerminalCreation { stage, source } = error else {
            panic!("terminal auxiliary failure: {error:?}");
        };
        let expected = match case {
            Case::BeforeCreate | Case::AfterCreate => "CREATE_QUEUE result",
            Case::BeforeDoorbell | Case::AfterDoorbell => {
                "USERPTR auxiliary queue-control creation"
            }
            Case::RuntimeCreated => "runtime queue-live transition",
            Case::RecoverOutputs => "CREATE_QUEUE output recovery",
            Case::RecoverId => "CREATE_QUEUE identity recovery",
            Case::EventId => unreachable!("event ID has no error-return channel"),
        };
        assert_eq!(*stage, expected);
        match case {
            Case::RuntimeCreated => assert!(matches!(
                &**source,
                ComputeAqlQueueSessionErrorV1::Contract("runtime-created")
            )),
            Case::RecoverOutputs => assert!(matches!(
                &**source,
                ComputeAqlQueueSessionErrorV1::Contract("missing CREATE outputs")
            )),
            Case::RecoverId => assert!(matches!(
                &**source,
                ComputeAqlQueueSessionErrorV1::Contract("missing queue id")
            )),
            _ => assert!(matches!(
                &**source,
                ComputeAqlQueueSessionErrorV1::Native("queue currentness lost")
            )),
        }
    }
    assert_history(&scope, &history, primary_record.unwrap(), case, panic);
    let primary = scope.primary.completed.as_ref().unwrap();
    primary.runtime.assert_local_runtime(&gate, true);
    primary.shadows.assert_local_published();
    let root = &scope.construction;
    if let Some(lane) = &root.completed {
        let exception = lane.exception.as_ref().unwrap();
        exception.runtime.assert_local_runtime(&gate, true);
        exception.shadows.assert_local_published();
    } else {
        root.runtime
            .as_ref()
            .unwrap()
            .assert_local_runtime(&gate, case.queue_live());
        if let Some(unpublished) = &root.unpublished {
            unpublished.assert_local_unpublished(true);
        } else {
            root.published.as_ref().unwrap().assert_local_published();
        }
    }
    assert_eq!(gate.observation(), (true, true));
    assert_eq!(
        gate.runtime_observation(),
        LocalRuntimeObservationV1::Poisoned
    );
    let t = trace.borrow();
    let position = t
        .calls
        .iter()
        .enumerate()
        .filter(|(_, c)| **c == name)
        .nth(occurrence - 1)
        .unwrap()
        .0;
    if let Some((calls, expected_position)) = baseline {
        assert_eq!(position, expected_position);
        assert_eq!(&t.calls[..=position], &calls[..=expected_position]);
    }
    let mut tail = vec!["poison", "auxiliary-parent-retain"];
    if !case.created() {
        tail.push("cleanup");
    }
    tail.push("retain-auxiliary-root");
    assert_eq!(
        &t.calls[position + 1..],
        tail,
        "no construction callback after failure"
    );
    for (callback, reached) in [
        ("create", case.created()),
        ("publish", case.created()),
        (
            "runtime-created",
            !matches!(case, Case::BeforeCreate | Case::AfterCreate),
        ),
        ("recover-outputs", case.queue_live()),
        ("recover-id", case.recovered()),
        ("event-id", case.assembled() || case == Case::EventId),
        ("doorbell", case == Case::AfterDoorbell),
        ("doorbell-observe", case == Case::AfterDoorbell),
        ("gate-finish", false),
    ] {
        assert_eq!(
            t.calls.iter().filter(|&&c| c == callback).count(),
            1 + usize::from(reached),
            "{case:?}: {callback}"
        );
    }
    assert_eq!(t.create_returns.len(), 1 + usize::from(case.created()));
    assert_eq!(t.cleanup, usize::from(!case.created()));
    assert_eq!(t.drops, 0);
    drop(t);
    let resources = resources.unwrap();
    assert_eq!(
        resources.live(),
        if case.created() { (2, 2, 2) } else { (2, 1, 2) }
    );
    drop(scope);
    assert_eq!(
        resources.live(),
        (0, 0, 0),
        "only fixture-owned resources disposed"
    );
    assert_eq!(gate.observation(), (false, true));
}

#[test]
fn same_engine_auxiliary_currentness_boundaries_retain_exact_history_and_phase_owners() {
    for external in [false, true] {
        let (calls, positions) = currentness_trace(external);
        for (case, position) in [
            Case::BeforeCreate,
            Case::AfterCreate,
            Case::BeforeDoorbell,
            Case::AfterDoorbell,
        ]
        .into_iter()
        .zip(positions)
        {
            for panic in [false, true] {
                exercise(case, panic, external, Some((&calls, position)));
            }
        }
    }
}

#[test]
fn same_engine_auxiliary_recovery_failures_keep_exact_outputs_before_owner_assembly() {
    for external in [false, true] {
        for case in [Case::RuntimeCreated, Case::RecoverOutputs, Case::RecoverId] {
            for panic in [false, true] {
                exercise(case, panic, external, None);
            }
        }
        exercise(Case::EventId, true, external, None);
    }
}
