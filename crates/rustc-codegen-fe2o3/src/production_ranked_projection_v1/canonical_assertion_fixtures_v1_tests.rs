// Genuine semantic owners for the canonical assertion consumer tests.

const A_UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const A_BOOL: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const A_U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const A_U64: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const A_CHECKED: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
const A_BOOL_PTR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
const A_ARRAY: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(6);
const A_NAME: &str = "canonical_assertion_root";

fn assertion_types() -> Vec<SemanticTypeDeclV1> {
    let scalar = |tag, size, shape, maximum| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(tag)),
            SemanticLayoutIdentityV1::from_sha256(bytes(tag)),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(size),
                size,
                neutral_scalar_backend_v1(
                    SemanticBackendPrimitiveV1::integer(false, (size * 8) as u16, size),
                    maximum,
                ),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(shape),
        )
    };
    vec![
        neutral_semantic_types_v1().remove(0),
        scalar(231, 1, SemanticScalarTypeV1::Bool, 1),
        scalar(
            232,
            4,
            SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            },
            u32::MAX.into(),
        ),
        scalar(
            233,
            8,
            SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 64,
            },
            u64::MAX.into(),
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(234)),
            SemanticLayoutIdentityV1::from_sha256(bytes(234)),
            SemanticTypeLayoutV1::aggregate(
                Some(16),
                8,
                SemanticAggregateLayoutV1::new(
                    vec![0, 8],
                    vec![SemanticPaddingV1::new(9, 7).unwrap()],
                )
                .unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![A_U64, A_BOOL]).unwrap()),
        ),
        neutral_pointer_type_v1(
            235,
            A_BOOL,
            SemanticPointerKindV1::Raw,
            SemanticMutabilityV1::Mutable,
            0,
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(236)),
            SemanticLayoutIdentityV1::from_sha256(bytes(236)),
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                32,
                4,
                SemanticFieldsShapeV1::array(4, 8),
                SemanticRustcVariantsV1::Single { index: 0 },
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                4,
                0,
                SemanticTypeLayoutDetailsV1::None,
            )
            .unwrap(),
            SemanticTypeShapeV1::Array {
                element: A_U32,
                length: 8,
            },
        ),
    ]
}

fn assertion_root(
    locals: Vec<(SemanticTypeIdV1, SemanticLocalRoleV1)>,
    inputs: Vec<SemanticTypeIdV1>,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    assertion_root_with_access(locals, inputs, blocks, true)
}

fn assertion_root_with_access(
    mut locals: Vec<(SemanticTypeIdV1, SemanticLocalRoleV1)>,
    inputs: Vec<SemanticTypeIdV1>,
    mut blocks: Vec<SemanticBasicBlockV1>,
    include_access: bool,
) -> SemanticFunctionDeclV1 {
    // Keep the shared fixture catalog in the exact MIR type closure. These
    // unread temporaries require no initialization and emit no graph values.
    for ty in [A_UNIT, A_BOOL, A_U32, A_U64, A_CHECKED, A_BOOL_PTR, A_ARRAY] {
        if !locals.iter().any(|(local_type, _)| *local_type == ty) {
            locals.push((ty, SemanticLocalRoleV1::Temporary));
        }
    }
    if include_access {
        let array = u32::try_from(locals.len()).unwrap();
        let index = array + 1;
        locals.extend([
            (A_ARRAY, SemanticLocalRoleV1::Temporary),
            (A_U32, SemanticLocalRoleV1::Temporary),
        ]);
        let entry = &blocks[0];
        let mut statements = entry.statements().to_vec();
        statements.extend(assertion_ranked_write_statements(array, index));
        blocks[0] = SemanticBasicBlockV1::new(
            entry.identity(),
            entry.source(),
            statements,
            entry.terminator().clone(),
        )
        .unwrap();
    }
    let argument_count = inputs.len();
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(236)),
        SemanticLayoutIdentityV1::from_sha256(bytes(250)),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        inputs.len() as u32,
        inputs
            .into_iter()
            .map(|ty| {
                if ty == A_BOOL {
                    SemanticAbiValueV1::new(
                        ty,
                        SemanticAbiPassModeV1::Direct(
                            SemanticAbiValueAttributesV1::new(
                                SemanticAbiRegularAttributesV1::new(
                                    false, None, false, false, false, true,
                                ),
                                SemanticAbiExtensionV1::ZeroExtend,
                                0,
                                None,
                            )
                            .unwrap(),
                        ),
                    )
                } else {
                    neutral_plain_direct_abi_value_v1(ty)
                }
            })
            .map(SemanticAbiArgumentV1::source)
            .collect(),
        SemanticAbiValueV1::new(A_UNIT, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![
        SemanticSourceArgumentOwnershipV1::ByValue;
        argument_count
    ])
    .unwrap();
    let dimensions = SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(237)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(238)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(239)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(240)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(241)),
        SemanticSourceProvenanceV1::unavailable(),
        abi,
        locals
            .into_iter()
            .enumerate()
            .map(|(i, (ty, role))| local(100 + i as u8, ty, role))
            .collect(),
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(A_NAME.as_bytes().to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256(bytes(247)),
        SemanticKernelSourceContractV1::new(
            Some(
                SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None)
                    .unwrap(),
            ),
            None,
            None,
        )
        .unwrap(),
    ))
}

