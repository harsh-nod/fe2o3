fn borrowed_workgroup_types() -> SemanticWorkgroupEpochProjectionTypesV1 {
    SemanticWorkgroupEpochProjectionTypesV1::new([9, 8, 10, 7].map(SemanticTypeIdV1))
}

fn borrowed_workgroup_attributes(reference: bool, is_return: bool) -> SemanticAbiValueAttributesV1 {
    SemanticAbiValueAttributesV1::new(
        if reference {
            SemanticAbiRegularAttributesV1::new(
                !is_return,
                (!is_return).then_some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
                true,
                !is_return,
                false,
                true,
            )
        } else {
            SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true)
        },
        SemanticAbiExtensionV1::None,
        if reference && !is_return { 16 } else { 0 },
        (reference && !is_return).then_some(8),
    )
    .unwrap()
}

fn borrowed_workgroup_abi(output: SemanticTypeIdV1) -> SemanticFunctionAbiV1 {
    SemanticFunctionAbiV1::from_rustc_with_source_signature(
        SemanticAbiIdentityV1(identity(202)),
        SemanticLayoutIdentityV1(identity(203)),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        1,
        vec![SemanticTypeIdV1(9)],
        output,
        vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            SemanticTypeIdV1(9),
            SemanticAbiPassModeV1::Direct(borrowed_workgroup_attributes(true, false)),
        ))],
        SemanticAbiValueV1::new(
            output,
            SemanticAbiPassModeV1::Direct(borrowed_workgroup_attributes(
                output == SemanticTypeIdV1(10),
                true,
            )),
        ),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::SharedBorrow])
    .unwrap()
}

fn borrowed_workgroup_contract(derive: bool) -> SemanticExecutionCapabilityContractV1 {
    let (operation, input, output, source) = if derive {
        (
            SemanticExecutionCapabilityOperationV1::WorkgroupDerive {
                context: SemanticTypeIdV1(1),
                workgroup: SemanticTypeIdV1(8),
            },
            1,
            8,
            240,
        )
    } else {
        (
            SemanticExecutionCapabilityOperationV1::SubgroupDeriveBorrowed {
                workgroup_reference: SemanticTypeIdV1(9),
                workgroup: SemanticTypeIdV1(8),
                subgroup: SemanticTypeIdV1(12),
                width: 64,
            },
            9,
            12,
            241,
        )
    };
    SemanticExecutionCapabilityContractV1::new(
        operation,
        SemanticExecutionCapabilitySignatureV1::new(
            &[SemanticTypeIdV1(input)],
            SemanticTypeIdV1(output),
        )
        .unwrap(),
        numerical_contract().provenance(),
        SemanticTypeIdentityV1(identity(107)),
        SemanticTypeIdentityV1(identity(108)),
        None,
        SemanticFunctionIdentityV1(identity(source)),
    )
    .unwrap()
}

fn borrowed_workgroup_getter() -> SemanticFunctionDeclV1 {
    let types = borrowed_workgroup_types();
    let source = SemanticSourceProvenanceV1::unavailable();
    let output = SemanticPlaceV1::new(SemanticLocalIdV1(0), vec![], types.epoch_reference).unwrap();
    let epoch = SemanticPlaceV1::new(
        SemanticLocalIdV1(1),
        vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, types.workgroup)
                .unwrap(),
            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(2), types.epoch_type)
                .unwrap(),
        ],
        types.epoch_type,
    )
    .unwrap();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1(identity(200)),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1(identity(200)),
        SemanticMonomorphizationIdentityV1(identity(200)),
        SemanticGenericTypeArgumentsIdentityV1(identity(200)),
        SemanticConstGenericArgumentsIdentityV1(identity(200)),
        source,
        borrowed_workgroup_abi(types.epoch_reference),
        vec![
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1(identity(1)),
                types.epoch_reference,
                SemanticLocalRoleV1::Return,
                source,
            ),
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1(identity(2)),
                types.reference,
                SemanticLocalRoleV1::Argument(0),
                source,
            ),
        ],
        SemanticBlockIdV1(0),
        vec![
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1(identity(1)),
                source,
                vec![SemanticStatementV1::new(
                    source,
                    SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                        output,
                        SemanticRvalueV1::new(
                            types.epoch_reference,
                            SemanticRvalueKindV1::Borrow {
                                kind: SemanticBorrowKindV1::Shared,
                                place: epoch,
                            },
                        ),
                    )),
                )],
                SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

