fn dead_cast_function_v1(
    function: &SemanticFunctionDeclV1,
    locals: Vec<SemanticLocalDeclV1>,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    SemanticFunctionDeclV1::new(
        function.identity(),
        function.role(),
        function.item_definition_identity(),
        function.monomorphization_identity(),
        function.generic_type_arguments_identity(),
        function.const_generic_arguments_identity(),
        function.source(),
        function.abi().clone(),
        locals,
        function.entry(),
        blocks,
    )
    .unwrap()
}

fn dead_cast_place_v1(local: u32, ty: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(local),
        vec![],
        SemanticTypeIdV1::from_index(ty),
    )
    .unwrap()
}

fn dead_cast_assignment_v1(kind: SemanticCastKindV1) -> SemanticStatementV1 {
    statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
        dead_cast_place_v1(12, 7),
        SemanticRvalueV1::new(
            SemanticTypeIdV1::from_index(7),
            SemanticRvalueKindV1::Cast {
                kind,
                operand: SemanticOperandV1::Copy(dead_cast_place_v1(6, 6)),
            },
        ),
    )))
}

fn dead_cast_fixture_v1() -> (Vec<SemanticTypeDeclV1>, SemanticFunctionDeclV1) {
    let (types, function) = indexed_atomic_fixture_v1(true, false);
    let mut locals = function.locals().to_vec();
    locals.push(local(
        240,
        SemanticTypeIdV1::from_index(7),
        SemanticLocalRoleV1::Temporary,
    ));
    let mut blocks = function.blocks().to_vec();
    let mut statements = blocks[1].statements().to_vec();
    statements.insert(3, dead_cast_assignment_v1(SemanticCastKindV1::Pointer));
    statements.insert(
        4,
        statement(SemanticStatementKindV1::StorageDead(
            SemanticLocalIdV1::from_index(12),
        )),
    );
    blocks[1] = SemanticBasicBlockV1::new(
        blocks[1].identity(),
        blocks[1].source(),
        statements,
        blocks[1].terminator().clone(),
    )
    .unwrap();
    let changed = dead_cast_function_v1(&function, locals, blocks);
    (types, changed)
}

fn dead_cast_check_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    inventory: &AuthenticatedAtomicAllocationsV1,
) -> Result<bool, ProductionRankedProjectionErrorV1> {
    let SemanticStatementKindV1::Assign(assignment) = function.blocks()[1].statements()[3].kind()
    else {
        panic!("cast fixture");
    };
    indexed_atomic_dead_cast_v1(
        types,
        function,
        inventory,
        ScalarAssignmentSiteV1 {
            block: 1,
            statement: 3,
        },
        assignment,
    )
}

#[test]
fn indexed_atomic_dead_sibling_cast_adds_no_memory_authority() {
    let (types, function) = dead_cast_fixture_v1();
    let (checks, allocations) = indexed_atomic_inventory_fixture_v1(&types, &function);
    let inventory =
        authenticated_atomic_allocations_v1(&types, &function, &checks, &allocations).unwrap();
    assert!(dead_cast_check_v1(&types, &function, &inventory).unwrap());
    assert_eq!(inventory.indexed_uses.len(), 2);
    assert_eq!(inventory.coherent, vec![1]);
    assert!(
        !inventory
            .contains_local(SemanticLocalIdV1::from_index(12))
            .unwrap()
    );
    let before = inventory.work.get();
    dead_cast_check_v1(&types, &function, &inventory).unwrap();
    let cost = inventory.work.get() - before;
    inventory.work.set(MAX_PROJECTED_LOOP_GRAPH_WORK_V1 - cost);
    assert!(dead_cast_check_v1(&types, &function, &inventory).unwrap());
    assert_eq!(inventory.work.get(), MAX_PROJECTED_LOOP_GRAPH_WORK_V1);
    inventory
        .work
        .set(MAX_PROJECTED_LOOP_GRAPH_WORK_V1 - cost + 1);
    assert!(dead_cast_check_v1(&types, &function, &inventory).is_err());
}

