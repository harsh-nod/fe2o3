use super::*;
use crate::queue_linux::primary_fixture::LocalRuntimeObservationV1;
use crate::sdma::{
    Gfx942SdmaQueueObservationV1, id_only_sdma_owner_for_auxiliary_construction_test_v1,
};
use fe2o3_runtime_model::{
    ComputeAqlQueuePhaseV1 as Phase, QueueHistoryEntryV1, QueueHistoryEventKindV1 as Event,
    UntrustedQueueIdObservationV1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Case {
    Reuse,
    AuxiliaryCollision,
    DirectionalCollision,
    StripedCollision,
    ChangedIndex,
    ChangedGeneration,
    ChangedKind,
    UnreservedAppend,
}

impl Case {
    fn collision(self) -> bool {
        matches!(
            self,
            Self::AuxiliaryCollision | Self::DirectionalCollision | Self::StripedCollision
        )
    }
}

#[derive(Debug, Eq, PartialEq)]
struct SlotSnapshot {
    generation: u64,
    state: Option<(QueueKeyV1, ComputeAqlQueueObservationV1, usize)>,
}

fn slots(scope: &Scope) -> Vec<SlotSnapshot> {
    scope
        .lanes
        .iter()
        .map(|slot| SlotSnapshot {
            generation: slot.generation,
            state: slot
                .state
                .as_ref()
                .map(|lane| (lane.key, lane.observation, lane as *const _ as usize)),
        })
        .collect()
}

#[derive(Debug, Eq, PartialEq)]
struct SdmaSnapshot {
    striped_cursor: Option<usize>,
    storage: usize,
    capacity: usize,
    observations: Vec<Gfx942SdmaQueueObservationV1>,
}

fn sdma_snapshot(roster: &Option<Gfx942SdmaQueueSetV1>) -> Option<SdmaSnapshot> {
    roster.as_ref().map(|roster| {
        let (owners, striped_cursor) = match roster {
            Gfx942SdmaQueueSetV1::Directional(owners) => (owners, None),
            Gfx942SdmaQueueSetV1::Striped { owners, next_owner } => (owners, Some(*next_owner)),
            _ => panic!("only explicit directional/striped ID observations"),
        };
        SdmaSnapshot {
            striped_cursor,
            storage: owners.as_ptr() as usize,
            capacity: owners.capacity(),
            observations: owners.iter().map(|owner| owner.observation()).collect(),
        }
    })
}

fn id_only_sdma_roster(key: QueueKeyV1, ids: [u32; 2], striped: bool) -> Gfx942SdmaQueueSetV1 {
    let owners = ids
        .into_iter()
        .enumerate()
        .map(|(index, id)| {
            id_only_sdma_owner_for_auxiliary_construction_test_v1(key, id, index as u32)
        })
        .collect();
    if striped {
        Gfx942SdmaQueueSetV1::Striped {
            owners,
            next_owner: 0,
        }
    } else {
        Gfx942SdmaQueueSetV1::Directional(owners)
    }
}

// Deliberately inconsistent roster metadata, not a second constructed auxiliary.
fn id_only_auxiliary(key: QueueKeyV1) -> ComputeAqlQueueLaneStateV1<Fixture> {
    ComputeAqlQueueLaneStateV1 {
        key,
        doorbell: None,
        submission: None,
        completion_signals: None,
        completion_owner: QueueOwnerSlotV1(None),
        dispatch: None,
        unpublished_dispatch: UnpublishedDispatchStateV1::default(),
        detached_data_count: 0,
        detached_dispatch_generation: None,
        detached_data_identities: Vec::new(),
        detached_next_insertion_index: None,
        exception: None,
        observation: ComputeAqlQueueObservationV1 {
            queue_id: 8,
            ring_bytes: 0,
            doorbell_slice_bytes: 0,
            doorbell_byte_offset: 0,
            event_id: 0,
            cwsr_shadow_pages: 0,
        },
    }
}

fn assert_preflight_rejection(scope: &Scope, gate: &LocalGateV1, expected: &'static str) {
    let primary = scope.primary.completed.as_ref().unwrap();
    let engine = &primary.engine;
    let memory = engine.backend.session.observation();
    let loan = engine
        .backend
        .session
        .primary_loan_state_v1(&engine.foundation);
    let records = engine.model.queues().to_vec();
    let history = engine.model.history().to_vec();
    let poisoned = engine.authority_poisoned;
    let completion = primary.completion_owner.custody_snapshot_for_test();
    let dependency = primary.dependency_owner.custody_snapshot_for_test();
    let platform = platform::platform_identities(&scope.primary, None);
    let calls = trace().borrow().calls.clone();
    let metadata = slots(scope);
    let storage = (
        scope.lanes.as_ptr(),
        scope.lanes.len(),
        scope.lanes.capacity(),
    );
    let rosters = (
        sdma_snapshot(&scope.sdma),
        sdma_snapshot(&scope.striped_sdma),
    );
    let gate_before = (gate.observation(), gate.runtime_observation());
    let resources = trace().borrow().local_resources.clone();
    let live = resources.live();
    assert!(matches!(
        prepare_auxiliary_compute_lane_slot_v1(&scope.lanes),
        Err(ComputeAqlQueueSessionErrorV1::Contract(actual)) if actual == expected
    ));
    assert_eq!(engine.backend.session.observation(), memory);
    assert_eq!(
        engine
            .backend
            .session
            .primary_loan_state_v1(&engine.foundation),
        loan
    );
    assert_eq!(engine.model.queues(), records);
    assert_eq!(engine.model.history(), history);
    assert_eq!(engine.authority_poisoned, poisoned);
    assert_eq!(
        primary.completion_owner.custody_snapshot_for_test(),
        completion
    );
    assert_eq!(
        primary.dependency_owner.custody_snapshot_for_test(),
        dependency
    );
    assert_eq!(
        platform::platform_identities(&scope.primary, None),
        platform
    );
    assert_eq!(trace().borrow().calls, calls);
    assert_eq!(slots(scope), metadata);
    assert_eq!(
        (
            scope.lanes.as_ptr(),
            scope.lanes.len(),
            scope.lanes.capacity()
        ),
        storage
    );
    assert_eq!(
        (
            sdma_snapshot(&scope.sdma),
            sdma_snapshot(&scope.striped_sdma)
        ),
        rosters
    );
    assert_eq!(
        (gate.observation(), gate.runtime_observation()),
        gate_before
    );
    assert_eq!(resources.live(), live);
    assert_parent_transport(scope, false);
}

fn exercise(case: Case, external: bool) {
    let gate = LocalGateV1::new();
    let mut resources = None;
    let mut before = None;
    let mut original_records = Vec::new();
    let mut history = Vec::new();
    let mut storage = None;
    let mut metadata = Vec::new();
    let mut rosters = None;
    let (scope, result, trace) = prefix_case_with_runner(
        external,
        |trace| {
            trace.borrow_mut().local_gate = Some(gate.clone());
            resources = Some(trace.borrow().local_resources.clone());
        },
        |scope, _| {
            let engine = &scope.primary.completed.as_ref().unwrap().engine;
            before = Some(engine.backend.session.observation());
            original_records = engine.model.queues().to_vec();
            history = engine.model.history().to_vec();
        },
        |mut scope, programs| {
            let original = scope.parent.original.as_mut().unwrap();
            let key = original.primary.completed.as_ref().unwrap().key;
            match case {
                Case::Reuse => {
                    original.sdma = Some(id_only_sdma_roster(key, [40, 41], false));
                    original.striped_sdma = Some(id_only_sdma_roster(key, [50, 51], true));
                    original.lanes.push(AuxiliaryComputeLaneSlotV1 {
                        generation: u64::MAX,
                        state: None,
                    });
                }
                Case::DirectionalCollision => {
                    original.sdma = Some(id_only_sdma_roster(key, [40, 8], false));
                }
                Case::StripedCollision => {
                    original.sdma = Some(id_only_sdma_roster(key, [40, 41], false));
                    original.striped_sdma = Some(id_only_sdma_roster(key, [50, 8], true));
                }
                Case::ChangedGeneration => {
                    original.lanes.push(AuxiliaryComputeLaneSlotV1 {
                        generation: 4,
                        state: None,
                    });
                }
                Case::UnreservedAppend => original.lanes = Vec::new(),
                _ => {}
            }
            if case == Case::Reuse {
                assert_preflight_rejection(
                    &scope,
                    &gate,
                    "compute queue lane generation exhausted",
                );
                scope.parent.original.as_mut().unwrap().lanes[0].generation = 4;
            }
            let mut slot = prepare_auxiliary_compute_lane_slot_v1(&scope.lanes).unwrap();
            match case {
                Case::AuxiliaryCollision => {
                    scope.parent.original.as_mut().unwrap().lanes.push(
                        AuxiliaryComputeLaneSlotV1 {
                            generation: 4,
                            state: Some(id_only_auxiliary(key)),
                        },
                    );
                }
                Case::ChangedIndex => slot.index += 1,
                Case::ChangedGeneration => slot.generation += 1,
                Case::ChangedKind => slot.append = !slot.append,
                _ => {}
            }
            storage = Some((
                scope.lanes.as_ptr(),
                scope.lanes.len(),
                scope.lanes.capacity(),
            ));
            metadata = slots(&scope);
            rosters = Some((
                sdma_snapshot(&scope.sdma),
                sdma_snapshot(&scope.striped_sdma),
            ));
            let (scope, result) = run_auxiliary_with_slot(scope, programs, slot);
            if case == Case::Reuse && result.is_ok() {
                assert_preflight_rejection(&scope, &gate, "compute queue lane capacity exhausted");
            }
            (scope, result)
        },
        |scope, failed| {
            assert_eq!(failed, case != Case::Reuse, "{case:?}, external={external}");
            let installed = (!failed).then(|| scope.lanes[0].state.as_ref().unwrap());
            assert_pair_with_candidate(scope, installed);
        },
    );
    if case != Case::Reuse {
        let payload = result.expect_err("selected roster/slot rejection");
        let error = payload
            .downcast_ref::<ComputeAqlQueueSessionErrorV1>()
            .unwrap();
        let ComputeAqlQueueSessionErrorV1::TerminalCreation { stage, source } = error else {
            panic!("terminal creation error: {error:?}");
        };
        assert_eq!(
            *stage,
            if case.collision() {
                "auxiliary compute queue ID admission"
            } else {
                "USERPTR auxiliary queue-control creation"
            }
        );
        let expected = if case.collision() {
            "auxiliary compute queue ID collides with a session-owned queue"
        } else if case == Case::UnreservedAppend {
            "compute queue lane destination not reserved"
        } else {
            "compute queue lane destination changed"
        };
        assert!(
            matches!(&**source, ComputeAqlQueueSessionErrorV1::Contract(actual) if *actual == expected)
        );
    }
    let primary = scope.primary.completed.as_ref().unwrap();
    let engine = &primary.engine;
    let root = &scope.construction;
    let key = root.key.unwrap();
    assert_eq!(
        &engine.model.queues()[..original_records.len()],
        original_records
    );
    assert_eq!(engine.model.queues().len(), original_records.len() + 1);
    let record = engine.model.queues().last().unwrap();
    assert_eq!(record.plan.queue, key);
    assert_eq!(record.phase, Phase::Active);
    assert_eq!(record.queue_id, Some(UntrustedQueueIdObservationV1(8)));
    assert_eq!(record.configuration, record.plan.initial_configuration);
    assert!(record.pending_configuration.is_none());
    assert!(!engine.authority_poisoned);
    assert_eq!(&engine.model.history()[..history.len()], history);
    let suffix: Vec<_> = [
        (Event::PlanAdmitted, Phase::Planned, Phase::Planned, None),
        (
            Event::CreateBegan,
            Phase::Planned,
            Phase::CreatePending,
            None,
        ),
        (
            Event::CreateSucceeded,
            Phase::CreatePending,
            Phase::Active,
            Some(UntrustedQueueIdObservationV1(8)),
        ),
    ]
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
    assert_eq!(&engine.model.history()[history.len()..], suffix);
    assert_eq!(engine.native_queue_id(key), Some(8));
    let outputs = engine.create_outputs(key).unwrap();
    assert_eq!(root.outputs, Some(outputs));
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
    assert!(root.creation_arm.is_some());
    assert!(root.unpublished.is_none());
    assert_eq!(
        root.completed.is_some(),
        !case.collision() && case != Case::Reuse
    );
    assert_eq!(root.dispatch.is_some(), case.collision());
    assert_eq!(root.completion.retained.is_some(), case.collision());
    assert_eq!(root.completion_owner.is_some(), case.collision());
    assert_eq!(root.submission.is_some(), case.collision());
    assert_eq!(root.runtime.is_some(), case.collision());
    assert_eq!(root.event.is_some(), case.collision());
    assert_eq!(root.published.is_some(), case.collision());
    let installed = (case == Case::Reuse).then(|| scope.lanes[0].state.as_ref().unwrap());
    if let Some(lane) = installed.or(root.completed.as_ref()) {
        assert_eq!(lane.key, key);
        assert!(lane.doorbell.is_some());
        assert_eq!(lane.observation.queue_id, 8);
        assert_eq!(lane.observation.ring_bytes, 4096);
        assert_eq!(
            lane.observation.doorbell_slice_bytes,
            fe2o3_kfd_uapi::KFD_GFX942_PROCESS_DOORBELL_SLICE_BYTES as usize
        );
        assert_eq!(
            lane.observation.doorbell_byte_offset,
            outputs.doorbell_offset().in_process_byte_offset()
        );
        let exception = lane.exception.as_ref().unwrap();
        exception.runtime.assert_local_runtime(&gate, true);
        exception.shadows.assert_local_published();
    } else {
        root.runtime
            .as_ref()
            .unwrap()
            .assert_local_runtime(&gate, true);
        root.published.as_ref().unwrap().assert_local_published();
    }
    primary.runtime.assert_local_runtime(&gate, true);
    primary.shadows.assert_local_published();
    let before = before.unwrap();
    let after = engine.backend.session.observation();
    assert_eq!(
        after.device.unwrap().used_backing_bytes - before.device.unwrap().used_backing_bytes,
        8192
    );
    assert_eq!(
        after.device.unwrap().used_allocation_records
            - before.device.unwrap().used_allocation_records,
        2
    );
    assert_eq!(
        after.host.unwrap().used_backing_bytes - before.host.unwrap().used_backing_bytes,
        8192 + COMPLETION_SIGNAL_ARENA_BYTES_V1 as u64
    );
    assert_eq!(
        after.host.unwrap().used_allocation_records - before.host.unwrap().used_allocation_records,
        3
    );
    assert_eq!(
        (
            scope.lanes.as_ptr(),
            scope.lanes.len(),
            scope.lanes.capacity()
        ),
        storage.unwrap()
    );
    assert_eq!(
        (
            sdma_snapshot(&scope.sdma),
            sdma_snapshot(&scope.striped_sdma)
        ),
        rosters.unwrap()
    );
    if case == Case::Reuse {
        assert_eq!(metadata.len(), 1);
        assert_eq!(metadata[0].generation, 4);
        assert!(metadata[0].state.is_none());
        assert_eq!(scope.lanes[0].generation, 5);
        for generation in [4, 5] {
            let admission = admit_compute_lane_v1(
                primary.key,
                &scope.lanes,
                ComputeAqlQueueLaneV1 {
                    session: primary.key,
                    ordinal: 1,
                    generation,
                },
            );
            if generation == 5 {
                assert_eq!(admission.unwrap(), AdmittedComputeLaneV1::Auxiliary(0));
            } else {
                assert!(matches!(
                    admission,
                    Err(ComputeAqlQueueSessionErrorV1::Contract(
                        "stale compute queue lane"
                    ))
                ));
            }
        }
        assert_eq!(gate.observation(), (false, false));
        assert_eq!(
            gate.runtime_observation(),
            LocalRuntimeObservationV1::Enabled {
                opener_pid: std::process::id(),
                leases: 2
            }
        );
    } else {
        assert_eq!(slots(&scope), metadata);
        assert_eq!(gate.observation(), (true, true));
        assert_eq!(
            gate.runtime_observation(),
            LocalRuntimeObservationV1::Poisoned
        );
    }
    let t = trace.borrow();
    assert!(!t.create_collision);
    for (callback, count) in [
        ("create", 2),
        ("publish", 2),
        ("runtime-created", 2),
        ("recover-outputs", 2),
        ("recover-id", 2),
        ("event-id", 1 + usize::from(!case.collision())),
        ("doorbell", 1 + usize::from(!case.collision())),
        ("doorbell-observe", 1 + usize::from(!case.collision())),
        ("gate-finish", 1 + usize::from(case == Case::Reuse)),
    ] {
        assert_eq!(
            t.calls.iter().filter(|&&c| c == callback).count(),
            count,
            "{case:?}: {callback}"
        );
    }
    if case == Case::Reuse {
        assert_eq!(t.calls.last(), Some(&"gate-finish"));
    } else {
        let last = if case.collision() {
            "recover-id"
        } else {
            "currentness"
        };
        let position = t.calls.iter().rposition(|&c| c == last).unwrap();
        assert_eq!(
            &t.calls[position + 1..],
            ["poison", "auxiliary-parent-retain", "retain-auxiliary-root"]
        );
    }
    assert_eq!(t.poison, case != Case::Reuse);
    assert_eq!(t.cleanup, 0);
    assert_eq!(t.drops, 0);
    drop(t);
    let resources = resources.unwrap();
    assert_eq!(resources.live(), (2, 2, 2));
    drop(scope);
    assert_eq!(
        resources.live(),
        (0, 0, 0),
        "only fixture-owned resources disposed"
    );
    assert_eq!(gate.observation(), (false, true));
}

#[test]
fn same_engine_auxiliary_retained_roster_collisions_keep_exact_unassembled_owners() {
    for external in [false, true] {
        for case in [
            Case::AuxiliaryCollision,
            Case::DirectionalCollision,
            Case::StripedCollision,
        ] {
            exercise(case, external);
        }
    }
}

#[test]
fn same_engine_auxiliary_slot_reuse_and_destination_rejections_keep_exact_owners() {
    for external in [false, true] {
        for case in [
            Case::Reuse,
            Case::ChangedIndex,
            Case::ChangedGeneration,
            Case::ChangedKind,
            Case::UnreservedAppend,
        ] {
            exercise(case, external);
        }
    }
}
