fn blocked_global_fixture_call(
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

fn blocked_global_projection_fixture_v1() -> (
    Vec<SemanticTypeDeclV1>,
    Vec<SemanticCallableDeclV1>,
    SemanticFunctionDeclV1,
) {
    use SemanticCompilerIntrinsicOperationV1 as Op;
    use SemanticSourceArgumentOwnershipV1::{ByValue, ExclusiveOwner, SharedBorrow, UniqueBorrow};
    let (mut types, mut callables, base) = typed_global_projection_fixture_v1();
    let mut tag = 230_u8;
    let mut add = |shape, size| {
        let id = SemanticTypeIdV1::from_index(types.len() as u32);
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(tag)),
            SemanticLayoutIdentityV1::from_sha256(bytes(tag)),
            SemanticTypeLayoutV1::new(Some(size), 8).unwrap(),
            shape,
        ));
        tag += 1;
        id
    };
    let witness = add(
        SemanticTypeShapeV1::Aggregate(
            SemanticAggregateTypeV1::new(vec![U64_TYPE, U64_TYPE]).unwrap(),
        ),
        16,
    );
    let witness_ref = add(
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                witness,
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
        8,
    );
    let option = add(
        SemanticTypeShapeV1::enum_type(
            U64_TYPE,
            vec![
                SemanticEnumVariantV1::new(0, SemanticAggregateTypeV1::new(vec![]).unwrap()),
                SemanticEnumVariantV1::new(1, SemanticAggregateTypeV1::new(vec![witness]).unwrap()),
            ],
        )
        .unwrap(),
        24,
    );
    let view = base.locals()[14].ty();
    let physical = base.locals()[13].ty();
    let view_ref = base.locals()[20].ty();
    let mapping = SemanticDisjointIndexSpaceV1::BlockedIndex1d {
        lanes_per_block: 16,
        elements_per_lane: 4,
    };
    let contract =
        SemanticCapabilityMemoryContractV1::global_disjoint_write(CAP_INDEX_CONTEXT, mapping);
    let provenance = capability_index_provenance(None);
    let source_identity = SemanticFunctionIdentityV1::from_sha256(bytes(116));
    callables[6] = capability_index_callable(
        Op::CapabilityGlobalBindDisjointWrite {
            context: CAP_INDEX_CONTEXT,
            physical,
            view,
            element: U64_TYPE,
            contract,
            provenance,
            source_identity,
        },
        &[
            (CAP_INDEX_CONTEXT_BORROW, SharedBorrow),
            (physical, ExclusiveOwner),
        ],
        view,
    );
    callables[8] = capability_index_callable(
        Op::CapabilityGlobalStoreBlock {
            view,
            witness,
            component: U64_TYPE,
            element: U64_TYPE,
            result: BOOL_TYPE,
            contract,
            lanes_per_block: 16,
            elements_per_lane: 4,
            provenance,
            source_identity,
        },
        &[
            (view_ref, UniqueBorrow),
            (witness_ref, SharedBorrow),
            (U64_TYPE, ByValue),
            (U64_TYPE, ByValue),
        ],
        BOOL_TYPE,
    );
    assert_eq!(callables.len(), 9);
    callables.push(capability_index_callable(
        Op::ThreadIndexCheckedBlock {
            input_witness: CAP_INDEX_WITNESS,
            output_block: witness,
            raw_index: U64_TYPE,
            input_space: SemanticDisjointIndexSpaceV1::Index1d,
            output_space: mapping,
            lanes_per_block: 16,
            elements_per_lane: 4,
        },
        &[(CAP_INDEX_WITNESS, ByValue)],
        option,
    ));
    let mut locals = base.locals().to_vec();
    assert_eq!(locals.len(), 22);
    for (i, ty) in [option, U64_TYPE, witness, witness_ref]
        .into_iter()
        .enumerate()
    {
        locals.push(local(222 + i as u8, ty, SemanticLocalRoleV1::Temporary));
    }
    let mut blocks = base.blocks()[..4].to_vec();
    blocks.push(block(
        164,
        vec![],
        blocked_global_fixture_call(
            9,
            vec![SemanticOperandV1::Move(typed_place(4, CAP_INDEX_WITNESS))],
            22,
            option,
            5,
        ),
    ));
    blocks.push(block(
        165,
        vec![typed_assignment(
            23,
            U64_TYPE,
            SemanticRvalueKindV1::Discriminant(typed_place(22, option)),
        )],
        zero_switch(23, U64_TYPE, 9, 6),
    ));
    for (old, next) in [(5, 7), (6, 8)] {
        let original = &base.blocks()[old];
        let SemanticTerminatorKindV1::Call(call) = original.terminator().kind() else {
            unreachable!()
        };
        let destination = call.destination().unwrap().place();
        blocks.push(block(
            190 + old as u8,
            original.statements().to_vec(),
            blocked_global_fixture_call(
                call.callee().index(),
                call.arguments().to_vec(),
                destination.local().index(),
                destination.ty(),
                next,
            ),
        ));
    }
    let payload = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(22),
        vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Downcast(1), option).unwrap(),
            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), witness).unwrap(),
        ],
        witness,
    )
    .unwrap();
    let mut statements = base.blocks()[7].statements().to_vec();
    statements.push(typed_assignment(
        24,
        witness,
        SemanticRvalueKindV1::Use(SemanticOperandV1::Move(payload)),
    ));
    statements.push(typed_assignment(
        25,
        witness_ref,
        SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Shared,
            place: typed_place(24, witness),
        },
    ));
    blocks.push(block(
        197,
        statements,
        blocked_global_fixture_call(
            8,
            vec![
                SemanticOperandV1::Move(typed_place(20, view_ref)),
                typed_operand(25, witness_ref),
                typed_constant(U64_TYPE, 2, 8),
                typed_constant(U64_TYPE, 777, 8),
            ],
            21,
            BOOL_TYPE,
            9,
        ),
    ));
    blocks.push(block(198, vec![], SemanticTerminatorKindV1::Return));
    (
        types,
        callables,
        typed_global_fixture_with_body_v1(&base, locals, blocks),
    )
}

