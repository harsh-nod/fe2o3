use super::*;
use fe2o3_kernel_ir::{
    Atomic, AtomicKind, Barrier, BarrierSemantics, BasicBlock, BinaryOp, BlockId,
    CanonicalKernelIrWorkBudgetV1 as Work, CanonicalKirFunctionCoordinateV1 as FunctionCoordinate,
    Function, MemoryAccess, MemoryOrdering, Module, Operation, Signature, SynchronizationScope,
    Terminator, ValueDef, VerifiedCanonicalKernelIrModuleV12 as Owner,
};

const WORK: usize = 100_000_000;
const STORAGE: usize = 64 << 20;
const FLOOR: usize = 37;
const TYPES: [ScalarType; 8] = [
    ScalarType::I8,
    ScalarType::U8,
    ScalarType::I16,
    ScalarType::U16,
    ScalarType::I32,
    ScalarType::U32,
    ScalarType::I64,
    ScalarType::U64,
];
type Census<'i, 'g> = CanonicalKirPrivateCellCensusV1<'i, 'g>;

fn coord(operation: u32) -> Coordinate {
    Coordinate {
        block: Block {
            function: FunctionCoordinate(0),
            block: 0,
        },
        operation,
    }
}
fn ops(module: &mut Module) -> &mut Vec<Operation> {
    &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations
}
fn constant(id: u32, value: u64) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), Type::INDEX),
        Kind::Constant(Constant::Index(value)),
    )
}
fn fixture(scalar: ScalarType, count: Option<u64>) -> Module {
    let ty = Type::Scalar(scalar);
    let bytes = u32::from(scalar.bit_width().unwrap_or(64) / 8).max(1);
    let pointer = Type::pointer(ty.clone(), AddressSpace::Private, AccessMode::ReadWrite);
    let access = MemoryAccess::new(AddressSpace::Private, bytes);
    let mut block = BasicBlock::new(BlockId(91));
    block.operations = vec![
        constant(10, count.unwrap_or(1)),
        Operation::effect_free(
            ValueDef::new(ValueId(11), pointer.clone()),
            Kind::Alloca {
                element: ty.clone(),
                count: count.map(|_| ValueId(10)),
                address_space: AddressSpace::Private,
                alignment: bytes,
            },
        ),
        constant(12, 0),
        Operation::effect_free(
            ValueDef::new(ValueId(13), pointer),
            Kind::GetElementPointer {
                base: ValueId(11),
                offset: ValueId(12),
            },
        ),
        Operation::new(
            vec![],
            Kind::Store {
                pointer: ValueId(13),
                value: ValueId(0),
                access,
            },
        ),
        Operation::new(
            vec![],
            Kind::Store {
                pointer: ValueId(11),
                value: ValueId(1),
                access,
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(14), ty.clone()),
            Kind::Load {
                pointer: ValueId(13),
                access,
            },
        ),
    ];
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(14)],
    });
    let mut module = Module::new("private-census");
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(
            vec![ty.clone(), ty.clone(), Type::INDEX, Type::BOOL],
            vec![ty],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
        vec![block],
    ));
    module
}
fn with_inventory(module: Module, run: impl FnOnce(&Inventory<'_>, &mut Budget<'_>)) {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let (owner, os) =
        Owner::from_module_ref_with_verification_budget_v12(&module, &mut budget).unwrap();
    budget.reserve_storage(os.retained_storage()).unwrap();
    drop(module);
    let (inventory, is) = Inventory::derive(&owner, &mut budget).unwrap();
    budget.reserve_storage(is.retained_storage()).unwrap();
    let floor = budget.storage();
    run(&inventory, &mut budget);
    assert_eq!(budget.storage(), floor);
    drop(inventory);
    budget.release_storage(is.retained_storage()).unwrap();
    drop(owner);
    budget.release_storage(os.retained_storage()).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}
