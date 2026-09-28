use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

fn ty(index: u32) -> SemanticTypeIdV1 {
    SemanticTypeIdV1::from_index(index)
}
fn source() -> SemanticSourceProvenanceV1 {
    SemanticSourceProvenanceV1::unavailable()
}
fn local(index: u32) -> SemanticLocalIdV1 {
    SemanticLocalIdV1::from_index(index)
}
fn statement(kind: SemanticStatementKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(source(), kind)
}
fn place(index: u32, kind: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(local(index), vec![], ty(kind)).unwrap()
}
fn projected(index: u32, projections: &[(SemanticProjectionKindV1, u32)]) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        local(index),
        projections
            .iter()
            .map(|&(kind, result)| SemanticProjectionV1::new(kind, ty(result)).unwrap())
            .collect(),
        ty(projections.last().unwrap().1),
    )
    .unwrap()
}
fn dereference(index: u32, kind: u32) -> SemanticPlaceV1 {
    projected(index, &[(SemanticProjectionKindV1::Dereference, kind)])
}
fn field_read(index: u32) -> SemanticPlaceV1 {
    projected(
        index,
        &[
            (SemanticProjectionKindV1::Field(1), 1),
            (SemanticProjectionKindV1::Dereference, 0),
        ],
    )
}
fn constant(kind: u32) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        ty(kind),
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(17, 4).unwrap()),
    ))
}
fn assign(destination: SemanticPlaceV1, value: SemanticRvalueKindV1) -> SemanticStatementV1 {
    let result = destination.ty();
    statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
        destination,
        SemanticRvalueV1::new(result, value),
    )))
}
fn assign_operand(destination: SemanticPlaceV1, value: SemanticOperandV1) -> SemanticStatementV1 {
    assign(destination, SemanticRvalueKindV1::Use(value))
}
fn borrow(
    destination: u32,
    reference: u32,
    referent: u32,
    pointee: u32,
    kind: SemanticBorrowKindV1,
) -> SemanticStatementV1 {
    assign(
        place(destination, reference),
        SemanticRvalueKindV1::Borrow {
            kind,
            place: place(referent, pointee),
        },
    )
}
fn shared() -> SemanticStatementV1 {
    borrow(2, 1, 1, 0, SemanticBorrowKindV1::Shared)
}
fn read(index: u32) -> SemanticStatementV1 {
    assign_operand(place(5, 0), SemanticOperandV1::Copy(dereference(index, 0)))
}
fn dead(index: u32) -> SemanticStatementV1 {
    statement(SemanticStatementKindV1::StorageDead(local(index)))
}
fn tuple_holder() -> SemanticStatementV1 {
    assign(
        place(4, 2),
        SemanticRvalueKindV1::aggregate(
            SemanticAggregateKindV1::Tuple,
            vec![constant(0), SemanticOperandV1::Move(place(2, 1))],
        )
        .unwrap(),
    )
}
fn declaration(index: usize, shape: SemanticTypeShapeV1) -> SemanticTypeDeclV1 {
    let layout = match &shape {
        SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_) => {
            SemanticTypeLayoutV1::new(Some(4), 4).unwrap()
        }
        SemanticTypeShapeV1::Tuple(_) | SemanticTypeShapeV1::Aggregate(_) => {
            let (size, second) = if index == 4 {
                (24, 8)
            } else if index >= 10 {
                (1u64 << (index - 6), 1u64 << (index - 7))
            } else {
                (16, 8)
            };
            SemanticTypeLayoutV1::aggregate(
                Some(size),
                8,
                SemanticAggregateLayoutV1::new(vec![0, second], vec![]).unwrap(),
            )
            .unwrap()
        }
        _ => SemanticTypeLayoutV1::new(Some(if index == 7 { 16 } else { 8 }), 8).unwrap(),
    };
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([index as u8 + 1; 32]),
        SemanticLayoutIdentityV1::from_sha256([index as u8 + 40; 32]),
        layout,
        shape,
    )
}
fn types() -> Vec<SemanticTypeDeclV1> {
    let pointer = |pointee, kind, mutability, metadata| {
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(ty(pointee), kind, mutability, 5, 64, metadata)
                .unwrap(),
        )
    };
    let tuple = |fields: Vec<u32>| {
        SemanticTypeShapeV1::Tuple(
            SemanticAggregateTypeV1::new(fields.into_iter().map(ty).collect()).unwrap(),
        )
    };
    let scalar = SemanticScalarTypeV1::Integer {
        signed: false,
        bits: 32,
    };
    vec![
        SemanticTypeShapeV1::Scalar(scalar),
        pointer(
            0,
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
            SemanticPointerMetadataV1::None,
        ),
        tuple(vec![0, 1]),
        tuple(vec![1, 1]),
        tuple(vec![0, 2]),
        pointer(
            0,
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Mutable,
            SemanticPointerMetadataV1::None,
        ),
        pointer(
            0,
            SemanticPointerKindV1::Raw,
            SemanticMutabilityV1::Immutable,
            SemanticPointerMetadataV1::None,
        ),
        pointer(
            0,
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
            SemanticPointerMetadataV1::SliceLength,
        ),
        SemanticTypeShapeV1::ValidityScalar(
            SemanticValidityScalarTypeV1::new(
                scalar,
                vec![SemanticScalarValidityRangeV1::new(1, u32::MAX as u128)],
            )
            .unwrap(),
        ),
        pointer(
            8,
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
            SemanticPointerMetadataV1::None,
        ),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, shape)| declaration(index, shape))
    .collect()
}
fn block(
    index: u8,
    statements: Vec<SemanticStatementV1>,
    terminator: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([index + 100; 32]),
        source(),
        statements,
        SemanticTerminatorV1::new(source(), terminator),
    )
    .unwrap()
}
fn function_with_locals(
    local_types: &[u32],
    blocks: Vec<SemanticBasicBlockV1>,
    role: SemanticFunctionRoleV1,
) -> SemanticFunctionDeclV1 {
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([201; 32]),
        SemanticLayoutIdentityV1::from_sha256([202; 32]),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        0,
        vec![],
        SemanticAbiValueV1::new(ty(local_types[0]), SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([203; 32]),
        role,
        SemanticItemDefinitionIdentityV1::from_sha256([204; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([205; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([206; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([207; 32]),
        source(),
        abi,
        local_types
            .iter()
            .enumerate()
            .map(|(index, &kind)| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([index as u8 + 60; 32]),
                    ty(kind),
                    if index == 0 {
                        SemanticLocalRoleV1::Return
                    } else {
                        SemanticLocalRoleV1::Temporary
                    },
                    source(),
                )
            })
            .collect(),
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
}
fn function_with_end(
    statements: Vec<SemanticStatementV1>,
    terminator: SemanticTerminatorKindV1,
) -> SemanticFunctionDeclV1 {
    function_with_locals(
        &[0, 0, 1, 1, 2, 0, 3, 4, 9, 8, 5, 6, 7, 8],
        vec![block(0, statements, terminator)],
        SemanticFunctionRoleV1::KernelRoot,
    )
}
fn function(statements: Vec<SemanticStatementV1>) -> SemanticFunctionDeclV1 {
    function_with_end(statements, SemanticTerminatorKindV1::Return)
}
fn effects() -> NominalReferenceEffectsV29 {
    NominalReferenceEffectsV29 {
        parameters: vec![],
        parameter_count: 0,
        scratch_peak: 0,
    }
}
fn evaluate(
    function: &SemanticFunctionDeclV1,
    types: &[SemanticTypeDeclV1],
) -> (
    BTreeSet<SemanticTransparentBorrowSiteV1>,
    ProductionSemanticSsaSummaryV1,
) {
    let mut summary = ProductionSemanticSsaSummaryV1::default();
    let sites = effects()
        .borrow_sites(
            function,
            &[],
            types,
            ProductionSemanticSsaLimitsV1::default(),
            &mut summary,
        )
        .unwrap();
    (sites, summary)
}
fn assert_closed(function: &SemanticFunctionDeclV1, types: &[SemanticTypeDeclV1], count: usize) {
    let (sites, _) = evaluate(function, types);
    assert_eq!(sites.len(), count);
    let empty = BTreeSet::new();
    let old = semantic_function_ssa_input_v1(function, Some(types), &[], &empty).0;
    let new = semantic_function_ssa_input_v1(function, Some(types), &[], &sites).0;
    assert!(!old.promotable()[1]);
    assert!(new.promotable()[1]);
    assert_eq!(old.blocks().len(), new.blocks().len());
    for (old, new) in old.blocks().iter().zip(new.blocks()) {
        assert_eq!(old.events(), new.events());
    }
}

#[test]
fn shared_primitive_direct_scalar_and_validity_preserve_source_events() {
    let types = types();
    assert_closed(
        &function(vec![
            assign_operand(place(1, 0), constant(0)),
            shared(),
            read(2),
            dead(2),
        ]),
        &types,
        1,
    );
    let validity = function(vec![
        borrow(8, 9, 9, 8, SemanticBorrowKindV1::Shared),
        assign_operand(place(13, 8), SemanticOperandV1::Copy(dereference(8, 8))),
        dead(8),
    ]);
    let (sites, _) = evaluate(&validity, &types);
    assert_eq!(sites.len(), 1);
    assert!(
        semantic_function_ssa_input_v1(&validity, Some(&types), &[], &sites)
            .0
            .promotable()[9]
    );
}

#[test]
fn shared_primitive_tuple_move_distinguishes_unrelated_scalar_field() {
    let types = types();
    for role in [
        SemanticFunctionRoleV1::KernelRoot,
        SemanticFunctionRoleV1::InternalHelper,
    ] {
        let statements = vec![
            assign_operand(place(1, 0), constant(0)),
            shared(),
            tuple_holder(),
            assign_operand(
                projected(4, &[(SemanticProjectionKindV1::Field(0), 0)]),
                constant(0),
            ),
            assign_operand(
                place(5, 0),
                SemanticOperandV1::Move(projected(4, &[(SemanticProjectionKindV1::Field(0), 0)])),
            ),
            assign_operand(place(5, 0), SemanticOperandV1::Copy(field_read(4))),
            dead(4),
            dead(2),
            assign_operand(place(1, 0), constant(0)),
        ];
        let function = function_with_locals(
            &[0, 0, 1, 1, 2, 0, 3, 4, 9, 8, 5, 6, 7, 8],
            vec![block(0, statements, SemanticTerminatorKindV1::Return)],
            role,
        );
        assert_closed(&function, &types, 1);
        assert_eq!(evaluate(&function, &types), evaluate(&function, &types));
    }
}

#[test]
fn shared_primitive_nested_static_paths_and_aggregate_shapes() {
    let mut types = types();
    let outer = assign(
        place(7, 4),
        SemanticRvalueKindV1::aggregate(
            SemanticAggregateKindV1::Tuple,
            vec![constant(0), SemanticOperandV1::Move(place(4, 2))],
        )
        .unwrap(),
    );
    let nested = projected(
        7,
        &[
            (SemanticProjectionKindV1::Field(1), 2),
            (SemanticProjectionKindV1::Field(1), 1),
            (SemanticProjectionKindV1::Dereference, 0),
        ],
    );
    assert_closed(
        &function(vec![
            shared(),
            tuple_holder(),
            outer,
            assign_operand(place(5, 0), SemanticOperandV1::Copy(nested)),
            dead(7),
        ]),
        &types,
        1,
    );
    types[2] = declaration(
        2,
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![ty(0), ty(1)]).unwrap()),
    );
    let aggregate = assign(
        place(4, 2),
        SemanticRvalueKindV1::aggregate(
            SemanticAggregateKindV1::Aggregate,
            vec![constant(0), SemanticOperandV1::Move(place(2, 1))],
        )
        .unwrap(),
    );
    assert_closed(
        &function(vec![
            shared(),
            aggregate,
            assign_operand(place(5, 0), SemanticOperandV1::Copy(field_read(4))),
            dead(4),
        ]),
        &types,
        1,
    );
}

#[test]
fn shared_primitive_projected_reference_redefinition_closes_only_old_leaf() {
    let function = function(vec![
        shared(),
        read(2),
        borrow(3, 1, 1, 0, SemanticBorrowKindV1::Shared),
        tuple_holder(),
        assign_operand(
            projected(4, &[(SemanticProjectionKindV1::Field(1), 1)]),
            SemanticOperandV1::Move(place(3, 1)),
        ),
        assign_operand(place(5, 0), SemanticOperandV1::Copy(field_read(4))),
        dead(4),
        assign_operand(place(1, 0), constant(0)),
    ]);
    assert_closed(&function, &types(), 2);
}

#[test]
fn shared_primitive_site_does_not_override_other_storage_observation() {
    let function = function(vec![
        assign(
            place(11, 6),
            SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Immutable,
                place: place(1, 0),
            },
        ),
        shared(),
        read(2),
        dead(2),
    ]);
    let types = types();
    let (sites, _) = evaluate(&function, &types);
    assert_eq!(sites.len(), 1);
    assert!(transparent_borrow_sites_v1(&function, &[]).is_empty());
    assert!(
        !semantic_function_ssa_input_v1(&function, Some(&types), &[], &sites)
            .0
            .promotable()[1]
    );
}

#[test]
fn shared_primitive_copy_aliases_all_remain_live_until_closed() {
    for close_copy in [false, true] {
        let mut statements = vec![
            shared(),
            assign_operand(place(3, 1), SemanticOperandV1::Copy(place(2, 1))),
            read(2),
            read(3),
            dead(2),
        ];
        if close_copy {
            statements.push(dead(3));
        }
        statements.push(assign_operand(place(1, 0), constant(0)));
        if !close_copy {
            statements.push(read(3));
        }
        assert_eq!(
            evaluate(&function(statements), &types()).0.len(),
            usize::from(close_copy)
        );
    }
}

#[test]
fn shared_primitive_moves_consume_only_selected_reference_fields() {
    let pair = assign(
        place(6, 3),
        SemanticRvalueKindV1::aggregate(
            SemanticAggregateKindV1::Tuple,
            vec![
                SemanticOperandV1::Copy(place(2, 1)),
                SemanticOperandV1::Move(place(2, 1)),
            ],
        )
        .unwrap(),
    );
    for stale in [false, true] {
        let mut statements = vec![
            shared(),
            pair.clone(),
            assign_operand(
                place(3, 1),
                SemanticOperandV1::Move(projected(6, &[(SemanticProjectionKindV1::Field(0), 1)])),
            ),
            read(3),
            assign_operand(
                place(5, 0),
                SemanticOperandV1::Copy(projected(
                    6,
                    &[
                        (SemanticProjectionKindV1::Field(1), 1),
                        (SemanticProjectionKindV1::Dereference, 0),
                    ],
                )),
            ),
        ];
        if stale {
            statements.push(assign_operand(
                place(5, 0),
                SemanticOperandV1::Copy(projected(
                    6,
                    &[
                        (SemanticProjectionKindV1::Field(0), 1),
                        (SemanticProjectionKindV1::Dereference, 0),
                    ],
                )),
            ));
        }
        assert_eq!(
            evaluate(&function(statements), &types()).0.len(),
            usize::from(!stale)
        );
    }
    assert!(
        evaluate(
            &function(vec![
                shared(),
                assign_operand(place(3, 1), SemanticOperandV1::Move(place(2, 1))),
                read(3),
                read(2)
            ]),
            &types()
        )
        .0
        .is_empty()
    );
}

#[test]
fn shared_primitive_referent_changes_moves_and_lifetime_resets_refuse() {
    let mutations = vec![
        assign_operand(place(1, 0), constant(0)),
        assign_operand(place(5, 0), SemanticOperandV1::Move(place(1, 0))),
        statement(SemanticStatementKindV1::Deinitialize(place(1, 0))),
        dead(1),
        statement(SemanticStatementKindV1::StorageLive(local(1))),
    ];
    for mutation in mutations {
        assert!(
            evaluate(
                &function(vec![shared(), read(2), mutation, read(2)]),
                &types()
            )
            .0
            .is_empty()
        );
    }
    assert!(
        evaluate(
            &function(vec![
                shared(),
                assign_operand(place(5, 0), SemanticOperandV1::Move(dereference(2, 0)))
            ]),
            &types()
        )
        .0
        .is_empty()
    );
}

#[test]
fn shared_primitive_holder_end_deinitialize_and_redefinition_close_old_loans() {
    for end in [
        dead(2),
        statement(SemanticStatementKindV1::Deinitialize(place(2, 1))),
        statement(SemanticStatementKindV1::StorageLive(local(2))),
    ] {
        assert_closed(
            &function(vec![
                shared(),
                read(2),
                end.clone(),
                assign_operand(place(1, 0), constant(0)),
            ]),
            &types(),
            1,
        );
        assert!(
            evaluate(&function(vec![shared(), read(2), end, read(2)]), &types())
                .0
                .is_empty()
        );
    }
    assert_closed(
        &function(vec![
            shared(),
            read(2),
            borrow(2, 1, 5, 0, SemanticBorrowKindV1::Shared),
            assign_operand(place(1, 0), constant(0)),
            dead(2),
        ]),
        &types(),
        1,
    );
}

#[test]
fn shared_primitive_explicit_loads_keep_the_current_production_pointer_representation() {
    for (volatility, atomic) in [
        (SemanticVolatilityV1::NonVolatile, None),
        (SemanticVolatilityV1::Volatile, None),
        (
            SemanticVolatilityV1::NonVolatile,
            Some(SemanticAtomicAccessV1::new(
                SemanticAtomicOrderingV1::Relaxed,
                SemanticAtomicScopeV1::Device,
            )),
        ),
    ] {
        let load = assign(
            place(5, 0),
            SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                dereference(2, 0),
                volatility,
                atomic,
            )),
        );
        for prefix in [vec![shared()], vec![shared(), read(2)]] {
            let mut statements = prefix;
            statements.extend([load.clone(), dead(2)]);
            let function = function(statements);
            let types = types();
            let (sites, _) = evaluate(&function, &types);
            assert!(sites.is_empty());
            assert!(
                !semantic_function_ssa_input_v1(&function, Some(&types), &[], &sites)
                    .0
                    .promotable()[1]
            );
        }
    }
}

