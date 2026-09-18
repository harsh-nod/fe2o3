use super::*;
use fe2o3_kernel_ir::{
    AccessMode, BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work, Constant, Function,
    Signature, Terminator, ValueDef,
};

const WORK: usize = 100_000_000;
const STORAGE: usize = 64 * 1024 * 1024;
const FLOOR: usize = 29;
fn scalar() -> Type {
    Type::Scalar(ScalarType::U32)
}
fn access() -> MemoryAccess {
    MemoryAccess::new(AddressSpace::Private, 4)
}
fn write(pointer: u32) -> Operation {
    Operation::new(
        vec![],
        Kind::Store {
            pointer: ValueId(pointer),
            value: ValueId(2),
            access: access(),
        },
    )
}
fn read(result: u32) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(result), scalar()),
        Kind::Load {
            pointer: ValueId(0),
            access: access(),
        },
    )
}
fn fixture() -> Module {
    let mut block = BasicBlock::new(BlockId(91));
    block.operations = vec![
        write(0),
        Operation::effect_free(
            ValueDef::new(ValueId(3), scalar()),
            Kind::Constant(Constant::U32(9)),
        ),
        read(4),
        read(5),
    ];
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(5)],
    });
    let mut module = Module::new("private-store-forwarding");
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(
            vec![
                Type::pointer(scalar(), AddressSpace::Private, AccessMode::ReadWrite),
                Type::pointer(scalar(), AddressSpace::Private, AccessMode::ReadWrite),
                scalar(),
            ],
            vec![scalar()],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![block],
    ));
    module
}
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
fn with_memory(
    module: Module,
    body: impl FnOnce(&Inventory<'_>, &MemorySsa<'_, '_>, &mut Budget<'_>),
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
    body(&inventory, &memory, &mut budget);
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
            CanonicalKirStoreForwardingPlanV1::derive(inventory, memory, budget).unwrap();
        assert_eq!(plan.rows().len(), expected);
        assert_eq!(
            receipt.retained_storage(),
            size_of::<CanonicalKirStoreForwardingPlanV1<'_, '_, '_>>()
                + plan.rows.capacity() * size_of::<Row>()
        );
    });
}

#[test]
fn actual_store_and_both_loads_have_an_exact_independently_replayed_value_relation() {
    with_memory(fixture(), |inventory, memory, budget| {
        let (plan, ps) =
            CanonicalKirStoreForwardingPlanV1::derive(inventory, memory, budget).unwrap();
        budget.reserve_storage(ps.retained_storage()).unwrap();
        assert_eq!(
            plan.rows(),
            &[
                Row {
                    store: coordinate(0),
                    load: coordinate(2)
                },
                Row {
                    store: coordinate(0),
                    load: coordinate(3)
                }
            ]
        );
        let (mut candidate, cs) = inventory
            .owner()
            .copy_module_for_transformation_v12(budget)
            .unwrap();
        budget.reserve_storage(cs.retained_storage()).unwrap();
        let applied = plan
            .apply(inventory.owner(), &mut candidate, budget)
            .unwrap();
        assert_eq!(operations(&mut candidate)[0], write(0));
        for row in applied.rows() {
            assert_eq!(
                operations(&mut candidate)[row.load.operation as usize].kind,
                Kind::Binary {
                    op: BinaryOp::BitOr,
                    lhs: ValueId(2),
                    rhs: ValueId(2)
                }
            );
        }
        let (output, os) =
            Owner::from_module_ref_with_verification_budget_v12(&candidate, budget).unwrap();
        budget.reserve_storage(os.retained_storage()).unwrap();
        let (checked, _) = applied.check_output(&output, budget).unwrap();
        assert!(std::ptr::eq(checked.input(), inventory.owner()));
        assert!(std::ptr::eq(checked.output(), &output));
        assert!(!checked.grants_authority());
        drop(checked);
        drop(output);
        budget.release_storage(os.retained_storage()).unwrap();
        drop(candidate);
        budget.release_storage(cs.retained_storage()).unwrap();
        drop(applied);
        budget.release_storage(ps.retained_storage()).unwrap();
    });
}

