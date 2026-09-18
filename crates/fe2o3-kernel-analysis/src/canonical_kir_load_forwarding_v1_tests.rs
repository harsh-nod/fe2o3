use super::*;
use fe2o3_kernel_ir::{
    AccessMode, Atomic, AtomicKind, Barrier, BarrierSemantics, BasicBlock, BlockId,
    CanonicalKernelIrWorkBudgetV1 as Work, Constant, Function, MemoryOrdering, Signature,
    SynchronizationScope, Terminator, ValueDef,
};

const WORK: usize = 100_000_000;
const STORAGE: usize = 64 << 20;
const FLOOR: usize = 29;
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
    let pointer = Type::pointer(scalar.clone(), AddressSpace::Private, AccessMode::ReadWrite);
    let mut block = BasicBlock::new(BlockId(91));
    block.operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(2), pointer),
            Kind::Alloca {
                element: scalar.clone(),
                count: None,
                address_space: AddressSpace::Private,
                alignment: bytes,
            },
        ),
        Operation::new(
            vec![],
            Kind::Store {
                pointer: ValueId(2),
                value: ValueId(0),
                access: MemoryAccess::new(AddressSpace::Private, bytes),
            },
        ),
        // The old store-forwarding service stops here, but this disjoint
        // address-space write does not invalidate the proven private slot.
        Operation::new(
            vec![],
            Kind::Store {
                pointer: ValueId(1),
                value: ValueId(0),
                access: MemoryAccess::new(AddressSpace::Global, bytes),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(3), scalar.clone()),
            Kind::Load {
                pointer: ValueId(2),
                access: MemoryAccess::new(AddressSpace::Private, bytes),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(6), scalar.clone()),
            Kind::Select {
                condition: ValueId(20),
                true_value: ValueId(0),
                false_value: ValueId(0),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(4), scalar.clone()),
            Kind::Load {
                pointer: ValueId(2),
                access: MemoryAccess::new(AddressSpace::Private, bytes),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(5), scalar.clone()),
            Kind::Load {
                pointer: ValueId(2),
                access: MemoryAccess::new(AddressSpace::Private, bytes),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(5)],
    });
    let mut module = Module::new("private-load-forwarding");
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(
            vec![
                scalar.clone(),
                Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite),
                Type::BOOL,
            ],
            vec![scalar],
        ),
        vec![ValueId(0), ValueId(1), ValueId(20)],
        vec![block],
    ));
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
fn assert_rows(module: Module, expected: usize) {
    with_memory(module, |inventory, memory, budget| {
        let (plan, receipt) =
            CanonicalKirLoadForwardingPlanV1::derive(inventory, memory, budget).unwrap();
        assert_eq!(plan.rows().len(), expected);
        assert_eq!(
            receipt.retained_storage(),
            size_of::<CanonicalKirLoadForwardingPlanV1<'_, '_, '_>>()
                + plan.rows.capacity() * size_of::<Row>()
        );
    });
}

