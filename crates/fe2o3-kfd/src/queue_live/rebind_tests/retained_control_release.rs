//! Real public forwarding and parent transport; no Linux engine or hardware completion.

use super::data_release::{poison_snapshot, selected_parent, snapshot};
use super::*;
use crate::queue::dispatch_binding::control_release::RetainedControlSnapshotV1 as OwnerSnapshot;
use crate::queue::dispatch_binding::preparation::{
    control_release_fixture_v1, recycle_and_detach_persistent_fixture_v1,
    single_persistent_control_in_memory_v1,
};
use crate::shared_memory::PreparationMemoryFixtureV1 as Memory;

fn install(
    session: &mut ComputeAqlQueueSessionV1,
    memory: &mut Memory,
) -> Vec<Gfx942FixedDispatchDataV1> {
    let mut owner = single_persistent_control_in_memory_v1(memory, session.key);
    let (generation, data) = recycle_and_detach_persistent_fixture_v1(&mut owner);
    assert!(session.dispatch.is_none());
    session.dispatch = Some(owner);
    session.detached_dispatch_generation = Some(generation);
    session.detached_data_count = 0;
    session.detached_data_identities.clear();
    session.detached_next_insertion_index = Some(0);
    data
}

fn owner_snapshot(session: &ComputeAqlQueueSessionV1, ordinal: usize) -> OwnerSnapshot {
    let (owner, generation) = if ordinal == 0 {
        (&session.dispatch, session.detached_dispatch_generation)
    } else {
        let state = session.auxiliary_compute_lanes[ordinal - 1]
            .state
            .as_ref()
            .unwrap();
        (&state.dispatch, state.detached_dispatch_generation)
    };
    assert!(owner.is_some(), "selected original dispatch stays owned");
    OwnerSnapshot::owner_v1(owner.as_ref().unwrap(), generation.unwrap())
}

fn data_snapshot(data: &[Gfx942FixedDispatchDataV1]) -> impl std::fmt::Debug + PartialEq + use<> {
    data.iter()
        .map(|d| {
            (
                d.storage_identity(),
                d.layout(),
                d.is_fully_initialized(),
                d.initialized_content(),
            )
        })
        .collect::<Vec<_>>()
}

fn attachment_snapshot(
    session: &ComputeAqlQueueSessionV1,
) -> impl std::fmt::Debug + PartialEq + use<> {
    (
        session.next_persistent_compute_generation,
        session.persistent_compute.as_ref().map(|attachment| {
            assert!(attachment.terminal_custody.is_none());
            (
                attachment.binding,
                attachment.predecessor_dispatch_generation,
                attachment
                    .entries
                    .iter()
                    .map(|entry| {
                        assert!(matches!(
                            entry.state,
                            PersistentComputeUseStateV1::Quarantined
                        ));
                        (
                            entry.storage_identity,
                            entry.authenticated_sha256,
                            entry.fully_initialized,
                            entry.effect,
                            entry.allocation.byte_len(),
                            entry.allocation.owner.live_use_count(),
                            entry.allocation.owner.quarantine_reason(),
                        )
                    })
                    .collect::<Vec<_>>(),
            )
        }),
    )
}

#[test]
fn retained_control_direct_missing_engine_transports_parent_before_returning_error() {
    let (mut session, lane) = selected_parent(0);
    let mut memory = Memory::new(true);
    let data = install(&mut session, &mut memory);
    let before = memory.observation();
    let outside = data_snapshot(&data);
    let _ = take_dispatch_terminal_process_gate_record_v1();
    let _ = take_lane_unwind_process_gate_record_v1();
    assert!(matches!(
        session.release_retained_persistent_fixed_dispatch_control_v1(),
        Err(ComputeAqlQueueSessionErrorV1::Contract(
            "missing queue engine"
        ))
    ));
    assert_shell(&mut session, lane);
    assert_eq!(memory.observation(), before);
    assert_eq!(data_snapshot(&data), outside);
    assert!(!take_dispatch_terminal_process_gate_record_v1());
    assert!(!take_lane_unwind_process_gate_record_v1());
}

