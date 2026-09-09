// Projection-unit fixtures exercise the checked execution assignment shapes.
// Production admission and final KIR correspondence remain separate gates.
fn typed_global_projection_fixture_v1() -> (
    Vec<SemanticTypeDeclV1>,
    Vec<SemanticCallableDeclV1>,
    SemanticFunctionDeclV1,
) {
    use SemanticCompilerIntrinsicOperationV1 as Op;
    use SemanticSourceArgumentOwnershipV1::{ByValue, ExclusiveOwner, SharedBorrow, UniqueBorrow};
    let (mut types, mut callables, base) = capability_index_fixture();
    let mut add = |shape, size, align| {
        let id = SemanticTypeIdV1::from_index(types.len() as u32);
        let tag = 180 + id.index() as u8;
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(tag)),
            SemanticLayoutIdentityV1::from_sha256(bytes(tag)),
            SemanticTypeLayoutV1::new(size, align).unwrap(),
            shape,
        ));
        id
    };
    let slice = add(SemanticTypeShapeV1::Slice { element: U64_TYPE }, None, 8);
    let pointer = |pointee, kind, mutability, metadata| {
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(pointee, kind, mutability, 0, 64, metadata)
                .unwrap(),
        )
    };
    let physical_read = add(
        pointer(
            slice,
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
            SemanticPointerMetadataV1::SliceLength,
        ),
        Some(16),
        8,
    );
    let raw = add(
        pointer(
            U64_TYPE,
            SemanticPointerKindV1::Raw,
            SemanticMutabilityV1::Mutable,
            SemanticPointerMetadataV1::None,
        ),
        Some(8),
        8,
    );
    let aggregate =
        |fields| SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(fields).unwrap());
    let physical_write = add(aggregate(vec![raw, U64_TYPE]), Some(16), 8);
    let read_view = add(aggregate(vec![physical_read]), Some(16), 8);
    let write_view = add(aggregate(vec![physical_write]), Some(16), 8);
    let read_borrow = add(
        pointer(
            read_view,
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
            SemanticPointerMetadataV1::None,
        ),
        Some(8),
        8,
    );
    let write_borrow = add(
        pointer(
            write_view,
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Mutable,
            SemanticPointerMetadataV1::None,
        ),
        Some(8),
        8,
    );
    let option = add(
        SemanticTypeShapeV1::enum_type(
            U64_TYPE,
            vec![
                SemanticEnumVariantV1::new(0, SemanticAggregateTypeV1::new(vec![]).unwrap()),
                SemanticEnumVariantV1::new(
                    1,
                    SemanticAggregateTypeV1::new(vec![U64_TYPE]).unwrap(),
                ),
            ],
        )
        .unwrap(),
        Some(16),
        8,
    );
    types[physical_read.index() as usize] = types[physical_read.index() as usize]
        .clone()
        .with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                Some(
                    SemanticAbiPointeeInfoV1::new(
                        SemanticAbiPointeeKindV1::SharedReference { frozen: false },
                        0,
                        8,
                    )
                    .unwrap(),
                ),
                None,
            ),
        );
    let provenance = capability_index_provenance(None);
    let source_identity = SemanticFunctionIdentityV1::from_sha256(bytes(116));
    let read_contract = SemanticCapabilityMemoryContractV1::global_read_only();
    let write_contract = SemanticCapabilityMemoryContractV1::global_disjoint_write(
        CAP_INDEX_CONTEXT,
        SemanticDisjointIndexSpaceV1::Index1d,
    );
    callables[1] = capability_index_callable(
        Op::CapabilityGlobalBindReadOnly {
            context: CAP_INDEX_CONTEXT,
            physical: physical_read,
            view: read_view,
            element: U64_TYPE,
            contract: read_contract,
            provenance,
            source_identity,
        },
        &[
            (CAP_INDEX_CONTEXT_BORROW, SharedBorrow),
            (physical_read, SharedBorrow),
        ],
        read_view,
    );
    callables.push(capability_index_callable(
        Op::CapabilityGlobalBindDisjointWrite {
            context: CAP_INDEX_CONTEXT,
            physical: physical_write,
            view: write_view,
            element: U64_TYPE,
            contract: write_contract,
            provenance,
            source_identity,
        },
        &[
            (CAP_INDEX_CONTEXT_BORROW, SharedBorrow),
            (physical_write, ExclusiveOwner),
        ],
        write_view,
    ));
    callables.push(capability_index_callable(
        Op::CapabilityGlobalLoad {
            view: read_view,
            option,
            element: U64_TYPE,
            contract: read_contract,
            provenance,
            source_identity,
        },
        &[(read_borrow, SharedBorrow), (U64_TYPE, ByValue)],
        option,
    ));
    callables.push(capability_index_callable(
        Op::CapabilityGlobalStore {
            view: write_view,
            witness: CAP_INDEX_DISJOINT,
            element: U64_TYPE,
            result: BOOL_TYPE,
            contract: write_contract,
            provenance,
            source_identity,
        },
        &[
            (write_borrow, UniqueBorrow),
            (CAP_INDEX_DISJOINT, ByValue),
            (U64_TYPE, ByValue),
        ],
        BOOL_TYPE,
    ));
    let call = |callee, arguments, destination, ty, target| {
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
    };
    let borrow = |destination, ty, source, source_ty, kind| {
        typed_assignment(
            destination,
            ty,
            SemanticRvalueKindV1::Borrow {
                kind,
                place: typed_place(source, source_ty),
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
    let mut locals = base.locals().to_vec();
    assert_eq!(locals.len(), 12);
    locals[11] = local(191, read_view, SemanticLocalRoleV1::Temporary);
    for (index, ty) in [
        physical_read,
        physical_write,
        write_view,
        read_view,
        read_borrow,
        option,
        option,
        write_view,
        write_borrow,
        BOOL_TYPE,
    ]
    .into_iter()
    .enumerate()
    {
        locals.push(local(
            192 + index as u8,
            ty,
            if index < 2 {
                SemanticLocalRoleV1::Argument(index as u32)
            } else {
                SemanticLocalRoleV1::Temporary
            },
        ));
    }
    let mut blocks = base.blocks()[..5].to_vec();
    blocks[1] = block(
        161,
        vec![borrow(
            10,
            CAP_INDEX_CONTEXT_BORROW,
            1,
            CAP_INDEX_CONTEXT,
            SemanticBorrowKindV1::Shared,
        )],
        call(
            1,
            vec![
                typed_operand(10, CAP_INDEX_CONTEXT_BORROW),
                typed_operand(12, physical_read),
            ],
            11,
            read_view,
            2,
        ),
    );
    blocks.push(block(
        195,
        vec![],
        call(
            6,
            vec![
                typed_operand(10, CAP_INDEX_CONTEXT_BORROW),
                SemanticOperandV1::Move(typed_place(13, physical_write)),
            ],
            14,
            write_view,
            6,
        ),
    ));
    blocks.push(block(
        196,
        vec![
            transfer(15, read_view, 11),
            borrow(16, read_borrow, 15, read_view, SemanticBorrowKindV1::Shared),
        ],
        call(
            7,
            vec![typed_operand(16, read_borrow), typed_operand(6, U64_TYPE)],
            17,
            option,
            7,
        ),
    ));
    blocks.push(block(
        197,
        vec![
            transfer(18, option, 17),
            transfer(19, write_view, 14),
            borrow(
                20,
                write_borrow,
                19,
                write_view,
                SemanticBorrowKindV1::Mutable,
            ),
        ],
        call(
            8,
            vec![
                SemanticOperandV1::Move(typed_place(20, write_borrow)),
                SemanticOperandV1::Move(typed_place(7, CAP_INDEX_DISJOINT)),
                typed_constant(U64_TYPE, 7, 8),
            ],
            21,
            BOOL_TYPE,
            8,
        ),
    ));
    blocks.push(block(198, vec![], SemanticTerminatorKindV1::Return));
    let argument = |ty, pointee| {
        SemanticAbiValueV1::new(
            ty,
            SemanticAbiPassModeV1::Pair {
                first: SemanticAbiValueAttributesV1::plain(),
                second: SemanticAbiValueAttributesV1::plain(),
            },
        )
        .with_pointee_override(pointee)
    };
    let abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256(bytes(202)),
        SemanticLayoutIdentityV1::from_sha256(bytes(202)),
        SemanticCanonAbiV1::GpuKernel,
        false,
        false,
        vec![
            SemanticAbiValueV1::new(
                physical_read,
                SemanticAbiPassModeV1::Pair {
                    first: SemanticAbiValueAttributesV1::plain(),
                    second: SemanticAbiValueAttributesV1::plain(),
                },
            ),
            argument(
                physical_write,
                SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap(),
            ),
        ],
        SemanticAbiValueV1::new(SCALAR_TYPE, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SharedBorrow, ExclusiveOwner])
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
fn typed_global_projection_preserves_transfer_borrows_and_per_allocation_bounds() {
    let (types, callables, function) = typed_global_projection_fixture_v1();
    let argument = &function.abi().adjusted_arguments()[0];
    assert!(argument.value().pointee_override().is_none());
    assert!(matches!(
        types[argument.value().ty().index() as usize]
            .abi_properties()
            .first_pointee()
            .unwrap()
            .kind(),
        SemanticAbiPointeeKindV1::SharedReference { frozen: false }
    ));
    let (projection, operations) =
        project_capability_index_fixture(&types, &callables, &function).unwrap();
    let read = projection.direct_read_effects[6].as_ref().unwrap();
    let write = projection.direct_write_effects[7].as_ref().unwrap();
    assert_eq!(read.access, AccessKindAttr::Read);
    assert_eq!(write.access, AccessKindAttr::Write);
    assert_eq!(read.indices, write.indices);
    assert_ne!(read.comparisons[0].1, write.comparisons[0].1);
    for (access, origin, class, writable) in [(read, 1, 1, false), (write, 2, 3, true)] {
        assert_eq!(access.comparisons.len(), 1);
        assert!(operations.iter().any(|operation| matches!(operation,
            ProductionRankedOperationV1::ViewInSpace { result, allocation_origin, noalias_class,
                writable: actual_writable, dynamic_extents, .. }
            if *result == access.view && *allocation_origin == origin && *noalias_class == class
                && *actual_writable == writable && dynamic_extents == &[access.comparisons[0].1]
        )));
    }
    assert_eq!(
        projection.option_predicates[17],
        Some(GuardPredicateV1::for_access(read))
    );
    assert_eq!(
        projection.direct_switch_predicates[21],
        Some(GuardPredicateV1::for_access(write))
    );
}

fn typed_global_fixture_with_body_v1(
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

#[test]
fn typed_global_projection_frame_markers_do_not_count_as_value_definitions() {
    let (types, callables, function) = typed_global_projection_fixture_v1();
    let (baseline, _) = project_capability_index_fixture(&types, &callables, &function).unwrap();
    let mut blocks = function.blocks().to_vec();
    for (block_index, local, live) in [(6, 17, true), (7, 21, true), (8, 17, false), (8, 21, false)]
    {
        let original = &blocks[block_index];
        let mut statements = original.statements().to_vec();
        let local = SemanticLocalIdV1::from_index(local);
        statements.push(statement(if live {
            SemanticStatementKindV1::StorageLive(local)
        } else {
            SemanticStatementKindV1::StorageDead(local)
        }));
        blocks[block_index] = SemanticBasicBlockV1::new(
            original.identity(),
            original.source(),
            statements,
            original.terminator().clone(),
        )
        .unwrap();
    }
    let changed = typed_global_fixture_with_body_v1(&function, function.locals().to_vec(), blocks);
    let inventory = assertion_definition_inventory(&changed).unwrap();
    assert_eq!(inventory.counts[17], 1);
    assert_eq!(inventory.counts[21], 1);
    let (projection, _) = project_capability_index_fixture(&types, &callables, &changed).unwrap();
    assert_eq!(projection.direct_read_effects, baseline.direct_read_effects);
    assert_eq!(
        projection.direct_write_effects,
        baseline.direct_write_effects
    );
    assert_eq!(projection.option_predicates, baseline.option_predicates);
    assert_eq!(
        projection.direct_switch_predicates,
        baseline.direct_switch_predicates
    );
}

#[test]
fn typed_global_projection_merged_store_result_keeps_effect_without_authorizing_guard() {
    let (types, callables, function) = typed_global_projection_fixture_v1();
    let (baseline, _) = project_capability_index_fixture(&types, &callables, &function).unwrap();
    for consumed in [false, true] {
        let mut blocks = function.blocks().to_vec();
        let original = &blocks[6];
        let SemanticTerminatorKindV1::Call(load) = original.terminator().kind() else {
            unreachable!();
        };
        blocks[6] = SemanticBasicBlockV1::new(
            original.identity(),
            original.source(),
            original.statements().to_vec(),
            SemanticTerminatorV1::new(
                original.terminator().source(),
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new_callable(
                        load.callee(),
                        load.arguments().to_vec(),
                        Some(SemanticCallDestinationV1::new(
                            load.destination().unwrap().place().clone(),
                            cfg_edge(SemanticEdgeRoleV1::CallReturn, 9),
                        )),
                        load.unwind(),
                    )
                    .unwrap(),
                ),
            ),
        )
        .unwrap();
        // The index branch either executes the store in bb7 or assigns false in bb10.
        blocks.push(block(211, vec![], zero_switch(6, U64_TYPE, 10, 7)));
        blocks.push(block(
            212,
            vec![typed_assignment(
                21,
                BOOL_TYPE,
                SemanticRvalueKindV1::Use(typed_constant(BOOL_TYPE, 0, 1)),
            )],
            SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 8)),
        ));
        if consumed {
            blocks[8] = block(198, vec![], zero_switch(21, BOOL_TYPE, 11, 12));
            blocks.push(block(213, vec![], SemanticTerminatorKindV1::Return));
            blocks.push(block(214, vec![], SemanticTerminatorKindV1::Return));
        }
        let changed =
            typed_global_fixture_with_body_v1(&function, function.locals().to_vec(), blocks);
        assert_eq!(
            assertion_definition_inventory(&changed).unwrap().counts[21],
            2
        );
        let (projection, _) =
            project_capability_index_fixture(&types, &callables, &changed).unwrap();
        assert_eq!(
            projection.direct_read_effects[6],
            baseline.direct_read_effects[6]
        );
        assert_eq!(
            projection.direct_write_effects[7],
            baseline.direct_write_effects[7]
        );
        assert_eq!(projection.direct_write_effects.iter().flatten().count(), 1);
        assert!(projection.direct_switch_predicates[21].is_none());
        if consumed {
            let predicates = switch_predicates(
                &changed,
                &projection.option_predicates,
                &projection.direct_switch_predicates,
            )
            .unwrap();
            assert!(projection.deterministic_switches[8].is_none());
            assert_eq!(
                projected_cfg_terminator(
                    &changed,
                    8,
                    &callables,
                    false,
                    &constant_locals(&changed).unwrap(),
                    &predicates,
                    &projection.deterministic_switches,
                )
                .unwrap(),
                ProjectedCfgTerminatorV1::AnalysisSplit {
                    first_block: 11,
                    second_block: 12
                }
            );
        }
    }
}