#[test]
fn indexed_atomic_dead_cast_rejects_roles_types_and_redefinitions() {
    let (types, function) = dead_cast_fixture_v1();
    let (checks, allocations) = indexed_atomic_inventory_fixture_v1(&types, &function);
    let inventory =
        authenticated_atomic_allocations_v1(&types, &function, &checks, &allocations).unwrap();
    for role in [
        SemanticLocalRoleV1::Return,
        SemanticLocalRoleV1::Argument(0),
    ] {
        let mut locals = function.locals().to_vec();
        locals[12] = local(240, SemanticTypeIdV1::from_index(7), role);
        let changed = dead_cast_function_v1(&function, locals, function.blocks().to_vec());
        assert!(!dead_cast_check_v1(&types, &changed, &inventory).unwrap());
    }
    for (space, width, pointee, kind) in [
        (1, 64, 0, SemanticPointerKindV1::Raw),
        (0, 32, 0, SemanticPointerKindV1::Raw),
        (0, 64, 1, SemanticPointerKindV1::Raw),
        (0, 64, 0, SemanticPointerKindV1::Reference),
    ] {
        let mut changed_types = types.clone();
        changed_types[7] = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(241)),
            SemanticLayoutIdentityV1::from_sha256(bytes(241)),
            SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    SemanticTypeIdV1::from_index(pointee),
                    kind,
                    SemanticMutabilityV1::Immutable,
                    space,
                    width,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        );
        assert!(!dead_cast_check_v1(&changed_types, &function, &inventory).unwrap());
    }
    let mut statements = function.blocks()[1].statements().to_vec();
    statements[3] = dead_cast_assignment_v1(SemanticCastKindV1::PointerExposeProvenance);
    let changed = indexed_atomic_replace_block_v1(&function, 1, statements);
    assert!(!dead_cast_check_v1(&types, &changed, &inventory).unwrap());
    let mut statements = function.blocks()[1].statements().to_vec();
    statements.push(dead_cast_assignment_v1(SemanticCastKindV1::Pointer));
    let changed = indexed_atomic_replace_block_v1(&function, 1, statements);
    assert!(!dead_cast_check_v1(&types, &changed, &inventory).unwrap());
}

#[test]
fn indexed_atomic_dead_cast_rejects_all_statement_use_families() {
    let (types, function) = dead_cast_fixture_v1();
    let (checks, allocations) = indexed_atomic_inventory_fixture_v1(&types, &function);
    let inventory =
        authenticated_atomic_allocations_v1(&types, &function, &checks, &allocations).unwrap();
    let pointer = dead_cast_place_v1(12, 7);
    let operand = SemanticOperandV1::Copy(pointer.clone());
    let address = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(12),
        vec![
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::Dereference,
                SemanticTypeIdV1::from_index(0),
            )
            .unwrap(),
        ],
        SemanticTypeIdV1::from_index(0),
    )
    .unwrap();
    let rvalues = vec![
        SemanticRvalueKindV1::Use(operand.clone()),
        SemanticRvalueKindV1::Cast {
            kind: SemanticCastKindV1::Pointer,
            operand: operand.clone(),
        },
        SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Shared,
            place: pointer.clone(),
        },
        SemanticRvalueKindV1::AddressOf {
            mutability: SemanticMutabilityV1::Immutable,
            place: pointer.clone(),
        },
        SemanticRvalueKindV1::Length(pointer.clone()),
        SemanticRvalueKindV1::Discriminant(pointer.clone()),
        SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
            address.clone(),
            SemanticVolatilityV1::NonVolatile,
            None,
        )),
    ];
    let mut uses: Vec<_> = rvalues
        .into_iter()
        .map(|value| {
            statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                dead_cast_place_v1(11, 6),
                SemanticRvalueV1::new(SemanticTypeIdV1::from_index(6), value),
            )))
        })
        .collect();
    uses.extend([
        statement(SemanticStatementKindV1::Assume(operand.clone())),
        statement(SemanticStatementKindV1::Deinitialize(pointer.clone())),
        statement(SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
            address,
            typed_constant(SemanticTypeIdV1::from_index(0), 1, 4),
            SemanticVolatilityV1::NonVolatile,
            None,
        ))),
        statement(SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
            dead_cast_place_v1(11, 6),
            operand,
            SemanticVolatilityV1::NonVolatile,
            None,
        ))),
    ]);
    // Even an ill-typed use as another place's index is not silently discarded.
    let projected = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(12)),
                SemanticTypeIdV1::from_index(0),
            )
            .unwrap(),
        ],
        SemanticTypeIdV1::from_index(0),
    )
    .unwrap();
    uses.push(statement(SemanticStatementKindV1::Deinitialize(projected)));
    for usage in uses {
        let mut statements = function.blocks()[2].statements().to_vec();
        statements.push(usage);
        let changed = indexed_atomic_replace_block_v1(&function, 2, statements);
        assert!(!dead_cast_check_v1(&types, &changed, &inventory).unwrap());
        if let Ok(rejected) =
            authenticated_atomic_allocations_v1(&types, &changed, &checks, &allocations)
        {
            // Mutating the source allocation itself can invalidate both roots
            // before the escape check; that must not grant any indexed use.
            assert!(rejected.indexed_uses.is_empty());
        }
    }
}

