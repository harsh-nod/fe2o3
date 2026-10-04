use super::*;
use fe2o3_kernel_ir::{
    AccessMode, Atomic, AtomicKind, Barrier, BarrierSemantics, BasicBlock, BlockId,
    CanonicalKernelIrWorkBudgetV1 as Work, Constant, Function, MemoryOrdering, Signature,
    SynchronizationScope, Terminator, ValueDef,
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
fn operations(module: &mut Module) -> &mut Vec<Operation> {
    &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations
}
fn coordinate(operation: u32) -> Coordinate {
    Coordinate {
        block: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 {
            function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(0),
            block: 0,
        },
        operation,
    }
}
fn fixture(ty: ScalarType) -> Module {
    let scalar = Type::Scalar(ty);
    let bytes = u32::from(ty.bit_width().unwrap() / 8);
    let mut block = BasicBlock::new(BlockId(91));
    let access = MemoryAccess::new(AddressSpace::Private, bytes);
    let store = || {
        Operation::new(
            vec![],
            Kind::Store {
                pointer: ValueId(10),
                value: ValueId(0),
                access,
            },
        )
    };
    block.operations = vec![
        Operation::effect_free(
            ValueDef::new(
                ValueId(10),
                Type::pointer(scalar.clone(), AddressSpace::Private, AccessMode::ReadWrite),
            ),
            Kind::Alloca {
                element: scalar.clone(),
                count: None,
                address_space: AddressSpace::Private,
                alignment: bytes,
            },
        ),
        store(),
        Operation::effect_free(
            ValueDef::new(ValueId(11), scalar.clone()),
            Kind::Unary {
                op: UnaryOp::Not,
                operand: ValueId(1),
            },
        ),
        store(),
        store(),
        Operation::effect_free(
            ValueDef::new(ValueId(12), scalar.clone()),
            Kind::Load {
                pointer: ValueId(10),
                access,
            },
        ),
        Operation::new(
            vec![],
            Kind::Store {
                pointer: ValueId(2),
                value: ValueId(12),
                access: MemoryAccess::new(AddressSpace::Global, bytes),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(12)],
    });
    let mut module = Module::new("redundant-private-store");
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(
            vec![
                scalar.clone(),
                scalar.clone(),
                Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite),
                Type::BOOL,
            ],
            vec![scalar],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
        vec![block],
    ));
    module
}
fn two_stores() -> Module {
    let mut module = fixture(ScalarType::U32);
    operations(&mut module).remove(4);
    module
}
fn with_memory(
    module: Module,
    next: impl FnOnce(&Inventory<'_>, &MemorySsa<'_, '_>, &mut Budget<'_>),
) {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let (owner, os) =
        Owner::from_module_ref_with_verification_budget_v12(&module, &mut budget).unwrap();
    budget.reserve_storage(os.retained_storage()).unwrap();
    drop(module);
    let (inventory, is) = Inventory::derive(&owner, &mut budget).unwrap();
    budget.reserve_storage(is.retained_storage()).unwrap();
    let (memory, ms) = MemorySsa::derive(&inventory, Default::default(), &mut budget).unwrap();
    budget.reserve_storage(ms.retained_storage()).unwrap();
    let floor = budget.storage();
    next(&inventory, &memory, &mut budget);
    assert_eq!(budget.storage(), floor);
    drop(memory);
    budget.release_storage(ms.retained_storage()).unwrap();
    drop(inventory);
    budget.release_storage(is.retained_storage()).unwrap();
    drop(owner);
    budget.release_storage(os.retained_storage()).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}
