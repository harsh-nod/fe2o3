use std::collections::BTreeSet;
use std::panic::{AssertUnwindSafe, catch_unwind};

use fe2o3_kernel_ir::*;

fn launch_extent(axis: Axis) -> IntrinsicOperation {
    IntrinsicOperation::new(IntrinsicKind::LaunchExtent { axis }, Type::INDEX)
}

fn invocation_index(kind: IndexKind, axis: Axis) -> IntrinsicOperation {
    IntrinsicOperation::new(IntrinsicKind::InvocationIndex { kind, axis }, Type::INDEX)
}

#[test]
fn schema_codec_is_fixed_width_and_explicitly_payload_blind() {
    let schema = SemanticOperationSchema::v1(SemanticOperationKind::LaunchExtent);
    let encoded = encode_semantic_operation_schema(schema);

    assert_eq!(encoded.len(), SEMANTIC_OPERATION_SCHEMA_BYTES_V1);
    assert_eq!(
        encoded,
        [
            b'F', b'E', b'2', b'O', b'3', b'S', b'O', 0, 1, 0, 4, 0, 2, 0, 0, 0,
        ]
    );
    assert_eq!(decode_semantic_operation_schema(&encoded), Ok(schema));
    assert_eq!(encode_semantic_operation_schema(schema), encoded);

    assert_eq!(
        launch_extent(Axis::X).contract().schema(),
        launch_extent(Axis::Z).contract().schema()
    );
    assert_ne!(
        launch_extent(Axis::X).contract().instance_id(),
        launch_extent(Axis::Z).contract().instance_id()
    );
}

#[test]
fn schema_decoder_rejects_unknown_dispatch_authority() {
    let encoded = encode_semantic_operation_schema(SemanticOperationSchema::v1(
        SemanticOperationKind::LaunchInvocationIndex,
    ));

    let mut unknown_version = encoded;
    unknown_version[8..10].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(
        decode_semantic_operation_schema(&unknown_version),
        Err(SemanticOperationSchemaDecodeError::UnknownVersion(2))
    );

    let mut unknown_family = encoded;
    unknown_family[10] = 0xff;
    assert_eq!(
        decode_semantic_operation_schema(&unknown_family),
        Err(SemanticOperationSchemaDecodeError::UnknownFamily(0xff))
    );

    let mut unknown_operation = encoded;
    unknown_operation[12..14].copy_from_slice(&99_u16.to_le_bytes());
    assert_eq!(
        decode_semantic_operation_schema(&unknown_operation),
        Err(SemanticOperationSchemaDecodeError::UnknownOperation {
            family: SemanticOperationFamily::Launch,
            opcode: 99,
        })
    );

    let mut unimplemented_family = encoded;
    unimplemented_family[10] = 5;
    assert_eq!(
        decode_semantic_operation_schema(&unimplemented_family),
        Err(SemanticOperationSchemaDecodeError::UnknownOperation {
            family: SemanticOperationFamily::Matrix,
            opcode: 1,
        })
    );
}

#[test]
fn schema_decoder_rejects_malformed_encodings() {
    let encoded = encode_semantic_operation_schema(SemanticOperationSchema::v1(
        SemanticOperationKind::LaunchExtent,
    ));

    assert_eq!(
        decode_semantic_operation_schema(&encoded[..15]),
        Err(SemanticOperationSchemaDecodeError::Truncated { actual: 15 })
    );

    let mut trailing = encoded.to_vec();
    trailing.push(0);
    assert_eq!(
        decode_semantic_operation_schema(&trailing),
        Err(SemanticOperationSchemaDecodeError::TrailingBytes { actual: 17 })
    );

    let mut invalid_magic = encoded;
    invalid_magic[0] ^= 0xff;
    assert_eq!(
        decode_semantic_operation_schema(&invalid_magic),
        Err(SemanticOperationSchemaDecodeError::InvalidMagic)
    );

    for offset in [11, 14, 15] {
        let mut reserved = encoded;
        reserved[offset] = 1;
        assert_eq!(
            decode_semantic_operation_schema(&reserved),
            Err(SemanticOperationSchemaDecodeError::ReservedNonZero { offset })
        );
    }
}

