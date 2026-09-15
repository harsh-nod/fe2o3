// Included in the existing component fixture module. Reuses its independently
// admitted Context/Workgroup issuer; this is not live frontend source custody.
pub(super) fn transpose_endpoint_source(
    kill: bool,
    backedge: bool,
    prior: bool,
) -> AdmittedInertSemanticMirV1 {
    let base = context_reborrow_source(false, false, false);
    let mut types = base.types().to_vec();
    assert_eq!(types.len(), 18);
    let scalar32 = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 32, 4),
        SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
    );
    let scalar64 = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 64, 8),
        SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
    );
    types.push(
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([227; 32]),
            SemanticLayoutIdentityV1::from_sha256([227; 32]),
            types[2].layout().clone(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    ty(17),
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Immutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        )
        .with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false)
                .with_rustc_layout_is_noundef(true)
                .with_scalar_pointee_info(
                    Some(
                        SemanticAbiPointeeInfoV1::new(
                            SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                            16,
                            8,
                        )
                        .unwrap(),
                    ),
                    None,
                ),
        ),
    );
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([228; 32]),
        SemanticLayoutIdentityV1::from_sha256([228; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(4),
            4,
            SemanticBackendReprV1::scalar(scalar32),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32,
        }),
    ));
    for (index, members, offsets) in [
        (20, vec![], vec![]),
        (21, vec![], vec![]),
        (22, vec![], vec![]),
        (23, vec![19, 21, 20, 22], vec![0, 4, 4, 4]),
        (24, vec![], vec![]),
        (25, vec![23, 20, 24], vec![0, 4, 4]),
        (26, vec![16, 20], vec![0, 0]),
        (27, vec![15], vec![0]),
        (28, vec![14, 14, 27, 15], vec![0, 8, 16, 16]),
        (29, vec![27, 24], vec![0, 0]),
        (30, vec![28, 29], vec![0, 16]),
    ] {
        assert_eq!(types.len(), index as usize);
        let (size, align, backend) = match index {
            23 | 25 => (4, 4, SemanticBackendReprV1::scalar(scalar32)),
            28 | 30 => (
                16,
                8,
                SemanticBackendReprV1::ScalarPair {
                    first: scalar64,
                    second: scalar64,
                },
            ),
            _ => (0, 1, SemanticBackendReprV1::memory(true)),
        };
        let fields = SemanticAggregateTypeV1::new(members.into_iter().map(ty).collect()).unwrap();
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([209 + index as u8; 32]),
            SemanticLayoutIdentityV1::from_sha256([209 + index as u8; 32]),
            SemanticTypeLayoutV1::aggregate_with_backend_repr(
                Some(size),
                align,
                backend,
                false,
                SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
            )
            .unwrap(),
            if index == 30 {
                SemanticTypeShapeV1::Tuple(fields)
            } else {
                SemanticTypeShapeV1::Aggregate(fields)
            },
        ));
    }
    let SemanticCallableDeclV1::CompilerIntrinsic {
        operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract: wg },
        ..
    } = base.callables()[8]
    else {
        unreachable!()
    };
    let subgroup = SemanticExecutionCapabilityContractV1::new(
        SemanticExecutionCapabilityOperationV1::SubgroupDeriveBorrowed {
            workgroup_reference: ty(18),
            workgroup: ty(17),
            subgroup: ty(25),
            width: 64,
        },
        SemanticExecutionCapabilitySignatureV1::new(&[ty(18)], ty(25)).unwrap(),
        wg.provenance(),
        wg.workgroup_brand().unwrap(),
        wg.epoch_before().unwrap(),
        None,
        SemanticFunctionIdentityV1::from_sha256([240; 32]),
    )
    .unwrap();
    let transpose = SemanticGfx950TransposeContractV1::new(
        SemanticGfx950TransposeOperationV1::Publish {
            input_tile: ty(26),
            input_workgroup: ty(17),
            transition: ty(30),
            output_workgroup: ty(28),
            output_tile: ty(29),
        },
        SemanticGfx950LdsTransposeFormatV1::Fp8E4M3,
        types[20].identity(),
        Some(types[24].identity()),
    )
    .unwrap();
    let publish = SemanticExecutionCapabilityContractV1::new(
        SemanticExecutionCapabilityOperationV1::Gfx950Transpose(transpose),
        SemanticExecutionCapabilitySignatureV1::new(&[ty(26), ty(17)], ty(30)).unwrap(),
        wg.provenance(),
        wg.workgroup_brand().unwrap(),
        wg.epoch_before().unwrap(),
        Some(types[27].identity()),
        SemanticFunctionIdentityV1::from_sha256([241; 32]),
    )
    .unwrap();
    let attrs = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let shared = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(
            true,
            Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
            true,
            true,
            false,
            true,
        ),
        SemanticAbiExtensionV1::None,
        16,
        Some(8),
    )
    .unwrap();
    let pair = SemanticAbiPassModeV1::Pair {
        first: attrs,
        second: attrs,
    };
    let subgroup_abi = SemanticFunctionAbiV1::from_rustc_with_source_signature(
        SemanticAbiIdentityV1::from_sha256([240; 32]),
        SemanticLayoutIdentityV1::from_sha256([240; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        1,
        vec![ty(18)],
        ty(25),
        vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            ty(18),
            SemanticAbiPassModeV1::Direct(shared),
        ))],
        SemanticAbiValueV1::new(ty(25), SemanticAbiPassModeV1::Direct(attrs)),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::SharedBorrow])
    .unwrap();
    let publish_abi = SemanticFunctionAbiV1::from_rustc_with_source_signature(
        SemanticAbiIdentityV1::from_sha256([241; 32]),
        SemanticLayoutIdentityV1::from_sha256([241; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        2,
        vec![ty(26), ty(17)],
        ty(30),
        vec![
            SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                ty(26),
                SemanticAbiPassModeV1::Ignore,
            )),
            SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(ty(17), pair.clone())),
        ],
        SemanticAbiValueV1::new(ty(30), pair),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue; 2])
    .unwrap();
    let mut callables = base.callables().to_vec();
    callables.push(terminal(
        240,
        subgroup_abi,
        SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract: subgroup },
    ));
    callables.push(terminal(
        241,
        publish_abi,
        SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract: publish },
    ));
    let root = &base.functions()[0];
    let mut locals = root.locals().to_vec();
    assert_eq!(locals.len(), 20);
    for (index, ty) in [(20, 18), (21, 25), (22, 30), (23, 17)] {
        locals.push(local(220 + index, ty, SemanticLocalRoleV1::Temporary));
    }
    let borrow = SemanticStatementV1::new(
        location(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(20, 18),
            SemanticRvalueV1::new(
                ty(18),
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: place(19, 17),
                },
            ),
        )),
    );
    let mut blocks = root.blocks().to_vec();
    assert_eq!(blocks.len(), 8);
    let copy = |destination, source| {
        SemanticStatementV1::new(
            location(),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(destination, 17),
                SemanticRvalueV1::new(
                    ty(17),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(source, 17))),
                ),
            )),
        )
    };
    let mut before = Vec::new();
    if kill {
        before.push(copy(23, 19));
    }
    before.push(borrow);
    blocks[7] = block(
        240,
        before,
        call(9, vec![SemanticOperandV1::Copy(place(20, 18))], 21, 25, 8),
    );
    let statements = if kill { vec![copy(19, 23)] } else { vec![] };
    blocks.push(block(
        241,
        statements,
        call(
            10,
            vec![
                SemanticOperandV1::Constant(SemanticConstantV1::new(
                    ty(26),
                    SemanticConstantValueV1::ZeroSized,
                )),
                SemanticOperandV1::Copy(place(19, 17)),
            ],
            22,
            30,
            9,
        ),
    ));
    blocks.push(block(
        242,
        vec![],
        if backedge {
            SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::Goto,
                SemanticBlockIdV1::from_index(8),
            ))
        } else {
            SemanticTerminatorKindV1::Return
        },
    ));
    if prior {
        blocks[9] = block(
            242,
            vec![],
            call(
                10,
                vec![
                    SemanticOperandV1::Constant(SemanticConstantV1::new(
                        ty(26),
                        SemanticConstantValueV1::ZeroSized,
                    )),
                    SemanticOperandV1::Copy(place(19, 17)),
                ],
                22,
                30,
                10,
            ),
        );
        blocks.push(block(243, vec![], SemanticTerminatorKindV1::Return));
    }
    let mut functions = base.functions().to_vec();
    functions[0] = function(135, root.abi().clone(), locals, blocks, true)
        .with_kernel_entry(root.kernel_entry().unwrap().clone());
    InertSemanticMirRequestV1::new_with_callables(
        base.target(),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        callables,
        base.roots().to_vec(),
    )
    .unwrap()
    .admit_exact_v24(SemanticMirLimitsV1::default())
    .unwrap()
}
