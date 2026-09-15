// Reuse the public-MIR component builder without duplicating the getter recipe.
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../fe2o3-pliron/src/production/semantic_ssa/defined_math_results/fixture.rs"
));

include!("capture_fixture.rs");
include!("context_reborrow_fixture.rs");
include!("transpose_endpoint_fixture.rs");

pub(super) fn full_source(reborrow: bool, kill_math: bool) -> AdmittedInertSemanticMirV1 {
    source_with_dead_owner(reborrow, kill_math.then_some(3))
}

pub(super) fn policy_dead_source(reborrow: bool) -> AdmittedInertSemanticMirV1 {
    source_with_dead_owner(reborrow, Some(4))
}

fn source_with_dead_owner(reborrow: bool, dead_owner: Option<u32>) -> AdmittedInertSemanticMirV1 {
    let base = source(false, true);
    let SemanticDefinedCapabilityContractV1::KernelMathDerive(original) =
        base.functions()[1].defined_capability_contract().unwrap()
    else {
        unreachable!()
    };
    let provenance = original.provenance();
    let mut types = base.types().to_vec();
    let pointer = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
    );
    for (index, shape, layout, properties) in [
        (5, types[3].shape().clone(), types[3].layout().clone(), None),
        (
            6,
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    ty(3),
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Immutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
            types[2].layout().clone(),
            Some(types[2].abi_properties().clone()),
        ),
        (
            7,
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    ty(5),
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Immutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
            types[2].layout().clone(),
            Some(types[2].abi_properties().clone()),
        ),
        (8, types[3].shape().clone(), types[3].layout().clone(), None),
        (
            9,
            SemanticTypeShapeV1::Aggregate(
                SemanticAggregateTypeV1::new(vec![ty(6), ty(7), ty(8)]).unwrap(),
            ),
            SemanticTypeLayoutV1::aggregate_with_backend_repr(
                Some(16),
                8,
                SemanticBackendReprV1::ScalarPair {
                    first: pointer,
                    second: pointer,
                },
                false,
                SemanticAggregateLayoutV1::new(vec![0, 8, 16], vec![]).unwrap(),
            )
            .unwrap(),
            Some(
                SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                    Some(
                        SemanticAbiPointeeInfoV1::new(
                            SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                            0,
                            1,
                        )
                        .unwrap(),
                    ),
                    Some(
                        SemanticAbiPointeeInfoV1::new(
                            SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                            0,
                            1,
                        )
                        .unwrap(),
                    ),
                ),
            ),
        ),
        (
            10,
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    ty(9),
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Immutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
            types[2].layout().clone(),
            Some(
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
        ),
        (
            11,
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 32 }),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(4),
                4,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::float(32, 4),
                    SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
                )),
                false,
            )
            .unwrap(),
            None,
        ),
    ] {
        let mut declaration = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([209 + index; 32]),
            SemanticLayoutIdentityV1::from_sha256([209 + index; 32]),
            layout,
            shape,
        );
        if let Some(properties) = properties {
            declaration = declaration.with_rustc_abi_properties(properties);
        }
        types.push(declaration);
    }
    let attrs = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let pointer_attrs = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, true, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let bound_abi = |tag: u8, inputs: &[u32], output: u32, pair: bool| {
        let old = abi(tag, inputs, output, false);
        SemanticFunctionAbiV1::from_rustc_with_source_signature(
            old.identity(),
            old.layout_identity(),
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::Rust,
            false,
            false,
            inputs.len() as u32,
            inputs.iter().copied().map(ty).collect(),
            ty(output),
            inputs
                .iter()
                .map(|input| {
                    SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                        ty(*input),
                        SemanticAbiPassModeV1::Direct(if *input == 11 {
                            attrs
                        } else {
                            SemanticAbiValueAttributesV1::new(
                                SemanticAbiRegularAttributesV1::new(
                                    true,
                                    Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
                                    true,
                                    true,
                                    false,
                                    true,
                                ),
                                SemanticAbiExtensionV1::None,
                                if *input == 10 { 16 } else { 0 },
                                if *input == 10 { Some(8) } else { None },
                            )
                            .unwrap()
                        }),
                    ))
                })
                .collect(),
            SemanticAbiValueV1::new(
                ty(output),
                if pair {
                    SemanticAbiPassModeV1::Pair {
                        first: pointer_attrs,
                        second: pointer_attrs,
                    }
                } else {
                    SemanticAbiPassModeV1::Direct(attrs)
                },
            ),
        )
        .unwrap()
        .with_source_argument_ownership(
            inputs
                .iter()
                .map(|input| {
                    if *input == 11 {
                        SemanticSourceArgumentOwnershipV1::ByValue
                    } else {
                        SemanticSourceArgumentOwnershipV1::SharedBorrow
                    }
                })
                .collect(),
        )
        .unwrap()
    };
    let borrow = |destination, reference, owner, owned| {
        SemanticStatementV1::new(
            location(),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(destination, reference),
                SemanticRvalueV1::new(
                    ty(reference),
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Shared,
                        place: place(owner, owned),
                    },
                ),
            )),
        )
    };
    let mut use_statements = vec![borrow(8, 10, 7, 9)];
    if reborrow {
        use_statements.push(SemanticStatementV1::new(
            location(),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(10, 10),
                SemanticRvalueV1::new(
                    ty(10),
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Shared,
                        place: SemanticPlaceV1::new(
                            SemanticLocalIdV1::from_index(8),
                            vec![
                                SemanticProjectionV1::new(
                                    SemanticProjectionKindV1::Dereference,
                                    ty(9),
                                )
                                .unwrap(),
                            ],
                            ty(9),
                        )
                        .unwrap(),
                    },
                ),
            )),
        ));
        use_statements.push(SemanticStatementV1::new(
            location(),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(11, 10),
                SemanticRvalueV1::new(
                    ty(10),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(10, 10))),
                ),
            )),
        ));
    }
    if let Some(owner) = dead_owner {
        use_statements.push(SemanticStatementV1::new(
            location(),
            SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(owner)),
        ));
    }
    let scalar = SemanticOperandV1::Constant(SemanticConstantV1::new(
        ty(11),
        SemanticConstantValueV1::Scalar(
            SemanticScalarValueV1::new(1.0_f32.to_bits().into(), 4).unwrap(),
        ),
    ));
    let root = function(
        135,
        abi(133, &[], 0, true),
        [0, 1, 2, 3, 5, 6, 7, 9, 10, 11, 10, 10]
            .into_iter()
            .enumerate()
            .map(|(i, value)| {
                local(
                    180 + i as u8,
                    value,
                    if i == 0 {
                        SemanticLocalRoleV1::Return
                    } else {
                        SemanticLocalRoleV1::Temporary
                    },
                )
            })
            .collect(),
        vec![
            block(180, vec![], call(4, vec![], 1, 1, 1)),
            block(
                181,
                vec![borrow(2, 2, 1, 1)],
                call(1, vec![SemanticOperandV1::Copy(place(2, 2))], 3, 3, 2),
            ),
            block(
                182,
                vec![],
                call(6, vec![SemanticOperandV1::Copy(place(2, 2))], 4, 5, 3),
            ),
            block(
                183,
                vec![borrow(5, 6, 3, 3), borrow(6, 7, 4, 5)],
                call(
                    3,
                    vec![
                        SemanticOperandV1::Copy(place(5, 6)),
                        SemanticOperandV1::Copy(place(6, 7)),
                    ],
                    7,
                    9,
                    4,
                ),
            ),
            block(
                184,
                use_statements,
                call(
                    7,
                    vec![
                        SemanticOperandV1::Copy(place(if reborrow { 11 } else { 8 }, 10)),
                        scalar,
                    ],
                    9,
                    11,
                    5,
                ),
            ),
            block(185, vec![], SemanticTerminatorKindV1::Return),
        ],
        true,
    )
    .with_kernel_entry(base.functions()[0].kernel_entry().unwrap().clone());
    let getter = function(
        160,
        abi(160, &[2], 3, false),
        base.functions()[1].locals().to_vec(),
        base.functions()[1].blocks().to_vec(),
        false,
    );
    let bridge = function(
        161,
        abi(161, &[], 3, false),
        base.functions()[2].locals().to_vec(),
        vec![
            block(180, vec![], call(5, vec![], 1, 4, 1)),
            block(181, vec![], SemanticTerminatorKindV1::Return),
        ],
        false,
    );
    let bind = function(
        162,
        bound_abi(162, &[6, 7], 9, true),
        vec![
            local(180, 9, SemanticLocalRoleV1::Return),
            local(181, 6, SemanticLocalRoleV1::Argument(0)),
            local(182, 7, SemanticLocalRoleV1::Argument(1)),
        ],
        vec![block(
            180,
            vec![SemanticStatementV1::new(
                location(),
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    place(0, 9),
                    SemanticRvalueV1::new(
                        ty(9),
                        SemanticRvalueKindV1::aggregate(
                            SemanticAggregateKindV1::Aggregate,
                            vec![
                                SemanticOperandV1::Copy(place(1, 6)),
                                SemanticOperandV1::Copy(place(2, 7)),
                                SemanticOperandV1::Constant(SemanticConstantV1::new(
                                    ty(8),
                                    SemanticConstantValueV1::ZeroSized,
                                )),
                            ],
                        )
                        .unwrap(),
                    ),
                )),
            )],
            SemanticTerminatorKindV1::Return,
        )],
        false,
    );
    let policy = SemanticTypeIdentityV1::from_sha256([8; 32]);
    let issue = SemanticExecutionCapabilityContractV1::new_kernel_scoped(
        SemanticExecutionCapabilityOperationV1::NumericalPolicyIssue {
            context: ty(2),
            capability: ty(5),
            policy,
        },
        SemanticExecutionCapabilitySignatureV1::new(&[ty(2)], ty(5)).unwrap(),
        provenance,
        SemanticFunctionIdentityV1::from_sha256([182; 32]),
    )
    .unwrap();
    let consumer = SemanticNumericalPolicyMathContractV1::new(
        SemanticNumericalPolicyMathTypesV1::new([10, 9, 6, 3, 7, 5, 11].map(ty)),
        policy,
        original.kernel_brand(),
        SemanticF32MathFunctionV1::Sqrt,
        SemanticNumericalModeV1::StrictIeee,
        SemanticF32MathFunctionV1::Sqrt.required_implementation(),
        provenance,
        SemanticFunctionIdentityV1::from_sha256([183; 32]),
    )
    .unwrap();
    let mut functions = vec![root, getter, bridge, bind];
    let callables = vec![
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(2)),
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(3)),
        base.callables()[3].clone(),
        base.callables()[4].clone(),
        terminal(
            182,
            abi(182, &[2], 5, false),
            SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract: issue },
        ),
        terminal(
            183,
            bound_abi(183, &[10, 11], 11, false),
            SemanticCompilerIntrinsicOperationV1::PolicyMathF32 { contract: consumer },
        ),
    ];
    let getter = SemanticKernelMathDeriveV1::for_defined_function(
        SemanticFunctionIdV1::from_index(1),
        &functions,
        &callables,
        &types,
        original.types(),
        provenance,
        original.kernel_brand(),
    )
    .unwrap();
    let bind = SemanticPolicyMathBindV1::for_defined_function(
        SemanticFunctionIdV1::from_index(3),
        &functions,
        &callables,
        &types,
        SemanticPolicyMathBindTypesV1::new([6, 3, 7, 5, 9].map(ty)),
        provenance,
        policy,
        original.kernel_brand(),
    )
    .unwrap();
    functions[1] = functions[1]
        .clone()
        .with_defined_capability_contract(SemanticDefinedCapabilityContractV1::KernelMathDerive(
            getter,
        ))
        .unwrap();
    functions[3] = functions[3]
        .clone()
        .with_defined_capability_contract(SemanticDefinedCapabilityContractV1::PolicyMathBind(bind))
        .unwrap();
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
    .admit_exact_v21(SemanticMirLimitsV1::default())
    .unwrap()
}