#[test]
fn typed_global_projection_reassignment_cannot_reuse_store_result_predicate() {
    let (types, callables, function) = typed_global_projection_fixture_v1();
    let mut blocks = function.blocks().to_vec();
    blocks[8] = block(
        198,
        vec![typed_assignment(
            21,
            BOOL_TYPE,
            SemanticRvalueKindV1::Use(typed_constant(BOOL_TYPE, 0, 1)),
        )],
        zero_switch(21, BOOL_TYPE, 9, 10),
    );
    blocks.push(block(211, vec![], SemanticTerminatorKindV1::Return));
    blocks.push(block(212, vec![], SemanticTerminatorKindV1::Return));
    let changed = typed_global_fixture_with_body_v1(&function, function.locals().to_vec(), blocks);
    assert_eq!(
        assertion_definition_inventory(&changed).unwrap().counts[21],
        2
    );
    let (projection, _) = project_capability_index_fixture(&types, &callables, &changed).unwrap();
    assert!(projection.direct_write_effects[7].is_some());
    assert!(projection.direct_switch_predicates[21].is_none());
    assert!(projection.deterministic_switches[8].is_none());
    assert_eq!(
        projected_cfg_terminator(
            &changed,
            8,
            &callables,
            false,
            &constant_locals(&changed).unwrap(),
            &switch_predicates(
                &changed,
                &projection.option_predicates,
                &projection.direct_switch_predicates,
            )
            .unwrap(),
            &projection.deterministic_switches,
        )
        .unwrap(),
        ProjectedCfgTerminatorV1::AnalysisSplit {
            first_block: 9,
            second_block: 10
        }
    );
}