#[test]
fn shared_primitive_mutable_fake_raw_fat_and_type_mismatch_never_promote() {
    for (holder, reference, kind, referent, pointee) in [
        (10, 5, SemanticBorrowKindV1::Mutable, 1, 0),
        (2, 1, SemanticBorrowKindV1::Fake, 1, 0),
        (11, 6, SemanticBorrowKindV1::Shared, 1, 0),
        (12, 7, SemanticBorrowKindV1::Shared, 1, 0),
        (8, 9, SemanticBorrowKindV1::Shared, 1, 0),
        (2, 1, SemanticBorrowKindV1::Shared, 9, 8),
    ] {
        let function = function(vec![
            borrow(holder, reference, referent, pointee, kind),
            assign_operand(place(5, 0), SemanticOperandV1::Copy(dereference(holder, 0))),
        ]);
        let (sites, summary) = evaluate(&function, &types());
        assert!(sites.is_empty());
        assert_eq!(summary.storage_words, 0);
    }
}

#[test]
fn shared_primitive_escaping_operations_and_writes_refuse() {
    let escapes = vec![
        assign(
            place(5, 0),
            SemanticRvalueKindV1::Cast {
                kind: SemanticCastKindV1::PointerExposeProvenance,
                operand: SemanticOperandV1::Copy(place(2, 1)),
            },
        ),
        statement(SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
            place(3, 1),
            SemanticOperandV1::Copy(place(2, 1)),
            SemanticVolatilityV1::NonVolatile,
            None,
        ))),
        assign(dereference(2, 0), SemanticRvalueKindV1::Use(constant(0))),
        assign(
            place(11, 6),
            SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Immutable,
                place: dereference(2, 0),
            },
        ),
        borrow(10, 5, 1, 0, SemanticBorrowKindV1::Mutable),
        statement(SemanticStatementKindV1::AtomicRmw(
            SemanticAtomicRmwV1::new(
                place(5, 0),
                dereference(2, 0),
                constant(0),
                SemanticAtomicRmwOpV1::Add,
                SemanticAtomicAccessV1::new(
                    SemanticAtomicOrderingV1::Relaxed,
                    SemanticAtomicScopeV1::Device,
                ),
            ),
        )),
    ];
    for escape in escapes {
        assert!(
            evaluate(
                &function(vec![shared(), read(2), escape, read(2)]),
                &types()
            )
            .0
            .is_empty()
        );
    }
    let call = SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(0),
            vec![SemanticOperandV1::Copy(place(2, 1))],
            None,
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    );
    assert!(
        evaluate(&function_with_end(vec![shared(), read(2)], call), &types())
            .0
            .is_empty()
    );
    let returning = function_with_locals(
        &[1, 0, 1, 1, 2, 0],
        vec![block(
            0,
            vec![
                shared(),
                read(2),
                assign_operand(place(0, 1), SemanticOperandV1::Move(place(2, 1))),
            ],
            SemanticTerminatorKindV1::Return,
        )],
        SemanticFunctionRoleV1::InternalHelper,
    );
    assert!(evaluate(&returning, &types()).0.is_empty());
}