fn blocked_global_fixture_store(function: &SemanticFunctionDeclV1) -> &SemanticDirectCallV1 {
    let SemanticTerminatorKindV1::Call(call) = function.blocks()[8].terminator().kind() else {
        panic!("blocked source store")
    };
    call
}

fn blocked_global_changed_store(
    function: &SemanticFunctionDeclV1,
    arguments: Vec<SemanticOperandV1>,
) -> SemanticFunctionDeclV1 {
    let mut blocks = function.blocks().to_vec();
    blocks[8] = block(
        197,
        blocks[8].statements().to_vec(),
        blocked_global_fixture_call(8, arguments, 21, BOOL_TYPE, 9),
    );
    typed_global_fixture_with_body_v1(function, function.locals().to_vec(), blocks)
}

#[test]
fn typed_blocked_store_shape_retains_four_arguments_and_value_three() {
    let (types, callables, function) = blocked_global_projection_fixture_v1();
    let SemanticCallableDeclV1::CompilerIntrinsic {
        operation, binding, ..
    } = &callables[8]
    else {
        unreachable!()
    };
    let contract = global_access_contract_v1(operation).unwrap();
    let call = blocked_global_fixture_store(&function);
    assert_eq!(contract.shape.arity(), 4);
    assert_eq!(contract.shape.value_ordinal(), Some(3));
    assert!(contract.shape.arguments_match(call, U64_TYPE));
    assert!(contract.shape.witness_borrow_matches(&types, call));
    assert!(contract.shape.source_inputs_match(
        call,
        binding.abi().source_input_types(),
        binding.abi().source_argument_ownership(),
    ));
    assert_eq!(
        constant_operand_value(contract.shape.value_operand(call).unwrap(), &[]),
        Some(777)
    );
    assert_eq!(constant_operand_value(&call.arguments()[2], &[]), Some(2));
    assert_eq!(ProjectedGlobalAccessShapeV1::Read.arity(), 2);
    assert_eq!(ProjectedGlobalAccessShapeV1::Read.value_ordinal(), None);
    assert_eq!(ProjectedGlobalAccessShapeV1::Store.arity(), 3);
    assert_eq!(ProjectedGlobalAccessShapeV1::Store.value_ordinal(), Some(2));
    let SemanticTerminatorKindV1::Call(truncated) =
        blocked_global_fixture_call(8, call.arguments()[..3].to_vec(), 21, BOOL_TYPE, 9)
    else {
        unreachable!()
    };
    assert!(!contract.shape.arguments_match(&truncated, U64_TYPE));
    assert!(contract.shape.value_operand(&truncated).is_none());
    let mut ownership = binding.abi().source_argument_ownership().to_vec();
    ownership[1] = SemanticSourceArgumentOwnershipV1::ByValue;
    assert!(!contract.shape.source_inputs_match(
        call,
        binding.abi().source_input_types(),
        &ownership,
    ));
}

