use super::*;

fn scalar_module(operations: usize) -> Module {
    let mut module = Module::new("m");
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(vec![], vec![]),
        vec![],
        vec![BasicBlock {
            id: BlockId(u32::MAX),
            parameters: vec![],
            operations: (0..operations)
                .map(|index| {
                    Operation::effect_free(
                        ValueDef::new(
                            ValueId(u32::try_from(index).unwrap()),
                            Type::Scalar(ScalarType::U32),
                        ),
                        OperationKind::Constant(Constant::U32(7)),
                    )
                })
                .collect(),
            terminator: Some(Terminator::Return { values: vec![] }),
        }],
    ));
    module
}

#[test]
fn comparing_writer_has_exact_chunk_work_and_no_output_allocation() {
    // Five header chunks, the two-part ID, and three roster counts. The
    // placeholder query is deferred to the length patch; finish queries the
    // total length once. Empty text still performs its checked slice query.
    const CHUNKS: usize = 5 + 2 + 3;
    for text_bytes in [0, 1, 1_024] {
        let module = Module::new("m".repeat(text_bytes));
        let bytes = encode_module_v12(&module).unwrap();
        let wire = 20 + 4 + text_bytes + 3 * 4;
        let work = wire + 4 + CHUNKS + 1;
        assert_eq!(bytes.len(), wire);
        let mut budget = CanonicalKernelIrWorkBudgetV1::new(work);
        let mut writer = Writer::comparing(KERNEL_IR_VERSION_V12, &bytes, Some(&mut budget));
        write_module_v1(&module, &mut writer, true).unwrap();
        assert_eq!(writer.length(), wire);
        assert_eq!(writer.bytes.capacity(), 0);
        assert!(writer.finish_comparison().unwrap());
        assert_eq!(budget.work(), work);

        let mut one_under = CanonicalKernelIrWorkBudgetV1::new(work - 1);
        assert!(matches!(
            compare_module_encoding_v1(&module, KERNEL_IR_VERSION_V12, &bytes, Some(&mut one_under)),
            Err(KernelIrEncodeError::WorkLimit(error))
                if error.actual() == work && error.limit() == work - 1
        ));
        assert_eq!(one_under.work(), work - 1);
        assert_eq!(one_under.failed_work(), Some(work));
    }
}

#[test]
fn comparing_writer_covers_versions_scalar_rosters_and_spare_capacity() {
    // Module 10; function ID/signature/body/counts/capabilities 8; block 3;
    // return 3. Each U32 constant adds result-count/id/type(2)/kind/tag/value.
    const FIXED_CHUNKS: usize = 10 + 8 + 3 + 3;
    const CONSTANT_CHUNKS: usize = 1 + 1 + 2 + 1 + 1 + 1;
    const ROLE_WORK: usize = 1 + (1 + 1);
    for operations in [0, 1, 3, 2_048] {
        let mut module = scalar_module(operations);
        module.functions.reserve(37);
        module.functions[0].body.as_mut().unwrap().blocks[0]
            .operations
            .reserve(4_096);
        let wire = 81 + 16 * operations;
        let work = wire + 4 + FIXED_CHUNKS + CONSTANT_CHUNKS * operations + 1 + ROLE_WORK;
        for version in [KERNEL_IR_VERSION_V1, KERNEL_IR_VERSION_V12] {
            let bytes = encode_module(&module, version).unwrap();
            assert_eq!(bytes.len(), wire);
            let encode_work = wire + FIXED_CHUNKS + CONSTANT_CHUNKS * operations + 4 + ROLE_WORK;
            let mut encoder_budget = CanonicalKernelIrWorkBudgetV1::new(encode_work);
            assert_eq!(
                encode_module_with_work_v1(&module, version, &mut encoder_budget),
                Ok(bytes.clone())
            );
            assert_eq!(encoder_budget.work(), encode_work);
            let mut budget = CanonicalKernelIrWorkBudgetV1::new(work);
            assert_eq!(
                compare_module_encoding_v1(&module, version, &bytes, Some(&mut budget)),
                Ok(true)
            );
            assert_eq!(budget.work(), work);
            assert_eq!(decode_module_v12(&bytes), Ok(module.clone()));
        }
    }
}

