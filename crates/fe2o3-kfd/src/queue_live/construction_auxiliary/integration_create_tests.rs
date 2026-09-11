use super::*;
use crate::queue_linux::primary_fixture::LocalRuntimeObservationV1;
use fe2o3_runtime_model::{
    ComputeAqlQueuePhaseV1 as Phase, QueueHistoryEntryV1, QueueHistoryEventKindV1 as Event,
    UntrustedQueueIdObservationV1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CreateCase {
    NoEffect,
    Indeterminate,
    ReturnedIndeterminate,
    InputDrift,
    Panic,
    InvalidDoorbell,
    NoEffectChangedOutputs,
}

impl CreateCase {
    fn mode(self) -> u8 {
        match self {
            Self::NoEffect => 1,
            Self::Indeterminate => 2,
            Self::ReturnedIndeterminate => 3,
            Self::InputDrift => 4,
            Self::Panic => 5,
            Self::InvalidDoorbell => 6,
            Self::NoEffectChangedOutputs => 7,
        }
    }

    fn phase(self) -> Phase {
        match self {
            Self::NoEffect => Phase::Planned,
            Self::Panic => Phase::CreatePending,
            _ => Phase::Ambiguous,
        }
    }

    fn queue_id(self) -> Option<u32> {
        match self {
            Self::NoEffect | Self::Indeterminate | Self::Panic => None,
            _ => Some(8),
        }
    }

    fn malformed(self) -> bool {
        matches!(
            self,
            Self::InputDrift | Self::InvalidDoorbell | Self::NoEffectChangedOutputs
        )
    }
}

fn assert_create_prefix(scope: &Scope, case: CreateCase) {
    assert_pair_custody(scope);
    let root = &scope.construction;
    let engine = &scope.primary.completed.as_ref().unwrap().engine;
    let key = root.key.unwrap();
    assert!(scope.lanes.is_empty());
    assert!(root.completed.is_none() && root.outputs.is_none());
    assert!(root.dispatch.is_some() && root.completion.retained.is_some());
    assert!(root.completion_owner.is_some() && root.submission.is_some());
    assert!(root.runtime.is_some() && root.event.is_some() && root.creation_arm.is_some());
    assert!(root.unpublished.is_none() && root.published.is_some());
    assert!(root.authority.is_none());
    assert!(
        root.resource_prefix
            .as_ref()
            .unwrap()
            .primary_fixture_identities_v1()
            .is_empty()
    );
    assert_eq!(engine.phase(key), Some(case.phase()));
    assert_eq!(engine.native_queue_id(key), case.queue_id());
    assert!(engine.create_outputs(key).is_none());
    assert_eq!(engine.authority_poisoned, case.malformed());
    let resource = engine.resources.iter().find(|r| r.key == key).unwrap();
    assert_eq!(resource.authority.as_ref().unwrap().view.plan.queue, key);
    assert!(resource.create_outputs.is_none());
    let t = trace();
    let t = t.borrow();
    for name in ["create", "publish"] {
        assert_eq!(t.calls.iter().filter(|&&call| call == name).count(), 2);
    }
    for name in [
        "runtime-created",
        "recover-outputs",
        "recover-id",
        "event-id",
        "doorbell",
        "gate-finish",
    ] {
        assert_eq!(
            t.calls.iter().filter(|&&call| call == name).count(),
            1,
            "{name}"
        );
    }
    assert_eq!(
        t.create_returns.len(),
        if case == CreateCase::Panic { 1 } else { 2 }
    );
    assert_eq!(
        t.cleanup, 0,
        "published shadows cannot be cleaned as unpublished"
    );
    assert_eq!(t.drops, 0);
    assert!(t.poison && t.calls.contains(&"retain-auxiliary-root"));
}

fn exercise_create(case: CreateCase, external_runtime: bool) {
    let gate = LocalGateV1::new();
    let mut resources = None;
    let mut history = Vec::new();
    let mut primary_record = None;
    let (scope, result, trace) = prefix_case_with_oracle(
        external_runtime,
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
            assert_eq!(
                gate.runtime_observation(),
                LocalRuntimeObservationV1::Enabled {
                    opener_pid: std::process::id(),
                    leases: 1,
                }
            );
            assert_eq!(trace.borrow().local_resources.live(), (1, 1, 1));
            trace.borrow_mut().create = case.mode();
        },
        |scope, failed| {
            assert!(failed, "{case:?}, external={external_runtime}");
            assert_create_prefix(scope, case);
        },
    );
    let payload = result.expect_err("CREATE failure retains the complete original scope");
    if case == CreateCase::Panic {
        assert_eq!(payload.downcast_ref::<&str>(), Some(&"CREATE panic"));
    } else {
        let error = payload
            .downcast_ref::<ComputeAqlQueueSessionErrorV1>()
            .unwrap();
        let ComputeAqlQueueSessionErrorV1::TerminalCreation { stage, source } = error else {
            panic!("outer USERPTR boundary must remain terminal: {error:?}");
        };
        assert_eq!(
            *stage,
            if case == CreateCase::NoEffect {
                "USERPTR auxiliary queue-control creation"
            } else {
                "CREATE_QUEUE result"
            }
        );
        let message = if case == CreateCase::NoEffect {
            "queue syscall failed with no effect"
        } else if case.malformed() {
            "malformed queue kernel result"
        } else {
            "queue syscall result indeterminate"
        };
        assert!(
            matches!(&**source, ComputeAqlQueueSessionErrorV1::Native(actual) if *actual == message),
            "{error:?}"
        );
    }
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
    assert_eq!(
        engine
            .model
            .queues()
            .iter()
            .find(|r| r.plan.queue == primary.key)
            .copied(),
        primary_record
    );
    assert_eq!(&engine.model.history()[..history.len()], history);
    let appended = &engine.model.history()[history.len()..];
    let mut expected = vec![
        (Event::PlanAdmitted, Phase::Planned, Phase::Planned, None),
        (
            Event::CreateBegan,
            Phase::Planned,
            Phase::CreatePending,
            None,
        ),
    ];
    if case != CreateCase::Panic {
        expected.push((
            if case == CreateCase::NoEffect {
                Event::CreateFailedNoEffect
            } else {
                Event::CreateAmbiguous
            },
            Phase::CreatePending,
            case.phase(),
            case.queue_id().map(UntrustedQueueIdObservationV1),
        ));
    }
    let expected: Vec<_> = expected
        .into_iter()
        .enumerate()
        .map(|(index, (event, from, to, queue_id))| QueueHistoryEntryV1 {
            sequence: (history.len() + index + 1) as u64,
            queue: key,
            event,
            from,
            to,
            queue_id,
            configuration: record.configuration,
        })
        .collect();
    assert_eq!(
        appended, expected,
        "exact admitted CREATE history: {case:?}"
    );
    primary.runtime.assert_local_runtime(&gate, true);
    primary.shadows.assert_local_published();
    scope
        .construction
        .runtime
        .as_ref()
        .unwrap()
        .assert_local_runtime(&gate, false);
    scope
        .construction
        .published
        .as_ref()
        .unwrap()
        .assert_local_published();
    assert_eq!(gate.observation(), (true, true));
    assert_eq!(
        gate.runtime_observation(),
        LocalRuntimeObservationV1::Poisoned
    );
    let resources = resources.unwrap();
    assert_eq!(resources.live(), (2, 2, 2));
    if case != CreateCase::Panic {
        let returned = trace.borrow().create_returns[1];
        assert_eq!(
            returned.0,
            case.queue_id()
                .unwrap_or(fe2o3_runtime_model::CREATE_QUEUE_ID_SENTINEL_V1)
        );
        if matches!(
            case,
            CreateCase::NoEffect | CreateCase::Indeterminate | CreateCase::InvalidDoorbell
        ) {
            assert_eq!(returned.1, u64::MAX);
        }
    }
    drop(scope);
    assert_eq!(
        resources.live(),
        (0, 0, 0),
        "only fixture-owned resources disposed"
    );
    assert_eq!(gate.observation(), (false, true));
}

#[test]
fn same_engine_auxiliary_create_outcomes_retain_admitted_prefix_and_exact_history() {
    for external in [false, true] {
        for case in [
            CreateCase::NoEffect,
            CreateCase::Indeterminate,
            CreateCase::ReturnedIndeterminate,
            CreateCase::InputDrift,
            CreateCase::Panic,
        ] {
            exercise_create(case, external);
        }
    }
}

#[test]
fn same_engine_auxiliary_malformed_create_outputs_retain_admitted_prefix_and_exact_history() {
    for external in [false, true] {
        for case in [
            CreateCase::InvalidDoorbell,
            CreateCase::NoEffectChangedOutputs,
        ] {
            exercise_create(case, external);
        }
    }
}
