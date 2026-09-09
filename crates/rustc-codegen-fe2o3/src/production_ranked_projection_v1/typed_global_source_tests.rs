fn typed_global_source_fixture_v1() -> (
    Vec<SemanticTypeDeclV1>,
    Vec<SemanticCallableDeclV1>,
    SemanticFunctionDeclV1,
) {
    let (mut types, mut callables, function) = typed_global_projection_fixture_v1();
    let read_view = function.locals()[11].ty();
    let write_view = function.locals()[14].ty();
    let physical_read = function.locals()[12].ty();
    let physical_write = function.locals()[13].ty();
    let option = function.locals()[17].ty();
    for view in [read_view, write_view] {
        let declaration = &types[view.index() as usize];
        types[view.index() as usize] = SemanticTypeDeclV1::new(
            declaration.identity(),
            declaration.layout_identity(),
            SemanticTypeLayoutV1::aggregate(
                Some(16),
                8,
                SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
            )
            .unwrap(),
            declaration.shape().clone(),
        );
    }
    let mut reference = |pointee, tag| {
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
    let view_reference = reference(write_view, 225);
    let physical_reference = reference(physical_write, 226);
    callables.push(capability_index_callable(
        SemanticCompilerIntrinsicOperationV1::WriteOnlyDisjointSliceLen {
            disjoint_slice: physical_write,
            element: U64_TYPE,
            raw_index: U64_TYPE,
            index_space: SemanticDisjointIndexSpaceV1::Index1d,
        },
        &[(
            physical_reference,
            SemanticSourceArgumentOwnershipV1::SharedBorrow,
        )],
        U64_TYPE,
    ));
    let mut locals = function.locals().to_vec();
    assert_eq!(locals.len(), 22);
    for (index, ty) in [
        view_reference,
        physical_reference,
        U64_TYPE,
        U64_TYPE,
        BOOL_TYPE,
        U64_TYPE,
        U64_TYPE,
        U64_TYPE,
        function.locals()[16].ty(),
        physical_read,
        U64_TYPE,
        U64_TYPE,
    ]
    .into_iter()
    .enumerate()
    {
        locals.push(local(220 + index as u8, ty, SemanticLocalRoleV1::Temporary));
    }
    let field = |local, view, physical| {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(local),
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, view).unwrap(),
                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), physical).unwrap(),
            ],
            physical,
        )
        .unwrap()
    };
    let shared = |destination, ty, place| {
        typed_assignment(
            destination,
            ty,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place,
            },
        )
    };
    let transfer = |destination, ty, source| {
        typed_assignment(
            destination,
            ty,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(typed_place(source, ty))),
        )
    };
    let mut blocks = function.blocks().to_vec();
    let SemanticTerminatorKindV1::Call(bind) = blocks[5].terminator().kind() else {
        unreachable!();
    };
    blocks[5] = block(
        195,
        vec![],
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                bind.callee(),
                bind.arguments().to_vec(),
                Some(SemanticCallDestinationV1::new(
                    bind.destination().unwrap().place().clone(),
                    cfg_edge(SemanticEdgeRoleV1::CallReturn, 9),
                )),
                bind.unwind(),
            )
            .unwrap(),
        ),
    );
    let original_store = blocks[7].clone();
    blocks[7] = block(
        197,
        vec![
            transfer(18, option, 17),
            typed_assignment(
                27,
                U64_TYPE,
                SemanticRvalueKindV1::Discriminant(typed_place(18, option)),
            ),
        ],
        zero_switch(27, U64_TYPE, 12, 11),
    );
    blocks.push(block(
        215,
        vec![
            shared(22, view_reference, typed_place(14, write_view)),
            shared(
                23,
                physical_reference,
                field(22, write_view, physical_write),
            ),
            shared(30, function.locals()[16].ty(), typed_place(11, read_view)),
            typed_assignment(
                31,
                physical_read,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(field(
                    30,
                    read_view,
                    physical_read,
                ))),
            ),
            typed_assignment(
                32,
                U64_TYPE,
                SemanticRvalueKindV1::Unary {
                    operation: SemanticUnaryOpV1::PointerMetadata,
                    operand: typed_operand(31, physical_read),
                },
            ),
        ],
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(9),
                vec![typed_operand(23, physical_reference)],
                Some(SemanticCallDestinationV1::new(
                    typed_place(24, U64_TYPE),
                    cfg_edge(SemanticEdgeRoleV1::CallReturn, 10),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        ),
    ));
    blocks.push(block(
        216,
        vec![
            transfer(25, U64_TYPE, 24),
            statement(SemanticStatementKindV1::StorageDead(
                SemanticLocalIdV1::from_index(24),
            )),
            typed_assignment(
                26,
                BOOL_TYPE,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::LessThan,
                    left: typed_operand(6, U64_TYPE),
                    right: typed_operand(25, U64_TYPE),
                },
            ),
        ],
        zero_switch(26, BOOL_TYPE, 12, 6),
    ));
    let payload = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(18),
        vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Downcast(1), option).unwrap(),
            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), U64_TYPE).unwrap(),
        ],
        U64_TYPE,
    )
    .unwrap();
    let mut statements = vec![
        typed_assignment(
            28,
            U64_TYPE,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(payload)),
        ),
        typed_assignment(
            29,
            U64_TYPE,
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::Add,
                left: typed_operand(28, U64_TYPE),
                right: typed_constant(U64_TYPE, 7, 8),
            },
        ),
    ];
    statements.extend_from_slice(&original_store.statements()[1..]);
    let SemanticTerminatorKindV1::Call(store) = original_store.terminator().kind() else {
        unreachable!();
    };
    let mut arguments = store.arguments().to_vec();
    arguments[2] = typed_operand(29, U64_TYPE);
    blocks.push(
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256(bytes(218)),
            original_store.source(),
            statements,
            SemanticTerminatorV1::new(
                original_store.terminator().source(),
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new_callable(
                        store.callee(),
                        arguments,
                        store.destination().cloned(),
                        store.unwind(),
                    )
                    .unwrap(),
                ),
            ),
        )
        .unwrap(),
    );
    blocks.push(block(217, vec![], SemanticTerminatorKindV1::Return));
    (
        types,
        callables,
        typed_global_fixture_with_body_v1(&function, locals, blocks),
    )
}

