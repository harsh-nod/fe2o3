const POLICY_MATH_FUNCTIONS: [SemanticF32MathFunctionV1; 13] = [
    SemanticF32MathFunctionV1::Sqrt,
    SemanticF32MathFunctionV1::FusedMultiplyAdd,
    SemanticF32MathFunctionV1::Floor,
    SemanticF32MathFunctionV1::Ceil,
    SemanticF32MathFunctionV1::Truncate,
    SemanticF32MathFunctionV1::RoundTiesEven,
    SemanticF32MathFunctionV1::Sin,
    SemanticF32MathFunctionV1::Cos,
    SemanticF32MathFunctionV1::Exp,
    SemanticF32MathFunctionV1::Exp2,
    SemanticF32MathFunctionV1::Ln,
    SemanticF32MathFunctionV1::Log2,
    SemanticF32MathFunctionV1::Log10,
];

fn policy_math_contract(
    function: SemanticF32MathFunctionV1,
) -> SemanticNumericalPolicyMathContractV1 {
    SemanticNumericalPolicyMathContractV1::new(
        SemanticNumericalPolicyMathTypesV1::new([4, 5, 6, 7, 8, 3, 9].map(SemanticTypeIdV1)),
        SemanticTypeIdentityV1(identity(14)),
        SemanticTypeIdentityV1(identity(15)),
        function,
        SemanticNumericalModeV1::StrictIeee,
        function.required_implementation(),
        numerical_contract().provenance(),
        SemanticFunctionIdentityV1(identity(200)),
    )
    .unwrap()
}

fn policy_math_abi(function: SemanticF32MathFunctionV1) -> SemanticFunctionAbiV1 {
    let contract = policy_math_contract(function);
    let inputs = contract.signature().arguments().collect::<Vec<_>>();
    let attributes = |reference| {
        SemanticAbiValueAttributesV1::new(
            if reference {
                SemanticAbiRegularAttributesV1::new(
                    true,
                    Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
                    true,
                    true,
                    false,
                    true,
                )
            } else {
                SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true)
            },
            SemanticAbiExtensionV1::None,
            if reference { 16 } else { 0 },
            reference.then_some(8),
        )
        .unwrap()
    };
    let arguments = inputs
        .iter()
        .enumerate()
        .map(|(index, ty)| {
            SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                *ty,
                SemanticAbiPassModeV1::Direct(attributes(index == 0)),
            ))
        })
        .collect();
    let ownership = inputs
        .iter()
        .enumerate()
        .map(|(index, _)| {
            if index == 0 {
                SemanticSourceArgumentOwnershipV1::SharedBorrow
            } else {
                SemanticSourceArgumentOwnershipV1::ByValue
            }
        })
        .collect();
    SemanticFunctionAbiV1::from_rustc_with_source_signature(
        SemanticAbiIdentityV1(identity(201)),
        SemanticLayoutIdentityV1(identity(202)),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        inputs.len() as u32,
        inputs,
        contract.types().element,
        arguments,
        SemanticAbiValueV1::new(
            contract.types().element,
            SemanticAbiPassModeV1::Direct(attributes(false)),
        ),
    )
    .unwrap()
    .with_source_argument_ownership(ownership)
    .unwrap()
}