#[test]
fn all_fixed_integer_widths_preserve_first_load_and_replay_actual_pair() {
    for ty in [
        ScalarType::I8,
        ScalarType::U8,
        ScalarType::I16,
        ScalarType::U16,
        ScalarType::I32,
        ScalarType::U32,
        ScalarType::I64,
        ScalarType::U64,
    ] {
        with_memory(fixture(ty), |inventory, memory, budget| {
            let (plan, ps) =
                CanonicalKirLoadForwardingPlanV1::derive(inventory, memory, budget).unwrap();
            budget.reserve_storage(ps.retained_storage()).unwrap();
            assert_eq!(
                plan.rows(),
                &[
                    Row {
                        first: coordinate(3),
                        load: coordinate(5)
                    },
                    Row {
                        first: coordinate(3),
                        load: coordinate(6)
                    }
                ]
            );
            assert_eq!(
                incoming(memory, coordinate(3), budget).unwrap(),
                incoming(memory, coordinate(6), budget).unwrap()
            );
            let (mut candidate, cs) = inventory
                .owner()
                .copy_module_for_transformation_v12(budget)
                .unwrap();
            budget.reserve_storage(cs.retained_storage()).unwrap();
            let applied = plan
                .apply(inventory.owner(), &mut candidate, budget)
                .unwrap();
            assert_eq!(
                candidate.functions[0].body.as_ref().unwrap().blocks[0].operations[3],
                inventory.owner().module().functions[0]
                    .body
                    .as_ref()
                    .unwrap()
                    .blocks[0]
                    .operations[3]
            );
            assert!(matches!(
                operations(&mut candidate)[5].kind,
                Kind::Binary {
                    op: BinaryOp::BitOr,
                    lhs: ValueId(3),
                    rhs: ValueId(3)
                }
            ));
            let (output, os) =
                Owner::from_module_ref_with_verification_budget_v12(&candidate, budget).unwrap();
            budget.reserve_storage(os.retained_storage()).unwrap();
            {
                let (checked, _) = applied.check_output(&output, budget).unwrap();
                assert!(std::ptr::eq(checked.input(), inventory.owner()));
                assert!(std::ptr::eq(checked.output(), &output));
                assert!(!checked.grants_authority());
            }
            drop(output);
            budget.release_storage(os.retained_storage()).unwrap();
            drop(candidate);
            budget.release_storage(cs.retained_storage()).unwrap();
            drop(applied);
            budget.release_storage(ps.retained_storage()).unwrap();
        });
    }
}

#[test]
fn uninitialized_invalid_alignment_parameter_and_noninteger_slots_get_no_plan() {
    let mut uninitialized = fixture(ScalarType::U32);
    operations(&mut uninitialized).remove(1);
    assert_rows(uninitialized, 0);
    let mut misaligned = fixture(ScalarType::U32);
    for index in [3, 5, 6] {
        let Kind::Load { ref mut access, .. } = operations(&mut misaligned)[index].kind else {
            panic!()
        };
        access.alignment = 8;
    }
    assert_rows(misaligned, 0);
    let mut parameter = fixture(ScalarType::U32);
    let pointer = operations(&mut parameter)[0].results[0].ty.clone();
    operations(&mut parameter).remove(0);
    parameter.functions[0].signature.parameters.push(pointer);
    parameter.functions[0]
        .body
        .as_mut()
        .unwrap()
        .parameters
        .push(ValueId(2));
    assert_rows(parameter, 0);
    for ty in [ScalarType::F32, ScalarType::F64] {
        assert_rows(fixture(ty), 0);
    }
}

#[test]
fn every_pointer_escape_and_nonliteral_single_slot_shape_refuses() {
    let mut escaped = fixture(ScalarType::U32);
    let pointer = operations(&mut escaped)[0].results[0].ty.clone();
    operations(&mut escaped).insert(
        2,
        Operation::new(
            vec![],
            Kind::Call {
                callee: "g".into(),
                arguments: vec![ValueId(2)],
            },
        ),
    );
    let mut block = BasicBlock::new(BlockId(14));
    block.terminator = Some(Terminator::Return { values: vec![] });
    escaped.functions.push(Function::internal_helper(
        "g",
        Signature::new(vec![pointer], vec![]),
        vec![ValueId(0)],
        vec![block],
    ));
    assert_rows(escaped, 0);
    let mut count = fixture(ScalarType::U32);
    let Kind::Alloca {
        count: ref mut size,
        ..
    } = operations(&mut count)[0].kind
    else {
        panic!()
    };
    *size = Some(ValueId(10));
    operations(&mut count).insert(
        0,
        Operation::effect_free(
            ValueDef::new(ValueId(10), Type::INDEX),
            Kind::Constant(Constant::Index(1)),
        ),
    );
    assert_rows(count, 0);
    let mut alias = fixture(ScalarType::U32);
    let pointer = operations(&mut alias)[0].results[0].ty.clone();
    operations(&mut alias).insert(
        2,
        Operation::effect_free(
            ValueDef::new(ValueId(10), Type::INDEX),
            Kind::Constant(Constant::Index(0)),
        ),
    );
    operations(&mut alias).insert(
        3,
        Operation::effect_free(
            ValueDef::new(ValueId(11), pointer),
            Kind::GetElementPointer {
                base: ValueId(2),
                offset: ValueId(10),
            },
        ),
    );
    assert_rows(alias, 0);
}