fn borrowed_workgroup_projection(
    body: &SemanticFunctionDeclV1,
) -> SemanticWorkgroupEpochProjectionV1 {
    SemanticWorkgroupEpochProjectionV1::for_defined_function(
        SemanticFunctionIdV1(1),
        body,
        borrowed_workgroup_types(),
        numerical_contract().provenance(),
        SemanticTypeIdentityV1(identity(107)),
        SemanticTypeIdentityV1(identity(108)),
    )
    .unwrap()
}

fn borrowed_workgroup_request(same_owner: bool) -> InertSemanticMirRequestV1 {
    let mut request = numerical_request();
    let source = SemanticSourceProvenanceV1::unavailable();
    let scalar = |bits: u16| {
        SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::integer(false, bits, u64::from(bits / 8)),
            SemanticScalarValidityRangeV1::new(0, (1u128 << bits) - 1),
        )
    };
    let declaration = |index: u8, layout, shape| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1(identity(20 + index)),
            SemanticLayoutIdentityV1(identity(20 + index)),
            layout,
            shape,
        )
    };
    let aggregate = |index, size, align, fields: &[u32], offsets, repr| {
        declaration(
            index,
            SemanticTypeLayoutV1::aggregate_with_backend_repr(
                Some(size),
                align,
                repr,
                false,
                SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(
                SemanticAggregateTypeV1::new(
                    fields.iter().copied().map(SemanticTypeIdV1).collect(),
                )
                .unwrap(),
            ),
        )
    };
    let mut types = request.types.to_vec();
    for (index, bits) in [(4, 32), (5, 64)] {
        types.push(declaration(
            index,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(u64::from(bits / 8)),
                u64::from(bits / 8),
                SemanticBackendReprV1::scalar(scalar(bits)),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits,
            }),
        ));
    }
    types.push(aggregate(
        6,
        0,
        1,
        &[],
        vec![],
        SemanticBackendReprV1::memory(true),
    ));
    types.push(aggregate(
        7,
        0,
        1,
        &[6, 6, 6],
        vec![0, 0, 0],
        SemanticBackendReprV1::memory(true),
    ));
    types.push(aggregate(
        8,
        16,
        8,
        &[5, 5, 7, 6],
        vec![0, 8, 16, 16],
        SemanticBackendReprV1::scalar_pair(scalar(64), scalar(64)),
    ));
    for (index, pointee, size, alignment) in [(9, 8, 16, 8), (10, 7, 0, 1)] {
        let mut reference = request.types[2].clone();
        reference.identity = SemanticTypeIdentityV1(identity(20 + index));
        reference.layout_identity = SemanticLayoutIdentityV1(identity(20 + index));
        let SemanticTypeShapeV1::Pointer(pointer) = &mut reference.shape else {
            unreachable!()
        };
        pointer.pointee = SemanticTypeIdV1(pointee);
        reference.abi_properties = reference.abi_properties.with_scalar_pointee_info(
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
        types.push(reference);
    }
    types.push(aggregate(
        11,
        4,
        4,
        &[4, 6, 6, 6],
        vec![0, 4, 4, 4],
        SemanticBackendReprV1::scalar(scalar(32)),
    ));
    types.push(aggregate(
        12,
        4,
        4,
        &[11, 6, 6],
        vec![0, 4, 4],
        SemanticBackendReprV1::scalar(scalar(32)),
    ));
    request.types = types.into_boxed_slice();

    let getter = borrowed_workgroup_getter();
    let projection = borrowed_workgroup_projection(&getter);
    let getter = getter.with_workgroup_epoch_projection(projection).unwrap();
    let mut wrapper = borrowed_workgroup_getter();
    wrapper.identity = SemanticFunctionIdentityV1(identity(201));
    let place = |local, ty| {
        SemanticPlaceV1::new(SemanticLocalIdV1(local), vec![], SemanticTypeIdV1(ty)).unwrap()
    };
    let call = |callee, arguments, destination, next| {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1(callee),
                arguments,
                Some(SemanticCallDestinationV1::new(
                    destination,
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        SemanticBlockIdV1(next),
                    ),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    };
    let block = |tag, statements, terminator| {
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1(identity(tag)),
            source,
            statements,
            SemanticTerminatorV1::new(source, terminator),
        )
        .unwrap()
    };
    wrapper.blocks = vec![
        block(
            1,
            vec![],
            call(
                1,
                vec![SemanticOperandV1::Copy(place(1, 9))],
                place(0, 10),
                1,
            ),
        ),
        block(2, vec![], SemanticTerminatorKindV1::Return),
    ]
    .into_boxed_slice();
    let mut functions = request.functions.to_vec();
    functions.extend([getter, wrapper]);
    request.functions = functions.into_boxed_slice();
    let mut callables = vec![
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1(0)),
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1(1)),
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1(2)),
    ];
    callables.extend(request.callables[1..].iter().cloned());
    for derive in [true, false] {
        let contract = borrowed_workgroup_contract(derive);
        let abi = if derive {
            let attributes = borrowed_workgroup_attributes(false, false);
            SemanticFunctionAbiV1::from_rustc_with_source_signature(
                SemanticAbiIdentityV1(identity(240)),
                SemanticLayoutIdentityV1(identity(240)),
                SemanticCanonAbiV1::Rust,
                SemanticExternAbiV1::Rust,
                false,
                false,
                1,
                vec![SemanticTypeIdV1(1)],
                SemanticTypeIdV1(8),
                vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                    SemanticTypeIdV1(1),
                    SemanticAbiPassModeV1::Ignore,
                ))],
                SemanticAbiValueV1::new(
                    SemanticTypeIdV1(8),
                    SemanticAbiPassModeV1::Pair {
                        first: attributes,
                        second: attributes,
                    },
                ),
            )
            .unwrap()
            .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue])
            .unwrap()
        } else {
            borrowed_workgroup_abi(SemanticTypeIdV1(12))
        };
        let tag = if derive { 240 } else { 241 };
        callables.push(SemanticCallableDeclV1::CompilerIntrinsic {
            binding: SemanticNonBodyCallableBindingV1::new(
                contract.source_identity(),
                SemanticItemDefinitionIdentityV1(identity(tag)),
                SemanticMonomorphizationIdentityV1(identity(tag)),
                SemanticGenericTypeArgumentsIdentityV1(identity(tag)),
                SemanticConstGenericArgumentsIdentityV1(identity(tag)),
                source,
                abi,
            ),
            operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
            operation_identity: SemanticCompilerIntrinsicIdentityV1(identity(tag)),
        });
    }
    request.callables = callables.into_boxed_slice();
    let mut locals = request.functions[0].locals.to_vec();
    for (index, ty) in [8, 8, 9, 9, 12, 10, 10].into_iter().enumerate() {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1(identity(170 + index as u8)),
            SemanticTypeIdV1(ty),
            SemanticLocalRoleV1::Temporary,
            source,
        ));
    }
    request.functions[0].locals = locals.into_boxed_slice();
    let mut blocks = request.functions[0].blocks[..2].to_vec();
    for block in &mut blocks {
        let SemanticTerminatorKindV1::Call(call) = &mut block.terminator.kind else {
            unreachable!()
        };
        call.callee = SemanticCallableIdV1(call.callee.index() + 2);
    }
    blocks.push(block(
        132,
        vec![],
        call(
            5,
            vec![SemanticOperandV1::Copy(place(1, 1))],
            place(4, 8),
            3,
        ),
    ));
    blocks.push(block(
        133,
        vec![],
        call(
            5,
            vec![SemanticOperandV1::Copy(place(1, 1))],
            place(5, 8),
            4,
        ),
    ));
    let borrow = |destination, owner| {
        SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(destination, 9),
                SemanticRvalueV1::new(
                    SemanticTypeIdV1(9),
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Shared,
                        place: place(owner, 8),
                    },
                ),
            )),
        )
    };
    blocks.push(block(
        134,
        vec![borrow(6, 4), borrow(7, 5)],
        call(
            6,
            vec![SemanticOperandV1::Copy(place(6, 9))],
            place(8, 12),
            5,
        ),
    ));
    blocks.push(block(
        135,
        vec![],
        call(
            2,
            vec![SemanticOperandV1::Copy(place(6, 9))],
            place(9, 10),
            6,
        ),
    ));
    blocks.push(block(
        136,
        vec![],
        call(
            2,
            vec![SemanticOperandV1::Copy(place(
                if same_owner { 6 } else { 7 },
                9,
            ))],
            place(10, 10),
            7,
        ),
    ));
    blocks.push(block(137, vec![], SemanticTerminatorKindV1::Return));
    request.functions[0].blocks = blocks.into_boxed_slice();
    request
}