#[test]
fn foreign_inventory_and_equal_byte_foreign_owner_are_not_substitutable() {
    with_memory(fixture(), |inventory, memory, budget| {
        let (other, os) =
            Owner::from_module_ref_with_verification_budget_v12(inventory.owner().module(), budget)
                .unwrap();
        budget.reserve_storage(os.retained_storage()).unwrap();
        let (foreign, fs) = Inventory::derive(&other, budget).unwrap();
        budget.reserve_storage(fs.retained_storage()).unwrap();
        assert!(matches!(
            CanonicalKirStoreForwardingPlanV1::derive(&foreign, memory, budget),
            Err(Error::ForeignSubject)
        ));
        let (plan, ps) =
            CanonicalKirStoreForwardingPlanV1::derive(inventory, memory, budget).unwrap();
        budget.reserve_storage(ps.retained_storage()).unwrap();
        let (mut candidate, cs) = inventory
            .owner()
            .copy_module_for_transformation_v12(budget)
            .unwrap();
        budget.reserve_storage(cs.retained_storage()).unwrap();
        assert!(matches!(
            plan.apply(&other, &mut candidate, budget),
            Err(Error::ForeignSubject)
        ));
        assert_eq!(&candidate, inventory.owner().module());
        budget.release_storage(ps.retained_storage()).unwrap();
        drop(candidate);
        budget.release_storage(cs.retained_storage()).unwrap();
        drop(foreign);
        budget.release_storage(fs.retained_storage()).unwrap();
        drop(other);
        budget.release_storage(os.retained_storage()).unwrap();
    });
}

#[test]
fn stale_candidate_and_prepaid_mutation_denial_leave_every_load_unchanged() {
    with_memory(fixture(), |inventory, memory, budget| {
        for stale in [true, false] {
            let (plan, ps) =
                CanonicalKirStoreForwardingPlanV1::derive(inventory, memory, budget).unwrap();
            budget.reserve_storage(ps.retained_storage()).unwrap();
            let (mut candidate, cs) = inventory
                .owner()
                .copy_module_for_transformation_v12(budget)
                .unwrap();
            budget.reserve_storage(cs.retained_storage()).unwrap();
            let bytes = inventory.owner().canonical().canonical_bytes().len();
            let mut component_work = Work::new(if stale { WORK } else { 3 + bytes + 1 });
            let mut component = Budget::new(&mut component_work, STORAGE);
            component.reserve_storage(budget.storage()).unwrap();
            if stale {
                operations(&mut candidate)[1].kind = Kind::Constant(Constant::U32(10));
            }
            let result = plan.apply(inventory.owner(), &mut candidate, &mut component);
            if stale {
                assert!(matches!(result, Err(Error::StaleCandidate)));
            } else {
                assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
                assert_eq!(component.work(), 3 + bytes);
                assert_eq!(component_work.failed_work(), Some(3 + bytes + 2));
            }
            assert!(matches!(
                operations(&mut candidate)[2].kind,
                Kind::Load { .. }
            ));
            assert!(matches!(
                operations(&mut candidate)[3].kind,
                Kind::Load { .. }
            ));
            drop(candidate);
            budget.release_storage(cs.retained_storage()).unwrap();
            budget.release_storage(ps.retained_storage()).unwrap();
        }
    });
}

#[test]
fn forged_wrong_store_duplicate_missing_and_reversed_rows_are_rejected() {
    with_memory(fixture(), |inventory, memory, budget| {
        let (plan, ps) =
            CanonicalKirStoreForwardingPlanV1::derive(inventory, memory, budget).unwrap();
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
        let first = applied.rows()[0];
        let second = applied.rows()[1];
        for rows in [
            vec![],
            vec![first],
            vec![second, first],
            vec![first, first, second],
            vec![
                Row {
                    store: coordinate(1),
                    ..first
                },
                second,
            ],
        ] {
            assert!(
                check_canonical_kir_store_forwarding_v1(inventory.owner(), &output, &rows, budget)
                    .is_err()
            );
        }
        drop(output);
        budget.release_storage(os.retained_storage()).unwrap();
        drop(candidate);
        budget.release_storage(cs.retained_storage()).unwrap();
        drop(applied);
        budget.release_storage(ps.retained_storage()).unwrap();
    });
}

