use super::*;

#[path = "evidence_v1/tests.rs"]
mod evidence_v1_tests;

const U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const PTR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);

fn identity(tag: u32) -> [u8; 32] {
    let mut bytes = [0; 32];
    bytes[..4].copy_from_slice(&tag.to_be_bytes());
    bytes
}
fn provenance() -> SemanticSourceProvenanceV1 {
    SemanticSourceProvenanceV1::unavailable()
}
fn types() -> Vec<SemanticTypeDeclV1> {
    let scalar = |size, alignment, primitive, maximum| {
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(size),
            alignment,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                primitive,
                SemanticScalarValidityRangeV1::new(0, maximum),
            )),
            false,
        )
        .unwrap()
    };
    vec![
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(identity(1)),
            SemanticLayoutIdentityV1::from_sha256(identity(1)),
            scalar(
                4,
                4,
                SemanticBackendPrimitiveV1::integer(false, 32, 4),
                u32::MAX.into(),
            ),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            }),
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(identity(2)),
            SemanticLayoutIdentityV1::from_sha256(identity(2)),
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                0,
                1,
                SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
                SemanticRustcVariantsV1::Single { index: 0 },
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                1,
                0,
                SemanticTypeLayoutDetailsV1::None,
            )
            .unwrap(),
            SemanticTypeShapeV1::Unit,
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(identity(3)),
            SemanticLayoutIdentityV1::from_sha256(identity(3)),
            scalar(
                8,
                8,
                SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                u64::MAX.into(),
            ),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new(
                    U32,
                    SemanticMutabilityV1::Mutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        )
        .with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                Some(SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap()),
                None,
            ),
        ),
    ]
}
fn direct(ty: SemanticTypeIdV1) -> SemanticAbiValueV1 {
    SemanticAbiValueV1::new(
        ty,
        SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                SemanticAbiExtensionV1::None,
                0,
                None,
            )
            .unwrap(),
        ),
    )
}
fn abi(tag: u32, c_abi: bool) -> SemanticFunctionAbiV1 {
    SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256(identity(tag)),
        SemanticLayoutIdentityV1::from_sha256(identity(tag)),
        if c_abi {
            SemanticCanonAbiV1::C
        } else {
            SemanticCanonAbiV1::Rust
        },
        false,
        false,
        vec![direct(PTR), direct(U32)],
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
}
fn local(index: u32) -> SemanticLocalIdV1 {
    SemanticLocalIdV1::from_index(index)
}
fn function_id(index: u32) -> SemanticFunctionIdV1 {
    SemanticFunctionIdV1::from_index(index)
}
fn place(index: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(local(index), vec![], ty).unwrap()
}
fn memory() -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        local(1),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, U32).unwrap()],
        U32,
    )
    .unwrap()
}
fn edge(role: SemanticEdgeRoleV1, target: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
}
fn block(
    index: u32,
    statements: Vec<SemanticStatementV1>,
    kind: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256(identity(index + 1)),
        provenance(),
        statements,
        SemanticTerminatorV1::new(provenance(), kind),
    )
    .unwrap()
}
fn call(
    callee: u32,
    target: u32,
    moved: bool,
    unwind: SemanticUnwindActionV1,
) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new(
            function_id(callee),
            vec![
                SemanticOperandV1::Copy(place(1, PTR)),
                if moved {
                    SemanticOperandV1::Move(place(2, U32))
                } else {
                    SemanticOperandV1::Copy(place(2, U32))
                },
            ],
            Some(SemanticCallDestinationV1::new(
                place(3, UNIT),
                edge(SemanticEdgeRoleV1::CallReturn, target),
            )),
            unwind,
        )
        .unwrap(),
    )
}
fn function(
    index: u32,
    root: bool,
    c_abi: bool,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    let tag = index + 1;
    let locals = [
        (UNIT, SemanticLocalRoleV1::Return),
        (PTR, SemanticLocalRoleV1::Argument(0)),
        (U32, SemanticLocalRoleV1::Argument(1)),
        (UNIT, SemanticLocalRoleV1::Temporary),
        (U32, SemanticLocalRoleV1::Temporary),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (ty, role))| {
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(identity(index as u32 + 1)),
            ty,
            role,
            provenance(),
        )
    })
    .collect();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(identity(tag)),
        if root {
            SemanticFunctionRoleV1::KernelRoot
        } else {
            SemanticFunctionRoleV1::InternalHelper
        },
        SemanticItemDefinitionIdentityV1::from_sha256(identity(tag)),
        SemanticMonomorphizationIdentityV1::from_sha256(identity(tag)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(identity(tag)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(identity(tag)),
        provenance(),
        abi(tag, c_abi),
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
}
fn leaf(index: u32) -> SemanticFunctionDeclV1 {
    function(
        index,
        false,
        false,
        vec![
            block(
                0,
                vec![SemanticStatementV1::new(
                    provenance(),
                    SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                        memory(),
                        SemanticOperandV1::Copy(place(2, U32)),
                        SemanticVolatilityV1::NonVolatile,
                        None,
                    )),
                )],
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1)),
            ),
            block(
                1,
                vec![],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(place(2, U32)),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            edge(SemanticEdgeRoleV1::SwitchValue, 2),
                        )],
                        edge(SemanticEdgeRoleV1::SwitchOtherwise, 3),
                    )
                    .unwrap(),
                },
            ),
            block(2, vec![], SemanticTerminatorKindV1::Return),
            block(
                3,
                vec![],
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1)),
            ),
        ],
    )
}
fn admit(
    functions: Vec<SemanticFunctionDeclV1>,
    roots: Vec<SemanticFunctionIdV1>,
) -> AdmittedInertSemanticMirV1 {
    InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(identity(250))),
        types(),
        vec![],
        vec![],
        vec![],
        functions,
        roots,
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap()
}
fn repeated() -> AdmittedInertSemanticMirV1 {
    admit(
        vec![
            function(
                0,
                true,
                false,
                vec![
                    block(
                        0,
                        vec![],
                        call(1, 1, false, SemanticUnwindActionV1::Unreachable),
                    ),
                    block(
                        1,
                        vec![],
                        call(1, 2, true, SemanticUnwindActionV1::Unreachable),
                    ),
                    block(2, vec![], SemanticTerminatorKindV1::Return),
                ],
            ),
            leaf(1),
        ],
        vec![function_id(0)],
    )
}

