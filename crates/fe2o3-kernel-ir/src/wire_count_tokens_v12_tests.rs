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
                        ValueDef::new(ValueId(index as u32), Type::Scalar(ScalarType::U32)),
                        OperationKind::Constant(Constant::U32(7)),
                    )
                })
                .collect(),
            terminator: Some(Terminator::Return { values: vec![] }),
        }],
    ));
    module
}

fn primitive_fields(writer: &mut Writer<'_>) -> Result<(), KernelIrEncodeError> {
    writer.u8(0xa5)?;
    writer.u16(0x1234)?;
    writer.u32(0x1234_5678)?;
    writer.u64(0x0123_4567_89ab_cdef)?;
    writer.bytes(&[8, 9, 10])?;
    writer.bytes(&[])?;
    writer.text("text", "abc")?;
    writer.count("count", 2, 2)
}

#[test]
fn primitive_count_tokens_leave_materialize_and_compare_work_unchanged() {
    // Four scalars, two spans (one empty), text length/span, and a count.
    const TOKENS: usize = 4 + 2 + 2 + 1;
    const BYTES: [u8; 29] = [
        0xa5, 0x34, 0x12, 0x78, 0x56, 0x34, 0x12, 0xef, 0xcd, 0xab, 0x89, 0x67, 0x45, 0x23, 0x01,
        8, 9, 10, 3, 0, 0, 0, b'a', b'b', b'c', 2, 0, 0, 0,
    ];
    let mut work = CanonicalKernelIrWorkBudgetV1::new(TOKENS);
    let mut counter = Writer::counter(KERNEL_IR_VERSION_V12, &mut work);
    primitive_fields(&mut counter).unwrap();
    assert_eq!(counter.length(), BYTES.len());
    assert_eq!(counter.bytes.capacity(), 0);
    assert_eq!(counter.peak_auxiliary_bytes, 0);
    assert!(matches!(
        counter.finish_module(),
        Err(KernelIrEncodeError::NonCanonical { .. })
    ));
    assert_eq!(work.work(), TOKENS);

    let mut one_under = CanonicalKernelIrWorkBudgetV1::new(TOKENS - 1);
    let mut counter = Writer::counter(KERNEL_IR_VERSION_V12, &mut one_under);
    assert!(matches!(
        primitive_fields(&mut counter),
        Err(KernelIrEncodeError::WorkLimit(error)) if error.actual() == TOKENS
    ));
    assert_eq!(counter.length(), BYTES.len() - 4);
    assert_eq!(counter.bytes.capacity(), 0);
    drop(counter);
    assert_eq!(one_under.work(), TOKENS - 1);

    let mut work = CanonicalKernelIrWorkBudgetV1::new(BYTES.len());
    let mut writer = Writer::new(KERNEL_IR_VERSION_V12, Some(&mut work));
    primitive_fields(&mut writer).unwrap();
    assert_eq!(writer.bytes, BYTES);
    assert!(matches!(
        writer.count_bytes(1),
        Err(KernelIrEncodeError::NonCanonical { .. })
    ));
    drop(writer);
    assert_eq!(work.work(), BYTES.len());

    let mut work = CanonicalKernelIrWorkBudgetV1::new(BYTES.len() + TOKENS);
    let mut writer = Writer::comparing(KERNEL_IR_VERSION_V12, &BYTES, Some(&mut work));
    primitive_fields(&mut writer).unwrap();
    assert!(matches!(
        writer.mode,
        WriterModeV1::Compare { matches: true, .. }
    ));
    assert_eq!(writer.bytes.capacity(), 0);
    assert!(matches!(
        writer.count_bytes(1),
        Err(KernelIrEncodeError::NonCanonical { .. })
    ));
    drop(writer);
    assert_eq!(work.work(), BYTES.len() + TOKENS);
}

