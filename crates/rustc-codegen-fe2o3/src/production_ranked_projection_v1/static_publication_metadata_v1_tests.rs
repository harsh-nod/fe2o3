fn static_publication_metadata_fixture_v1() -> (
    Vec<SemanticTypeDeclV1>,
    SemanticFunctionDeclV1,
    Vec<SemanticCallableDeclV1>,
    Vec<Option<AllocationContractV1>>,
) {
    let ty = SemanticTypeIdV1::from_index;
    let (types, function, mut callables, mut allocations) = static_publication_fixture_v1();
    let pair = || SemanticAbiPassModeV1::Pair {
        first: SemanticAbiValueAttributesV1::plain(),
        second: SemanticAbiValueAttributesV1::plain(),
    };
    let abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256(bytes(235)),
        SemanticLayoutIdentityV1::from_sha256(bytes(235)),
        SemanticCanonAbiV1::GpuKernel,
        false,
        false,
        vec![
            SemanticAbiValueV1::new(ty(11), pair()),
            SemanticAbiValueV1::new(ty(4), pair()),
            SemanticAbiValueV1::new(
                ty(10),
                SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::plain()),
            ),
            SemanticAbiValueV1::new(ty(11), pair()),
        ],
        SemanticAbiValueV1::new(ty(0), SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![
        SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
        SemanticSourceArgumentOwnershipV1::SharedBorrow,
        SemanticSourceArgumentOwnershipV1::ByValue,
        SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
    ])
    .unwrap();
    let mut locals = function.locals().to_vec();
    locals.extend([
        local(251, ty(11), SemanticLocalRoleV1::Argument(3)),
        local(252, ty(14), SemanticLocalRoleV1::Temporary),
        local(253, ty(8), SemanticLocalRoleV1::Temporary),
        local(254, ty(11), SemanticLocalRoleV1::Temporary),
    ]);
    allocations.resize(locals.len(), None);
    allocations[11] = Some(AllocationContractV1 {
        allocation_origin: 4,
        noalias_class: 5,
        writable: true,
        singleton_object: false,
    });
    callables.push(compiler_intrinsic_callable(
        SemanticCompilerIntrinsicOperationV1::DisjointSliceLen {
            disjoint_slice: ty(11),
            element: ty(10),
            raw_index: ty(8),
            index_space: fe2o3_mir_model::semantic_mir_v1::SemanticDisjointIndexSpaceV1::Index1d,
        },
    ));
    let mut blocks = function.blocks().to_vec();
    let branch = blocks[0].terminator().kind().clone();
    let mut statements = blocks[0].statements().to_vec();
    statements.extend([
        typed_assignment(
            7,
            ty(11),
            SemanticRvalueKindV1::Use(typed_operand(1, ty(11))),
        ),
        typed_assignment(
            9,
            ty(14),
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: typed_place(7, ty(11)),
            },
        ),
    ]);
    blocks[0] = block(
        230,
        statements,
        consumed_read_only_call_v1(2, vec![typed_operand(9, ty(14))], (10, 8), 4),
    );
    blocks.push(block(234, vec![], branch));
    let function = SemanticFunctionDeclV1::new(
        function.identity(),
        function.role(),
        function.item_definition_identity(),
        function.monomorphization_identity(),
        function.generic_type_arguments_identity(),
        function.const_generic_arguments_identity(),
        function.source(),
        abi,
        locals,
        function.entry(),
        blocks,
    )
    .unwrap();
    (types, function, callables, allocations)
}

#[test]
fn static_publication_custody_admits_only_original_argument_metadata_snapshot() {
    let (types, function, callables, allocations) = static_publication_metadata_fixture_v1();
    let source = audit_static_publication_source_v1(&types, &callables, &function, &allocations)
        .unwrap()
        .unwrap();
    assert_eq!(source.payload.local, SemanticLocalIdV1::from_index(1));
    assert_eq!(
        source.metadata_snapshot,
        Some(StaticPublicationMetadataSnapshotV1 {
            local: SemanticLocalIdV1::from_index(7),
            block: 0,
            statement: 1,
        })
    );
}