// Same admitted private fixed-array indexed write as the existing deep-constant
// projection fixture. It gives every full-entry assertion test a real ranked
// effect without inventing argument custody or changing its assertion blocks.
fn assertion_ranked_write_statements(array: u32, index: u32) -> Vec<SemanticStatementV1> {
    vec![
        typed_assignment(
            index,
            A_U32,
            SemanticRvalueKindV1::Use(typed_constant(A_U32, 0, 4)),
        ),
        statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(array),
                vec![
                    SemanticProjectionV1::new(
                        SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(index)),
                        A_U32,
                    )
                    .unwrap(),
                ],
                A_U32,
            )
            .unwrap(),
            SemanticRvalueV1::new(
                A_U32,
                SemanticRvalueKindV1::Use(typed_constant(A_U32, 7, 4)),
            ),
        ))),
    ]
}

fn assertion_materialized(
    function: SemanticFunctionDeclV1,
) -> fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1 {
    assertion_materialized_functions(assertion_types(), vec![function])
}

fn assertion_ssa_functions(
    types: Vec<SemanticTypeDeclV1>,
    functions: Vec<SemanticFunctionDeclV1>,
) -> ProductionSemanticSsaOwnerV1 {
    let callables = (0..functions.len())
        .map(|i| SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(i as u32)))
        .collect();
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(250))),
        types,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        functions,
        callables,
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let semantic = ProductionSemanticMirOwnerV1::try_new(
        admitted,
        fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        semantic,
        fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn assertion_materialized_functions(
    types: Vec<SemanticTypeDeclV1>,
    functions: Vec<SemanticFunctionDeclV1>,
) -> fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1 {
    let ssa = assertion_ssa_functions(types, functions);
    materialize_ranked_fixture_v1(ssa, &[ranked_root_input_1d(A_NAME, 247, 64)]).unwrap()
}

// Reuse a production-admitted collective memory fixture, then retain a real
// assertion before its final return. Private-array attachment stays a separate
// unsupported case instead of becoming an incidental requirement of this test.
fn collective_assertion_materialized_v1() -> fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1 {
    let original = neutral_ranked_source_for_operation_v1(
        SemanticCompilerIntrinsicOperationV1::NeutralWorkgroupReduceSum {
            context: NEUTRAL_CONTEXT_TYPE,
            dynamic_lds: NEUTRAL_DYNAMIC_LDS_TYPE,
            element_storage: NEUTRAL_ELEMENT_TYPE,
            element: NEUTRAL_ELEMENT_TYPE,
        },
        64,
    );
    let semantic = original.source_semantic();
    let mut types = semantic.types().to_vec();
    let bool_type = SemanticTypeIdV1::from_index(u32::try_from(types.len()).unwrap());
    types.push(assertion_types()[A_BOOL.index() as usize].clone());
    let source = &semantic.functions()[0];
    let mut blocks = source.blocks().to_vec();
    assert_eq!(blocks.len(), 4);
    assert!(matches!(
        blocks[3].terminator().kind(),
        SemanticTerminatorKindV1::Return
    ));
    blocks[3] = block(
        233,
        Vec::new(),
        SemanticTerminatorKindV1::Assert {
            condition: typed_constant(bool_type, 1, 1),
            expected: true,
            message: SemanticAssertMessageV1::DivisionByZero(typed_constant(
                NEUTRAL_ELEMENT_TYPE,
                0,
                4,
            )),
            target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, 4),
            unwind: SemanticUnwindActionV1::Unreachable,
        },
    );
    blocks.push(block(234, Vec::new(), SemanticTerminatorKindV1::Return));
    let function = SemanticFunctionDeclV1::new(
        source.identity(),
        source.role(),
        source.item_definition_identity(),
        source.monomorphization_identity(),
        source.generic_type_arguments_identity(),
        source.const_generic_arguments_identity(),
        source.source(),
        source.abi().clone(),
        source.locals().to_vec(),
        source.entry(),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(source.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(250))),
        types,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![function],
        semantic.callables().to_vec(),
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let owner = ProductionSemanticMirOwnerV1::try_new(
        admitted,
        fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap();
    let ssa = ProductionSemanticSsaOwnerV1::try_new(
        owner,
        fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    materialize_ranked_fixture_v1(
        ssa,
        &[ranked_root_input_1d("neutral_generated_hostile", 247, 64)],
    )
    .unwrap()
}

fn assertion_project(
    materialized: fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1,
) -> Result<ProductionRankedSemanticProgramV1, ProductionRankedProjectionErrorV1> {
    project_and_verify_ranked_materialized_semantic_mir_v1(
        materialized,
        &[ranked_root_input_1d(A_NAME, 247, 64)],
        &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
    )
}

fn assertion_terminator(
    condition: SemanticOperandV1,
    expected: bool,
    target: u32,
) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Assert {
        condition,
        expected,
        message: SemanticAssertMessageV1::DivisionByZero(typed_constant(A_U64, 1, 8)),
        target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, target),
        unwind: SemanticUnwindActionV1::Unreachable,
    }
}