#[test]
fn empty_module_token_count_is_independent_of_payload_length_and_capacity() {
    // Header: magic and four scalars. ID: length/span. Three roster counts.
    const TOKENS: usize = 5 + 2 + 3;
    for text_bytes in [0, 1, 1_024, MAX_TEXT_BYTES_V1] {
        let mut text = "m".repeat(text_bytes);
        text.reserve(97);
        let module = Module::new(text);
        let wire_bytes = 20 + 4 + text_bytes + 3 * 4;
        let mut work = CanonicalKernelIrWorkBudgetV1::new(TOKENS);
        let extent = count_module_v12_wire_extent_with_work_v1(&module, &mut work).unwrap();
        assert_eq!(extent.wire_bytes(), wire_bytes);
        assert_eq!(extent.peak_auxiliary_bytes(), 0);
        assert_eq!(work.work(), TOKENS);
        let mut one_under = CanonicalKernelIrWorkBudgetV1::new(TOKENS - 1);
        assert!(matches!(
            count_module_v12_wire_extent_with_work_v1(&module, &mut one_under),
            Err(KernelIrEncodeError::WorkLimit(error)) if error.actual() == TOKENS
        ));
        assert_eq!(one_under.work(), TOKENS - 1);

        let exact_encode = TOKENS + wire_bytes + 4;
        let mut work = CanonicalKernelIrWorkBudgetV1::new(exact_encode);
        let encoded = encode_module_v12_with_work_v1(&module, &mut work).unwrap();
        assert_eq!(encoded, encode_module_v12(&module).unwrap());
        assert_eq!(encoded.len(), wire_bytes);
        assert_eq!(decode_module_v12(&encoded).unwrap(), module);
        assert_eq!(work.work(), exact_encode);
        let mut one_under = CanonicalKernelIrWorkBudgetV1::new(exact_encode - 1);
        assert!(matches!(
            encode_module_v12_with_work_v1(&module, &mut one_under),
            Err(KernelIrEncodeError::WorkLimit(error)) if error.actual() == exact_encode
        ));
        assert_eq!(one_under.work(), exact_encode - 4);
    }
}

#[test]
fn scalar_module_has_independent_literal_tokens_and_two_encoder_passes() {
    // Function: ID2, signature2, body flag1, body rosters2, block rosters3,
    // return option/tag/count3, capabilities1. Each constant: results1,
    // result ID/type3, operation tag1, constant tag/value2.
    for operations in [0, 1, 3, 2_048] {
        let mut module = scalar_module(operations);
        module.functions.reserve(37);
        module.functions[0].body.as_mut().unwrap().blocks[0]
            .operations
            .reserve(4_096);
        let tokens = 10 + 14 + 7 * operations;
        let wire_bytes = 81 + 16 * operations;
        let mut work = CanonicalKernelIrWorkBudgetV1::new(tokens);
        let extent = count_module_v12_wire_extent_with_work_v1(&module, &mut work).unwrap();
        assert_eq!(extent.wire_bytes(), wire_bytes);
        assert_eq!(work.work(), tokens);
        let mut one_under = CanonicalKernelIrWorkBudgetV1::new(tokens - 1);
        assert!(matches!(
            count_module_v12_wire_extent_with_work_v1(&module, &mut one_under),
            Err(KernelIrEncodeError::WorkLimit(error)) if error.actual() == tokens
        ));
        assert_eq!(one_under.work(), tokens - 1);

        // One function census plus a two-cell empty-entry query = three.
        const ROLE_WORK: usize = 1 + 2;
        let mut preflight_one_under = CanonicalKernelIrWorkBudgetV1::new(tokens + ROLE_WORK - 1);
        assert!(matches!(
            encode_module_v12_with_work_v1(&module, &mut preflight_one_under),
            Err(KernelIrEncodeError::WorkLimit(error)) if error.actual() == tokens + ROLE_WORK
        ));
        assert_eq!(preflight_one_under.work(), tokens + ROLE_WORK - 1);
        let exact_encode = tokens + ROLE_WORK + wire_bytes + 4;
        let mut work = CanonicalKernelIrWorkBudgetV1::new(exact_encode);
        let encoded = encode_module_v12_with_work_v1(&module, &mut work).unwrap();
        assert_eq!(encoded, encode_module_v12(&module).unwrap());
        assert_eq!(decode_module_v12(&encoded).unwrap(), module);
        assert_eq!(work.work(), exact_encode);
        let mut one_under = CanonicalKernelIrWorkBudgetV1::new(exact_encode - 1);
        assert!(matches!(
            encode_module_v12_with_work_v1(&module, &mut one_under),
            Err(KernelIrEncodeError::WorkLimit(error)) if error.actual() == exact_encode
        ));
        assert_eq!(one_under.work(), exact_encode - 4);
    }
}