fn policy_math_request(function: SemanticF32MathFunctionV1) -> InertSemanticMirRequestV1 {
    let mut request = numerical_request();
    let contract = policy_math_contract(function);
    let ids = contract.types();
    let source = SemanticSourceProvenanceV1::unavailable();
    let mut types = request.types.to_vec();
    let reference = |tag, pointee, size, alignment| {
        let mut ty = request.types[2].clone();
        ty.identity = SemanticTypeIdentityV1(identity(tag));
        ty.layout_identity = SemanticLayoutIdentityV1(identity(tag));
        let SemanticTypeShapeV1::Pointer(pointer) = &mut ty.shape else {
            unreachable!()
        };
        pointer.pointee = pointee;
        ty.abi_properties = ty.abi_properties.with_scalar_pointee_info(
            Some(
                SemanticAbiPointeeInfoV1::new(
                    SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                    size,
                    alignment,
                )
                .unwrap(),
            ),
            None,
        );
        ty
    };
    let zst = |tag| {
        let mut ty = request.types[3].clone();
        ty.identity = SemanticTypeIdentityV1(identity(tag));
        ty.layout_identity = SemanticLayoutIdentityV1(identity(tag));
        ty
    };
    types.push(reference(144, ids.bound, 16, 8));
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1(identity(145)),
        SemanticLayoutIdentityV1(identity(145)),
        SemanticTypeLayoutV1::aggregate(
            Some(16),
            8,
            SemanticAggregateLayoutV1::new(vec![0, 8, 16], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(
            SemanticAggregateTypeV1::new(vec![
                ids.math_reference,
                ids.policy_reference,
                SemanticTypeIdV1(10),
            ])
            .unwrap(),
        ),
    ));
    types.push(reference(146, ids.math, 0, 1));
    types.push(zst(147));
    types.push(reference(148, ids.capability, 0, 1));
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1(identity(149)),
        SemanticLayoutIdentityV1(identity(149)),
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
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 32 }),
    ));
    types.push(zst(150));
    request.types = types.into_boxed_slice();
    let mut callables = request.callables.to_vec();
    callables.push(SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            contract.source_identity(),
            SemanticItemDefinitionIdentityV1(identity(200)),
            SemanticMonomorphizationIdentityV1(identity(200)),
            SemanticGenericTypeArgumentsIdentityV1(identity(200)),
            SemanticConstGenericArgumentsIdentityV1(identity(200)),
            source,
            policy_math_abi(function),
        ),
        operation: SemanticCompilerIntrinsicOperationV1::PolicyMathF32 { contract },
        operation_identity: SemanticCompilerIntrinsicIdentityV1(identity(200)),
    });
    request.callables = callables.into_boxed_slice();
    let mut locals = request.functions[0].locals.to_vec();
    for (index, ty) in [
        ids.bound_reference,
        ids.element,
        ids.math,
        ids.math_reference,
        ids.policy_reference,
        ids.bound,
        SemanticTypeIdV1(10),
    ]
    .into_iter()
    .enumerate()
    {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1(identity(160 + index as u8)),
            ty,
            SemanticLocalRoleV1::Temporary,
            source,
        ));
    }
    request.functions[0].locals = locals.into_boxed_slice();
    let place = |local: u32| {
        SemanticPlaceV1::new(
            SemanticLocalIdV1(local),
            vec![],
            request.functions[0].locals[local as usize].ty(),
        )
        .unwrap()
    };
    let assign = |local, kind| {
        SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(local),
                SemanticRvalueV1::new(place(local).ty(), kind),
            )),
        )
    };
    let zero = |ty| {
        SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
            ty,
            SemanticConstantValueV1::ZeroSized,
        )))
    };
    let borrow = |local| SemanticRvalueKindV1::Borrow {
        kind: SemanticBorrowKindV1::Shared,
        place: place(local),
    };
    // Retain both reference fields as ordinary MIR construction, not a new
    // terminal or a claim that authenticated Rust/SSA custody has been proven.
    let statements = vec![
        assign(6, zero(ids.math)),
        assign(7, borrow(6)),
        assign(8, borrow(3)),
        assign(10, zero(SemanticTypeIdV1(10))),
        assign(
            9,
            SemanticRvalueKindV1::aggregate(
                SemanticAggregateKindV1::Aggregate,
                [7, 8, 10]
                    .map(|local| SemanticOperandV1::Copy(place(local)))
                    .to_vec(),
            )
            .unwrap(),
        ),
        assign(4, borrow(9)),
    ];
    let mut arguments = vec![SemanticOperandV1::Copy(place(4))];
    arguments.extend(std::iter::repeat_n(
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            ids.element,
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(0x3f800000, 4).unwrap()),
        )),
        function.arity(),
    ));
    let destination = place(5);
    let mut blocks = request.functions[0].blocks.to_vec();
    blocks[2] = SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1(identity(132)),
        source,
        statements,
        SemanticTerminatorV1::new(
            source,
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    SemanticCallableIdV1(3),
                    arguments,
                    Some(SemanticCallDestinationV1::new(
                        destination,
                        SemanticControlFlowEdgeV1::new(
                            SemanticEdgeRoleV1::CallReturn,
                            SemanticBlockIdV1(3),
                        ),
                    )),
                    SemanticUnwindActionV1::Unreachable,
                )
                .unwrap(),
            ),
        ),
    )
    .unwrap();
    blocks.push(
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1(identity(133)),
            source,
            vec![],
            SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
        )
        .unwrap(),
    );
    request.functions[0].blocks = blocks.into_boxed_slice();
    request
}