#[test]
fn independent_replay_rejects_wrong_value_and_unrelated_payload_mutations() {
    with_memory(fixture(), |inventory, memory, budget| {
        for wrong_value in [true, false] {
            let (plan, ps) =
                CanonicalKirStoreForwardingPlanV1::derive(inventory, memory, budget).unwrap();
            budget.reserve_storage(ps.retained_storage()).unwrap();
            let (mut candidate, cs) = inventory
                .owner()
                .copy_module_for_transformation_v12(budget)
                .unwrap();
            budget.reserve_storage(cs.retained_storage()).unwrap();
            let applied = plan
                .apply(inventory.owner(), &mut candidate, budget)
                .unwrap();
            if wrong_value {
                operations(&mut candidate)[2].kind = Kind::Binary {
                    op: BinaryOp::BitOr,
                    lhs: ValueId(3),
                    rhs: ValueId(3),
                };
            } else {
                operations(&mut candidate)[1].kind = Kind::Constant(Constant::U32(10));
            }
            let (output, os) =
                Owner::from_module_ref_with_verification_budget_v12(&candidate, budget).unwrap();
            budget.reserve_storage(os.retained_storage()).unwrap();
            assert!(matches!(
                applied.check_output(&output, budget),
                Err(Error::Rule(_))
            ));
            drop(output);
            budget.release_storage(os.retained_storage()).unwrap();
            drop(candidate);
            budget.release_storage(cs.retained_storage()).unwrap();
            drop(applied);
            budget.release_storage(ps.retained_storage()).unwrap();
        }
    });
}

#[test]
fn unknown_alias_store_different_pointer_and_alignment_do_not_forward() {
    let mut alias = fixture();
    operations(&mut alias).insert(2, write(1));
    assert_rows(alias, 0);
    let mut pointer = fixture();
    operations(&mut pointer)[0] = write(1);
    assert_rows(pointer, 0);
    let mut alignment = fixture();
    if let Kind::Load { access, .. } = &mut operations(&mut alignment)[2].kind {
        access.alignment = 1;
    }
    assert_rows(alignment, 0);
}

#[test]
fn first_uninitialized_read_is_never_eliminated_but_a_later_store_reestablishes_the_rule() {
    let mut input = fixture();
    operations(&mut input).insert(0, read(6));
    assert_rows(input, 2);
    let mut no_store = fixture();
    operations(&mut no_store).remove(0);
    assert_rows(no_store, 0);
}

#[test]
fn volatile_store_load_and_global_address_space_are_not_forwarded() {
    for ordinal in [0, 2] {
        let mut input = fixture();
        match &mut operations(&mut input)[ordinal].kind {
            Kind::Load { access, .. } | Kind::Store { access, .. } => access.volatile = true,
            _ => unreachable!(),
        }
        assert_rows(input, 0);
    }
    let mut input = fixture();
    for ty in &mut input.functions[0].signature.parameters[..2] {
        *ty = Type::pointer(scalar(), AddressSpace::Global, AccessMode::ReadWrite);
    }
    for operation in operations(&mut input) {
        if let Kind::Load { access, .. } | Kind::Store { access, .. } = &mut operation.kind {
            access.address_space = AddressSpace::Global;
        }
    }
    assert_rows(input, 0);
}

#[test]
fn calls_and_possible_trapping_arithmetic_clear_the_seed() {
    let mut input = fixture();
    operations(&mut input).insert(
        2,
        Operation::new(
            vec![],
            Kind::Call {
                callee: "opaque".into(),
                arguments: vec![],
            },
        ),
    );
    input.functions.push(Function::declaration(
        "opaque",
        Signature::new(vec![], vec![]),
    ));
    assert_rows(input, 0);
    let mut input = fixture();
    operations(&mut input).insert(
        2,
        Operation::effect_free(
            ValueDef::new(ValueId(6), scalar()),
            Kind::Binary {
                op: BinaryOp::Divide,
                lhs: ValueId(2),
                rhs: ValueId(3),
            },
        ),
    );
    assert_rows(input, 0);
}

