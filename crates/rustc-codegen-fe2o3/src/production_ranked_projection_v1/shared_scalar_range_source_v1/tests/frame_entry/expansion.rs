use super::*;
use fe2o3_mir_model::{
    SemanticCallExpansionLimitsV1, SemanticCallExpansionV1, SemanticExpandedStatementOriginV1,
};

const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(9);

fn function(root: bool, fixture: &Fixture) -> SemanticFunctionDeclV1 {
    let template = fixture.function();
    let word = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let reference = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(
            true,
            Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
            true,
            true,
            false,
            true,
        ),
        SemanticAbiExtensionV1::None,
        8,
        Some(8),
    )
    .unwrap();
    let direct = |ty| {
        SemanticAbiValueV1::new(
            ty,
            if ty == CAPTURE {
                SemanticAbiPassModeV1::Pair {
                    first: reference,
                    second: word,
                }
            } else {
                SemanticAbiPassModeV1::Direct(word)
            },
        )
    };
    let tag = if root { 21 } else { 22 };
    let inputs = if root {
        vec![direct(WORD)]
    } else {
        vec![direct(CAPTURE), direct(WORD)]
    };
    let abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([tag; 32]),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        inputs,
        if root {
            SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore)
        } else {
            direct(WORD)
        },
    )
    .unwrap();
    let blocks = if root {
        vec![
            block(
                0,
                fixture.blocks[0].statements()[..5].to_vec(),
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new(
                        SemanticFunctionIdV1::from_index(1),
                        vec![moved(6, CAPTURE), copy(3, WORD)],
                        Some(SemanticCallDestinationV1::new(
                            place(23, WORD),
                            edge(SemanticEdgeRoleV1::CallReturn, 1),
                        )),
                        SemanticUnwindActionV1::Unreachable,
                    )
                    .unwrap(),
                ),
            ),
            block(1, vec![], SemanticTerminatorKindV1::Return),
        ]
    } else {
        vec![
            block(
                0,
                vec![
                    assign(8, CAPTURE, SemanticRvalueKindV1::Use(moved(7, CAPTURE))),
                    assign(
                        9,
                        REF,
                        SemanticRvalueKindV1::Use(projection(
                            8,
                            SemanticProjectionKindV1::Field(0),
                            REF,
                        )),
                    ),
                    assign(
                        10,
                        WORD,
                        SemanticRvalueKindV1::Use(projection(
                            9,
                            SemanticProjectionKindV1::Dereference,
                            WORD,
                        )),
                    ),
                    checked(
                        16,
                        SemanticCheckedBinaryOpV1::Add,
                        copy(1, WORD),
                        copy(10, WORD),
                    ),
                ],
                overflow(
                    16,
                    SemanticBinaryOpV1::Add,
                    copy(1, WORD),
                    copy(10, WORD),
                    1,
                ),
            ),
            block(
                1,
                vec![assign(
                    0,
                    WORD,
                    SemanticRvalueKindV1::Use(checked_field(16, 0)),
                )],
                SemanticTerminatorKindV1::Return,
            ),
        ]
    };
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([tag; 32]),
        if root {
            SemanticFunctionRoleV1::KernelRoot
        } else {
            SemanticFunctionRoleV1::InternalHelper
        },
        template.item_definition_identity(),
        template.monomorphization_identity(),
        template.generic_type_arguments_identity(),
        template.const_generic_arguments_identity(),
        template.source(),
        abi,
        template
            .locals()
            .iter()
            .enumerate()
            .map(|(index, local)| {
                SemanticLocalDeclV1::new(
                    local.identity(),
                    if root && index == 0 { UNIT } else { local.ty() },
                    match index {
                        0 => SemanticLocalRoleV1::Return,
                        1 => SemanticLocalRoleV1::Argument(if root { 0 } else { 1 }),
                        7 if !root => SemanticLocalRoleV1::Argument(0),
                        _ => SemanticLocalRoleV1::Temporary,
                    },
                    local.source(),
                )
            })
            .collect(),
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
}

