use super::super::global_enum_transport_v1::tests::{build_owner, initial, joined};
use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

const REF: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);

fn place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
}

// Reconstructed transfer inputs, not collected device-source evidence.
fn aggregate_fixture(count: usize) -> (Vec<SemanticTypeDeclV1>, SemanticFunctionDeclV1) {
    let owner = build_owner();
    let original = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap()
        .body();
    let mut types = owner.source_semantic().types().to_vec();
    let tuple = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([201; 32]),
        SemanticLayoutIdentityV1::from_sha256([202; 32]),
        SemanticTypeLayoutV1::aggregate(
            Some(count as u64 * 8),
            8,
            SemanticAggregateLayoutV1::new((0..count).map(|i| i as u64 * 8).collect(), vec![])
                .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![REF; count]).unwrap()),
    ));
    let origin = |id, line| {
        SemanticSourceOriginV1::new(
            SemanticSourceFileIdentityV1::from_sha256([id; 32]),
            10,
            20,
            line,
            2,
            line,
            12,
        )
        .unwrap()
    };
    let source = SemanticSourceProvenanceV1::new(Some(origin(203, 31)), Some(origin(204, 45)));
    let statements = vec![SemanticStatementV1::new(
        source,
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(3, tuple),
            SemanticRvalueV1::new(
                tuple,
                SemanticRvalueKindV1::aggregate(
                    SemanticAggregateKindV1::Tuple,
                    (0..count)
                        .map(|i| SemanticOperandV1::Move(place(1 + (i % 2) as u32, REF)))
                        .collect(),
                )
                .unwrap(),
            ),
        )),
    )];
    let locals = [SemanticTypeIdV1::from_index(0), REF, REF, tuple]
        .into_iter()
        .enumerate()
        .map(|(i, ty)| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([210 + i as u8; 32]),
                ty,
                if i == 0 {
                    SemanticLocalRoleV1::Return
                } else {
                    SemanticLocalRoleV1::Temporary
                },
                source,
            )
        })
        .collect();
    let function = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        source,
        original.abi().clone(),
        locals,
        SemanticBlockIdV1::from_index(0),
        vec![
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([220; 32]),
                source,
                statements,
                SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    (types, function)
}

fn two_globals() -> ProjectedCapabilityStateV1 {
    let first = initial()[&2].clone();
    let ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::GlobalView(mut second)) =
        first
    else {
        panic!()
    };
    second.allocation.allocation_origin += 1;
    second.allocation.noalias_class += 1;
    HashMap::from([
        (1, first),
        (
            2,
            ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::GlobalView(second)),
        ),
    ])
}

fn transfer(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    state: &mut ProjectedCapabilityStateV1,
    trace: Option<&mut Trace>,
) {
    let dominance = SemanticEnumPayloadDominanceV1::analyze(function, types).unwrap();
    transfer_capability_statements_observed_v1(
        types, function, 0, state, &dominance, None, None, None, trace, &mut 0,
    )
    .unwrap();
}

#[test]
fn aggregate_records_live_operands_and_computed_loss_at_actual_transfer() {
    let (types, function) = aggregate_fixture(2);
    let mut without = two_globals();
    // Writable storage with a shared-only contract is an invalid exclusive
    // candidate, not permission to retain just its read-only sibling.
    let ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::GlobalView(second)) =
        without.get_mut(&2).unwrap()
    else {
        panic!()
    };
    second.allocation.writable = true;
    let mut observed = without.clone();
    transfer(&types, &function, &mut without, None);
    let mut trace = Trace::default();
    transfer(&types, &function, &mut observed, Some(&mut trace));
    assert_eq!(observed, without);
    assert!(matches!(observed.get(&3), Some(ProjectedCapabilityValueV1::Invalid)));
    let event = trace.multi_reference_aggregates_in_final_replay_order[0].unwrap();
    assert_eq!((event.block, event.statement, event.destination), (0, 0, 3));
    assert_eq!(event.source, function.blocks()[0].statements()[0].source());
    assert_ne!(event.source.expansion(), event.source.call_site());
    assert_eq!(event.computed_origin_before_consumption, Custody::Invalid);
    assert_eq!(event.destination_after, Custody::Invalid);
    for (i, input) in event.inputs.iter().flatten().enumerate() {
        assert_eq!((input.ordinal, input.local), (i, i as u32 + 1));
        assert!(
            matches!(input.base_before, Custody::Global { allocation, .. } if allocation == 11 + i as u64)
        );
        assert_eq!(input.base_after, Custody::Invalid);
    }
    assert!(trace.untransferred_input_removals_in_final_replay_order[0].is_some());
}

