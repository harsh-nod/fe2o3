// Private component fixtures exercise provenance checks, not source admission.
fn consumed_read_only_types_v1() -> Vec<SemanticTypeDeclV1> {
    let ty = SemanticTypeIdV1::from_index;
    let scalar = |bits| {
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits,
        })
    };
    let pointer = |pointee, kind, mutability| {
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                ty(pointee),
                kind,
                mutability,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        )
    };
    let specs = vec![
        (2, 2, scalar(16)),
        (8, 8, scalar(64)),
        (
            16,
            8,
            SemanticTypeShapeV1::Aggregate(
                SemanticAggregateTypeV1::new(vec![ty(6), ty(1)]).unwrap(),
            ),
        ),
        (
            16,
            8,
            SemanticTypeShapeV1::Aggregate(
                SemanticAggregateTypeV1::new(vec![ty(7), ty(1)]).unwrap(),
            ),
        ),
        (
            8,
            8,
            pointer(
                2,
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
            ),
        ),
        (
            8,
            8,
            pointer(
                3,
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
            ),
        ),
        (
            8,
            8,
            pointer(0, SemanticPointerKindV1::Raw, SemanticMutabilityV1::Mutable),
        ),
        (
            8,
            8,
            pointer(
                0,
                SemanticPointerKindV1::Raw,
                SemanticMutabilityV1::Immutable,
            ),
        ),
    ];
    let mut types: Vec<_> = specs
        .into_iter()
        .enumerate()
        .map(|(i, (bytes_count, align, shape))| {
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256(bytes(180 + i as u8)),
                SemanticLayoutIdentityV1::from_sha256(bytes(180 + i as u8)),
                SemanticTypeLayoutV1::new(Some(bytes_count), align).unwrap(),
                shape,
            )
        })
        .collect();
    types[2] = types[2].clone().with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
            Some(SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap()),
            None,
        ),
    );
    types
}

fn consumed_read_only_call_v1(
    callee: u32,
    args: Vec<SemanticOperandV1>,
    destination: (u32, u32),
    target: u32,
) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(callee),
            args,
            Some(SemanticCallDestinationV1::new(
                typed_place(destination.0, SemanticTypeIdV1::from_index(destination.1)),
                cfg_edge(SemanticEdgeRoleV1::CallReturn, target),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    )
}

fn consumed_read_only_blocks_v1() -> Vec<SemanticBasicBlockV1> {
    let ty = SemanticTypeIdV1::from_index;
    vec![
        block(
            180,
            vec![],
            consumed_read_only_call_v1(
                0,
                vec![SemanticOperandV1::Move(typed_place(1, ty(2)))],
                (2, 3),
                1,
            ),
        ),
        block(
            181,
            vec![typed_assignment(
                3,
                ty(5),
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: typed_place(2, ty(3)),
                },
            )],
            consumed_read_only_call_v1(1, vec![typed_operand(3, ty(5))], (5, 1), 2),
        ),
        block(
            182,
            vec![],
            consumed_read_only_call_v1(
                2,
                vec![
                    typed_operand(3, ty(5)),
                    typed_constant(ty(1), 0, 8),
                    typed_constant(ty(0), 17, 2),
                ],
                (4, 0),
                3,
            ),
        ),
        block(183, vec![], SemanticTerminatorKindV1::Return),
    ]
}

fn consumed_read_only_function_v1(blocks: Vec<SemanticBasicBlockV1>) -> SemanticFunctionDeclV1 {
    let ty = SemanticTypeIdV1::from_index;
    let abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256(bytes(180)),
        SemanticLayoutIdentityV1::from_sha256(bytes(180)),
        SemanticCanonAbiV1::GpuKernel,
        false,
        false,
        vec![SemanticAbiValueV1::new(
            ty(2),
            SemanticAbiPassModeV1::Pair {
                first: SemanticAbiValueAttributesV1::plain(),
                second: SemanticAbiValueAttributesV1::plain(),
            },
        )],
        SemanticAbiValueV1::new(ty(0), SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ExclusiveOwner])
    .unwrap();
    let locals = [0, 2, 3, 5, 0, 1, 2, 4, 3, 7, 1]
        .into_iter()
        .enumerate()
        .map(|(i, t)| {
            local(
                190 + i as u8,
                ty(t),
                match i {
                    0 => SemanticLocalRoleV1::Return,
                    1 => SemanticLocalRoleV1::Argument(0),
                    _ => SemanticLocalRoleV1::Temporary,
                },
            )
        })
        .collect();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(181)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(182)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(183)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(184)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(185)),
        SemanticSourceProvenanceV1::unavailable(),
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
}

