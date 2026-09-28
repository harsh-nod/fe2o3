use super::*;
use fe2o3_kernel_ir::{
    AccessMode, BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1, Function, Operation, Signature,
    StorageLayoutLimitsV1, Terminator, ValueDef,
};

const WORK: usize = 50_000_000;
const STORAGE: usize = 64 * 1024 * 1024;
const LAYOUTS: StorageLayoutLimitsV1 = StorageLayoutLimitsV1 {
    rows: 16,
    edges: 32,
    containment_depth: 8,
    object_bytes: 4096,
};

fn branch(target: u32) -> Terminator {
    Terminator::Branch {
        target: BlockId(target),
        arguments: vec![],
    }
}
fn conditional(left: u32, right: u32) -> Terminator {
    Terminator::ConditionalBranch {
        condition: ValueId(900),
        then_target: BlockId(left),
        then_arguments: vec![],
        else_target: BlockId(right),
        else_arguments: vec![],
    }
}
fn ret() -> Terminator {
    Terminator::Return { values: vec![] }
}
fn block(id: u32, operations: Vec<Operation>, terminator: Terminator) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(id));
    block.operations = operations;
    block.terminator = Some(terminator);
    block
}
fn allocation(alignment: u32) -> Operation {
    Operation::effect_free(
        ValueDef::new(
            ValueId(10),
            Type::pointer(
                Type::StorageObject(StorageLayoutIdV1(0)),
                AddressSpace::Private,
                AccessMode::ReadWrite,
            ),
        ),
        OperationKind::Alloca {
            element: Type::StorageObject(StorageLayoutIdV1(0)),
            count: None,
            address_space: AddressSpace::Private,
            alignment,
        },
    )
}
fn write(alignment: u32) -> Operation {
    Operation::new(
        vec![],
        OperationKind::Storage(StorageOperationV1::WriteValue {
            address: ValueId(10),
            value: ValueId(20),
            access: MemoryAccess::new(AddressSpace::Private, alignment),
        }),
    )
}
fn read(scalar: ScalarType, alignment: u32) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(40), Type::Scalar(scalar)),
        OperationKind::Storage(StorageOperationV1::ReadValue {
            address: ValueId(10),
            access: MemoryAccess::new(AddressSpace::Private, alignment),
        }),
    )
}
fn fixture(scalar: ScalarType, bytes: u64, alignment: u32, blocks: Vec<BasicBlock>) -> Module {
    let mut module = Module::new("neutral-typed-scalar-private-memory");
    module.storage_layouts.push(StorageLayoutV1 {
        size: bytes,
        alignment,
        kind: StorageLayoutKindV1::Scalar(scalar),
    });
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(vec![Type::Scalar(scalar), Type::BOOL], vec![]),
        vec![ValueId(20), ValueId(900)],
        blocks,
    ));
    module
}
fn local(scalar: ScalarType, bytes: u64, alignment: u32) -> Module {
    fixture(
        scalar,
        bytes,
        alignment,
        vec![block(
            100,
            vec![
                allocation(alignment),
                write(alignment),
                read(scalar, alignment),
            ],
            ret(),
        )],
    )
}
fn with_inventory<T>(
    module: &Module,
    run: impl FnOnce(&CanonicalKirInventoryV18<'_>, usize) -> T,
) -> T {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(23).unwrap();
    let (owner, receipt) =
        VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
            module,
            LAYOUTS,
            &mut budget,
        )
        .unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let (inventory, receipt) = CanonicalKirInventoryV18::derive_v18(&owner, &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    run(&inventory, budget.storage())
}

fn exercise(
    inventory: &CanonicalKirInventoryV18<'_>,
    floor: usize,
    work_limit: usize,
    storage_limit: usize,
    max_cells: usize,
    expected: &[Option<usize>],
) -> (R<()>, usize, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(floor).unwrap();
    let result = check_canonical_kir_private_memory_v18(
        inventory,
        CanonicalKirPrivateMemoryLimitsV1 { max_cells },
        &mut budget,
    );
    assert_eq!(budget.storage(), floor);
    let result = match result {
        Ok((proof, receipt)) => {
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            assert!(proof.is_for(inventory));
            assert!(std::ptr::eq(proof.inventory().owner(), inventory.owner()));
            assert!(!proof.grants_authority());
            assert_eq!(proof.latest_stores(), expected);
            drop(proof);
            budget.release_storage(receipt.retained_storage()).unwrap();
            Ok(())
        }
        Err(error) => Err(error),
    };
    assert_eq!(budget.storage(), floor);
    let peak = budget.peak_storage();
    drop(budget);
    (result, work.work(), peak)
}

#[test]
fn typed_private_all_scalar_layouts_and_index_widths_keep_exact_owner_and_store() {
    let fixed = [
        ScalarType::Bool,
        ScalarType::I8,
        ScalarType::I16,
        ScalarType::I32,
        ScalarType::I64,
        ScalarType::I128,
        ScalarType::U8,
        ScalarType::U16,
        ScalarType::U32,
        ScalarType::U64,
        ScalarType::U128,
        ScalarType::F16,
        ScalarType::Bf16,
        ScalarType::F32,
        ScalarType::F64,
    ];
    for scalar in fixed {
        let bytes = u64::from(scalar.bit_width().unwrap().div_ceil(8));
        for alignment in [1, bytes as u32] {
            with_inventory(&local(scalar, bytes, alignment), |inventory, floor| {
                assert!(
                    exercise(inventory, floor, WORK, STORAGE, 1, &[None, None, Some(1)])
                        .0
                        .is_ok()
                );
                let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
                let mut budget = Budget::new(&mut work, STORAGE);
                budget.reserve_storage(floor).unwrap();
                let (other, receipt) =
                    CanonicalKirInventoryV18::derive_v18(inventory.owner(), &mut budget).unwrap();
                let other_credit = receipt.retained_storage();
                budget.reserve_storage(other_credit).unwrap();
                let (proof, receipt) = check_canonical_kir_private_memory_v18(
                    inventory,
                    CanonicalKirPrivateMemoryLimitsV1 { max_cells: 1 },
                    &mut budget,
                )
                .unwrap();
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                assert!(!proof.is_for(&other));
                let address = proof.address(2).unwrap();
                assert_eq!(
                    (address.stride(), address.length(), address.offset()),
                    (bytes as usize, 1, 0)
                );
                drop(proof);
                budget.release_storage(receipt.retained_storage()).unwrap();
                drop(other);
                budget.release_storage(other_credit).unwrap();
                assert_eq!(budget.storage(), floor);
            });
        }
    }
    for bytes in [1, 2, 4, 8, 16] {
        with_inventory(&local(ScalarType::Index, bytes, 1), |inventory, floor| {
            assert!(
                exercise(inventory, floor, WORK, STORAGE, 1, &[None, None, Some(1)])
                    .0
                    .is_ok()
            );
        });
    }
}

#[test]
fn typed_private_constant_extents_are_positive_bounded_and_not_dynamic() {
    for (constant, ty, length) in [
        (Constant::Index(3), Type::INDEX, 3),
        (Constant::U32(7), Type::Scalar(ScalarType::U32), 7),
        (Constant::I64(2), Type::Scalar(ScalarType::I64), 2),
    ] {
        let mut module = local(ScalarType::U32, 4, 4);
        let ops = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
        if let OperationKind::Alloca { count, .. } = &mut ops[0].kind {
            *count = Some(ValueId(30));
        }
        ops.insert(
            0,
            Operation::effect_free(
                ValueDef::new(ValueId(30), ty),
                OperationKind::Constant(constant),
            ),
        );
        with_inventory(&module, |inventory, floor| {
            assert!(
                exercise(
                    inventory,
                    floor,
                    WORK,
                    STORAGE,
                    length,
                    &[None, None, None, Some(2)]
                )
                .0
                .is_ok()
            );
            assert!(matches!(
                exercise(inventory, floor, WORK, STORAGE, length - 1, &[]).0,
                Err(Error::Unsupported {
                    detail: "bounded nonzero allocation extent",
                    ..
                })
            ));
        });
    }
    for count in [0i64, -1] {
        let mut module = local(ScalarType::U32, 4, 4);
        let ops = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
        if let OperationKind::Alloca { count, .. } = &mut ops[0].kind {
            *count = Some(ValueId(30));
        }
        ops.insert(
            0,
            Operation::effect_free(
                ValueDef::new(ValueId(30), Type::Scalar(ScalarType::I64)),
                OperationKind::Constant(Constant::I64(count)),
            ),
        );
        with_inventory(&module, |inventory, floor| {
            assert!(matches!(
                exercise(inventory, floor, WORK, STORAGE, 16, &[]).0,
                Err(Error::Unsupported { .. })
            ))
        });
    }
    let mut module = local(ScalarType::U32, 4, 4);
    if let OperationKind::Alloca { count, .. } =
        &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations[0].kind
    {
        *count = Some(ValueId(20));
    }
    with_inventory(&module, |inventory, floor| {
        assert!(matches!(
            exercise(inventory, floor, WORK, STORAGE, 16, &[]).0,
            Err(Error::Unsupported {
                detail: "constant allocation extent",
                ..
            })
        ))
    });
}

#[test]
fn typed_private_cfg_joins_exact_store_occurrences_not_equal_written_values() {
    let positive = fixture(
        ScalarType::U32,
        4,
        4,
        vec![
            block(100, vec![allocation(4), write(4)], conditional(200, 300)),
            block(200, vec![], branch(400)),
            block(300, vec![], branch(400)),
            block(400, vec![read(ScalarType::U32, 4)], ret()),
        ],
    );
    with_inventory(&positive, |inventory, floor| {
        assert!(
            exercise(inventory, floor, WORK, STORAGE, 1, &[None, None, Some(1)])
                .0
                .is_ok()
        )
    });
    for both in [false, true] {
        let mut mutant = positive.clone();
        let blocks = &mut mutant.functions[0].body.as_mut().unwrap().blocks;
        blocks[0].operations.pop();
        blocks[1].operations.push(write(4));
        if both {
            blocks[2].operations.push(write(4));
        }
        with_inventory(&mutant, |inventory, floor| {
            assert!(matches!(
                exercise(inventory, floor, WORK, STORAGE, 1, &[]).0,
                Err(Error::Unsupported { .. })
            ))
        });
    }
}

#[test]
fn typed_private_loop_fixed_point_and_allocation_reentry_reset_are_mandatory() {
    let stable = fixture(
        ScalarType::U32,
        4,
        4,
        vec![
            block(100, vec![allocation(4), write(4)], branch(200)),
            block(200, vec![read(ScalarType::U32, 4)], conditional(300, 400)),
            block(300, vec![], branch(200)),
            block(400, vec![], ret()),
        ],
    );
    with_inventory(&stable, |inventory, floor| {
        assert!(
            exercise(inventory, floor, WORK, STORAGE, 1, &[None, None, Some(1)])
                .0
                .is_ok()
        )
    });
    let mut clobber = stable.clone();
    clobber.functions[0].body.as_mut().unwrap().blocks[2]
        .operations
        .push(write(4));
    with_inventory(&clobber, |inventory, floor| {
        assert!(matches!(
            exercise(inventory, floor, WORK, STORAGE, 1, &[]).0,
            Err(Error::Unsupported { .. })
        ))
    });
    let reentry = fixture(
        ScalarType::U32,
        4,
        4,
        vec![
            block(100, vec![], branch(200)),
            block(200, vec![allocation(4), write(4)], branch(300)),
            block(300, vec![read(ScalarType::U32, 4)], conditional(200, 400)),
            block(400, vec![], ret()),
        ],
    );
    with_inventory(&reentry, |inventory, floor| {
        assert!(
            exercise(inventory, floor, WORK, STORAGE, 1, &[None, None, Some(1)])
                .0
                .is_ok()
        )
    });
    let mut stale = reentry;
    let blocks = &mut stale.functions[0].body.as_mut().unwrap().blocks;
    blocks[1].operations.pop();
    blocks[2].operations.push(write(4));
    with_inventory(&stale, |inventory, floor| {
        assert!(matches!(
            exercise(inventory, floor, WORK, STORAGE, 1, &[]).0,
            Err(Error::Unsupported { .. })
        ))
    });
}

#[test]
fn typed_private_volatile_workgroup_and_pointer_escape_remain_closed() {
    for mode in 0..4 {
        let mut module = local(ScalarType::U32, 4, 4);
        let function = &mut module.functions[0];
        let block = &mut function.body.as_mut().unwrap().blocks[0];
        match mode {
            0 | 1 => {
                let OperationKind::Storage(ref mut operation) = block.operations[mode + 1].kind
                else {
                    unreachable!()
                };
                match operation {
                    StorageOperationV1::ReadValue { access, .. }
                    | StorageOperationV1::WriteValue { access, .. } => access.volatile = true,
                    _ => unreachable!(),
                }
            }
            2 => {
                let Type::Pointer(pointer) = &mut block.operations[0].results[0].ty else {
                    unreachable!()
                };
                pointer.address_space = AddressSpace::Workgroup;
                let OperationKind::Alloca { address_space, .. } = &mut block.operations[0].kind
                else {
                    unreachable!()
                };
                *address_space = AddressSpace::Workgroup;
                for operation in &mut block.operations[1..] {
                    match &mut operation.kind {
                        OperationKind::Storage(StorageOperationV1::ReadValue {
                            access, ..
                        })
                        | OperationKind::Storage(StorageOperationV1::WriteValue {
                            access, ..
                        }) => access.address_space = AddressSpace::Workgroup,
                        _ => unreachable!(),
                    }
                }
            }
            3 => {
                function
                    .signature
                    .results
                    .push(block.operations[0].results[0].ty.clone());
                block.terminator = Some(Terminator::Return {
                    values: vec![ValueId(10)],
                });
            }
            _ => unreachable!(),
        }
        with_inventory(&module, |inventory, floor| {
            assert!(matches!(
                exercise(inventory, floor, WORK, STORAGE, 1, &[]).0,
                Err(Error::Unsupported { .. })
            ))
        });
    }
}

#[test]
fn typed_private_aggregate_storage_is_not_silently_scalarized() {
    let mut module = local(ScalarType::U32, 4, 4);
    module.storage_layouts[0] = StorageLayoutV1 {
        size: 0,
        alignment: 1,
        kind: StorageLayoutKindV1::Record(Box::new([])),
    };
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .truncate(1);
    with_inventory(&module, |inventory, floor| {
        assert!(matches!(
            exercise(inventory, floor, WORK, STORAGE, 1, &[]).0,
            Err(Error::Unsupported {
                detail: "whole scalar storage object only",
                ..
            })
        ))
    });
}

#[test]
fn typed_private_exact_work_peak_and_one_short_return_no_partial_proof() {
    let module = fixture(
        ScalarType::U32,
        4,
        4,
        vec![
            block(100, vec![allocation(4), write(4)], branch(200)),
            block(200, vec![read(ScalarType::U32, 4)], ret()),
        ],
    );
    with_inventory(&module, |inventory, floor| {
        let expected = [None, None, Some(1)];
        let (result, used, peak) = exercise(inventory, floor, WORK, STORAGE, 1, &expected);
        result.unwrap();
        assert!(
            exercise(inventory, floor, used, peak, 1, &expected)
                .0
                .is_ok()
        );
        for (work_limit, storage_limit, work_cut) in
            [(used - 1, peak, true), (used, peak - 1, false)]
        {
            let (result, _, _) =
                exercise(inventory, floor, work_limit, storage_limit, 1, &expected);
            match result {
                Err(Error::Resource(Resource::Work(error))) if work_cut => {
                    assert_eq!(error.limit(), work_limit);
                    assert!(error.actual() > error.limit());
                }
                Err(Error::Resource(Resource::Storage(error))) if !work_cut => {
                    assert_eq!(error.limit(), storage_limit);
                    assert!(error.actual() > error.limit());
                }
                other => panic!("wrong refusal: {other:?}"),
            }
        }
    });
}

#[test]
fn typed_private_scalar_layout_query_has_independent_fixed_work_and_checked_owner_row() {
    let layouts = [StorageLayoutV1 {
        size: 4,
        alignment: 1,
        kind: StorageLayoutKindV1::Scalar(ScalarType::U32),
    }];
    for limit in [10, 11] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(headers().unwrap()).unwrap();
        let result = scalar_stride(&layouts, StorageLayoutIdV1(0), 4, &mut budget);
        if limit == 11 {
            assert_eq!(result.unwrap(), 4);
        } else {
            assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
        }
        assert_eq!(budget.storage(), headers().unwrap());
    }
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    for id in [1, u32::MAX] {
        assert!(matches!(
            scalar_stride(&layouts, StorageLayoutIdV1(id), 4, &mut budget),
            Err(Error::Unsupported {
                detail: "exact owner scalar storage layout",
                ..
            })
        ));
    }
    for (size, alignment) in [(3, 1), (4, 0), (4, 3), (4, 8)] {
        let bad = [StorageLayoutV1 {
            size,
            alignment,
            kind: StorageLayoutKindV1::Scalar(ScalarType::U32),
        }];
        assert!(scalar_stride(&bad, StorageLayoutIdV1(0), 8, &mut budget).is_err());
    }
}