#[test]
fn typed_blocked_store_reassigned_bool_keeps_effect_not_predicate_authority() {
    let (types, callables, function) = blocked_global_projection_fixture_v1();
    let mut blocks = function.blocks().to_vec();
    blocks[9] = block(
        198,
        vec![typed_assignment(
            21,
            BOOL_TYPE,
            SemanticRvalueKindV1::Use(typed_constant(BOOL_TYPE, 0, 1)),
        )],
        SemanticTerminatorKindV1::Return,
    );
    let changed = typed_global_fixture_with_body_v1(&function, function.locals().to_vec(), blocks);
    let (projection, _) = project_capability_index_fixture(&types, &callables, &changed).unwrap();
    assert!(projection.direct_write_effects[8].is_some());
    assert_eq!(projection.direct_switch_predicates[21], None);
}

#[test]
fn typed_blocked_store_projects_available_witness_and_own_extent() {
    let (types, callables, function) = blocked_global_projection_fixture_v1();
    let (projection, operations) =
        project_capability_index_fixture_with_launch(&types, &callables, &function, Some(1024))
            .unwrap();
    let write = projection.direct_write_effects[8].as_ref().unwrap();
    let read = projection.direct_read_effects[7].as_ref().unwrap();
    assert_eq!(write.comparisons.len(), 1);
    assert_eq!(write.comparisons[0].0, write.indices[0]);
    assert_ne!(write.comparisons[0].1, read.comparisons[0].1);
    assert!(projection.index_values[25].unwrap().availability.is_some());
    assert_eq!(
        projection.direct_switch_predicates[21],
        Some(GuardPredicateV1::for_access(write))
    );
    for kind in [IndexBinaryKindAttr::Divide, IndexBinaryKindAttr::Remainder] {
        assert!(operations.iter().any(|op| matches!(op,
            ProductionRankedOperationV1::IndexBinary { kind: actual, .. } if *actual == kind)));
    }
    assert!(operations.iter().any(|op| matches!(op,
        ProductionRankedOperationV1::ViewInSpace { result, allocation_origin: 2,
            noalias_class: 3, dynamic_extents, .. }
        if *result == write.view && dynamic_extents == &[write.comparisons[0].1])));
}