fn consumed_read_only_callables_v1() -> Vec<SemanticCallableDeclV1> {
    let ty = SemanticTypeIdV1::from_index;
    vec![
        compiler_intrinsic_callable(
            SemanticCompilerIntrinsicOperationV1::DisjointSliceIntoReadOnly {
                slice: ty(2),
                view: ty(3),
                element: ty(0),
            },
        ),
        compiler_intrinsic_callable(
            SemanticCompilerIntrinsicOperationV1::ReadOnlyAllocationLen { view: ty(3) },
        ),
        compiler_intrinsic_callable(
            SemanticCompilerIntrinsicOperationV1::ReadOnlyAllocationLoadOr {
                view: ty(3),
                element: ty(0),
            },
        ),
    ]
}

fn consumed_read_only_allocations_v1() -> Vec<Option<AllocationContractV1>> {
    let mut allocations = vec![None; 11];
    allocations[1] = Some(AllocationContractV1 {
        allocation_origin: 1,
        noalias_class: 2,
        writable: true,
        singleton_object: false,
    });
    allocations
}

#[test]
fn consumed_read_only_retains_exclusive_origin_extent_and_total_read() {
    let types = consumed_read_only_types_v1();
    let function = consumed_read_only_function_v1(consumed_read_only_blocks_v1());
    let callables = consumed_read_only_callables_v1();
    let allocations = consumed_read_only_allocations_v1();
    let provenance = local_provenance_v1(&types, &function).unwrap();
    let actual =
        local_allocation_contracts(&types, &function, &provenance.allocation_origins).unwrap();
    assert_eq!(actual[1], allocations[1]);
    audit_consumed_read_only_roots_v1(&types, &callables, &function, &allocations).unwrap();
    let dominance = SemanticEnumPayloadDominanceV1::analyze(&function, &types).unwrap();
    let effects = project_authenticated_capabilities_v1(
        &types,
        &callables,
        &function,
        &dominance,
        &allocations,
        &[None; 11],
    )
    .unwrap();
    let effect = effects.read_views[2].unwrap();
    assert_eq!(effect.view.allocation.allocation_origin, 1);
    assert_eq!(effect.view.allocation.noalias_class, 2);
    assert!(!effect.view.allocation.writable);
    assert_eq!(
        effect.view.columns,
        ProjectedReadValueV1::AllocationExtent(0)
    );
    let mut operations = Vec::new();
    let projected = project_strided_read_effects_v1(
        &types,
        &function,
        &effects.read_views,
        &[None; 11],
        &mut [None; 11],
        &mut [None; 11],
        &mut 1,
        &mut operations,
        &mut 0,
    )
    .unwrap();
    let access = projected[2].as_ref().unwrap();
    assert_eq!(
        access.failure,
        GuardedAccessFailureV1::ContinueWithoutAccess
    );
    assert_eq!(access.access, AccessKindAttr::Read);
    assert_eq!(access.comparisons.len(), 2);
    assert_eq!(
        access.comparisons[1].1,
        ProductionRankedValueV1::Argument(1)
    );
}