#[test]
fn metered_decode_replaces_reencoding_with_one_exact_comparison_pass() {
    for text_bytes in [0, 1, 1_024] {
        let module = Module::new("m".repeat(text_bytes));
        let bytes = encode_module_v12(&module).unwrap();
        let wire = 20 + 4 + text_bytes + 3 * 4;
        // Read W, validate/copy text, compare W, patch4, ten chunk queries,
        // and the final total-length query. No function-role rows are present.
        let work = 2 * wire + 2 * text_bytes + 4 + 10 + 1;
        let mut budget = CanonicalKernelIrWorkBudgetV1::new(work);
        assert_eq!(
            decode_module_v12_with_work_v1(&bytes, &mut budget),
            Ok(module)
        );
        assert_eq!(budget.work(), work);
        let mut one_under = CanonicalKernelIrWorkBudgetV1::new(work - 1);
        assert!(matches!(
            decode_module_v12_with_work_v1(&bytes, &mut one_under),
            Err(KernelIrDecodeError::Encode(KernelIrEncodeError::WorkLimit(error)))
                if error.actual() == work && error.limit() == work - 1
        ));
        assert_eq!(one_under.work(), work - 1);
    }
}

#[test]
fn comparison_mismatches_are_delayed_and_include_length_and_empty_chunks() {
    let module = Module::new("");
    let bytes = encode_module_v12(&module).unwrap();
    const WORK: usize = 36 + 4 + 10 + 1;
    let mut mismatches = Vec::new();
    for offset in 0..bytes.len() {
        let mut changed = bytes.clone();
        changed[offset] ^= 1;
        mismatches.push(changed);
    }
    mismatches.push(bytes[..bytes.len() - 1].to_vec());
    mismatches.push(Vec::new());
    let mut trailing = bytes.clone();
    trailing.push(0);
    mismatches.push(trailing);
    for expected in mismatches {
        let mut budget = CanonicalKernelIrWorkBudgetV1::new(WORK);
        assert_eq!(
            compare_module_encoding_v1(
                &module,
                KERNEL_IR_VERSION_V12,
                &expected,
                Some(&mut budget)
            ),
            Ok(false)
        );
        assert_eq!(budget.work(), WORK);
        let mut one_under = CanonicalKernelIrWorkBudgetV1::new(WORK - 1);
        assert!(matches!(
            compare_module_encoding_v1(&module, KERNEL_IR_VERSION_V12, &expected, Some(&mut one_under)),
            Err(KernelIrEncodeError::WorkLimit(error)) if error.actual() == WORK
        ));
    }
}

#[test]
fn later_schema_role_and_depth_errors_precede_an_earlier_byte_mismatch() {
    let mut malformed = scalar_module(0);
    malformed.functions[0].id = FunctionId::new("f".repeat(MAX_TEXT_BYTES_V1 + 1));
    let mut unsupported_role = scalar_module(0);
    unsupported_role.functions[0].role = FunctionRole::DeviceFfiExport;
    let mut wrong_role = scalar_module(0);
    wrong_role.functions[0].role = FunctionRole::KernelEntry;
    for module in [malformed, unsupported_role, wrong_role] {
        let expected_error = encode_module_v12(&module).unwrap_err();
        let mut budget = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        assert_eq!(
            compare_module_encoding_v1(&module, KERNEL_IR_VERSION_V12, &[], Some(&mut budget)),
            Err(expected_error)
        );
        assert_eq!(budget.failed_work(), None);
    }

    for depth in [MAX_TYPE_DEPTH_V1, MAX_TYPE_DEPTH_V1 + 1] {
        let mut ty = Type::Scalar(ScalarType::U32);
        for _ in 0..depth {
            ty = Type::pointer(ty, AddressSpace::Global, AccessMode::ReadOnly);
        }
        let mut module = Module::new("m");
        module
            .functions
            .push(Function::declaration("f", Signature::new(vec![ty], vec![])));
        match encode_module_v12(&module) {
            Ok(bytes) => {
                assert_eq!(
                    compare_module_encoding_v1(&module, KERNEL_IR_VERSION_V12, &bytes, None),
                    Ok(true)
                );
                assert_eq!(decode_module_v12(&bytes), Ok(module));
            }
            Err(error) => {
                assert_eq!(depth, MAX_TYPE_DEPTH_V1 + 1);
                assert_eq!(
                    compare_module_encoding_v1(&module, KERNEL_IR_VERSION_V12, &[], None),
                    Err(error)
                );
            }
        }
    }
}