fn assert_count(module: Module, count: usize) {
    with_inventory(module, |inventory, budget| {
        let before = inventory.owner().canonical().canonical_bytes().to_vec();
        let (census, storage) = Census::derive(inventory, Limits::default(), budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        assert_eq!(census.allocations().len(), count);
        assert!(census.belongs_to(inventory));
        assert!(!census.grants_transformation_authority());
        assert_eq!(inventory.owner().canonical().canonical_bytes(), before);
        if count == 0 {
            assert!(census.addresses().is_empty());
            assert!(census.accesses().is_empty());
        }
        drop(census);
        budget.release_storage(storage.retained_storage()).unwrap();
    });
}

#[test]
fn scalar_and_counted_all_widths_keep_exact_latest_store_and_original_owner() {
    for scalar in TYPES {
        for count in [None, Some(1), Some(32)] {
            with_inventory(fixture(scalar, count), |inventory, budget| {
                let (census, receipt) =
                    Census::derive(inventory, Limits::default(), budget).unwrap();
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                assert_eq!(census.allocations().len(), 1);
                assert_eq!(census.addresses().len(), 2);
                assert_eq!(census.accesses().len(), 3);
                assert_eq!(census.allocations()[0].allocation, coord(1));
                assert_eq!(census.allocations()[0].count, count.unwrap_or(1));
                assert_eq!(
                    census.accesses()[2].kind,
                    AccessKind::Load {
                        result: ValueId(14),
                        previous_store: coord(5),
                        stored_value: ValueId(1),
                    }
                );
                assert!(std::ptr::eq(census.inventory(), inventory));
                drop(census);
                budget.release_storage(receipt.retained_storage()).unwrap();
            });
        }
    }
}

#[test]
fn sparse_multiple_cells_do_not_invent_zero_for_unobserved_cells() {
    let mut module = fixture(ScalarType::U32, Some(1 << 32));
    ops(&mut module)[2] = constant(12, (1 << 32) - 1);
    with_inventory(module, |inventory, budget| {
        let (census, receipt) = Census::derive(inventory, Limits::default(), budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        assert_eq!(census.allocations().len(), 1);
        assert_eq!(census.accesses().len(), 3);
        assert_eq!(
            census.accesses()[2].kind,
            AccessKind::Load {
                result: ValueId(14),
                previous_store: coord(4),
                stored_value: ValueId(0),
            }
        );
        assert!(receipt.retained_storage() < 4096);
        drop(census);
        budget.release_storage(receipt.retained_storage()).unwrap();
    });
}

#[test]
fn literal_count_and_offset_limits_are_whole_allocation_exclusions() {
    assert_count(fixture(ScalarType::U32, Some(0)), 0);
    for offset in [2, u64::MAX] {
        let mut module = fixture(ScalarType::U32, Some(2));
        ops(&mut module)[2] = constant(12, offset);
        assert_count(module, 0);
    }
    let mut module = fixture(ScalarType::U32, Some(2));
    let Kind::Alloca { count, .. } = &mut ops(&mut module)[1].kind else {
        unreachable!()
    };
    *count = Some(ValueId(2));
    assert_count(module, 0);
    let mut module = fixture(ScalarType::U32, Some(2));
    let Kind::GetElementPointer { offset, .. } = &mut ops(&mut module)[3].kind else {
        unreachable!()
    };
    *offset = ValueId(2);
    assert_count(module, 0);
    with_inventory(fixture(ScalarType::U32, Some(2)), |inventory, budget| {
        let (census, _) = Census::derive(
            inventory,
            Limits {
                array_count: 1,
                ..Limits::default()
            },
            budget,
        )
        .unwrap();
        assert!(census.allocations().is_empty());
        drop(census);
    });
    with_inventory(
        fixture(ScalarType::U64, Some(u64::MAX)),
        |inventory, budget| {
            let (census, _) = Census::derive(
                inventory,
                Limits {
                    array_count: u64::MAX,
                    ..Limits::default()
                },
                budget,
            )
            .unwrap();
            assert!(census.allocations().is_empty());
            drop(census);
        },
    );
}

#[test]
fn any_uninitialized_cell_rejects_the_entire_allocation() {
    let mut module = fixture(ScalarType::U32, Some(2));
    ops(&mut module)[2] = constant(12, 1);
    ops(&mut module).remove(4);
    assert_count(module, 0);
    let mut module = fixture(ScalarType::U32, None);
    ops(&mut module).swap(4, 6);
    assert_count(module, 0);
}

#[test]
fn nonvolatile_alignment_and_one_level_gep_are_exact() {
    for position in [4, 5, 6] {
        let mut module = fixture(ScalarType::U32, None);
        match &mut ops(&mut module)[position].kind {
            Kind::Load { access, .. } | Kind::Store { access, .. } => access.volatile = true,
            _ => unreachable!(),
        }
        assert_count(module, 0);
    }
    for alignment in [1, 8] {
        let mut module = fixture(ScalarType::U32, None);
        let Kind::Load { access, .. } = &mut ops(&mut module)[6].kind else {
            unreachable!()
        };
        access.alignment = alignment;
        assert_count(module, 0);
    }
    let mut module = fixture(ScalarType::U32, Some(2));
    let pointer = ops(&mut module)[3].results[0].ty.clone();
    ops(&mut module).insert(
        4,
        Operation::effect_free(
            ValueDef::new(ValueId(15), pointer),
            Kind::GetElementPointer {
                base: ValueId(13),
                offset: ValueId(12),
            },
        ),
    );
    assert_count(module, 0);
}

#[test]
fn guarded_memory_is_not_ordinary_access_evidence() {
    let mut module = fixture(ScalarType::U32, None);
    let Kind::Load { pointer, access } = ops(&mut module)[6].kind else {
        unreachable!()
    };
    ops(&mut module)[6].kind = Kind::GuardedLoad {
        pointer,
        predicate: ValueId(3),
        fallback: ValueId(0),
        access,
    };
    assert_count(module, 0);
    let mut module = fixture(ScalarType::U32, None);
    let Kind::Store {
        pointer,
        value,
        access,
    } = ops(&mut module)[4].kind
    else {
        unreachable!()
    };
    ops(&mut module)[4].kind = Kind::GuardedStore {
        pointer,
        value,
        predicate: ValueId(3),
        access,
    };
    assert_count(module, 0);
}

#[test]
fn returned_pointer_and_edge_argument_escapes_reject_all_rows() {
    let mut module = fixture(ScalarType::U32, None);
    let pointer = ops(&mut module)[1].results[0].ty.clone();
    module.functions[0].signature.results = vec![pointer.clone()];
    module.functions[0].body.as_mut().unwrap().blocks[0].terminator = Some(Terminator::Return {
        values: vec![ValueId(11)],
    });
    assert_count(module, 0);

    let mut module = fixture(ScalarType::U32, None);
    let mut second = BasicBlock::new(BlockId(72));
    second.parameters = vec![ValueDef::new(ValueId(20), pointer)];
    second.terminator = Some(Terminator::Return {
        values: vec![ValueId(14)],
    });
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(72),
        arguments: vec![ValueId(13)],
    });
    body.blocks.push(second);
    assert_count(module, 0);
}