#[test]
fn ordinary_global_store_is_not_a_private_pointer_escape_exception() {
    // A pointer-valued global Store would first use the alloca as a value and
    // fail the complete nonescape census; it cannot inherit the scalar rule.
    let mut escaped = fixture(ScalarType::U32);
    let pointer = operations(&mut escaped)[0].results[0].ty.clone();
    escaped.functions[0]
        .signature
        .parameters
        .push(Type::pointer(
            pointer,
            AddressSpace::Global,
            AccessMode::ReadWrite,
        ));
    escaped.functions[0]
        .body
        .as_mut()
        .unwrap()
        .parameters
        .push(ValueId(21));
    operations(&mut escaped).insert(
        2,
        Operation::new(
            vec![],
            Kind::Store {
                pointer: ValueId(21),
                value: ValueId(2),
                access: MemoryAccess::new(AddressSpace::Global, 8),
            },
        ),
    );
    assert_rows(escaped, 0);
}

#[test]
fn volatile_other_load_partial_arithmetic_and_convergence_are_barriers() {
    let barrier = Operation::new(
        vec![],
        Kind::Barrier(Barrier {
            execution_scope: SynchronizationScope::Workgroup,
            memory_scope: SynchronizationScope::Workgroup,
            semantics: BarrierSemantics::new(
                MemoryOrdering::AcquireRelease,
                [AddressSpace::Workgroup],
            ),
        }),
    );
    for operation in [
        Operation::effect_free(
            ValueDef::new(ValueId(8), Type::Scalar(ScalarType::U32)),
            Kind::Load {
                pointer: ValueId(1),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(8), Type::Scalar(ScalarType::U32)),
            Kind::Binary {
                op: BinaryOp::Divide,
                lhs: ValueId(0),
                rhs: ValueId(0),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(8), Type::Scalar(ScalarType::U32)),
            Kind::GuardedLoad {
                pointer: ValueId(2),
                predicate: ValueId(20),
                fallback: ValueId(0),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ),
        Operation::new(
            vec![],
            Kind::Atomic(Atomic {
                kind: AtomicKind::Store,
                pointer: ValueId(1),
                value: Some(ValueId(0)),
                compare: None,
                access: MemoryAccess::new(AddressSpace::Global, 4),
                scope: SynchronizationScope::Device,
                ordering: MemoryOrdering::Relaxed,
                failure_ordering: None,
            }),
        ),
        barrier,
    ] {
        let mut module = fixture(ScalarType::U32);
        operations(&mut module)[4] = operation;
        assert_rows(module, 0);
    }
    let mut volatile = fixture(ScalarType::U32);
    let Kind::Load { ref mut access, .. } = operations(&mut volatile)[3].kind else {
        panic!()
    };
    access.volatile = true;
    assert_rows(volatile, 0);
    let mut volatile_store = fixture(ScalarType::U32);
    let Kind::Store { ref mut access, .. } = operations(&mut volatile_store)[2].kind else {
        panic!()
    };
    access.volatile = true;
    assert_rows(volatile_store, 0);
}

#[test]
fn even_complete_empty_calls_are_closed_initialization_and_forwarding_barriers() {
    let mut module = fixture(ScalarType::U32);
    operations(&mut module)[4] = Operation::new(
        vec![],
        Kind::Call {
            callee: "g".into(),
            arguments: vec![],
        },
    );
    let mut block = BasicBlock::new(BlockId(14));
    block.terminator = Some(Terminator::Return { values: vec![] });
    module.functions.push(Function::internal_helper(
        "g",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    assert_rows(module, 0);
}

#[test]
fn forged_forwarding_of_an_uninitialized_first_read_is_independently_rejected() {
    let mut module = fixture(ScalarType::U32);
    operations(&mut module).remove(1);
    with_memory(module, |inventory, _, budget| {
        let (mut candidate, cs) = inventory
            .owner()
            .copy_module_for_transformation_v12(budget)
            .unwrap();
        budget.reserve_storage(cs.retained_storage()).unwrap();
        let first = coordinate(2);
        let last = coordinate(5);
        let seed = load(&operations(&mut candidate)[2], first).unwrap();
        operations(&mut candidate)[5].kind = replacement(seed);
        let (output, os) =
            Owner::from_module_ref_with_verification_budget_v12(&candidate, budget).unwrap();
        budget.reserve_storage(os.retained_storage()).unwrap();
        assert!(matches!(
            check_canonical_kir_load_forwarding_v1(
                inventory.owner(),
                &output,
                &[Row { first, load: last }],
                budget
            ),
            Err(Error::Rule("Load provenance or closed barrier"))
        ));
        drop(output);
        drop(candidate);
        budget
            .release_storage(os.retained_storage() + cs.retained_storage())
            .unwrap();
    });
}

#[test]
fn changed_access_or_a_new_initializing_store_never_reuses_the_old_load() {
    let mut access = fixture(ScalarType::U32);
    let Kind::Load {
        access: ref mut changed,
        ..
    } = operations(&mut access)[5].kind
    else {
        panic!()
    };
    changed.alignment = 2;
    assert_rows(access, 0);
    let mut store = fixture(ScalarType::U32);
    operations(&mut store)[4] = Operation::new(
        vec![],
        Kind::Store {
            pointer: ValueId(2),
            value: ValueId(0),
            access: MemoryAccess::new(AddressSpace::Private, 4),
        },
    );
    with_memory(store, |inventory, memory, budget| {
        let (plan, _) =
            CanonicalKirLoadForwardingPlanV1::derive(inventory, memory, budget).unwrap();
        assert_eq!(
            plan.rows(),
            &[Row {
                first: coordinate(5),
                load: coordinate(6)
            }]
        );
    });
}

#[test]
fn block_boundaries_and_reused_function_local_ids_do_not_share_seeds() {
    let mut module = fixture(ScalarType::U32);
    let body = module.functions[0].body.as_mut().unwrap();
    let tail = body.blocks[0].operations.split_off(5);
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(14),
        arguments: vec![],
    });
    let mut next = BasicBlock::new(BlockId(14));
    next.operations = tail;
    next.terminator = Some(Terminator::Return {
        values: vec![ValueId(5)],
    });
    body.blocks.push(next);
    assert_rows(module, 0);
    let mut two = fixture(ScalarType::U32);
    let mut other = two.functions[0].clone();
    other.id = "g".into();
    two.functions.push(other);
    with_memory(two, |inventory, memory, budget| {
        let (plan, _) =
            CanonicalKirLoadForwardingPlanV1::derive(inventory, memory, budget).unwrap();
        assert_eq!(plan.rows().len(), 4);
        assert_eq!(plan.rows()[0].first.block.function.0, 0);
        assert_eq!(plan.rows()[2].first.block.function.0, 1);
    });
}

#[test]
fn a_loop_reinitializes_its_slot_and_keeps_its_first_load_on_every_iteration() {
    let mut module = fixture(ScalarType::U32);
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[0].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(20),
        then_target: BlockId(91),
        then_arguments: vec![],
        else_target: BlockId(14),
        else_arguments: vec![],
    });
    let mut exit = BasicBlock::new(BlockId(14));
    exit.terminator = Some(Terminator::Return {
        values: vec![ValueId(5)],
    });
    body.blocks.push(exit);
    assert_rows(module, 2);
}