#[test]
fn typed_private_pointer_parameters_and_access_restriction_are_not_new_provenance() {
    for cast in [false, true] {
        let mut module = local(ScalarType::U32, 4, 4);
        let function = &mut module.functions[0];
        let pointer = Type::pointer(
            Type::StorageObject(StorageLayoutIdV1(0)),
            AddressSpace::Private,
            if cast {
                AccessMode::ReadOnly
            } else {
                AccessMode::ReadWrite
            },
        );
        if cast {
            function.body.as_mut().unwrap().blocks[0]
                .operations
                .push(Operation::effect_free(
                    ValueDef::new(ValueId(50), pointer.clone()),
                    OperationKind::Cast {
                        kind: fe2o3_kernel_ir::CastKind::RestrictPointerAccess,
                        value: ValueId(10),
                        to: pointer,
                    },
                ));
        } else {
            function.signature.parameters.push(pointer);
            function.body.as_mut().unwrap().parameters.push(ValueId(50));
        }
        with_inventory(&module, |inventory, floor| {
            if !cast {
                assert!(matches!(
                    exercise(inventory, floor, WORK, STORAGE, 1, &[]).0,
                    Err(Error::Unsupported { .. })
                ));
                return;
            }
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = Budget::new(&mut work, STORAGE);
            budget.reserve_storage(floor).unwrap();
            let (proof, receipt) = check_canonical_kir_private_memory_v18(
                inventory,
                CanonicalKirPrivateMemoryLimitsV1 { max_cells: 1 },
                &mut budget,
            )
            .unwrap();
            assert_eq!(budget.storage(), floor);
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            let allocation = inventory.operations()[0].results.start;
            let alias = inventory.operations()[3].results.start;
            assert_eq!(proof.address(alias), proof.address(allocation));
            let address = proof.address(alias).unwrap();
            assert_eq!(
                (address.allocation(), address.length(), address.offset()),
                (0, 1, 0)
            );
            assert_eq!(
                (0..4).map(|op| proof.operation(op)).collect::<Vec<_>>(),
                [true, true, true, false]
            );
            assert_eq!(proof.latest_stores(), [None, None, Some(1), None]);
            assert!(proof.is_for(inventory));
            assert!(std::ptr::eq(proof.inventory().owner(), inventory.owner()));
            assert!(!proof.grants_authority());
            drop(proof);
            budget.release_storage(receipt.retained_storage()).unwrap();
            assert_eq!(budget.storage(), floor);
        });
    }
}

