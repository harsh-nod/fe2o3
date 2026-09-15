use super::super::global_enum_transport_v1::tests::{build_owner, initial};
use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

// These are observer/transfer component inputs, not authenticated source loans.
fn value() -> ProjectedCapabilityValueV1 {
    initial()[&2].clone()
}

fn record(state: &ProjectedCapabilityStateV1) -> Before {
    let mut before = Before::new(
        "Copy",
        0,
        Some(0),
        SemanticSourceProvenanceV1::unavailable(),
    )
    .unwrap();
    before.local(2, state).unwrap();
    before
}

fn records() -> usize {
    TRACE.with(|slot| slot.borrow().as_ref().unwrap().records)
}

#[test]
fn pipeline_payload_replay_refreshes_phase_function_and_visit() {
    let owner = build_owner();
    let function = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap()
        .body();
    observe(|| {
        for (phase, visit) in [
            ("propagation", 1),
            ("pipeline-payload-replay", 2),
            ("pipeline-payload-replay", 3),
            ("final-replay", 4),
        ] {
            let previous_work = TRACE.with(|slot| slot.borrow().as_ref().unwrap().work);
            begin_block(function, phase);
            TRACE.with(|slot| {
                let slot = slot.borrow();
                let trace = slot.as_ref().unwrap();
                assert_eq!(trace.phase, phase);
                assert_eq!(trace.function, Some(function.identity()));
                assert_eq!(trace.visit, visit);
                assert_eq!(trace.work, previous_work + 1);
                assert_eq!(trace.records, 0);
            });
        }
    });
}

#[test]
fn disabled_receiver_observation_does_not_start_or_change_state() {
    let state = initial();
    assert!(Before::new("Copy", 0, None, SemanticSourceProvenanceV1::unavailable()).is_none());
    assert_eq!(state, initial());
    after(None, None, &state);
    assert!(TRACE.with(|slot| slot.borrow().is_none()));
}

#[test]
fn observed_consumption_keeps_the_original_invalid_state() {
    observe(|| {
        let mut state = initial();
        let before = record(&state);
        let mut expected = state.clone();
        invalidate_capability_local_v1(&mut expected, 2);
        invalidate_capability_local_v1(&mut state, 2);
        after(Some(before), None, &state);
        assert_eq!(state, expected);
        assert_eq!(records(), 1);
        assert_eq!(fact(state.get(&2)), Fact::Invalid);
    });
}

#[test]
fn enabled_real_statement_transfer_matches_unobserved_transfer() {
    let owner = build_owner();
    let function = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap()
        .body();
    let types = owner.source_semantic().types();
    let dominance = SemanticEnumPayloadDominanceV1::analyze(function, types).unwrap();
    let mut expected = initial();
    transfer_capability_statements_with_transport_v1(
        types,
        function,
        0,
        &mut expected,
        &dominance,
        None,
        None,
        None,
    )
    .unwrap();
    let mut actual = initial();
    observe(|| {
        begin_block(function, "propagation");
        transfer_capability_statements_with_transport_v1(
            types,
            function,
            0,
            &mut actual,
            &dominance,
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(actual, expected);
        assert!(TRACE.with(|slot| slot.borrow().as_ref().unwrap().work) > 1);
    });
}

#[test]
fn owner_and_root_substitutions_remain_distinct_snapshots() {
    let ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::GlobalView(original)) =
        value()
    else {
        panic!()
    };
    let mut changed = original;
    changed.allocation.allocation_origin += 1;
    assert_ne!(Fact::Global(original), Fact::Global(changed));
    changed = original;
    changed.provenance = SemanticKernelCapabilityProvenanceV1::new(
        SemanticFunctionIdV1::from_index(1),
        original.provenance.kernel_binding(),
        original.provenance.frontend_unit(),
        original.provenance.kernel_marker(),
        original.provenance.target_brand(),
        original.provenance.launch_brand(),
        original.provenance.issuance(),
    )
    .unwrap();
    assert_ne!(Fact::Global(original), Fact::Global(changed));
}

#[test]
fn missing_dead_and_invalid_carriers_never_gain_an_origin() {
    observe(|| {
        for after_value in [None, Some(ProjectedCapabilityValueV1::Invalid)] {
            let mut state = initial();
            let before = record(&state);
            state.remove(&2);
            if let Some(value) = after_value {
                state.insert(2, value);
            }
            let expected = state.clone();
            after(Some(before), None, &state);
            assert_eq!(state, expected);
            assert!(!matches!(fact(state.get(&2)), Fact::Global(_)));
        }
        assert_eq!(records(), 2);
    });
}

