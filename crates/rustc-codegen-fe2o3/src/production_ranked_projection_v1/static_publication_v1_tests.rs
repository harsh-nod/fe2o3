// Private custody fixtures are not source admission or HB certificates.
include!("static_publication_metadata_v1_tests.rs");
fn static_publication_fixture_v1() -> (
    Vec<SemanticTypeDeclV1>,
    SemanticFunctionDeclV1,
    Vec<SemanticCallableDeclV1>,
    Vec<Option<AllocationContractV1>>,
) {
    let ty = SemanticTypeIdV1::from_index;
    let (mut types, _) = indexed_atomic_fixture_v1(true, false);
    let shape = |id, size, alignment, shape| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(id)),
            SemanticLayoutIdentityV1::from_sha256(bytes(id)),
            SemanticTypeLayoutV1::new(Some(size), alignment).unwrap(),
            shape,
        )
    };
    types.push(shape(
        230,
        4,
        4,
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 32 }),
    ));
    types.push(
        shape(
            231,
            16,
            8,
            SemanticTypeShapeV1::Aggregate(
                SemanticAggregateTypeV1::new(vec![ty(12), ty(8)]).unwrap(),
            ),
        )
        .with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                Some(SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap()),
                None,
            ),
        ),
    );
    types.push(shape(
        232,
        8,
        8,
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                ty(10),
                SemanticPointerKindV1::Raw,
                SemanticMutabilityV1::Mutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    ));
    types.push(shape(
        233,
        8,
        4,
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![ty(0), ty(10)]).unwrap()),
    ));
    types.push(shape(
        234,
        8,
        8,
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                ty(11),
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    ));
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
        ],
        SemanticAbiValueV1::new(ty(0), SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![
        SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
        SemanticSourceArgumentOwnershipV1::SharedBorrow,
        SemanticSourceArgumentOwnershipV1::ByValue,
    ])
    .unwrap();
    let locals = [0, 11, 4, 10, 8, 13, 8, 11, 4, 14, 8]
        .into_iter()
        .enumerate()
        .map(|(i, t)| {
            local(
                240 + i as u8,
                ty(t),
                match i {
                    0 => SemanticLocalRoleV1::Return,
                    1..=3 => SemanticLocalRoleV1::Argument(i as u32 - 1),
                    _ => SemanticLocalRoleV1::Temporary,
                },
            )
        })
        .collect();
    let args = || {
        vec![
            typed_operand(1, ty(11)),
            typed_operand(2, ty(4)),
            typed_operand(4, ty(8)),
        ]
    };
    let mut producer = args();
    producer.push(typed_operand(3, ty(10)));
    let blocks = vec![
        block(
            230,
            vec![typed_assignment(
                4,
                ty(8),
                SemanticRvalueKindV1::Use(typed_constant(ty(8), 17, 8)),
            )],
            zero_switch(4, ty(8), 1, 2),
        ),
        block(
            231,
            vec![],
            consumed_read_only_call_v1(0, producer, (5, 13), 3),
        ),
        block(
            232,
            vec![],
            consumed_read_only_call_v1(1, args(), (5, 13), 3),
        ),
        block(233, vec![], SemanticTerminatorKindV1::Return),
    ];
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(236)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(237)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(238)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(239)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(240)),
        SemanticSourceProvenanceV1::unavailable(),
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap();
    let callables = vec![
        compiler_intrinsic_callable(
            SemanticCompilerIntrinsicOperationV1::StaticPublication128PublishF32 {
                payload: ty(11),
                flags: ty(4),
                result: ty(13),
            },
        ),
        compiler_intrinsic_callable(
            SemanticCompilerIntrinsicOperationV1::StaticPublication128TryReadF32 {
                payload: ty(11),
                flags: ty(4),
                result: ty(13),
            },
        ),
    ];
    let mut allocations = vec![None; 11];
    allocations[1] = Some(AllocationContractV1 {
        allocation_origin: 1,
        noalias_class: 2,
        writable: true,
        singleton_object: false,
    });
    allocations[2] = Some(AllocationContractV1 {
        allocation_origin: 2,
        noalias_class: 1,
        writable: true,
        singleton_object: false,
    });
    (types, function, callables, allocations)
}

fn static_publication_reblock_v1(
    function: &SemanticFunctionDeclV1,
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
        function.locals().to_vec(),
        function.entry(),
        blocks,
    )
    .unwrap()
}