#[test]
fn unreachable_uses_and_cross_block_accesses_are_not_omitted() {
    let mut module = fixture(ScalarType::U32, None);
    let pointer = ops(&mut module)[1].results[0].ty.clone();
    let mut dead = module.functions[0].body.as_mut().unwrap().blocks.remove(0);
    dead.id = BlockId(72);
    dead.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(20), pointer),
        Kind::GetElementPointer {
            base: ValueId(11),
            offset: ValueId(2),
        },
    ));
    dead.terminator = Some(Terminator::Return {
        values: vec![ValueId(0)],
    });
    let mut entry = BasicBlock::new(BlockId(91));
    entry.terminator = Some(Terminator::Return {
        values: vec![ValueId(0)],
    });
    module.functions[0].body.as_mut().unwrap().blocks = vec![entry, dead];
    assert_count(module, 0);

    let mut module = fixture(ScalarType::U32, None);
    let load = ops(&mut module).pop().unwrap();
    let mut second = BasicBlock::new(BlockId(72));
    second.operations.push(load);
    second.terminator = Some(Terminator::Return {
        values: vec![ValueId(14)],
    });
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(72),
        arguments: vec![],
    });
    body.blocks.push(second);
    assert_count(module, 0);
}

#[test]
fn scalar_arithmetic_and_stored_trap_computations_are_preserved_not_proved_dead() {
    let mut module = fixture(ScalarType::U32, None);
    ops(&mut module).insert(
        4,
        Operation::effect_free(
            ValueDef::new(ValueId(21), Type::Scalar(ScalarType::U32)),
            Kind::Binary {
                op: BinaryOp::Divide,
                lhs: ValueId(0),
                rhs: ValueId(1),
            },
        ),
    );
    let Kind::Store { value, .. } = &mut ops(&mut module)[5].kind else {
        unreachable!()
    };
    *value = ValueId(21);
    assert_count(module, 1);
}