#[test]
fn retained_control_facade_restores_exact_selected_owner_before_sticky_parent_transport() {
    for ordinal in 0..3 {
        for mode in 0..3 {
            let (mut session, lane) = selected_parent(ordinal);
            let mut memory = Memory::new(true);
            let mut data = None;
            session
                .with_compute_lane_custody_v1(
                    lane,
                    |selected| {
                        data = Some(install(selected.session, &mut memory));
                    },
                    |_| panic!("setup cannot transfer parent"),
                )
                .unwrap();
            let before = snapshot(&session);
            let owner = owner_snapshot(&session, ordinal);
            let memory_before = memory.observation();
            let outside = data_snapshot(data.as_ref().unwrap());
            let transfers = Cell::new(0);
            let retained = RefCell::new(None);
            let _ = take_dispatch_terminal_process_gate_record_v1();
            let _ = take_lane_unwind_process_gate_record_v1();
            let result = catch_unwind(AssertUnwindSafe(|| {
                session.with_compute_lane_custody_v1(
                    lane,
                    |selected| {
                        let error = selected
                            .release_retained_persistent_fixed_dispatch_control_v1()
                            .unwrap_err();
                        assert!(matches!(
                            error,
                            ComputeAqlQueueSessionErrorV1::Contract("missing queue engine")
                        ));
                        assert!(*selected.terminal_transport);
                        let retry = selected
                            .release_retained_persistent_fixed_dispatch_control_v1()
                            .unwrap_err();
                        assert!(matches!(
                            retry,
                            ComputeAqlQueueSessionErrorV1::DispatchBinding(
                                Gfx942DispatchBindingErrorV1::Poisoned
                            )
                        ));
                        assert!(
                            *selected.terminal_transport,
                            "retry cannot erase prior terminal transport"
                        );
                        match mode {
                            0 => Err(error),
                            1 => Ok(37),
                            _ => panic!("retained control caller panic"),
                        }
                    },
                    |root| {
                        transfers.set(transfers.get() + 1);
                        let parent = root.as_ref().as_ref().unwrap();
                        assert_eq!(
                            snapshot(parent),
                            before,
                            "restore selected lane before retaining parent"
                        );
                        owner.assert_restored_v1(owner_snapshot(parent, ordinal), true);
                        assert!(
                            parent.terminal_poisoned
                                && parent.completion_owner.is_poisoned_for_test()
                        );
                        for (index, slot) in parent.auxiliary_compute_lanes.iter().enumerate() {
                            if let Some(state) = &slot.state {
                                assert_eq!(
                                    state.completion_owner.is_poisoned_for_test(),
                                    index + 1 == ordinal
                                );
                            }
                        }
                        if ordinal == 2 {
                            assert!(parent.auxiliary_compute_lanes[0].state.is_none());
                        }
                        *retained.borrow_mut() = Some(root);
                    },
                )
            }));
            if mode == 2 {
                assert_eq!(
                    result.unwrap_err().downcast_ref::<&str>(),
                    Some(&"retained control caller panic")
                );
            } else {
                assert_eq!(result.unwrap().unwrap().is_err(), mode == 0);
            }
            assert_eq!(transfers.get(), 1);
            assert_shell(&mut session, lane);
            let retained = retained.into_inner().unwrap();
            assert_eq!(snapshot(retained.as_ref().as_ref().unwrap()), before);
            owner.assert_restored_v1(
                owner_snapshot(retained.as_ref().as_ref().unwrap(), ordinal),
                true,
            );
            assert_eq!(memory.observation(), memory_before);
            assert_eq!(data_snapshot(data.as_ref().unwrap()), outside);
            assert!(!take_dispatch_terminal_process_gate_record_v1());
            assert_eq!(take_lane_unwind_process_gate_record_v1(), mode == 2);
        }
    }
}

fn unpublished_fixture() -> (
    Memory,
    Vec<Gfx942FixedDispatchDataV1>,
    PristineDispatchContinuationV1,
) {
    let (mut memory, owner) = control_release_fixture_v1();
    let buffers = owner.prepare_pristine_abort_v1().unwrap();
    let mut abort = owner.begin_pristine_abort_v1(buffers);
    abort.release_controls(&mut memory).unwrap();
    let (continuation, data, _) = abort.into_detached();
    (memory, data, continuation)
}