#[test]
fn typed_private_fixed_helper_headers_are_paid_before_traversal() {
    fn slot<T>() -> usize {
        size_of::<T>() + size_of::<Result<T, Error>>()
    }
    let independent = slot::<&VerifiedCanonicalKernelIrModuleV18>()
        + slot::<&Module>()
        + slot::<&[StorageLayoutV1]>()
        + slot::<&Constant>()
        + slot::<Option<u64>>()
        + slot::<u64>()
        + slot::<&StorageLayoutIdV1>()
        + slot::<StorageLayoutIdV1>()
        + slot::<&Option<ValueId>>()
        + slot::<&u32>()
        + slot::<&AddressSpace>()
        + slot::<Option<&StorageLayoutV1>>()
        + slot::<&StorageLayoutV1>()
        + slot::<ScalarType>()
        + 3 * slot::<usize>()
        + slot::<u32>()
        + slot::<Option<u16>>()
        + slot::<bool>()
        + slot::<Address>()
        + slot::<Result<usize, std::num::TryFromIntError>>()
        + slot::<Option<&CanonicalKirOperationRefV1<'_>>>()
        + 2 * slot::<&CanonicalKirOperationRefV1<'_>>()
        + slot::<&Operation>()
        + slot::<Option<&CanonicalKirDefinitionRefV1<'_>>>()
        + 2 * slot::<&CanonicalKirDefinitionRefV1<'_>>()
        + 2 * slot::<&Type>()
        + 2 * slot::<Type>()
        + slot::<&PointerType>()
        + slot::<&Box<Type>>()
        + 2 * slot::<ValueId>()
        + slot::<Option<ValueId>>()
        + slot::<MemoryAccess>()
        + slot::<Option<(ValueId, MemoryAccess, bool)>>()
        + slot::<Option<(ScalarType, usize)>>()
        + slot::<(ScalarType, usize)>()
        + slot::<Option<&ValueDef>>()
        + slot::<&ValueDef>()
        + 2 * slot::<()>()
        + slot::<(
            &CanonicalKirInventoryV18<'_>,
            CanonicalKirPrivateMemoryLimitsV1,
        )>()
        + slot::<(
            &mut Cleanup<'_, '_>,
            (
                &CanonicalKirInventoryV18<'_>,
                CanonicalKirPrivateMemoryLimitsV1,
            ),
        )>()
        + slot::<
            AssertUnwindSafe<(
                &mut Cleanup<'_, '_>,
                (
                    &CanonicalKirInventoryV18<'_>,
                    CanonicalKirPrivateMemoryLimitsV1,
                ),
            )>,
        >()
        + slot::<(
            CheckedCanonicalKirPrivateMemoryV18<'_, '_>,
            CanonicalKirPrivateMemoryStorageV1,
        )>()
        + restriction_tests::header_oracle();
    assert_eq!(headers().unwrap(), independent);
    for (work_limit, storage_limit, expected_work) in [
        (86, 23 + independent, 86),
        (85, 23 + independent, 0),
        (86, 22 + independent, 86),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(23).unwrap();
        let result = reserve_headers(&mut budget);
        match (work_limit, storage_limit, result) {
            (86, limit, Ok(())) if limit == 23 + independent => {
                assert_eq!(budget.storage(), 23 + independent);
                budget.release_storage(independent).unwrap();
            }
            (85, _, Err(Error::Resource(Resource::Work(error)))) => {
                assert_eq!((error.actual(), error.limit()), (86, 85));
            }
            (86, limit, Err(Error::Resource(Resource::Storage(error)))) => {
                assert_eq!((error.actual(), error.limit()), (23 + independent, limit));
            }
            other => panic!("unexpected header boundary: {other:?}"),
        }
        assert_eq!(budget.storage(), 23);
        assert_eq!(budget.work(), expected_work);
    }
}