#[test]
fn comparing_memory_instances_checks_real_payload_bytes_and_exact_work() {
    const PAYLOAD: usize = 7 + 12;
    const INSTANCE: usize = crate::SEMANTIC_OPERATION_INSTANCE_HEADER_BYTES_V1 + PAYLOAD;
    const INSTANCE_WORK: usize = INSTANCE + PAYLOAD;
    const INSTANCE_FIELDS: usize = 3;
    let intrinsic = MemoryIntrinsicOperation::VolatileLoad {
        pointer: ValueId(19),
        element: crate::MemoryElementType::Scalar(ScalarType::U32),
        address_space: AddressSpace::Global,
        layout: crate::MemoryLayout::new(4, 4),
        contract: crate::VolatileAccessContract::rust_allocation_load(),
    };
    let mut actual = Writer::new(KERNEL_IR_VERSION_V12, None);
    encode_memory_intrinsic(&mut actual, &intrinsic).unwrap();
    let bytes = actual.bytes;
    const EXTENT: usize = 4 + INSTANCE + 4;
    const WORK: usize = INSTANCE_WORK + EXTENT + INSTANCE_FIELDS;
    assert_eq!(bytes.len(), EXTENT);
    for offset in 0..EXTENT {
        let mut expected = bytes.clone();
        expected[offset] ^= 1;
        let mut budget = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut writer = Writer::comparing(KERNEL_IR_VERSION_V12, &expected, Some(&mut budget));
        encode_memory_intrinsic(&mut writer, &intrinsic).unwrap();
        assert!(matches!(
            writer.mode,
            WriterModeV1::Compare { matches: false, .. }
        ));
        assert_eq!(writer.length(), EXTENT);
        assert_eq!(writer.bytes.capacity(), 0);
        drop(writer);
        assert_eq!(budget.work(), WORK);
    }
    let mut one_under = CanonicalKernelIrWorkBudgetV1::new(WORK - 1);
    let mut writer = Writer::comparing(KERNEL_IR_VERSION_V12, &bytes, Some(&mut one_under));
    assert!(matches!(encode_memory_intrinsic(&mut writer, &intrinsic),
        Err(KernelIrEncodeError::WorkLimit(error)) if error.actual() == WORK));
    assert_eq!(writer.length(), EXTENT - 4);
    drop(writer);
    assert_eq!(one_under.work(), WORK - 1);

    for instances in [1, 3, 257] {
        let mut module = scalar_module(0);
        module.functions[0].body.as_mut().unwrap().blocks[0].operations = (0..instances)
            .map(|index| {
                Operation::effect_free(
                    ValueDef::new(
                        ValueId(u32::try_from(index).unwrap()),
                        Type::Scalar(ScalarType::U32),
                    ),
                    OperationKind::MemoryIntrinsic(intrinsic),
                )
            })
            .collect();
        let bytes = encode_module_v12(&module).unwrap();
        let wire = 81 + instances * (4 + 6 + 1 + EXTENT);
        let chunks = 24 + instances * (1 + 1 + 2 + 1 + INSTANCE_FIELDS);
        let work = wire + 4 + chunks + 1 + 3 + instances * INSTANCE_WORK;
        assert_eq!(bytes.len(), wire);
        let mut budget = CanonicalKernelIrWorkBudgetV1::new(work);
        assert_eq!(
            compare_module_encoding_v1(&module, KERNEL_IR_VERSION_V12, &bytes, Some(&mut budget)),
            Ok(true)
        );
        assert_eq!(budget.work(), work);
        assert_eq!(decode_module_v12(&bytes), Ok(module.clone()));
        if instances == 1 {
            assert_eq!(
                compare_module_encoding_v1(&module, KERNEL_IR_VERSION_V1, &[], None),
                encode_module(&module, KERNEL_IR_VERSION_V1).map(|_| true)
            );
            let instance =
                encode_semantic_operation_instance_id(intrinsic.semantic_instance_id_v1());
            let offset = bytes
                .windows(instance.len())
                .position(|part| part == instance.as_slice())
                .unwrap();
            let mut malformed = bytes;
            malformed[offset] ^= 1;
            assert_eq!(
                decode_module_v12(&malformed),
                Err(KernelIrDecodeError::InvalidSemanticOperationInstance)
            );
        }
    }
}