#[test]
fn admitted_effectful_repeated_calls_expand_with_exact_origins() {
    let source = repeated();
    let expansion =
        SemanticCallExpansionV1::try_new(&source, SemanticCallExpansionLimitsV1::default())
            .unwrap();
    expansion.verify_replay(&source).unwrap();
    let root = expansion.root(function_id(0)).unwrap();
    assert!(root.has_expanded_calls());
    assert_eq!(root.source_body(), function_id(0));
    assert_eq!(root.instances().len(), 3);
    assert_eq!(root.body().locals().len(), 15);
    assert_eq!(root.body().blocks().len(), 11);
    assert_ne!(root.body().identity(), source.functions()[0].identity());
    assert_eq!(
        root.instances()[1].function_identity(),
        root.instances()[2].function_identity()
    );
    assert_ne!(
        root.instances()[1].block_start(),
        root.instances()[2].block_start()
    );
    assert_ne!(
        root.instances()[1].local_start(),
        root.instances()[2].local_start()
    );
    assert_eq!(root.local_origins().len(), root.body().locals().len());
    assert_eq!(root.block_origins().len(), root.body().blocks().len());
    let mut stores = 0;
    for (index, block) in root.body().blocks().iter().enumerate() {
        let origin = &root.block_origins()[index];
        assert_eq!(block.statements().len(), origin.statements().len());
        for statement in block.statements() {
            if let SemanticStatementKindV1::Store(store) = statement.kind() {
                stores += 1;
                assert_eq!(
                    store.destination().local().index(),
                    root.instances()[origin.instance().index() as usize].local_start() + 1
                );
            }
        }
        assert!(!matches!(
            block.terminator().kind(),
            SemanticTerminatorKindV1::Call(_)
        ));
        block
            .terminator()
            .kind()
            .try_for_each_edge::<()>(|edge| {
                assert!((edge.target().index() as usize) < root.body().blocks().len());
                Ok(())
            })
            .unwrap();
    }
    assert_eq!(stores, 2);
    for (block, origins) in root.body().blocks().iter().zip(root.block_origins()) {
        for (statement, origin) in block.statements().iter().zip(origins.statements()) {
            if matches!(
                origin,
                SemanticExpandedStatementOriginV1::ReturnTransfer { .. }
            ) {
                let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                    panic!()
                };
                assert!(
                    matches!(assignment.value().kind(), SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(value)) if value.ty() == UNIT && *value.value() == SemanticConstantValueV1::ZeroSized)
                );
            }
        }
    }
    for (call_block, expected_move) in [(0, false), (1, true)] {
        let root_block = &root.body().blocks()[call_block];
        let origins = root.block_origins()[call_block].statements();
        let index = origins
            .iter()
            .position(|origin| {
                matches!(
                    origin,
                    SemanticExpandedStatementOriginV1::ParameterTransfer { argument: 1, .. }
                )
            })
            .unwrap();
        let SemanticStatementKindV1::Assign(assignment) = root_block.statements()[index].kind()
        else {
            panic!()
        };
        assert_eq!(
            matches!(
                assignment.value().kind(),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Move(_))
            ),
            expected_move
        );
    }
    assert!(!expansion.grants_proof_or_artifact_authority());
}