#[test]
fn full_instance_identity_separates_every_launch_payload() {
    let axes = [Axis::X, Axis::Y, Axis::Z];
    let index_kinds = [
        IndexKind::Global,
        IndexKind::Workgroup,
        IndexKind::Local,
        IndexKind::WorkgroupSize,
        IndexKind::WorkgroupCount,
    ];
    let mut instances = BTreeSet::new();
    let mut encodings = BTreeSet::new();

    for kind in index_kinds {
        for axis in axes {
            let id = invocation_index(kind, axis).contract().instance_id();
            assert_eq!(
                id.schema(),
                SemanticOperationSchema::v1(SemanticOperationKind::LaunchInvocationIndex)
            );
            assert!(instances.insert(id), "aliased instance {kind:?} {axis:?}");
            let encoded = encode_semantic_operation_instance_id(id);
            assert!(encodings.insert(encoded.clone()));
            assert_eq!(decode_semantic_operation_instance_id(&encoded), Ok(id));
        }
    }

    for axis in axes {
        let id = launch_extent(axis).contract().instance_id();
        assert_eq!(
            id.schema(),
            SemanticOperationSchema::v1(SemanticOperationKind::LaunchExtent)
        );
        assert!(instances.insert(id), "aliased launch extent {axis:?}");
        let encoded = encode_semantic_operation_instance_id(id);
        assert!(encodings.insert(encoded.clone()));
        assert_eq!(decode_semantic_operation_instance_id(&encoded), Ok(id));
    }

    assert_eq!(instances.len(), 18);
    assert_eq!(encodings.len(), 18);
}

#[test]
fn instance_codec_has_canonical_payload_bytes() {
    let id =
        SemanticOperationInstanceId::launch_invocation_index(IndexKind::WorkgroupSize, Axis::Z);
    let encoded = encode_semantic_operation_instance_id(id);

    assert_eq!(
        encoded,
        [
            b'F', b'E', b'2', b'O', b'3', b'S', b'I', 0, 1, 0, 4, 0, 1, 0, 2, 0, 0, 0, 0, 0, 4, 3,
        ]
    );
    assert_eq!(decode_semantic_operation_instance_id(&encoded), Ok(id));
    assert_eq!(encode_semantic_operation_instance_id(id), encoded);
}

#[test]
fn instance_decoder_rejects_unknown_and_malformed_payloads() {
    let extent = SemanticOperationInstanceId::launch_extent(Axis::X);
    let encoded = encode_semantic_operation_instance_id(extent);

    let mut invalid_magic = encoded.clone();
    invalid_magic[0] ^= 0xff;
    assert_eq!(
        decode_semantic_operation_instance_id(&invalid_magic),
        Err(SemanticOperationInstanceDecodeError::InvalidMagic)
    );

    let mut unknown_version = encoded.clone();
    unknown_version[8..10].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(
        decode_semantic_operation_instance_id(&unknown_version),
        Err(SemanticOperationInstanceDecodeError::UnknownVersion(2))
    );

    let mut unknown_family = encoded.clone();
    unknown_family[10] = 0xff;
    assert_eq!(
        decode_semantic_operation_instance_id(&unknown_family),
        Err(SemanticOperationInstanceDecodeError::UnknownFamily(0xff))
    );

    let mut unknown_operation = encoded.clone();
    unknown_operation[12..14].copy_from_slice(&99_u16.to_le_bytes());
    assert_eq!(
        decode_semantic_operation_instance_id(&unknown_operation),
        Err(SemanticOperationInstanceDecodeError::UnknownOperation {
            family: SemanticOperationFamily::Launch,
            opcode: 99,
        })
    );

    let mut flags = encoded.clone();
    flags[11] = 1;
    assert_eq!(
        decode_semantic_operation_instance_id(&flags),
        Err(SemanticOperationInstanceDecodeError::UnsupportedFlags(1))
    );

    let mut reserved = encoded.clone();
    reserved[18] = 1;
    assert_eq!(
        decode_semantic_operation_instance_id(&reserved),
        Err(SemanticOperationInstanceDecodeError::ReservedNonZero { offset: 18 })
    );

    let mut bad_length = encoded.clone();
    bad_length[14..16].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(
        decode_semantic_operation_instance_id(&bad_length),
        Err(SemanticOperationInstanceDecodeError::InvalidPayloadLength {
            kind: SemanticOperationKind::LaunchExtent,
            actual: 2,
            expected: 1,
        })
    );

    let mut oversized_payload = encoded.clone();
    oversized_payload[14..16].copy_from_slice(
        &((MAX_SEMANTIC_OPERATION_INSTANCE_PAYLOAD_BYTES_V1 + 1) as u16).to_le_bytes(),
    );
    assert_eq!(
        decode_semantic_operation_instance_id(&oversized_payload),
        Err(SemanticOperationInstanceDecodeError::PayloadLimitExceeded {
            actual: MAX_SEMANTIC_OPERATION_INSTANCE_PAYLOAD_BYTES_V1 + 1,
            max: MAX_SEMANTIC_OPERATION_INSTANCE_PAYLOAD_BYTES_V1,
        })
    );

    let mut bad_axis = encoded.clone();
    bad_axis[SEMANTIC_OPERATION_INSTANCE_HEADER_BYTES_V1] = 0xff;
    assert_eq!(
        decode_semantic_operation_instance_id(&bad_axis),
        Err(SemanticOperationInstanceDecodeError::UnknownPayloadTag {
            field: "axis",
            tag: 0xff,
        })
    );

    assert_eq!(
        decode_semantic_operation_instance_id(&encoded[..20]),
        Err(SemanticOperationInstanceDecodeError::Truncated {
            actual: 20,
            expected: 21,
        })
    );
    let mut trailing = encoded;
    trailing.push(0);
    assert_eq!(
        decode_semantic_operation_instance_id(&trailing),
        Err(SemanticOperationInstanceDecodeError::TrailingBytes {
            actual: 22,
            expected: 21,
        })
    );

    let invocation =
        SemanticOperationInstanceId::launch_invocation_index(IndexKind::Global, Axis::Y);
    let mut bad_index_kind = encode_semantic_operation_instance_id(invocation);
    bad_index_kind[SEMANTIC_OPERATION_INSTANCE_HEADER_BYTES_V1] = 0xff;
    assert_eq!(
        decode_semantic_operation_instance_id(&bad_index_kind),
        Err(SemanticOperationInstanceDecodeError::UnknownPayloadTag {
            field: "index kind",
            tag: 0xff,
        })
    );
}