fn typed_global_ranked_source_fixture_v1(
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    function: &SemanticFunctionDeclV1,
) -> (
    IntrinsicProjectionV1,
    Vec<ProductionRankedBlockV1>,
    Vec<ProjectedAccessSourceV1>,
) {
    let (projection, operations) =
        project_capability_index_fixture(types, callables, function).unwrap();
    let projected_blocks = function
        .blocks()
        .iter()
        .enumerate()
        .map(|(block, _)| {
            let items = [
                &projection.direct_read_effects,
                &projection.direct_write_effects,
            ]
            .into_iter()
            .filter_map(|effects| {
                let mut access = effects[block].clone()?;
                access.semantic_site = Some(ProjectedSemanticAccessSiteV1 {
                    block,
                    statement: None,
                });
                Some(ProjectedBlockItemV1::Guarded(access))
            })
            .collect();
            ProjectedSemanticBlockV1 { items }
        })
        .collect();
    let predicates = switch_predicates(
        function,
        &projection.option_predicates,
        &projection.direct_switch_predicates,
    )
    .unwrap();
    let (blocks, sources, _) = build_ranked_cfg(
        types,
        function,
        callables,
        &predicates,
        &projection.deterministic_switches,
        &projection.uniform_inductions,
        operations,
        projected_blocks,
    )
    .unwrap();
    (projection, blocks, sources)
}