#[test]
fn repeated_call_frames_open_arguments_with_explicit_lifetime_ends() {
    let original = repeated();
    let source = admit(
        vec![
            original.functions()[0].clone(),
            function(
                1,
                false,
                false,
                vec![block(
                    0,
                    [
                        SemanticStatementKindV1::StorageLive(local(4)),
                        SemanticStatementKindV1::StorageDead(local(2)),
                        SemanticStatementKindV1::StorageDead(local(0)),
                    ]
                    .into_iter()
                    .map(|kind| SemanticStatementV1::new(provenance(), kind))
                    .collect(),
                    SemanticTerminatorKindV1::Return,
                )],
            ),
        ],
        vec![function_id(0)],
    );
    let expansion =
        SemanticCallExpansionV1::try_new(&source, SemanticCallExpansionLimitsV1::default())
            .unwrap();
    let view = expansion.root(function_id(0)).unwrap();
    for call_block in 0..2 {
        let origins = view.block_origins()[call_block].statements();
        let entry_live = origins
            .iter()
            .filter_map(|origin| match origin {
                SemanticExpandedStatementOriginV1::FrameStorageLive { local, .. } => {
                    Some(local.index())
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(entry_live, [0, 1, 2, 3]);
        let argument_transfer = origins
            .iter()
            .position(|origin| {
                matches!(
                    origin,
                    SemanticExpandedStatementOriginV1::ParameterTransfer { argument: 1, .. }
                )
            })
            .unwrap();
        let argument_live = origins
            .iter()
            .position(|origin| {
                matches!(origin,
                SemanticExpandedStatementOriginV1::FrameStorageLive { local: argument, .. }
                if *argument == local(2))
            })
            .unwrap();
        assert!(argument_live < argument_transfer);
        let SemanticStatementKindV1::StorageLive(argument) =
            view.body().blocks()[call_block].statements()[argument_live].kind()
        else {
            panic!("argument lifetime origin must describe StorageLive");
        };
        assert_eq!(
            argument.index(),
            view.instances()[call_block + 1].local_start() + 2
        );
    }
    expansion.verify_replay(&source).unwrap();
}

#[test]
fn call_free_root_without_export_is_exact_passthrough() {
    let source = admit(
        vec![function(
            0,
            true,
            false,
            vec![block(0, vec![], SemanticTerminatorKindV1::Return)],
        )],
        vec![function_id(0)],
    );
    let expansion =
        SemanticCallExpansionV1::try_new(&source, SemanticCallExpansionLimitsV1::default())
            .unwrap();
    let root = &expansion.roots()[0];
    assert!(!root.has_expanded_calls());
    assert_eq!(root.body(), &source.functions()[0]);
    assert!(root.body().kernel_entry().is_none());
    expansion.verify_replay(&source).unwrap();
}

#[test]
fn nested_calls_are_iterative_and_recursion_is_rejected() {
    let chain = |cycle| {
        admit(
            vec![
                function(
                    0,
                    true,
                    false,
                    vec![
                        block(
                            0,
                            vec![],
                            call(1, 1, false, SemanticUnwindActionV1::Unreachable),
                        ),
                        block(1, vec![], SemanticTerminatorKindV1::Return),
                    ],
                ),
                function(
                    1,
                    false,
                    false,
                    vec![
                        block(
                            0,
                            vec![],
                            call(2, 1, false, SemanticUnwindActionV1::Unreachable),
                        ),
                        block(1, vec![], SemanticTerminatorKindV1::Return),
                    ],
                ),
                if cycle {
                    function(
                        2,
                        false,
                        false,
                        vec![
                            block(
                                0,
                                vec![],
                                call(1, 1, false, SemanticUnwindActionV1::Unreachable),
                            ),
                            block(1, vec![], SemanticTerminatorKindV1::Return),
                        ],
                    )
                } else {
                    leaf(2)
                },
            ],
            vec![function_id(0)],
        )
    };
    let source = chain(false);
    let expansion =
        SemanticCallExpansionV1::try_new(&source, SemanticCallExpansionLimitsV1::default())
            .unwrap();
    assert_eq!(expansion.roots()[0].instances()[2].depth(), 2);
    assert_eq!(
        expansion.roots()[0].instances()[2].parent(),
        Some(SemanticCallInstanceIdV1(1))
    );
    assert!(matches!(
        SemanticCallExpansionV1::try_new(&chain(true), SemanticCallExpansionLimitsV1::default()),
        Err(SemanticCallExpansionErrorV1::Unsupported {
            reason: "recursive defined call",
            ..
        })
    ));
}

#[test]
fn replay_rejects_source_body_and_origin_mutations() {
    let source = repeated();
    let make = || {
        SemanticCallExpansionV1::try_new(&source, SemanticCallExpansionLimitsV1::default()).unwrap()
    };
    let mut expansion = make();
    expansion.roots[0].local_origins[5].local = local(2);
    assert!(matches!(
        expansion.verify_replay(&source),
        Err(SemanticCallExpansionErrorV1::ReplayMismatch)
    ));
    let mut expansion = make();
    expansion.roots[0].block_origins[0].terminator = SemanticExpandedTerminatorOriginV1::Source;
    assert!(matches!(
        expansion.verify_replay(&source),
        Err(SemanticCallExpansionErrorV1::ReplayMismatch)
    ));
    let mut expansion = make();
    expansion.roots[0].body = source.functions()[0].clone();
    assert!(matches!(
        expansion.verify_replay(&source),
        Err(SemanticCallExpansionErrorV1::ReplayMismatch)
    ));
    let other = admit(
        vec![function(
            0,
            true,
            false,
            vec![block(0, vec![], SemanticTerminatorKindV1::Return)],
        )],
        vec![function_id(0)],
    );
    assert!(matches!(
        make().verify_replay(&other),
        Err(SemanticCallExpansionErrorV1::SourceMismatch)
    ));
}

#[test]
fn content_identity_binds_body_bytes_and_origins_not_only_node_ids() {
    let source = repeated();
    let make = || {
        SemanticCallExpansionV1::try_new(&source, SemanticCallExpansionLimitsV1::default()).unwrap()
    };
    let mut expansion = make();
    let root = &mut expansion.roots[0];
    let expected = *root.identity();
    let body = &root.body;
    let mut blocks = body.blocks().to_vec();
    let original_block = &blocks[3];
    let changed = SemanticStatementV1::new(
        provenance(),
        SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
            SemanticPlaceV1::new(
                local(6),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, U32).unwrap(),
                ],
                U32,
            )
            .unwrap(),
            SemanticOperandV1::Constant(SemanticConstantV1::new(
                U32,
                SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(77, 4).unwrap()),
            )),
            SemanticVolatilityV1::NonVolatile,
            None,
        )),
    );
    blocks[3] = SemanticBasicBlockV1::new(
        original_block.identity(),
        original_block.source(),
        vec![changed],
        original_block.terminator().clone(),
    )
    .unwrap();
    root.body = SemanticFunctionDeclV1::new(
        body.identity(),
        body.role(),
        body.item_definition_identity(),
        body.monomorphization_identity(),
        body.generic_type_arguments_identity(),
        body.const_generic_arguments_identity(),
        body.source(),
        body.abi().clone(),
        body.locals().to_vec(),
        body.entry(),
        blocks,
    )
    .unwrap();
    let actual = super::identity::root_content_identity(
        &source,
        root,
        &mut Budget::new(SemanticCallExpansionLimitsV1::default()).unwrap(),
    )
    .unwrap();
    assert_ne!(expected, actual);
    assert!(matches!(
        expansion.verify_replay(&source),
        Err(SemanticCallExpansionErrorV1::ReplayMismatch)
    ));
    let mut expansion = make();
    let root = &mut expansion.roots[0];
    root.block_origins[0].statements = Box::new([]);
    assert_ne!(
        *root.identity(),
        super::identity::root_content_identity(
            &source,
            root,
            &mut Budget::new(SemanticCallExpansionLimitsV1::default()).unwrap()
        )
        .unwrap()
    );
    assert!(matches!(
        expansion.verify_replay(&source),
        Err(SemanticCallExpansionErrorV1::ReplayMismatch)
    ));
}