#[test]
fn typed_global_projection_escaped_result_cannot_authorize_guard() {
    for destination in [17, 21] {
        let (mut types, callables, function) = typed_global_projection_fixture_v1();
        let result_ty = function.locals()[destination].ty();
        let pointer_ty = SemanticTypeIdV1::from_index(types.len() as u32);
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(215)),
            SemanticLayoutIdentityV1::from_sha256(bytes(215)),
            SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    result_ty,
                    SemanticPointerKindV1::Raw,
                    SemanticMutabilityV1::Mutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        ));
        let mut locals = function.locals().to_vec();
        let pointer = locals.len() as u32;
        locals.push(local(216, pointer_ty, SemanticLocalRoleV1::Temporary));
        let block_index = if destination == 17 { 7 } else { 8 };
        let mut blocks = function.blocks().to_vec();
        let original = &blocks[block_index];
        let mut statements = original.statements().to_vec();
        statements.insert(
            0,
            typed_assignment(
                pointer,
                pointer_ty,
                SemanticRvalueKindV1::AddressOf {
                    mutability: SemanticMutabilityV1::Mutable,
                    place: typed_place(destination as u32, result_ty),
                },
            ),
        );
        blocks[block_index] = SemanticBasicBlockV1::new(
            original.identity(),
            original.source(),
            statements,
            original.terminator().clone(),
        )
        .unwrap();
        let changed = typed_global_fixture_with_body_v1(&function, locals, blocks);
        let inventory = assertion_definition_inventory(&changed).unwrap();
        assert_eq!(inventory.counts[destination], 1);
        assert!(inventory.address_escaped[destination]);
        let (projection, _) =
            project_capability_index_fixture(&types, &callables, &changed).unwrap();
        assert!(projection.direct_read_effects[6].is_some());
        assert!(projection.direct_write_effects[7].is_some());
        assert!(projection.option_predicates[destination].is_none());
        assert!(projection.direct_switch_predicates[destination].is_none());
    }
}