#[test]
fn authority_free_extent_cannot_skip_fresh_encoder_role_or_schema_checks() {
    let mut module = scalar_module(0);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(24);
    assert_eq!(
        count_module_v12_wire_extent_with_work_v1(&module, &mut work)
            .unwrap()
            .wire_bytes(),
        81
    );
    for role in [FunctionRole::DeviceFfiExport, FunctionRole::KernelEntry] {
        module.functions[0].role = role;
        let expected_error = encode_module_v12(&module).unwrap_err();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(24);
        assert_eq!(
            count_module_v12_wire_extent_with_work_v1(&module, &mut work)
                .unwrap()
                .wire_bytes(),
            81
        );
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        assert_eq!(
            encode_module_v12_with_work_v1(&module, &mut work),
            Err(expected_error)
        );
        // Header5, ID2, function/kernel counts2, then role census/query3.
        assert_eq!(work.work(), 9 + 3);
        assert_eq!(work.failed_work(), None);
    }

    module.functions[0].role = FunctionRole::InternalHelper;
    module.functions[0].signature.parameters =
        vec![Type::Scalar(ScalarType::U32); MAX_SIGNATURE_TYPES_V1 + 1];
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    assert_eq!(
        encode_module_v12_with_work_v1(&module, &mut work),
        encode_module_v12(&module)
    );
    assert_eq!(work.work(), 10 + 3 + 2);
    assert_eq!(work.failed_work(), None);
}

#[test]
fn count_tokens_preserve_depth_boundary_and_late_error_prefixes() {
    for depth in [0, MAX_TYPE_DEPTH_V1, MAX_TYPE_DEPTH_V1 + 1] {
        let mut ty = Type::Scalar(ScalarType::U32);
        for _ in 0..depth {
            ty = Type::pointer(ty, AddressSpace::Global, AccessMode::ReadOnly);
        }
        let mut module = Module::new("m");
        module
            .functions
            .push(Function::declaration("f", Signature::new(vec![ty], vec![])));
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let counted = count_module_v12_wire_extent_with_work_v1(&module, &mut work)
            .map(KernelIrV12WireExtentV1::wire_bytes);
        let materialized = encode_module_v12(&module).map(|bytes| bytes.len());
        assert_eq!(counted, materialized);
        if depth <= MAX_TYPE_DEPTH_V1 {
            // Module10, function ID2, signature counts2, scalar type2,
            // body flag1, capabilities1. Each pointer adds tag/space/access3.
            let tokens = 18 + 3 * depth;
            assert_eq!(counted, Ok(57 + 3 * depth));
            assert_eq!(work.work(), tokens);
            let mut exact = CanonicalKernelIrWorkBudgetV1::new(tokens);
            assert!(count_module_v12_wire_extent_with_work_v1(&module, &mut exact).is_ok());
            let mut one_under = CanonicalKernelIrWorkBudgetV1::new(tokens - 1);
            assert!(matches!(
                count_module_v12_wire_extent_with_work_v1(&module, &mut one_under),
                Err(KernelIrEncodeError::WorkLimit(error)) if error.actual() == tokens
            ));
            assert_eq!(one_under.work(), tokens - 1);
        } else {
            assert!(matches!(
                counted,
                Err(KernelIrEncodeError::TypeNestingTooDeep { .. })
            ));
            assert_eq!(work.work(), 10 + 2 + 1 + 3 * depth);
            let mut encoded_work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            assert_eq!(
                encode_module_v12_with_work_v1(&module, &mut encoded_work).map(|bytes| bytes.len()),
                counted
            );
            assert_eq!(encoded_work.work(), 13 + 3 * depth + 3);
            assert_eq!(encoded_work.failed_work(), None);
        }
    }
}