#[test]
fn foreign_inventory_stale_copy_and_one_short_apply_cannot_mutate() {
    with_memory(fixture(ScalarType::U32), |inventory, memory, budget| {
        let (other, os) =
            Owner::from_module_ref_with_verification_budget_v12(inventory.owner().module(), budget)
                .unwrap();
        budget.reserve_storage(os.retained_storage()).unwrap();
        let (foreign, fs) = Inventory::derive(&other, budget).unwrap();
        budget.reserve_storage(fs.retained_storage()).unwrap();
        assert!(matches!(
            CanonicalKirLoadForwardingPlanV1::derive(&foreign, memory, budget),
            Err(Error::ForeignSubject)
        ));
        for mode in 0..3 {
            let (plan, ps) =
                CanonicalKirLoadForwardingPlanV1::derive(inventory, memory, budget).unwrap();
            budget.reserve_storage(ps.retained_storage()).unwrap();
            let (mut candidate, cs) = inventory
                .owner()
                .copy_module_for_transformation_v12(budget)
                .unwrap();
            budget.reserve_storage(cs.retained_storage()).unwrap();
            let mut work = Work::new(if mode == 2 {
                3 + inventory.owner().canonical().canonical_bytes().len() + 1
            } else {
                WORK
            });
            let mut check = Budget::new(&mut work, STORAGE);
            check.reserve_storage(budget.storage()).unwrap();
            if mode == 1 {
                candidate.id = "changed".into();
            }
            let result = plan.apply(
                if mode == 0 { &other } else { inventory.owner() },
                &mut candidate,
                &mut check,
            );
            assert!(matches!(
                (mode, result),
                (0, Err(Error::ForeignSubject))
                    | (1, Err(Error::StaleCandidate))
                    | (2, Err(Error::Resource(Resource::Work(_))))
            ));
            assert!(matches!(
                operations(&mut candidate)[5].kind,
                Kind::Load { .. }
            ));
            assert!(matches!(
                operations(&mut candidate)[6].kind,
                Kind::Load { .. }
            ));
            drop(candidate);
            budget
                .release_storage(cs.retained_storage() + ps.retained_storage())
                .unwrap();
        }
        drop(foreign);
        budget.release_storage(fs.retained_storage()).unwrap();
        drop(other);
        budget.release_storage(os.retained_storage()).unwrap();
    });
}