#[test]
fn fake_and_shared_to_mutable_reborrows_still_fail() {
    observe(|| {
        let state = initial();
        let ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::GlobalView(view)) =
            state[&2]
        else {
            panic!()
        };
        let place = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(2),
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, view.view)
                    .unwrap(),
            ],
            view.view,
        )
        .unwrap();
        for kind in [SemanticBorrowKindV1::Fake, SemanticBorrowKindV1::Mutable] {
            let before = record(&state);
            let result = capability_borrow_origin_v1(&state, &place, kind);
            after(Some(before), result.as_ref(), &state);
            assert_eq!(result, None);
            assert_eq!(state, initial());
        }
    });
}

#[test]
fn meet_observation_preserves_missing_and_foreign_owner_rejection() {
    let owner = build_owner();
    let function = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap()
        .body();
    observe(|| {
        for foreign in [false, true] {
            let mut current = initial();
            let mut incoming = HashMap::new();
            if foreign {
                let ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::GlobalView(
                    mut view,
                )) = value()
                else {
                    panic!()
                };
                view.allocation.allocation_origin += 1;
                incoming.insert(
                    2,
                    ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::GlobalView(
                        view,
                    )),
                );
            }
            let before = meet(function, 0, 1, &current, &incoming).unwrap();
            assert_eq!(before.locals[0], Some((2, fact(current.get(&2)))));
            assert_eq!(before.incoming[0], Some(fact(incoming.get(&2))));
            let mut expected = current.clone();
            merge_capability_states_v1(&mut expected, &incoming).unwrap();
            merge_capability_states_v1(&mut current, &incoming).unwrap();
            after(Some(before), None, &current);
            assert_eq!(current, expected);
            assert!(!current.contains_key(&2));
        }
    });
}

#[test]
fn incoming_only_global_is_reported_without_restoring_a_meet_fact() {
    let owner = build_owner();
    let function = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap()
        .body();
    observe(|| {
        let mut current = HashMap::new();
        let incoming = initial();
        let before = meet(function, 0, 1, &current, &incoming).unwrap();
        assert_eq!(before.locals[0], Some((2, Fact::Absent)));
        assert_eq!(before.incoming[0], Some(fact(incoming.get(&2))));
        assert!(!merge_capability_states_v1(&mut current, &incoming).unwrap());
        after(Some(before), None, &current);
        assert!(current.is_empty());
        assert_eq!(records(), 1);
    });
}

#[test]
fn meet_prefix_is_deterministic_and_explicitly_truncated() {
    let owner = build_owner();
    let function = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap()
        .body();
    observe(|| {
        let forward = (0..12).map(|local| (local, value())).collect();
        let reverse = (0..12).rev().map(|local| (local, value())).collect();
        let a = meet(function, 0, 1, &forward, &HashMap::new()).unwrap();
        let b = meet(function, 0, 1, &reverse, &HashMap::new()).unwrap();
        assert_eq!(a.locals, b.locals);
        assert!(a.prefix_truncated && b.prefix_truncated);
        assert_eq!(a.locals.map(|row| row.unwrap().0), [0, 1, 2, 3, 4]);
    });
}

#[test]
fn work_and_output_limits_do_not_change_the_subject() {
    observe(|| {
        let state = initial();
        for _ in 0..=MAX_RECORDS {
            let mut before = record(&state);
            before.kind = "GlobalTerminal";
            after(Some(before), None, &state);
        }
        TRACE.with(|slot| {
            let slot = slot.borrow();
            let trace = slot.as_ref().unwrap();
            assert_eq!(trace.records, MAX_RECORDS);
            assert!(trace.truncated && trace.work <= MAX_WORK);
        });
        assert_eq!(state, initial());
    });
    observe(|| {
        assert!(charge(MAX_WORK));
        assert!(!charge(1));
        assert_eq!(initial(), initial());
    });
}

#[test]
fn panic_cleans_trace_and_preserves_original_error() {
    let error = std::panic::catch_unwind(|| observe(|| panic!("original failure"))).unwrap_err();
    assert_eq!(error.downcast_ref::<&str>(), Some(&"original failure"));
    assert!(TRACE.with(|slot| slot.borrow().is_none()));
    assert_eq!(
        observe(|| Err::<(), _>("original typed rejection")),
        Err("original typed rejection")
    );
}