#[test]
fn consumed_read_only_rejects_weakened_root_or_conversion_identity() {
    let types = consumed_read_only_types_v1();
    let function = consumed_read_only_function_v1(consumed_read_only_blocks_v1());
    for mutation in 0..8 {
        let mut allocations = consumed_read_only_allocations_v1();
        let mut callables = consumed_read_only_callables_v1();
        match mutation {
            0 => allocations[1] = None,
            1 => allocations[1].as_mut().unwrap().noalias_class = 1,
            2 => allocations[1].as_mut().unwrap().allocation_origin = 2,
            3 => allocations[1].as_mut().unwrap().writable = false,
            4 => allocations[1].as_mut().unwrap().singleton_object = true,
            5..=7 => {
                let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = &mut callables[0]
                else {
                    unreachable!()
                };
                let SemanticCompilerIntrinsicOperationV1::DisjointSliceIntoReadOnly {
                    slice,
                    view,
                    element,
                } = operation
                else {
                    unreachable!()
                };
                match mutation {
                    5 => *slice = SemanticTypeIdV1::from_index(3),
                    6 => *view = SemanticTypeIdV1::from_index(2),
                    _ => *element = SemanticTypeIdV1::from_index(1),
                }
            }
            _ => unreachable!(),
        }
        assert!(
            audit_consumed_read_only_roots_v1(&types, &callables, &function, &allocations).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn consumed_read_only_audits_writes_and_escapes_before_and_after_conversion() {
    let ty = SemanticTypeIdV1::from_index;
    let types = consumed_read_only_types_v1();
    let mutants = vec![
        typed_assignment(
            9,
            ty(7),
            SemanticRvalueKindV1::Cast {
                kind: SemanticCastKindV1::Pointer,
                operand: typed_operand(1, ty(2)),
            },
        ),
        typed_assignment(6, ty(2), SemanticRvalueKindV1::Use(typed_operand(1, ty(2)))),
        typed_assignment(
            7,
            ty(4),
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Mutable,
                place: typed_place(1, ty(2)),
            },
        ),
        typed_assignment(
            9,
            ty(7),
            SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Immutable,
                place: typed_place(1, ty(2)),
            },
        ),
        typed_assignment(
            5,
            ty(1),
            SemanticRvalueKindV1::Length(typed_place(1, ty(2))),
        ),
        statement(SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
            typed_place(1, ty(2)),
            typed_constant(ty(0), 1, 2),
            SemanticVolatilityV1::NonVolatile,
            None,
        ))),
        statement(SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
            typed_place(1, ty(2)),
            typed_constant(ty(0), 1, 2),
            SemanticVolatilityV1::NonVolatile,
            Some(SemanticAtomicAccessV1::new(
                SemanticAtomicOrderingV1::Release,
                SemanticAtomicScopeV1::System,
            )),
        ))),
        statement(SemanticStatementKindV1::Deinitialize(typed_place(1, ty(2)))),
        statement(SemanticStatementKindV1::Assume(typed_operand(1, ty(2)))),
    ];
    for (mutation, statement) in mutants.into_iter().enumerate() {
        for site in [0, 3] {
            let mut blocks = consumed_read_only_blocks_v1();
            blocks[site] = block(
                210 + site as u8,
                vec![statement.clone()],
                blocks[site].terminator().kind().clone(),
            );
            let function = consumed_read_only_function_v1(blocks);
            assert!(
                audit_consumed_read_only_roots_v1(
                    &types,
                    &consumed_read_only_callables_v1(),
                    &function,
                    &consumed_read_only_allocations_v1()
                )
                .is_err(),
                "mutation {mutation}, block {site}"
            );
        }
    }
}

#[test]
fn consumed_read_only_rejects_unknown_calls_tailcalls_and_projected_uses() {
    let ty = SemanticTypeIdV1::from_index;
    let types = consumed_read_only_types_v1();
    for mutation in 0..4 {
        let mut blocks = consumed_read_only_blocks_v1();
        let receiver = if mutation == 3 {
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(3),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ty(3))
                        .unwrap(),
                ],
                ty(3),
            )
            .unwrap()
        } else {
            typed_place(3, ty(5))
        };
        let call = match mutation {
            0 => consumed_read_only_call_v1(99, vec![SemanticOperandV1::Copy(receiver)], (4, 0), 3),
            1 => SemanticTerminatorKindV1::Drop {
                place: receiver,
                drop_glue: SemanticFunctionIdV1::from_index(99),
                target: cfg_edge(SemanticEdgeRoleV1::DropReturn, 3),
                unwind: SemanticUnwindActionV1::Unreachable,
            },
            2 => SemanticTerminatorKindV1::Assert {
                condition: SemanticOperandV1::Copy(receiver),
                expected: true,
                message: SemanticAssertMessageV1::NullPointerDereference,
                target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, 3),
                unwind: SemanticUnwindActionV1::Unreachable,
            },
            _ => consumed_read_only_call_v1(1, vec![SemanticOperandV1::Copy(receiver)], (5, 1), 3),
        };
        blocks[2] = block(221, vec![], call);
        assert!(
            audit_consumed_read_only_roots_v1(
                &types,
                &consumed_read_only_callables_v1(),
                &consumed_read_only_function_v1(blocks),
                &consumed_read_only_allocations_v1()
            )
            .is_err()
        );
    }
}

