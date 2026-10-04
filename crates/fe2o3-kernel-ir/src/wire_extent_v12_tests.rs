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
fn exact_wire_extent_has_literal_work_and_one_under_boundaries() {
    const HEADER: usize = 20;
    const MODULE_ROSTERS: usize = 3 * 4;
    // Five header fields, two ID fields, and three roster counts.
    const COUNT_TOKENS: usize = 5 + 2 + 3;
    for text_bytes in [0, 1, 1_024] {
        let module = Module::new("m".repeat(text_bytes));
        let exact = HEADER + 4 + text_bytes + MODULE_ROSTERS;
        let mut budget = CanonicalKernelIrWorkBudgetV1::new(COUNT_TOKENS);
        assert_eq!(
            count_module_v12_wire_extent_with_work_v1(&module, &mut budget),
            Ok(KernelIrV12WireExtentV1 {
                wire_bytes: exact,
                peak_auxiliary_bytes: 0
            })
        );
        assert_eq!(budget.work(), COUNT_TOKENS);
        assert_eq!(encode_module_v12(&module).unwrap().len(), exact);
        let mut one_under = CanonicalKernelIrWorkBudgetV1::new(COUNT_TOKENS - 1);
        assert!(matches!(
            count_module_v12_wire_extent_with_work_v1(&module, &mut one_under),
            Err(KernelIrEncodeError::WorkLimit(error))
                if error.actual() == COUNT_TOKENS && error.limit() == COUNT_TOKENS - 1
        ));
        assert_eq!(one_under.work(), COUNT_TOKENS - 1);
        assert_eq!(one_under.failed_work(), Some(COUNT_TOKENS));
    }
}

#[test]
fn exact_wire_extent_tracks_scalar_rows_and_ignores_spare_capacity() {
    const MODULE: usize = 20 + 4 + 1 + 3 * 4;
    const FUNCTION: usize = (4 + 1) + 8 + 1 + 8 + 12 + (1 + 1 + 4) + 4;
    const CONSTANT: usize = 4 + (4 + 2) + 1 + 1 + 4;
    for operations in [0, 1, 3, 2_048] {
        let mut module = scalar_module(operations);
        module.functions.reserve(37);
        module.functions[0].body.as_mut().unwrap().blocks[0]
            .operations
            .reserve(4_096);
        let expected = MODULE + FUNCTION + operations * CONSTANT;
        // Module/function/block/return contribute 24 tokens; each constant adds seven.
        let count_work = 24 + 7 * operations;
        let mut work = CanonicalKernelIrWorkBudgetV1::new(count_work);
        assert_eq!(
            count_module_v12_wire_extent_with_work_v1(&module, &mut work),
            Ok(KernelIrV12WireExtentV1 {
                wire_bytes: expected,
                peak_auxiliary_bytes: 0
            })
        );
        assert_eq!(work.work(), count_work);
        assert_eq!(encode_module_v12(&module).unwrap().len(), expected);
    }
}

#[test]
fn wire_extent_grants_no_function_role_or_semantic_authority() {
    let mut module = scalar_module(0);
    module.functions[0].role = FunctionRole::DeviceFfiExport;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(81);
    assert_eq!(
        count_module_v12_wire_extent_with_work_v1(&module, &mut work),
        Ok(KernelIrV12WireExtentV1 {
            wire_bytes: 81,
            peak_auxiliary_bytes: 0
        })
    );
    assert!(matches!(
        encode_module_v12(&module),
        Err(KernelIrEncodeError::UnsupportedInVersion {
            version: KERNEL_IR_VERSION_V12,
            feature: "device-FFI export function roles",
        })
    ));
    module.functions[0].role = FunctionRole::KernelEntry;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(81);
    assert_eq!(
        count_module_v12_wire_extent_with_work_v1(&module, &mut work),
        Ok(KernelIrV12WireExtentV1 {
            wire_bytes: 81,
            peak_auxiliary_bytes: 0
        })
    );
    assert!(matches!(
        encode_module_v12(&module),
        Err(KernelIrEncodeError::NonCanonical {
            field: "function role does not match the V1/V2 body and kernel records",
        })
    ));
}

#[test]
fn wire_extent_preserves_structural_failures_and_accepted_prefix() {
    let module = Module::new("m".repeat(MAX_TEXT_BYTES_V1 + 1));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let error = count_module_v12_wire_extent_with_work_v1(&module, &mut work).unwrap_err();
    assert_eq!(error, encode_module_v12(&module).unwrap_err());
    assert_eq!(work.work(), 5);
    assert_eq!(work.failed_work(), None);

    for depth in [MAX_TYPE_DEPTH_V1, MAX_TYPE_DEPTH_V1 + 1] {
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
        if depth == MAX_TYPE_DEPTH_V1 {
            const BASE: usize = (20 + 4 + 1 + 3 * 4) + (4 + 1) + 8 + 2 + 1 + 4;
            assert_eq!(counted, Ok(BASE + 3 * depth));
            assert_eq!(work.work(), 18 + 3 * depth);
            assert_eq!(encode_module_v12(&module).unwrap().len(), BASE + 3 * depth);
        } else {
            assert_eq!(counted, encode_module_v12(&module).map(|bytes| bytes.len()));
            assert_eq!(work.work(), 13 + 3 * depth);
        }
    }
}

