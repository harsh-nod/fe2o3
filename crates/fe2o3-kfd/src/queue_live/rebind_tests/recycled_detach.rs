//! Public routing and complete parent restoration, with no native queue engine.

use super::data_release::{poison_snapshot, selected_parent, snapshot};
use super::retained_control_release::{attachment_snapshot, unpublished_fixture};
use super::*;
use crate::queue::dispatch_binding::control_release::RetainedControlSnapshotV1 as OwnerSnapshot;
use crate::queue::dispatch_binding::preparation::ordinary_recycled_in_memory_v1;
use crate::shared_memory::PreparationMemoryFixtureV1 as Memory;

fn install(session: &mut ComputeAqlQueueSessionV1, memory: &mut Memory) {
    let (owner, _, _) = ordinary_recycled_in_memory_v1(memory, session.key, 1);
    assert!(session.dispatch.is_none());
    session.dispatch = Some(owner);
    clear_detached_ledger(session);
}

fn clear_detached_ledger(session: &mut ComputeAqlQueueSessionV1) {
    session.detached_dispatch_generation = None;
    session.detached_data_count = 0;
    session.detached_data_identities.clear();
    session.detached_next_insertion_index = None;
}

fn owner_snapshot(session: &ComputeAqlQueueSessionV1, ordinal: usize) -> OwnerSnapshot {
    let dispatch = if ordinal == 0 {
        &session.dispatch
    } else {
        &session.auxiliary_compute_lanes[ordinal - 1]
            .state
            .as_ref()
            .unwrap()
            .dispatch
    };
    OwnerSnapshot::recycled_owner_v1(dispatch.as_ref().unwrap())
}

#[test]
fn recycled_detach_direct_missing_engine_transports_parent_before_error() {
    let (mut session, lane) = selected_parent(0);
    let mut memory = Memory::new(true);
    install(&mut session, &mut memory);
    let before = memory.observation();
    let _ = take_dispatch_terminal_process_gate_record_v1();
    let _ = take_lane_unwind_process_gate_record_v1();
    assert!(matches!(
        session.detach_recycled_fixed_dispatch(),
        Err(ComputeAqlQueueSessionErrorV1::Contract(
            "missing queue engine"
        ))
    ));
    assert_shell(&mut session, lane);
    assert_eq!(memory.observation(), before);
    assert!(!take_dispatch_terminal_process_gate_record_v1());
    assert!(!take_lane_unwind_process_gate_record_v1());
}

#[test]
fn recycled_detach_facade_restores_selected_owner_and_keeps_transport_sticky() {
    for ordinal in 0..3 {
        for mode in 0..3 {
            let (mut session, lane) = selected_parent(ordinal);
            let mut memory = Memory::new(true);
            session
                .with_compute_lane_custody_v1(
                    lane,
                    |selected| install(selected.session, &mut memory),
                    |_| panic!("setup cannot transfer parent"),
                )
                .unwrap();
            let before = snapshot(&session);
            let owner = owner_snapshot(&session, ordinal);
            let memory_before = memory.observation();
            let transfers = Cell::new(0);
            let retained = RefCell::new(None);
            let _ = take_dispatch_terminal_process_gate_record_v1();
            let _ = take_lane_unwind_process_gate_record_v1();
            let result = catch_unwind(AssertUnwindSafe(|| {
                session.with_compute_lane_custody_v1(
                    lane,
                    |selected| {
                        let error = selected.detach_recycled_fixed_dispatch().err().unwrap();
                        assert!(matches!(
                            error,
                            ComputeAqlQueueSessionErrorV1::Contract("missing queue engine")
                        ));
                        assert!(*selected.terminal_transport);
                        assert!(matches!(
                            selected.detach_recycled_fixed_dispatch(),
                            Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                                Gfx942DispatchBindingErrorV1::Poisoned
                            ))
                        ));
                        assert!(*selected.terminal_transport);
                        match mode {
                            0 => Err(error),
                            1 => Ok(37),
                            _ => panic!("recycled detach caller panic"),
                        }
                    },
                    |root| {
                        transfers.set(transfers.get() + 1);
                        let parent = root.as_ref().as_ref().unwrap();
                        assert_eq!(snapshot(parent), before);
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
                        *retained.borrow_mut() = Some(root);
                    },
                )
            }));
            if mode == 2 {
                assert_eq!(
                    result.unwrap_err().downcast_ref::<&str>(),
                    Some(&"recycled detach caller panic")
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
            assert!(!take_dispatch_terminal_process_gate_record_v1());
            assert_eq!(take_lane_unwind_process_gate_record_v1(), mode == 2);
        }
    }
}

