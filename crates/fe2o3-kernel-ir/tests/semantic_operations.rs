use std::collections::BTreeSet;
use std::panic::{AssertUnwindSafe, catch_unwind};

use fe2o3_kernel_ir::*;

fn launch_extent() -> IntrinsicOperation {
    IntrinsicOperation::new(IntrinsicKind::LaunchExtent { axis: Axis::X }, Type::INDEX)
}

#[test]
fn semantic_identity_codec_is_fixed_width_and_deterministic() {
    let id = SemanticOperationId::v1(SemanticOperationKind::LaunchExtent);
    let encoded = encode_semantic_operation_id(id);

    assert_eq!(encoded.len(), SEMANTIC_OPERATION_ID_BYTES_V1);
    assert_eq!(
        encoded,
        [
            b'F', b'E', b'2', b'O', b'3', b'S', b'O', 0, 1, 0, 4, 0, 2, 0, 0, 0,
        ]
    );
    assert_eq!(decode_semantic_operation_id(&encoded), Ok(id));
    assert_eq!(encode_semantic_operation_id(id), encoded);
}

#[test]
fn semantic_identity_decoder_rejects_unknown_authority() {
    let encoded = encode_semantic_operation_id(SemanticOperationId::v1(
        SemanticOperationKind::LaunchInvocationIndex,
    ));

    let mut unknown_version = encoded;
    unknown_version[8..10].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(
        decode_semantic_operation_id(&unknown_version),
        Err(SemanticOperationIdDecodeError::UnknownVersion(2))
    );

    let mut unknown_family = encoded;
    unknown_family[10] = 0xff;
    assert_eq!(
        decode_semantic_operation_id(&unknown_family),
        Err(SemanticOperationIdDecodeError::UnknownFamily(0xff))
    );

    let mut unknown_operation = encoded;
    unknown_operation[12..14].copy_from_slice(&99_u16.to_le_bytes());
    assert_eq!(
        decode_semantic_operation_id(&unknown_operation),
        Err(SemanticOperationIdDecodeError::UnknownOperation {
            family: SemanticOperationFamily::Launch,
            opcode: 99,
        })
    );

    let mut unimplemented_family = encoded;
    unimplemented_family[10] = 5;
    assert_eq!(
        decode_semantic_operation_id(&unimplemented_family),
        Err(SemanticOperationIdDecodeError::UnknownOperation {
            family: SemanticOperationFamily::Matrix,
            opcode: 1,
        })
    );
}

#[test]
fn semantic_identity_decoder_rejects_malformed_encodings() {
    let encoded =
        encode_semantic_operation_id(SemanticOperationId::v1(SemanticOperationKind::LaunchExtent));

    assert_eq!(
        decode_semantic_operation_id(&encoded[..15]),
        Err(SemanticOperationIdDecodeError::Truncated { actual: 15 })
    );

    let mut trailing = encoded.to_vec();
    trailing.push(0);
    assert_eq!(
        decode_semantic_operation_id(&trailing),
        Err(SemanticOperationIdDecodeError::TrailingBytes { actual: 17 })
    );

    let mut invalid_magic = encoded;
    invalid_magic[0] ^= 0xff;
    assert_eq!(
        decode_semantic_operation_id(&invalid_magic),
        Err(SemanticOperationIdDecodeError::InvalidMagic)
    );

    for offset in [11, 14, 15] {
        let mut reserved = encoded;
        reserved[offset] = 1;
        assert_eq!(
            decode_semantic_operation_id(&reserved),
            Err(SemanticOperationIdDecodeError::ReservedNonZero { offset })
        );
    }
}

#[test]
fn existing_launch_intrinsic_exposes_a_target_neutral_contract() {
    let intrinsic = launch_extent();
    let contract = intrinsic.contract();

    assert_eq!(
        contract.id,
        SemanticOperationId::v1(SemanticOperationKind::LaunchExtent)
    );
    assert_eq!(contract.operands, Vec::<ValueId>::new());
    assert_eq!(contract.result_types, vec![Type::INDEX]);
    assert_eq!(contract.memory_effects, Vec::<MemoryEffect>::new());
    assert_eq!(contract.required_capabilities, BTreeSet::new());

    let result = ValueDef::new(ValueId(7), Type::INDEX);
    assert!(
        intrinsic
            .verify(SemanticOperationVerificationContext {
                results: &[result],
                operand_types: &[],
            })
            .is_empty()
    );
}

#[test]
fn semantic_verifier_reports_shape_and_declared_type_failures() {
    let intrinsic =
        IntrinsicOperation::new(IntrinsicKind::LaunchExtent { axis: Axis::X }, Type::F32);
    let result = ValueDef::new(ValueId(9), Type::INDEX);
    let issues = intrinsic.verify(SemanticOperationVerificationContext {
        results: &[result],
        operand_types: &[Some(Type::INDEX)],
    });

    assert_eq!(issues.len(), 3);
    assert_eq!(
        issues
            .iter()
            .map(|issue| issue.kind)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            SemanticOperationIssueKind::InvalidStructure,
            SemanticOperationIssueKind::TypeMismatch,
        ])
    );

    let issues = intrinsic.verify(SemanticOperationVerificationContext {
        results: &[],
        operand_types: &[],
    });
    assert!(
        issues
            .iter()
            .any(|issue| issue.kind == SemanticOperationIssueKind::ResultArity)
    );
}

#[test]
fn semantic_identity_decoder_never_panics_on_bounded_noise() {
    let mut state = 0x6c8e_9cf5_7093_2bd1_u64;
    for length in 0..=32 {
        for _ in 0..64 {
            let mut bytes = vec![0; length];
            for byte in &mut bytes {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                *byte = state as u8;
            }
            assert!(
                catch_unwind(AssertUnwindSafe(|| decode_semantic_operation_id(&bytes))).is_ok()
            );
        }
    }
}
