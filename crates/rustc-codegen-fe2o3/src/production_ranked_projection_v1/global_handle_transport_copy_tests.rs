fn captured_global_extraction_place_v1(
    function: &SemanticFunctionDeclV1,
    through_reference: bool,
) -> SemanticPlaceV1 {
    let mut projections = Vec::new();
    if through_reference {
        projections.push(
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::Dereference,
                function.locals()[35].ty(),
            )
            .unwrap(),
        );
    }
    projections.push(
        SemanticProjectionV1::new(
            SemanticProjectionKindV1::Field(0),
            function.locals()[37].ty(),
        )
        .unwrap(),
    );
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(if through_reference { 36 } else { 35 }),
        projections,
        function.locals()[37].ty(),
    )
    .unwrap()
}

fn captured_global_assert_repeated_copy_v1(through_reference: bool) {
    let (types, callables, function) = captured_global_fixture_v1(false);
    let (baseline, _) = project_capability_index_fixture(&types, &callables, &function).unwrap();
    let mut statements = function.blocks()[6].statements().to_vec();
    statements[5] = typed_assignment(
        37,
        function.locals()[37].ty(),
        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
            captured_global_extraction_place_v1(&function, through_reference),
        )),
    );
    let repeated_field = statements[5].clone();
    statements.insert(6, repeated_field);
    let repeated_dereference = statements.last().unwrap().clone();
    statements.push(repeated_dereference);
    let function = captured_global_replace_statements_v1(&function, statements);
    let (projection, _) = project_capability_index_fixture(&types, &callables, &function).unwrap();
    assert_eq!(projection.global_views, baseline.global_views);
    assert_eq!(projection.direct_read_effects, baseline.direct_read_effects);
    for index in 5..=8 {
        let statement = &function.blocks()[6].statements()[index];
        let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
            unreachable!()
        };
        assert!(
            projection
                .global_uses
                .metadata_assignments
                .contains(&(assignment as *const _))
        );
        let (operations, sources, _) =
            audit_typed_global_statement_v1(&types, &function, &projection, 6, statement).unwrap();
        assert!(operations.is_empty() && sources.is_empty());
    }
}

#[test]
fn captured_global_repeated_field_copy_preserves_exact_read_effect() {
    captured_global_assert_repeated_copy_v1(false);
}

#[test]
fn captured_global_repeated_shared_reference_copy_preserves_exact_read_effect() {
    captured_global_assert_repeated_copy_v1(true);
}

fn captured_global_before_extraction_v1(
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    function: &SemanticFunctionDeclV1,
) -> ProjectedCapabilityStateV1 {
    let (projection, _) = project_capability_index_fixture(types, callables, function).unwrap();
    let mut state = HashMap::from([(
        16,
        ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::GlobalView(
            projection.global_views[6].unwrap(),
        )),
    )]);
    for index in 2..=4 {
        let SemanticStatementKindV1::Assign(assignment) =
            function.blocks()[6].statements()[index].kind()
        else {
            unreachable!()
        };
        let capture = global_handle_transport_v1::assignment(types, function, &state, assignment)
            .expect("authenticated capture assignment");
        state.insert(assignment.destination().local().index() as usize, capture);
    }
    state
}

#[test]
fn captured_global_copy_consumption_apis_preserve_only_copy() {
    let (types, callables, function) = captured_global_fixture_v1(false);
    let initial = captured_global_before_extraction_v1(&types, &callables, &function);
    let place = captured_global_extraction_place_v1(&function, true);
    for api in 0..3 {
        for moving in [false, true] {
            let mut state = initial.clone();
            let operand = if moving {
                SemanticOperandV1::Move(place.clone())
            } else {
                SemanticOperandV1::Copy(place.clone())
            };
            for _ in 0..2 {
                match api {
                    0 => consume_capability_operand_v1(&types, &function, &mut state, &operand),
                    1 => consume_capability_operands_v1(
                        &types,
                        &function,
                        &mut state,
                        std::slice::from_ref(&operand),
                    ),
                    2 => consume_capability_rvalue_operands_v1(
                        &types,
                        &function,
                        &SemanticRvalueKindV1::Use(operand.clone()),
                        &mut state,
                    ),
                    _ => unreachable!(),
                }
            }
            if moving {
                assert_eq!(state[&36], ProjectedCapabilityValueV1::Invalid);
            } else {
                assert_eq!(state, initial);
            }
        }
    }
}

#[test]
fn captured_global_projected_copy_rejects_mutated_path_and_owned_or_scalar_result() {
    let (types, callables, function) = captured_global_fixture_v1(false);
    let initial = captured_global_before_extraction_v1(&types, &callables, &function);
    let nested = function.locals()[37].ty();
    let environment = function.locals()[35].ty();
    for (base, path, result) in [
        (
            35,
            vec![(SemanticProjectionKindV1::Field(1), nested)],
            nested,
        ),
        (
            35,
            vec![(SemanticProjectionKindV1::Field(0), U64_TYPE)],
            U64_TYPE,
        ),
        (
            36,
            vec![(SemanticProjectionKindV1::Field(0), nested)],
            nested,
        ),
        (
            35,
            vec![(SemanticProjectionKindV1::Field(1), U64_TYPE)],
            U64_TYPE,
        ),
        (
            36,
            vec![(SemanticProjectionKindV1::Dereference, environment)],
            environment,
        ),
    ] {
        let place = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(base),
            path.into_iter()
                .map(|(kind, ty)| SemanticProjectionV1::new(kind, ty).unwrap())
                .collect(),
            result,
        )
        .unwrap();
        let mut state = initial.clone();
        assert!(!global_handle_transport_v1::preserves_projected_copy(
            &types, &function, &state, &place,
        ));
        consume_capability_operand_v1(
            &types,
            &function,
            &mut state,
            &SemanticOperandV1::Copy(place),
        );
        assert_eq!(state[&(base as usize)], ProjectedCapabilityValueV1::Invalid);
    }
}

