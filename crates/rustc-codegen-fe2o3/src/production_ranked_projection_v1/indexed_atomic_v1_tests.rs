// These private component fixtures do not mint source admission authority.
include!("indexed_atomic_dead_cast_v1_tests.rs");
fn indexed_atomic_fixture_v1(
    marker: bool,
    escape: bool,
) -> (Vec<SemanticTypeDeclV1>, SemanticFunctionDeclV1) {
    let ty = SemanticTypeIdV1::from_index;
    let decl = |id, layout, shape| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(id)),
            SemanticLayoutIdentityV1::from_sha256(bytes(id)),
            layout,
            shape,
        )
    };
    let integer = |bits| {
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            bits,
            signed: false,
        })
    };
    let aggregate = |field| {
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![ty(field)]).unwrap())
    };
    let aggregate_layout = || {
        SemanticTypeLayoutV1::aggregate(
            Some(4),
            4,
            SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
        )
        .unwrap()
    };
    let pointer = |pointee, kind, metadata| {
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                ty(pointee),
                kind,
                SemanticMutabilityV1::Immutable,
                0,
                64,
                metadata,
            )
            .unwrap(),
        )
    };
    let mut types = vec![
        decl(
            210,
            SemanticTypeLayoutV1::new(Some(4), 4).unwrap(),
            integer(32),
        ), // 0 u32
        decl(211, aggregate_layout(), aggregate(2)), // 1 AtomicU32
        decl(212, aggregate_layout(), aggregate(0)), // 2 physical cell
        decl(
            213,
            SemanticTypeLayoutV1::new(None, 4).unwrap(),
            SemanticTypeShapeV1::Slice { element: ty(1) },
        ),
        decl(
            214,
            SemanticTypeLayoutV1::new(Some(16), 8).unwrap(),
            pointer(
                3,
                SemanticPointerKindV1::Reference,
                SemanticPointerMetadataV1::SliceLength,
            ),
        ),
        decl(
            215,
            SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
            pointer(
                1,
                SemanticPointerKindV1::Reference,
                SemanticPointerMetadataV1::None,
            ),
        ),
        decl(
            216,
            SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
            pointer(
                2,
                SemanticPointerKindV1::Raw,
                SemanticPointerMetadataV1::None,
            ),
        ),
        decl(
            217,
            SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
            pointer(
                0,
                SemanticPointerKindV1::Raw,
                SemanticPointerMetadataV1::None,
            ),
        ),
        decl(
            218,
            SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
            integer(64),
        ),
        decl(
            219,
            SemanticTypeLayoutV1::new(Some(1), 1).unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool),
        ),
    ];
    if marker {
        types[1] = types[1]
            .clone()
            .with_rust_type_kind(SemanticRustTypeKindV1::CoreAtomicU32);
    }
    types[4] = types[4].clone().with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
            Some(
                SemanticAbiPointeeInfoV1::new(
                    SemanticAbiPointeeKindV1::SharedReference { frozen: false },
                    0,
                    4,
                )
                .unwrap(),
            ),
            None,
        ),
    );
    let place = |local, kind| {
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty(kind)).unwrap()
    };
    let operand = |local, kind| SemanticOperandV1::Copy(place(local, kind));
    let assign = |local, kind, value| {
        statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(local, kind),
            SemanticRvalueV1::new(ty(kind), value),
        )))
    };
    let indexed = || {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(1),
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ty(3)).unwrap(),
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(2)),
                    ty(1),
                )
                .unwrap(),
            ],
            ty(1),
        )
        .unwrap()
    };
    let chain = |base| {
        vec![
            assign(
                base,
                5,
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: indexed(),
                },
            ),
            assign(
                base + 1,
                6,
                SemanticRvalueKindV1::AddressOf {
                    mutability: SemanticMutabilityV1::Immutable,
                    place: SemanticPlaceV1::new(
                        SemanticLocalIdV1::from_index(base),
                        vec![
                            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ty(1))
                                .unwrap(),
                            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), ty(2))
                                .unwrap(),
                        ],
                        ty(2),
                    )
                    .unwrap(),
                },
            ),
            assign(
                base + 2,
                7,
                SemanticRvalueKindV1::Cast {
                    kind: SemanticCastKindV1::Pointer,
                    operand: operand(base + 1, 6),
                },
            ),
        ]
    };
    let address = |local| {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(local),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ty(0)).unwrap()],
            ty(0),
        )
        .unwrap()
    };
    let release = SemanticAtomicAccessV1::new(
        SemanticAtomicOrderingV1::Release,
        SemanticAtomicScopeV1::System,
    );
    let acquire = SemanticAtomicAccessV1::new(
        SemanticAtomicOrderingV1::Acquire,
        SemanticAtomicScopeV1::System,
    );
    let mut store = chain(5);
    if escape {
        store.push(assign(
            11,
            6,
            SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Immutable,
                place: place(5, 5),
            },
        ));
    }
    store.push(statement(SemanticStatementKindV1::Store(
        SemanticMemoryStoreV1::new(
            address(7),
            typed_constant(ty(0), 17, 4),
            SemanticVolatilityV1::NonVolatile,
            Some(release),
        ),
    )));
    let mut load = chain(8);
    load.push(assign(
        0,
        0,
        SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
            address(10),
            SemanticVolatilityV1::NonVolatile,
            Some(acquire),
        )),
    ));
    let blocks = vec![
        block(
            220,
            vec![
                assign(
                    3,
                    8,
                    SemanticRvalueKindV1::Unary {
                        operation: SemanticUnaryOpV1::PointerMetadata,
                        operand: operand(1, 4),
                    },
                ),
                assign(
                    4,
                    9,
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::LessThan,
                        left: operand(2, 8),
                        right: operand(3, 8),
                    },
                ),
            ],
            SemanticTerminatorKindV1::Assert {
                condition: operand(4, 9),
                expected: true,
                message: SemanticAssertMessageV1::BoundsCheck {
                    length: operand(3, 8),
                    index: operand(2, 8),
                },
                target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, 1),
                unwind: SemanticUnwindActionV1::Unreachable,
            },
        ),
        block(
            221,
            store,
            SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 2)),
        ),
        block(222, load, SemanticTerminatorKindV1::Return),
    ];
    let local_types = [0, 4, 8, 8, 9, 5, 6, 7, 5, 6, 7, 6];
    let locals = local_types
        .into_iter()
        .enumerate()
        .map(|(i, kind)| {
            local(
                i as u8,
                ty(kind),
                match i {
                    0 => SemanticLocalRoleV1::Return,
                    1 => SemanticLocalRoleV1::Argument(0),
                    2 => SemanticLocalRoleV1::Argument(1),
                    _ => SemanticLocalRoleV1::Temporary,
                },
            )
        })
        .collect();
    let abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256(bytes(223)),
        SemanticLayoutIdentityV1::from_sha256(bytes(223)),
        SemanticCanonAbiV1::GpuKernel,
        false,
        false,
        vec![
            SemanticAbiValueV1::new(
                ty(4),
                SemanticAbiPassModeV1::Pair {
                    first: SemanticAbiValueAttributesV1::plain(),
                    second: SemanticAbiValueAttributesV1::plain(),
                },
            ),
            SemanticAbiValueV1::new(
                ty(8),
                SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::plain()),
            ),
        ],
        SemanticAbiValueV1::new(ty(0), SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![
        SemanticSourceArgumentOwnershipV1::SharedBorrow,
        SemanticSourceArgumentOwnershipV1::ByValue,
    ])
    .unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(224)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(225)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(226)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(227)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(228)),
        SemanticSourceProvenanceV1::unavailable(),
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap();
    (types, function)
}