#[test]
fn shared_primitive_dead_shared_holder_does_not_erase_later_mutable_storage() {
    let function = function(vec![
        assign_operand(place(1, 0), constant(0)),
        shared(),
        read(2),
        borrow(10, 5, 1, 0, SemanticBorrowKindV1::Mutable),
    ]);
    let types = types();
    let (sites, _) = evaluate(&function, &types);
    assert_eq!(sites.len(), 1);
    let input = semantic_function_ssa_input_v1(&function, Some(&types), &[], &sites).0;
    assert!(!input.promotable()[1]);
}

#[test]
fn shared_primitive_projection_metadata_and_aggregate_operand_types_are_exact() {
    for bad in [
        projected(
            4,
            &[
                (SemanticProjectionKindV1::Field(0), 1),
                (SemanticProjectionKindV1::Dereference, 0),
            ],
        ),
        projected(
            4,
            &[
                (SemanticProjectionKindV1::Field(1), 9),
                (SemanticProjectionKindV1::Dereference, 8),
            ],
        ),
        projected(
            4,
            &[
                (SemanticProjectionKindV1::Index(local(5)), 1),
                (SemanticProjectionKindV1::Dereference, 0),
            ],
        ),
    ] {
        assert!(
            evaluate(
                &function(vec![
                    shared(),
                    tuple_holder(),
                    assign_operand(place(5, 0), SemanticOperandV1::Copy(bad))
                ]),
                &types()
            )
            .0
            .is_empty()
        );
    }
    let wrong_scalar = assign(
        place(4, 2),
        SemanticRvalueKindV1::aggregate(
            SemanticAggregateKindV1::Tuple,
            vec![constant(8), SemanticOperandV1::Move(place(2, 1))],
        )
        .unwrap(),
    );
    assert!(
        evaluate(
            &function(vec![
                shared(),
                wrong_scalar,
                assign_operand(place(5, 0), SemanticOperandV1::Copy(field_read(4)))
            ]),
            &types()
        )
        .0
        .is_empty()
    );
}

