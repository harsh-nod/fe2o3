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

    let mut noncanonical_v2 = encoded;
    noncanonical_v2[8..10].copy_from_slice(&SEMANTIC_OPERATION_VERSION_V2.to_le_bytes());
    assert_eq!(
        decode_semantic_operation_schema(&noncanonical_v2),
        Err(SemanticOperationSchemaDecodeError::NonCanonicalVersion {
            version: SEMANTIC_OPERATION_VERSION_V2,
            kind: SemanticOperationKind::LaunchExtent,
        })
    );
}

#[test]
fn schema_decoder_rejects_unknown_dispatch_authority() {
    let encoded = encode_semantic_operation_schema(SemanticOperationSchema::v1(
        SemanticOperationKind::LaunchInvocationIndex,
    ));

    let mut unknown_version = encoded;
    unknown_version[8..10].copy_from_slice(&3_u16.to_le_bytes());
    assert_eq!(
        decode_semantic_operation_schema(&unknown_version),
        Err(SemanticOperationSchemaDecodeError::UnknownVersion(3))
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
fn wide_memory_elements_use_additive_v2_identity() {
    let element = MemoryElementType::Scalar(ScalarType::I128);
    let id = SemanticOperationInstanceId::volatile_load(
        element,
        AddressSpace::Global,
        element.expected_layout(),
        VolatileAccessContract::rust_allocation_load(),
    );
    assert_eq!(id.schema().version(), SEMANTIC_OPERATION_VERSION_V2);
    let schema_bytes = encode_semantic_operation_schema(id.schema());
    assert_eq!(
        decode_semantic_operation_schema(&schema_bytes),
        Ok(id.schema())
    );
    let encoded = encode_semantic_operation_instance_id(id);
    assert_eq!(encoded[8..10], SEMANTIC_OPERATION_VERSION_V2.to_le_bytes());
    assert_eq!(decode_semantic_operation_instance_id(&encoded), Ok(id));

    let mut forged_v1 = encoded.clone();
    forged_v1[8..10].copy_from_slice(&SEMANTIC_OPERATION_VERSION_V1.to_le_bytes());
    assert_eq!(
        decode_semantic_operation_instance_id(&forged_v1),
        Err(SemanticOperationInstanceDecodeError::UnknownPayloadTag {
            field: "memory element",
            tag: 15,
        })
    );

    let legacy = SemanticOperationInstanceId::volatile_load(
        MemoryElementType::Scalar(ScalarType::U64),
        AddressSpace::Global,
        MemoryElementType::Scalar(ScalarType::U64).expected_layout(),
        VolatileAccessContract::rust_allocation_load(),
    );
    let mut noncanonical_v2 = encode_semantic_operation_instance_id(legacy);
    noncanonical_v2[8..10].copy_from_slice(&SEMANTIC_OPERATION_VERSION_V2.to_le_bytes());
    assert_eq!(
        decode_semantic_operation_instance_id(&noncanonical_v2),
        Err(SemanticOperationInstanceDecodeError::NonCanonicalVersion {
            version: SEMANTIC_OPERATION_VERSION_V2,
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
fn memory_instance_byte_vectors_are_frozen_independently() {
    let u32_element = MemoryElementType::Scalar(ScalarType::U32);
    let cases = [
        (
            SemanticOperationInstanceId::pointer_distance(
                PointerDistanceKind::Signed,
                PointerDistanceUnit::Elements,
                u32_element,
                AddressSpace::Global,
                MemoryLayout::new(4, 4),
                PointerDistanceContract::supported_rust(PointerDistanceKind::Signed),
            ),
            vec![
                b'F', b'E', b'2', b'O', b'3', b'S', b'I', 0, 1, 0, 1, 0, 1, 0, 21, 0, 0, 0, 0, 0,
                1, 1, 8, 3, 1, 1, 1, 1, 1, 4, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0,
            ],
        ),
        (
            SemanticOperationInstanceId::volatile_load(
                u32_element,
                AddressSpace::Global,
                MemoryLayout::new(4, 4),
                VolatileAccessContract::external_mmio_load(),
            ),
            vec![
                b'F', b'E', b'2', b'O', b'3', b'S', b'I', 0, 1, 0, 1, 0, 2, 0, 19, 0, 0, 0, 0, 0,
                8, 3, 2, 1, 1, 1, 2, 4, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0,
            ],
        ),
        (
            SemanticOperationInstanceId::volatile_store(
                u32_element,
                AddressSpace::Global,
                MemoryLayout::new(4, 4),
                VolatileAccessContract::external_mmio_store(),
            ),
            vec![
                b'F', b'E', b'2', b'O', b'3', b'S', b'I', 0, 1, 0, 1, 0, 3, 0, 19, 0, 0, 0, 0, 0,
                8, 3, 2, 2, 1, 1, 2, 4, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0,
            ],
        ),
        (
            SemanticOperationInstanceId::volatile_load(
                MemoryElementType::Unit,
                AddressSpace::Global,
                MemoryLayout::new(0, 1),
                VolatileAccessContract::zero_sized_aligned_no_access(),
            ),
            vec![
                b'F', b'E', b'2', b'O', b'3', b'S', b'I', 0, 1, 0, 1, 0, 2, 0, 19, 0, 0, 0, 0, 0,
                0, 3, 3, 3, 1, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0,
            ],
        ),
        (
            SemanticOperationInstanceId::copy_nonoverlapping(
                u32_element,
                AddressSpace::Constant,
                AddressSpace::Global,
                MemoryLayout::new(4, 4),
                CopyNonOverlappingContract::supported_rust(),
            ),
            vec![
                b'F', b'E', b'2', b'O', b'3', b'S', b'I', 0, 1, 0, 1, 0, 4, 0, 22, 0, 0, 0, 0, 0,
                8, 4, 3, 1, 1, 1, 1, 1, 1, 1, 4, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0,
            ],
        ),
    ];

    for (id, expected) in cases {
        assert_eq!(encode_semantic_operation_instance_id(id), expected);
        assert_eq!(decode_semantic_operation_instance_id(&expected), Ok(id));
    }
}

#[test]
fn memory_instance_decoder_rejects_obligation_mutations() {
    let pointer = SemanticOperationInstanceId::pointer_distance(
        PointerDistanceKind::Signed,
        PointerDistanceUnit::Elements,
        MemoryElementType::Scalar(ScalarType::U32),
        AddressSpace::Global,
        MemoryLayout::new(4, 4),
        PointerDistanceContract::supported_rust(PointerDistanceKind::Signed),
    );
    let pointer_bytes = encode_semantic_operation_instance_id(pointer);
    for offset in 24..=28 {
        let mut mutated = pointer_bytes.clone();
        mutated[offset] = if offset == 28 { 2 } else { 0xff };
        assert!(decode_semantic_operation_instance_id(&mutated).is_err());
    }

    let load = SemanticOperationInstanceId::volatile_load(
        MemoryElementType::Scalar(ScalarType::U32),
        AddressSpace::Global,
        MemoryLayout::new(4, 4),
        VolatileAccessContract::rust_allocation_load(),
    );
    let load_bytes = encode_semantic_operation_instance_id(load);
    for offset in 22..=26 {
        let mut mutated = load_bytes.clone();
        mutated[offset] = match offset {
            22 | 23 | 25 | 26 => 2,
            _ => 0xff,
        };
        assert!(decode_semantic_operation_instance_id(&mutated).is_err());
    }

    let zst_load = SemanticOperationInstanceId::volatile_load(
        MemoryElementType::Unit,
        AddressSpace::Workgroup,
        MemoryLayout::new(0, 1),
        VolatileAccessContract::zero_sized_aligned_no_access(),
    );
    let mut positive_access_contract = encode_semantic_operation_instance_id(zst_load);
    positive_access_contract[22] = 1;
    assert!(decode_semantic_operation_instance_id(&positive_access_contract).is_err());

    let copy = SemanticOperationInstanceId::copy_nonoverlapping(
        MemoryElementType::Unit,
        AddressSpace::Global,
        AddressSpace::Global,
        MemoryLayout::new(0, 1),
        CopyNonOverlappingContract::supported_rust(),
    );
    let copy_bytes = encode_semantic_operation_instance_id(copy);
    for offset in 23..=29 {
        let mut mutated = copy_bytes.clone();
        mutated[offset] = 0xff;
        assert!(decode_semantic_operation_instance_id(&mutated).is_err());
    }
}

#[test]
fn memory_instance_decoder_rejects_noncanonical_element_layout_pairs() {
    const INSTANCE_HEADER_BYTES: usize = 20;

    fn mutate_layout(
        id: SemanticOperationInstanceId,
        payload_layout_offset: usize,
        layout: MemoryLayout,
    ) -> Vec<u8> {
        let mut bytes = encode_semantic_operation_instance_id(id);
        let offset = INSTANCE_HEADER_BYTES + payload_layout_offset;
        bytes[offset..offset + 8].copy_from_slice(&layout.size_bytes.to_le_bytes());
        bytes[offset + 8..offset + 12].copy_from_slice(&layout.alignment_bytes.to_le_bytes());
        bytes
    }

    let u32_element = MemoryElementType::Scalar(ScalarType::U32);
    let cases = [
        (
            SemanticOperationInstanceId::pointer_distance(
                PointerDistanceKind::Signed,
                PointerDistanceUnit::Elements,
                u32_element,
                AddressSpace::Global,
                u32_element.expected_layout(),
                PointerDistanceContract::supported_rust(PointerDistanceKind::Signed),
            ),
            9,
            SemanticOperationKind::PointerDistance,
            u32_element,
            MemoryLayout::new(8, 4),
        ),
        (
            SemanticOperationInstanceId::pointer_distance(
                PointerDistanceKind::Unsigned,
                PointerDistanceUnit::Bytes,
                u32_element,
                AddressSpace::Global,
                u32_element.expected_layout(),
                PointerDistanceContract::supported_rust(PointerDistanceKind::Unsigned),
            ),
            9,
            SemanticOperationKind::PointerDistance,
            u32_element,
            MemoryLayout::new(4, 8),
        ),
        (
            SemanticOperationInstanceId::volatile_load(
                u32_element,
                AddressSpace::Global,
                u32_element.expected_layout(),
                VolatileAccessContract::external_mmio_load(),
            ),
            7,
            SemanticOperationKind::VolatileLoad,
            u32_element,
            MemoryLayout::new(0, 1),
        ),
        (
            SemanticOperationInstanceId::volatile_store(
                u32_element,
                AddressSpace::Global,
                u32_element.expected_layout(),
                VolatileAccessContract::external_mmio_store(),
            ),
            7,
            SemanticOperationKind::VolatileStore,
            u32_element,
            MemoryLayout::new(8, 8),
        ),
        (
            SemanticOperationInstanceId::volatile_load(
                MemoryElementType::Unit,
                AddressSpace::Global,
                MemoryElementType::Unit.expected_layout(),
                VolatileAccessContract::zero_sized_aligned_no_access(),
            ),
            7,
            SemanticOperationKind::VolatileLoad,
            MemoryElementType::Unit,
            MemoryLayout::new(4, 4),
        ),
        (
            SemanticOperationInstanceId::volatile_store(
                MemoryElementType::Unit,
                AddressSpace::Workgroup,
                MemoryElementType::Unit.expected_layout(),
                VolatileAccessContract::zero_sized_aligned_no_access(),
            ),
            7,
            SemanticOperationKind::VolatileStore,
            MemoryElementType::Unit,
            MemoryLayout::new(0, 4),
        ),
        (
            SemanticOperationInstanceId::copy_nonoverlapping(
                u32_element,
                AddressSpace::Constant,
                AddressSpace::Global,
                u32_element.expected_layout(),
                CopyNonOverlappingContract::supported_rust(),
            ),
            10,
            SemanticOperationKind::CopyNonOverlapping,
            u32_element,
            MemoryLayout::new(4, 2),
        ),
        (
            SemanticOperationInstanceId::copy_nonoverlapping(
                MemoryElementType::Unit,
                AddressSpace::Global,
                AddressSpace::Global,
                MemoryElementType::Unit.expected_layout(),
                CopyNonOverlappingContract::supported_rust(),
            ),
            10,
            SemanticOperationKind::CopyNonOverlapping,
            MemoryElementType::Unit,
            MemoryLayout::new(1, 1),
        ),
    ];

    for (id, payload_layout_offset, kind, element, actual) in cases {
        assert_eq!(
            decode_semantic_operation_instance_id(&mutate_layout(
                id,
                payload_layout_offset,
                actual,
            )),
            Err(
                SemanticOperationInstanceDecodeError::NonCanonicalMemoryLayout {
                    kind,
                    element,
                    actual,
                    expected: element.expected_layout(),
                }
            )
        );
    }
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
    unknown_version[8..10].copy_from_slice(&3_u16.to_le_bytes());
    assert_eq!(
        decode_semantic_operation_instance_id(&unknown_version),
        Err(SemanticOperationInstanceDecodeError::UnknownVersion(3))
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

fn pointer(element: MemoryElementType, address_space: AddressSpace, access: AccessMode) -> Type {
    Type::pointer(element.ir_type(), address_space, access)
}

#[test]
fn memory_instance_identity_binds_layout_address_space_and_preconditions() {
    let instances = [
        SemanticOperationInstanceId::pointer_distance(
            PointerDistanceKind::Signed,
            PointerDistanceUnit::Elements,
            MemoryElementType::Scalar(ScalarType::U32),
            AddressSpace::Global,
            MemoryLayout::new(4, 4),
            PointerDistanceContract::supported_rust(PointerDistanceKind::Signed),
        ),
        SemanticOperationInstanceId::volatile_load(
            MemoryElementType::Scalar(ScalarType::U32),
            AddressSpace::Constant,
            MemoryLayout::new(4, 4),
            VolatileAccessContract::rust_allocation_load(),
        ),
        SemanticOperationInstanceId::volatile_store(
            MemoryElementType::Scalar(ScalarType::U32),
            AddressSpace::Global,
            MemoryLayout::new(4, 4),
            VolatileAccessContract::external_mmio_store(),
        ),
        SemanticOperationInstanceId::copy_nonoverlapping(
            MemoryElementType::Scalar(ScalarType::U32),
            AddressSpace::Constant,
            AddressSpace::Global,
            MemoryLayout::new(4, 4),
            CopyNonOverlappingContract::supported_rust(),
        ),
    ];

    let mut encodings = BTreeSet::new();
    for instance in instances {
        let encoded = encode_semantic_operation_instance_id(instance);
        assert!(encodings.insert(encoded.clone()));
        assert_eq!(
            decode_semantic_operation_instance_id(&encoded),
            Ok(instance)
        );
    }

    let changed_layout = SemanticOperationInstanceId::volatile_load(
        MemoryElementType::Scalar(ScalarType::U32),
        AddressSpace::Constant,
        MemoryLayout::new(8, 8),
        VolatileAccessContract::rust_allocation_load(),
    );
    assert!(!encodings.contains(&encode_semantic_operation_instance_id(changed_layout)));
}

#[test]
fn pointer_distance_verifier_checks_layout_zst_contract_and_address_space() {
    let operation = MemoryIntrinsicOperation::PointerDistance {
        pointer: ValueId(0),
        origin: ValueId(1),
        kind: PointerDistanceKind::Signed,
        unit: PointerDistanceUnit::Elements,
        element: MemoryElementType::Scalar(ScalarType::U32),
        address_space: AddressSpace::Global,
        layout: MemoryLayout::new(4, 4),
        contract: PointerDistanceContract::supported_rust(PointerDistanceKind::Signed),
    };
    let operands = operation.operands();
    let results = [ValueDef::new(ValueId(2), Type::Scalar(ScalarType::I64))];
    let valid_types = [
        Some(pointer(
            MemoryElementType::Scalar(ScalarType::U32),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        )),
        Some(pointer(
            MemoryElementType::Scalar(ScalarType::U32),
            AddressSpace::Global,
            AccessMode::ReadWrite,
        )),
    ];
    assert!(
        operation
            .verify(SemanticOperationVerificationContext {
                operands: &operands,
                results: &results,
                operand_types: &valid_types,
            })
            .is_empty()
    );

    let mismatched_types = [
        valid_types[0].clone(),
        Some(pointer(
            MemoryElementType::Scalar(ScalarType::U32),
            AddressSpace::Workgroup,
            AccessMode::ReadOnly,
        )),
    ];
    let issues = operation.verify(SemanticOperationVerificationContext {
        operands: &operands,
        results: &results,
        operand_types: &mismatched_types,
    });
    assert!(
        issues
            .iter()
            .any(|issue| issue.kind == SemanticOperationIssueKind::InvalidOperandType)
    );

    let zst = MemoryIntrinsicOperation::PointerDistance {
        pointer: ValueId(0),
        origin: ValueId(1),
        kind: PointerDistanceKind::Signed,
        unit: PointerDistanceUnit::Elements,
        element: MemoryElementType::Unit,
        address_space: AddressSpace::Global,
        layout: MemoryLayout::new(0, 1),
        contract: PointerDistanceContract::supported_rust(PointerDistanceKind::Signed),
    };
    let issues = zst.verify(SemanticOperationVerificationContext {
        operands: &zst.operands(),
        results: &results,
        operand_types: &[
            Some(pointer(
                MemoryElementType::Unit,
                AddressSpace::Global,
                AccessMode::ReadOnly,
            )),
            Some(pointer(
                MemoryElementType::Unit,
                AddressSpace::Global,
                AccessMode::ReadOnly,
            )),
        ],
    });
    assert!(
        issues
            .iter()
            .any(|issue| issue.message.contains("zero-sized"))
    );

    let wrong_contract = MemoryIntrinsicOperation::PointerDistance {
        pointer: ValueId(0),
        origin: ValueId(1),
        kind: PointerDistanceKind::Signed,
        unit: PointerDistanceUnit::Elements,
        element: MemoryElementType::Scalar(ScalarType::U32),
        address_space: AddressSpace::Global,
        layout: MemoryLayout::new(4, 4),
        contract: PointerDistanceContract::supported_rust(PointerDistanceKind::Unsigned),
    };
    let issues = wrong_contract.verify(SemanticOperationVerificationContext {
        operands: &operands,
        results: &results,
        operand_types: &valid_types,
    });
    assert!(
        issues
            .iter()
            .any(|issue| issue.message.contains("kind-specific ordering"))
    );
}

#[test]
fn pointer_distance_identity_binds_the_equal_address_disjunction() {
    let contract = PointerDistanceContract::supported_rust(PointerDistanceKind::Signed);
    assert_eq!(
        contract.provenance,
        PointerDistanceProvenanceContract::EqualAddressesOrSameAllocation
    );
    assert_eq!(
        contract.range,
        PointerDistanceRangeContract::BothPointersInBoundsOrOnePastWhenAddressesDiffer
    );

    let identity = SemanticOperationInstanceId::pointer_distance(
        PointerDistanceKind::Signed,
        PointerDistanceUnit::Elements,
        MemoryElementType::Scalar(ScalarType::U32),
        AddressSpace::Global,
        MemoryLayout::new(4, 4),
        contract,
    );
    assert_eq!(
        decode_semantic_operation_instance_id(&encode_semantic_operation_instance_id(identity)),
        Ok(identity)
    );

    let element = MemoryElementType::Scalar(ScalarType::U32);
    let operation = MemoryIntrinsicOperation::PointerDistance {
        pointer: ValueId(0),
        origin: ValueId(0),
        kind: PointerDistanceKind::Signed,
        unit: PointerDistanceUnit::Elements,
        element,
        address_space: AddressSpace::Global,
        layout: element.expected_layout(),
        contract,
    };
    let pointer_type = Some(pointer(element, AddressSpace::Global, AccessMode::ReadOnly));
    assert!(
        operation
            .verify(SemanticOperationVerificationContext {
                operands: &operation.operands(),
                results: &[ValueDef::new(ValueId(1), Type::Scalar(ScalarType::I64))],
                operand_types: &[pointer_type.clone(), pointer_type],
            })
            .is_empty()
    );
}

#[test]
fn volatile_and_copy_contracts_retain_effects_and_overlap_obligation() {
    let load = MemoryIntrinsicOperation::VolatileLoad {
        pointer: ValueId(0),
        element: MemoryElementType::Scalar(ScalarType::U32),
        address_space: AddressSpace::Global,
        layout: MemoryLayout::new(4, 4),
        contract: VolatileAccessContract::external_mmio_load(),
    };
    assert_eq!(
        load.contract().memory_effects,
        vec![MemoryEffect::VolatileRead(AddressSpace::Global)]
    );
    let summary = MemoryEffectSummary::new(load.contract().memory_effects);
    assert!(summary.reads(AddressSpace::Global));
    assert!(summary.volatile_reads(AddressSpace::Global));
    assert!(
        !summary
            .effects()
            .contains(&MemoryEffect::Read(AddressSpace::Global))
    );

    let copy = MemoryIntrinsicOperation::CopyNonOverlapping {
        source: ValueId(0),
        destination: ValueId(1),
        count: ValueId(2),
        element: MemoryElementType::Scalar(ScalarType::U32),
        source_address_space: AddressSpace::Constant,
        destination_address_space: AddressSpace::Global,
        layout: MemoryLayout::new(4, 4),
        contract: CopyNonOverlappingContract::supported_rust(),
    };
    assert_eq!(
        copy.contract().memory_effects,
        vec![
            MemoryEffect::Read(AddressSpace::Constant),
            MemoryEffect::Write(AddressSpace::Global),
        ]
    );
    assert!(matches!(
        copy.contract().instance_id().payload(),
        SemanticOperationInstancePayloadV1::CopyNonOverlapping {
            contract: CopyNonOverlappingContract {
                overlap: CopyOverlapContract::NonOverlappingWhenBytesPositive,
                zero_bytes: CopyZeroByteContract::AlignmentRequiredRangesAndOverlapConditionalOnPositiveBytes,
                ..
            },
            ..
        }
    ));
}

#[test]
fn semantic_memory_reads_reject_write_only_while_stores_accept_it() {
    let element = MemoryElementType::Scalar(ScalarType::U32);
    let write_only = Some(pointer(
        element,
        AddressSpace::Global,
        AccessMode::WriteOnly,
    ));
    let load = MemoryIntrinsicOperation::VolatileLoad {
        pointer: ValueId(0),
        element,
        address_space: AddressSpace::Global,
        layout: element.expected_layout(),
        contract: VolatileAccessContract::rust_allocation_load(),
    };
    assert!(
        load.verify(SemanticOperationVerificationContext {
            operands: &load.operands(),
            results: &[ValueDef::new(ValueId(2), element.ir_type())],
            operand_types: std::slice::from_ref(&write_only),
        })
        .iter()
        .any(|issue| issue.kind == SemanticOperationIssueKind::InvalidOperandType)
    );

    let store = MemoryIntrinsicOperation::VolatileStore {
        pointer: ValueId(0),
        value: ValueId(1),
        element,
        address_space: AddressSpace::Global,
        layout: element.expected_layout(),
        contract: VolatileAccessContract::rust_allocation_store(),
    };
    assert!(
        store
            .verify(SemanticOperationVerificationContext {
                operands: &store.operands(),
                results: &[],
                operand_types: &[write_only, Some(element.ir_type())],
            })
            .is_empty()
    );
}

#[test]
fn volatile_origins_are_distinct_and_access_obligations_fail_closed() {
    let rust = SemanticOperationInstanceId::volatile_load(
        MemoryElementType::Scalar(ScalarType::U32),
        AddressSpace::Global,
        MemoryLayout::new(4, 4),
        VolatileAccessContract::rust_allocation_load(),
    );
    let external = SemanticOperationInstanceId::volatile_load(
        MemoryElementType::Scalar(ScalarType::U32),
        AddressSpace::Global,
        MemoryLayout::new(4, 4),
        VolatileAccessContract::external_mmio_load(),
    );
    assert_ne!(rust, external);
    assert_ne!(
        encode_semantic_operation_instance_id(rust),
        encode_semantic_operation_instance_id(external)
    );
    for identity in [
        external,
        SemanticOperationInstanceId::volatile_store(
            MemoryElementType::Scalar(ScalarType::U32),
            AddressSpace::Global,
            MemoryLayout::new(4, 4),
            VolatileAccessContract::external_mmio_store(),
        ),
    ] {
        let (SemanticOperationInstancePayloadV1::VolatileLoad { contract, .. }
        | SemanticOperationInstancePayloadV1::VolatileStore { contract, .. }) = identity.payload()
        else {
            unreachable!()
        };
        assert_eq!(
            contract.external_effect,
            VolatileExternalEffectContract::SideEffectsDoNotModifyRustAllocatedMemory
        );
        assert_eq!(encode_semantic_operation_instance_id(identity)[26], 2);
    }
    let mut wrong_external_space = encode_semantic_operation_instance_id(external);
    wrong_external_space[21] = 2;
    assert!(matches!(
        decode_semantic_operation_instance_id(&wrong_external_space),
        Err(SemanticOperationInstanceDecodeError::InvalidContract {
            kind: SemanticOperationKind::VolatileLoad,
        })
    ));
    let mut missing_external_isolation = encode_semantic_operation_instance_id(external);
    missing_external_isolation[26] = 1;
    assert!(matches!(
        decode_semantic_operation_instance_id(&missing_external_isolation),
        Err(SemanticOperationInstanceDecodeError::InvalidContract {
            kind: SemanticOperationKind::VolatileLoad,
        })
    ));

    let malformed = MemoryIntrinsicOperation::VolatileLoad {
        pointer: ValueId(0),
        element: MemoryElementType::Scalar(ScalarType::U32),
        address_space: AddressSpace::Global,
        layout: MemoryLayout::new(4, 4),
        contract: VolatileAccessContract::rust_allocation_store(),
    };
    let issues = malformed.verify(SemanticOperationVerificationContext {
        operands: &malformed.operands(),
        results: &[ValueDef::new(ValueId(1), Type::Scalar(ScalarType::U32))],
        operand_types: &[Some(pointer(
            MemoryElementType::Scalar(ScalarType::U32),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        ))],
    });
    assert!(
        issues
            .iter()
            .any(|issue| issue.message.contains("readable initialized-element"))
    );

    let external_workgroup = MemoryIntrinsicOperation::VolatileLoad {
        pointer: ValueId(0),
        element: MemoryElementType::Scalar(ScalarType::U32),
        address_space: AddressSpace::Workgroup,
        layout: MemoryLayout::new(4, 4),
        contract: VolatileAccessContract::external_mmio_load(),
    };
    let issues = external_workgroup.verify(SemanticOperationVerificationContext {
        operands: &external_workgroup.operands(),
        results: &[ValueDef::new(ValueId(1), Type::Scalar(ScalarType::U32))],
        operand_types: &[Some(pointer(
            MemoryElementType::Scalar(ScalarType::U32),
            AddressSpace::Workgroup,
            AccessMode::ReadOnly,
        ))],
    });
    assert!(
        issues
            .iter()
            .any(|issue| issue.message.contains("external side-effect isolation"))
    );
}

#[test]
fn volatile_zst_is_an_aligned_no_access_profile() {
    let zst = MemoryElementType::Unit;
    let operation = MemoryIntrinsicOperation::VolatileLoad {
        pointer: ValueId(0),
        element: zst,
        address_space: AddressSpace::Workgroup,
        layout: zst.expected_layout(),
        contract: VolatileAccessContract::zero_sized_aligned_no_access(),
    };
    let operand_types = [Some(pointer(
        zst,
        AddressSpace::Workgroup,
        AccessMode::ReadOnly,
    ))];
    assert!(
        operation
            .verify(SemanticOperationVerificationContext {
                operands: &operation.operands(),
                results: &[],
                operand_types: &operand_types,
            })
            .is_empty()
    );
    assert!(operation.contract().memory_effects.is_empty());

    let store = MemoryIntrinsicOperation::VolatileStore {
        pointer: ValueId(0),
        value: ValueId(1),
        element: zst,
        address_space: AddressSpace::Workgroup,
        layout: zst.expected_layout(),
        contract: VolatileAccessContract::zero_sized_aligned_no_access(),
    };
    assert!(
        store
            .verify(SemanticOperationVerificationContext {
                operands: &store.operands(),
                results: &[],
                operand_types: &[
                    Some(pointer(zst, AddressSpace::Workgroup, AccessMode::ReadWrite)),
                    Some(Type::Unit),
                ],
            })
            .is_empty()
    );
    assert!(store.contract().memory_effects.is_empty());
    let store_identity = store.contract().instance_id();
    assert_eq!(
        decode_semantic_operation_instance_id(&encode_semantic_operation_instance_id(
            store_identity
        )),
        Ok(store_identity)
    );

    let mut positive_access_claim = operation;
    let MemoryIntrinsicOperation::VolatileLoad { contract, .. } = &mut positive_access_claim else {
        unreachable!()
    };
    *contract = VolatileAccessContract::rust_allocation_load();
    assert!(
        positive_access_claim
            .verify(SemanticOperationVerificationContext {
                operands: &positive_access_claim.operands(),
                results: &[],
                operand_types: &operand_types,
            })
            .iter()
            .any(|issue| issue.message.contains("aligned ZST no-access"))
    );
}

#[test]
fn zst_copy_keeps_alignment_and_conditional_range_contracts() {
    let zst = MemoryElementType::Unit;
    let operation = MemoryIntrinsicOperation::CopyNonOverlapping {
        source: ValueId(0),
        destination: ValueId(1),
        count: ValueId(2),
        element: zst,
        source_address_space: AddressSpace::Global,
        destination_address_space: AddressSpace::Global,
        layout: zst.expected_layout(),
        contract: CopyNonOverlappingContract::supported_rust(),
    };
    let operand_types = [
        Some(pointer(zst, AddressSpace::Global, AccessMode::ReadOnly)),
        Some(pointer(zst, AddressSpace::Global, AccessMode::ReadWrite)),
        Some(Type::INDEX),
    ];
    assert!(
        operation
            .verify(SemanticOperationVerificationContext {
                operands: &operation.operands(),
                results: &[],
                operand_types: &operand_types,
            })
            .is_empty()
    );
    assert_eq!(
        CopyNonOverlappingContract::supported_rust().zero_bytes,
        CopyZeroByteContract::AlignmentRequiredRangesAndOverlapConditionalOnPositiveBytes
    );

    let misaligned_contract = MemoryIntrinsicOperation::CopyNonOverlapping {
        source: ValueId(0),
        destination: ValueId(1),
        count: ValueId(2),
        element: zst,
        source_address_space: AddressSpace::Global,
        destination_address_space: AddressSpace::Global,
        layout: MemoryLayout::new(0, 2),
        contract: CopyNonOverlappingContract::supported_rust(),
    };
    let issues = misaligned_contract.verify(SemanticOperationVerificationContext {
        operands: &misaligned_contract.operands(),
        results: &[],
        operand_types: &operand_types,
    });
    assert!(issues.iter().any(|issue| issue.message.contains("layout")));
}

#[test]
fn byte_scaling_is_checked_and_zst_copy_is_zero_bytes() {
    for count in [0, 1, 7, u32::MAX as u64] {
        assert_eq!(
            MemoryLayout::new(4, 4).checked_byte_count(count),
            count.checked_mul(4)
        );
    }
    assert_eq!(
        MemoryLayout::new(0, 1).checked_byte_count(u64::MAX),
        Some(0)
    );
    assert_eq!(MemoryLayout::new(8, 8).checked_byte_count(u64::MAX), None);
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