#[test]
fn atomic_and_convergent_barrier_clear_the_private_seed_even_for_other_memory() {
    use fe2o3_kernel_ir::{
        Atomic, AtomicKind, Barrier, BarrierSemantics, MemoryOrdering, SynchronizationScope,
    };
    let mut atomic = fixture();
    atomic.functions[0].signature.parameters.push(Type::pointer(
        scalar(),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    ));
    atomic.functions[0]
        .body
        .as_mut()
        .unwrap()
        .parameters
        .push(ValueId(6));
    operations(&mut atomic).insert(
        2,
        Operation::new(
            vec![],
            Kind::Atomic(Atomic {
                kind: AtomicKind::Store,
                pointer: ValueId(6),
                value: Some(ValueId(2)),
                compare: None,
                access: MemoryAccess::new(AddressSpace::Global, 4),
                scope: SynchronizationScope::Device,
                ordering: MemoryOrdering::Relaxed,
                failure_ordering: None,
            }),
        ),
    );
    assert_rows(atomic, 0);
    let mut barrier = fixture();
    operations(&mut barrier).insert(
        2,
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
    );
    assert_rows(barrier, 0);
}

#[test]
fn exact_signed_unsigned_widths_are_preserved_without_integer_conversion() {
    for (scalar, constant, alignment) in [
        (ScalarType::I8, Constant::I8(-1), 1),
        (ScalarType::U16, Constant::U16(u16::MAX), 2),
        (ScalarType::I64, Constant::I64(i64::MIN), 8),
        (ScalarType::U64, Constant::U64(u64::MAX), 8),
    ] {
        let ty = Type::Scalar(scalar);
        let mut input = fixture();
        input.functions[0].signature.parameters = vec![
            Type::pointer(ty.clone(), AddressSpace::Private, AccessMode::ReadWrite),
            Type::pointer(ty.clone(), AddressSpace::Private, AccessMode::ReadWrite),
            ty.clone(),
        ];
        input.functions[0].signature.results = vec![ty.clone()];
        for operation in operations(&mut input) {
            for result in &mut operation.results {
                result.ty = ty.clone();
            }
            match &mut operation.kind {
                Kind::Constant(value) => *value = constant.clone(),
                Kind::Load { access, .. } | Kind::Store { access, .. } => {
                    access.alignment = alignment
                }
                _ => unreachable!(),
            }
        }
        assert_rows(input, 2);
    }
}

#[test]
fn branch_duplicate_edges_and_nonmonotonic_block_ids_never_supply_cross_block_dominance() {
    let mut input = fixture();
    input.functions[0].signature.parameters.push(Type::BOOL);
    let body = input.functions[0].body.as_mut().unwrap();
    body.parameters.push(ValueId(6));
    let reads = body.blocks[0].operations.split_off(2);
    body.blocks[0].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(6),
        then_target: BlockId(2),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let mut next = BasicBlock::new(BlockId(2));
    next.operations = reads;
    next.terminator = Some(Terminator::Return {
        values: vec![ValueId(5)],
    });
    body.blocks.push(next);
    assert_rows(input, 0);
}