#[test]
fn memory_intrinsic_counter_uses_exact_producer_extent_without_byte_owners() {
    let intrinsic = MemoryIntrinsicOperation::VolatileLoad {
        pointer: ValueId(19),
        element: crate::MemoryElementType::Scalar(ScalarType::U32),
        address_space: AddressSpace::Global,
        layout: crate::MemoryLayout::new(4, 4),
        contract: crate::VolatileAccessContract::rust_allocation_load(),
    };
    const PAYLOAD: usize = 7 + 12;
    const INSTANCE: usize = crate::SEMANTIC_OPERATION_INSTANCE_HEADER_BYTES_V1 + PAYLOAD;
    const ENCODING_WORK: usize = crate::SEMANTIC_OPERATION_INSTANCE_HEADER_BYTES_V1 + 2 * PAYLOAD;
    const EXTENT: usize = 4 + INSTANCE + 4;
    // Both modes retain the real instance producer; only Count uses three extent tokens.
    const COUNT_WORK: usize = ENCODING_WORK + 3;
    const MATERIALIZE_WORK: usize = ENCODING_WORK + EXTENT;
    let mut budget = CanonicalKernelIrWorkBudgetV1::new(COUNT_WORK);
    let mut counter = Writer::counter(KERNEL_IR_VERSION_V12, &mut budget);
    encode_memory_intrinsic(&mut counter, &intrinsic).unwrap();
    assert_eq!(counter.length(), EXTENT);
    assert_eq!(counter.peak_auxiliary_bytes, ENCODING_WORK);
    assert_eq!(counter.bytes.capacity(), 0);
    drop(counter);
    assert_eq!(budget.work(), COUNT_WORK);

    let mut materialized_work = CanonicalKernelIrWorkBudgetV1::new(MATERIALIZE_WORK);
    let mut writer =
        Writer::with_exact_capacity(KERNEL_IR_VERSION_V12, EXTENT, &mut materialized_work).unwrap();
    encode_memory_intrinsic(&mut writer, &intrinsic).unwrap();
    assert_eq!(writer.bytes.len(), EXTENT);
    drop(writer);
    assert_eq!(materialized_work.work(), MATERIALIZE_WORK);

    let mut one_under = CanonicalKernelIrWorkBudgetV1::new(COUNT_WORK - 1);
    let mut counter = Writer::counter(KERNEL_IR_VERSION_V12, &mut one_under);
    assert!(matches!(
        encode_memory_intrinsic(&mut counter, &intrinsic),
        Err(KernelIrEncodeError::WorkLimit(error)) if error.actual() == COUNT_WORK
    ));
    assert_eq!(counter.length(), EXTENT - 4);
    assert_eq!(counter.bytes.capacity(), 0);
    drop(counter);
    assert_eq!(one_under.work(), COUNT_WORK - 1);

    for instances in [1, 3, 257] {
        let mut module = scalar_module(0);
        for index in 0..instances {
            module.functions[0].body.as_mut().unwrap().blocks[0]
                .operations
                .push(Operation::effect_free(
                    ValueDef::new(
                        ValueId(u32::try_from(index).unwrap()),
                        Type::Scalar(ScalarType::U32),
                    ),
                    OperationKind::MemoryIntrinsic(intrinsic),
                ));
        }
        let wire = 81 + instances * (4 + (4 + 2) + 1 + EXTENT);
        let exact_work = 24 + instances * (8 + ENCODING_WORK);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(exact_work);
        let extent = count_module_v12_wire_extent_with_work_v1(&module, &mut work).unwrap();
        assert_eq!(extent.wire_bytes(), wire);
        assert_eq!(extent.peak_auxiliary_bytes(), ENCODING_WORK);
        assert_eq!(encode_module_v12(&module).unwrap().len(), wire);
        assert_eq!(work.work(), exact_work);
        let mut one_under = CanonicalKernelIrWorkBudgetV1::new(exact_work - 1);
        assert!(matches!(
            count_module_v12_wire_extent_with_work_v1(&module, &mut one_under),
            Err(KernelIrEncodeError::WorkLimit(error)) if error.actual() == exact_work
        ));
        assert_eq!(one_under.work(), exact_work - 1);
    }
}

#[test]
fn counter_extent_failure_never_advances_or_omits_materialized_bytes() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1);
    let mut counter = Writer::counter(KERNEL_IR_VERSION_V12, &mut work);
    counter.count_bytes(2).unwrap();
    assert!(matches!(
        counter.count_bytes(1),
        Err(KernelIrEncodeError::WorkLimit(_))
    ));
    assert_eq!(counter.length(), 2);
    assert_eq!(counter.bytes.capacity(), 0);

    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut writer = Writer::with_exact_capacity(KERNEL_IR_VERSION_V12, 4, &mut work).unwrap();
    assert!(matches!(
        writer.count_bytes(4),
        Err(KernelIrEncodeError::NonCanonical { .. })
    ));
    assert_eq!(writer.length(), 0);
    assert!(writer.bytes.is_empty());
    drop(writer);
    assert_eq!(work.work(), 0);

    let mut counter = Writer::counter(KERNEL_IR_VERSION_V12, &mut work);
    assert!(matches!(
        counter.count_bytes(MAX_MODULE_BYTES_V1 + 1),
        Err(KernelIrEncodeError::TooLarge {
            max: MAX_MODULE_BYTES_V1
        })
    ));
    assert_eq!(counter.length(), 0);
    counter.count_bytes(1).unwrap();
    assert!(matches!(
        counter.count_bytes(usize::MAX),
        Err(KernelIrEncodeError::Overflow {
            field: "module length"
        })
    ));
    assert_eq!(counter.length(), 1);
}