#[test]
fn typed_global_projection_rejects_source_provenance_and_mapping_substitution() {
    let (types, callables, function) = typed_global_projection_fixture_v1();
    for axis in 0..3 {
        let mut changed = callables.clone();
        let SemanticCallableDeclV1::CompilerIntrinsic {
            operation:
                SemanticCompilerIntrinsicOperationV1::CapabilityGlobalStore {
                    source_identity,
                    provenance,
                    contract,
                    ..
                },
            ..
        } = &mut changed[8]
        else {
            unreachable!();
        };
        match axis {
            0 => *source_identity = SemanticFunctionIdentityV1::from_sha256(bytes(210)),
            1 => *provenance = capability_index_provenance(Some(3)),
            _ => {
                *contract = SemanticCapabilityMemoryContractV1::global_disjoint_write(
                    CAP_INDEX_CONTEXT,
                    SemanticDisjointIndexSpaceV1::ShiftedIndex1d { offset: 1 },
                )
            }
        }
        assert_incomplete(
            project_capability_index_fixture(&types, &changed, &function),
            "a typed global access lacks its exact bound allocation, borrow, or source contract",
        );
    }
}

#[test]
fn typed_global_projection_does_not_invent_exclusive_root_ownership() {
    let (types, callables, function) = typed_global_projection_fixture_v1();
    let abi = function
        .abi()
        .clone()
        .with_source_argument_ownership(vec![
            SemanticSourceArgumentOwnershipV1::SharedBorrow,
            SemanticSourceArgumentOwnershipV1::RawPointer,
        ])
        .unwrap();
    let changed = SemanticFunctionDeclV1::new(
        function.identity(),
        function.role(),
        function.item_definition_identity(),
        function.monomorphization_identity(),
        function.generic_type_arguments_identity(),
        function.const_generic_arguments_identity(),
        function.source(),
        abi,
        function.locals().to_vec(),
        function.entry(),
        function.blocks().to_vec(),
    )
    .unwrap();
    assert_incomplete(
        project_capability_index_fixture(&types, &callables, &changed),
        "a typed global access lacks its exact bound allocation, borrow, or source contract",
    );
}