#[test]
fn static_publication_custody_retains_exact_original_roots_and_sites() {
    let (types, function, callables, allocations) = static_publication_fixture_v1();
    let source = audit_static_publication_source_v1(&types, &callables, &function, &allocations)
        .unwrap()
        .unwrap();
    assert_eq!(source.payload.argument, 0);
    assert_eq!(source.flags.argument, 1);
    assert_eq!(source.payload.allocation.noalias_class, 2);
    assert_eq!(source.flags.allocation.noalias_class, 1);
    assert_eq!((source.producer.block, source.consumer.block), (1, 2));
    assert_eq!(source.producer.cell, SemanticLocalIdV1::from_index(4));
}

#[test]
fn static_publication_custody_rejects_repeated_sequential_or_missing_sites() {
    let (types, function, callables, allocations) = static_publication_fixture_v1();
    for target in [0, 1, 2] {
        let mut blocks = function.blocks().to_vec();
        blocks[3] = block(
            233,
            vec![],
            SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, target)),
        );
        assert!(
            audit_static_publication_source_v1(
                &types,
                &callables,
                &static_publication_reblock_v1(&function, blocks),
                &allocations
            )
            .is_err()
        );
    }
    let mut blocks = function.blocks().to_vec();
    blocks[2] = block(232, vec![], SemanticTerminatorKindV1::Return);
    assert!(
        audit_static_publication_source_v1(
            &types,
            &callables,
            &static_publication_reblock_v1(&function, blocks),
            &allocations
        )
        .is_err()
    );
}

#[test]
fn static_publication_custody_rejects_owner_copy_root_mutation_and_fake_atomic() {
    let ty = SemanticTypeIdV1::from_index;
    let (types, function, callables, allocations) = static_publication_fixture_v1();
    let mut blocks = function.blocks().to_vec();
    let mut statements = blocks[0].statements().to_vec();
    statements.push(typed_assignment(
        7,
        ty(11),
        SemanticRvalueKindV1::Use(typed_operand(1, ty(11))),
    ));
    blocks[0] = block(230, statements, blocks[0].terminator().kind().clone());
    assert!(
        audit_static_publication_source_v1(
            &types,
            &callables,
            &static_publication_reblock_v1(&function, blocks),
            &allocations
        )
        .is_err()
    );
    let mut wrong = allocations.clone();
    wrong[1].as_mut().unwrap().noalias_class = 1;
    assert!(audit_static_publication_source_v1(&types, &callables, &function, &wrong).is_err());
    let (mut fake, _) = indexed_atomic_fixture_v1(false, false);
    fake.extend(types[10..].iter().cloned());
    assert!(
        audit_static_publication_source_v1(&fake, &callables, &function, &allocations).is_err()
    );
}

#[test]
fn static_publication_custody_rejects_other_access_and_post_consumption_reuse() {
    let ty = SemanticTypeIdV1::from_index;
    let (types, function, callables, allocations) = static_publication_fixture_v1();
    for block_index in [0, 3] {
        let mut blocks = function.blocks().to_vec();
        let mut statements = blocks[block_index].statements().to_vec();
        statements.push(typed_assignment(
            8,
            ty(4),
            SemanticRvalueKindV1::Use(typed_operand(2, ty(4))),
        ));
        if block_index == 0 {
            statements.push(statement(SemanticStatementKindV1::Assume(typed_operand(
                8,
                ty(4),
            ))));
        }
        blocks[block_index] = block(
            230 + block_index as u8,
            statements,
            blocks[block_index].terminator().kind().clone(),
        );
        assert!(
            audit_static_publication_source_v1(
                &types,
                &callables,
                &static_publication_reblock_v1(&function, blocks),
                &allocations
            )
            .is_err()
        );
    }
}

#[test]
fn static_publication_custody_budget_is_cumulative_and_exact() {
    let (types, function, callables, allocations) = static_publication_fixture_v1();
    let source = audit_static_publication_source_v1(&types, &callables, &function, &allocations)
        .unwrap()
        .unwrap();
    let mut required = 0;
    static_publication_audit_uses_v1(
        &types,
        &callables,
        &function,
        &allocations,
        &source,
        &mut required,
    )
    .unwrap();
    assert!(required > 0);
    let mut exact = MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1 - required;
    static_publication_audit_uses_v1(
        &types,
        &callables,
        &function,
        &allocations,
        &source,
        &mut exact,
    )
    .unwrap();
    assert_eq!(exact, MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1);
    let mut insufficient = MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1 - required + 1;
    assert!(
        static_publication_audit_uses_v1(
            &types,
            &callables,
            &function,
            &allocations,
            &source,
            &mut insufficient
        )
        .is_err()
    );
}