#[test]
fn shared_primitive_occurrences_are_distinct_and_no_live_alias_crosses_a_block() {
    let statements = vec![shared(), read(2), dead(2), shared(), read(2), dead(2)];
    assert_closed(&function(statements.clone()), &types(), 2);
    let mut effects = effects();
    let mut summary = ProductionSemanticSsaSummaryV1::default();
    let first = effects
        .borrow_sites(
            &function(statements.clone()),
            &[],
            &types(),
            ProductionSemanticSsaLimitsV1::default(),
            &mut summary,
        )
        .unwrap();
    let second = effects
        .borrow_sites(
            &function(statements),
            &[],
            &types(),
            ProductionSemanticSsaLimitsV1::default(),
            &mut summary,
        )
        .unwrap();
    assert_eq!(first, second);
    let edge =
        SemanticControlFlowEdgeV1::new(SemanticEdgeRoleV1::Goto, SemanticBlockIdV1::from_index(1));
    let crossing = function_with_locals(
        &[0, 0, 1, 1, 2, 0],
        vec![
            block(
                0,
                vec![shared(), read(2)],
                SemanticTerminatorKindV1::Goto(edge),
            ),
            block(1, vec![dead(2)], SemanticTerminatorKindV1::Return),
        ],
        SemanticFunctionRoleV1::KernelRoot,
    );
    assert!(evaluate(&crossing, &types()).0.is_empty());
}