#[test]
fn unknown_call_fences_the_access_interval_but_not_unrelated_prefix_or_suffix() {
    for (position, expected) in [(0, 1), (5, 0), (7, 1)] {
        let mut module = fixture(ScalarType::U32, None);
        let call = Operation::new(
            vec![],
            Kind::Call {
                callee: "other".into(),
                arguments: vec![],
            },
        );
        ops(&mut module).insert(position, call);
        let mut other = BasicBlock::new(BlockId(0));
        other.terminator = Some(Terminator::Return { values: vec![] });
        module.functions.push(Function::internal_helper(
            "other",
            Signature::new(vec![], vec![]),
            vec![],
            vec![other],
        ));
        assert_count(module, expected);
    }
}

#[test]
fn unused_allocation_and_address_producers_are_complete_not_implicit_loads() {
    let mut module = fixture(ScalarType::U32, Some(32));
    ops(&mut module).truncate(4);
    module.functions[0].body.as_mut().unwrap().blocks[0].terminator = Some(Terminator::Return {
        values: vec![ValueId(0)],
    });
    with_inventory(module, |inventory, budget| {
        let (census, _) = Census::derive(inventory, Limits::default(), budget).unwrap();
        assert_eq!(census.allocations().len(), 1);
        assert_eq!(census.addresses().len(), 2);
        assert!(census.accesses().is_empty());
        assert_eq!(census.allocations()[0].access_block, None);
        drop(census);
    });
}

