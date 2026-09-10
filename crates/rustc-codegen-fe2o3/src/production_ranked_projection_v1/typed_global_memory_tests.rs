fn typed_global_memory_call_v1(
    callee: u32,
    arguments: Vec<SemanticOperandV1>,
    destination: u32,
    ty: SemanticTypeIdV1,
    target: u32,
) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(callee),
            arguments,
            Some(SemanticCallDestinationV1::new(
                typed_place(destination, ty),
                cfg_edge(SemanticEdgeRoleV1::CallReturn, target),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    )
}

fn typed_global_memory_fixture_v1() -> (
    Vec<SemanticTypeDeclV1>,
    Vec<SemanticCallableDeclV1>,
    SemanticFunctionDeclV1,
) {
    let (mut types, mut callables, function) = typed_global_exclusive_source_fixture_v1(true);
    let view = function.locals()[11].ty();
    let shared = function.locals()[16].ty();
    let option = function.locals()[17].ty();
    let mutable = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(130)),
        SemanticLayoutIdentityV1::from_sha256(bytes(130)),
        SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                view,
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Mutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    ));
    callables[8] = capability_index_callable(
        SemanticCompilerIntrinsicOperationV1::CapabilityGlobalExclusiveStore {
            view,
            index: U64_TYPE,
            element: U64_TYPE,
            result: BOOL_TYPE,
            contract: SemanticCapabilityMemoryContractV1::global_exclusive_read_write(),
            provenance: capability_index_provenance(None),
            source_identity: SemanticFunctionIdentityV1::from_sha256(bytes(116)),
        },
        &[
            (mutable, SemanticSourceArgumentOwnershipV1::UniqueBorrow),
            (U64_TYPE, SemanticSourceArgumentOwnershipV1::ByValue),
            (U64_TYPE, SemanticSourceArgumentOwnershipV1::ByValue),
        ],
        BOOL_TYPE,
    );
    let mut locals = function.locals().to_vec();
    locals[20] = local(131, mutable, SemanticLocalRoleV1::Temporary);
    assert_eq!(locals.len(), 34);
    for (offset, ty) in [
        shared, option, U64_TYPE, U64_TYPE, U64_TYPE, mutable, BOOL_TYPE,
    ]
    .into_iter()
    .enumerate()
    {
        locals.push(local(
            132 + offset as u8,
            ty,
            SemanticLocalRoleV1::Temporary,
        ));
    }
    let borrow = |destination, ty, kind| {
        typed_assignment(
            destination,
            ty,
            SemanticRvalueKindV1::Borrow {
                kind,
                place: typed_place(15, view),
            },
        )
    };
    let mut blocks = function.blocks().to_vec();
    let mut statements = blocks[11].statements()[..2].to_vec();
    statements.push(borrow(20, mutable, SemanticBorrowKindV1::Mutable));
    blocks[11] = block(
        218,
        statements,
        typed_global_memory_call_v1(
            8,
            vec![
                SemanticOperandV1::Move(typed_place(20, mutable)),
                typed_operand(6, U64_TYPE),
                typed_operand(29, U64_TYPE),
            ],
            21,
            BOOL_TYPE,
            13,
        ),
    );
    assert_eq!(blocks.len(), 13);
    blocks.push(block(
        130,
        vec![borrow(34, shared, SemanticBorrowKindV1::Shared)],
        typed_global_memory_call_v1(
            7,
            vec![typed_operand(34, shared), typed_operand(6, U64_TYPE)],
            35,
            option,
            14,
        ),
    ));
    blocks.push(block(
        131,
        vec![typed_assignment(
            36,
            U64_TYPE,
            SemanticRvalueKindV1::Discriminant(typed_place(35, option)),
        )],
        zero_switch(36, U64_TYPE, 12, 15),
    ));
    let payload = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(35),
        vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Downcast(1), option).unwrap(),
            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), U64_TYPE).unwrap(),
        ],
        U64_TYPE,
    )
    .unwrap();
    blocks.push(block(
        132,
        vec![
            typed_assignment(
                37,
                U64_TYPE,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(payload)),
            ),
            typed_assignment(
                38,
                U64_TYPE,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::Add,
                    left: typed_operand(37, U64_TYPE),
                    right: typed_constant(U64_TYPE, 1, 8),
                },
            ),
            borrow(39, mutable, SemanticBorrowKindV1::Mutable),
        ],
        typed_global_memory_call_v1(
            8,
            vec![
                SemanticOperandV1::Move(typed_place(39, mutable)),
                typed_operand(6, U64_TYPE),
                typed_operand(38, U64_TYPE),
            ],
            40,
            BOOL_TYPE,
            8,
        ),
    ));
    let function = typed_global_fixture_with_body_v1(&function, locals, blocks);
    (types, callables, function)
}