fn limits(work: usize, storage: usize) -> ProductionSemanticSsaLimitsV1 {
    let base = ProductionSemanticSsaModuleLimitsV1::production();
    let module = ProductionSemanticSsaModuleLimitsV1::try_new(
        base.max_variables(),
        base.max_blocks(),
        base.max_edges(),
        base.max_events(),
        base.max_edge_definitions(),
        base.max_output_items(),
        storage,
        work,
    )
    .unwrap();
    ProductionSemanticSsaLimitsV1::with_module_limits(SsaPlannerLimitsV1::default(), module)
}

#[test]
fn shared_primitive_exact_work_storage_and_caller_accounting() {
    let function = function(vec![
        shared(),
        tuple_holder(),
        assign_operand(place(5, 0), SemanticOperandV1::Copy(field_read(4))),
        dead(4),
    ]);
    let types = types();
    let initial = ProductionSemanticSsaSummaryV1 {
        work_units: 123,
        storage_words: 456,
        ..Default::default()
    };
    let mut total = initial;
    let expected = effects()
        .borrow_sites(
            &function,
            &[],
            &types,
            ProductionSemanticSsaLimitsV1::default(),
            &mut total,
        )
        .unwrap();
    assert_eq!(expected.len(), 1);
    let mut exact = initial;
    assert_eq!(
        effects()
            .borrow_sites(
                &function,
                &[],
                &types,
                limits(total.work_units, total.storage_words),
                &mut exact
            )
            .unwrap(),
        expected
    );
    assert_eq!(exact, total);
    for (resource, work, storage) in [
        (
            SsaPlannerResourceV1::WorkUnits,
            total.work_units - 1,
            total.storage_words,
        ),
        (
            SsaPlannerResourceV1::StorageWords,
            total.work_units,
            total.storage_words - 1,
        ),
    ] {
        let mut denied = initial;
        let error = effects()
            .borrow_sites(&function, &[], &types, limits(work, storage), &mut denied)
            .unwrap_err();
        assert!(
            matches!(error, Error::AggregateResourceLimit { resource: actual, .. } if actual == resource)
        );
        assert!(
            denied.work_units >= initial.work_units
                && denied.storage_words >= initial.storage_words
        );
    }
    let mut overflow = ProductionSemanticSsaSummaryV1 {
        work_units: usize::MAX,
        ..Default::default()
    };
    assert!(matches!(
        effects().borrow_sites(
            &function,
            &[],
            &types,
            ProductionSemanticSsaLimitsV1::default(),
            &mut overflow
        ),
        Err(Error::ResourceOverflow)
    ));
}

