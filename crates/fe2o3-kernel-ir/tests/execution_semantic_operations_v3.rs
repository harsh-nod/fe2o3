use fe2o3_kernel_ir::*;

fn payloads() -> [SemanticExecutionInstancePayloadV3; 6] {
    use SemanticExecutionInstancePayloadV3::*;
    [
        ContextIssue,
        WorkgroupDerive,
        ScopeEnd { discard_count: 2 },
        MaskedTileLoadU32 {
            lanes: 64,
            elements: 4,
        },
        TileIntoFragmentU32 {
            lanes: 64,
            elements: 4,
        },
        FragmentIntoPartsU32 {
            lanes: 64,
            elements: 4,
        },
    ]
}

fn operations() -> [ExecutionOperationV15; 6] {
    use ExecutionOperationV15::*;
    [
        ContextIssue,
        WorkgroupDerive {
            context: ValueId(0),
        },
        ScopeEnd {
            workgroup: ValueId(1),
            discarded: vec![ValueId(2), ValueId(3)],
        },
        MaskedTileLoadU32 {
            workgroup: ValueId(1),
            input: ValueId(2),
            base: ValueId(3),
            lanes: 64,
            elements: 4,
        },
        TileIntoFragmentU32 {
            tile: ValueId(2),
            lanes: 64,
            elements: 4,
        },
        FragmentIntoPartsU32 {
            fragment: ValueId(3),
            lanes: 64,
            elements: 4,
        },
    ]
}

#[test]
fn execution_so3_exact_framing_and_distinct_instances() {
    for (index, (payload, operation)) in payloads().into_iter().zip(operations()).enumerate() {
        let id = SemanticOperationInstanceId::execution_v3(payload).unwrap();
        assert_eq!(operation.semantic_instance_id_v3().unwrap(), id);
        let expected_payload: &[u8] = match index {
            0 | 1 => &[],
            2 => &[2, 0, 0, 0],
            _ => &[64, 0, 4, 0],
        };
        let opcode = (index + 1) as u8;
        let mut expected = vec![
            b'F',
            b'E',
            b'2',
            b'O',
            b'3',
            b'S',
            b'I',
            0,
            3,
            0,
            6,
            0,
            opcode,
            0,
            expected_payload.len() as u8,
            0,
            0,
            0,
            0,
            0,
        ];
        expected.extend_from_slice(expected_payload);
        assert_eq!(encode_semantic_operation_instance_id(id), expected);
        assert_eq!(decode_semantic_operation_instance_id(&expected), Ok(id));
        let schema = encode_semantic_operation_schema(id.schema());
        assert_eq!(
            schema,
            [
                b'F', b'E', b'2', b'O', b'3', b'S', b'O', 0, 3, 0, 6, 0, opcode, 0, 0, 0
            ]
        );
        assert_eq!(decode_semantic_operation_schema(&schema), Ok(id.schema()));
        assert!(SemanticOperationSchema::v1(payload.kind()).is_err());
    }
}

#[test]
fn execution_so3_decoder_rejects_version_family_and_payload_substitution() {
    for payload in payloads() {
        let id = SemanticOperationInstanceId::execution_v3(payload).unwrap();
        let encoded = encode_semantic_operation_instance_id(id);
        for version in [0_u16, 1, 2, 4, u16::MAX] {
            let mut bytes = encoded.clone();
            bytes[8..10].copy_from_slice(&version.to_le_bytes());
            assert!(decode_semantic_operation_instance_id(&bytes).is_err());
            let mut schema = encode_semantic_operation_schema(id.schema());
            schema[8..10].copy_from_slice(&version.to_le_bytes());
            assert!(decode_semantic_operation_schema(&schema).is_err());
        }
        for offset in [11, 16, 17, 18, 19] {
            let mut bytes = encoded.clone();
            bytes[offset] = 1;
            assert!(decode_semantic_operation_instance_id(&bytes).is_err());
        }
        for opcode in [0_u16, 7, u16::MAX] {
            let mut bytes = encoded.clone();
            bytes[12..14].copy_from_slice(&opcode.to_le_bytes());
            assert!(decode_semantic_operation_instance_id(&bytes).is_err());
        }
        for family in [0_u8, 1, 4, 5, 7, u8::MAX] {
            let mut bytes = encoded.clone();
            bytes[10] = family;
            assert!(decode_semantic_operation_instance_id(&bytes).is_err());
        }
        for length in [1_u16, 3, 5, u16::MAX] {
            let mut bytes = encoded.clone();
            bytes[14..16].copy_from_slice(&length.to_le_bytes());
            assert!(decode_semantic_operation_instance_id(&bytes).is_err());
        }
        for end in 0..encoded.len() {
            assert!(decode_semantic_operation_instance_id(&encoded[..end]).is_err());
        }
        let mut trailing = encoded;
        trailing.push(0);
        assert!(decode_semantic_operation_instance_id(&trailing).is_err());
    }
    let legacy = SemanticOperationInstanceId::launch_extent(Axis::X);
    let mut bytes = encode_semantic_operation_instance_id(legacy);
    bytes[8..10].copy_from_slice(&3_u16.to_le_bytes());
    assert_eq!(
        decode_semantic_operation_instance_id(&bytes),
        Err(SemanticOperationInstanceDecodeError::NonCanonicalVersion { version: 3 })
    );
    let mut schema = encode_semantic_operation_schema(legacy.schema());
    schema[8..10].copy_from_slice(&3_u16.to_le_bytes());
    assert!(matches!(
        decode_semantic_operation_schema(&schema),
        Err(SemanticOperationSchemaDecodeError::NonCanonicalVersion { version: 3, .. })
    ));
}