#[test]
fn policy_math_all_functions_retain_exact_contract_and_round_trip_v19() {
    for function in POLICY_MATH_FUNCTIONS {
        let contract = policy_math_contract(function);
        assert_eq!(
            contract.types().all(),
            [4, 5, 6, 7, 8, 3, 9].map(SemanticTypeIdV1)
        );
        assert_eq!(
            contract.signature().arguments().count(),
            function.arity() + 1
        );
        assert_eq!(contract.signature().output(), contract.types().element);
        assert_eq!(
            contract.numerical_requirements(),
            (
                SemanticNumericalModeV1::StrictIeee,
                function.required_implementation()
            )
        );
        assert_eq!(contract.obligations().bits(), 0x10001);
        let operation = SemanticCompilerIntrinsicOperationV1::PolicyMathF32 { contract };
        let bytes = compiler_intrinsic_round_trip(operation, SemanticMirWireVersionV1::V19);
        assert_eq!(bytes.len(), 328);
        assert_eq!(bytes[0], 79);
        assert_eq!(&bytes[29..61], contract.policy().as_bytes());
        assert_eq!(&bytes[61..93], contract.kernel_brand().as_bytes());
        assert_eq!(&bytes[296..328], contract.source_identity().as_bytes());
        let request = policy_math_request(function);
        assert!(compiler_intrinsic_signature_matches(
            &request,
            operation,
            &policy_math_abi(function)
        ));
        let admitted = request
            .admit_current_production(SemanticMirLimitsV1::default())
            .unwrap();
        assert_eq!(admitted.wire_version(), SemanticMirWireVersionV1::V19);
        let decoded = AdmittedInertSemanticMirV1::decode_exact_v19_canonical(
            admitted.canonical_encoding(),
            SemanticMirLimitsV1::default(),
        )
        .unwrap();
        assert_eq!(decoded.canonical_encoding(), admitted.canonical_encoding());
        let decoded = AdmittedInertSemanticMirV1::decode_current_production_canonical(
            admitted.canonical_encoding(),
            SemanticMirLimitsV1::default(),
        )
        .unwrap();
        assert_eq!(decoded.canonical_encoding(), admitted.canonical_encoding());
    }
}

#[test]
fn policy_math_requires_v19_and_preserves_prior_v18_bytes() {
    let operation = SemanticCompilerIntrinsicOperationV1::PolicyMathF32 {
        contract: policy_math_contract(SemanticF32MathFunctionV1::Sqrt),
    };
    for version in [
        SemanticMirWireVersionV1::V5,
        SemanticMirWireVersionV1::V17,
        SemanticMirWireVersionV1::V18,
    ] {
        let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
        assert_eq!(
            encode_compiler_intrinsic_operation(&mut writer, operation, version),
            Err(SemanticMirErrorV1::WireVersionCannotRepresent {
                requested: version,
                required: SemanticMirWireVersionV1::V19
            })
        );
        assert!(writer.finish().is_empty());
    }
    assert!(matches!(
        policy_math_request(SemanticF32MathFunctionV1::Sqrt)
            .admit_exact_v18(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::WireVersionCannotRepresent {
            required: SemanticMirWireVersionV1::V19,
            ..
        })
    ));
    let old = numerical_request()
        .admit_exact_v18(SemanticMirLimitsV1::default())
        .unwrap();
    let current = numerical_request()
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
    assert_eq!(old.canonical_encoding(), current.canonical_encoding());
    for legacy in [
        SemanticCompilerIntrinsicOperationV1::ExecutionCapability {
            contract: numerical_contract(),
        },
        SemanticCompilerIntrinsicOperationV1::MathF32 {
            context: SemanticTypeIdV1(7),
            function: SemanticF32MathFunctionV1::Sqrt,
        },
    ] {
        assert_eq!(
            compiler_intrinsic_round_trip(legacy, SemanticMirWireVersionV1::V18),
            compiler_intrinsic_round_trip(legacy, SemanticMirWireVersionV1::V19)
        );
    }
    let bare = compiler_intrinsic_round_trip(
        SemanticCompilerIntrinsicOperationV1::MathF32 {
            context: SemanticTypeIdV1(7),
            function: SemanticF32MathFunctionV1::Sqrt,
        },
        SemanticMirWireVersionV1::V18,
    );
    assert_eq!(bare, [30, 7, 0, 0, 0, 0]);
    let mut issuance_v18 = vec![73, 23, 2, 0, 0, 0, 3, 0, 0, 0];
    issuance_v18.extend([14; 32]);
    issuance_v18.extend([1, 2, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0]);
    for tag in 101..=106 {
        issuance_v18.extend([tag; 32]);
    }
    issuance_v18.extend([0, 0, 0, 1, 0, 1, 0]);
    issuance_v18.extend([110; 32]);
    assert_eq!(issuance_v18.len(), 286);
    assert_eq!(
        compiler_intrinsic_round_trip(
            SemanticCompilerIntrinsicOperationV1::ExecutionCapability {
                contract: numerical_contract()
            },
            SemanticMirWireVersionV1::V18
        ),
        issuance_v18
    );
}