#[test]
fn limits_are_cumulative_across_instances_and_roots() {
    let source = repeated();
    let defaults = SemanticCallExpansionLimitsV1::default();
    for limits in [
        SemanticCallExpansionLimitsV1 {
            instances: 2,
            ..defaults
        },
        SemanticCallExpansionLimitsV1 {
            locals: 14,
            ..defaults
        },
        SemanticCallExpansionLimitsV1 {
            blocks: 10,
            ..defaults
        },
        SemanticCallExpansionLimitsV1 {
            statements: 1,
            ..defaults
        },
        SemanticCallExpansionLimitsV1 {
            work: 1,
            ..defaults
        },
        SemanticCallExpansionLimitsV1 {
            depth: 0,
            ..defaults
        },
    ] {
        assert!(matches!(
            SemanticCallExpansionV1::try_new(&source, limits),
            Err(SemanticCallExpansionErrorV1::Limit(_))
        ));
    }
    assert!(matches!(
        SemanticCallExpansionV1::try_new(
            &source,
            SemanticCallExpansionLimitsV1 {
                depth: SemanticCallExpansionLimitsV1::HARD_MAX.depth + 1,
                ..defaults
            }
        ),
        Err(SemanticCallExpansionErrorV1::InvalidLimits)
    ));
    let source = admit(
        vec![
            function(
                0,
                true,
                false,
                vec![block(0, vec![], SemanticTerminatorKindV1::Return)],
            ),
            function(
                1,
                true,
                false,
                vec![block(0, vec![], SemanticTerminatorKindV1::Return)],
            ),
        ],
        vec![function_id(0), function_id(1)],
    );
    assert!(matches!(
        SemanticCallExpansionV1::try_new(
            &source,
            SemanticCallExpansionLimitsV1 {
                locals: 5,
                ..defaults
            }
        ),
        Err(SemanticCallExpansionErrorV1::Limit(
            SemanticCallExpansionResourceV1::Locals
        ))
    ));
}