#[test]
fn execution_so3_bounds_reject_without_descriptor_panics() {
    for (lanes, elements) in [(0, 1), (257, 1), (1, 0), (1, 126), (u16::MAX, u16::MAX)] {
        for payload in [
            SemanticExecutionInstancePayloadV3::MaskedTileLoadU32 { lanes, elements },
            SemanticExecutionInstancePayloadV3::TileIntoFragmentU32 { lanes, elements },
            SemanticExecutionInstancePayloadV3::FragmentIntoPartsU32 { lanes, elements },
        ] {
            assert!(SemanticOperationInstanceId::execution_v3(payload).is_err());
        }
        let operation = ExecutionOperationV15::FragmentIntoPartsU32 {
            fragment: ValueId(0),
            lanes,
            elements,
        };
        assert!(operation.semantic_instance_id_v3().is_err());
        assert!(operation.try_contract_v3().is_err());
        let valid = SemanticOperationInstanceId::execution_v3(
            SemanticExecutionInstancePayloadV3::MaskedTileLoadU32 {
                lanes: 1,
                elements: 1,
            },
        )
        .unwrap();
        let mut bytes = encode_semantic_operation_instance_id(valid);
        bytes[20..22].copy_from_slice(&lanes.to_le_bytes());
        bytes[22..24].copy_from_slice(&elements.to_le_bytes());
        assert!(decode_semantic_operation_instance_id(&bytes).is_err());
    }
    for discard_count in [0, (MAX_VALUE_ARGUMENTS_V1 - 1) as u32] {
        let id = SemanticOperationInstanceId::execution_v3(
            SemanticExecutionInstancePayloadV3::ScopeEnd { discard_count },
        )
        .unwrap();
        assert_eq!(
            decode_semantic_operation_instance_id(&encode_semantic_operation_instance_id(id)),
            Ok(id)
        );
    }
    for discard_count in [MAX_VALUE_ARGUMENTS_V1 as u32, u32::MAX] {
        assert!(
            SemanticOperationInstanceId::execution_v3(
                SemanticExecutionInstancePayloadV3::ScopeEnd { discard_count },
            )
            .is_err()
        );
    }
    for discarded in [
        vec![ValueId(2), ValueId(2)],
        vec![ValueId(3), ValueId(2)],
        vec![ValueId(0); MAX_VALUE_ARGUMENTS_V1],
    ] {
        let operation = ExecutionOperationV15::ScopeEnd {
            workgroup: ValueId(1),
            discarded,
        };
        assert!(operation.semantic_instance_id_v3().is_err());
        assert!(operation.try_contract_v3().is_err());
    }
    let boundary = ExecutionOperationV15::FragmentIntoPartsU32 {
        fragment: ValueId(0),
        lanes: 256,
        elements: 125,
    }
    .try_contract_v3()
    .unwrap();
    assert_eq!(
        boundary.result_types[..125],
        vec![Type::Scalar(ScalarType::U32); 125]
    );
    assert_eq!(boundary.result_types[125..], vec![Type::BOOL; 125]);
}