#[test]
fn shared_capture_frame_entry_actual_call_expansion_retains_exact_lifetime_origin() {
    let mut fixture = Fixture::new(15);
    fixture.locals.extend([NARROW, SIGNED, OTHER_WORD]);
    fixture.types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([19; 32]),
        SemanticLayoutIdentityV1::from_sha256([19; 32]),
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
    ));
    let backend = |primitive, minimum, maximum| {
        SemanticBackendScalarV1::initialized(
            primitive,
            SemanticScalarValidityRangeV1::new(minimum, maximum),
        )
    };
    for ty in [WORD, BOOL, NARROW, SIGNED, OTHER_WORD] {
        let original = &fixture.types[ty.index() as usize];
        let SemanticTypeShapeV1::Scalar(scalar) = original.shape() else {
            unreachable!()
        };
        let (signed, bits, maximum) = match *scalar {
            SemanticScalarTypeV1::Bool => (false, 8, 1),
            SemanticScalarTypeV1::Integer { signed, bits } => (signed, bits, (1u128 << bits) - 1),
            _ => unreachable!(),
        };
        let bytes = u64::from(bits / 8);
        fixture.types[ty.index() as usize] = SemanticTypeDeclV1::new(
            original.identity(),
            original.layout_identity(),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(bytes),
                bytes,
                SemanticBackendReprV1::scalar(backend(
                    SemanticBackendPrimitiveV1::integer(signed, bits, bytes),
                    0,
                    maximum,
                )),
                false,
            )
            .unwrap(),
            original.shape().clone(),
        );
    }
    for ty in [PTR, REF] {
        let original = &fixture.types[ty.index() as usize];
        fixture.types[ty.index() as usize] = SemanticTypeDeclV1::new(
            original.identity(),
            original.layout_identity(),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                SemanticBackendReprV1::scalar(backend(
                    SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                    if ty == REF { 1 } else { 0 },
                    u64::MAX.into(),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    WORD,
                    if ty == REF {
                        SemanticPointerKindV1::Reference
                    } else {
                        SemanticPointerKindV1::Raw
                    },
                    if ty == REF {
                        SemanticMutabilityV1::Immutable
                    } else {
                        SemanticMutabilityV1::Mutable
                    },
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        );
    }
    let original = &fixture.types[CAPTURE.index() as usize];
    fixture.types[CAPTURE.index() as usize] = SemanticTypeDeclV1::new(
        original.identity(),
        original.layout_identity(),
        SemanticTypeLayoutV1::aggregate_with_backend_repr(
            Some(16),
            8,
            SemanticBackendReprV1::ScalarPair {
                first: backend(
                    SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                    1,
                    u64::MAX.into(),
                ),
                second: backend(
                    SemanticBackendPrimitiveV1::integer(false, 64, 8),
                    0,
                    u64::MAX.into(),
                ),
            },
            false,
            SemanticAggregateLayoutV1::new(vec![0, 8], vec![]).unwrap(),
        )
        .unwrap(),
        original.shape().clone(),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
            Some(
                SemanticAbiPointeeInfoV1::new(
                    SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                    8,
                    8,
                )
                .unwrap(),
            ),
            None,
        ),
    );
    let source = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([90; 32])),
        fixture.types.clone(),
        vec![],
        vec![],
        vec![],
        vec![function(true, &fixture), function(false, &fixture)],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    let expansion =
        SemanticCallExpansionV1::try_new(&source, SemanticCallExpansionLimitsV1::default())
            .unwrap();
    expansion.verify_replay(&source).unwrap();
    let view = expansion.root(SemanticFunctionIdV1::from_index(0)).unwrap();
    assert_eq!(view.instances().len(), 2);
    let child = &view.instances()[1];
    let read_local = child.local_start() + 9;
    let mut lives = Vec::new();
    for (bb, (block, origin)) in view
        .body()
        .blocks()
        .iter()
        .zip(view.block_origins())
        .enumerate()
    {
        for (statement, provenance) in block.statements().iter().zip(origin.statements()) {
            if let SemanticStatementKindV1::StorageLive(local) = statement.kind()
                && local.index() == read_local
            {
                assert!(
                    matches!(provenance, SemanticExpandedStatementOriginV1::FrameStorageLive { local, .. } if local.index() == 9)
                );
                lives.push(bb);
            }
        }
    }
    assert_eq!(lives, vec![0]);
    assert_ne!(child.block_start(), 0);
    let results = SemanticAssertProofsV1::analyze(source.types(), view.body()).unwrap();
    assert!(results[child.block_start() as usize]);
    assert!(matches!(
        source.functions()[0].blocks()[0].terminator().kind(),
        SemanticTerminatorKindV1::Call(_)
    ));
    assert!(matches!(
        view.body().blocks()[child.block_start() as usize]
            .terminator()
            .kind(),
        SemanticTerminatorKindV1::Assert { .. }
    ));
}