#[test]
fn comparing_writer_failure_does_not_advance_or_publish_output() {
    let bytes = [1_u8, 2, 3, 4];
    let mut budget = CanonicalKernelIrWorkBudgetV1::new(bytes.len());
    let mut writer = Writer::comparing(KERNEL_IR_VERSION_V12, &bytes, Some(&mut budget));
    assert!(
        matches!(writer.bytes(&bytes), Err(KernelIrEncodeError::WorkLimit(error))
        if error.actual() == bytes.len() + 1)
    );
    assert_eq!(writer.length(), 0);
    assert!(matches!(
        writer.mode,
        WriterModeV1::Compare { matches: true, .. }
    ));
    assert_eq!(writer.bytes.capacity(), 0);
    assert!(matches!(
        writer.count_bytes(4),
        Err(KernelIrEncodeError::NonCanonical { .. })
    ));
    assert!(matches!(
        writer.finish_module(),
        Err(KernelIrEncodeError::NonCanonical { .. })
    ));
    assert_eq!(budget.work(), bytes.len());
    assert_eq!(budget.failed_work(), Some(bytes.len() + 1));

    let mut budget = CanonicalKernelIrWorkBudgetV1::new(bytes.len());
    let mut counter = Writer::counter(KERNEL_IR_VERSION_V12, &mut budget);
    counter.count_bytes(bytes.len()).unwrap();
    assert!(matches!(
        counter.finish_module(),
        Err(KernelIrEncodeError::NonCanonical { .. })
    ));
    assert_eq!(budget.work(), 1);
    assert_eq!(budget.failed_work(), None);
    assert!(matches!(
        Writer::counter(KERNEL_IR_VERSION_V12, &mut budget).finish_comparison(),
        Err(KernelIrEncodeError::NonCanonical { .. })
    ));
    assert!(matches!(
        Writer::new(KERNEL_IR_VERSION_V12, None).finish_comparison(),
        Err(KernelIrEncodeError::NonCanonical { .. })
    ));
}

#[test]
fn public_decoder_keeps_header_and_structural_error_precedence() {
    let bytes = encode_module_v12(&Module::new("m")).unwrap();
    let mut malformed = bytes.clone();
    malformed[0] ^= 1;
    malformed[8..10].copy_from_slice(&0_u16.to_le_bytes());
    assert_eq!(
        decode_module_v12(&malformed),
        Err(KernelIrDecodeError::InvalidMagic)
    );
    malformed = bytes.clone();
    malformed[8..10].copy_from_slice(&0_u16.to_le_bytes());
    malformed[10..12].copy_from_slice(&1_u16.to_le_bytes());
    assert_eq!(
        decode_module_v12(&malformed),
        Err(KernelIrDecodeError::UnknownVersion(0))
    );
    malformed = bytes.clone();
    malformed[10..12].copy_from_slice(&1_u16.to_le_bytes());
    malformed[12..16].copy_from_slice(&0_u32.to_le_bytes());
    assert_eq!(
        decode_module_v12(&malformed),
        Err(KernelIrDecodeError::UnsupportedFlags(1))
    );
    malformed = bytes.clone();
    malformed[12..16].copy_from_slice(&0_u32.to_le_bytes());
    assert_eq!(
        decode_module_v12(&malformed),
        Err(KernelIrDecodeError::InvalidLength { declared: 0 })
    );
    malformed = bytes.clone();
    malformed[12..16].copy_from_slice(&(u32::try_from(bytes.len()).unwrap() + 1).to_le_bytes());
    assert_eq!(
        decode_module_v12(&malformed),
        Err(KernelIrDecodeError::Truncated)
    );
    malformed = bytes;
    malformed.push(0);
    assert_eq!(
        decode_module_v12(&malformed),
        Err(KernelIrDecodeError::TrailingBytes)
    );
}
