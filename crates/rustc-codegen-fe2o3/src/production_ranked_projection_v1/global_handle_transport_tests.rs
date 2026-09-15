fn captured_global_fixture_v1(
    duplicate: bool,
) -> (
    Vec<SemanticTypeDeclV1>,
    Vec<SemanticCallableDeclV1>,
    SemanticFunctionDeclV1,
) {
    let (mut types, callables, function) = typed_global_source_fixture_v1();
    let reference = function.locals()[16].ty();
    let mut add_reference = |pointee, tag| {
        let ty = SemanticTypeIdV1::from_index(types.len() as u32);
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(tag)),
            SemanticLayoutIdentityV1::from_sha256(bytes(tag)),
            SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    pointee,
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Immutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        ));
        ty
    };
    let nested = add_reference(reference, 241);
    let environment = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(242)),
        SemanticLayoutIdentityV1::from_sha256(bytes(242)),
        SemanticTypeLayoutV1::aggregate(
            Some(16),
            8,
            SemanticAggregateLayoutV1::new(vec![0, 8], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(
            SemanticAggregateTypeV1::new(vec![nested, if duplicate { nested } else { U64_TYPE }])
                .unwrap(),
        ),
    ));
    let borrowed_environment = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(243)),
        SemanticLayoutIdentityV1::from_sha256(bytes(243)),
        SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                environment,
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    ));
    let mut locals = function.locals().to_vec();
    assert_eq!(locals.len(), 34);
    for (index, ty) in [nested, environment, borrowed_environment, nested, reference]
        .into_iter()
        .enumerate()
    {
        locals.push(local(241 + index as u8, ty, SemanticLocalRoleV1::Temporary));
    }
    let mut blocks = function.blocks().to_vec();
    let original = &blocks[6];
    let mut statements = original.statements().to_vec();
    assert_eq!(statements.len(), 2);
    statements.extend([
        typed_assignment(
            34,
            nested,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: typed_place(16, reference),
            },
        ),
        typed_assignment(
            35,
            environment,
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(
                    SemanticAggregateKindV1::Aggregate,
                    vec![
                        typed_operand(34, nested),
                        if duplicate {
                            typed_operand(34, nested)
                        } else {
                            typed_constant(U64_TYPE, 0, 8)
                        },
                    ],
                )
                .unwrap(),
            ),
        ),
        typed_assignment(
            36,
            borrowed_environment,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: typed_place(35, environment),
            },
        ),
        typed_assignment(
            37,
            nested,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
                SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(36),
                    vec![
                        SemanticProjectionV1::new(
                            SemanticProjectionKindV1::Dereference,
                            environment,
                        )
                        .unwrap(),
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), nested)
                            .unwrap(),
                    ],
                    nested,
                )
                .unwrap(),
            )),
        ),
        typed_assignment(
            38,
            reference,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
                SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(37),
                    vec![
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, reference)
                            .unwrap(),
                    ],
                    reference,
                )
                .unwrap(),
            )),
        ),
    ]);
    let SemanticTerminatorKindV1::Call(call) = original.terminator().kind() else {
        unreachable!()
    };
    let mut arguments = call.arguments().to_vec();
    arguments[0] = typed_operand(38, reference);
    blocks[6] = SemanticBasicBlockV1::new(
        original.identity(),
        original.source(),
        statements,
        SemanticTerminatorV1::new(
            original.terminator().source(),
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    call.callee(),
                    arguments,
                    call.destination().cloned(),
                    call.unwind(),
                )
                .unwrap(),
            ),
        ),
    )
    .unwrap();
    (
        types,
        callables,
        typed_global_fixture_with_body_v1(&function, locals, blocks),
    )
}