#[test]
fn typed_global_source_preserves_helper_extent_and_load_store_sites() {
    let (types, callables, function) = typed_global_source_fixture_v1();
    let (projection, blocks, sources) =
        typed_global_ranked_source_fixture_v1(&types, &callables, &function);
    assert_eq!(
        projection.direct_switch_predicates[26],
        Some(GuardPredicateV1::for_access(
            projection.direct_write_effects[11].as_ref().unwrap()
        ))
    );
    assert_eq!(
        projection.direct_switch_predicates[27],
        Some(GuardPredicateV1::for_access(
            projection.direct_read_effects[6].as_ref().unwrap()
        ))
    );
    let writes = projected_reference_gpu_writes_v2(
        &types,
        &callables,
        &function,
        &projection,
        &blocks,
        &sources,
    )
    .unwrap();
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].allocation_origin, 2);
    let write_source = sources
        .iter()
        .find(|source| source.access == AccessKindAttr::Write)
        .unwrap();
    assert_eq!(
        write_source.semantic_site,
        Some(ProjectedSemanticAccessSiteV1 {
            block: 11,
            statement: None
        })
    );
    let ProductionSemanticExpressionV2::Binary {
        operation: ProductionSemanticBinaryOpV2::Add,
        lhs,
        ..
    } = writes[0].value.as_ref().unwrap()
    else {
        panic!("source store arithmetic was not retained");
    };
    let ProductionSemanticExpressionV2::Load(load) = lhs.as_ref() else {
        panic!("source load was not retained");
    };
    let read_source = sources
        .iter()
        .find(|source| source.access == AccessKindAttr::Read)
        .unwrap();
    assert_eq!(
        read_source.semantic_site,
        Some(ProjectedSemanticAccessSiteV1 {
            block: 6,
            statement: None
        })
    );
    assert_eq!(
        (
            load.block as usize,
            load.operation as usize,
            load.allocation_origin
        ),
        (read_source.block, read_source.operation, 1)
    );
}

#[test]
fn typed_global_source_effects_do_not_depend_on_mir_storage_addresses() {
    let (types, callables, function) = typed_global_source_fixture_v1();
    let cloned = function.clone();
    assert_ne!(function.blocks().as_ptr(), cloned.blocks().as_ptr());
    let extract = |function: &SemanticFunctionDeclV1| {
        let (projection, blocks, sources) =
            typed_global_ranked_source_fixture_v1(&types, &callables, function);
        let writes = projected_reference_gpu_writes_v2(
            &types,
            &callables,
            function,
            &projection,
            &blocks,
            &sources,
        )
        .unwrap();
        (blocks, sources, writes)
    };
    assert_eq!(extract(&function), extract(&cloned));
}

#[test]
fn typed_global_source_length_variants_require_exact_physical_contract() {
    for write_only in [false, true] {
        for axis in 0..5 {
            let (types, mut callables, function) = typed_global_source_fixture_v1();
            let disjoint_slice = function.locals()[if axis == 1 { 12 } else { 13 }].ty();
            let element = if axis == 2 { BOOL_TYPE } else { U64_TYPE };
            let raw_index = if axis == 3 { BOOL_TYPE } else { U64_TYPE };
            let index_space = if axis == 4 {
                SemanticDisjointIndexSpaceV1::ShiftedIndex1d { offset: 1 }
            } else {
                SemanticDisjointIndexSpaceV1::Index1d
            };
            let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = &mut callables[9]
            else {
                unreachable!();
            };
            *operation = if write_only {
                SemanticCompilerIntrinsicOperationV1::WriteOnlyDisjointSliceLen {
                    disjoint_slice,
                    element,
                    raw_index,
                    index_space,
                }
            } else {
                SemanticCompilerIntrinsicOperationV1::DisjointSliceLen {
                    disjoint_slice,
                    element,
                    raw_index,
                    index_space,
                }
            };
            let (projection, _) =
                project_capability_index_fixture(&types, &callables, &function).unwrap();
            let write = projection.direct_write_effects[11].as_ref().unwrap();
            assert_eq!(
                projection.direct_switch_predicates[26],
                (axis == 0).then(|| GuardPredicateV1::for_access(write)),
                "write_only {write_only}, axis {axis}"
            );
            assert_eq!(
                projection.global_uses.comparisons.contains_key(&(10, 2)),
                axis == 0
            );
        }
    }
}

