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

fn wire_cause_borrow<T: std::error::Error + 'static>(parent: &dyn std::error::Error, child: &T) {
    assert!(std::ptr::eq(
        parent.source().unwrap().downcast_ref::<T>().unwrap(),
        child
    ));
}

#[test]
fn wire_cause_links_preserve_direct_work_nested_encode_and_all_resources() {
    use crate::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    };
    let mut work = CanonicalKernelIrWorkBudgetV1::new(3);
    let mut budget = Budget::new(&mut work, 5);
    let work_error = budget.charge_work(4).unwrap_err();
    let storage_error = budget.reserve_storage(6).unwrap_err();
    let accounting = budget.release_storage(1).unwrap_err();
    assert_eq!(accounting, Resource::Accounting);
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (0, 0, 0)
    );
    assert_eq!(budget.failed_storage(), Some(6));
    drop(budget);
    assert_eq!(work.failed_work(), Some(4));
    // These wrappers test diagnostic structure, not decoder execution.
    for resource in [
        work_error,
        storage_error,
        accounting,
        Resource::Allocation,
        Resource::Arithmetic,
    ] {
        let error = KernelIrDecodeError::Resource(resource);
        let KernelIrDecodeError::Resource(child) = &error else {
            unreachable!()
        };
        wire_cause_borrow(&error, child);
        assert_eq!(*child, resource);
        match child {
            Resource::Work(leaf) => wire_cause_borrow(child, leaf),
            Resource::Storage(leaf) => wire_cause_borrow(child, leaf),
            Resource::Allocation | Resource::Accounting | Resource::Arithmetic => {
                assert!(std::error::Error::source(child).is_none())
            }
        }
    }
    let Resource::Work(leaf) = work_error else {
        unreachable!()
    };
    let error = KernelIrEncodeError::WorkLimit(leaf);
    let KernelIrEncodeError::WorkLimit(child) = &error else {
        unreachable!()
    };
    wire_cause_borrow(&error, child);
    let error = KernelIrDecodeError::WorkLimit(leaf);
    let KernelIrDecodeError::WorkLimit(child) = &error else {
        unreachable!()
    };
    wire_cause_borrow(&error, child);
    let error = KernelIrDecodeError::Encode(KernelIrEncodeError::WorkLimit(leaf));
    let KernelIrDecodeError::Encode(child) = &error else {
        unreachable!()
    };
    wire_cause_borrow(&error, child);
    let KernelIrEncodeError::WorkLimit(grandchild) = child else {
        unreachable!()
    };
    wire_cause_borrow(child, grandchild);
    assert_eq!((grandchild.actual(), grandchild.limit()), (4, 3));
    let error = KernelIrDecodeError::Encode(KernelIrEncodeError::Allocation);
    let KernelIrDecodeError::Encode(child) = &error else {
        unreachable!()
    };
    wire_cause_borrow(&error, child);
    assert!(std::error::Error::source(child).is_none());
}

#[test]
fn wire_cause_links_keep_every_structural_marker_terminal() {
    for error in [
        KernelIrEncodeError::TooLarge { max: 1 },
        KernelIrEncodeError::LimitExceeded {
            field: "fixture",
            actual: 2,
            max: 1,
        },
        KernelIrEncodeError::TypeNestingTooDeep { max: 1 },
        KernelIrEncodeError::Overflow { field: "fixture" },
        KernelIrEncodeError::UnsupportedInVersion {
            version: 1,
            feature: "fixture",
        },
        KernelIrEncodeError::NonCanonical { field: "fixture" },
        KernelIrEncodeError::Allocation,
    ] {
        assert!(std::error::Error::source(&error).is_none());
    }
    for error in [
        KernelIrDecodeError::TooLarge { max: 1 },
        KernelIrDecodeError::InvalidMagic,
        KernelIrDecodeError::UnknownVersion(99),
        KernelIrDecodeError::UnsupportedFlags(1),
        KernelIrDecodeError::InvalidLength { declared: 1 },
        KernelIrDecodeError::Truncated,
        KernelIrDecodeError::TrailingBytes,
        KernelIrDecodeError::ReservedNonZero { field: "fixture" },
        KernelIrDecodeError::UnknownTag {
            kind: "fixture",
            tag: 255,
        },
        KernelIrDecodeError::InvalidUtf8 { field: "fixture" },
        KernelIrDecodeError::LimitExceeded {
            field: "fixture",
            actual: 2,
            max: 1,
        },
        KernelIrDecodeError::TypeNestingTooDeep { max: 1 },
        KernelIrDecodeError::NonCanonical,
        KernelIrDecodeError::InvalidSemanticOperationInstance,
    ] {
        assert!(std::error::Error::source(&error).is_none());
    }
    let old = encode_module_v11(&Module::new("m")).unwrap();
    // The legacy reader accepts older versions; exact allocation admission does not.
    assert_eq!(decode_module_v12(&old).unwrap(), Module::new("m"));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000);
    let mut budget = crate::CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
    let error = decode_module_v12_with_allocation_budget_v1(&old, &mut budget).unwrap_err();
    assert_eq!(error, KernelIrDecodeError::UnknownVersion(11));
    assert!(std::error::Error::source(&error).is_none());
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (10, 0, 0)
    );
    assert_eq!(budget.failed_storage(), None);
    drop(budget);
    assert_eq!(work.failed_work(), None);
}