fn with_output(
    inventory: &Inventory<'_>,
    memory: &MemorySsa<'_, '_>,
    budget: &mut Budget<'_>,
    next: impl FnOnce(&CanonicalKirAppliedRedundantStoreV1<'_>, &Owner, &mut Budget<'_>),
) {
    let (plan, ps) = CanonicalKirRedundantStorePlanV1::derive(inventory, memory, budget).unwrap();
    budget.reserve_storage(ps.retained_storage()).unwrap();
    let (mut candidate, cs) = inventory
        .owner()
        .copy_module_for_transformation_v12(budget)
        .unwrap();
    budget.reserve_storage(cs.retained_storage()).unwrap();
    let applied = plan
        .apply(inventory.owner(), &mut candidate, budget)
        .unwrap();
    let (output, os) =
        Owner::from_module_ref_with_verification_budget_v12(&candidate, budget).unwrap();
    budget.reserve_storage(os.retained_storage()).unwrap();
    next(&applied, &output, budget);
    drop(output);
    budget.release_storage(os.retained_storage()).unwrap();
    drop(candidate);
    budget.release_storage(cs.retained_storage()).unwrap();
    drop(applied);
    budget.release_storage(ps.retained_storage()).unwrap();
}
fn assert_rows(module: Module, count: usize) {
    with_memory(module, |inventory, memory, budget| {
        with_output(inventory, memory, budget, |applied, output, budget| {
            assert_eq!(applied.rows().len(), count);
            let (checked, _) = applied.check_output(output, budget).unwrap();
            assert!(!checked.grants_authority());
            assert_eq!(
                checked.retained_operations().len(),
                inventory.operations().len() - count
            );
            if count == 0 {
                assert_eq!(
                    inventory.owner().canonical().canonical_bytes(),
                    output.canonical().canonical_bytes()
                );
            }
        });
    });
}

#[test]
fn all_integer_widths_delete_only_later_stores_and_join_shifted_effect_coordinates() {
    for ty in TYPES {
        with_memory(fixture(ty), |inventory, memory, budget| {
            with_output(inventory, memory, budget, |applied, output, budget| {
                assert_eq!(
                    applied.rows(),
                    &[
                        Row {
                            anchor: coordinate(1),
                            removed: coordinate(3)
                        },
                        Row {
                            anchor: coordinate(1),
                            removed: coordinate(4)
                        }
                    ]
                );
                assert_ne!(
                    inventory.owner().canonical().canonical_bytes(),
                    output.canonical().canonical_bytes()
                );
                let before = &inventory.owner().module().functions[0]
                    .body
                    .as_ref()
                    .unwrap()
                    .blocks[0]
                    .operations;
                let after =
                    &output.module().functions[0].body.as_ref().unwrap().blocks[0].operations;
                assert_eq!(after.len(), 5);
                assert_eq!(after[1], before[1]);
                assert_eq!(after[3], before[5]);
                assert_eq!(after[4], before[6]);
                assert_eq!(
                    applied.retained_operations()[4],
                    Retained {
                        input: coordinate(6),
                        output: coordinate(4)
                    }
                );
                let (checked, _) = applied.check_output(output, budget).unwrap();
                assert!(std::ptr::eq(checked.input(), inventory.owner()));
                assert!(std::ptr::eq(checked.output(), output));
            });
        });
    }
}

#[test]
fn long_runs_keep_one_anchor_and_track_every_original_memory_def() {
    let mut module = fixture(ScalarType::U32);
    let duplicate = operations(&mut module)[1].clone();
    for _ in 0..128 {
        operations(&mut module).insert(3, duplicate.clone());
    }
    assert_rows(module, 130);
}

#[test]
fn distinct_values_accesses_and_other_stores_reset_the_run() {
    let mut value = fixture(ScalarType::U32);
    if let Kind::Store { value, .. } = &mut operations(&mut value)[3].kind {
        *value = ValueId(1);
    }
    assert_rows(value, 0);
    let mut alignment = fixture(ScalarType::U32);
    if let Kind::Store { access, .. } = &mut operations(&mut alignment)[3].kind {
        access.alignment = 1;
    }
    assert_rows(alignment, 0);
    let mut other = two_stores();
    let global = operations(&mut other)[5].clone();
    let Kind::Store {
        pointer, access, ..
    } = global.kind
    else {
        panic!()
    };
    operations(&mut other).insert(
        3,
        Operation::new(
            vec![],
            Kind::Store {
                pointer,
                value: ValueId(0),
                access,
            },
        ),
    );
    assert_rows(other, 0);
}