fn typed_global_source_alias_mutation_v1(
    types: &mut Vec<SemanticTypeDeclV1>,
    locals: &mut Vec<SemanticLocalDeclV1>,
    target: u32,
    value: SemanticOperandV1,
    kind: SemanticPointerKindV1,
) -> [SemanticStatementV1; 2] {
    let pointee = value.ty();
    assert_eq!(locals[target as usize].ty(), pointee);
    let pointer = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(240)),
        SemanticLayoutIdentityV1::from_sha256(bytes(240)),
        SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                pointee,
                kind,
                SemanticMutabilityV1::Mutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    ));
    let alias = locals.len() as u32;
    locals.push(local(240, pointer, SemanticLocalRoleV1::Temporary));
    let place = typed_place(target, pointee);
    let address = match kind {
        SemanticPointerKindV1::Reference => SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Mutable,
            place,
        },
        SemanticPointerKindV1::Raw => SemanticRvalueKindV1::AddressOf {
            mutability: SemanticMutabilityV1::Mutable,
            place,
        },
    };
    [
        typed_assignment(alias, pointer, address),
        statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(alias),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, pointee)
                        .unwrap(),
                ],
                pointee,
            )
            .unwrap(),
            SemanticRvalueV1::new(pointee, SemanticRvalueKindV1::Use(value)),
        ))),
    ]
}

#[test]
fn typed_global_source_invalidated_scalars_never_recover_assignment_values() {
    for kind in [SemanticPointerKindV1::Reference, SemanticPointerKindV1::Raw] {
        // Direct payload, an arithmetic operand, and an arithmetic result must
        // reject stale recovery. A value copied before the escape remains valid.
        for (target, consumed, insert_at, accepted) in [
            (28, 28, 2, false),
            (28, 29, 1, false),
            (29, 29, 2, false),
            (28, 29, 2, true),
        ] {
            let (mut types, callables, function) = typed_global_source_fixture_v1();
            let mut locals = function.locals().to_vec();
            let mutation = typed_global_source_alias_mutation_v1(
                &mut types,
                &mut locals,
                target,
                typed_constant(U64_TYPE, 19, 8),
                kind,
            );
            let mut body = function.blocks().to_vec();
            let original = &body[11];
            let mut statements = original.statements()[..insert_at].to_vec();
            statements.extend(mutation);
            statements.extend_from_slice(&original.statements()[insert_at..]);
            let SemanticTerminatorKindV1::Call(store) = original.terminator().kind() else {
                unreachable!();
            };
            let mut arguments = store.arguments().to_vec();
            arguments[2] = typed_operand(consumed, U64_TYPE);
            body[11] = SemanticBasicBlockV1::new(
                original.identity(),
                original.source(),
                statements,
                SemanticTerminatorV1::new(
                    original.terminator().source(),
                    SemanticTerminatorKindV1::Call(
                        SemanticDirectCallV1::new_callable(
                            store.callee(),
                            arguments,
                            store.destination().cloned(),
                            store.unwind(),
                        )
                        .unwrap(),
                    ),
                ),
            )
            .unwrap();
            let changed = typed_global_fixture_with_body_v1(&function, locals, body);
            let (projection, ranked, sources) =
                typed_global_ranked_source_fixture_v1(&types, &callables, &changed);
            let writes = projected_reference_gpu_writes_v2(
                &types,
                &callables,
                &changed,
                &projection,
                &ranked,
                &sources,
            )
            .unwrap();
            assert_eq!(writes.len(), 1);
            if accepted {
                assert!(matches!(
                    writes[0].value,
                    Ok(ProductionSemanticExpressionV2::Binary { .. })
                ));
            } else {
                assert_eq!(
                    writes[0].value.as_ref().unwrap_err(),
                    &"GPU semantic scalar local escapes through a mutable reference or address",
                    "target {target}, consumed {consumed}, insert_at {insert_at}, kind {kind:?}"
                );
            }
        }
    }
}