#[test]
fn captured_global_projected_copy_rechecks_missing_and_invalid_dependencies() {
    let (types, callables, function) = captured_global_fixture_v1(false);
    let initial = captured_global_before_extraction_v1(&types, &callables, &function);
    let field = captured_global_extraction_place_v1(&function, true);
    let mut projections = field.projections().to_vec();
    projections.push(
        SemanticProjectionV1::new(
            SemanticProjectionKindV1::Dereference,
            function.locals()[16].ty(),
        )
        .unwrap(),
    );
    let place =
        SemanticPlaceV1::new(field.local(), projections, function.locals()[16].ty()).unwrap();
    assert!(global_handle_transport_v1::preserves_projected_copy(
        &types, &function, &initial, &place
    ));
    for source in [16, 35] {
        for rebound in [false, true] {
            let mut state = initial.clone();
            if rebound {
                state.insert(source, ProjectedCapabilityValueV1::Invalid);
            } else {
                state.remove(&source);
            }
            assert!(!global_handle_transport_v1::preserves_projected_copy(
                &types, &function, &state, &place
            ));
            consume_capability_operand_v1(
                &types,
                &function,
                &mut state,
                &SemanticOperandV1::Copy(place.clone()),
            );
            assert_eq!(state[&36], ProjectedCapabilityValueV1::Invalid);
        }
    }
}

#[test]
fn captured_global_projected_copy_after_alias_or_write_still_loses_custody() {
    let (base_types, callables, base_function) = captured_global_fixture_v1(false);
    let initial = captured_global_before_extraction_v1(&base_types, &callables, &base_function);
    for raw in [false, true] {
        let mut types = base_types.clone();
        let mut locals = base_function.locals().to_vec();
        let environment = locals[35].ty();
        let alias = SemanticTypeIdV1::from_index(types.len() as u32);
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(247)),
            SemanticLayoutIdentityV1::from_sha256(bytes(247)),
            SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    environment,
                    if raw {
                        SemanticPointerKindV1::Raw
                    } else {
                        SemanticPointerKindV1::Reference
                    },
                    SemanticMutabilityV1::Mutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        ));
        let alias_local = locals.len() as u32;
        locals.push(local(247, alias, SemanticLocalRoleV1::Temporary));
        let value = if raw {
            SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Mutable,
                place: typed_place(35, environment),
            }
        } else {
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Mutable,
                place: typed_place(35, environment),
            }
        };
        let function = typed_global_fixture_with_body_v1(
            &base_function,
            locals,
            base_function.blocks().to_vec(),
        );
        let function = captured_global_replace_statements_v1(
            &function,
            vec![
                typed_assignment(alias_local, alias, value),
                base_function.blocks()[6].statements()[5].clone(),
            ],
        );
        let payload = SemanticEnumPayloadDominanceV1::analyze(&function, &types).unwrap();
        let mut state = initial.clone();
        transfer_capability_statements_v1(&types, &function, 6, &mut state, &payload, None, None)
            .unwrap();
        assert_eq!(state[&35], ProjectedCapabilityValueV1::Invalid);
        assert!(!state.contains_key(&37));
        assert!(!global_handle_transport_v1::preserves_projected_copy(
            &types,
            &function,
            &state,
            &captured_global_extraction_place_v1(&function, true),
        ));
    }
    let field = captured_global_extraction_place_v1(&base_function, false);
    let write = statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
        field.clone(),
        SemanticRvalueV1::new(
            field.ty(),
            SemanticRvalueKindV1::Use(typed_operand(34, field.ty())),
        ),
    )));
    let function = captured_global_replace_statements_v1(&base_function, vec![write]);
    let payload = SemanticEnumPayloadDominanceV1::analyze(&function, &base_types).unwrap();
    let mut state = initial;
    transfer_capability_statements_v1(&base_types, &function, 6, &mut state, &payload, None, None)
        .unwrap();
    assert_eq!(state[&35], ProjectedCapabilityValueV1::Invalid);
}

#[test]
fn captured_global_second_copy_rejects_dead_or_moved_carrier() {
    for kill in [None, Some(35), Some(36)] {
        let (types, callables, function) = captured_global_fixture_v1(false);
        let mut statements = function.blocks()[6].statements().to_vec();
        let repeated = statements[5].clone();
        match kill {
            Some(local) => statements.insert(
                6,
                statement(SemanticStatementKindV1::StorageDead(
                    SemanticLocalIdV1::from_index(local),
                )),
            ),
            None => {
                statements[5] = typed_assignment(
                    37,
                    function.locals()[37].ty(),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(
                        captured_global_extraction_place_v1(&function, true),
                    )),
                );
            }
        }
        statements.insert(if kill.is_some() { 7 } else { 6 }, repeated);
        let function = captured_global_replace_statements_v1(&function, statements);
        assert!(project_capability_index_fixture(&types, &callables, &function).is_err());
    }
}
