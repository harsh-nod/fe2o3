use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, CanonicalKernelIrWorkBudgetV1 as Work, Constant, MemoryAccess,
    ScalarType, Signature, StorageCopyOverlapV1, StorageFieldV1, StorageLayoutIdV1,
    StorageLayoutKindV1, StorageLayoutLimitsV1, StorageLayoutV1, StorageOperationV1,
    StorageProjectionV1, StorageVariantEncodingV1, StorageVariantV1, ValueDef, WorkgroupMemory,
    WorkgroupMemoryExtent,
};

const LIMIT: usize = 10_000_000;
const FLOOR: usize = 17;
const LAYOUT_LIMITS: StorageLayoutLimitsV1 = StorageLayoutLimitsV1 {
    rows: 64,
    edges: 256,
    containment_depth: 32,
    object_bytes: 4096,
};

fn address(row: u32, space: AddressSpace, access: AccessMode) -> Type {
    Type::pointer(Type::StorageObject(StorageLayoutIdV1(row)), space, access)
}

fn value(id: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::new(vec![ValueDef::new(ValueId(id), ty)], kind)
}

fn storage(result: Option<(u32, Type)>, kind: StorageOperationV1) -> Operation {
    Operation::new(
        result
            .into_iter()
            .map(|(id, ty)| ValueDef::new(ValueId(id), ty))
            .collect(),
        OperationKind::Storage(kind),
    )
}

fn allocate(id: u32, row: u32, space: AddressSpace) -> Operation {
    value(
        id,
        address(row, space, AccessMode::ReadWrite),
        if space == AddressSpace::Workgroup {
            OperationKind::WorkgroupMemory(WorkgroupMemory {
                element: Type::StorageObject(StorageLayoutIdV1(row)),
                extent: WorkgroupMemoryExtent::Static(1),
                alignment: 8,
            })
        } else {
            OperationKind::Alloca {
                element: Type::StorageObject(StorageLayoutIdV1(row)),
                count: None,
                address_space: space,
                alignment: 8,
            }
        },
    )
}