#[test]
fn existing_launch_intrinsic_exposes_a_full_target_neutral_contract() {
    let intrinsic = launch_extent(Axis::Y);
    let contract = intrinsic.contract();

    assert_eq!(
        contract.schema(),
        SemanticOperationSchema::v1(SemanticOperationKind::LaunchExtent)
    );
    assert_eq!(
        contract.instance_id(),
        SemanticOperationInstanceId::launch_extent(Axis::Y)
    );
    assert_eq!(contract.operand_count, 0);
    assert_eq!(contract.result_types, vec![Type::INDEX]);
    assert_eq!(contract.memory_effects, Vec::<MemoryEffect>::new());
    assert_eq!(contract.required_capabilities, BTreeSet::new());

    let result = ValueDef::new(ValueId(7), Type::INDEX);
    assert!(
        intrinsic
            .verify(SemanticOperationVerificationContext {
                operands: &[],
                results: &[result],
                operand_types: &[],
            })
            .is_empty()
    );
}

#[test]
fn operation_operands_are_extracted_independently_of_the_contract() {
    let intrinsic = launch_extent(Axis::X);
    let operation = Operation::effect_free(
        ValueDef::new(ValueId(7), Type::INDEX),
        OperationKind::Intrinsic(intrinsic.clone()),
    );
    let extracted = operation.kind.operands();

    assert!(extracted.is_empty());
    assert_eq!(intrinsic.contract().operand_count, 0);
    assert!(
        intrinsic
            .verify(SemanticOperationVerificationContext {
                operands: &extracted,
                results: &operation.results,
                operand_types: &[],
            })
            .is_empty()
    );

    let issues = intrinsic.verify(SemanticOperationVerificationContext {
        operands: &[ValueId(99)],
        results: &operation.results,
        operand_types: &[Some(Type::INDEX)],
    });
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].kind, SemanticOperationIssueKind::InvalidStructure);
}

#[test]
fn canonical_result_constraints_ignore_malformed_declared_types() {
    let intrinsic =
        IntrinsicOperation::new(IntrinsicKind::LaunchExtent { axis: Axis::X }, Type::F32);
    let contract = intrinsic.contract();
    assert_eq!(contract.result_types, vec![Type::INDEX]);

    let malformed_result = ValueDef::new(ValueId(9), Type::F32);
    let issues = intrinsic.verify(SemanticOperationVerificationContext {
        operands: &[],
        results: &[malformed_result],
        operand_types: &[],
    });
    assert_eq!(issues.len(), 2);
    assert!(
        issues
            .iter()
            .all(|issue| issue.kind == SemanticOperationIssueKind::TypeMismatch)
    );

    let canonical_result = ValueDef::new(ValueId(10), Type::INDEX);
    let issues = intrinsic.verify(SemanticOperationVerificationContext {
        operands: &[],
        results: &[canonical_result],
        operand_types: &[],
    });
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].kind, SemanticOperationIssueKind::TypeMismatch);
}

#[test]
fn semantic_decoders_never_panic_on_bounded_noise() {
    let mut state = 0x6c8e_9cf5_7093_2bd1_u64;
    for length in 0..=64 {
        for _ in 0..64 {
            let mut bytes = vec![0; length];
            for byte in &mut bytes {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                *byte = state as u8;
            }
            assert!(
                catch_unwind(AssertUnwindSafe(|| decode_semantic_operation_schema(
                    &bytes
                )))
                .is_ok()
            );
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    decode_semantic_operation_instance_id(&bytes)
                }))
                .is_ok()
            );
        }
    }
}