#[test]
fn count_extent_checks_precede_token_charge_and_rejection_does_not_advance() {
    let module = Module::new("m".repeat(MAX_TEXT_BYTES_V1 + 1));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(5);
    assert_eq!(
        count_module_v12_wire_extent_with_work_v1(&module, &mut work)
            .map(|extent| extent.wire_bytes()),
        encode_module_v12(&module).map(|bytes| bytes.len())
    );
    assert_eq!(work.work(), 5);
    assert_eq!(work.failed_work(), None);

    let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
    let mut counter = Writer::counter(KERNEL_IR_VERSION_V12, &mut work);
    assert!(matches!(
        counter.count("roster", 2, 1),
        Err(KernelIrEncodeError::LimitExceeded {
            field: "roster",
            ..
        })
    ));
    assert!(matches!(
        counter.count_bytes(MAX_MODULE_BYTES_V1 + 1),
        Err(KernelIrEncodeError::TooLarge { .. })
    ));
    if let Some(overflow) = (u32::MAX as usize).checked_add(1) {
        assert!(matches!(
            counter.count("roster", overflow, usize::MAX),
            Err(KernelIrEncodeError::Overflow { field: "roster" })
        ));
    }
    assert_eq!(counter.length(), 0);
    drop(counter);
    assert_eq!(work.work(), 0);
    assert_eq!(work.failed_work(), None);

    let mut work = CanonicalKernelIrWorkBudgetV1::new(2);
    let mut counter = Writer::counter(KERNEL_IR_VERSION_V12, &mut work);
    counter.count_bytes(MAX_MODULE_BYTES_V1).unwrap();
    counter.count_bytes(0).unwrap();
    assert!(matches!(
        counter.count_bytes(1),
        Err(KernelIrEncodeError::TooLarge { .. })
    ));
    assert!(matches!(
        counter.count_bytes(usize::MAX),
        Err(KernelIrEncodeError::Overflow {
            field: "module length"
        })
    ));
    assert!(matches!(
        counter.count_bytes(0),
        Err(KernelIrEncodeError::WorkLimit(error)) if error.actual() == 3
    ));
    assert_eq!(counter.length(), MAX_MODULE_BYTES_V1);
    assert_eq!(counter.bytes.capacity(), 0);
    drop(counter);
    assert_eq!(work.work(), 2);
    assert_eq!(work.failed_work(), Some(3));
}