#[test]
fn workgroup_borrow_v20_retains_live_values_defined_body_and_canonical_roundtrip() {
    let request = borrowed_workgroup_request(true);
    let body = request.functions[1].clone();
    let projection = *body.workgroup_epoch_projection().unwrap();
    assert_eq!(
        projection.types().all(),
        [9, 8, 10, 7].map(SemanticTypeIdV1)
    );
    assert_eq!(projection.receiver_argument(), 0);
    assert_eq!(projection.source_field(), 2);
    assert_eq!(projection.function(), SemanticFunctionIdV1(1));
    let admitted = request
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
    assert_eq!(admitted.wire_version(), SemanticMirWireVersionV1::V20);
    assert_eq!(admitted.functions()[1], body);
    assert_eq!(admitted.types()[8].layout().size_bytes(), Some(16));
    assert_eq!(admitted.types()[12].layout().size_bytes(), Some(4));
    for decode in [
        AdmittedInertSemanticMirV1::decode_exact_v20_canonical,
        AdmittedInertSemanticMirV1::decode_current_production_canonical,
    ] {
        let decoded = decode(
            admitted.canonical_encoding(),
            SemanticMirLimitsV1::default(),
        )
        .unwrap();
        assert_eq!(decoded.canonical_encoding(), admitted.canonical_encoding());
        assert_eq!(
            decoded.functions()[1].workgroup_epoch_projection(),
            Some(&projection)
        );
    }
}