#[test]
fn shared_primitive_zero_candidate_path_has_no_alias_scratch() {
    for count in [0, 64, 256] {
        let statements = (0..count)
            .map(|_| assign_operand(place(1, 0), constant(0)))
            .collect();
        let (sites, summary) = evaluate(&function(statements), &types());
        assert!(sites.is_empty());
        assert_eq!(summary.storage_words, 0);
        assert_eq!(summary.work_units, 1 + 24 * count);
    }
}

#[test]
fn shared_primitive_long_scalar_prefix_and_alias_copy_growth_are_separately_bounded() {
    let measure = |prefix: usize, copies: usize| {
        let mut statements: Vec<_> = (0..prefix)
            .map(|_| assign_operand(place(5, 0), constant(0)))
            .collect();
        statements.push(shared());
        for _ in 0..copies {
            statements.extend([
                assign_operand(place(3, 1), SemanticOperandV1::Copy(place(2, 1))),
                read(3),
                dead(3),
            ]);
        }
        statements.extend([read(2), dead(2)]);
        let (sites, summary) = evaluate(&function(statements), &types());
        assert_eq!(sites.len(), 1);
        summary
    };
    let base = measure(0, 1);
    let prefix_small = measure(128, 1);
    let prefix_large = measure(256, 1);
    assert!(prefix_large.work_units > prefix_small.work_units);
    assert!(
        prefix_large.work_units - base.work_units < 3 * (prefix_small.work_units - base.work_units)
    );
    assert!(prefix_large.work_units - base.work_units < 2_000 * 256);
    let aliases_small = measure(0, 64);
    let aliases_large = measure(0, 128);
    assert!(aliases_large.work_units > aliases_small.work_units);
    assert!(aliases_large.work_units < 3 * aliases_small.work_units);
}

#[test]
fn shared_primitive_hostile_aggregate_alias_expansion_falls_back_without_authority() {
    for (depth, accepted) in [(2, true), (10, false)] {
        let mut types = types();
        let mut locals = vec![0, 0, 1];
        let mut statements = vec![shared()];
        let mut previous_type = 1;
        for level in 0..depth {
            let next_type = types.len() as u32;
            types.push(declaration(
                next_type as usize,
                SemanticTypeShapeV1::Tuple(
                    SemanticAggregateTypeV1::new(vec![ty(previous_type), ty(previous_type)])
                        .unwrap(),
                ),
            ));
            let previous_local = level + 2;
            let next_local = level + 3;
            locals.push(next_type);
            statements.push(assign(
                place(next_local, next_type),
                SemanticRvalueKindV1::aggregate(
                    SemanticAggregateKindV1::Tuple,
                    vec![
                        SemanticOperandV1::Copy(place(previous_local, previous_type)),
                        SemanticOperandV1::Move(place(previous_local, previous_type)),
                    ],
                )
                .unwrap(),
            ));
            previous_type = next_type;
        }
        let mut projections = Vec::new();
        for level in (0..depth).rev() {
            projections.push((
                SemanticProjectionKindV1::Field(0),
                if level == 0 { 1 } else { 9 + level },
            ));
        }
        projections.push((SemanticProjectionKindV1::Dereference, 0));
        statements.push(assign_operand(
            place(0, 0),
            SemanticOperandV1::Copy(projected(depth + 2, &projections)),
        ));
        let function = function_with_locals(
            &locals,
            vec![block(0, statements, SemanticTerminatorKindV1::Return)],
            SemanticFunctionRoleV1::KernelRoot,
        );
        let (sites, summary) = evaluate(&function, &types);
        assert_eq!(sites.len(), usize::from(accepted));
        assert!(summary.work_units < 1_000_000);
        assert!(summary.storage_words < 100_000);
        if !accepted {
            assert!(
                !semantic_function_ssa_input_v1(&function, Some(&types), &[], &sites)
                    .0
                    .promotable()[1]
            );
        }
    }
}