#[test]
fn admitted_shared_product_is_observed_as_retained_not_untransferred_loss() {
    let (types, function) = aggregate_fixture(2);
    let mut without = two_globals();
    let mut observed = without.clone();
    let dominance = SemanticEnumPayloadDominanceV1::analyze(&function, &types).unwrap();
    let mut ordinary_work = 7;
    let mut observed_work = 7;
    transfer_capability_statements_metered_v1(
        &types,
        &function,
        0,
        &mut without,
        &dominance,
        None,
        None,
        None,
        &mut ordinary_work,
    )
    .unwrap();
    let mut trace = Trace::default();
    transfer_capability_statements_observed_v1(
        &types,
        &function,
        0,
        &mut observed,
        &dominance,
        None,
        None,
        None,
        Some(&mut trace),
        &mut observed_work,
    )
    .unwrap();
    assert_eq!(observed, without);
    assert_eq!(observed_work, ordinary_work);
    assert!(observed_work > 7);
    assert!(matches!(
        observed.get(&3),
        Some(ProjectedCapabilityValueV1::CapturedGlobalProduct(_))
    ));
    let event = trace.multi_reference_aggregates_in_final_replay_order[0].unwrap();
    assert_eq!(event.computed_origin_before_consumption, Custody::Capture);
    assert_eq!(event.destination_after, Custody::Capture);
    for input in event.inputs.iter().flatten() {
        assert!(matches!(input.base_before, Custody::Global { .. }));
        assert_eq!(input.base_after, Custody::Invalid);
    }
    assert!(
        trace
            .untransferred_input_removals_in_final_replay_order
            .iter()
            .all(Option::is_none)
    );
}

#[test]
fn absent_invalid_and_other_inputs_are_observed_not_invented_as_global() {
    let (types, function) = aggregate_fixture(2);
    for second in [
        None,
        Some(ProjectedCapabilityValueV1::Invalid),
        Some(ProjectedCapabilityValueV1::Known(
            ProjectedCapabilityOriginV1::GlobalLoadedScalar { block: 9 },
        )),
    ] {
        let mut state = two_globals();
        state.remove(&2);
        if let Some(second) = second {
            state.insert(2, second);
        }
        let expected = custody(state.get(&2));
        let mut without = state.clone();
        transfer(&types, &function, &mut without, None);
        let mut trace = Trace::default();
        transfer(&types, &function, &mut state, Some(&mut trace));
        assert_eq!(state, without);
        assert_eq!(
            trace.multi_reference_aggregates_in_final_replay_order[0]
                .unwrap()
                .inputs[1]
                .unwrap()
                .base_before,
            expected
        );
    }
    let mut trace = Trace::default();
    transfer(&types, &function, &mut HashMap::new(), Some(&mut trace));
    assert!(trace.multi_reference_aggregates_in_final_replay_order[0].is_some());
    assert!(
        trace
            .untransferred_input_removals_in_final_replay_order
            .iter()
            .all(Option::is_none)
    );
}

#[test]
fn scalar_sibling_loss_is_distinct_from_real_owner_certificate_preservation() {
    let owner = build_owner();
    let types = owner.source_semantic().types();
    let function = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap()
        .body();
    let reads = private_scalar_capture_v1::PrivateScalarReads::for_root(
        &owner,
        SemanticFunctionIdV1::from_index(0),
    )
    .unwrap();
    let initial = joined(
        types,
        function,
        reads.conditions_for(types, function).unwrap(),
    );
    let dominance = SemanticEnumPayloadDominanceV1::analyze(function, types).unwrap();
    for certificate in [None, Some(&reads)] {
        let mut state = initial.clone();
        let mut without = initial.clone();
        transfer_capability_statements_with_transport_v1(
            types,
            function,
            4,
            &mut without,
            &dominance,
            None,
            None,
            certificate,
        )
        .unwrap();
        let mut trace = Trace::default();
        transfer_capability_statements_observed_v1(
            types,
            function,
            4,
            &mut state,
            &dominance,
            None,
            None,
            certificate,
            Some(&mut trace),
            &mut 0,
        )
        .unwrap();
        assert_eq!(state, without);
        if certificate.is_some() {
            assert!(
                trace
                    .untransferred_input_removals_in_final_replay_order
                    .iter()
                    .flatten()
                    .all(|event| event.statement != 2)
            );
            assert!(state.contains_key(&10));
            // Isolate consumption of the exact original scalar statement after
            // obtaining its live shared carrier through the successful hook.
            let statement = &function.blocks()[4].statements()[2];
            assert!(reads.contains(function, statement));
            assert!(
                !global_handle_transport_v1::preserves_initialized_scalar_copy(
                    types, function, &state, statement, None
                )
            );
            let mut scalar_trace = Trace::default();
            let before = scalar_trace.before(types, function, 4, 2, &state);
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                panic!()
            };
            consume_capability_rvalue_operands_v1(
                types,
                function,
                assignment.value().kind(),
                &mut state,
            );
            scalar_trace.after(before, None, false, &state);
            let event = scalar_trace.untransferred_input_removals_in_final_replay_order[0].unwrap();
            assert_eq!(event.kind, "Copy");
            assert!(!event.initialized_scalar_copy_preserved);
            assert_eq!(event.inputs[0].unwrap().base_before, Custody::Capture);
            assert_eq!(event.inputs[0].unwrap().base_after, Custody::Invalid);
        } else {
            // The missing certificate also removes enum-edge evidence, so this
            // full-block run loses the payload earlier, not at the scalar copy.
            assert_eq!(
                trace.untransferred_input_removals_in_final_replay_order[0]
                    .unwrap()
                    .statement,
                0
            );
            assert!(!state.contains_key(&10));
        }
    }
}