#[test]
fn typed_blocked_store_source_rhs_is_not_component_and_site_is_exact() {
    let (types, callables, function) = blocked_global_projection_fixture_v1();
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
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].allocation_origin, 2);
    assert!(matches!(
        &writes[0].value,
        Ok(ProductionSemanticExpressionV2::Constant { bits: 777, .. })
    ));
    let source = sources
        .iter()
        .find(|source| source.access == AccessKindAttr::Write)
        .unwrap();
    assert_eq!(
        source.semantic_site,
        Some(ProjectedSemanticAccessSiteV1 {
            block: 8,
            statement: None
        })
    );
    for mutation in 0..3 {
        let mut changed = sources.clone();
        let write = changed
            .iter_mut()
            .find(|source| source.access == AccessKindAttr::Write)
            .unwrap();
        match mutation {
            0 => {
                write.semantic_site = Some(ProjectedSemanticAccessSiteV1 {
                    block: 7,
                    statement: None,
                })
            }
            1 => {
                write.semantic_site = Some(ProjectedSemanticAccessSiteV1 {
                    block: 8,
                    statement: Some(0),
                })
            }
            _ => changed.push(source.clone()),
        }
        let result = projected_reference_gpu_writes_v2(
            &types,
            &callables,
            &function,
            &projection,
            &blocks,
            &changed,
        )
        .unwrap();
        assert!(
            result.iter().any(|write| write.value.is_err()),
            "mutation {mutation}"
        );
    }
}

#[test]
fn typed_blocked_store_rejects_dynamic_component_and_component_limit() {
    let (types, callables, function) = blocked_global_projection_fixture_v1();
    project_capability_index_fixture(&types, &callables, &function).unwrap();
    for (component, detail) in [
        (
            typed_operand(6, U64_TYPE),
            "typed blocked store requires an exact literal component",
        ),
        (
            typed_constant(U64_TYPE, 4, 8),
            "a blocked component is outside the authenticated elements-per-lane bound",
        ),
    ] {
        let mut args = blocked_global_fixture_store(&function).arguments().to_vec();
        args[2] = component;
        let changed = blocked_global_changed_store(&function, args);
        let error = project_capability_index_fixture(&types, &callables, &changed)
            .err()
            .unwrap();
        assert!(matches!(error,
            ProductionRankedProjectionErrorV1::Incomplete(actual)
            | ProductionRankedProjectionErrorV1::Unsupported(actual) if actual == detail));
    }
}

#[test]
fn typed_blocked_store_rejects_changed_mapping_root_and_borrow() {
    let (types, callables, function) = blocked_global_projection_fixture_v1();
    project_capability_index_fixture(&types, &callables, &function).unwrap();
    for mutation in 0..3 {
        let mut changed = callables.clone();
        let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = &mut changed[8] else {
            unreachable!()
        };
        let SemanticCompilerIntrinsicOperationV1::CapabilityGlobalStoreBlock {
            contract,
            provenance,
            witness,
            ..
        } = operation
        else {
            unreachable!()
        };
        match mutation {
            0 => {
                *contract = SemanticCapabilityMemoryContractV1::global_disjoint_write(
                    CAP_INDEX_CONTEXT,
                    SemanticDisjointIndexSpaceV1::Index1d,
                )
            }
            1 => *provenance = capability_index_provenance(Some(1)),
            _ => *witness = U64_TYPE,
        }
        assert!(matches!(
            project_capability_index_fixture(&types, &changed, &function),
            Err(ProductionRankedProjectionErrorV1::GlobalAccess { .. })
        ));
    }
}

#[test]
fn typed_blocked_store_needs_some_dominance_and_finite_launch() {
    let (types, callables, function) = blocked_global_projection_fixture_v1();
    project_capability_index_fixture(&types, &callables, &function).unwrap();
    assert!(matches!(
        project_capability_index_fixture_with_launch(&types, &callables, &function, None),
        Err(ProductionRankedProjectionErrorV1::Incomplete(
            "a multi-lane blocked mapping requires an authenticated finite rank-1 launch extent whose full blocked index range fits u64"
        ))
    ));
    let mut blocks = function.blocks().to_vec();
    blocks[5] = block(
        165,
        blocks[5].statements().to_vec(),
        zero_switch(23, U64_TYPE, 6, 9),
    );
    let wrong = typed_global_fixture_with_body_v1(&function, function.locals().to_vec(), blocks);
    assert!(matches!(
        project_capability_index_fixture(&types, &callables, &wrong),
        Err(ProductionRankedProjectionErrorV1::Unsupported(_))
            | Err(ProductionRankedProjectionErrorV1::Incomplete(_))
    ));
}