#[test]
fn typed_global_projection_rejects_unbound_receiver_and_borrow_escalation() {
    let (types, callables, function) = typed_global_projection_fixture_v1();
    for replacement in [
        SemanticStatementKindV1::Nop,
        SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(19)),
    ] {
        let mut blocks = function.blocks().to_vec();
        let original = &blocks[7];
        let mut statements = original.statements().to_vec();
        if matches!(replacement, SemanticStatementKindV1::Nop) {
            statements[1] = SemanticStatementV1::new(statements[1].source(), replacement);
        } else {
            statements.insert(
                2,
                SemanticStatementV1::new(original.terminator().source(), replacement),
            );
        }
        blocks[7] = SemanticBasicBlockV1::new(
            original.identity(),
            original.source(),
            statements,
            original.terminator().clone(),
        )
        .unwrap();
        let changed = SemanticFunctionDeclV1::new(
            function.identity(),
            function.role(),
            function.item_definition_identity(),
            function.monomorphization_identity(),
            function.generic_type_arguments_identity(),
            function.const_generic_arguments_identity(),
            function.source(),
            function.abi().clone(),
            function.locals().to_vec(),
            function.entry(),
            blocks,
        )
        .unwrap();
        assert_incomplete(
            project_capability_index_fixture(&types, &callables, &changed),
            "a typed global access lacks its exact bound allocation, borrow, or source contract",
        );
    }
    let mut state = HashMap::new();
    state.insert(
        1,
        ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::KernelContext {
            context: CAP_INDEX_CONTEXT,
            shared_borrow: false,
        }),
    );
    assert_eq!(
        capability_borrow_origin_v1(
            &state,
            &typed_place(1, CAP_INDEX_CONTEXT),
            SemanticBorrowKindV1::Mutable
        ),
        None
    );
}