pub(crate) fn storage_module(space: AddressSpace, volatile: bool) -> Module {
    let field = |offset| StorageFieldV1 {
        offset,
        layout: StorageLayoutIdV1(0),
    };
    let mut module = Module::new("storage-inventory");
    module.storage_layouts = vec![
        StorageLayoutV1 {
            size: 8,
            alignment: 8,
            kind: StorageLayoutKindV1::Scalar(ScalarType::U64),
        },
        StorageLayoutV1 {
            size: 16,
            alignment: 8,
            kind: StorageLayoutKindV1::Record(vec![field(0), field(8)].into_boxed_slice()),
        },
        StorageLayoutV1 {
            size: 32,
            alignment: 8,
            kind: StorageLayoutKindV1::Array {
                element: StorageLayoutIdV1(1),
                length: 2,
                stride: 16,
            },
        },
        StorageLayoutV1 {
            size: 24,
            alignment: 8,
            kind: StorageLayoutKindV1::Variants {
                encoding: StorageVariantEncodingV1::Direct { tag: field(16) },
                variants: vec![StorageVariantV1 {
                    discriminant: 91,
                    direct_tag_bits: Some(3),
                    uninhabited: false,
                    layout: StorageLayoutIdV1(1),
                }]
                .into_boxed_slice(),
            },
        },
    ];
    let access = MemoryAccess {
        volatile,
        ..MemoryAccess::new(space, 8)
    };
    let rw = |row| address(row, space, AccessMode::ReadWrite);
    let wo = |row| address(row, space, AccessMode::WriteOnly);
    let mut entry = BasicBlock::new(BlockId(4_000_000_000));
    entry.operations = vec![
        allocate(900, 2, space),
        value(13, Type::INDEX, OperationKind::Constant(Constant::Index(1))),
        value(
            4_000_000_000,
            Type::Scalar(ScalarType::U64),
            OperationKind::Constant(Constant::U64(23)),
        ),
        storage(
            Some((400, rw(1))),
            StorageOperationV1::Project {
                base: ValueId(900),
                step: StorageProjectionV1::ArrayIndex(ValueId(13)),
            },
        ),
        storage(
            Some((1, rw(0))),
            StorageOperationV1::Project {
                base: ValueId(400),
                step: StorageProjectionV1::Field(1),
            },
        ),
        storage(
            None,
            StorageOperationV1::WriteValue {
                address: ValueId(1),
                value: ValueId(4_000_000_000),
                access,
            },
        ),
        storage(
            Some((99, Type::Scalar(ScalarType::U64))),
            StorageOperationV1::ReadValue {
                address: ValueId(1),
                access,
            },
        ),
        allocate(44, 1, space),
        storage(
            None,
            StorageOperationV1::CopyObject {
                source: ValueId(400),
                destination: ValueId(44),
                source_access: access,
                destination_access: access,
                overlap: StorageCopyOverlapV1::NonOverlapping,
            },
        ),
        allocate(81, 3, space),
        storage(
            Some((82, wo(1))),
            StorageOperationV1::Project {
                base: ValueId(81),
                step: StorageProjectionV1::VariantForWrite { index: 0 },
            },
        ),
        storage(
            Some((83, wo(0))),
            StorageOperationV1::Project {
                base: ValueId(82),
                step: StorageProjectionV1::Field(0),
            },
        ),
        storage(
            None,
            StorageOperationV1::WriteValue {
                address: ValueId(83),
                value: ValueId(99),
                access,
            },
        ),
        storage(
            None,
            StorageOperationV1::SetDiscriminant {
                address: ValueId(81),
                variant: 0,
                access: MemoryAccess::new(space, 8),
            },
        ),
        storage(
            Some((84, rw(1))),
            StorageOperationV1::Project {
                base: ValueId(81),
                step: StorageProjectionV1::Variant { index: 0, access },
            },
        ),
        storage(
            Some((85, rw(0))),
            StorageOperationV1::Project {
                base: ValueId(84),
                step: StorageProjectionV1::Field(0),
            },
        ),
        storage(
            Some((86, Type::Scalar(ScalarType::U64))),
            StorageOperationV1::ReadValue {
                address: ValueId(85),
                access,
            },
        ),
        value(6, Type::BOOL, OperationKind::Constant(Constant::Bool(true))),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(6),
        then_target: BlockId(7),
        then_arguments: vec![ValueId(86)],
        else_target: BlockId(7),
        else_arguments: vec![ValueId(99)],
    });
    let mut exit = BasicBlock::new(BlockId(7));
    exit.parameters
        .push(ValueDef::new(ValueId(2), Type::Scalar(ScalarType::U64)));
    exit.terminator = Some(Terminator::Return {
        values: vec![ValueId(2)],
    });
    module.functions.push(Function::definition(
        "storage",
        Signature::new(vec![], vec![Type::Scalar(ScalarType::U64)]),
        vec![],
        vec![entry, exit],
    ));
    let mut caller = BasicBlock::new(BlockId(7));
    caller.operations.push(value(
        2,
        Type::Scalar(ScalarType::U64),
        OperationKind::Call {
            callee: "storage".into(),
            arguments: vec![],
        },
    ));
    caller.terminator = Some(Terminator::Return {
        values: vec![ValueId(2)],
    });
    module.functions.push(Function::definition(
        "caller",
        Signature::new(vec![], vec![Type::Scalar(ScalarType::U64)]),
        vec![],
        vec![caller],
    ));
    module
}

pub(crate) fn storage_discriminant_module(space: AddressSpace) -> Module {
    let mut module = storage_module(space, false);
    let operations = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
    operations.extend([
        storage(
            Some((1001, Type::Scalar(ScalarType::U128))),
            StorageOperationV1::ReadDiscriminant {
                address: ValueId(81),
                access: MemoryAccess::new(space, 8),
            },
        ),
        storage(
            None,
            StorageOperationV1::SetDiscriminant {
                address: ValueId(81),
                variant: 0,
                access: MemoryAccess::new(space, 8),
            },
        ),
        storage(
            Some((1002, Type::Scalar(ScalarType::U128))),
            StorageOperationV1::ReadDiscriminant {
                address: ValueId(81),
                access: MemoryAccess::new(space, 8),
            },
        ),
    ]);
    module
}