#[test]
fn independent_replay_rejects_forged_rows_and_first_load_or_metadata_changes() {
    with_memory(fixture(ScalarType::U32), |inventory, memory, budget| {
        let (plan, ps) =
            CanonicalKirLoadForwardingPlanV1::derive(inventory, memory, budget).unwrap();
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
        let a = applied.rows()[0];
        let b = applied.rows()[1];
        for rows in [
            vec![],
            vec![a],
            vec![b, a],
            vec![a, a, b],
            vec![
                Row {
                    first: coordinate(1),
                    ..a
                },
                b,
            ],
            vec![
                Row {
                    first: coordinate(5),
                    ..a
                },
                b,
            ],
        ] {
            assert!(
                check_canonical_kir_load_forwarding_v1(inventory.owner(), &output, &rows, budget)
                    .is_err()
            );
        }
        for mutation in 0..5 {
            let (mut changed, ms) = output.copy_module_for_transformation_v12(budget).unwrap();
            budget.reserve_storage(ms.retained_storage()).unwrap();
            match mutation {
                0 => changed.id = "changed".into(),
                1 => changed.functions[0].id = "changed".into(),
                2 => changed.functions[0].body.as_mut().unwrap().blocks[0].id = BlockId(17),
                3 => {
                    operations(&mut changed)[3].kind = Kind::Binary {
                        op: BinaryOp::BitOr,
                        lhs: ValueId(0),
                        rhs: ValueId(0),
                    }
                }
                _ => {
                    changed.functions[0].body.as_mut().unwrap().blocks[0].terminator =
                        Some(Terminator::Return {
                            values: vec![ValueId(3)],
                        })
                }
            }
            let (other, rs) =
                Owner::from_module_ref_with_verification_budget_v12(&changed, budget).unwrap();
            budget.reserve_storage(rs.retained_storage()).unwrap();
            assert!(
                check_canonical_kir_load_forwarding_v1(
                    inventory.owner(),
                    &other,
                    applied.rows(),
                    budget
                )
                .is_err()
            );
            drop(other);
            drop(changed);
            budget
                .release_storage(rs.retained_storage() + ms.retained_storage())
                .unwrap();
        }
        drop(output);
        drop(candidate);
        drop(applied);
        budget
            .release_storage(os.retained_storage() + cs.retained_storage() + ps.retained_storage())
            .unwrap();
    });
}