#[test]
fn shared_primitive_tuple_self_assignment_retains_rhs_alias_before_overwrite() {
    let field = projected(4, &[(SemanticProjectionKindV1::Field(1), 1)]);
    for value in [
        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(4, 2))),
        SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(4, 2))),
        SemanticRvalueKindV1::aggregate(
            SemanticAggregateKindV1::Tuple,
            vec![constant(0), SemanticOperandV1::Move(field)],
        )
        .unwrap(),
    ] {
        assert_closed(
            &function(vec![
                assign_operand(place(1, 0), constant(0)),
                shared(),
                tuple_holder(),
                assign(place(4, 2), value),
                assign_operand(place(5, 0), SemanticOperandV1::Copy(field_read(4))),
                dead(4),
                assign_operand(place(1, 0), constant(0)),
            ]),
            &types(),
            1,
        );
    }
}

#[test]
fn shared_primitive_reference_field_retirement_preserves_live_sibling_identity() {
    let field = |index| projected(6, &[(SemanticProjectionKindV1::Field(index), 1)]);
    let read_field = |index| {
        assign_operand(
            place(0, 0),
            SemanticOperandV1::Copy(projected(
                6,
                &[
                    (SemanticProjectionKindV1::Field(index), 1),
                    (SemanticProjectionKindV1::Dereference, 0),
                ],
            )),
        )
    };
    for overwrite in [false, true] {
        for changed_source in [1, 5] {
            let function = function(vec![
                assign_operand(place(1, 0), constant(0)),
                assign_operand(place(5, 0), constant(0)),
                shared(),
                borrow(3, 1, 5, 0, SemanticBorrowKindV1::Shared),
                assign(
                    place(6, 3),
                    SemanticRvalueKindV1::aggregate(
                        SemanticAggregateKindV1::Tuple,
                        vec![
                            SemanticOperandV1::Move(place(2, 1)),
                            SemanticOperandV1::Move(place(3, 1)),
                        ],
                    )
                    .unwrap(),
                ),
                read_field(0),
                if overwrite {
                    assign_operand(field(0), SemanticOperandV1::Copy(field(1)))
                } else {
                    statement(SemanticStatementKindV1::Deinitialize(field(0)))
                },
                read_field(1),
                assign_operand(place(changed_source, 0), constant(0)),
                dead(6),
            ]);
            let types = types();
            let (sites, _) = evaluate(&function, &types);
            let observed: BTreeSet<_> = sites.iter().map(|site| site.test_coordinates()).collect();
            let mut expected = BTreeSet::from([(0, 2)]);
            if changed_source == 1 {
                expected.insert((0, 3));
            }
            assert_eq!(observed, expected);
            let input = semantic_function_ssa_input_v1(&function, Some(&types), &[], &sites).0;
            assert!(input.promotable()[1]);
            assert_eq!(input.promotable()[5], changed_source == 1);
        }
    }
}

#[test]
fn shared_primitive_referent_write_after_rhs_read_retains_live_loan_storage() {
    for tuple in [false, true] {
        for overwrite_referent in [false, true] {
            let mut statements = vec![assign_operand(place(1, 0), constant(0)), shared()];
            if tuple {
                statements.push(tuple_holder());
            }
            statements.extend([
                assign_operand(
                    place(if overwrite_referent { 1 } else { 5 }, 0),
                    SemanticOperandV1::Copy(if tuple {
                        field_read(4)
                    } else {
                        dereference(2, 0)
                    }),
                ),
                dead(if tuple { 4 } else { 2 }),
                assign_operand(place(1, 0), constant(0)),
            ]);
            let function = function(statements);
            let types = types();
            let (sites, _) = evaluate(&function, &types);
            assert_eq!(sites.len(), usize::from(!overwrite_referent));
            let input = semantic_function_ssa_input_v1(&function, Some(&types), &[], &sites).0;
            assert_eq!(input.promotable()[1], !overwrite_referent);
        }
    }
}

fn compiler_holder_chain() -> Vec<SemanticStatementV1> {
    vec![
        assign_operand(place(1, 0), constant(0)),
        shared(),
        assign_operand(place(3, 1), SemanticOperandV1::Copy(place(2, 1))),
        assign(
            place(4, 2),
            SemanticRvalueKindV1::aggregate(
                SemanticAggregateKindV1::Tuple,
                vec![constant(0), SemanticOperandV1::Move(place(3, 1))],
            )
            .unwrap(),
        ),
        dead(3),
        assign_operand(
            place(9, 1),
            SemanticOperandV1::Copy(projected(4, &[(SemanticProjectionKindV1::Field(1), 1)])),
        ),
        read(9),
        dead(4),
        dead(2),
        dead(1),
    ]
}