fn indexed_atomic_inventory_fixture_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
) -> (
    Vec<ProjectedBoundsCheckV1>,
    Vec<Option<AllocationContractV1>>,
) {
    let checks = project_rust_bounds_checks_with_ordinary_v1(
        types,
        function,
        0,
        &vec![None; function.locals().len()],
        &[],
        None,
        &mut Vec::new(),
        &mut 0,
    )
    .unwrap()
    .checks;
    let mut contracts = vec![None; function.locals().len()];
    contracts[1] = Some(AllocationContractV1 {
        allocation_origin: 1,
        noalias_class: 1,
        writable: true,
        singleton_object: false,
    });
    (checks, contracts)
}

#[test]
fn indexed_atomic_keeps_nominal_root_guard_and_exact_effects() {
    let (types, function) = indexed_atomic_fixture_v1(true, false);
    let (checks, contracts) = indexed_atomic_inventory_fixture_v1(&types, &function);
    let inventory =
        authenticated_atomic_allocations_v1(&types, &function, &checks, &contracts).unwrap();
    assert_eq!(inventory.indexed_uses.len(), 2);
    assert_eq!(inventory.coherent, vec![1]);
    for usage in &inventory.indexed_uses {
        assert_eq!(usage.origin.place.local().index(), 1);
        assert_eq!(usage.origin.guard.access_block, 1);
        assert_eq!(
            usage.origin.guard.extent_source,
            ProjectedBoundsExtentSourceV1::Slice(SemanticLocalIdV1::from_index(1))
        );
        assert_eq!(
            usage.origin.guard.index_identity,
            ProjectedBoundsIndexIdentityV1::Local(SemanticLocalIdV1::from_index(2))
        );
        assert_eq!(usage.atomic.scope(), SemanticAtomicScopeV1::System);
    }
    assert_eq!(
        inventory.indexed_uses[0].atomic.ordering(),
        SemanticAtomicOrderingV1::Release
    );
    assert_eq!(
        inventory.indexed_uses[1].atomic.ordering(),
        SemanticAtomicOrderingV1::Acquire
    );
    assert!(inventory.authorizes_bounds(checks[0]).unwrap());
}