#[test]
fn v18_discriminant_inventory_retains_actual_pointer_and_ordered_reads_between_retags() {
    for space in [AddressSpace::Private, AddressSpace::Workgroup] {
        with_inventory(&storage_discriminant_module(space), |inventory, budget| {
            for ordinal in [18, 20] {
                let row = &inventory.operations()[ordinal];
                let original = &inventory.owner().module().functions[0]
                    .body
                    .as_ref()
                    .unwrap()
                    .blocks[0]
                    .operations[ordinal];
                assert!(std::ptr::eq(row.operation, original));
                assert_eq!(row.operands.len(), 1);
                let used = &inventory.uses()[row.operands.start];
                assert_eq!(used.value, ValueId(81));
                assert_eq!(
                    inventory.definitions()[used.definition].value,
                    Some(ValueId(81))
                );
                assert_eq!(
                    inventory
                        .definition_for_value(FunctionCoordinate(0), ValueId(81), budget)
                        .unwrap()
                        .unwrap()
                        .value,
                    Some(ValueId(81))
                );
                let effects: Vec<_> = inventory
                    .effects()
                    .iter()
                    .filter(|effect| effect.coordinate.operation == row.coordinate)
                    .collect();
                assert_eq!(effects.len(), 1);
                assert_eq!(
                    effects[0].effect.to_owned(),
                    KirLocalMemoryEffectRefV1::Read(space).to_owned()
                );
                assert!(std::ptr::eq(effects[0].operation, original));
                assert_eq!(row.traps(), CanonicalKirBehaviorAnalysisV1::NotAnalyzed);
            }
        });
    }
}

fn ordinary_module() -> Module {
    let mut module = Module::new("ordinary-inventory");
    let mut entry = BasicBlock::new(BlockId(900));
    entry.operations = vec![
        value(6, Type::BOOL, OperationKind::Constant(Constant::Bool(true))),
        value(17, Type::INDEX, OperationKind::Constant(Constant::Index(1))),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(6),
        then_target: BlockId(7),
        then_arguments: vec![ValueId(17)],
        else_target: BlockId(7),
        else_arguments: vec![ValueId(17)],
    });
    let mut exit = BasicBlock::new(BlockId(7));
    exit.parameters
        .push(ValueDef::new(ValueId(99), Type::INDEX));
    exit.terminator = Some(Terminator::Return {
        values: vec![ValueId(99)],
    });
    module.functions.push(Function::definition(
        "ordinary",
        Signature::new(vec![], vec![Type::INDEX]),
        vec![],
        vec![entry, exit],
    ));
    module
}

pub(crate) fn admit(module: &Module) -> (VerifiedCanonicalKernelIrModuleV18, usize) {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let (owner, receipt) =
        VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
            module,
            LAYOUT_LIMITS,
            &mut budget,
        )
        .expect("actual V18 admission is required before inventory derivation");
    assert_eq!(budget.storage(), FLOOR);
    (owner, receipt.retained_storage())
}