#[test]
fn policy_math_mixed_records_preserve_exact_cursors_and_closed_function_requirements() {
    let issuance = SemanticCompilerIntrinsicOperationV1::ExecutionCapability {
        contract: numerical_contract(),
    };
    let sqrt = SemanticCompilerIntrinsicOperationV1::PolicyMathF32 {
        contract: policy_math_contract(SemanticF32MathFunctionV1::Sqrt),
    };
    let exp = SemanticCompilerIntrinsicOperationV1::PolicyMathF32 {
        contract: policy_math_contract(SemanticF32MathFunctionV1::Exp),
    };
    let bare = SemanticCompilerIntrinsicOperationV1::MathF32 {
        context: SemanticTypeIdV1(7),
        function: SemanticF32MathFunctionV1::Floor,
    };
    let operations = [issuance, sqrt, bare, exp, issuance];
    let records = operations
        .map(|operation| compiler_intrinsic_round_trip(operation, SemanticMirWireVersionV1::V19));
    assert_eq!(
        records.each_ref().map(|record| record.len()),
        [286, 328, 6, 328, 286]
    );
    let bytes = records.concat();
    let mut decoder = CanonicalDecoderV1::new(&bytes, SemanticMirLimitsV1::default());
    decoder.wire_version = SemanticMirWireVersionV1::V19;
    for operation in operations {
        assert_eq!(decoder.compiler_intrinsic().unwrap(), operation);
    }
    decoder.finish().unwrap();
    for function in POLICY_MATH_FUNCTIONS {
        let required = function.required_implementation();
        let expected = match function {
            SemanticF32MathFunctionV1::Sqrt => {
                SemanticF32MathImplementationV1::IeeeSqrtRoundTiesEvenIgnoreExceptionsV1
            }
            SemanticF32MathFunctionV1::FusedMultiplyAdd
            | SemanticF32MathFunctionV1::Floor
            | SemanticF32MathFunctionV1::Ceil
            | SemanticF32MathFunctionV1::Truncate
            | SemanticF32MathFunctionV1::RoundTiesEven => {
                SemanticF32MathImplementationV1::ConstrainedLlvm
            }
            _ => SemanticF32MathImplementationV1::OcmlAbiV1,
        };
        assert_eq!(required, expected);
        for implementation in [
            SemanticF32MathImplementationV1::ConstrainedLlvm,
            SemanticF32MathImplementationV1::OcmlAbiV1,
            SemanticF32MathImplementationV1::IeeeSqrtRoundTiesEvenIgnoreExceptionsV1,
        ] {
            let contract = policy_math_contract(function);
            assert_eq!(
                SemanticNumericalPolicyMathContractV1::new(
                    contract.types(),
                    contract.policy(),
                    contract.kernel_brand(),
                    function,
                    SemanticNumericalModeV1::StrictIeee,
                    implementation,
                    contract.provenance(),
                    contract.source_identity()
                )
                .is_ok(),
                implementation == required
            );
        }
    }
}