#[test]
fn typed_global_memory_read_write_read_uses_the_reaching_store_rhs() {
    let (types, callables, function) = typed_global_memory_fixture_v1();
    let (projection, blocks, sources) =
        typed_global_ranked_source_fixture_v1(&types, &callables, &function);
    let writes = projected_reference_gpu_writes_v2(
        &types,
        &callables,
        &function,
        &projection,
        &blocks,
        &sources,
    )
    .unwrap();
    assert_eq!(writes.len(), 2);
    let first = writes
        .iter()
        .find(|write| {
            sources.iter().any(|source| {
                source.block == write.block
                    && source.operation == write.operation
                    && source.semantic_site.is_some_and(|site| site.block == 11)
            })
        })
        .unwrap();
    let second = writes
        .iter()
        .find(|write| {
            sources.iter().any(|source| {
                source.block == write.block
                    && source.operation == write.operation
                    && source.semantic_site.is_some_and(|site| site.block == 15)
            })
        })
        .unwrap();
    let ProductionSemanticExpressionV2::Binary { lhs, rhs, .. } = second.value.as_ref().unwrap()
    else {
        panic!("missing final addition");
    };
    assert_eq!(lhs.as_ref(), first.value.as_ref().unwrap());
    assert!(matches!(
        rhs.as_ref(),
        ProductionSemanticExpressionV2::Constant { bits: 1, .. }
    ));
    let mut work = 0;
    let versions = typed_global_memory_versions_v1(
        &function,
        &callables,
        &projection,
        &blocks,
        &sources,
        &mut work,
    )
    .unwrap();
    for source in sources
        .iter()
        .filter(|source| source.access == AccessKindAttr::Read)
    {
        assert_eq!(
            versions[&(source.block, source.operation)],
            if source.semantic_site.unwrap().block == 6 {
                TypedGlobalMemoryVersionV1::Initial
            } else {
                TypedGlobalMemoryVersionV1::Store { semantic_block: 11 }
            }
        );
    }
    let mut repeated_work = 0;
    assert_eq!(
        versions,
        typed_global_memory_versions_v1(
            &function,
            &callables,
            &projection,
            &blocks,
            &sources,
            &mut repeated_work
        )
        .unwrap()
    );
    assert_eq!(work, repeated_work);
}

#[test]
fn typed_global_memory_reordered_load_does_not_read_a_future_store() {
    let (types, callables, function) = typed_global_memory_fixture_v1();
    let mut body = function.blocks().to_vec();
    let SemanticTerminatorKindV1::Call(store) = body[11].terminator().kind() else {
        unreachable!();
    };
    let SemanticTerminatorKindV1::Call(load) = body[13].terminator().kind() else {
        unreachable!();
    };
    let store = store.clone();
    let load = load.clone();
    let mut before = body[11].statements()[..2].to_vec();
    let store_borrow = body[11].statements()[2].clone();
    before.extend_from_slice(body[13].statements());
    body[11] = block(
        218,
        before,
        typed_global_memory_call_v1(
            7,
            load.arguments().to_vec(),
            35,
            function.locals()[35].ty(),
            14,
        ),
    );
    body[13] = block(
        130,
        vec![store_borrow],
        typed_global_memory_call_v1(8, store.arguments().to_vec(), 21, BOOL_TYPE, 15),
    );
    body[14] = block(
        131,
        body[14].statements().to_vec(),
        zero_switch(36, U64_TYPE, 12, 13),
    );
    let changed = typed_global_fixture_with_body_v1(&function, function.locals().to_vec(), body);
    let (projection, blocks, sources) =
        typed_global_ranked_source_fixture_v1(&types, &callables, &changed);
    let versions = typed_global_memory_versions_v1(
        &changed,
        &callables,
        &projection,
        &blocks,
        &sources,
        &mut 0,
    )
    .unwrap();
    assert!(
        versions
            .values()
            .all(|version| *version == TypedGlobalMemoryVersionV1::Initial)
    );
    let writes = projected_reference_gpu_writes_v2(
        &types,
        &callables,
        &changed,
        &projection,
        &blocks,
        &sources,
    )
    .unwrap();
    let final_write = writes
        .iter()
        .find(|write| {
            sources.iter().any(|source| {
                source.block == write.block
                    && source.operation == write.operation
                    && source.semantic_site.is_some_and(|site| site.block == 15)
            })
        })
        .unwrap();
    let ProductionSemanticExpressionV2::Binary { lhs, .. } = final_write.value.as_ref().unwrap()
    else {
        panic!("missing final addition");
    };
    assert!(
        matches!(lhs.as_ref(), ProductionSemanticExpressionV2::Load(_)),
        "a future store must not be substituted into the earlier load"
    );
}