#[test]
fn recycled_detach_public_preflight_rejection_preserves_owner_without_transport() {
    for route in 0usize..4 {
        for mode in 0..6 {
            let (mut session, lane) = selected_parent(route.saturating_sub(1));
            let mut memory = Memory::new(true);
            let saved = RefCell::new(None);
            let mut unpublished = None;
            let _ = take_dispatch_terminal_process_gate_record_v1();
            let _ = take_lane_unwind_process_gate_record_v1();
            let mut prepare = |selected: &mut ComputeAqlQueueSessionV1, memory: &mut Memory| {
                clear_detached_ledger(selected);
                if mode != 5 {
                    install(selected, memory);
                }
                if mode <= 3 {
                    // These guards must win over both a dirty ledger and active completion.
                    selected.detached_data_count = 17;
                    selected.completion_owner.bind_barrier_probe().unwrap();
                }
                match mode {
                    0 | 2 | 3 => {
                        let mut attached =
                            super::super::tests::persistent_compute_gate_test_session_v1(
                                selected.key,
                                if mode == 3 { 3 } else { 1 },
                            );
                        selected.persistent_compute = attached.persistent_compute.take();
                        if mode == 0 {
                            selected.terminal_poisoned = true;
                        }
                    }
                    1 => {
                        let (memory, data, continuation) = unpublished_fixture();
                        selected.unpublished_dispatch.continuation = Some(continuation);
                        unpublished = Some((memory, data));
                    }
                    4 => {
                        selected.completion_owner.bind_barrier_probe().unwrap();
                    }
                    5 => {}
                    _ => unreachable!(),
                }
                *saved.borrow_mut() = Some((
                    snapshot(selected),
                    selected
                        .dispatch
                        .as_ref()
                        .map(OwnerSnapshot::recycled_owner_v1),
                    poison_snapshot(selected),
                    memory.observation(),
                    attachment_snapshot(selected),
                    selected
                        .unpublished_dispatch
                        .continuation
                        .as_ref()
                        .map(|c| c.next_generation_for_test()),
                ));
            };
            let check = |selected: &ComputeAqlQueueSessionV1, memory: &Memory, error| {
                assert!(matches!(
                    (mode, error),
                    (
                        0,
                        ComputeAqlQueueSessionErrorV1::DispatchBinding(
                            Gfx942DispatchBindingErrorV1::Poisoned
                        )
                    ) | (
                        1..=3 | 5,
                        ComputeAqlQueueSessionErrorV1::DispatchBinding(
                            Gfx942DispatchBindingErrorV1::ResourcePhase
                        )
                    ) | (
                        4,
                        ComputeAqlQueueSessionErrorV1::Completion(
                            Gfx942CompletionErrorV1::Poisoned
                        )
                    )
                ));
                let saved = saved.borrow();
                let (before, owner, poison, memory_before, attachment, continuation) =
                    saved.as_ref().unwrap();
                assert_eq!(&snapshot(selected), before);
                assert_eq!(
                    &selected
                        .dispatch
                        .as_ref()
                        .map(OwnerSnapshot::recycled_owner_v1),
                    owner
                );
                assert_eq!(&poison_snapshot(selected), poison);
                assert_eq!(&memory.observation(), memory_before);
                assert_eq!(&attachment_snapshot(selected), attachment);
                assert_eq!(
                    &selected
                        .unpublished_dispatch
                        .continuation
                        .as_ref()
                        .map(|c| c.next_generation_for_test()),
                    continuation
                );
            };
            if route == 0 {
                prepare(&mut session, &mut memory);
                let error = session.detach_recycled_fixed_dispatch().err().unwrap();
                check(&session, &memory, error);
            } else {
                session
                    .with_compute_lane_custody_v1(
                        lane,
                        |selected| {
                            prepare(selected.session, &mut memory);
                            let error = selected.detach_recycled_fixed_dispatch().err().unwrap();
                            check(selected.session, &memory, error);
                            assert!(!*selected.terminal_transport);
                        },
                        |_| panic!("preflight cannot transfer"),
                    )
                    .unwrap();
            }
            let saved = saved.borrow();
            let (_, owner, _, _, attachment, _) = saved.as_ref().unwrap();
            let selected = if route <= 1 {
                &session.dispatch
            } else {
                &session.auxiliary_compute_lanes[route - 2]
                    .state
                    .as_ref()
                    .unwrap()
                    .dispatch
            };
            assert_eq!(
                &selected.as_ref().map(OwnerSnapshot::recycled_owner_v1),
                owner
            );
            assert_eq!(&attachment_snapshot(&session), attachment);
            assert!(!take_dispatch_terminal_process_gate_record_v1());
            assert!(!take_lane_unwind_process_gate_record_v1());
        }
    }
}

#[test]
fn recycled_detach_production_routing_roots_before_loan_and_commits_after_retake() {
    let source = include_str!("../recycled_detach.rs");
    let settle = source
        .split("pub(in crate::queue) fn settle_recycled_detach_v1")
        .nth(1)
        .unwrap();
    let reserve = settle
        .find("root.reserve_output(count, capacities)?")
        .unwrap();
    let currentness = settle.find("context.check_currentness()?").unwrap();
    let take = settle.find("original = context.dispatch().take()").unwrap();
    let loan = settle.find("context.with_memory_custody").unwrap();
    let retake = settle.find("retake?;").unwrap();
    let convert = settle
        .find("root.convert_completed(generation, count)?")
        .unwrap();
    let commit = settle.find("*ledger.identities =").unwrap();
    assert!(
        reserve < currentness
            && currentness < take
            && take < loan
            && loan < retake
            && retake < convert
            && convert < commit
    );
    assert!(
        settle.find("let mut root =").unwrap() < settle.find("let result = catch_unwind").unwrap()
    );
    assert!(
        settle.find("context.retain(root)").unwrap()
            < settle.find("context.poison(result.is_err())").unwrap()
    );
    let commit = settle[commit..].split("    }));").next().unwrap();
    for forbidden in ["?", "push(", "reserve", "context."] {
        assert!(!commit.contains(forbidden));
    }
    let facade = include_str!("../../queue_live.rs")
        .split("impl ComputeAqlQueueLaneDispatchV1<'_>")
        .nth(1)
        .unwrap()
        .split("pub fn abort_unpublished_fixed_dispatch_v1")
        .next()
        .unwrap();
    assert!(
        facade
            .find("*self.terminal_transport |= settled.transport")
            .unwrap()
            < facade.find("settled.into_result()").unwrap()
    );
}