#[test]
fn reached_wire_work_causes_preserve_distinct_encoder_decoder_and_reencode_phases() {
    const PRIOR: usize = 11;
    let module = Module::new("m");
    let bytes = encode_module_v12(&module).unwrap();
    assert_eq!(bytes.len(), 37);
    assert_eq!(decode_module_v12(&bytes).unwrap(), module);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(PRIOR);
    work.charge_work(PRIOR).unwrap();
    let error = encode_module_v12_with_work_v1(&module, &mut work).unwrap_err();
    let KernelIrEncodeError::WorkLimit(child) = &error else {
        panic!("{error:?}")
    };
    wire_cause_borrow(&error, child);
    // The counting encoder admits one schema token before its magic extent.
    assert_eq!((child.actual(), child.limit()), (PRIOR + 1, PRIOR));
    assert_eq!(work.work(), PRIOR);
    assert_eq!(work.failed_work(), Some(PRIOR + 1));

    for reencode in [false, true] {
        // Decode37 + UTF8/copy2 precedes the first comparison writer's eight bytes.
        let allowance = if reencode { 39 } else { 7 };
        let mut work = CanonicalKernelIrWorkBudgetV1::new(PRIOR + allowance);
        work.charge_work(PRIOR).unwrap();
        let error = decode_module_v12_with_work_v1(&bytes, &mut work).unwrap_err();
        let child = if reencode {
            let KernelIrDecodeError::Encode(encoded) = &error else {
                panic!("{error:?}")
            };
            wire_cause_borrow(&error, encoded);
            let KernelIrEncodeError::WorkLimit(child) = encoded else {
                panic!("{encoded:?}")
            };
            wire_cause_borrow(encoded, child);
            child
        } else {
            let KernelIrDecodeError::WorkLimit(child) = &error else {
                panic!("{error:?}")
            };
            wire_cause_borrow(&error, child);
            child
        };
        let accepted = PRIOR + if reencode { 39 } else { 0 };
        assert_eq!(
            (child.actual(), child.limit()),
            (accepted + 8, PRIOR + allowance)
        );
        assert_eq!(work.work(), accepted);
        assert_eq!(work.failed_work(), Some(accepted + 8));
    }
}

#[test]
fn reached_allocation_budgeted_decoder_storage_cause_keeps_literal_prefix_and_floor() {
    use crate::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    };
    const PRIOR: usize = 11;
    const FLOOR: usize = 7;
    let bytes = encode_module_v12(&Module::new("m")).unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    work.charge_work(PRIOR).unwrap();
    let mut budget = Budget::new(&mut work, FLOOR);
    budget.reserve_storage(FLOOR).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let error = decode_module_v12_with_allocation_budget_v1(&bytes, &mut budget).unwrap_err();
    let KernelIrDecodeError::Resource(child) = &error else {
        panic!("{error:?}")
    };
    wire_cause_borrow(&error, child);
    let Resource::Storage(leaf) = child else {
        panic!("{child:?}")
    };
    wire_cause_borrow(child, leaf);
    assert_eq!((leaf.actual(), leaf.limit()), (FLOOR + 1, FLOOR));
    // Header20, text length4, text byte1 and UTF8/copy2 precede the first allocation.
    assert_eq!(budget.work(), PRIOR + 27);
    assert_eq!((budget.storage(), budget.peak_storage()), (FLOOR, FLOOR));
    assert_eq!(budget.failed_storage(), Some(FLOOR + 1));
    assert!(budget.work_ledger_identity_v1() == ledger);
    drop(budget);
    assert_eq!(work.failed_work(), None);
}