#[test]
fn retained_control_public_preflight_preserves_exact_owner_and_does_not_request_transport() {
    for route in 0usize..4 {
        for mode in 0..4 {
            let (mut session, lane) = selected_parent(route.saturating_sub(1));
            let mut memory = Memory::new(true);
            let mut data = None;
            let mut unpublished = None;
            let saved = RefCell::new(None);
            let _ = take_dispatch_terminal_process_gate_record_v1();
            let _ = take_lane_unwind_process_gate_record_v1();
            let mut prepare = |selected: &mut ComputeAqlQueueSessionV1, memory: &mut Memory| {
                data = Some(install(selected, memory));
                match mode {
                    0 => selected.terminal_poisoned = true,
                    1 => {
                        let (memory, data, continuation) = unpublished_fixture();
                        selected.unpublished_dispatch.continuation = Some(continuation);
                        unpublished = Some((memory, data));
                    }
                    2 | 3 => {
                        let mut attached =
                            super::super::tests::persistent_compute_gate_test_session_v1(
                                selected.key,
                                if mode == 2 { 1 } else { 3 },
                            );
                        selected.persistent_compute = attached.persistent_compute.take();
                    }
                    _ => unreachable!(),
                }
                let before = snapshot(selected);
                let owner = owner_snapshot(selected, 0);
                let poison = poison_snapshot(selected);
                let memory_before = memory.observation();
                let unpublished_before = selected
                    .unpublished_dispatch
                    .continuation
                    .as_ref()
                    .map(|c| c.next_generation_for_test());
                *saved.borrow_mut() = Some((
                    before,
                    owner,
                    poison,
                    memory_before,
                    unpublished_before,
                    attachment_snapshot(selected),
                ));
            };
            let check = |selected: &ComputeAqlQueueSessionV1, memory: &Memory, error| {
                let saved = saved.borrow();
                let (before, owner, poison, memory_before, unpublished_before, attachment) =
                    saved.as_ref().unwrap();
                assert!(matches!(
                    (mode, error),
                    (
                        0,
                        ComputeAqlQueueSessionErrorV1::DispatchBinding(
                            Gfx942DispatchBindingErrorV1::Poisoned
                        )
                    ) | (
                        1..=3,
                        ComputeAqlQueueSessionErrorV1::DispatchBinding(
                            Gfx942DispatchBindingErrorV1::ResourcePhase
                        )
                    )
                ));
                assert_eq!(&snapshot(selected), before);
                assert_eq!(&owner_snapshot(selected, 0), owner);
                assert_eq!(&poison_snapshot(selected), poison);
                assert_eq!(&memory.observation(), memory_before);
                assert_eq!(&attachment_snapshot(selected), attachment);
                assert_eq!(
                    &selected
                        .unpublished_dispatch
                        .continuation
                        .as_ref()
                        .map(|c| c.next_generation_for_test()),
                    unpublished_before
                );
            };
            if route == 0 {
                prepare(&mut session, &mut memory);
                let error = session
                    .release_retained_persistent_fixed_dispatch_control_v1()
                    .unwrap_err();
                check(&session, &memory, error);
            } else {
                session
                    .with_compute_lane_custody_v1(
                        lane,
                        |selected| {
                            prepare(selected.session, &mut memory);
                            let error = selected
                                .release_retained_persistent_fixed_dispatch_control_v1()
                                .unwrap_err();
                            check(selected.session, &memory, error);
                            assert!(!*selected.terminal_transport);
                        },
                        |_| panic!("healthy preflight cannot transfer parent"),
                    )
                    .unwrap();
            }
            let saved = saved.borrow();
            let (_, owner, _, _, _, attachment) = saved.as_ref().unwrap();
            assert_eq!(
                &owner_snapshot(&session, route.saturating_sub(1)),
                owner,
                "healthy rejection restores the owner to the stable selected slot"
            );
            assert_eq!(&attachment_snapshot(&session), attachment);
            assert!(!take_dispatch_terminal_process_gate_record_v1());
            assert!(!take_lane_unwind_process_gate_record_v1());
            assert!(data.is_some());
        }
    }
}

#[test]
fn retained_control_absent_and_ordinary_owners_return_false_without_engine_access() {
    for route in 0usize..4 {
        for ordinary in [false, true] {
            let (mut session, lane) = selected_parent(route.saturating_sub(1));
            let (memory, owner) = control_release_fixture_v1();
            let before = memory.observation();
            let mut owner = Some(owner);
            let _ = take_dispatch_terminal_process_gate_record_v1();
            let _ = take_lane_unwind_process_gate_record_v1();
            let saved = RefCell::new(None);
            let mut prepare = |selected: &mut ComputeAqlQueueSessionV1| {
                if ordinary {
                    selected.dispatch = owner.take();
                }
                let dispatch = selected
                    .dispatch
                    .as_ref()
                    .map(|d| OwnerSnapshot::owner_v1(d, 0));
                let before = snapshot(selected);
                let poison = poison_snapshot(selected);
                *saved.borrow_mut() = Some((before, poison, dispatch));
            };
            let check = |selected: &ComputeAqlQueueSessionV1| {
                let saved = saved.borrow();
                let (before, poison, dispatch) = saved.as_ref().unwrap();
                assert_eq!(&snapshot(selected), before);
                assert_eq!(&poison_snapshot(selected), poison);
                assert_eq!(
                    &selected
                        .dispatch
                        .as_ref()
                        .map(|d| OwnerSnapshot::owner_v1(d, 0)),
                    dispatch
                );
            };
            if route == 0 {
                prepare(&mut session);
                for _ in 0..2 {
                    assert!(
                        !session
                            .release_retained_persistent_fixed_dispatch_control_v1()
                            .unwrap()
                    );
                }
                check(&session);
            } else {
                session
                    .with_compute_lane_custody_v1(
                        lane,
                        |selected| {
                            prepare(selected.session);
                            for _ in 0..2 {
                                assert!(
                                    !selected
                                        .release_retained_persistent_fixed_dispatch_control_v1()
                                        .unwrap()
                                );
                            }
                            check(selected.session);
                            assert!(!*selected.terminal_transport);
                        },
                        |_| panic!("false result cannot transfer parent"),
                    )
                    .unwrap();
            }
            let ordinal = route.saturating_sub(1);
            let restored = if ordinal == 0 {
                &session.dispatch
            } else {
                &session.auxiliary_compute_lanes[ordinal - 1]
                    .state
                    .as_ref()
                    .unwrap()
                    .dispatch
            };
            assert_eq!(
                &restored.as_ref().map(|d| OwnerSnapshot::owner_v1(d, 0)),
                &saved.borrow().as_ref().unwrap().2,
                "false result restores exact owner at stable selected slot"
            );
            assert_eq!(memory.observation(), before);
            assert!(!take_dispatch_terminal_process_gate_record_v1());
            assert!(!take_lane_unwind_process_gate_record_v1());
        }
    }
}