#[test]
fn renamed_helpers_and_repeated_value_ids_stay_owner_function_qualified() {
    let mut module = fixture(ScalarType::U32, Some(2));
    let mut second = module.functions[0].clone();
    second.id = "renamed".into();
    module.functions.push(second);
    with_inventory(module, |inventory, budget| {
        let (census, _) = Census::derive(inventory, Limits::default(), budget).unwrap();
        assert_eq!(census.allocations().len(), 2);
        assert_eq!(
            census.allocations()[0].allocation.block.function,
            FunctionCoordinate(0)
        );
        assert_eq!(
            census.allocations()[1].allocation.block.function,
            FunctionCoordinate(1)
        );
        drop(census);
        let (other, receipt) = Inventory::derive(inventory.owner(), budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let (census, _) = Census::derive(inventory, Limits::default(), budget).unwrap();
        assert!(!census.belongs_to(&other));
        drop(census);
        drop(other);
        budget.release_storage(receipt.retained_storage()).unwrap();
    });
}

#[test]
fn sparse_count_does_not_change_new_storage_or_work_and_output_is_deterministic() {
    let mut expected = None;
    for count in [2, 1 << 16, 1 << 32, 2] {
        with_inventory(fixture(ScalarType::U32, Some(count)), |inventory, _| {
            let mut work = Work::new(WORK);
            let mut budget = Budget::new(&mut work, STORAGE);
            budget.reserve_storage(FLOOR).unwrap();
            let (census, receipt) =
                Census::derive(inventory, Limits::default(), &mut budget).unwrap();
            let observation = (
                budget.work(),
                budget.peak_storage(),
                receipt.retained_storage(),
                census.addresses().to_vec(),
                census.accesses().to_vec(),
            );
            if let Some(expected) = &expected {
                assert_eq!(&observation, expected);
            } else {
                expected = Some(observation);
            }
            drop(census);
            assert_eq!(budget.storage(), FLOOR);
        });
    }
}

#[test]
fn exact_and_one_short_work_storage_and_cfg_denials_restore_entry_floor() {
    with_inventory(fixture(ScalarType::U32, Some(2)), |inventory, _| {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let (census, _) = Census::derive(inventory, Limits::default(), &mut budget).unwrap();
        drop(census);
        let required = budget.work();
        let peak = budget.peak_storage();
        for (work_limit, storage_limit, success) in [
            (required, peak, true),
            (required - 1, peak, false),
            (required, peak - 1, false),
        ] {
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(FLOOR).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let result = Census::derive(inventory, Limits::default(), &mut budget);
            assert_eq!(result.is_ok(), success);
            if let Err(error) = &result {
                assert!(matches!(
                    error,
                    Error::Resource(_) | Error::ControlFlow(CfgError::Resource(_))
                ));
            }
            drop(result);
            assert_eq!(budget.storage(), FLOOR);
            assert!(budget.work_ledger_identity_v1() == ledger);
        }
        let mut limits = Limits::default();
        limits.control_flow.blocks = 0;
        assert!(matches!(
            Census::derive(inventory, limits, &mut budget),
            Err(Error::ControlFlow(_))
        ));
        assert_eq!(budget.storage(), FLOOR);
    });
}

#[test]
fn local_error_panic_and_unrelated_extra_reservation_cleanup_are_scoped() {
    for mode in 0..4 {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let mut owned = 0;
        let result: Result<()> = scoped(&mut budget, |meter| {
            let (mut rows, bytes) = meter.table::<u64>(16)?;
            owned = bytes;
            meter.push(&mut rows, 7)?;
            match mode {
                0 => Err(Error::InconsistentInventory),
                1 => panic!("private table setup panic"),
                2 => {
                    meter.budget_for_test().reserve_storage(11)?;
                    Ok(())
                }
                _ => {
                    meter.budget_for_test().release_storage(FLOOR + 1)?;
                    Ok(())
                }
            }
        });
        assert!(result.is_err());
        match mode {
            0 | 1 => assert_eq!(budget.storage(), FLOOR),
            2 => assert_eq!(budget.storage(), FLOOR + 11),
            _ => assert_eq!(budget.storage(), owned - 1),
        }
    }
}

#[test]
fn allocation_may_dominate_a_different_single_access_block() {
    let mut module = fixture(ScalarType::U32, None);
    let accesses = ops(&mut module).split_off(4);
    let mut second = BasicBlock::new(BlockId(72));
    second.operations = accesses;
    second.terminator = Some(Terminator::Return {
        values: vec![ValueId(14)],
    });
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(72),
        arguments: vec![],
    });
    body.blocks.push(second);
    assert_count(module, 1);
}

#[test]
fn noninteger_cells_and_pointer_parameters_are_not_owned_allocation_candidates() {
    for scalar in [
        ScalarType::Bool,
        ScalarType::Index,
        ScalarType::F32,
        ScalarType::F64,
    ] {
        assert_count(fixture(scalar, None), 0);
    }
    let mut module = fixture(ScalarType::U32, None);
    let allocation = ops(&mut module).remove(1);
    module.functions[0]
        .signature
        .parameters
        .push(allocation.results[0].ty.clone());
    module.functions[0]
        .body
        .as_mut()
        .unwrap()
        .parameters
        .push(ValueId(11));
    assert_count(module, 0);
}