#[test]
fn diagnostic_work_exhaustion_never_changes_transfer_or_semantic_limits() {
    let (types, function) = aggregate_fixture(2);
    let mut expected = two_globals();
    transfer(&types, &function, &mut expected, None);
    for remaining in [0, 1, 5, 11] {
        let mut trace = Trace {
            work: MAX_WORK - remaining,
            ..Trace::default()
        };
        let mut state = two_globals();
        transfer(&types, &function, &mut state, Some(&mut trace));
        assert_eq!(state, expected);
        assert!(trace.work_truncated);
        assert!(trace.work <= MAX_WORK);
    }
}

#[test]
fn fixed_rosters_and_input_prefix_mark_every_truncation() {
    let (types, function) = aggregate_fixture(5);
    let mut trace = Trace::default();
    for _ in 0..=MAX_AGGREGATES {
        transfer(&types, &function, &mut two_globals(), Some(&mut trace));
    }
    assert!(trace.aggregates_truncated && trace.removals_truncated && trace.input_roster_truncated);
    assert!(
        trace
            .multi_reference_aggregates_in_final_replay_order
            .iter()
            .flatten()
            .all(|event| event.inputs_truncated)
    );
    assert_eq!(
        trace
            .multi_reference_aggregates_in_final_replay_order
            .iter()
            .flatten()
            .count(),
        MAX_AGGREGATES
    );
    assert_eq!(
        trace
            .untransferred_input_removals_in_final_replay_order
            .iter()
            .flatten()
            .count(),
        MAX_REMOVALS
    );
}

#[test]
fn formatting_is_deterministic_independent_of_state_insertion_order() {
    let (types, function) = aggregate_fixture(2);
    let initial = two_globals();
    let mut rendered = Vec::new();
    for order in [[1, 2], [2, 1]] {
        let mut state = order
            .into_iter()
            .map(|key| (key, initial[&key].clone()))
            .collect();
        let mut trace = Trace::default();
        transfer(&types, &function, &mut state, Some(&mut trace));
        rendered.push(format!("{trace:?}"));
    }
    assert_eq!(rendered[0], rendered[1]);
    assert!(std::mem::size_of::<Trace>() < 64 * 1024);
}

#[test]
fn attach_preserves_primary_rejection_and_does_not_attach_to_unrelated_errors() {
    let (types, function) = aggregate_fixture(2);
    let mut trace = Trace::default();
    transfer(&types, &function, &mut two_globals(), Some(&mut trace));
    let work = trace.work;
    let unrelated = attach(
        ProductionRankedProjectionErrorV1::Unsupported("unchanged"),
        &mut trace,
    );
    assert!(matches!(
        unrelated,
        ProductionRankedProjectionErrorV1::Unsupported("unchanged")
    ));
    assert_eq!(trace.work, work);
    let error = ProductionRankedProjectionErrorV1::GlobalAccess {
        block: 0,
        source: function.source(),
        receiver: Some(3),
        reason: "receiver binding is absent",
        observation: Some(Box::new(
            global_origin_observation_v1::Observation::collect(&function, &HashMap::new(), 3),
        )),
    };
    let attached = attach(error, &mut trace);
    assert!(matches!(
        &attached,
        ProductionRankedProjectionErrorV1::GlobalAccess {
            block: 0,
            receiver: Some(3),
            reason: "receiver binding is absent",
            ..
        }
    ));
    assert!(format!("{attached:?}").contains("multi_reference_aggregates_in_final_replay_order"));
    assert_eq!(trace.work, 0);
}