#[test]
fn indexed_atomic_rejects_missing_wrong_future_or_duplicate_guards() {
    let (types, function) = indexed_atomic_fixture_v1(true, false);
    let (checks, contracts) = indexed_atomic_inventory_fixture_v1(&types, &function);
    for mutation in 0..5 {
        let mut guards = checks.clone();
        match mutation {
            0 => guards.clear(),
            1 => {
                guards[0].extent_source =
                    ProjectedBoundsExtentSourceV1::Slice(SemanticLocalIdV1::from_index(2))
            }
            2 => guards[0].index_identity = ProjectedBoundsIndexIdentityV1::Literal(0),
            3 => guards[0].access_block = 2,
            _ => guards.push(guards[0]),
        }
        let result = authenticated_atomic_allocations_v1(&types, &function, &guards, &contracts);
        if mutation == 4 {
            assert!(result.is_err());
        } else {
            let result = result.unwrap();
            assert!(result.indexed_uses.iter().all(|usage| usage.block != 1));
            if mutation != 3 {
                assert!(result.coherent.is_empty());
            }
        }
    }
}

#[test]
fn indexed_atomic_marker_and_escape_are_not_structural_authority() {
    for (marker, escape) in [(false, false), (true, true)] {
        let (types, function) = indexed_atomic_fixture_v1(marker, escape);
        let (checks, contracts) = indexed_atomic_inventory_fixture_v1(&types, &function);
        let result =
            authenticated_atomic_allocations_v1(&types, &function, &checks, &contracts).unwrap();
        assert!(result.indexed_uses.iter().all(|usage| usage.block != 1));
        if !marker {
            assert!(result.coherent.is_empty());
        }
    }
}