#[test]
fn allocation_and_offset_alignment_are_not_inferred_from_access_claims() {
    let mut module = fixture(ScalarType::U32, None);
    let Kind::Alloca { alignment, .. } = &mut ops(&mut module)[1].kind else {
        unreachable!()
    };
    *alignment = 1;
    assert_count(module, 0);
    let mut module = fixture(ScalarType::U32, Some(2));
    let Kind::Alloca { alignment, .. } = &mut ops(&mut module)[1].kind else {
        unreachable!()
    };
    *alignment = 16;
    ops(&mut module)[2] = constant(12, 1);
    let Kind::Load { access, .. } = &mut ops(&mut module)[6].kind else {
        unreachable!()
    };
    access.alignment = 8;
    assert_count(module, 0);
}

#[test]
fn barriers_atomic_and_unknown_memory_fence_an_otherwise_valid_interval() {
    for kind in 0..3 {
        let mut module = fixture(ScalarType::U32, None);
        module.functions[0].signature.parameters.push(Type::pointer(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Global,
            AccessMode::ReadWrite,
        ));
        module.functions[0]
            .body
            .as_mut()
            .unwrap()
            .parameters
            .push(ValueId(4));
        let operation = match kind {
            0 => Operation::new(
                vec![],
                Kind::Barrier(Barrier {
                    execution_scope: SynchronizationScope::Workgroup,
                    memory_scope: SynchronizationScope::Workgroup,
                    semantics: BarrierSemantics::new(
                        MemoryOrdering::AcquireRelease,
                        [AddressSpace::Workgroup],
                    ),
                }),
            ),
            1 => Operation::new(
                vec![],
                Kind::Atomic(Atomic {
                    kind: AtomicKind::Store,
                    pointer: ValueId(4),
                    value: Some(ValueId(0)),
                    compare: None,
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                    scope: SynchronizationScope::Device,
                    ordering: MemoryOrdering::Relaxed,
                    failure_ordering: None,
                }),
            ),
            _ => Operation::effect_free(
                ValueDef::new(ValueId(20), Type::Scalar(ScalarType::U32)),
                Kind::Load {
                    pointer: ValueId(4),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            ),
        };
        ops(&mut module).insert(5, operation);
        assert_count(module, 0);
    }
}

#[test]
fn pointer_escape_after_last_access_still_disqualifies_the_whole_allocation() {
    let mut module = fixture(ScalarType::U32, None);
    let pointer = ops(&mut module)[1].results[0].ty.clone();
    ops(&mut module).push(Operation::new(
        vec![],
        Kind::Call {
            callee: "escape".into(),
            arguments: vec![ValueId(13)],
        },
    ));
    module.functions.push(Function::declaration(
        "escape",
        Signature::new(vec![pointer], vec![]),
    ));
    assert_count(module, 0);
}

#[test]
fn replacing_live_budget_never_charges_or_releases_the_foreign_ledger() {
    let mut work = Work::new(WORK);
    let mut foreign_work = Work::new(WORK);
    {
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let mut foreign_storage = 0;
        let result: Result<()> = scoped(&mut budget, |meter| {
            let (mut rows, _) = meter.table::<u64>(16)?;
            meter.push(&mut rows, 7)?;
            foreign_storage = meter.budget_for_test().storage();
            let mut foreign = Budget::new(&mut foreign_work, STORAGE);
            foreign.reserve_storage(foreign_storage)?;
            *meter.budget_for_test() = foreign;
            meter.work(1)
        });
        assert_eq!(result, Err(Error::Resource(Resource::Accounting)));
        assert_eq!(budget.storage(), foreign_storage);
        assert_eq!(budget.work(), 0);
    }
    assert!(work.work() > 0);
    assert_eq!(foreign_work.work(), 0);
}

#[test]
fn capacity_reconciliation_is_checked_and_accepted_before_initialization() {
    // Pure accounting-helper controls, not a claim that the allocator supplied
    // excess on this machine. Real table allocation calls this same helper.
    for (limit, actual, expected) in [
        (FLOOR + 40, 5, Ok(40)),
        (FLOOR + 39, 5, Err(false)),
        (STORAGE, 2, Err(true)),
    ] {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, limit);
        budget.reserve_storage(FLOOR).unwrap();
        let result = scoped(&mut budget, |meter| {
            meter.reserve(3 * size_of::<u64>())?;
            meter.capacity::<u64>(3, actual)
        });
        match expected {
            Ok(bytes) => assert_eq!(result.unwrap(), bytes),
            Err(true) => assert_eq!(result, Err(Error::Resource(Resource::Accounting))),
            Err(false) => assert!(matches!(result, Err(Error::Resource(Resource::Storage(_))))),
        }
        assert_eq!(budget.storage(), FLOOR);
    }
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let result = scoped(&mut budget, |meter| meter.table::<u64>(usize::MAX));
    assert!(matches!(result, Err(Error::Resource(Resource::Arithmetic))));
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn rejected_result_and_panic_payload_drop_only_after_their_required_cleanup() {
    struct Bomb;
    impl Drop for Bomb {
        fn drop(&mut self) {
            panic!("drop payload");
        }
    }
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _: Result<()> = scoped(&mut budget, |meter| {
            let (_rows, _) = meter.table::<u64>(16)?;
            std::panic::panic_any(Bomb);
        });
    }));
    assert!(result.is_err());
    assert_eq!(budget.storage(), FLOOR);
    let result: Result<Bomb> = scoped(&mut budget, |meter| {
        let (_rows, _) = meter.table::<u64>(16)?;
        meter.budget_for_test().reserve_storage(11)?;
        Ok(Bomb)
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    assert_eq!(budget.storage(), FLOOR + 11);
}

#[test]
fn failed_allocation_does_not_partially_suppress_an_independent_candidate() {
    let mut module = fixture(ScalarType::U32, None);
    let ty = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(ty.clone(), AddressSpace::Private, AccessMode::ReadWrite);
    let access = MemoryAccess::new(AddressSpace::Private, 4);
    ops(&mut module).extend([
        Operation::effect_free(
            ValueDef::new(ValueId(20), pointer.clone()),
            Kind::Alloca {
                element: ty.clone(),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ),
        Operation::new(
            vec![],
            Kind::Store {
                pointer: ValueId(20),
                value: ValueId(0),
                access,
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(21), ty),
            Kind::Load {
                pointer: ValueId(20),
                access,
            },
        ),
        Operation::new(
            vec![],
            Kind::Call {
                callee: "escape".into(),
                arguments: vec![ValueId(11)],
            },
        ),
    ]);
    module.functions.push(Function::declaration(
        "escape",
        Signature::new(vec![pointer], vec![]),
    ));
    with_inventory(module, |inventory, budget| {
        let (census, _) = Census::derive(inventory, Limits::default(), budget).unwrap();
        assert_eq!(census.allocations().len(), 1);
        assert_eq!(census.allocations()[0].value, ValueId(20));
        assert!(
            census
                .addresses()
                .iter()
                .all(|row| row.value == ValueId(20))
        );
        assert_eq!(census.accesses().len(), 2);
        drop(census);
    });
}

#[test]
fn storing_an_address_as_a_value_is_an_escape_even_outside_the_access_interval() {
    let mut module = fixture(ScalarType::U32, None);
    let pointer = ops(&mut module)[1].results[0].ty.clone();
    ops(&mut module).extend([
        Operation::effect_free(
            ValueDef::new(
                ValueId(20),
                Type::pointer(
                    pointer.clone(),
                    AddressSpace::Private,
                    AccessMode::ReadWrite,
                ),
            ),
            Kind::Alloca {
                element: pointer,
                count: None,
                address_space: AddressSpace::Private,
                alignment: 8,
            },
        ),
        Operation::new(
            vec![],
            Kind::Store {
                pointer: ValueId(20),
                value: ValueId(11),
                access: MemoryAccess::new(AddressSpace::Private, 8),
            },
        ),
    ]);
    assert_count(module, 0);
}