#[test]
fn local_rewrites_work_inside_a_multiblock_loop_and_keep_function_provenance_disjoint() {
    let mut input = fixture();
    input.functions[0].signature.parameters.push(Type::BOOL);
    let body = input.functions[0].body.as_mut().unwrap();
    body.parameters.push(ValueId(6));
    body.blocks[0].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(6),
        then_target: BlockId(2),
        then_arguments: vec![],
        else_target: BlockId(7),
        else_arguments: vec![],
    });
    let mut exit = BasicBlock::new(BlockId(2));
    exit.operations = vec![write(0), read(7)];
    exit.terminator = Some(Terminator::Return {
        values: vec![ValueId(7)],
    });
    let mut cycle = BasicBlock::new(BlockId(7));
    cycle.operations = vec![write(0), read(8)];
    cycle.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(6),
        then_target: BlockId(7),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    body.blocks.extend([exit, cycle]);
    let mut other = input.functions[0].clone();
    other.id = "other".into();
    input.functions.push(other);
    with_memory(input, |inventory, memory, budget| {
        let (plan, ps) =
            CanonicalKirStoreForwardingPlanV1::derive(inventory, memory, budget).unwrap();
        budget.reserve_storage(ps.retained_storage()).unwrap();
        assert_eq!(plan.rows().len(), 8);
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
        assert!(applied.check_output(&output, budget).is_ok());
        let mut foreign_rows = applied.rows().to_vec();
        foreign_rows[0].store.block.function = fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(1);
        assert!(matches!(
            check_canonical_kir_store_forwarding_v1(
                inventory.owner(),
                &output,
                &foreign_rows,
                budget
            ),
            Err(Error::Rule("Store/Load provenance or barrier"))
        ));
        drop(output);
        budget.release_storage(os.retained_storage()).unwrap();
        drop(candidate);
        budget.release_storage(cs.retained_storage()).unwrap();
        drop(applied);
        budget.release_storage(ps.retained_storage()).unwrap();
    });
}

#[test]
fn floats_and_boolean_results_are_not_integer_identity_claims() {
    for (ty, constant, alignment) in [
        (Type::Scalar(ScalarType::F32), Constant::F32Bits(9), 4),
        (Type::BOOL, Constant::Bool(true), 1),
    ] {
        let mut input = fixture();
        input.functions[0].signature.parameters = vec![
            Type::pointer(ty.clone(), AddressSpace::Private, AccessMode::ReadWrite),
            Type::pointer(ty.clone(), AddressSpace::Private, AccessMode::ReadWrite),
            ty.clone(),
        ];
        input.functions[0].signature.results = vec![ty.clone()];
        for operation in operations(&mut input) {
            for result in &mut operation.results {
                result.ty = ty.clone();
            }
            match &mut operation.kind {
                Kind::Constant(value) => *value = constant.clone(),
                Kind::Load { access, .. } | Kind::Store { access, .. } => {
                    access.alignment = alignment
                }
                _ => unreachable!(),
            }
        }
        assert_rows(input, 0);
    }
}

#[test]
fn literal_plan_work_boundary_and_partial_storage_failure_restore_floor() {
    with_memory(fixture(), |inventory, memory, _| {
        // Component accounting: source/inventory/MemorySSA were constructed
        // separately; this literal schedule measures only the proposed plan.
        // 6 header/vector + 1 block + 4*5 visits + 3*10 queries + 2 pushes + 2 final = 61.
        for limit in [61, 60] {
            let mut work = Work::new(limit);
            let mut budget = Budget::new(&mut work, STORAGE);
            budget.reserve_storage(FLOOR).unwrap();
            let result = CanonicalKirStoreForwardingPlanV1::derive(inventory, memory, &mut budget);
            assert_eq!(budget.storage(), FLOOR);
            if limit == 61 {
                assert!(result.is_ok());
                assert_eq!(budget.work(), 61);
            } else {
                assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
                assert_eq!(budget.work(), 59);
                assert_eq!(work.failed_work(), Some(61));
            }
        }
        let header = size_of::<CanonicalKirStoreForwardingPlanV1<'_, '_, '_>>();
        let rows = 4 * size_of::<Row>();
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, FLOOR + header + rows - 1);
        budget.reserve_storage(FLOOR).unwrap();
        assert!(matches!(
            CanonicalKirStoreForwardingPlanV1::derive(inventory, memory, &mut budget),
            Err(Error::Resource(Resource::Storage { .. }))
        ));
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.peak_storage(), FLOOR + header);
        assert_eq!(budget.work(), 6);
    });
}