#[test]
fn indexed_atomic_coherence_preserves_singletons_and_rejects_bad_slice_contracts() {
    let (types, function) = indexed_atomic_fixture_v1(true, false);
    let (checks, mut contracts) = indexed_atomic_inventory_fixture_v1(&types, &function);
    contracts[0] = Some(AllocationContractV1 {
        allocation_origin: 77,
        noalias_class: 78,
        writable: true,
        singleton_object: true,
    });
    let result =
        authenticated_atomic_allocations_v1(&types, &function, &checks, &contracts).unwrap();
    assert_eq!(result.coherent, vec![1, 77]);
    contracts[1].as_mut().unwrap().writable = false;
    assert!(authenticated_atomic_allocations_v1(&types, &function, &checks, &contracts).is_err());
    contracts[1] = None;
    assert!(authenticated_atomic_allocations_v1(&types, &function, &checks, &contracts).is_err());
}
#[test]
fn indexed_atomic_rejects_single_definition_reexecuted_by_backedge() {
    let (types, function) = indexed_atomic_fixture_v1(true, false);
    let (checks, allocations) = indexed_atomic_inventory_fixture_v1(&types, &function);
    let mut blocks = function.blocks().to_vec();
    blocks[2] = block(
        229,
        blocks[2].statements().to_vec(),
        SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
    );
    let cyclic = SemanticFunctionDeclV1::new(
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
    // Both borrows still have one syntactic definition, but each executes repeatedly.
    assert_eq!(local_definition_counts(&cyclic)[5], 1);
    assert_eq!(local_definition_counts(&cyclic)[8], 1);
    assert!(
        authenticated_atomic_allocations_v1(&types, &cyclic, &checks, &allocations)
            .unwrap()
            .indexed_uses
            .is_empty()
    );
}

#[test]
fn indexed_atomic_call_defined_index_requires_one_acyclic_definition() {
    let index = SemanticLocalIdV1::from_index(2);
    let destination = SemanticPlaceV1::new(index, vec![], SCALAR_TYPE).unwrap();
    let call = SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(0),
            vec![],
            Some(SemanticCallDestinationV1::new(
                destination,
                cfg_edge(SemanticEdgeRoleV1::CallReturn, 1),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    );
    for cyclic in [false, true] {
        let function = projection_function(vec![
            block(231, vec![], call.clone()),
            block(
                232,
                vec![],
                if cyclic {
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 0))
                } else {
                    SemanticTerminatorKindV1::Return
                },
            ),
        ]);
        let types = projection_types();
        let mut proofs = SemanticAssertProofsV1::new(&types, &function).unwrap();
        assert_eq!(proofs.definition_counts[2], 1);
        assert!(proofs.assignments[2].is_none());
        assert_eq!(
            indexed_atomic_index_stable_v1(&mut proofs, index).unwrap(),
            !cyclic
        );
        proofs.block_definitions.iter_mut().for_each(Vec::clear);
        assert!(!indexed_atomic_index_stable_v1(&mut proofs, index).unwrap());
    }
}

fn indexed_atomic_project_for_test(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    checks: &[ProjectedBoundsCheckV1],
    contracts: &ProjectionLocalContractsV1,
    block: usize,
    address: &SemanticPlaceV1,
    effect: (AccessKindAttr, Option<SemanticAtomicAccessV1>),
) -> Result<Vec<GuardedAccessSiteV1>, ProductionRankedProjectionErrorV1> {
    let (access, atomic) = effect;
    let mut guarded = Vec::new();
    project_place_access_with_atomic(
        types,
        function,
        block,
        checks,
        address,
        access,
        atomic,
        PlaceAccessRequirementV1::ExplicitMemory,
        SemanticSourceProvenanceV1::unavailable(),
        &constant_locals(function)?,
        contracts,
        &[],
        &mut guarded,
        &mut vec![None; function.locals().len()],
        &mut Vec::new(),
        &mut Vec::new(),
        &mut 0,
        &mut String::new(),
    )?;
    Ok(guarded)
}