#[test]
fn consumed_read_only_transparent_moves_preserve_root_but_copied_temporaries_do_not() {
    let ty = SemanticTypeIdV1::from_index;
    let mut blocks = consumed_read_only_blocks_v1();
    blocks[0] = block(
        220,
        vec![typed_assignment(
            6,
            ty(2),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(typed_place(1, ty(2)))),
        )],
        consumed_read_only_call_v1(
            0,
            vec![SemanticOperandV1::Move(typed_place(6, ty(2)))],
            (2, 3),
            1,
        ),
    );
    let mut allocations = consumed_read_only_allocations_v1();
    allocations[6] = allocations[1];
    let types = consumed_read_only_types_v1();
    audit_consumed_read_only_roots_v1(
        &types,
        &consumed_read_only_callables_v1(),
        &consumed_read_only_function_v1(blocks),
        &allocations,
    )
    .unwrap();
    let mut blocks = consumed_read_only_blocks_v1();
    blocks[0] = block(
        220,
        vec![],
        consumed_read_only_call_v1(0, vec![typed_operand(6, ty(2))], (2, 3), 1),
    );
    assert!(
        audit_consumed_read_only_roots_v1(
            &types,
            &consumed_read_only_callables_v1(),
            &consumed_read_only_function_v1(blocks),
            &allocations
        )
        .is_err()
    );
}

#[test]
fn consumed_read_only_does_not_reanimate_dead_or_overwritten_views() {
    let ty = SemanticTypeIdV1::from_index;
    let types = consumed_read_only_types_v1();
    for dead in [false, true] {
        let mut blocks = consumed_read_only_blocks_v1();
        let statements = if dead {
            vec![statement(SemanticStatementKindV1::StorageDead(
                SemanticLocalIdV1::from_index(3),
            ))]
        } else {
            vec![
                typed_assignment(
                    8,
                    ty(3),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(typed_place(2, ty(3)))),
                ),
                typed_assignment(
                    3,
                    ty(5),
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Shared,
                        place: typed_place(2, ty(3)),
                    },
                ),
            ]
        };
        blocks[2] = block(221, statements, blocks[2].terminator().kind().clone());
        let function = consumed_read_only_function_v1(blocks);
        let dominance = SemanticEnumPayloadDominanceV1::analyze(&function, &types).unwrap();
        let result = project_authenticated_capabilities_v1(
            &types,
            &consumed_read_only_callables_v1(),
            &function,
            &dominance,
            &consumed_read_only_allocations_v1(),
            &[None; 11],
        );
        assert!(result.is_err());
    }
}

#[test]
fn consumed_read_only_extent_and_audit_budgets_fail_closed() {
    let mut extents = [None; 2];
    let mut next = 1;
    assert_eq!(
        project_consumed_read_only_extent_v1(0, &mut extents, &mut next).unwrap(),
        ProductionRankedValueV1::Argument(1)
    );
    assert_eq!(
        project_consumed_read_only_extent_v1(0, &mut extents, &mut next).unwrap(),
        ProductionRankedValueV1::Argument(1)
    );
    assert_eq!(next, 2);
    assert!(project_consumed_read_only_extent_v1(2, &mut extents, &mut next).is_err());
    let mut at_limit = HARD_MAX_PRODUCTION_RANKED_ARGUMENTS;
    assert!(project_consumed_read_only_extent_v1(1, &mut extents, &mut at_limit).is_err());
    let types = consumed_read_only_types_v1();
    let mut audit = ConsumedReadOnlyAuditV1 {
        types: &types,
        roots: &[],
        aliases: &[],
        after_conversion: &[],
        block: 0,
        work: MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1 - 1,
    };
    audit.statement(&SemanticStatementKindV1::Nop).unwrap();
    assert!(audit.statement(&SemanticStatementKindV1::Nop).is_err());
}

