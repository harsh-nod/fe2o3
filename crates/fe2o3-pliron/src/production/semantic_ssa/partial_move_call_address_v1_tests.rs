use super::*;

fn pointer_destination(trailing_index: bool) -> SemanticPlaceV1 {
    let ty = SemanticTypeIdV1::from_index(1);
    let mut projections = vec![
        fe2o3_mir_model::semantic_mir_v1::SemanticProjectionV1::new(
            SemanticProjectionKindV1::Field(0),
            ty,
        )
        .unwrap(),
        fe2o3_mir_model::semantic_mir_v1::SemanticProjectionV1::new(
            SemanticProjectionKindV1::Dereference,
            ty,
        )
        .unwrap(),
    ];
    if trailing_index {
        projections.push(
            fe2o3_mir_model::semantic_mir_v1::SemanticProjectionV1::new(
                SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(2)),
                ty,
            )
            .unwrap(),
        );
    }
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), projections, ty).unwrap()
}

fn budget(work_limit: usize) -> SemanticPartialMoveBudgetV1 {
    let defaults = SsaPlannerLimitsV1::default();
    SemanticPartialMoveBudgetV1 {
        function: SemanticFunctionIdV1::from_index(0),
        base_storage_words: 11,
        base_work_units: 7,
        state_entries: 0,
        work_units: 0,
        limits: SsaPlannerLimitsV1::try_new(
            defaults.max_variables(),
            defaults.max_blocks(),
            defaults.max_edges(),
            defaults.max_events(),
            defaults.max_edge_definitions(),
            defaults.max_output_items(),
            defaults.max_storage_words(),
            work_limit,
        )
        .unwrap(),
    }
}

fn location() -> SemanticPartialMoveLocationV1 {
    SemanticPartialMoveLocationV1 {
        function: SemanticFunctionIdV1::from_index(0),
        block: 3,
        statement: None,
    }
}

#[test]
fn pointer_prefix_work_is_lookup_plus_one_path_plus_one_comparison() {
    for moved_field in [0, 1] {
        let state = BTreeMap::from([(
            1,
            BTreeSet::from([vec![SemanticMovePathElementV1::Field(moved_field)]]),
        )]);
        let original = state.clone();
        let destination = pointer_destination(false);
        // Direct prefix helper: lookup1 + one visited row1 + one comparison1.
        let mut exact = budget(7 + 3);
        let result = validate_partial_move_call_pointer_prefix_v1(
            &destination,
            1,
            location(),
            &state,
            &mut exact,
        );
        if moved_field == 0 {
            assert!(matches!(
                result,
                Err(ProductionSemanticSsaErrorV1::PartialMove {
                    block: 3,
                    statement: None,
                    local: 1,
                    violation: SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
                    ..
                })
            ));
        } else {
            result.unwrap();
        }
        assert_eq!(exact.work_units, 3);
        assert_eq!(exact.state_entries, 0);
        assert_eq!(exact.base_storage_words, 11);
        assert_eq!(state, original);

        let mut short = budget(7 + 2);
        assert!(matches!(
            validate_partial_move_call_pointer_prefix_v1(
                &destination,
                1,
                location(),
                &state,
                &mut short,
            ),
            Err(ProductionSemanticSsaErrorV1::PartialMoveResourceLimit {
                resource: SsaPlannerResourceV1::WorkUnits,
                required: 10,
                limit: 9,
                ..
            })
        ));
        // This existing budget records the attempted charge before rejecting it.
        assert_eq!(short.work_units, 3);
        assert_eq!(short.state_entries, 0);
        assert_eq!(state, original);
    }
}

#[test]
fn prefix_charge_denial_prevents_the_later_index_projection_read() {
    let state = BTreeMap::from([
        (
            1,
            BTreeSet::from([vec![SemanticMovePathElementV1::Field(1)]]),
        ),
        (2, BTreeSet::from([vec![]])),
    ]);
    let original = state.clone();
    let destination = pointer_destination(true);
    // Field and Deref visits2 + prefix lookup/row/comparison3 =5. Deny that
    // comparison; the later Index visit and moved-index path read cannot run.
    let mut short = budget(7 + 4);
    assert!(matches!(
        validate_partial_move_call_address_v1(&destination, location(), &state, &mut short,),
        Err(ProductionSemanticSsaErrorV1::PartialMoveResourceLimit {
            resource: SsaPlannerResourceV1::WorkUnits,
            required: 12,
            limit: 11,
            ..
        })
    ));
    assert_eq!(short.work_units, 5);
    assert_eq!(short.state_entries, 0);
    assert_eq!(state, original);

    // With those two later charges admitted, the existing moved-index failure
    // is reached: total5 + Index projection1 + empty-path lookup1 =7.
    let mut exact = budget(7 + 7);
    assert!(matches!(
        validate_partial_move_call_address_v1(&destination, location(), &state, &mut exact,),
        Err(ProductionSemanticSsaErrorV1::PartialMove {
            block: 3,
            statement: None,
            local: 2,
            violation: SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
            ..
        })
    ));
    assert_eq!(exact.work_units, 7);
    assert_eq!(exact.state_entries, 0);
    assert_eq!(state, original);
}