#[test]
fn typed_blocked_literal_arithmetic_preserves_existing_operation_cap() {
    let mut operations = vec![
        ProductionRankedOperationV1::IndexConstant {
            result: ProductionRankedValueIdV1::new(0),
            value: 0,
        };
        MAX_PROJECTED_OPERATIONS_V1
    ];
    let mut next = 1;
    let error = project_literal_blocked_index_v1(
        ProductionRankedValueV1::Argument(0),
        16,
        4,
        0,
        &mut operations,
        &mut next,
        &mut String::new(),
    )
    .err()
    .unwrap();
    assert!(matches!(
        error,
        ProductionRankedProjectionErrorV1::Unsupported(_)
    ));
    assert_eq!(operations.len(), MAX_PROJECTED_OPERATIONS_V1);
}

#[test]
fn typed_blocked_literal_arithmetic_matches_checked_device_formula() {
    for (lanes, elements) in [(1_u64, 4_u64), (3, 2), (16, 4)] {
        for component in 0..elements {
            let mut operations = Vec::new();
            let mut next = 0;
            let result = project_literal_blocked_index_v1(
                ProductionRankedValueV1::Argument(0),
                lanes,
                elements,
                component,
                &mut operations,
                &mut next,
                &mut String::new(),
            )
            .unwrap();
            assert_eq!(operations.len(), if lanes == 1 { 4 } else { 8 });
            for raw in 0..1024_u64 {
                let mut values = BTreeMap::new();
                let read = |value: ProductionRankedValueV1, values: &BTreeMap<_, u64>| match value {
                    ProductionRankedValueV1::Argument(0) => raw,
                    ProductionRankedValueV1::Local(id) => values[&id],
                    _ => panic!("arithmetic cannot invent another source root"),
                };
                for operation in &operations {
                    let (id, value) = match operation {
                        ProductionRankedOperationV1::IndexConstant { result, value } => {
                            (*result, *value)
                        }
                        ProductionRankedOperationV1::IndexBinary {
                            result,
                            kind,
                            lhs,
                            rhs,
                        } => {
                            let lhs = read(*lhs, &values);
                            let rhs = read(*rhs, &values);
                            let value = match kind {
                                IndexBinaryKindAttr::Add => lhs.checked_add(rhs),
                                IndexBinaryKindAttr::Multiply => lhs.checked_mul(rhs),
                                IndexBinaryKindAttr::Divide => lhs.checked_div(rhs),
                                IndexBinaryKindAttr::Remainder => lhs.checked_rem(rhs),
                            }
                            .unwrap();
                            (*result, value)
                        }
                        _ => panic!("unexpected blocked producer"),
                    };
                    assert!(values.insert(id, value).is_none());
                }
                let block_elements = lanes.checked_mul(elements).unwrap();
                let block_base = (raw / lanes).checked_mul(block_elements).unwrap();
                let component_and_lane = component
                    .checked_mul(lanes)
                    .unwrap()
                    .checked_add(raw % lanes)
                    .unwrap();
                assert_eq!(
                    read(result, &values),
                    block_base.checked_add(component_and_lane).unwrap()
                );
            }
        }
    }
    for (lanes, elements, component) in [(0, 4, 0), (16, 0, 0), (u64::MAX, 2, 0), (16, 4, 4)] {
        let mut operations = Vec::new();
        assert!(
            project_literal_blocked_index_v1(
                ProductionRankedValueV1::Argument(0),
                lanes,
                elements,
                component,
                &mut operations,
                &mut 0,
                &mut String::new(),
            )
            .is_err()
        );
        assert!(operations.is_empty());
    }
    assert!(blocked_mapping_fits_launch_v1(Some(1024), 16, 4));
    assert!(!blocked_mapping_fits_launch_v1(Some(u64::MAX), 16, 4));
}