#[test]
fn typed_global_source_rebound_physical_slice_cannot_retain_allocation_extent() {
    for kind in [SemanticPointerKindV1::Reference, SemanticPointerKindV1::Raw] {
        for mutate in [false, true] {
            let (mut types, callables, function) = typed_global_source_fixture_v1();
            let physical = function.locals()[31].ty();
            let SemanticTypeShapeV1::Pointer(pointer) = types[physical.index() as usize].shape()
            else {
                unreachable!();
            };
            let slice = pointer.pointee();
            let mut locals = function.locals().to_vec();
            let mut body = function.blocks().to_vec();
            if mutate {
                let replacement = locals.len() as u32;
                locals.push(local(239, physical, SemanticLocalRoleV1::Temporary));
                let mut mutation = vec![typed_assignment(
                    replacement,
                    physical,
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Shared,
                        place: SemanticPlaceV1::new(
                            SemanticLocalIdV1::from_index(12),
                            vec![
                                SemanticProjectionV1::new(
                                    SemanticProjectionKindV1::Dereference,
                                    slice,
                                )
                                .unwrap(),
                                SemanticProjectionV1::new(
                                    SemanticProjectionKindV1::Subslice {
                                        from: 0,
                                        to: 0,
                                        from_end: false,
                                    },
                                    slice,
                                )
                                .unwrap(),
                            ],
                            slice,
                        )
                        .unwrap(),
                    },
                )];
                mutation.extend(typed_global_source_alias_mutation_v1(
                    &mut types,
                    &mut locals,
                    31,
                    typed_operand(replacement, physical),
                    kind,
                ));
                let original = &body[9];
                let mut statements = original.statements()[..4].to_vec();
                statements.extend(mutation);
                statements.extend_from_slice(&original.statements()[4..]);
                body[9] = SemanticBasicBlockV1::new(
                    original.identity(),
                    original.source(),
                    statements,
                    original.terminator().clone(),
                )
                .unwrap();
            }
            let original = &body[10];
            let mut statements = original.statements()[..2].to_vec();
            statements.push(typed_assignment(
                26,
                BOOL_TYPE,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::LessThan,
                    left: typed_operand(6, U64_TYPE),
                    right: typed_operand(32, U64_TYPE),
                },
            ));
            body[10] = SemanticBasicBlockV1::new(
                original.identity(),
                original.source(),
                statements,
                original.terminator().clone(),
            )
            .unwrap();
            let changed = typed_global_fixture_with_body_v1(&function, locals, body);
            let (projection, _) =
                project_capability_index_fixture(&types, &callables, &changed).unwrap();
            assert!(projection.direct_write_effects[11].is_some());
            let read = projection.direct_read_effects[6].as_ref().unwrap();
            assert_eq!(
                projection.direct_switch_predicates[26],
                (!mutate).then(|| GuardPredicateV1::for_access(read)),
                "kind {kind:?}, mutate {mutate}"
            );
            assert_eq!(
                projection.global_uses.comparisons.contains_key(&(10, 2)),
                !mutate
            );
        }
    }
}