fn local_issues(
    operation: &ExecutionOperationV15,
    types: &[Type],
    result_types: &[Type],
) -> Vec<SemanticOperationIssue> {
    let mut operands = Vec::new();
    operation
        .try_visit_operands_v1(|id| {
            operands.push(id);
            Ok::<(), std::convert::Infallible>(())
        })
        .unwrap();
    let types: Vec<_> = types.iter().cloned().map(Some).collect();
    let results: Vec<_> = result_types
        .iter()
        .enumerate()
        .map(|(index, ty)| ValueDef::new(ValueId(100 + index as u32), ty.clone()))
        .collect();
    operation.verify_v3(SemanticOperationVerificationContext {
        operands: &operands,
        results: &results,
        operand_types: &types,
    })
}

#[test]
fn execution_so3_local_verifier_checks_exact_profile_roles_and_parts() {
    let context = Type::Execution(ExecutionRoleV15::Context);
    let workgroup = Type::Execution(ExecutionRoleV15::Workgroup);
    let tile = Type::Execution(ExecutionRoleV15::MaskedTileU32 {
        lanes: 64,
        elements: 4,
    });
    let fragment = Type::Execution(ExecutionRoleV15::LaneFragmentU32 {
        lanes: 64,
        elements: 4,
    });
    let slice = Type::slice(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    );
    for (operation, types) in operations().into_iter().zip([
        vec![],
        vec![context],
        vec![workgroup.clone(), tile.clone(), fragment.clone()],
        vec![workgroup, slice, Type::INDEX],
        vec![tile],
        vec![fragment],
    ]) {
        let results = operation.try_contract_v3().unwrap().result_types;
        assert!(local_issues(&operation, &types, &results).is_empty());
    }
    let operation = operations()[3].clone();
    let input = Type::slice(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    );
    let types = [
        Type::Execution(ExecutionRoleV15::Workgroup),
        input,
        Type::INDEX,
    ];
    let results = operation.try_contract_v3().unwrap().result_types;
    assert!(local_issues(&operation, &types, &results).is_empty());
    for (index, replacement) in [
        (0, Type::Execution(ExecutionRoleV15::Context)),
        (
            1,
            Type::slice(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Private,
                AccessMode::ReadOnly,
            ),
        ),
        (
            1,
            Type::slice(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Global,
                AccessMode::ReadWrite,
            ),
        ),
        (
            1,
            Type::slice(
                Type::Scalar(ScalarType::I32),
                AddressSpace::Global,
                AccessMode::ReadOnly,
            ),
        ),
        (2, Type::Scalar(ScalarType::U64)),
    ] {
        let mut wrong = types.clone();
        wrong[index] = replacement;
        assert!(!local_issues(&operation, &wrong, &results).is_empty());
    }
    let parts = operations()[5].clone();
    let input = [Type::Execution(ExecutionRoleV15::LaneFragmentU32 {
        lanes: 64,
        elements: 4,
    })];
    let mut results = parts.try_contract_v3().unwrap().result_types;
    assert!(local_issues(&parts, &input, &results).is_empty());
    results.swap(0, 4);
    assert!(!local_issues(&parts, &input, &results).is_empty());
    let end = ExecutionOperationV15::ScopeEnd {
        workgroup: ValueId(1),
        discarded: vec![],
    };
    assert!(
        !local_issues(
            &end,
            &[Type::Execution(ExecutionRoleV15::MaskedTileU32 {
                lanes: 64,
                elements: 4,
            })],
            &[]
        )
        .is_empty()
    );
}

#[test]
fn execution_so3_ordering_preserves_both_axes_even_for_invalid_raw_ir() {
    for execution in operations()
        .into_iter()
        .chain([ExecutionOperationV15::MaskedTileLoadU32 {
            workgroup: ValueId(0),
            input: ValueId(1),
            base: ValueId(2),
            lanes: 0,
            elements: 0,
        }])
    {
        let load = matches!(execution, ExecutionOperationV15::MaskedTileLoadU32 { .. });
        let operation = Operation::new(vec![], OperationKind::Execution(execution));
        let effects = operation.combined_effect_summary_v12();
        assert!(!effects.is_pure());
        assert!(effects.compiler_ordering().has_ordered_execution());
        assert_eq!(effects.reads(AddressSpace::Global), load);
        assert_eq!(effects.effects().len(), usize::from(load));
    }
    let old = CompilerOrderingEffectSummaryV12::ordered_verification_contract();
    let execution = CompilerOrderingEffectSummaryV12::ordered_execution();
    assert_eq!(old.union(execution), execution.union(old));
    assert_eq!(
        old.union(execution).effect(),
        Some(CompilerOrderingEffectV12::OrderedVerificationContractAndExecution)
    );
    assert!(old.union(execution).has_ordered_verification_contract());
    assert!(old.union(execution).has_ordered_execution());
    assert!(!old.union(execution).is_empty());
}