#[test]
fn policy_math_decoder_rejects_truncation_aliases_unknown_modes_and_dropped_requirements() {
    let encoded = compiler_intrinsic_round_trip(
        SemanticCompilerIntrinsicOperationV1::PolicyMathF32 {
            contract: policy_math_contract(SemanticF32MathFunctionV1::Sqrt),
        },
        SemanticMirWireVersionV1::V19,
    );
    let rejects = |bytes: &[u8]| {
        let mut decoder = CanonicalDecoderV1::new(bytes, SemanticMirLimitsV1::default());
        decoder.wire_version = SemanticMirWireVersionV1::V19;
        assert!(decoder.compiler_intrinsic().is_err());
    };
    for length in 0..encoded.len() {
        rejects(&encoded[..length]);
    }
    for (offset, value) in [
        (0, 80),
        (93, 13),
        (94, 1),
        (94, 255),
        (95, 0),
        (95, 1),
        (95, 3),
    ] {
        let mut bad = encoded.clone();
        bad[offset] = value;
        rejects(&bad);
    }
    for start in [29, 61, 100, 132, 164, 196, 228, 260, 296] {
        let mut bad = encoded.clone();
        bad[start..start + 32].fill(0);
        rejects(&bad);
    }
    for index in 0..7 {
        let mut bad = encoded.clone();
        bad[1 + 4 * index..5 + 4 * index].copy_from_slice(&u32::MAX.to_le_bytes());
        rejects(&bad);
    }
    for index in 1..7 {
        let mut bad = encoded.clone();
        bad[1 + 4 * index..5 + 4 * index].copy_from_slice(&4u32.to_le_bytes());
        rejects(&bad);
    }
    for bits in [0u32, 1, 0x10000, 0x10003, u32::MAX] {
        let mut bad = encoded.clone();
        bad[292..296].copy_from_slice(&bits.to_le_bytes());
        rejects(&bad);
    }
    let mut bad = encoded.clone();
    bad.copy_within(29..61, 61);
    rejects(&bad);
    let mut decoder = CanonicalDecoderV1::new(&encoded, SemanticMirLimitsV1::default());
    decoder.wire_version = SemanticMirWireVersionV1::V18;
    assert!(decoder.compiler_intrinsic().is_err());
}

#[test]
fn policy_math_admission_rejects_each_substituted_reference_and_type_graph() {
    let function = SemanticF32MathFunctionV1::Sqrt;
    let contract = policy_math_contract(function);
    let operation = SemanticCompilerIntrinsicOperationV1::PolicyMathF32 { contract };
    let rejects = |request: InertSemanticMirRequestV1| {
        assert!(!compiler_intrinsic_signature_matches(
            &request,
            operation,
            &policy_math_abi(function)
        ));
        assert!(request.admit(SemanticMirLimitsV1::default()).is_err());
    };
    for index in [4, 6, 8] {
        for mutation in 0..4 {
            let mut request = policy_math_request(function);
            let SemanticTypeShapeV1::Pointer(pointer) = &mut request.types[index].shape else {
                unreachable!()
            };
            match mutation {
                0 => pointer.kind = SemanticPointerKindV1::Raw,
                1 => pointer.mutability = SemanticMutabilityV1::Mutable,
                2 => pointer.pointee = SemanticTypeIdV1(10),
                _ => pointer.metadata = SemanticPointerMetadataV1::SliceLength,
            }
            rejects(request);
        }
    }
    for id in contract.types().all() {
        for identity in [
            contract.policy(),
            contract.kernel_brand(),
            SemanticTypeIdentityV1([0; 32]),
        ] {
            let mut request = policy_math_request(function);
            request.types[id.index() as usize].identity = identity;
            rejects(request);
        }
        let mut request = policy_math_request(function);
        request.types[id.index() as usize].layout.uninhabited = true;
        rejects(request);
    }
    for mutation in 0..8 {
        let mut request = policy_math_request(function);
        match mutation {
            0 => {
                request.types[5].shape = SemanticTypeShapeV1::Aggregate(
                    SemanticAggregateTypeV1::new(vec![
                        SemanticTypeIdV1(8),
                        SemanticTypeIdV1(6),
                        SemanticTypeIdV1(10),
                    ])
                    .unwrap(),
                )
            }
            1 => {
                request.types[5].shape = SemanticTypeShapeV1::Aggregate(
                    SemanticAggregateTypeV1::new(vec![SemanticTypeIdV1(6), SemanticTypeIdV1(8)])
                        .unwrap(),
                )
            }
            2 => request.types[10].layout.uninhabited = true,
            3 => request.types[10].layout = SemanticTypeLayoutV1::new(Some(1), 1).unwrap(),
            4 => request.types[7].shape = SemanticTypeShapeV1::Unit,
            5 => request.types[3].layout.alignment_bytes = 2,
            6 => {
                request.types[9].shape =
                    SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                        signed: false,
                        bits: 32,
                    })
            }
            _ => {
                request.types[9].shape =
                    SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 16 });
                request.types[9].layout = SemanticTypeLayoutV1::new(Some(2), 2).unwrap();
            }
        }
        rejects(request);
    }
    // Global nominal uniqueness also covers declarations outside the seven-edge set.
    let mut duplicate = policy_math_request(function);
    duplicate.types[10].identity = duplicate.types[7].identity;
    assert!(duplicate.admit(SemanticMirLimitsV1::default()).is_err());
}