#[test]
fn typed_global_source_guard_drift_never_becomes_output_bounds() {
    let (types, callables, function) = typed_global_source_fixture_v1();
    for axis in 0..4 {
        let mut blocks = function.blocks().to_vec();
        let original = &blocks[10];
        let mut statements = original.statements()[..2].to_vec();
        let mut left = typed_operand(6, U64_TYPE);
        let mut right = typed_operand(25, U64_TYPE);
        let mut operation = SemanticBinaryOpV1::LessThan;
        match axis {
            0 => right = typed_operand(32, U64_TYPE),
            1 => {
                statements.push(typed_assignment(
                    33,
                    U64_TYPE,
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::Add,
                        left: left.clone(),
                        right: typed_constant(U64_TYPE, 1, 8),
                    },
                ));
                left = typed_operand(33, U64_TYPE);
            }
            2 => operation = SemanticBinaryOpV1::LessOrEqual,
            _ => statements.push(typed_assignment(
                25,
                U64_TYPE,
                SemanticRvalueKindV1::Use(typed_constant(U64_TYPE, 1, 8)),
            )),
        }
        statements.push(typed_assignment(
            26,
            BOOL_TYPE,
            SemanticRvalueKindV1::Binary {
                operation,
                left,
                right,
            },
        ));
        blocks[10] = SemanticBasicBlockV1::new(
            original.identity(),
            original.source(),
            statements,
            original.terminator().clone(),
        )
        .unwrap();
        let changed =
            typed_global_fixture_with_body_v1(&function, function.locals().to_vec(), blocks);
        let (projection, _) =
            project_capability_index_fixture(&types, &callables, &changed).unwrap();
        let output =
            GuardPredicateV1::for_access(projection.direct_write_effects[11].as_ref().unwrap());
        assert_ne!(
            projection.direct_switch_predicates[26],
            Some(output),
            "mutation {axis}"
        );
        if axis == 0 {
            assert_eq!(
                projection.direct_switch_predicates[26],
                Some(GuardPredicateV1::for_access(
                    projection.direct_read_effects[6].as_ref().unwrap()
                ))
            );
        } else {
            assert!(projection.direct_switch_predicates[26].is_none());
        }
    }
}

#[test]
fn typed_global_source_missing_some_guard_rejects_payload_expression() {
    let (types, callables, function) = typed_global_source_fixture_v1();
    let mut blocks = function.blocks().to_vec();
    let original = &blocks[7];
    blocks[7] = SemanticBasicBlockV1::new(
        original.identity(),
        original.source(),
        original.statements().to_vec(),
        SemanticTerminatorV1::new(
            original.terminator().source(),
            SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 11)),
        ),
    )
    .unwrap();
    let changed = typed_global_fixture_with_body_v1(&function, function.locals().to_vec(), blocks);
    let (projection, ranked, sources) =
        typed_global_ranked_source_fixture_v1(&types, &callables, &changed);
    let writes = projected_reference_gpu_writes_v2(
        &types,
        &callables,
        &changed,
        &projection,
        &ranked,
        &sources,
    )
    .unwrap();
    assert_eq!(
        writes[0].value.as_ref().unwrap_err(),
        &"GPU semantic scalar operand uses an unsupported place projection"
    );
}

#[test]
fn typed_global_source_rejects_ranked_allocation_and_site_substitution() {
    let (types, callables, function) = typed_global_source_fixture_v1();
    let (projection, blocks, sources) =
        typed_global_ranked_source_fixture_v1(&types, &callables, &function);
    let mut changed = sources.clone();
    let source = changed
        .iter_mut()
        .find(|source| source.access == AccessKindAttr::Write)
        .unwrap();
    source.semantic_site = Some(ProjectedSemanticAccessSiteV1 {
        block: 6,
        statement: None,
    });
    assert!(
        projected_reference_gpu_writes_v2(
            &types,
            &callables,
            &function,
            &projection,
            &blocks,
            &changed
        )
        .unwrap()[0]
            .value
            .is_err()
    );
    for writable in [false, true] {
        let mut changed = blocks.clone();
        for block in &mut changed {
            let mut operations = block.operations().to_vec();
            for operation in &mut operations {
                if let ProductionRankedOperationV1::ViewInSpace {
                    allocation_origin,
                    writable: actual,
                    ..
                } = operation
                    && *actual == writable
                {
                    *allocation_origin = 99;
                }
            }
            *block = ProductionRankedBlockV1::new(operations, block.terminator().clone());
        }
        let result = projected_reference_gpu_writes_v2(
            &types,
            &callables,
            &function,
            &projection,
            &changed,
            &sources,
        );
        if writable {
            assert!(result.unwrap()[0].value.is_err());
        } else {
            assert!(result.is_err());
        }
    }
}