#[test]
fn indexed_atomic_dead_cast_rejects_all_terminator_use_families() {
    let (types, function) = dead_cast_fixture_v1();
    let (checks, allocations) = indexed_atomic_inventory_fixture_v1(&types, &function);
    let inventory =
        authenticated_atomic_allocations_v1(&types, &function, &checks, &allocations).unwrap();
    let pointer = dead_cast_place_v1(12, 7);
    let operand = SemanticOperandV1::Copy(pointer.clone());
    let next = cfg_edge(SemanticEdgeRoleV1::Goto, 2);
    let messages = vec![
        SemanticAssertMessageV1::BoundsCheck {
            length: operand.clone(),
            index: typed_constant(SemanticTypeIdV1::from_index(8), 0, 8),
        },
        SemanticAssertMessageV1::Overflow {
            operation: SemanticBinaryOpV1::Add,
            left: typed_constant(SemanticTypeIdV1::from_index(8), 0, 8),
            right: operand.clone(),
        },
        SemanticAssertMessageV1::DivisionByZero(operand.clone()),
        SemanticAssertMessageV1::RemainderByZero(operand.clone()),
        SemanticAssertMessageV1::MisalignedPointerDereference {
            required_alignment: operand.clone(),
            found_alignment: typed_constant(SemanticTypeIdV1::from_index(8), 0, 8),
        },
    ];
    let mut terminators: Vec<_> = messages
        .into_iter()
        .map(|message| SemanticTerminatorKindV1::Assert {
            condition: typed_constant(SemanticTypeIdV1::from_index(9), 1, 1),
            expected: true,
            message,
            target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, 2),
            unwind: SemanticUnwindActionV1::Unreachable,
        })
        .collect();
    terminators.extend([
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(0),
                vec![operand.clone()],
                None,
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        ),
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(0),
                vec![],
                Some(SemanticCallDestinationV1::new(
                    pointer.clone(),
                    cfg_edge(SemanticEdgeRoleV1::CallReturn, 2),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        ),
        SemanticTerminatorKindV1::TailCall(
            fe2o3_mir_model::semantic_mir_v1::SemanticDirectTailCallV1::new_callable(
                SemanticCallableIdV1::from_index(0),
                vec![operand.clone()],
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        ),
        SemanticTerminatorKindV1::SwitchInt {
            discriminant: operand.clone(),
            targets: SemanticSwitchTargetsV1::new(
                vec![],
                cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
            )
            .unwrap(),
        },
        SemanticTerminatorKindV1::Drop {
            place: pointer,
            drop_glue: SemanticFunctionIdV1::from_index(0),
            target: next,
            unwind: SemanticUnwindActionV1::Unreachable,
        },
        SemanticTerminatorKindV1::Assert {
            condition: operand,
            expected: true,
            message: SemanticAssertMessageV1::NullPointerDereference,
            target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, 2),
            unwind: SemanticUnwindActionV1::Unreachable,
        },
    ]);
    for terminator in terminators {
        let mut blocks = function.blocks().to_vec();
        blocks[2] = block(242, blocks[2].statements().to_vec(), terminator);
        let changed = dead_cast_function_v1(&function, function.locals().to_vec(), blocks);
        assert!(!dead_cast_check_v1(&types, &changed, &inventory).unwrap());
    }
}