#[test]
fn policy_math_rejects_signature_ownership_and_direct_abi_substitutions() {
    for function in [
        SemanticF32MathFunctionV1::Sqrt,
        SemanticF32MathFunctionV1::FusedMultiplyAdd,
    ] {
        let contract = policy_math_contract(function);
        let operation = SemanticCompilerIntrinsicOperationV1::PolicyMathF32 { contract };
        for mutation in 0..12 {
            let mut request = policy_math_request(function);
            let SemanticCallableDeclV1::CompilerIntrinsic { binding, .. } =
                &mut request.callables[3]
            else {
                unreachable!()
            };
            let abi = &mut binding.abi;
            match mutation {
                0 => abi.canon_abi = SemanticCanonAbiV1::C,
                1 => abi.source_signature.extern_abi = SemanticExternAbiV1::C { unwind: false },
                2 => abi.can_unwind = true,
                3 => abi.source_signature.c_variadic = true,
                4 => abi.fixed_count -= 1,
                5 => abi.source_signature.inputs[0] = contract.types().math_reference,
                6 => abi.source_signature.output = contract.types().bound,
                7 => abi.source_argument_ownership[0] = SemanticSourceArgumentOwnershipV1::ByValue,
                8 => {
                    abi.source_argument_ownership[1] =
                        SemanticSourceArgumentOwnershipV1::SharedBorrow
                }
                9 => abi.arguments[0].value.mode = SemanticAbiPassModeV1::Ignore,
                10 => abi.arguments[1].value.source_ty = contract.types().math,
                _ => abi.return_value.mode = SemanticAbiPassModeV1::Ignore,
            }
            let bad_abi = binding.abi.clone();
            assert!(
                !compiler_intrinsic_signature_matches(&request, operation, &bad_abi),
                "mutation {mutation}"
            );
            assert!(matches!(
                request.admit(SemanticMirLimitsV1::default()),
                Err(SemanticMirErrorV1::InvalidFunctionAbi)
            ));
        }
    }
}

#[test]
fn policy_math_constructor_requires_exact_ids_identities_and_implementation() {
    let original = policy_math_contract(SemanticF32MathFunctionV1::Sqrt);
    for mutation in 0..7 {
        let mut types = original.types();
        let mut policy = original.policy();
        let mut brand = original.kernel_brand();
        let mut source = original.source_identity();
        let mut implementation = original.function().required_implementation();
        match mutation {
            0 => types.math = types.capability,
            1 => types.element = SemanticTypeIdV1(HARD_MAX_TYPES_V1 as u32),
            2 => policy = SemanticTypeIdentityV1([0; 32]),
            3 => brand = SemanticTypeIdentityV1([0; 32]),
            4 => brand = policy,
            5 => source = SemanticFunctionIdentityV1([0; 32]),
            _ => implementation = SemanticF32MathImplementationV1::OcmlAbiV1,
        }
        assert!(
            SemanticNumericalPolicyMathContractV1::new(
                types,
                policy,
                brand,
                original.function(),
                SemanticNumericalModeV1::StrictIeee,
                implementation,
                original.provenance(),
                source
            )
            .is_err()
        );
    }
}