#[test]
fn signed_constants_and_assembly_immediates_preserve_exact_little_endian_bits() {
    let cases = [
        (Constant::I16(-1), vec![3, 0xff, 0xff]),
        (Constant::I16(i16::MIN), vec![3, 0, 0x80]),
        (Constant::I32(-1), vec![4, 0xff, 0xff, 0xff, 0xff]),
        (Constant::I32(i32::MIN), vec![4, 0, 0, 0, 0x80]),
        (
            Constant::I64(-1),
            vec![5, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
        ),
        (Constant::I64(i64::MIN), vec![5, 0, 0, 0, 0, 0, 0, 0, 0x80]),
    ];
    for (constant, expected) in cases {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(2);
        let mut counter = Writer::counter(KERNEL_IR_VERSION_V12, &mut work);
        encode_constant(&mut counter, &constant).unwrap();
        assert_eq!(counter.length(), expected.len());
        assert_eq!(counter.bytes.capacity(), 0);
        drop(counter);
        assert_eq!(work.work(), 2);

        let mut work = CanonicalKernelIrWorkBudgetV1::new(expected.len());
        let mut writer = Writer::new(KERNEL_IR_VERSION_V12, Some(&mut work));
        encode_constant(&mut writer, &constant).unwrap();
        assert_eq!(writer.bytes, expected);
        drop(writer);
        assert_eq!(work.work(), expected.len());

        let mut work = CanonicalKernelIrWorkBudgetV1::new(expected.len() + 2);
        let mut writer = Writer::comparing(KERNEL_IR_VERSION_V12, &expected, Some(&mut work));
        encode_constant(&mut writer, &constant).unwrap();
        assert!(matches!(
            writer.mode,
            WriterModeV1::Compare { matches: true, .. }
        ));
        drop(writer);
        assert_eq!(work.work(), expected.len() + 2);
    }

    let assembly = InlineAssembly {
        target: InlineAssemblyTarget::AmdGpuGfx942,
        source: AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32]),
        mnemonic: "x".into(),
        operands: vec![AssemblyOperand {
            constraint: AssemblyConstraint::ImmediateI32,
            kind: AssemblyOperandKind::ImmediateI32(i32::MIN),
        }],
        options: BTreeSet::new(),
        declared_effects: BTreeSet::new(),
    };
    // Target1, source spans4, mnemonic2, operand count1, operand3,
    // options/effects counts2. Wire bytes: 1+128+5+4+6+8=152.
    const TOKENS: usize = 1 + 4 + 2 + 1 + 3 + 2;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(TOKENS);
    let mut counter = Writer::counter(KERNEL_IR_VERSION_V12, &mut work);
    encode_inline_assembly(&mut counter, &assembly).unwrap();
    assert_eq!(counter.length(), 152);
    assert_eq!(counter.bytes.capacity(), 0);
    drop(counter);
    assert_eq!(work.work(), TOKENS);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(152);
    let mut writer = Writer::new(KERNEL_IR_VERSION_V12, Some(&mut work));
    encode_inline_assembly(&mut writer, &assembly).unwrap();
    assert_eq!(writer.bytes.len(), 152);
    assert_eq!(&writer.bytes[140..144], &[0, 0, 0, 0x80]);
    let expected = writer.bytes.clone();
    drop(writer);
    assert_eq!(work.work(), 152);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(152 + TOKENS);
    let mut writer = Writer::comparing(KERNEL_IR_VERSION_V12, &expected, Some(&mut work));
    encode_inline_assembly(&mut writer, &assembly).unwrap();
    assert!(matches!(
        writer.mode,
        WriterModeV1::Compare { matches: true, .. }
    ));
    drop(writer);
    assert_eq!(work.work(), 152 + TOKENS);
}