pub(crate) fn with_inventory(
    module: &Module,
    inspect: impl FnOnce(&CanonicalKirInventoryV18<'_>, &mut Budget<'_>),
) {
    let (owner, owner_storage) = admit(module);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR + owner_storage).unwrap();
    let floor = budget.storage();
    let (inventory, receipt) = CanonicalKirInventoryV18::derive_v18(&owner, &mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    inspect(&inventory, &mut budget);
    drop(inventory);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), floor);
    drop(owner);
    budget.release_storage(owner_storage).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn v18_storage_rosters_keep_exact_borrowed_payloads_operands_and_effect_order() {
    for space in [AddressSpace::Private, AddressSpace::Workgroup] {
        for volatile in [false, true] {
            with_inventory(&storage_module(space, volatile), |inventory, budget| {
                assert!(inventory.belongs_to(inventory.owner()));
                assert_eq!(inventory.identity_v18(), *inventory.owner().identity());
                assert_eq!(inventory.owner().module().storage_layouts.len(), 4);
                for row in 0..4 {
                    let identity = inventory
                        .owner()
                        .layout_identity(StorageLayoutIdV1(row))
                        .unwrap();
                    assert_eq!(identity.row(), StorageLayoutIdV1(row));
                    assert_eq!(*identity.module(), inventory.identity_v18());
                }
                assert_eq!(inventory.functions().len(), 2);
                assert_eq!(inventory.blocks().len(), 3);
                assert_eq!(inventory.operations().len(), 19);
                assert_eq!(inventory.calls()[0].target, Some(FunctionCoordinate(0)));
                assert_eq!(inventory.edges().len(), 2);
                for (edge, incoming) in inventory.edges().iter().zip([86, 99]) {
                    assert_eq!(edge.target.function, FunctionCoordinate(0));
                    assert_eq!(edge.target.block, 1);
                    assert_eq!(edge.target_id, BlockId(7));
                    let binding = &inventory.edge_arguments()[edge.bindings.start];
                    assert_eq!(binding.value, ValueId(incoming));
                    assert_eq!(
                        inventory.definitions()[binding.incoming_definition].value,
                        Some(ValueId(incoming))
                    );
                    assert_eq!(
                        inventory.definitions()[binding.target_definition].value,
                        Some(ValueId(2))
                    );
                }
                let expected_uses: &[&[u32]] = &[
                    &[],
                    &[],
                    &[],
                    &[900, 13],
                    &[400],
                    &[1, 4_000_000_000],
                    &[1],
                    &[],
                    &[400, 44],
                    &[],
                    &[81],
                    &[82],
                    &[83, 99],
                    &[81],
                    &[81],
                    &[84],
                    &[85],
                    &[],
                    &[],
                ];
                for (row, expected) in inventory.operations().iter().zip(expected_uses) {
                    let point = row.coordinate;
                    let original = &inventory.owner().module().functions
                        [point.block.function.0 as usize]
                        .body
                        .as_ref()
                        .unwrap()
                        .blocks[point.block.block as usize]
                        .operations[point.operation as usize];
                    assert!(std::ptr::eq(row.operation, original));
                    assert_eq!(
                        inventory.uses()[row.operands.clone()]
                            .iter()
                            .map(|operand| operand.value.0)
                            .collect::<Vec<_>>(),
                        *expected
                    );
                    for operand in &inventory.uses()[row.operands.clone()] {
                        assert_eq!(
                            inventory.definitions()[operand.definition].value,
                            Some(operand.value)
                        );
                    }
                    assert_eq!(row.traps(), CanonicalKirBehaviorAnalysisV1::NotAnalyzed);
                    assert_eq!(
                        row.convergence(),
                        CanonicalKirBehaviorAnalysisV1::NotAnalyzed
                    );
                }
                let read = if volatile {
                    KirLocalMemoryEffectRefV1::VolatileRead(space)
                } else {
                    KirLocalMemoryEffectRefV1::Read(space)
                };
                let write = if volatile {
                    KirLocalMemoryEffectRefV1::VolatileWrite(space)
                } else {
                    KirLocalMemoryEffectRefV1::Write(space)
                };
                let expected = [
                    (0, KirLocalMemoryEffectRefV1::Allocate(space)),
                    (5, write),
                    (6, read),
                    (7, KirLocalMemoryEffectRefV1::Allocate(space)),
                    (8, read),
                    (8, write),
                    (9, KirLocalMemoryEffectRefV1::Allocate(space)),
                    (12, write),
                    (13, KirLocalMemoryEffectRefV1::Write(space)),
                    (14, read),
                    (16, read),
                ];
                assert_eq!(inventory.effects().len(), expected.len());
                for (effect, (operation, expected)) in inventory.effects().iter().zip(expected) {
                    assert_eq!(effect.coordinate.operation.operation, operation);
                    assert_eq!(effect.effect.to_owned(), expected.to_owned());
                    assert!(std::ptr::eq(
                        effect.operation,
                        inventory.operations()[operation as usize].operation
                    ));
                }
                for function in [FunctionCoordinate(0), FunctionCoordinate(1)] {
                    let row = inventory
                        .definition_for_value(function, ValueId(2), budget)
                        .unwrap()
                        .unwrap();
                    assert_eq!(definition_function(row.coordinate), function);
                }
                assert!(
                    inventory
                        .definition_for_value(FunctionCoordinate(1), ValueId(900), budget)
                        .unwrap()
                        .is_none()
                );
                assert_eq!(
                    inventory
                        .function_for_name("caller", budget)
                        .unwrap()
                        .unwrap()
                        .coordinate,
                    FunctionCoordinate(1)
                );
                assert_eq!(
                    inventory
                        .block_for_id(FunctionCoordinate(0), BlockId(7), budget)
                        .unwrap()
                        .unwrap()
                        .coordinate
                        .block,
                    1
                );
            });
        }
    }
}