#[test]
fn workgroup_borrow_preserves_legacy_subgroup_and_mir19_policy_bytes() {
    let legacy = SemanticCompilerIntrinsicOperationV1::ExecutionCapability {
        contract: execution_contract(SemanticExecutionCapabilityOperationV1::SubgroupDerive {
            workgroup: SemanticTypeIdV1(8),
            subgroup: SemanticTypeIdV1(12),
            width: 64,
        })
        .unwrap(),
    };
    assert_eq!(
        compiler_intrinsic_round_trip(legacy, SemanticMirWireVersionV1::V17),
        compiler_intrinsic_round_trip(legacy, SemanticMirWireVersionV1::V20)
    );
    for function in POLICY_MATH_FUNCTIONS {
        let operation = SemanticCompilerIntrinsicOperationV1::PolicyMathF32 {
            contract: policy_math_contract(function),
        };
        assert_eq!(
            compiler_intrinsic_round_trip(operation, SemanticMirWireVersionV1::V19),
            compiler_intrinsic_round_trip(operation, SemanticMirWireVersionV1::V20)
        );
        let request = policy_math_request(function);
        let old = request
            .clone()
            .admit_exact_v19(SemanticMirLimitsV1::default())
            .unwrap();
        let current = request
            .admit_current_production(SemanticMirLimitsV1::default())
            .unwrap();
        assert_eq!(old.canonical_encoding(), current.canonical_encoding());
        assert_eq!(current.wire_version(), SemanticMirWireVersionV1::V19);
    }
    let borrowed = SemanticCompilerIntrinsicOperationV1::ExecutionCapability {
        contract: borrowed_workgroup_contract(false),
    };
    let bytes = compiler_intrinsic_round_trip(borrowed, SemanticMirWireVersionV1::V20);
    assert_eq!(&bytes[..2], &[73, 25]);
    assert_eq!(bytes.len(), 324);
    for version in [
        SemanticMirWireVersionV1::V17,
        SemanticMirWireVersionV1::V18,
        SemanticMirWireVersionV1::V19,
    ] {
        let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
        assert!(matches!(
            encode_compiler_intrinsic_operation(&mut writer, borrowed, version),
            Err(SemanticMirErrorV1::WireVersionCannotRepresent {
                required: SemanticMirWireVersionV1::V20,
                ..
            })
        ));
        assert!(writer.finish().is_empty());
        let mut decoder = CanonicalDecoderV1::new(&bytes, SemanticMirLimitsV1::default());
        decoder.wire_version = version;
        assert!(decoder.compiler_intrinsic().is_err());
    }
    assert!(matches!(
        borrowed_workgroup_request(true).admit_exact_v19(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::WireVersionCannotRepresent {
            required: SemanticMirWireVersionV1::V20,
            ..
        })
    ));
}