#[test]
fn static_publication_custody_allows_unrelated_same_type_length() {
    let ty = SemanticTypeIdV1::from_index;
    let (types, function, callables, allocations) = static_publication_metadata_fixture_v1();
    let mut blocks = function.blocks().to_vec();
    let branch = blocks[4].terminator().kind().clone();
    blocks[4] = block(
        234,
        vec![typed_assignment(
            12,
            ty(14),
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: typed_place(11, ty(11)),
            },
        )],
        consumed_read_only_call_v1(2, vec![typed_operand(12, ty(14))], (13, 8), 5),
    );
    blocks.push(block(235, vec![], branch));
    let actual = static_publication_reblock_v1(&function, blocks);
    let source = audit_static_publication_source_v1(&types, &callables, &actual, &allocations)
        .unwrap()
        .unwrap();
    assert_eq!(source.payload.argument, 0);
    assert_eq!(source.flags.argument, 1);
}

#[test]
fn static_publication_custody_metadata_snapshot_rejects_rebinding_and_owning_uses() {
    let ty = SemanticTypeIdV1::from_index;
    let (types, function, callables, allocations) = static_publication_metadata_fixture_v1();
    for mutation in 0..8 {
        let mut blocks = function.blocks().to_vec();
        let mut statements = blocks[0].statements().to_vec();
        match mutation {
            0 => statements.push(statements[1].clone()),
            1 => statements.push(typed_assignment(
                7,
                ty(11),
                SemanticRvalueKindV1::Use(typed_operand(11, ty(11))),
            )),
            2 => statements.push(typed_assignment(
                14,
                ty(11),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Move(typed_place(7, ty(11)))),
            )),
            3 => statements.push(typed_assignment(
                1,
                ty(11),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Move(typed_place(7, ty(11)))),
            )),
            4 => {
                statements[2] = typed_assignment(
                    9,
                    ty(14),
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Mutable,
                        place: typed_place(7, ty(11)),
                    },
                )
            }
            5 => statements.swap(1, 2),
            6 => statements.push(typed_assignment(
                14,
                ty(11),
                SemanticRvalueKindV1::Use(typed_operand(1, ty(11))),
            )),
            _ => statements.push(typed_assignment(
                9,
                ty(14),
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: typed_place(11, ty(11)),
                },
            )),
        }
        blocks[0] = block(230, statements, blocks[0].terminator().kind().clone());
        let actual = static_publication_reblock_v1(&function, blocks);
        assert!(
            audit_static_publication_source_v1(&types, &callables, &actual, &allocations).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn static_publication_custody_metadata_snapshot_rejects_cycles_terminals_and_late_lengths() {
    let ty = SemanticTypeIdV1::from_index;
    let (types, function, callables, allocations) = static_publication_metadata_fixture_v1();
    for mutation in 0..5 {
        let mut blocks = function.blocks().to_vec();
        match mutation {
            0 => blocks[4] = block(234, vec![], zero_switch(4, ty(8), 0, 1)),
            1 => {
                blocks[1] = block(
                    231,
                    vec![],
                    consumed_read_only_call_v1(
                        0,
                        vec![
                            typed_operand(7, ty(11)),
                            typed_operand(2, ty(4)),
                            typed_operand(4, ty(8)),
                            typed_operand(3, ty(10)),
                        ],
                        (5, 13),
                        3,
                    ),
                )
            }
            2 => {
                blocks[3] = block(
                    233,
                    vec![],
                    consumed_read_only_call_v1(2, vec![typed_operand(9, ty(14))], (10, 8), 5),
                );
                blocks.push(block(235, vec![], SemanticTerminatorKindV1::Return));
            }
            3 => {
                let mut statements = blocks[4].statements().to_vec();
                statements.push(statement(SemanticStatementKindV1::Deinitialize(
                    typed_place(7, ty(11)),
                )));
                blocks[4] = block(234, statements, blocks[4].terminator().kind().clone());
            }
            _ => {
                blocks[0] = block(
                    230,
                    blocks[0].statements().to_vec(),
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 4)),
                )
            }
        }
        let actual = static_publication_reblock_v1(&function, blocks);
        assert!(
            audit_static_publication_source_v1(&types, &callables, &actual, &allocations).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn static_publication_custody_metadata_proof_uses_the_shared_work_budget() {
    let (types, function, callables, allocations) = static_publication_metadata_fixture_v1();
    let source = audit_static_publication_source_v1(&types, &callables, &function, &allocations)
        .unwrap()
        .unwrap();
    let mut measured = SemanticAssertProofsV1::new(&types, &function).unwrap();
    let before = measured.work;
    static_publication_metadata_snapshot_v1(&mut measured, &callables, source.payload).unwrap();
    let required = measured.work - before;
    assert!(required > 0);
    for (remaining, accepted) in [(required, true), (required - 1, false)] {
        let mut proofs = SemanticAssertProofsV1::new(&types, &function).unwrap();
        proofs.work = MAX_PROJECTED_LOOP_GRAPH_WORK_V1 - remaining;
        assert_eq!(
            static_publication_metadata_snapshot_v1(&mut proofs, &callables, source.payload)
                .is_ok(),
            accepted
        );
    }
}

#[test]
fn static_publication_custody_allows_exact_original_flag_pointer_metadata() {
    let ty = SemanticTypeIdV1::from_index;
    let (types, function, callables, allocations) = static_publication_fixture_v1();
    let mut blocks = function.blocks().to_vec();
    let mut statements = blocks[0].statements().to_vec();
    statements.push(typed_assignment(
        6,
        ty(8),
        SemanticRvalueKindV1::Unary {
            operation: SemanticUnaryOpV1::PointerMetadata,
            operand: typed_operand(2, ty(4)),
        },
    ));
    blocks[0] = block(230, statements, blocks[0].terminator().kind().clone());
    let actual = static_publication_reblock_v1(&function, blocks);
    let source = audit_static_publication_source_v1(&types, &callables, &actual, &allocations)
        .unwrap()
        .unwrap();
    assert_eq!(source.flags.local, SemanticLocalIdV1::from_index(2));
    assert_eq!(source.flags.argument, 1);
    assert_eq!(source.metadata_snapshot, None);
}

#[test]
fn static_publication_custody_flag_metadata_rejects_alias_type_projection_and_late_uses() {
    let ty = SemanticTypeIdV1::from_index;
    let (types, function, callables, allocations) = static_publication_fixture_v1();
    for mutation in 0..8 {
        let mut blocks = function.blocks().to_vec();
        let at = if mutation == 6 { 3 } else { 0 };
        let mut statements = blocks[at].statements().to_vec();
        let mut operand = typed_operand(2, ty(4));
        let mut result = ty(8);
        if mutation == 0 {
            operand = SemanticOperandV1::Move(typed_place(2, ty(4)));
        }
        if mutation == 1 {
            statements.push(typed_assignment(
                8,
                ty(4),
                SemanticRvalueKindV1::Use(operand),
            ));
            operand = typed_operand(8, ty(4));
        }
        if mutation == 2 {
            operand = typed_operand(1, ty(11));
        }
        if mutation == 3 {
            result = ty(0);
        }
        let projected = || {
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(2),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ty(3))
                        .unwrap(),
                ],
                ty(3),
            )
            .unwrap()
        };
        if mutation == 4 {
            operand = SemanticOperandV1::Copy(projected());
        }
        let value = if mutation == 7 {
            SemanticRvalueKindV1::Length(projected())
        } else {
            SemanticRvalueKindV1::Unary {
                operation: if mutation == 5 {
                    SemanticUnaryOpV1::Not
                } else {
                    SemanticUnaryOpV1::PointerMetadata
                },
                operand,
            }
        };
        statements.push(typed_assignment(6, result, value));
        blocks[at] = block(
            230 + at as u8,
            statements,
            blocks[at].terminator().kind().clone(),
        );
        let actual = static_publication_reblock_v1(&function, blocks);
        assert!(
            audit_static_publication_source_v1(&types, &callables, &actual, &allocations).is_err(),
            "mutation {mutation}"
        );
    }
}