#[test]
fn v18_owner_identity_includes_layout_rows_but_never_replaces_exact_borrow_identity() {
    let mut module = storage_module(AddressSpace::Private, false);
    module.storage_layouts.push(StorageLayoutV1 {
        size: 4,
        alignment: 4,
        kind: StorageLayoutKindV1::Scalar(ScalarType::U32),
    });
    let (first, first_bytes) = admit(&module);
    let (same, same_bytes) = admit(&module);
    module.storage_layouts.last_mut().unwrap().kind = StorageLayoutKindV1::Scalar(ScalarType::I32);
    let (different, different_bytes) = admit(&module);
    assert_eq!(first.identity(), same.identity());
    assert_ne!(first.identity(), different.identity());
    assert_eq!(first.module().functions, different.module().functions);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let owners = first_bytes + same_bytes + different_bytes;
    budget.reserve_storage(FLOOR + owners).unwrap();
    let (inventory, receipt) = CanonicalKirInventoryV18::derive_v18(&first, &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    assert!(inventory.belongs_to(&first));
    assert!(!inventory.belongs_to(&same));
    assert!(!inventory.belongs_to(&different));
    assert_eq!(inventory.identity_v18(), *first.identity());
    assert!(std::ptr::eq(inventory.owner(), &first));
    drop(inventory);
    budget.release_storage(receipt.retained_storage()).unwrap();
    drop((first, same, different));
    budget.release_storage(owners).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn v18_empty_table_matches_v12_inventory_layout_work_storage_and_queries() {
    assert_eq!(
        size_of::<CanonicalKirInventoryV1<'_>>(),
        size_of::<CanonicalKirInventoryV18<'_>>()
    );
    let module = ordinary_module();
    let (owner18, bytes18) = admit(&module);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR + bytes18).unwrap();
    let (owner12, receipt12) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            &module,
            &mut budget,
        )
        .unwrap();
    budget
        .reserve_storage(receipt12.retained_storage())
        .unwrap();
    let mut measurements = Vec::new();
    macro_rules! measure {
        ($owner:expr, $derive:path) => {{
            let start = budget.work();
            let floor = budget.storage();
            let (inventory, receipt) = $derive($owner, &mut budget).unwrap();
            assert_eq!(budget.storage(), floor);
            let derivation = budget.work() - start;
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            let queries = budget.work();
            let function = inventory
                .function_for_name("ordinary", &mut budget)
                .unwrap()
                .unwrap();
            let block = inventory
                .block_for_id(function.coordinate, BlockId(7), &mut budget)
                .unwrap()
                .unwrap();
            let definition = inventory
                .definition_for_value(function.coordinate, ValueId(99), &mut budget)
                .unwrap()
                .unwrap();
            let snapshot = (
                inventory.functions().len(),
                inventory.blocks().len(),
                inventory.definitions().len(),
                inventory.operations().len(),
                inventory.uses().len(),
                inventory.edges().len(),
                inventory.edge_arguments().len(),
                inventory.effects().len(),
                inventory.calls().len(),
                inventory.kernels().len(),
                block.coordinate,
                definition.coordinate,
            );
            measurements.push((
                derivation,
                receipt.retained_storage(),
                budget.work() - queries,
                snapshot,
            ));
            drop(inventory);
            budget.release_storage(receipt.retained_storage()).unwrap();
            assert_eq!(budget.storage(), floor);
        }};
    }
    measure!(&owner12, CanonicalKirInventoryV1::derive);
    measure!(&owner18, CanonicalKirInventoryV18::derive_v18);
    assert_eq!(measurements[0], measurements[1]);
    // Keep existing UFCS names unambiguous as well as method calls.
    let (legacy, transfer) = CanonicalKirInventoryV1::derive(&owner12, &mut budget).unwrap();
    budget.reserve_storage(transfer.retained_storage()).unwrap();
    assert_eq!(
        CanonicalKirInventoryV1::identity(&legacy),
        *owner12.canonical().identity()
    );
    drop(legacy);
    budget.release_storage(transfer.retained_storage()).unwrap();
    drop(owner12);
    budget
        .release_storage(receipt12.retained_storage())
        .unwrap();
    drop(owner18);
    budget.release_storage(bytes18).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

fn derive_at(
    owner: &VerifiedCanonicalKernelIrModuleV18,
    owner_bytes: usize,
    work_limit: usize,
    extra_storage: usize,
) -> (Result<usize>, usize, usize) {
    let floor = FLOOR + owner_bytes;
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, floor + extra_storage);
    budget.reserve_storage(floor).unwrap();
    let result = CanonicalKirInventoryV18::derive_v18(owner, &mut budget);
    assert_eq!(budget.storage(), floor);
    let result = result.map(|(inventory, receipt)| {
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        drop(inventory);
        budget.release_storage(receipt.retained_storage()).unwrap();
        receipt.retained_storage()
    });
    assert_eq!(budget.storage(), floor);
    (result, budget.work(), budget.peak_storage() - floor)
}