fn literal_assertion(value: bool, expected: bool, alias: bool) -> SemanticFunctionDeclV1 {
    let mut statements = Vec::new();
    let condition = if alias {
        statements.push(typed_assignment(
            1,
            A_BOOL,
            SemanticRvalueKindV1::Use(typed_constant(A_BOOL, value.into(), 1)),
        ));
        typed_operand(1, A_BOOL)
    } else {
        typed_constant(A_BOOL, value.into(), 1)
    };
    assertion_root(
        vec![
            (A_UNIT, SemanticLocalRoleV1::Return),
            (A_BOOL, SemanticLocalRoleV1::Temporary),
        ],
        Vec::new(),
        vec![
            block(
                201,
                statements,
                assertion_terminator(condition, expected, 1),
            ),
            block(202, Vec::new(), SemanticTerminatorKindV1::Return),
        ],
    )
}

fn dynamic_assertion() -> SemanticFunctionDeclV1 {
    assertion_root(
        vec![
            (A_UNIT, SemanticLocalRoleV1::Return),
            (A_BOOL, SemanticLocalRoleV1::Argument(0)),
        ],
        vec![A_BOOL],
        vec![
            block(
                201,
                Vec::new(),
                assertion_terminator(typed_operand(1, A_BOOL), true, 1),
            ),
            block(202, Vec::new(), SemanticTerminatorKindV1::Return),
        ],
    )
}

fn checked_field(local: u32, field: u32, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(local),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(field), ty).unwrap()],
            ty,
        )
        .unwrap(),
    )
}