#[path = "canonical_kir_private_memory_restriction_v18_tests.rs"]
mod restriction_tests;

fn mixed_global_private(volatile: bool) -> Module {
    let mut module = local(ScalarType::U32, 4, 4);
    let function = &mut module.functions[0];
    function.signature.parameters.push(Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    ));
    let body = function.body.as_mut().unwrap();
    body.parameters.push(ValueId(901));
    assert_eq!(body.blocks.len(), 1);
    let operations = &mut body.blocks[0].operations;
    assert_eq!(operations.len(), 3);
    let mut access = MemoryAccess::new(AddressSpace::Global, 4);
    access.volatile = volatile;
    operations.insert(
        2,
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(901),
                value: ValueId(20),
                access,
            },
        ),
    );
    operations.insert(
        3,
        Operation::effect_free(
            ValueDef::new(ValueId(41), Type::Scalar(ScalarType::U32)),
            OperationKind::Load {
                pointer: ValueId(901),
                access,
            },
        ),
    );
    module
}

#[test]
fn typed_private_mixed_global_rows_stay_unclaimed_and_do_not_replace_store_identity() {
    for volatile in [false, true] {
        let module = mixed_global_private(volatile);
        with_inventory(&module, |inventory, floor| {
            assert_eq!(inventory.operations().len(), 5);
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = Budget::new(&mut work, STORAGE);
            budget.reserve_storage(floor).unwrap();
            let (proof, receipt) = check_canonical_kir_private_memory_v18(
                inventory,
                CanonicalKirPrivateMemoryLimitsV1 { max_cells: 1 },
                &mut budget,
            )
            .unwrap();
            assert_eq!(budget.storage(), floor);
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            assert!(proof.is_for(inventory));
            assert!(!proof.grants_authority());
            assert_eq!(
                (0..5)
                    .map(|index| proof.operation(index))
                    .collect::<Vec<_>>(),
                vec![true, true, false, false, true],
            );
            assert_eq!(proof.latest_stores(), &[None, None, None, None, Some(1)]);
            drop(proof);
            budget.release_storage(receipt.retained_storage()).unwrap();
            assert_eq!(budget.storage(), floor);

            // This is a private-only result even when the unclaimed global
            // access is volatile. It cannot complete any global/native role.
            let expected = [None, None, None, None, Some(1)];
            let measured = exercise(inventory, floor, WORK, STORAGE, 1, &expected);
            assert!(measured.0.is_ok());
            assert!(
                exercise(inventory, floor, measured.1, measured.2, 1, &expected)
                    .0
                    .is_ok()
            );
            assert!(matches!(
                exercise(inventory, floor, measured.1 - 1, measured.2, 1, &expected).0,
                Err(Error::Resource(Resource::Work(error)))
                    if error.limit() == measured.1 - 1 && error.actual() > error.limit(),
            ));
            assert!(matches!(
                exercise(inventory, floor, measured.1, measured.2 - 1, 1, &expected).0,
                Err(Error::Resource(Resource::Storage(error)))
                    if error.limit() == measured.2 - 1 && error.actual() > error.limit(),
            ));
        });
    }
}

#[test]
fn typed_private_global_store_cannot_initialize_a_same_scalar_private_allocation() {
    let mut module = mixed_global_private(false);
    let operations = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
    operations.remove(1);
    with_inventory(&module, |inventory, floor| {
        assert_eq!(inventory.operations().len(), 4);
        assert!(matches!(
            exercise(inventory, floor, WORK, STORAGE, 1, &[]).0,
            Err(Error::Unsupported {
                detail: "Load requires one exact reaching Store",
                ..
            }),
        ));
    });
}