#[test]
fn plan_exact_and_one_short_work_storage_release_all_initialization_scratch() {
    with_memory(fixture(ScalarType::U32), |inventory, memory, outer| {
        let floor = outer.storage();
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        let (plan, receipt) =
            CanonicalKirLoadForwardingPlanV1::derive(inventory, memory, &mut budget).unwrap();
        let cost = budget.work();
        let peak = budget.peak_storage();
        assert!(peak > floor + receipt.retained_storage());
        drop(plan);
        assert_eq!(budget.storage(), floor);
        for (limit, storage, ok) in [
            (cost, peak, true),
            (cost - 1, peak, false),
            (cost, peak - 1, false),
        ] {
            let mut work = Work::new(limit);
            let mut budget = Budget::new(&mut work, storage);
            budget.reserve_storage(floor).unwrap();
            assert_eq!(
                CanonicalKirLoadForwardingPlanV1::derive(inventory, memory, &mut budget).is_ok(),
                ok
            );
            assert_eq!(budget.storage(), floor);
            if ok {
                assert_eq!(budget.work(), cost);
            } else if limit == cost {
                assert!(budget.failed_storage().is_some());
            }
        }
    });
}

#[test]
fn independent_memory_census_replay_has_exact_and_one_short_resource_boundaries() {
    with_memory(fixture(ScalarType::U32), |inventory, memory, outer| {
        let (plan, ps) =
            CanonicalKirLoadForwardingPlanV1::derive(inventory, memory, outer).unwrap();
        outer.reserve_storage(ps.retained_storage()).unwrap();
        let (mut candidate, cs) = inventory
            .owner()
            .copy_module_for_transformation_v12(outer)
            .unwrap();
        outer.reserve_storage(cs.retained_storage()).unwrap();
        let applied = plan
            .apply(inventory.owner(), &mut candidate, outer)
            .unwrap();
        let (output, os) =
            Owner::from_module_ref_with_verification_budget_v12(&candidate, outer).unwrap();
        outer.reserve_storage(os.retained_storage()).unwrap();
        let floor = outer.storage();
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        let _ = check_canonical_kir_load_forwarding_v1(
            inventory.owner(),
            &output,
            applied.rows(),
            &mut budget,
        )
        .unwrap();
        let cost = budget.work();
        let peak = budget.peak_storage();
        assert_eq!(budget.storage(), floor);
        for (limit, storage, ok) in [
            (cost, peak, true),
            (cost - 1, peak, false),
            (cost, peak - 1, false),
        ] {
            let mut work = Work::new(limit);
            let mut budget = Budget::new(&mut work, storage);
            budget.reserve_storage(floor).unwrap();
            assert_eq!(
                check_canonical_kir_load_forwarding_v1(
                    inventory.owner(),
                    &output,
                    applied.rows(),
                    &mut budget
                )
                .is_ok(),
                ok
            );
            assert_eq!(budget.storage(), floor);
            if ok {
                assert_eq!(budget.work(), cost);
            } else if limit == cost {
                assert!(budget.failed_storage().is_some());
            }
        }
        drop(output);
        drop(candidate);
        drop(applied);
        outer
            .release_storage(os.retained_storage() + cs.retained_storage() + ps.retained_storage())
            .unwrap();
    });
}