#[test]
fn memory_instance_count_keeps_producer_work_scratch_and_materialized_payload() {
    let intrinsic = MemoryIntrinsicOperation::VolatileLoad {
        pointer: ValueId(19),
        element: crate::MemoryElementType::Scalar(ScalarType::U32),
        address_space: AddressSpace::Global,
        layout: crate::MemoryLayout::new(4, 4),
        contract: crate::VolatileAccessContract::rust_allocation_load(),
    };
    const PAYLOAD: usize = 7 + 12;
    const INSTANCE: usize = crate::SEMANTIC_OPERATION_INSTANCE_HEADER_BYTES_V1 + PAYLOAD;
    const PRODUCER: usize = crate::SEMANTIC_OPERATION_INSTANCE_HEADER_BYTES_V1 + 2 * PAYLOAD;
    const EXTENT: usize = 4 + INSTANCE + 4;
    const TOKENS: usize = 3;
    let mut producer_one_under = CanonicalKernelIrWorkBudgetV1::new(PRODUCER - 1);
    let mut counter = Writer::counter(KERNEL_IR_VERSION_V12, &mut producer_one_under);
    assert!(matches!(
        encode_memory_intrinsic(&mut counter, &intrinsic),
        Err(KernelIrEncodeError::WorkLimit(error)) if error.actual() == PRODUCER
    ));
    assert_eq!(counter.length(), 0);
    assert_eq!(counter.peak_auxiliary_bytes, 0);
    assert_eq!(counter.bytes.capacity(), 0);
    drop(counter);
    assert_eq!(producer_one_under.work(), 0);

    let mut work = CanonicalKernelIrWorkBudgetV1::new(PRODUCER + TOKENS);
    let mut counter = Writer::counter(KERNEL_IR_VERSION_V12, &mut work);
    encode_memory_intrinsic(&mut counter, &intrinsic).unwrap();
    assert_eq!(counter.length(), EXTENT);
    assert_eq!(counter.peak_auxiliary_bytes, PRODUCER);
    assert_eq!(counter.bytes.capacity(), 0);
    drop(counter);
    assert_eq!(work.work(), PRODUCER + TOKENS);

    let mut one_under = CanonicalKernelIrWorkBudgetV1::new(PRODUCER + TOKENS - 1);
    let mut counter = Writer::counter(KERNEL_IR_VERSION_V12, &mut one_under);
    assert!(matches!(
        encode_memory_intrinsic(&mut counter, &intrinsic),
        Err(KernelIrEncodeError::WorkLimit(error)) if error.actual() == PRODUCER + TOKENS
    ));
    assert_eq!(counter.length(), EXTENT - 4);
    assert_eq!(counter.bytes.capacity(), 0);
    drop(counter);
    assert_eq!(one_under.work(), PRODUCER + TOKENS - 1);

    let mut work = CanonicalKernelIrWorkBudgetV1::new(PRODUCER + EXTENT);
    let mut writer = Writer::new(KERNEL_IR_VERSION_V12, Some(&mut work));
    encode_memory_intrinsic(&mut writer, &intrinsic).unwrap();
    assert_eq!(writer.bytes.len(), EXTENT);
    assert_eq!(
        &writer.bytes[4..4 + INSTANCE],
        encode_semantic_operation_instance_id(intrinsic.semantic_instance_id_v1())
    );
    let mut expected = writer.bytes.clone();
    drop(writer);
    assert_eq!(work.work(), PRODUCER + EXTENT);
    for corrupt in [false, true] {
        if corrupt {
            expected[4] ^= 1;
        }
        let mut work = CanonicalKernelIrWorkBudgetV1::new(PRODUCER + EXTENT + TOKENS);
        let mut writer = Writer::comparing(KERNEL_IR_VERSION_V12, &expected, Some(&mut work));
        encode_memory_intrinsic(&mut writer, &intrinsic).unwrap();
        assert!(matches!(writer.mode, WriterModeV1::Compare { matches, .. } if matches != corrupt));
        assert_eq!(writer.bytes.capacity(), 0);
        drop(writer);
        assert_eq!(work.work(), PRODUCER + EXTENT + TOKENS);
    }

    for instances in [1, 3, 257] {
        let mut module = scalar_module(0);
        module.functions[0].body.as_mut().unwrap().blocks[0].operations = (0..instances)
            .map(|index| {
                Operation::effect_free(
                    ValueDef::new(ValueId(index as u32), Type::Scalar(ScalarType::U32)),
                    OperationKind::MemoryIntrinsic(intrinsic),
                )
            })
            .collect();
        // Result count/ID/type4, operation tag1, instance extent3, producer E.
        let count_work = 24 + instances * (5 + TOKENS + PRODUCER);
        let wire_bytes = 81 + instances * (11 + EXTENT);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(count_work);
        let extent = count_module_v12_wire_extent_with_work_v1(&module, &mut work).unwrap();
        assert_eq!(extent.wire_bytes(), wire_bytes);
        assert_eq!(extent.peak_auxiliary_bytes(), PRODUCER);
        assert_eq!(work.work(), count_work);
        let mut one_under = CanonicalKernelIrWorkBudgetV1::new(count_work - 1);
        assert!(matches!(
            count_module_v12_wire_extent_with_work_v1(&module, &mut one_under),
            Err(KernelIrEncodeError::WorkLimit(error)) if error.actual() == count_work
        ));
        assert_eq!(one_under.work(), count_work - 1);

        let encoder_work = count_work + 3 + wire_bytes + instances * PRODUCER + 4;
        let mut work = CanonicalKernelIrWorkBudgetV1::new(encoder_work);
        let encoded = encode_module_v12_with_work_v1(&module, &mut work).unwrap();
        assert_eq!(encoded, encode_module_v12(&module).unwrap());
        assert_eq!(decode_module_v12(&encoded).unwrap(), module);
        assert_eq!(work.work(), encoder_work);
    }
}
