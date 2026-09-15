use super::super::global_enum_transport_v1::tests::{build_owner, initial};
use super::*;

#[test]
fn receiver_observation_keeps_exact_projection_and_borrow_chain_without_authority() {
    let owner = build_owner();
    let function = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap()
        .body();
    let state = initial();
    let before = state.clone();
    let observed = Observation::collect(function, &state, 10);
    assert!(!observed.truncated);
    assert_eq!(observed.receiver_state_before_consumption, Custody::Absent);
    let first = &observed.syntactic_definitions_not_dominance[0];
    assert_eq!(
        (first.local, first.block, first.statement, first.kind),
        (10, 4, Some(3), "Copy")
    );
    let source = first.inputs[0].unwrap();
    assert_eq!(
        (source.local, source.projection_count, source.mode),
        (8, 2, "Copy")
    );
    assert_eq!(
        source.projections[0].unwrap().0,
        SemanticProjectionKindV1::Dereference
    );
    assert_eq!(
        source.projections[1].unwrap().0,
        SemanticProjectionKindV1::Field(0)
    );
    let borrow = observed
        .syntactic_definitions_not_dominance
        .iter()
        .find(|row| row.local == 8)
        .unwrap();
    assert_eq!(
        (
            borrow.kind,
            borrow.inputs[0].unwrap().local,
            borrow.inputs[0].unwrap().mode
        ),
        ("Borrow", 7, "SharedBorrow")
    );
    assert_eq!(state, before);
    assert!(observed.private_flow.is_none());
}

#[test]
fn mutually_exclusive_definitions_remain_a_roster_not_a_selected_payload() {
    let owner = build_owner();
    let function = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap()
        .body();
    let state = initial();
    let observed = Observation::collect(function, &state, 4);
    let rows = observed
        .syntactic_definitions_not_dominance
        .iter()
        .filter(|row| row.local == 4)
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 2);
    assert_eq!((rows[0].block, rows[1].block), (1, 2));
    assert!(rows[0].inputs[0].is_some());
    assert!(rows[1].inputs.iter().all(Option::is_none));
    assert_eq!(observed.receiver_state_before_consumption, Custody::Absent);
}

#[test]
fn missing_invalid_and_bound_input_observations_do_not_change_any_custody() {
    let owner = build_owner();
    let function = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap()
        .body();
    let mut state = initial();
    let bound = state[&2].clone();
    let expected = custody(&state, 2);
    assert!(matches!(expected, Custody::Global { .. }));
    for (value, expected) in [
        (None, Custody::Absent),
        (Some(ProjectedCapabilityValueV1::Invalid), Custody::Invalid),
        (Some(bound), expected),
    ] {
        state.remove(&2);
        if let Some(value) = value {
            state.insert(2, value);
        }
        let before = state.clone();
        let observed = Observation::collect(function, &state, 2);
        assert_eq!(observed.receiver_state_before_consumption, expected);
        assert_eq!(state, before);
    }
}

#[test]
fn diagnostic_work_exhaustion_and_foreign_local_are_explicit_not_proof_results() {
    let owner = build_owner();
    let function = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap()
        .body();
    let state = initial();
    let exact = Observation::collect(function, &state, 10);
    assert!(!exact.truncated);
    for limit in [0, 1, exact.work - 1] {
        let observed = Observation::with_budget(function, &state, 10, limit);
        assert!(observed.truncated);
        assert_eq!(observed.work, limit);
        assert!(observed.syntactic_definitions_not_dominance.len() <= MAX_ROWS);
    }
    let enough = Observation::with_budget(function, &state, 10, exact.work);
    assert!(!enough.truncated);
    let outside = Observation::collect(function, &state, u32::MAX);
    assert!(outside.truncated);
    assert!(outside.syntactic_definitions_not_dominance.is_empty());
}

#[test]
fn attached_private_flow_requires_the_exact_retained_function_and_preserves_rejection() {
    let owner = build_owner();
    let function = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap()
        .body();
    let foreign_function = function.clone();
    let reads = private_scalar_capture_v1::PrivateScalarReads::for_root(
        &owner,
        SemanticFunctionIdV1::from_index(0),
    )
    .unwrap();
    for (subject, available) in [(function, true), (&foreign_function, false)] {
        let error = ProductionRankedProjectionErrorV1::GlobalAccess {
            block: 4,
            source: function.source(),
            receiver: Some(10),
            reason: "receiver binding is absent",
            observation: Some(Box::new(Observation::collect(function, &initial(), 10))),
        };
        let error = attach_private_flow(error, subject, Some(&reads));
        let ProductionRankedProjectionErrorV1::GlobalAccess {
            block,
            receiver,
            reason,
            observation: Some(observation),
            ..
        } = error
        else {
            panic!("rejection kind changed");
        };
        assert_eq!(
            (block, receiver, reason),
            (4, Some(10), "receiver binding is absent")
        );
        assert_eq!(observation.private_flow.is_some(), available);
    }
    let untouched = attach_private_flow(
        ProductionRankedProjectionErrorV1::Incomplete("unchanged"),
        function,
        Some(&reads),
    );
    assert!(matches!(
        untouched,
        ProductionRankedProjectionErrorV1::Incomplete("unchanged")
    ));
}

#[test]
fn diagnostic_row_local_input_and_projection_caps_are_explicit() {
    use fe2o3_mir_model::semantic_mir_v1::SemanticProjectionV1;
    let owner = build_owner();
    let function = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap()
        .body();
    let state = initial();
    let mut observation = Observation::with_budget(function, &state, 10, 0);
    observation.truncated = false;
    let blank_row = || Row {
        local: 10,
        block: 4,
        statement: Some(3),
        source: function.source(),
        kind: "diagnostic-only",
        destination_projection_count: 0,
        callee: None,
        inputs: [None; MAX_INPUTS],
    };
    let place = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(2),
        vec![],
        SemanticTypeIdV1::from_index(3),
    )
    .unwrap();
    let mut row = blank_row();
    for _ in 0..MAX_INPUTS {
        observation.input(&mut row, &state, &place, "Copy");
    }
    assert!(!observation.truncated);
    observation.input(&mut row, &state, &place, "Copy");
    assert!(observation.truncated);
    assert_eq!(row.inputs.iter().flatten().count(), MAX_INPUTS);

    observation.truncated = false;
    let projected = SemanticPlaceV1::new(
        place.local(),
        (0..=MAX_PROJECTIONS)
            .map(|_| {
                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), place.ty()).unwrap()
            })
            .collect(),
        place.ty(),
    )
    .unwrap();
    let mut row = blank_row();
    observation.input(&mut row, &state, &projected, "Copy");
    assert!(observation.truncated);
    assert_eq!(row.inputs[0].unwrap().projection_count, MAX_PROJECTIONS + 1);
    assert_eq!(
        row.inputs[0].unwrap().projections.iter().flatten().count(),
        MAX_PROJECTIONS
    );

    observation.truncated = false;
    let mut pending = (100..100 + MAX_LOCALS as u32).collect::<Vec<_>>();
    assert!(observation.row(row, &mut pending));
    assert!(observation.truncated);
    assert_eq!(pending.len(), MAX_LOCALS);
    observation.truncated = false;
    observation.syntactic_definitions_not_dominance.clear();
    for _ in 0..MAX_ROWS {
        assert!(observation.row(blank_row(), &mut pending));
    }
    assert!(!observation.truncated);
    assert!(!observation.row(blank_row(), &mut pending));
    assert!(observation.truncated);
    assert_eq!(
        observation.syntactic_definitions_not_dominance.len(),
        MAX_ROWS
    );
}