#[test]
fn indexed_atomic_guarded_projection_preserves_effect_and_rejects_laundering() {
    let (types, function) = indexed_atomic_fixture_v1(true, false);
    let (checks, allocations) = indexed_atomic_inventory_fixture_v1(&types, &function);
    let mut contracts = synthetic_local_contracts_with_types(&function, &types);
    contracts.allocations = allocations;
    contracts.atomic_allocations =
        authenticated_atomic_allocations_v1(&types, &function, &checks, &contracts.allocations)
            .unwrap();
    for usage in &contracts.atomic_allocations.indexed_uses {
        let projected = indexed_atomic_project_for_test(
            &types,
            &function,
            &checks,
            &contracts,
            usage.block,
            &usage.address,
            (usage.access, Some(usage.atomic)),
        )
        .unwrap();
        assert_eq!(projected.len(), 1);
        assert_eq!(projected[0].access.atomic, Some(usage.atomic));
        assert_eq!(projected[0].access.failure, GuardedAccessFailureV1::Trap);
        assert_eq!(
            projected[0].access.comparisons,
            vec![(usage.origin.guard.index, usage.origin.guard.extent)]
        );
        assert!(
            indexed_atomic_project_for_test(
                &types,
                &function,
                &checks,
                &contracts,
                usage.block,
                &usage.address,
                (AccessKindAttr::Read, None)
            )
            .is_err()
        );
        assert!(
            indexed_atomic_project_for_test(
                &types,
                &function,
                &checks,
                &contracts,
                usage.origin.guard.access_block,
                &usage.origin.place,
                (AccessKindAttr::Read, None)
            )
            .is_err()
        );
        for atomic in [
            SemanticAtomicAccessV1::new(
                SemanticAtomicOrderingV1::Relaxed,
                SemanticAtomicScopeV1::System,
            ),
            SemanticAtomicAccessV1::new(usage.atomic.ordering(), SemanticAtomicScopeV1::Agent),
        ] {
            assert!(
                indexed_atomic_project_for_test(
                    &types,
                    &function,
                    &checks,
                    &contracts,
                    usage.block,
                    &usage.address,
                    (usage.access, Some(atomic))
                )
                .is_err()
            );
        }
        let wrong_kind = if usage.access == AccessKindAttr::AtomicRead {
            AccessKindAttr::AtomicWrite
        } else {
            AccessKindAttr::AtomicRead
        };
        assert!(
            indexed_atomic_project_for_test(
                &types,
                &function,
                &checks,
                &contracts,
                usage.block,
                &usage.address,
                (wrong_kind, Some(usage.atomic))
            )
            .is_err()
        );
    }
}

fn indexed_atomic_replace_block_v1(
    function: &SemanticFunctionDeclV1,
    index: usize,
    statements: Vec<SemanticStatementV1>,
) -> SemanticFunctionDeclV1 {
    let mut blocks = function.blocks().to_vec();
    blocks[index] = SemanticBasicBlockV1::new(
        blocks[index].identity(),
        blocks[index].source(),
        statements,
        blocks[index].terminator().clone(),
    )
    .unwrap();
    SemanticFunctionDeclV1::new(
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
    .unwrap()
}

#[test]
fn indexed_atomic_rejects_redefined_index_and_pointer_escape() {
    let (types, function) = indexed_atomic_fixture_v1(true, false);
    let (checks, allocations) = indexed_atomic_inventory_fixture_v1(&types, &function);
    let mut statements = function.blocks()[1].statements().to_vec();
    statements.push(statement(SemanticStatementKindV1::Assign(
        SemanticAssignmentV1::new(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(2),
                vec![],
                SemanticTypeIdV1::from_index(8),
            )
            .unwrap(),
            SemanticRvalueV1::new(
                SemanticTypeIdV1::from_index(8),
                SemanticRvalueKindV1::Use(typed_constant(SemanticTypeIdV1::from_index(8), 0, 8)),
            ),
        ),
    )));
    let changed = indexed_atomic_replace_block_v1(&function, 1, statements);
    assert!(
        authenticated_atomic_allocations_v1(&types, &changed, &checks, &allocations)
            .unwrap()
            .indexed_uses
            .is_empty()
    );
    let mut statements = function.blocks()[1].statements().to_vec();
    statements.push(statement(SemanticStatementKindV1::Assign(
        SemanticAssignmentV1::new(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(11),
                vec![],
                SemanticTypeIdV1::from_index(6),
            )
            .unwrap(),
            SemanticRvalueV1::new(
                SemanticTypeIdV1::from_index(6),
                SemanticRvalueKindV1::Cast {
                    kind: SemanticCastKindV1::Pointer,
                    operand: SemanticOperandV1::Copy(
                        SemanticPlaceV1::new(
                            SemanticLocalIdV1::from_index(7),
                            vec![],
                            SemanticTypeIdV1::from_index(7),
                        )
                        .unwrap(),
                    ),
                },
            ),
        ),
    )));
    let changed = indexed_atomic_replace_block_v1(&function, 1, statements);
    assert!(authenticated_atomic_allocations_v1(&types, &changed, &checks, &allocations).is_err());
}