#[test]
fn v18_inventory_exact_and_one_below_resources_preserve_nonzero_floor_and_input() {
    for module in [
        Module::new("empty"),
        storage_module(AddressSpace::Private, false),
    ] {
        let (owner, owner_bytes) = admit(&module);
        let identity = *owner.identity();
        let (result, work, peak) = derive_at(&owner, owner_bytes, LIMIT, LIMIT);
        let retained = result.unwrap();
        assert_eq!(
            derive_at(&owner, owner_bytes, work, peak),
            (Ok(retained), work, peak)
        );
        for _ in 0..3 {
            assert!(matches!(
                derive_at(&owner, owner_bytes, work - 1, peak).0,
                Err(CanonicalKirInventoryErrorV1::Resource(Resource::Work(_)))
            ));
            assert!(matches!(
                derive_at(&owner, owner_bytes, work, peak - 1).0,
                Err(CanonicalKirInventoryErrorV1::Resource(Resource::Storage(_)))
            ));
            assert!(matches!(
                derive_at(&owner, owner_bytes, 0, peak).0,
                Err(CanonicalKirInventoryErrorV1::Resource(Resource::Work(_)))
            ));
            assert_eq!(*owner.identity(), identity);
        }
        if module.functions.is_empty() {
            assert_eq!(work, 2);
            assert_eq!(retained, size_of::<CanonicalKirInventoryV18<'_>>());
            assert_eq!(peak, retained);
        }
    }
}

#[test]
fn v18_inventory_preserves_first_denials_and_leaves_owner_unchanged() {
    let (owner, owner_bytes) = admit(&storage_module(AddressSpace::Private, false));
    let identity = *owner.identity();
    let mut work = Work::new(20);
    work.charge_work(3).unwrap();
    {
        let mut budget = Budget::new(&mut work, FLOOR + owner_bytes + 1024);
        budget.reserve_storage(FLOOR + owner_bytes).unwrap();
        assert!(budget.charge_work(usize::MAX).is_err());
        assert!(budget.reserve_storage(usize::MAX).is_err());
        assert!(matches!(
            CanonicalKirInventoryV18::derive_v18(&owner, &mut budget),
            Err(CanonicalKirInventoryErrorV1::Resource(Resource::Work(_)))
        ));
        assert_eq!(budget.storage(), FLOOR + owner_bytes);
        assert_eq!(budget.failed_storage(), Some(usize::MAX));
    }
    assert_eq!(work.failed_work(), Some(usize::MAX));
    assert_eq!(*owner.identity(), identity);
}

#[test]
fn v18_retained_storage_query_preserves_v12_work_and_transfer_contract() {
    for module in [
        ordinary_module(),
        storage_module(AddressSpace::Private, false),
        Module::new("empty_v18_inventory_control"),
    ] {
        let (owner, owner_storage) = admit(&module);
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR + owner_storage).unwrap();
        let (inventory, receipt) =
            CanonicalKirInventoryV18::derive_v18(&owner, &mut budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let floor = budget.storage();
        let before = budget.work();
        assert_eq!(
            CanonicalKirInventoryV18::retained_storage_v1(&inventory, &mut budget).unwrap(),
            receipt.retained_storage()
        );
        assert_eq!(budget.work() - before, 14);
        assert_eq!(budget.storage(), floor);
        for limit in [13, 14] {
            let mut query_work = Work::new(limit);
            let mut query_budget = Budget::new(&mut query_work, 0);
            let result = inventory.retained_storage_v1(&mut query_budget);
            if limit == 14 {
                assert_eq!(result.unwrap(), receipt.retained_storage());
                assert_eq!(query_budget.work(), 14);
                assert_eq!(query_budget.failed_work(), None);
            } else {
                assert!(matches!(
                    result,
                    Err(CanonicalKirInventoryErrorV1::Resource(Resource::Work(error)))
                        if error.actual() == 14 && error.limit() == 13
                ));
                assert_eq!(query_budget.work(), 0);
                assert_eq!(query_budget.failed_work(), Some(14));
            }
            assert_eq!(query_budget.storage(), 0);
            assert_eq!(query_budget.failed_storage(), None);
        }
        drop(inventory);
        budget.release_storage(receipt.retained_storage()).unwrap();
        assert_eq!(budget.storage(), FLOOR + owner_storage);
        drop(owner);
        budget.release_storage(owner_storage).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}