#[test]
fn initialization_alignment_dynamic_extent_parameter_and_float_slots_are_not_admitted() {
    let mut alignment = fixture(ScalarType::U32);
    if let Kind::Alloca { alignment, .. } = &mut operations(&mut alignment)[0].kind {
        *alignment = 1;
    }
    assert_rows(alignment, 0);
    let mut count = fixture(ScalarType::U32);
    if let Kind::Alloca { count, .. } = &mut operations(&mut count)[0].kind {
        *count = Some(ValueId(20));
    }
    operations(&mut count).insert(
        0,
        Operation::effect_free(
            ValueDef::new(ValueId(20), Type::INDEX),
            Kind::Constant(Constant::Index(1)),
        ),
    );
    assert_rows(count, 0);
    let mut parameter = fixture(ScalarType::U32);
    let allocation = operations(&mut parameter).remove(0);
    parameter.functions[0]
        .signature
        .parameters
        .push(allocation.results[0].ty.clone());
    parameter.functions[0]
        .body
        .as_mut()
        .unwrap()
        .parameters
        .push(ValueId(10));
    assert_rows(parameter, 0);
    let mut float = fixture(ScalarType::F32);
    operations(&mut float).remove(2);
    assert_rows(float, 0);
}

#[test]
fn complete_pointer_census_refuses_later_escape_and_derived_alias() {
    let mut escaped = fixture(ScalarType::U32);
    let pointer = operations(&mut escaped)[0].results[0].ty.clone();
    operations(&mut escaped).push(Operation::new(
        vec![],
        Kind::Call {
            callee: "escape".into(),
            arguments: vec![ValueId(10)],
        },
    ));
    escaped.functions.push(Function::declaration(
        "escape",
        Signature::new(vec![pointer], vec![]),
    ));
    assert_rows(escaped, 0);
    let mut alias = fixture(ScalarType::U32);
    let pointer = operations(&mut alias)[0].results[0].ty.clone();
    operations(&mut alias).extend([
        Operation::effect_free(
            ValueDef::new(ValueId(20), Type::INDEX),
            Kind::Constant(Constant::Index(0)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(21), pointer),
            Kind::GetElementPointer {
                base: ValueId(10),
                offset: ValueId(20),
            },
        ),
    ]);
    assert_rows(alias, 0);
}

#[test]
fn reads_calls_partial_arithmetic_and_convergence_are_closed_barriers() {
    let barriers = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(20), Type::Scalar(ScalarType::U32)),
            Kind::Load {
                pointer: ValueId(10),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(20), Type::Scalar(ScalarType::U32)),
            Kind::Binary {
                op: BinaryOp::Divide,
                lhs: ValueId(0),
                rhs: ValueId(1),
            },
        ),
        Operation::new(
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
        Operation::new(
            vec![],
            Kind::Atomic(Atomic {
                kind: AtomicKind::Store,
                pointer: ValueId(2),
                value: Some(ValueId(0)),
                compare: None,
                access: MemoryAccess::new(AddressSpace::Global, 4),
                scope: SynchronizationScope::Device,
                ordering: MemoryOrdering::Relaxed,
                failure_ordering: None,
            }),
        ),
    ];
    for barrier in barriers {
        let mut module = two_stores();
        operations(&mut module).insert(3, barrier);
        assert_rows(module, 0);
    }
    let mut call = two_stores();
    operations(&mut call).insert(
        3,
        Operation::new(
            vec![],
            Kind::Call {
                callee: "opaque".into(),
                arguments: vec![],
            },
        ),
    );
    call.functions.push(Function::declaration(
        "opaque",
        Signature::new(vec![], vec![]),
    ));
    assert_rows(call, 0);
    for index in [1, 3, 4, 5] {
        let mut volatile = fixture(ScalarType::U32);
        match &mut operations(&mut volatile)[index].kind {
            Kind::Store { access, .. } | Kind::Load { access, .. } => access.volatile = true,
            _ => unreachable!(),
        }
        assert_rows(volatile, 0);
    }
}

#[test]
fn block_boundaries_loop_iterations_and_function_ids_do_not_share_anchors() {
    let mut module = two_stores();
    let body = module.functions[0].body.as_mut().unwrap();
    let tail = body.blocks[0].operations.split_off(3);
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(14),
        arguments: vec![],
    });
    let mut next = BasicBlock::new(BlockId(14));
    next.operations = tail;
    next.terminator = Some(Terminator::Return {
        values: vec![ValueId(12)],
    });
    body.blocks.push(next);
    assert_rows(module, 0);
    let mut module = fixture(ScalarType::U32);
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[0].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(3),
        then_target: BlockId(91),
        then_arguments: vec![],
        else_target: BlockId(14),
        else_arguments: vec![],
    });
    let mut exit = BasicBlock::new(BlockId(14));
    exit.terminator = Some(Terminator::Return {
        values: vec![ValueId(12)],
    });
    body.blocks.push(exit);
    let mut other = module.functions[0].clone();
    other.id = "g".into();
    module.functions.push(other);
    with_memory(module, |inventory, memory, budget| {
        let (plan, _) =
            CanonicalKirRedundantStorePlanV1::derive(inventory, memory, budget).unwrap();
        assert_eq!(plan.rows().len(), 4);
        assert_eq!(plan.rows()[0].anchor.block.function.0, 0);
        assert_eq!(plan.rows()[2].anchor.block.function.0, 1);
    });
}