fn consumed_read_only_audit_root_for_test_v1(argument: u32) -> ConsumedReadOnlyRootV1 {
    ConsumedReadOnlyRootV1 {
        argument,
        slice: SemanticTypeIdV1::from_index(2),
        view: SemanticTypeIdV1::from_index(3),
        element: SemanticTypeIdV1::from_index(0),
        conversion_block: argument as usize,
    }
}

#[test]
fn consumed_read_only_shared_reference_copy_is_exact_root_preserving() {
    let ty = SemanticTypeIdV1::from_index;
    let types = consumed_read_only_types_v1();
    let roots = [Some(consumed_read_only_audit_root_for_test_v1(0))];
    let aliases = [None, Some(0), Some(0), Some(0)];
    let mut audit = ConsumedReadOnlyAuditV1 {
        types: &types,
        roots: &roots,
        aliases: &aliases,
        after_conversion: &[None],
        block: 0,
        work: 0,
    };
    // This component fixture uses two shared-reference locals. Copying the
    // owning view itself is a different operation and remains forbidden.
    let copy = typed_assignment(2, ty(5), SemanticRvalueKindV1::Use(typed_operand(3, ty(5))));
    audit.statement(copy.kind()).unwrap();
    let copy_owner = typed_assignment(2, ty(3), SemanticRvalueKindV1::Use(typed_operand(3, ty(3))));
    assert!(audit.statement(copy_owner.kind()).is_err());
}

#[test]
fn consumed_read_only_overwrite_with_distinct_root_rejects_even_identical_view_types() {
    let ty = SemanticTypeIdV1::from_index;
    let types = consumed_read_only_types_v1();
    let roots = [
        Some(consumed_read_only_audit_root_for_test_v1(0)),
        Some(consumed_read_only_audit_root_for_test_v1(1)),
    ];
    let aliases = [None, Some(0), Some(0), Some(1)];
    let mut audit = ConsumedReadOnlyAuditV1 {
        types: &types,
        roots: &roots,
        aliases: &aliases,
        after_conversion: &[None, None],
        block: 0,
        work: 0,
    };
    let overwrite = typed_assignment(
        2,
        ty(3),
        SemanticRvalueKindV1::Use(SemanticOperandV1::Move(typed_place(3, ty(3)))),
    );
    assert!(audit.statement(overwrite.kind()).is_err());
}

#[test]
fn consumed_read_only_unrelated_intrinsic_never_inherits_receiver_authority() {
    let mut callables = consumed_read_only_callables_v1();
    callables[1] = compiler_intrinsic_callable(SemanticCompilerIntrinsicOperationV1::FabsF32);
    assert!(
        audit_consumed_read_only_roots_v1(
            &consumed_read_only_types_v1(),
            &callables,
            &consumed_read_only_function_v1(consumed_read_only_blocks_v1()),
            &consumed_read_only_allocations_v1()
        )
        .is_err()
    );
}

#[test]
fn consumed_read_only_original_argument_copy_consumes_the_exact_custody() {
    let ty = SemanticTypeIdV1::from_index;
    let types = consumed_read_only_types_v1();
    let callables = consumed_read_only_callables_v1();
    let allocations = consumed_read_only_allocations_v1();
    let mut blocks = consumed_read_only_blocks_v1();
    blocks[0] = block(
        220,
        vec![],
        consumed_read_only_call_v1(0, vec![typed_operand(1, ty(2))], (2, 3), 1),
    );
    let function = consumed_read_only_function_v1(blocks);
    audit_consumed_read_only_roots_v1(&types, &callables, &function, &allocations).unwrap();
    let dominance = SemanticEnumPayloadDominanceV1::analyze(&function, &types).unwrap();
    let effects = project_authenticated_capabilities_v1(
        &types,
        &callables,
        &function,
        &dominance,
        &allocations,
        &[None; 11],
    )
    .unwrap();
    assert_eq!(
        effects.read_views[2]
            .unwrap()
            .view
            .allocation
            .allocation_origin,
        1
    );
    let mut state = ProjectedCapabilityStateV1::default();
    transfer_capability_terminator_v1(
        &callables,
        &function,
        0,
        &mut state,
        &allocations,
        &[None; 11],
        &[None; 11],
        &HashMap::new(),
        true,
    )
    .unwrap();
    assert_eq!(state.get(&1), Some(&ProjectedCapabilityValueV1::Invalid));
    assert!(matches!(
        state.get(&2),
        Some(ProjectedCapabilityValueV1::Known(
            ProjectedCapabilityOriginV1::ConsumedReadOnly(_)
        ))
    ));
}