#[test]
fn workgroup_borrow_rejects_reference_substitution_and_zst_authority_fixtures() {
    for mutation in 0..8 {
        let mut request = borrowed_workgroup_request(true);
        match mutation {
            0 => {
                let SemanticTypeShapeV1::Pointer(pointer) = &mut request.types[9].shape else {
                    unreachable!()
                };
                pointer.pointee = SemanticTypeIdV1(7);
            }
            1 => {
                let SemanticTypeShapeV1::Pointer(pointer) = &mut request.types[9].shape else {
                    unreachable!()
                };
                pointer.mutability = SemanticMutabilityV1::Mutable;
            }
            2 => {
                let SemanticTypeShapeV1::Pointer(pointer) = &mut request.types[10].shape else {
                    unreachable!()
                };
                pointer.kind = SemanticPointerKindV1::Raw;
            }
            3 => {
                request.types[8].layout = SemanticTypeLayoutV1::aggregate(
                    Some(0),
                    1,
                    SemanticAggregateLayoutV1::new(vec![0, 0, 0, 0], vec![]).unwrap(),
                )
                .unwrap()
            }
            4 => {
                request.types[12].layout = SemanticTypeLayoutV1::aggregate(
                    Some(0),
                    1,
                    SemanticAggregateLayoutV1::new(vec![0, 0, 0], vec![]).unwrap(),
                )
                .unwrap()
            }
            5 => {
                request.types[8].shape = SemanticTypeShapeV1::Aggregate(
                    SemanticAggregateTypeV1::new(vec![
                        SemanticTypeIdV1(5),
                        SemanticTypeIdV1(5),
                        SemanticTypeIdV1(6),
                        SemanticTypeIdV1(7),
                    ])
                    .unwrap(),
                )
            }
            6 => request.types[7].layout.uninhabited = true,
            _ => {
                let SemanticCallableDeclV1::CompilerIntrinsic { binding, .. } =
                    &mut request.callables[6]
                else {
                    unreachable!()
                };
                binding.abi.source_argument_ownership[0] =
                    SemanticSourceArgumentOwnershipV1::ByValue;
            }
        }
        assert!(
            request.admit(SemanticMirLimitsV1::default()).is_err(),
            "mutation {mutation}"
        );
    }
    for width in [0, 32, 128] {
        let mut contract = borrowed_workgroup_contract(false);
        let SemanticExecutionCapabilityOperationV1::SubgroupDeriveBorrowed { width: value, .. } =
            &mut contract.operation
        else {
            unreachable!()
        };
        *value = width;
        assert!(!execution_operation_is_well_formed(contract.operation()));
    }
}

#[test]
fn workgroup_epoch_rejects_stale_body_wrong_field_extra_effect_and_function_identity() {
    let valid = borrowed_workgroup_getter();
    let record = borrowed_workgroup_projection(&valid);
    for mutation in 0..7 {
        let mut body = valid.clone();
        match mutation {
            0 => body.identity = SemanticFunctionIdentityV1(identity(199)),
            1 => {
                body.blocks[0].statements = vec![SemanticStatementV1::new(
                    SemanticSourceProvenanceV1::unavailable(),
                    SemanticStatementKindV1::Nop,
                )]
                .into_boxed_slice()
            }
            2 => {
                let SemanticStatementKindV1::Assign(assignment) =
                    &mut body.blocks[0].statements[0].kind
                else {
                    unreachable!()
                };
                let SemanticRvalueKindV1::Borrow { place, .. } = &mut assignment.value.kind else {
                    unreachable!()
                };
                place.projections[1].kind = SemanticProjectionKindV1::Field(3);
            }
            3 => {
                let SemanticStatementKindV1::Assign(assignment) =
                    &mut body.blocks[0].statements[0].kind
                else {
                    unreachable!()
                };
                let SemanticRvalueKindV1::Borrow { kind, .. } = &mut assignment.value.kind else {
                    unreachable!()
                };
                *kind = SemanticBorrowKindV1::Mutable;
            }
            4 => body.abi.source_argument_ownership[0] = SemanticSourceArgumentOwnershipV1::ByValue,
            5 => body.blocks[0].identity = SemanticBlockIdentityV1(identity(99)),
            _ => body.locals[1].role = SemanticLocalRoleV1::Temporary,
        }
        assert!(
            body.with_workgroup_epoch_projection(record).is_err(),
            "mutation {mutation}"
        );
    }
    let attached = valid
        .clone()
        .with_workgroup_epoch_projection(record)
        .unwrap();
    assert_eq!(attached.blocks(), valid.blocks());
    assert_eq!(attached.locals(), valid.locals());
}