#[test]
fn foreign_inventory_and_equal_byte_foreign_owner_cannot_substitute_mutation_subject() {
    with_memory(fixture(ScalarType::U32), |inventory, memory, budget| {
        with_memory(fixture(ScalarType::U32), |other, other_memory, _| {
            assert!(matches!(
                CanonicalKirRedundantStorePlanV1::derive(inventory, other_memory, budget),
                Err(Error::ForeignSubject)
            ));
            let (plan, _) =
                CanonicalKirRedundantStorePlanV1::derive(inventory, memory, budget).unwrap();
            let mut candidate = other.owner().module().clone();
            let original = candidate.clone();
            assert!(matches!(
                plan.apply(other.owner(), &mut candidate, budget),
                Err(Error::ForeignSubject)
            ));
            assert_eq!(candidate, original);
        });
    });
}

#[test]
fn stale_candidate_and_one_short_preflight_never_mutate() {
    with_memory(fixture(ScalarType::U32), |inventory, memory, budget| {
        let (plan, _) =
            CanonicalKirRedundantStorePlanV1::derive(inventory, memory, budget).unwrap();
        let mut stale = inventory.owner().module().clone();
        operations(&mut stale).remove(3);
        let unchanged = stale.clone();
        assert!(matches!(
            plan.apply(inventory.owner(), &mut stale, budget),
            Err(Error::StaleCandidate)
        ));
        assert_eq!(stale, unchanged);
        let cost = 3
            + inventory.owner().canonical().canonical_bytes().len()
            + inventory.functions().len()
            + inventory.blocks().len()
            + inventory.operations().len();
        for (limit, ok) in [(cost, true), (cost - 1, false)] {
            let (plan, _) =
                CanonicalKirRedundantStorePlanV1::derive(inventory, memory, budget).unwrap();
            let mut candidate = inventory.owner().module().clone();
            let mut work = Work::new(limit);
            let mut short = Budget::new(&mut work, STORAGE);
            assert_eq!(
                plan.apply(inventory.owner(), &mut candidate, &mut short)
                    .is_ok(),
                ok
            );
            if !ok {
                assert_eq!(&candidate, inventory.owner().module());
            }
        }
    });
}

#[test]
fn independent_replay_rejects_forged_deletions_origins_and_unchanged_candidate() {
    with_memory(fixture(ScalarType::U32), |inventory, memory, budget| {
        with_output(inventory, memory, budget, |applied, output, budget| {
            for mode in 0..5 {
                let mut rows = applied.rows().to_vec();
                match mode {
                    0 => {
                        rows.remove(0);
                    }
                    1 => rows.push(rows[0]),
                    2 => rows.reverse(),
                    3 => rows[0].anchor = coordinate(3),
                    4 => rows[0].removed = coordinate(1),
                    _ => unreachable!(),
                }
                assert!(
                    check_canonical_kir_redundant_store_v1(
                        inventory.owner(),
                        output,
                        &rows,
                        applied.retained_operations(),
                        budget
                    )
                    .is_err()
                );
            }
            for mode in 0..4 {
                let mut origins = applied.retained_operations().to_vec();
                match mode {
                    0 => {
                        origins.pop();
                    }
                    1 => origins[3].input = coordinate(3),
                    2 => origins[4].output = coordinate(6),
                    3 => origins.swap(0, 1),
                    _ => unreachable!(),
                }
                assert!(
                    check_canonical_kir_redundant_store_v1(
                        inventory.owner(),
                        output,
                        applied.rows(),
                        &origins,
                        budget
                    )
                    .is_err()
                );
            }
            let identity = inventory
                .operations()
                .iter()
                .map(|row| Retained {
                    input: row.coordinate,
                    output: row.coordinate,
                })
                .collect::<Vec<_>>();
            assert!(matches!(
                check_canonical_kir_redundant_store_v1(
                    inventory.owner(),
                    inventory.owner(),
                    &[],
                    &identity,
                    budget
                ),
                Err(Error::Rule("exact redundant Store deletion row"))
            ));
        });
    });
}