fn escaped_assertion(checked: bool) -> SemanticFunctionDeclV1 {
    let mut statements = if checked {
        vec![
            typed_assignment(
                3,
                A_CHECKED,
                SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                    SemanticCheckedBinaryOpV1::Add,
                    typed_constant(A_U64, 1, 8),
                    typed_constant(A_U64, 2, 8),
                )),
            ),
            typed_assignment(
                1,
                A_BOOL,
                SemanticRvalueKindV1::Use(checked_field(3, 1, A_BOOL)),
            ),
        ]
    } else {
        vec![typed_assignment(
            1,
            A_BOOL,
            SemanticRvalueKindV1::Use(typed_constant(A_BOOL, 1, 1)),
        )]
    };
    statements.push(typed_assignment(
        2,
        A_BOOL_PTR,
        SemanticRvalueKindV1::AddressOf {
            mutability: SemanticMutabilityV1::Mutable,
            place: typed_place(1, A_BOOL),
        },
    ));
    statements.push(statement(SemanticStatementKindV1::Store(
        SemanticMemoryStoreV1::new(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(2),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, A_BOOL)
                        .unwrap(),
                ],
                A_BOOL,
            )
            .unwrap(),
            typed_constant(A_BOOL, u128::from(checked), 1),
            SemanticVolatilityV1::NonVolatile,
            None,
        ),
    )));
    let message = if checked {
        SemanticAssertMessageV1::Overflow {
            operation: SemanticBinaryOpV1::Add,
            left: typed_constant(A_U64, 1, 8),
            right: typed_constant(A_U64, 2, 8),
        }
    } else {
        SemanticAssertMessageV1::DivisionByZero(typed_constant(A_U64, 1, 8))
    };
    assertion_root(
        vec![
            (A_UNIT, SemanticLocalRoleV1::Return),
            (A_BOOL, SemanticLocalRoleV1::Temporary),
            (A_BOOL_PTR, SemanticLocalRoleV1::Temporary),
            (A_CHECKED, SemanticLocalRoleV1::Temporary),
        ],
        Vec::new(),
        vec![
            block(
                201,
                statements,
                SemanticTerminatorKindV1::Assert {
                    condition: typed_operand(1, A_BOOL),
                    expected: !checked,
                    message,
                    target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, 1),
                    unwind: SemanticUnwindActionV1::Unreachable,
                },
            ),
            block(202, Vec::new(), SemanticTerminatorKindV1::Return),
        ],
    )
}

fn checked_induction_assertion() -> SemanticFunctionDeclV1 {
    let step = || typed_constant(A_U64, 16, 8);
    let go = |target| SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, target));
    assertion_root(
        vec![
            (A_UNIT, SemanticLocalRoleV1::Return),
            (A_U64, SemanticLocalRoleV1::Temporary),
            (A_BOOL, SemanticLocalRoleV1::Temporary),
            (A_U64, SemanticLocalRoleV1::Temporary),
            (A_U32, SemanticLocalRoleV1::Argument(0)),
            (A_CHECKED, SemanticLocalRoleV1::Temporary),
            (A_U64, SemanticLocalRoleV1::Temporary),
        ],
        vec![A_U32],
        vec![
            block(
                210,
                vec![
                    typed_assignment(
                        1,
                        A_U64,
                        SemanticRvalueKindV1::Use(typed_constant(A_U64, 0, 8)),
                    ),
                    typed_assignment(
                        3,
                        A_U64,
                        SemanticRvalueKindV1::Cast {
                            kind: SemanticCastKindV1::Integer,
                            operand: typed_operand(4, A_U32),
                        },
                    ),
                ],
                go(1),
            ),
            block(
                211,
                vec![typed_assignment(
                    2,
                    A_BOOL,
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::LessThan,
                        left: typed_operand(1, A_U64),
                        right: typed_operand(3, A_U64),
                    },
                )],
                zero_switch(2, A_BOOL, 5, 2),
            ),
            block(212, Vec::new(), go(3)),
            block(
                213,
                vec![
                    typed_assignment(6, A_U64, SemanticRvalueKindV1::Use(typed_operand(1, A_U64))),
                    typed_assignment(
                        5,
                        A_CHECKED,
                        SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                            SemanticCheckedBinaryOpV1::Add,
                            typed_operand(6, A_U64),
                            step(),
                        )),
                    ),
                ],
                SemanticTerminatorKindV1::Assert {
                    condition: checked_field(5, 1, A_BOOL),
                    expected: false,
                    message: SemanticAssertMessageV1::Overflow {
                        operation: SemanticBinaryOpV1::Add,
                        left: typed_operand(6, A_U64),
                        right: step(),
                    },
                    target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, 4),
                    unwind: SemanticUnwindActionV1::Unreachable,
                },
            ),
            block(
                214,
                vec![typed_assignment(
                    1,
                    A_U64,
                    SemanticRvalueKindV1::Use(checked_field(5, 0, A_U64)),
                )],
                go(1),
            ),
            block(215, Vec::new(), SemanticTerminatorKindV1::Return),
        ],
    )
}