#[test]
fn workgroup_epoch_decoder_rejects_truncated_forged_recipe_and_exact_body_commitment() {
    let body = borrowed_workgroup_getter();
    let record = borrowed_workgroup_projection(&body);
    let mut writer = CanonicalWriterV1::new(350);
    record.encode(&mut writer).unwrap();
    let encoded = writer.finish();
    assert_eq!(encoded.len(), 350);
    for length in 0..encoded.len() {
        let mut decoder =
            CanonicalDecoderV1::new(&encoded[..length], SemanticMirLimitsV1::default());
        decoder.wire_version = SemanticMirWireVersionV1::V20;
        assert!(decoder.workgroup_epoch_projection().is_err());
    }
    for (offset, value) in [(0, 1), (345, 1), (346, 3)] {
        let mut bad = encoded.clone();
        bad[offset] = value;
        let mut decoder = CanonicalDecoderV1::new(&bad, SemanticMirLimitsV1::default());
        assert!(decoder.workgroup_epoch_projection().is_err());
    }
    let admitted = borrowed_workgroup_request(true)
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    let offsets = admitted
        .canonical_encoding()
        .windows(encoded.len())
        .enumerate()
        .filter_map(|(index, bytes)| (bytes == encoded).then_some(index))
        .collect::<Vec<_>>();
    assert_eq!(offsets.len(), 1);
    for offset in [1, 5, 37, 69, 73, 77, 81, 85, 89, 281, 313] {
        let mut bad = admitted.canonical_encoding().to_vec();
        bad[offsets[0] + offset] ^= 1;
        assert!(
            AdmittedInertSemanticMirV1::decode_exact_v20_canonical(
                &bad,
                SemanticMirLimitsV1::default()
            )
            .is_err(),
            "offset {offset}"
        );
    }
}

#[test]
fn workgroup_epoch_bounds_roster_bytes_work_and_conflicting_attachments() {
    let request = borrowed_workgroup_request(true);
    let admitted = request
        .clone()
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    for (resource, max) in [
        (SemanticMirResourceV1::Types, 12),
        (SemanticMirResourceV1::Functions, 2),
        (
            SemanticMirResourceV1::CanonicalBytes,
            admitted.canonical_encoding().len() as u64 - 1,
        ),
        (SemanticMirResourceV1::ValidationWork, 0),
    ] {
        let limits = SemanticMirLimitsV1::default()
            .with_limit(resource, max)
            .unwrap();
        assert!(matches!(
            request.clone().admit(limits),
            Err(SemanticMirErrorV1::LimitExceeded { .. })
        ));
        assert!(
            AdmittedInertSemanticMirV1::decode_exact_v20_canonical(
                admitted.canonical_encoding(),
                limits
            )
            .is_err()
        );
    }
    let record = *request.functions[1].workgroup_epoch_projection().unwrap();
    let mut writer = CanonicalWriterV1::new(349);
    assert!(matches!(
        record.encode(&mut writer),
        Err(SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::CanonicalBytes,
            ..
        })
    ));
    let limits = SemanticMirLimitsV1::default()
        .with_limit(SemanticMirResourceV1::ValidationWork, 1)
        .unwrap();
    let mut context = ValidationContextV1 {
        request: &request,
        limits,
        totals: ValidationTotalsV1::default(),
        work: 0,
    };
    assert!(matches!(
        super::super::workgroup_borrow_v1::validate_function_projection(
            &mut context,
            SemanticFunctionIdV1(1),
            &request.functions[1]
        ),
        Err(SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::ValidationWork,
            ..
        })
    ));
    let attached = request.functions[1].clone();
    let same = SemanticDefinedCapabilityContractV1::WorkgroupEpochProjection(record);
    assert_eq!(
        attached
            .clone()
            .with_defined_capability_contract(same)
            .unwrap(),
        attached
    );
    let other = SemanticWorkgroupEpochProjectionV1::for_defined_function(
        SemanticFunctionIdV1(2),
        &attached,
        record.types(),
        record.provenance(),
        record.brand(),
        record.epoch(),
    )
    .unwrap();
    assert!(
        attached
            .with_defined_capability_contract(
                SemanticDefinedCapabilityContractV1::WorkgroupEpochProjection(other)
            )
            .is_err()
    );
}