#[test]
fn independent_replay_rejects_unreported_global_effect_or_header_changes() {
    with_memory(fixture(ScalarType::U32), |inventory, memory, budget| {
        with_output(inventory, memory, budget, |applied, output, budget| {
            for header in [false, true] {
                let mut altered = output.module().clone();
                if header {
                    altered.functions[0].id = "changed".into();
                } else if let Kind::Store { value, .. } = &mut operations(&mut altered)[4].kind {
                    *value = ValueId(1);
                }
                let (changed, cs) =
                    Owner::from_module_ref_with_verification_budget_v12(&altered, budget).unwrap();
                budget.reserve_storage(cs.retained_storage()).unwrap();
                assert!(applied.check_output(&changed, budget).is_err());
                drop(changed);
                budget.release_storage(cs.retained_storage()).unwrap();
            }
        });
    });
}

#[test]
fn exact_and_one_short_plan_and_independent_replay_restore_all_scratch() {
    with_memory(fixture(ScalarType::U32), |inventory, memory, outer| {
        let floor = outer.storage();
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        let (plan, receipt) =
            CanonicalKirRedundantStorePlanV1::derive(inventory, memory, &mut budget).unwrap();
        let cost = budget.work();
        let peak = budget.peak_storage();
        assert!(peak > floor + receipt.retained_storage());
        drop(plan);
        for (limit, storage, ok) in [
            (cost, peak, true),
            (cost - 1, peak, false),
            (cost, peak - 1, false),
        ] {
            let mut work = Work::new(limit);
            let mut budget = Budget::new(&mut work, storage);
            budget.reserve_storage(floor).unwrap();
            assert_eq!(
                CanonicalKirRedundantStorePlanV1::derive(inventory, memory, &mut budget).is_ok(),
                ok
            );
            assert_eq!(budget.storage(), floor);
        }
        with_output(inventory, memory, outer, |applied, output, outer| {
            let floor = outer.storage();
            let mut work = Work::new(WORK);
            let mut budget = Budget::new(&mut work, STORAGE);
            budget.reserve_storage(floor).unwrap();
            let _ = applied.check_output(output, &mut budget).unwrap();
            let cost = budget.work();
            let peak = budget.peak_storage();
            for (limit, storage, ok) in [
                (cost, peak, true),
                (cost - 1, peak, false),
                (cost, peak - 1, false),
            ] {
                let mut work = Work::new(limit);
                let mut budget = Budget::new(&mut work, storage);
                budget.reserve_storage(floor).unwrap();
                assert_eq!(applied.check_output(output, &mut budget).is_ok(), ok);
                assert_eq!(budget.storage(), floor);
            }
        });
    });
}

#[test]
fn scoped_failure_panic_and_ledger_replacement_never_release_unrelated_storage() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let flag = std::cell::Cell::new(false);
    struct DropFlag<'a>(&'a std::cell::Cell<bool>);
    impl Drop for DropFlag<'_> {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }
    let result: Result<()> = scoped(&mut budget, |budget| {
        let _owned = DropFlag(&flag);
        budget.reserve_storage(17)?;
        panic!("test callback");
    });
    assert_eq!(result, Err(Error::Panicked));
    assert!(flag.get());
    assert_eq!(budget.storage(), FLOOR);
    let result: Result<()> = scoped(&mut budget, |budget| {
        budget.release_storage(FLOOR)?;
        Ok(())
    });
    assert_eq!(result, Err(Error::Resource(Resource::Accounting)));
    assert_eq!(budget.storage(), 0);
    let mut replacement_work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let result: Result<()> = scoped(&mut budget, |budget| {
        *budget = Budget::new(&mut replacement_work, STORAGE);
        budget.reserve_storage(19)?;
        Ok(())
    });
    assert_eq!(result, Err(Error::Resource(Resource::Accounting)));
    assert_eq!(budget.storage(), 19);
}