#[test]
fn policy_math_claims_reject_substituted_policy_brand_or_provenance_in_either_order() {
    let original = policy_math_contract(SemanticF32MathFunctionV1::Sqrt);
    for mutation in 0..8 {
        let mut policy = original.policy();
        let mut brand = original.kernel_brand();
        let mut provenance = original.provenance();
        match mutation {
            0 => policy = SemanticTypeIdentityV1(identity(99)),
            1 => brand = SemanticTypeIdentityV1(identity(99)),
            2 => provenance.kernel_binding = SemanticKernelBindingIdentityV1(identity(99)),
            3 => {
                provenance.frontend_unit =
                    SemanticKernelCapabilityFrontendUnitIdentityV1(identity(99))
            }
            4 => provenance.kernel_marker = SemanticTypeIdentityV1(identity(99)),
            5 => {
                provenance.target_brand =
                    SemanticKernelCapabilityTargetBrandIdentityV1(identity(99))
            }
            6 => {
                provenance.launch_brand =
                    SemanticKernelCapabilityLaunchBrandIdentityV1(identity(99))
            }
            _ => provenance.issuance = SemanticKernelCapabilityIssuanceIdentityV1(identity(99)),
        }
        let forged = SemanticNumericalPolicyMathContractV1::new(
            original.types(),
            policy,
            brand,
            original.function(),
            SemanticNumericalModeV1::StrictIeee,
            original.function().required_implementation(),
            provenance,
            original.source_identity(),
        )
        .unwrap();
        for (first, second) in [(original, forged), (forged, original)] {
            let mut claims = IntrinsicCapabilityClaimsV1::default();
            assert!(claims.record_policy_math(first));
            assert!(claims.record_policy_math(first));
            assert!(!claims.record_policy_math(second));
        }
        if mutation != 1 {
            let mut claims = IntrinsicCapabilityClaimsV1::default();
            assert!(claims.record_execution_contract(numerical_contract()));
            assert!(!claims.record_policy_math(forged));
            let mut claims = IntrinsicCapabilityClaimsV1::default();
            assert!(claims.record_policy_math(forged));
            assert!(!claims.record_execution_contract(numerical_contract()));
        }
    }
    let mut claims = IntrinsicCapabilityClaimsV1::default();
    assert!(claims.record_execution_contract(numerical_contract()));
    for function in POLICY_MATH_FUNCTIONS {
        assert!(claims.record_policy_math(policy_math_contract(function)));
    }
}

#[test]
fn policy_math_full_decode_rejects_substituted_source_custody_and_type_ids() {
    let function = SemanticF32MathFunctionV1::Sqrt;
    let encoded = compiler_intrinsic_round_trip(
        SemanticCompilerIntrinsicOperationV1::PolicyMathF32 {
            contract: policy_math_contract(function),
        },
        SemanticMirWireVersionV1::V19,
    );
    let admitted = policy_math_request(function)
        .admit_exact_v19(SemanticMirLimitsV1::default())
        .unwrap();
    let positions = admitted
        .canonical_encoding()
        .windows(encoded.len())
        .enumerate()
        .filter_map(|(position, bytes)| (bytes == encoded).then_some(position))
        .collect::<Vec<_>>();
    assert_eq!(positions.len(), 1);
    let offset = positions[0];
    for relative in [29, 100, 132, 164, 196, 228, 260, 296] {
        let mut bad = admitted.canonical_encoding().to_vec();
        bad[offset + relative..offset + relative + 32].fill(99);
        assert!(
            AdmittedInertSemanticMirV1::decode_exact_v19_canonical(
                &bad,
                SemanticMirLimitsV1::default()
            )
            .is_err()
        );
    }
    for relative in [1, 5, 9, 13, 17, 21, 25, 96] {
        let mut bad = admitted.canonical_encoding().to_vec();
        bad[offset + relative..offset + relative + 4].copy_from_slice(&99u32.to_le_bytes());
        assert!(
            AdmittedInertSemanticMirV1::decode_exact_v19_canonical(
                &bad,
                SemanticMirLimitsV1::default()
            )
            .is_err()
        );
    }
    let mut trailing = admitted.canonical_encoding().to_vec();
    trailing.push(0);
    assert!(
        AdmittedInertSemanticMirV1::decode_exact_v19_canonical(
            &trailing,
            SemanticMirLimitsV1::default()
        )
        .is_err()
    );
    let mut downgraded = admitted.canonical_encoding().to_vec();
    downgraded[MAGIC.len()..MAGIC.len() + 2].copy_from_slice(&18u16.to_le_bytes());
    assert!(
        AdmittedInertSemanticMirV1::decode_exact_v18_canonical(
            &downgraded,
            SemanticMirLimitsV1::default()
        )
        .is_err()
    );
}