#[test]
fn indexed_atomic_statement_binding_and_work_budget_fail_closed() {
    let (types, function) = indexed_atomic_fixture_v1(true, false);
    let (checks, allocations) = indexed_atomic_inventory_fixture_v1(&types, &function);
    let inventory =
        authenticated_atomic_allocations_v1(&types, &function, &checks, &allocations).unwrap();
    let usage = &inventory.indexed_uses[0];
    let record = &function.blocks()[usage.block].statements()[usage.statement];
    inventory
        .validate_statement(usage.block, usage.statement, record)
        .unwrap();
    assert!(
        inventory
            .validate_statement(
                usage.block,
                usage.statement,
                &statement(SemanticStatementKindV1::Nop)
            )
            .is_err()
    );
    let last = MAX_PROJECTED_LOOP_GRAPH_WORK_V1 - inventory.indexed_locals.len();
    inventory.work.set(last);
    assert!(
        inventory
            .contains_local(SemanticLocalIdV1::from_index(7))
            .unwrap()
    );
    assert_eq!(inventory.work.get(), MAX_PROJECTED_LOOP_GRAPH_WORK_V1);
    assert!(
        inventory
            .contains_local(SemanticLocalIdV1::from_index(7))
            .is_err()
    );
    assert_eq!(inventory.work.get(), MAX_PROJECTED_LOOP_GRAPH_WORK_V1);
}

#[test]
fn indexed_atomic_guarded_cfg_keeps_atomic_order_scope_and_trap() {
    let function = projection_function(vec![block(230, vec![], SemanticTerminatorKindV1::Return)]);
    let atomic = SemanticAtomicAccessV1::new(
        SemanticAtomicOrderingV1::Release,
        SemanticAtomicScopeV1::System,
    );
    let guarded = GuardedRankedAccessV1 {
        view: ProductionRankedValueIdV1::new(0),
        indices: vec![ProductionRankedValueV1::Argument(0)],
        checked_success: None,
        failure: GuardedAccessFailureV1::Trap,
        atomic: Some(atomic),
        comparisons: vec![(
            ProductionRankedValueV1::Argument(0),
            ProductionRankedValueV1::Argument(1),
        )],
        access: AccessKindAttr::AtomicWrite,
        memory_space: MemorySpaceAttr::Global,
        source: SemanticSourceProvenanceV1::unavailable(),
        semantic_site: Some(ProjectedSemanticAccessSiteV1 {
            block: 0,
            statement: Some(0),
        }),
    };
    for bad_failure in [false, true] {
        let mut guarded = guarded.clone();
        if bad_failure {
            guarded.failure = GuardedAccessFailureV1::ContinueWithoutAccess;
        }
        let result = build_ranked_cfg(
            &projection_types(),
            &function,
            &[],
            &[],
            &[],
            &[],
            vec![],
            vec![ProjectedSemanticBlockV1 {
                items: vec![ProjectedBlockItemV1::Guarded(guarded)],
            }],
        );
        if bad_failure {
            assert!(result.is_err());
            continue;
        }
        let (blocks, sources, _) = result.unwrap();
        let source = sources[0];
        assert!(matches!(
            &blocks[source.block].operations()[0],
            ProductionRankedOperationV1::AtomicAccess {
                kind: AccessKindAttr::AtomicWrite,
                ordering: AtomicOrderingAttr::Release,
                scope: AtomicScopeAttr::System,
                ..
            }
        ));
        assert!(matches!(
            blocks[source.block + 1].terminator(),
            ProductionRankedTerminatorV1::Trap
        ));
        assert_eq!(sources[0].semantic_site.unwrap().statement, Some(0));
    }
}