fn wrapped_literal_assertion_ssa() -> ProductionSemanticSsaOwnerV1 {
    let result_ty = SemanticTypeIdV1::from_index(7);
    let mut types = assertion_types();
    let tag = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 32, 4),
        SemanticScalarValidityRangeV1::new(0, 1),
    );
    let variant = |index| {
        SemanticEnumVariantLayoutV1::from_rustc(
            index,
            4,
            4,
            SemanticFieldsShapeV1::arbitrary(vec![4], vec![0]).unwrap(),
            SemanticBackendReprV1::scalar(tag),
            None,
            false,
            None,
            4,
            0,
            SemanticAggregateLayoutV1::new(vec![4], Vec::new()).unwrap(),
        )
        .unwrap()
    };
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(242)),
        SemanticLayoutIdentityV1::from_sha256(bytes(242)),
        SemanticTypeLayoutV1::enum_layout_with_backend_repr(
            4,
            4,
            SemanticBackendReprV1::scalar(tag),
            false,
            SemanticEnumLayoutV1::new(
                vec![variant(0), variant(1)],
                SemanticEnumEncodingV1::Direct(SemanticDirectEnumEncodingV1::new(0, 0, tag)),
            )
            .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::enum_type(
            A_U32,
            vec![
                SemanticEnumVariantV1::new(0, SemanticAggregateTypeV1::new(vec![A_UNIT]).unwrap()),
                SemanticEnumVariantV1::new(1, SemanticAggregateTypeV1::new(vec![A_UNIT]).unwrap()),
            ],
        )
        .unwrap(),
    ));
    let wrapper = assertion_root_with_access(
        vec![
            (A_UNIT, SemanticLocalRoleV1::Return),
            (result_ty, SemanticLocalRoleV1::Temporary),
        ],
        Vec::new(),
        vec![
            block(
                220,
                Vec::new(),
                neutral_test_call_v1(1, Vec::new(), 1, result_ty, 1),
            ),
            block(221, Vec::new(), SemanticTerminatorKindV1::Return),
        ],
        false,
    );
    let body_abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(243)),
        SemanticLayoutIdentityV1::from_sha256(bytes(250)),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        0,
        Vec::new(),
        neutral_plain_direct_abi_value_v1(result_ty),
    )
    .unwrap();
    let body = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(244)),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(245)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(246)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(248)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(249)),
        SemanticSourceProvenanceV1::unavailable(),
        body_abi,
        vec![
            local(222, result_ty, SemanticLocalRoleV1::Return),
            local(224, A_ARRAY, SemanticLocalRoleV1::Temporary),
            local(225, A_U32, SemanticLocalRoleV1::Temporary),
        ],
        SemanticBlockIdV1::from_index(0),
        vec![
            block(
                222,
                assertion_ranked_write_statements(1, 2),
                assertion_terminator(typed_constant(A_BOOL, 1, 1), true, 1),
            ),
            block(
                223,
                vec![typed_assignment(
                    0,
                    result_ty,
                    SemanticRvalueKindV1::Aggregate(
                        SemanticAggregateRvalueV1::new(
                            SemanticAggregateKindV1::EnumVariant(0),
                            vec![SemanticOperandV1::Constant(SemanticConstantV1::new(
                                A_UNIT,
                                SemanticConstantValueV1::ZeroSized,
                            ))],
                        )
                        .unwrap(),
                    ),
                )],
                SemanticTerminatorKindV1::Return,
            ),
        ],
    )
    .unwrap();
    assertion_ssa_functions(types, vec![wrapper, body])
}

fn wrapped_literal_assertion() -> fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1 {
    materialize_ranked_fixture_v1(
        wrapped_literal_assertion_ssa(),
        &[ranked_root_input_1d(A_NAME, 247, 64)],
    )
    .unwrap()
}