#[test]
fn policy_math_fixed_record_obeys_byte_type_callable_and_validation_limits() {
    let function = SemanticF32MathFunctionV1::FusedMultiplyAdd;
    let operation = SemanticCompilerIntrinsicOperationV1::PolicyMathF32 {
        contract: policy_math_contract(function),
    };
    let mut exact = CanonicalWriterV1::new(328);
    encode_compiler_intrinsic_operation(&mut exact, operation, SemanticMirWireVersionV1::V19)
        .unwrap();
    assert_eq!(exact.finish().len(), 328);
    let mut short = CanonicalWriterV1::new(327);
    assert!(matches!(
        encode_compiler_intrinsic_operation(&mut short, operation, SemanticMirWireVersionV1::V19),
        Err(SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::CanonicalBytes,
            ..
        })
    ));
    let request = policy_math_request(function);
    let admitted = request
        .clone()
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    let length = admitted.canonical_encoding().len() as u64;
    for (resource, limit) in [
        (SemanticMirResourceV1::Types, request.types.len() as u64 - 1),
        (
            SemanticMirResourceV1::Callables,
            request.callables.len() as u64 - 1,
        ),
        (SemanticMirResourceV1::ValidationWork, 0),
        (SemanticMirResourceV1::CanonicalBytes, length - 1),
    ] {
        let limits = SemanticMirLimitsV1::default()
            .with_limit(resource, limit)
            .unwrap();
        assert!(matches!(request.clone().admit(limits),
            Err(SemanticMirErrorV1::LimitExceeded { resource: actual, .. }) if actual == resource));
        assert!(
            AdmittedInertSemanticMirV1::decode_exact_v19_canonical(
                admitted.canonical_encoding(),
                limits
            )
            .is_err()
        );
    }
    let exact = SemanticMirLimitsV1::default()
        .with_limit(SemanticMirResourceV1::CanonicalBytes, length)
        .unwrap();
    assert_eq!(
        request.admit(exact).unwrap().canonical_encoding(),
        admitted.canonical_encoding()
    );
    AdmittedInertSemanticMirV1::decode_exact_v19_canonical(admitted.canonical_encoding(), exact)
        .unwrap();
}

#[test]
fn policy_math_call_expansion_preserves_source_ordinals_and_reference_construction() {
    use crate::semantic_direct_call_expansion_v1::{
        SemanticCallExpansionLimitsV1, SemanticCallExpansionV1,
    };
    let contract = policy_math_contract(SemanticF32MathFunctionV1::Sqrt);
    let source = policy_math_request(contract.function())
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    let expansion =
        SemanticCallExpansionV1::try_new(&source, SemanticCallExpansionLimitsV1::default())
            .unwrap();
    expansion.verify_replay(&source).unwrap();
    let root = &expansion.roots()[0];
    assert_eq!(root.root(), contract.provenance().root());
    let consumers = root
        .body()
        .blocks()
        .iter()
        .filter_map(|block| {
            let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                return None;
            };
            let SemanticCallableDeclV1::CompilerIntrinsic {
                operation:
                    SemanticCompilerIntrinsicOperationV1::PolicyMathF32 { contract: retained },
                ..
            } = source.callables()[call.callee().index() as usize]
            else {
                return None;
            };
            assert_eq!(retained, contract);
            assert_eq!(call.callee(), SemanticCallableIdV1(3));
            assert_eq!(
                call.arguments()
                    .iter()
                    .map(SemanticOperandV1::ty)
                    .collect::<Vec<_>>(),
                contract.signature().arguments().collect::<Vec<_>>()
            );
            Some(block)
        })
        .collect::<Vec<_>>();
    assert_eq!(consumers.len(), 1);
    let aggregate = consumers[0]
        .statements()
        .iter()
        .find_map(|statement| {
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                return None;
            };
            if assignment.destination().ty() != contract.types().bound {
                return None;
            }
            let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind() else {
                return None;
            };
            Some(aggregate)
        })
        .expect("ordinary retained wrapper construction");
    assert_eq!(aggregate.kind(), &SemanticAggregateKindV1::Aggregate);
    assert_eq!(
        aggregate
            .operands()
            .iter()
            .map(SemanticOperandV1::ty)
            .collect::<Vec<_>>(),
        [
            contract.types().math_reference,
            contract.types().policy_reference,
            SemanticTypeIdV1(10)
        ]
    );
}