#[test]
fn typed_global_memory_bypass_and_overlapping_index_fail_closed() {
    for overlap in [false, true] {
        let (types, callables, function) = typed_global_memory_fixture_v1();
        let mut body = function.blocks().to_vec();
        let mut locals = function.locals().to_vec();
        if overlap {
            let local_index = locals.len() as u32;
            locals.push(local(139, U64_TYPE, SemanticLocalRoleV1::Temporary));
            let mut statements = body[13].statements().to_vec();
            statements.push(typed_assignment(
                local_index,
                U64_TYPE,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::Add,
                    left: typed_operand(6, U64_TYPE),
                    right: typed_constant(U64_TYPE, 1, 8),
                },
            ));
            body[13] = block(
                130,
                statements,
                typed_global_memory_call_v1(
                    7,
                    vec![
                        typed_operand(34, locals[34].ty()),
                        typed_operand(local_index, U64_TYPE),
                    ],
                    35,
                    locals[35].ty(),
                    14,
                ),
            );
        } else {
            body[7] = block(
                197,
                body[7].statements().to_vec(),
                zero_switch(27, U64_TYPE, 12, 16),
            );
            body.push(block(133, vec![], zero_switch(6, U64_TYPE, 13, 11)));
        }
        let changed = typed_global_fixture_with_body_v1(&function, locals, body);
        let (projection, blocks, sources) =
            typed_global_ranked_source_fixture_v1(&types, &callables, &changed);
        assert!(
            projected_reference_gpu_writes_v2(
                &types,
                &callables,
                &changed,
                &projection,
                &blocks,
                &sources
            )
            .is_err()
        );
    }
}

#[test]
fn typed_global_memory_rejects_missing_guard_cycles_unknown_aliases_and_barriers() {
    let (types, callables, function) = typed_global_memory_fixture_v1();
    let (projection, blocks, sources) =
        typed_global_ranked_source_fixture_v1(&types, &callables, &function);
    let read = sources
        .iter()
        .find(|source| {
            source.access == AccessKindAttr::Read && source.semantic_site.unwrap().block == 6
        })
        .unwrap();
    for axis in 0..4 {
        let mut changed = blocks.clone();
        match axis {
            0 => {
                let predecessor = changed
                    .iter()
                    .position(|block| {
                        matches!(block.terminator(),
                    ProductionRankedTerminatorV1::IndexLessThan { true_block, .. }
                        if *true_block as usize == read.block)
                    })
                    .unwrap();
                let ProductionRankedTerminatorV1::IndexLessThan {
                    lhs,
                    rhs,
                    true_block,
                    ..
                } = *changed[predecessor].terminator()
                else {
                    unreachable!();
                };
                changed[predecessor] = ProductionRankedBlockV1::new(
                    changed[predecessor].operations().to_vec(),
                    ProductionRankedTerminatorV1::IndexLessThan {
                        lhs,
                        rhs,
                        true_block,
                        false_block: true_block,
                    },
                );
            }
            1 => {
                let exit = changed
                    .iter()
                    .position(|block| {
                        matches!(block.terminator(), ProductionRankedTerminatorV1::Return)
                    })
                    .unwrap();
                changed[exit] = ProductionRankedBlockV1::new(
                    changed[exit].operations().to_vec(),
                    ProductionRankedTerminatorV1::Branch { target: 0 },
                );
            }
            2 => {
                let mut operations = changed[0].operations().to_vec();
                for operation in &mut operations {
                    match operation {
                        ProductionRankedOperationV1::View { noalias_class, .. }
                        | ProductionRankedOperationV1::ViewInSpace { noalias_class, .. } => {
                            *noalias_class = 0
                        }
                        _ => {}
                    }
                }
                changed[0] =
                    ProductionRankedBlockV1::new(operations, changed[0].terminator().clone());
            }
            _ => {
                let mut operations = changed[0].operations().to_vec();
                operations.push(ProductionRankedOperationV1::Barrier {
                    execution_scope: HierarchyAttr::Workgroup,
                    memory_scope: MemoryScopeAttr::Workgroup,
                    address_space: AddressSpaceAttr::Workgroup,
                    order: MemoryOrderAttr::AcquireRelease,
                });
                changed[0] =
                    ProductionRankedBlockV1::new(operations, changed[0].terminator().clone());
            }
        }
        assert!(
            typed_global_memory_versions_v1(
                &function,
                &callables,
                &projection,
                &changed,
                &sources,
                &mut 0
            )
            .is_err(),
            "axis {axis}"
        );
    }
}

#[test]
fn typed_global_memory_charges_replay_and_vector_initialization() {
    let (types, callables, function) = typed_global_memory_fixture_v1();
    let (projection, blocks, sources) =
        typed_global_ranked_source_fixture_v1(&types, &callables, &function);
    let mut cost = 0;
    typed_global_memory_versions_v1(
        &function,
        &callables,
        &projection,
        &blocks,
        &sources,
        &mut cost,
    )
    .unwrap();
    assert!(cost > blocks.len());
    let mut exact = MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1 - cost;
    typed_global_memory_versions_v1(
        &function,
        &callables,
        &projection,
        &blocks,
        &sources,
        &mut exact,
    )
    .unwrap();
    assert_eq!(exact, MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1);
    let mut short = MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1 - cost + 1;
    assert!(
        typed_global_memory_versions_v1(
            &function,
            &callables,
            &projection,
            &blocks,
            &sources,
            &mut short
        )
        .is_err()
    );
}