#[test]
fn workgroup_epoch_recipe_validation_is_independent_of_stale_commitments() {
    for mutation in 0..3 {
        let mut body = borrowed_workgroup_getter();
        let SemanticStatementKindV1::Assign(assignment) = &mut body.blocks[0].statements[0].kind
        else {
            unreachable!()
        };
        let SemanticRvalueKindV1::Borrow { kind, place } = &mut assignment.value.kind else {
            unreachable!()
        };
        match mutation {
            0 => place.projections[1].kind = SemanticProjectionKindV1::Field(3),
            1 => *kind = SemanticBorrowKindV1::Mutable,
            _ => *kind = SemanticBorrowKindV1::Fake,
        }
        assert!(
            SemanticWorkgroupEpochProjectionV1::for_defined_function(
                SemanticFunctionIdV1(1),
                &body,
                borrowed_workgroup_types(),
                numerical_contract().provenance(),
                SemanticTypeIdentityV1(identity(107)),
                SemanticTypeIdentityV1(identity(108))
            )
            .is_err()
        );
    }
}

#[test]
fn workgroup_epoch_checked_expansion_retains_nested_same_and_different_owner_calls() {
    use crate::semantic_direct_call_expansion_v1::{
        SemanticCallExpansionLimitsV1, SemanticCallExpansionV1, SemanticExpandedStatementOriginV1,
    };
    for same_owner in [true, false] {
        let source = borrowed_workgroup_request(same_owner)
            .admit(SemanticMirLimitsV1::default())
            .unwrap();
        let expansion =
            SemanticCallExpansionV1::try_new(&source, SemanticCallExpansionLimitsV1::default())
                .unwrap();
        let common = expansion.defined_capability_bindings(&source).unwrap();
        let bindings = expansion
            .workgroup_epoch_projection_bindings(&source)
            .unwrap();
        assert_eq!(bindings.len(), 2);
        assert_eq!(common.len(), bindings.len());
        let root = &expansion.roots()[0];
        assert!(root.body().defined_capability_contract().is_none());
        assert!(
            source.functions()[1]
                .defined_capability_contract()
                .is_some()
        );
        assert_ne!(bindings[0].callee_instance(), bindings[1].callee_instance());
        assert_ne!(bindings[0].caller_instance(), bindings[1].caller_instance());
        let mut receivers = Vec::new();
        for (binding, shared) in bindings.iter().zip(&common) {
            assert_eq!(binding.binding(), shared);
            assert_eq!(binding.expansion_identity(), expansion.identity());
            assert_eq!(binding.root_identity(), root.identity());
            let mut operand = binding.receiver().clone();
            for _ in 0..4 {
                let (SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = &operand
                else {
                    panic!("retained reference operand")
                };
                let origin = root.local_origins()[place.local().index() as usize];
                if origin.instance().index() == 0 {
                    break;
                }
                let SemanticLocalRoleV1::Argument(argument) = source.functions()
                    [origin.function().index() as usize]
                    .locals()[origin.local().index() as usize]
                    .role()
                else {
                    panic!("wrapper parameter")
                };
                let transfers = root
                    .body()
                    .blocks()
                    .iter()
                    .zip(root.block_origins())
                    .flat_map(|(block, origin)| block.statements().iter().zip(origin.statements()))
                    .filter_map(|(statement, attribution)| {
                        if *attribution
                            != (SemanticExpandedStatementOriginV1::ParameterTransfer {
                                callee: origin.instance(),
                                argument,
                            })
                        {
                            return None;
                        }
                        let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                            panic!("parameter assignment")
                        };
                        assert_eq!(assignment.destination().local(), place.local());
                        let SemanticRvalueKindV1::Use(value) = assignment.value().kind() else {
                            panic!("exact parameter transfer")
                        };
                        Some(value.clone())
                    })
                    .collect::<Vec<_>>();
                assert_eq!(transfers.len(), 1);
                operand = transfers[0].clone();
            }
            let (SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = operand else {
                panic!("root receiver")
            };
            assert_eq!(
                root.local_origins()[place.local().index() as usize]
                    .instance()
                    .index(),
                0
            );
            receivers.push(place.local().index());
        }
        assert_eq!(receivers, if same_owner { vec![6, 6] } else { vec![6, 7] });
        let changed = borrowed_workgroup_request(!same_owner)
            .admit(SemanticMirLimitsV1::default())
            .unwrap();
        assert!(expansion.defined_capability_bindings(&changed).is_err());
    }
}