#[test]
fn captured_global_shared_reference_preserves_bound_view_and_element_access() {
    let (base_types, base_callables, base_function) = typed_global_source_fixture_v1();
    let (baseline, _) =
        project_capability_index_fixture(&base_types, &base_callables, &base_function).unwrap();
    let (types, callables, function) = captured_global_fixture_v1(false);
    let (projection, _) = project_capability_index_fixture(&types, &callables, &function).unwrap();
    assert_eq!(projection.global_views[6], baseline.global_views[6]);
    assert_eq!(projection.global_views[11], baseline.global_views[11]);
    assert!(projection.global_views[6].is_some());
    for index in [2, 4, 5, 6] {
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
    // The original load remains an indexed element access, never a free scalar.
    assert_eq!(projection.direct_read_effects, baseline.direct_read_effects);
    let read = projection.direct_read_effects[6]
        .as_ref()
        .expect("source load");
    assert_eq!(read.access, AccessKindAttr::Read);
    assert_eq!(read.memory_space, MemorySpaceAttr::Global);
    assert_eq!(read.indices.len(), 1);
    assert_eq!(read.comparisons.len(), 1);
    assert_eq!(read.comparisons[0].0, read.indices[0]);
    assert!(matches!(
        read.comparisons[0].1,
        ProductionRankedValueV1::Argument(_)
    ));
}

fn captured_global_replace_statements_v1(
    function: &SemanticFunctionDeclV1,
    statements: Vec<SemanticStatementV1>,
) -> SemanticFunctionDeclV1 {
    let mut blocks = function.blocks().to_vec();
    let original = &blocks[6];
    blocks[6] = SemanticBasicBlockV1::new(
        original.identity(),
        original.source(),
        statements,
        original.terminator().clone(),
    )
    .unwrap();
    typed_global_fixture_with_body_v1(function, function.locals().to_vec(), blocks)
}

#[test]
fn captured_global_rejects_dead_or_rebound_reference_storage() {
    for (local_id, position) in [(16, 3), (35, 5)] {
        let (types, callables, function) = captured_global_fixture_v1(false);
        let mut statements = function.blocks()[6].statements().to_vec();
        statements.insert(
            position,
            statement(SemanticStatementKindV1::StorageDead(
                SemanticLocalIdV1::from_index(local_id),
            )),
        );
        let function = captured_global_replace_statements_v1(&function, statements);
        assert!(project_capability_index_fixture(&types, &callables, &function).is_err());
    }
}

#[test]
fn captured_global_never_recovers_a_missing_source_binding() {
    let (types, callables, function) = captured_global_fixture_v1(false);
    let mut statements = function.blocks()[6].statements().to_vec();
    statements.insert(
        0,
        statement(SemanticStatementKindV1::StorageDead(
            SemanticLocalIdV1::from_index(11),
        )),
    );
    let function = captured_global_replace_statements_v1(&function, statements);
    assert!(project_capability_index_fixture(&types, &callables, &function).is_err());
}

#[test]
fn captured_global_does_not_select_one_of_multiple_captured_handles() {
    let (types, callables, function) = captured_global_fixture_v1(true);
    assert!(project_capability_index_fixture(&types, &callables, &function).is_err());
}

#[test]
fn captured_global_rejects_mutable_and_fake_environment_borrows() {
    for kind in [SemanticBorrowKindV1::Mutable, SemanticBorrowKindV1::Fake] {
        let (types, callables, function) = captured_global_fixture_v1(false);
        let mut statements = function.blocks()[6].statements().to_vec();
        statements[4] = typed_assignment(
            36,
            function.locals()[36].ty(),
            SemanticRvalueKindV1::Borrow {
                kind,
                place: typed_place(35, function.locals()[35].ty()),
            },
        );
        let function = captured_global_replace_statements_v1(&function, statements);
        assert!(project_capability_index_fixture(&types, &callables, &function).is_err());
    }
}

#[test]
fn captured_global_mutable_or_raw_alias_invalidates_only_reference_storage() {
    let (types, callables, function) = captured_global_fixture_v1(false);
    let (projection, _) = project_capability_index_fixture(&types, &callables, &function).unwrap();
    let mut view = projection.global_views[6].unwrap();
    let shared = ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::GlobalView(view));
    let mut state = HashMap::from([(16, shared)]);
    let SemanticStatementKindV1::Assign(borrow) = function.blocks()[6].statements()[2].kind()
    else {
        unreachable!()
    };
    let capture =
        global_handle_transport_v1::assignment(&types, &function, &state, borrow).unwrap();
    state.insert(34, capture);
    for local_id in [16, 34] {
        assert!(global_handle_transport_v1::invalidates_reference_storage(
            &state,
            &typed_place(local_id, function.locals()[local_id as usize].ty()),
        ));
    }
    view.borrow = None;
    state.insert(
        15,
        ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::GlobalView(view)),
    );
    assert!(
        !global_handle_transport_v1::invalidates_reference_storage(
            &state,
            &typed_place(15, function.locals()[15].ty()),
        ),
        "ordinary owned Global borrowing retains its existing lifecycle"
    );
}

#[test]
fn captured_global_preserves_exact_source_contract_rejection() {
    let (types, mut callables, function) = captured_global_fixture_v1(false);
    let SemanticTerminatorKindV1::Call(call) = function.blocks()[6].terminator().kind() else {
        unreachable!()
    };
    let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } =
        &mut callables[call.callee().index() as usize]
    else {
        unreachable!()
    };
    let SemanticCompilerIntrinsicOperationV1::CapabilityGlobalLoad {
        source_identity, ..
    } = operation
    else {
        unreachable!()
    };
    *source_identity = SemanticFunctionIdentityV1::from_sha256(bytes(249));
    assert!(project_capability_index_fixture(&types, &callables, &function).is_err());
}

include!("global_handle_transport_copy_tests.rs");