#[test]
fn unsupported_call_abi_and_unwind_fail_closed() {
    for (c_abi, unwind) in [
        (true, SemanticUnwindActionV1::Unreachable),
        (false, SemanticUnwindActionV1::Continue),
    ] {
        let helper = function(
            1,
            false,
            c_abi,
            vec![block(0, vec![], SemanticTerminatorKindV1::Return)],
        );
        let source = admit(
            vec![
                function(
                    0,
                    true,
                    false,
                    vec![
                        block(0, vec![], call(1, 1, false, unwind)),
                        block(1, vec![], SemanticTerminatorKindV1::Return),
                    ],
                ),
                helper,
            ],
            vec![function_id(0)],
        );
        assert!(matches!(
            SemanticCallExpansionV1::try_new(&source, SemanticCallExpansionLimitsV1::default()),
            Err(SemanticCallExpansionErrorV1::Unsupported { .. })
        ));
    }
}

#[test]
fn remap_preserves_borrow_index_places_and_atomic_contracts() {
    let instance = SemanticCallInstanceV1 {
        function: function_id(1),
        function_identity: SemanticFunctionIdentityV1::from_sha256(identity(2)),
        parent: Some(SemanticCallInstanceIdV1(0)),
        call_block: Some(SemanticBlockIdV1::from_index(0)),
        local_start: 10,
        local_count: 8,
        block_start: 20,
        block_count: 4,
        depth: 1,
    };
    let indexed = SemanticPlaceV1::new(
        local(1),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Index(local(2)), U32).unwrap()],
        U32,
    )
    .unwrap();
    let borrow = SemanticStatementV1::new(
        provenance(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(3, PTR),
            SemanticRvalueV1::new(
                PTR,
                SemanticRvalueKindV1::AddressOf {
                    mutability: SemanticMutabilityV1::Mutable,
                    place: indexed,
                },
            ),
        )),
    );
    let mut budget = Budget::new(SemanticCallExpansionLimitsV1::default()).unwrap();
    let mapped = remap::statement(&borrow, &instance, &mut budget).unwrap();
    let SemanticStatementKindV1::Assign(assignment) = mapped.kind() else {
        panic!()
    };
    let SemanticRvalueKindV1::AddressOf { place, mutability } = assignment.value().kind() else {
        panic!()
    };
    assert_eq!(*mutability, SemanticMutabilityV1::Mutable);
    assert_eq!(place.local(), local(11));
    assert_eq!(
        place.projections()[0].kind(),
        SemanticProjectionKindV1::Index(local(12))
    );
    let access = SemanticAtomicAccessV1::new(
        SemanticAtomicOrderingV1::Relaxed,
        SemanticAtomicScopeV1::Device,
    );
    let atomic = SemanticStatementV1::new(
        provenance(),
        SemanticStatementKindV1::AtomicRmw(SemanticAtomicRmwV1::new(
            super::tests::place(4, U32),
            memory(),
            SemanticOperandV1::Move(super::tests::place(2, U32)),
            SemanticAtomicRmwOpV1::Add,
            access,
        )),
    );
    let mapped = remap::statement(&atomic, &instance, &mut budget).unwrap();
    let SemanticStatementKindV1::AtomicRmw(atomic) = mapped.kind() else {
        panic!()
    };
    assert_eq!(atomic.access(), access);
    assert!(matches!(atomic.value(), SemanticOperandV1::Move(value) if value.local() == local(12)));
}