#[test]
fn consumed_read_only_source_reuse_and_duplicate_conversion_fail_closed() {
    let ty = SemanticTypeIdV1::from_index;
    let types = consumed_read_only_types_v1();
    let mut callables = consumed_read_only_callables_v1();
    callables.push(compiler_intrinsic_callable(
        SemanticCompilerIntrinsicOperationV1::DisjointSliceLen {
            disjoint_slice: ty(2),
            element: ty(0),
            raw_index: ty(1),
            index_space: fe2o3_mir_model::semantic_mir_v1::SemanticDisjointIndexSpaceV1::Index1d,
        },
    ));
    for mutation in 0..4 {
        let mut blocks = consumed_read_only_blocks_v1();
        // An alias captured before conversion is just as consumed as its owner.
        blocks[0] = block(
            220,
            vec![typed_assignment(
                7,
                ty(4),
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: typed_place(1, ty(2)),
                },
            )],
            blocks[0].terminator().kind().clone(),
        );
        blocks[3] = match mutation {
            0 => block(
                221,
                vec![typed_assignment(
                    7,
                    ty(4),
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Shared,
                        place: typed_place(1, ty(2)),
                    },
                )],
                SemanticTerminatorKindV1::Return,
            ),
            1 => block(
                221,
                vec![],
                consumed_read_only_call_v1(3, vec![typed_operand(7, ty(4))], (5, 1), 4),
            ),
            2 => block(
                221,
                vec![],
                consumed_read_only_call_v1(0, vec![typed_operand(1, ty(2))], (8, 3), 4),
            ),
            _ => block(
                221,
                vec![],
                SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 0)),
            ),
        };
        blocks.push(block(222, vec![], SemanticTerminatorKindV1::Return));
        assert!(
            audit_consumed_read_only_roots_v1(
                &types,
                &callables,
                &consumed_read_only_function_v1(blocks),
                &consumed_read_only_allocations_v1(),
            )
            .is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn consumed_read_only_view_loops_and_source_storage_markers_remain_supported() {
    let mut blocks = consumed_read_only_blocks_v1();
    blocks[3] = block(
        221,
        vec![statement(SemanticStatementKindV1::StorageDead(
            SemanticLocalIdV1::from_index(1),
        ))],
        SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 2)),
    );
    let function = consumed_read_only_function_v1(blocks);
    let types = consumed_read_only_types_v1();
    let callables = consumed_read_only_callables_v1();
    let allocations = consumed_read_only_allocations_v1();
    audit_consumed_read_only_roots_v1(&types, &callables, &function, &allocations).unwrap();
    let dominance = SemanticEnumPayloadDominanceV1::analyze(&function, &types).unwrap();
    let effects = project_authenticated_capabilities_v1(
        &types,
        &callables,
        &function,
        &dominance,
        &allocations,
        &[None; 11],
    )
    .unwrap();
    assert!(effects.read_views[2].is_some());
}

#[test]
fn consumed_read_only_successor_custody_has_cumulative_exact_work_budget() {
    let function = consumed_read_only_function_v1(consumed_read_only_blocks_v1());
    let roots = [Some(consumed_read_only_audit_root_for_test_v1(0))];
    let mut work = 0;
    let expected = consumed_read_only_successor_custody_v1(&function, &roots, &mut work).unwrap();
    assert_eq!(expected, vec![Some(vec![false, true, true, true])]);
    let mut exact = MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1 - work;
    assert_eq!(
        consumed_read_only_successor_custody_v1(&function, &roots, &mut exact).unwrap(),
        expected
    );
    assert_eq!(exact, MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1);
    let mut short = MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1 - work + 1;
    assert!(consumed_read_only_successor_custody_v1(&function, &roots, &mut short).is_err());
    let roots = [roots[0], Some(consumed_read_only_audit_root_for_test_v1(1))];
    let mut insufficient_for_both = MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1 - work;
    assert!(
        consumed_read_only_successor_custody_v1(&function, &roots, &mut insufficient_for_both)
            .is_err()
    );
}
