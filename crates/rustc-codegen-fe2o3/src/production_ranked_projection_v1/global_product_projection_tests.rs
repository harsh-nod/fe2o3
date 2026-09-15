// Uses the existing projection fixture's Global issuers, allocation contracts,
// source-body read and bounds collectors. These are not device/source evidence.
fn global_product_projection_fixture_v1(
    field: u32,
) -> (
    Vec<SemanticTypeDeclV1>,
    Vec<SemanticCallableDeclV1>,
    SemanticFunctionDeclV1,
) {
    let (mut types, callables, base) = typed_global_source_fixture_v1();
    let physical = base.locals()[12].ty();
    let view = base.locals()[15].ty();
    let reference = base.locals()[16].ty();
    let product = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(230)),
        SemanticLayoutIdentityV1::from_sha256(bytes(230)),
        SemanticTypeLayoutV1::aggregate(
            Some(16),
            8,
            SemanticAggregateLayoutV1::new(vec![0, 8], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![reference; 2]).unwrap()),
    ));
    let product_reference = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(231)),
        SemanticLayoutIdentityV1::from_sha256(bytes(231)),
        SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                product,
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    ));
    let mut locals = base.locals().to_vec();
    assert_eq!(locals.len(), 34);
    for (offset, ty) in [
        physical,
        view,
        reference,
        product,
        product_reference,
        reference,
    ]
    .into_iter()
    .enumerate()
    {
        locals.push(local(
            232 + offset as u8,
            ty,
            if offset == 0 {
                SemanticLocalRoleV1::Argument(2)
            } else {
                SemanticLocalRoleV1::Temporary
            },
        ));
    }
    let mut arguments = base
        .abi()
        .adjusted_arguments()
        .iter()
        .map(|arg| arg.value().clone())
        .collect::<Vec<_>>();
    arguments.push(arguments[0].clone());
    let mut ownership = base.abi().source_argument_ownership().to_vec();
    ownership.push(SemanticSourceArgumentOwnershipV1::SharedBorrow);
    let abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256(bytes(238)),
        SemanticLayoutIdentityV1::from_sha256(bytes(238)),
        base.abi().canon_abi(),
        base.abi().c_variadic(),
        base.abi().can_unwind(),
        arguments,
        base.abi().return_value().clone(),
    )
    .unwrap()
    .with_source_argument_ownership(ownership)
    .unwrap();
    let mut blocks = base.blocks().to_vec();
    // Both views come from the real closed BindReadOnly transfer. The second
    // has a distinct source parameter/allocation, not a copied side record.
    let issuer_block = blocks.len() as u32;
    let SemanticTerminatorKindV1::Call(previous) = blocks[5].terminator().kind() else {
        unreachable!()
    };
    let previous_destination = previous.destination().unwrap();
    let previous_edge = previous_destination.edge();
    blocks[5] = SemanticBasicBlockV1::new(
        blocks[5].identity(),
        blocks[5].source(),
        blocks[5].statements().to_vec(),
        SemanticTerminatorV1::new(
            blocks[5].terminator().source(),
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    previous.callee(),
                    previous.arguments().to_vec(),
                    Some(SemanticCallDestinationV1::new(
                        previous_destination.place().clone(),
                        cfg_edge(SemanticEdgeRoleV1::CallReturn, issuer_block),
                    )),
                    previous.unwind(),
                )
                .unwrap(),
            ),
        ),
    )
    .unwrap();
    blocks.push(block(
        239,
        vec![],
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(1),
                vec![
                    typed_operand(10, CAP_INDEX_CONTEXT_BORROW),
                    typed_operand(34, physical),
                ],
                Some(SemanticCallDestinationV1::new(
                    typed_place(35, view),
                    previous_edge,
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        ),
    ));
    let original = &blocks[6];
    let mut statements = original.statements().to_vec();
    statements.extend([
        typed_assignment(
            36,
            reference,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: typed_place(35, view),
            },
        ),
        typed_assignment(
            37,
            product,
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(
                    SemanticAggregateKindV1::Aggregate,
                    vec![typed_operand(16, reference), typed_operand(36, reference)],
                )
                .unwrap(),
            ),
        ),
        typed_assignment(
            38,
            product_reference,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: typed_place(37, product),
            },
        ),
        typed_assignment(
            39,
            reference,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
                SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(38),
                    vec![
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, product)
                            .unwrap(),
                        SemanticProjectionV1::new(
                            SemanticProjectionKindV1::Field(field),
                            reference,
                        )
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
    let mut args = call.arguments().to_vec();
    args[0] = typed_operand(39, reference);
    blocks[6] = SemanticBasicBlockV1::new(
        original.identity(),
        original.source(),
        statements,
        SemanticTerminatorV1::new(
            original.terminator().source(),
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    call.callee(),
                    args,
                    call.destination().cloned(),
                    call.unwind(),
                )
                .unwrap(),
            ),
        ),
    )
    .unwrap();
    let function = SemanticFunctionDeclV1::new(
        base.identity(),
        base.role(),
        base.item_definition_identity(),
        base.monomorphization_identity(),
        base.generic_type_arguments_identity(),
        base.const_generic_arguments_identity(),
        base.source(),
        abi,
        locals,
        base.entry(),
        blocks,
    )
    .unwrap();
    (types, callables, function)
}

#[test]
fn global_product_projection_preserves_selected_allocation_and_indexed_read() {
    let mut observed = Vec::new();
    for field in [0, 1] {
        let (types, callables, function) = global_product_projection_fixture_v1(field);
        let (projection, operations) =
            project_capability_index_fixture(&types, &callables, &function).unwrap();
        let view = projection.global_views[6].expect("selected Global access");
        let read = projection.direct_read_effects[6]
            .as_ref()
            .expect("source read effect");
        assert_eq!(read.access, AccessKindAttr::Read);
        assert_eq!(read.memory_space, MemorySpaceAttr::Global);
        assert_eq!(read.indices.len(), 1);
        assert_eq!(read.comparisons.len(), 1);
        assert_eq!(read.comparisons[0].0, read.indices[0]);
        assert!(operations.iter().any(
            |op| matches!(op, ProductionRankedOperationV1::ViewInSpace {
            result, allocation_origin, noalias_class, writable: false, ..
        } if *result == read.view && *allocation_origin == view.allocation.allocation_origin
            && *noalias_class == view.allocation.noalias_class)
        ));
        assert_eq!(
            view.allocation.allocation_origin,
            if field == 0 { 1 } else { 3 }
        );
        observed.push(view.allocation);
    }
    assert_ne!(observed[0], observed[1]);
}

#[test]
fn global_product_projection_rejects_dead_issuer_result_and_borrowed_source() {
    for missing in [false, true] {
        let (types, callables, function) = global_product_projection_fixture_v1(1);
        let mut statements = function.blocks()[6].statements().to_vec();
        statements.insert(
            if missing { 2 } else { 5 },
            statement(SemanticStatementKindV1::StorageDead(
                SemanticLocalIdV1::from_index(if missing { 35 } else { 37 }),
            )),
        );
        let function = captured_global_replace_statements_v1(&function, statements);
        let result = project_capability_index_fixture(&types, &callables, &function);
        assert!(
            matches!(result, Err(ProductionRankedProjectionErrorV1::GlobalAccess { reason, .. })
            if reason == "receiver binding is absent")
        );
    }
}