#[test]
fn shared_primitive_unscoped_compiler_copy_from_tuple_dies_after_complete_read() {
    let local_types = vec![0, 0, 1, 1, 2, 0, 3, 4, 9, 1];
    let original = compiler_holder_chain();
    for final_effect in [
        dead(1),
        statement(SemanticStatementKindV1::StorageLive(local(1))),
        assign_operand(place(1, 0), constant(0)),
    ] {
        let mut statements = original.clone();
        *statements.last_mut().unwrap() = final_effect;
        assert_closed(
            &function_with_locals(
                &local_types,
                vec![block(0, statements, SemanticTerminatorKindV1::Return)],
                SemanticFunctionRoleV1::KernelRoot,
            ),
            &types(),
            1,
        );
    }
    let mut statements = original;
    statements.push(read(9));
    assert!(
        evaluate(
            &function_with_locals(
                &local_types,
                vec![block(0, statements, SemanticTerminatorKindV1::Return)],
                SemanticFunctionRoleV1::KernelRoot
            ),
            &types()
        )
        .0
        .is_empty()
    );
}

#[test]
fn shared_primitive_synthetic_holder_death_does_not_cross_edges_or_cycles() {
    let local_types = [0, 0, 1, 1, 2, 0, 3, 4, 9, 1];
    for cycle in [false, true] {
        let mut statements = compiler_holder_chain();
        statements.pop();
        let target = if cycle { 0 } else { 1 };
        let mut blocks = vec![block(
            0,
            statements,
            SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::Goto,
                SemanticBlockIdV1::from_index(target),
            )),
        )];
        if !cycle {
            blocks.push(block(1, vec![read(9)], SemanticTerminatorKindV1::Return));
        }
        assert!(
            evaluate(
                &function_with_locals(&local_types, blocks, SemanticFunctionRoleV1::KernelRoot),
                &types()
            )
            .0
            .is_empty()
        );
    }
    // A two-block repeated activation is also pinned; no ordinal-order shortcut.
    let blocks = vec![
        block(
            0,
            compiler_holder_chain(),
            SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::Goto,
                SemanticBlockIdV1::from_index(1),
            )),
        ),
        block(
            1,
            vec![],
            SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::Goto,
                SemanticBlockIdV1::from_index(0),
            )),
        ),
    ];
    assert!(
        evaluate(
            &function_with_locals(&local_types, blocks, SemanticFunctionRoleV1::KernelRoot),
            &types()
        )
        .0
        .is_empty()
    );
}

#[test]
fn shared_primitive_synthetic_holder_death_preserves_return_and_terminator_escape() {
    let local_types = [1, 0, 1, 1, 2, 0];
    let statements = vec![
        shared(),
        read(2),
        assign_operand(place(0, 1), SemanticOperandV1::Copy(place(2, 1))),
        dead(2),
        dead(1),
    ];
    assert!(
        evaluate(
            &function_with_locals(
                &local_types,
                vec![block(0, statements, SemanticTerminatorKindV1::Return)],
                SemanticFunctionRoleV1::InternalHelper
            ),
            &types()
        )
        .0
        .is_empty()
    );
    let crossing = function_with_end(
        vec![shared(), read(2)],
        SemanticTerminatorKindV1::Drop {
            place: place(2, 1),
            drop_glue: SemanticFunctionIdV1::from_index(0),
            target: SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::DropReturn,
                SemanticBlockIdV1::from_index(0),
            ),
            unwind: SemanticUnwindActionV1::Unreachable,
        },
    );
    assert!(evaluate(&crossing, &types()).0.is_empty());
}

#[test]
fn shared_primitive_holder_liveness_has_independent_storage_equation_and_exact_limits() {
    let function = function_with_locals(
        &[0, 0, 1, 1, 2, 0, 3, 4, 9, 1],
        vec![block(
            0,
            compiler_holder_chain(),
            SemanticTerminatorKindV1::Return,
        )],
        SemanticFunctionRoleV1::KernelRoot,
    );
    let types = types();
    let mut sizing = ProductionSemanticSsaSummaryV1::default();
    let (units, candidates) = scan_size(
        &function,
        0,
        &mut Meter {
            limits: ProductionSemanticSsaLimitsV1::default(),
            summary: &mut sizing,
        },
    )
    .unwrap();
    let (expected, observed) = evaluate(&function, &types);
    assert_eq!(expected.len(), 1);
    assert_eq!(
        observed.storage_words,
        128 + units * 96
            + candidates * 64
            + 64
            + function.locals().len() * 3
            + function.blocks().len() * 2
    );
    for short in [false, true] {
        for work_cut in [false, true] {
            let mut actual = ProductionSemanticSsaSummaryV1::default();
            let result = effects().borrow_sites(
                &function,
                &[],
                &types,
                limits(
                    observed.work_units - usize::from(short && work_cut),
                    observed.storage_words - usize::from(short && !work_cut),
                ),
                &mut actual,
            );
            if short {
                let selected = if work_cut {
                    SsaPlannerResourceV1::WorkUnits
                } else {
                    SsaPlannerResourceV1::StorageWords
                };
                assert!(
                    matches!(result, Err(Error::AggregateResourceLimit { resource, .. }) if resource == selected)
                );
            } else {
                assert_eq!(result.unwrap(), expected);
                assert_eq!(actual, observed);
            }
        }
    }
}
