use super::*;
use crate::{CanonicalKirInventoryV18, CanonicalKirMemorySsaNodeV1, CanonicalKirMemorySsaV18};
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, ExecutionOperationV15 as Execution, ExecutionRoleV15 as Role,
    MemoryAccess, StorageLayoutIdV1 as LayoutId, StorageLayoutKindV1, StorageLayoutLimitsV1,
    StorageLayoutV1, StorageOperationV1 as Storage,
};

fn storage_fixture(checked: bool) -> Module {
    let mut module = fixture(Constant::U32(1), checked, false);
    module.functions[0].role = fe2o3_kernel_ir::FunctionRole::KernelEntry;
    module.kernels.push(fe2o3_kernel_ir::Kernel::new(
        "f",
        "f",
        fe2o3_kernel_ir::LaunchDomain::D1 {
            x: fe2o3_kernel_ir::LaunchExtent::Static(1),
        },
    ));
    module.storage_layouts = vec![StorageLayoutV1 {
        size: 4,
        alignment: 4,
        kind: StorageLayoutKindV1::Scalar(ScalarType::U32),
    }];
    let body = module.functions[0].body.as_mut().unwrap();
    let entry = &mut body.blocks[0].operations;
    entry.push(KirOperation::effect_free(
        ValueDef::new(ValueId(10), Type::Execution(Role::Context)),
        OperationKind::Execution(Execution::ContextIssue),
    ));
    entry.push(KirOperation::effect_free(
        ValueDef::new(ValueId(11), Type::Execution(Role::Workgroup)),
        OperationKind::Execution(Execution::WorkgroupDerive {
            context: ValueId(10),
        }),
    ));
    entry.push(KirOperation::effect_free(
        ValueDef::new(
            ValueId(12),
            Type::pointer(
                Type::StorageObject(LayoutId(0)),
                AddressSpace::Private,
                AccessMode::ReadWrite,
            ),
        ),
        OperationKind::Alloca {
            element: Type::StorageObject(LayoutId(0)),
            count: None,
            address_space: AddressSpace::Private,
            alignment: 4,
        },
    ));
    let access = MemoryAccess::new(AddressSpace::Private, 4);
    body.blocks[2].operations.push(KirOperation::new(
        vec![],
        OperationKind::Storage(Storage::WriteValue {
            address: ValueId(12),
            value: ValueId(2),
            access,
        }),
    ));
    body.blocks[2].operations.push(KirOperation::effect_free(
        ValueDef::new(ValueId(13), Type::Scalar(ScalarType::U32)),
        OperationKind::Storage(Storage::ReadValue {
            address: ValueId(12),
            access,
        }),
    ));
    body.blocks[3].operations.push(KirOperation::new(
        vec![],
        OperationKind::Execution(Execution::ScopeEnd {
            workgroup: ValueId(11),
            discarded: vec![],
        }),
    ));
    module
}

fn with_inventory_v18(
    module: Module,
    run: impl FnOnce(&CanonicalKirInventoryV18<'_>, &mut Budget<'_>),
) {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let (owner, os) = Owner18::from_module_ref_with_verification_budget_v18(
        &module,
        StorageLayoutLimitsV1 {
            rows: 2,
            edges: 0,
            containment_depth: 1,
            object_bytes: 4,
        },
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(os.retained_storage()).unwrap();
    let (inventory, is) = CanonicalKirInventoryV18::derive_v18(&owner, &mut budget).unwrap();
    budget.reserve_storage(is.retained_storage()).unwrap();
    let floor = budget.storage();
    run(&inventory, &mut budget);
    assert_eq!(budget.storage(), floor);
    drop(inventory);
    drop(owner);
    budget
        .release_storage(is.retained_storage() + os.retained_storage())
        .unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn v18_loops_and_memory_ssa_borrow_the_same_actual_storage_execution_owner() {
    for checked in [false, true] {
        with_inventory_v18(storage_fixture(checked), |inventory, budget| {
            let floor = budget.storage();
            let (loops, ls) =
                CanonicalKirLoopsV18::derive_v18(inventory, Default::default(), budget).unwrap();
            budget.reserve_storage(ls.retained_storage()).unwrap();
            loops.replay(inventory, Default::default(), budget).unwrap();
            assert!(std::ptr::eq(loops.inventory(), inventory));
            assert_eq!(loops.members(0, budget).unwrap(), &[block(1), block(2)]);
            assert_eq!(
                loops.recurrences(0, budget).unwrap()[0]
                    .overflow()
                    .is_some(),
                checked
            );
            let (memory, ms) =
                CanonicalKirMemorySsaV18::derive_v18(inventory, Default::default(), budget)
                    .unwrap();
            budget.reserve_storage(ms.retained_storage()).unwrap();
            assert!(std::ptr::eq(memory.inventory(), inventory));
            let mut reads = 0;
            let mut barriers = 0;
            for row in inventory.operations() {
                if let Some(node) = memory.operation(row.coordinate, budget).unwrap() {
                    match memory.node(node, budget).unwrap() {
                        CanonicalKirMemorySsaNodeV1::Use { operation, .. } => {
                            assert_eq!(*operation, row.coordinate);
                            reads += 1;
                        }
                        CanonicalKirMemorySsaNodeV1::Def { operation, .. } => {
                            assert_eq!(*operation, row.coordinate);
                            barriers += 1;
                        }
                        other => panic!("operation maps to non-operation node: {other:?}"),
                    }
                }
            }
            assert_eq!(reads, 1);
            assert!(barriers >= 4); // Write plus all three execution operations.
            drop(memory);
            drop(loops);
            budget
                .release_storage(ms.retained_storage() + ls.retained_storage())
                .unwrap();
            assert_eq!(budget.storage(), floor);
        });
    }
}

#[test]
fn v18_scoped_cfg_uses_entry_ordinal_and_actual_complete_successor_graph() {
    let mut source = storage_fixture(false);
    let body = source.functions[0].body.as_mut().unwrap();
    // A disconnected self-cycle is not a reachable natural loop or dominator.
    body.blocks
        .push(basic(888, vec![], vec![], branch(888, vec![])));
    with_inventory_v18(source, |inventory, budget| {
        with_canonical_kir_control_flow_v18(
            inventory.owner(),
            Function(0),
            Default::default(),
            budget,
            |view, budget| {
                assert!(std::ptr::eq(view.owner(), inventory.owner()));
                assert!(view.is_reachable(block(0), budget)?);
                assert!(view.dominates(block(1), block(2), budget)?);
                assert!(!view.is_reachable(block(4), budget)?);
                assert!(!view.dominates(block(4), block(4), budget)?);
                Ok::<_, CanonicalKirControlFlowScopeErrorV1>(())
            },
        )
        .unwrap();
        let (loops, receipt) =
            CanonicalKirLoopsV18::derive_v18(inventory, Default::default(), budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        assert_eq!(loops.loop_count(), 1);
        loops.replay(inventory, Default::default(), budget).unwrap();
        drop(loops);
        budget.release_storage(receipt.retained_storage()).unwrap();
        let invalid = with_canonical_kir_control_flow_v18(
            inventory.owner(),
            Function(0),
            Default::default(),
            budget,
            |view, budget| {
                view.is_reachable(
                    Block {
                        function: Function(1),
                        block: 0,
                    },
                    budget,
                )
            },
        );
        assert!(matches!(
            invalid,
            Err(CanonicalKirControlFlowScopeErrorV1::InvalidBlock(_))
        ));
    });
}

#[test]
fn v18_loop_replay_rejects_foreign_inventory_and_same_count_fact_substitutions() {
    with_inventory_v18(storage_fixture(true), |inventory, budget| {
        let (foreign, fs) =
            CanonicalKirInventoryV18::derive_v18(inventory.owner(), budget).unwrap();
        budget.reserve_storage(fs.retained_storage()).unwrap();
        let (mut loops, ls) =
            CanonicalKirLoopsV18::derive_v18(inventory, Default::default(), budget).unwrap();
        budget.reserve_storage(ls.retained_storage()).unwrap();
        assert!(!loops.belongs_to(&foreign));
        assert_eq!(
            loops.replay(&foreign, Default::default(), budget),
            Err(Error::ForeignInventory)
        );
        loops.replay(inventory, Default::default(), budget).unwrap();
        let member = loops.members[0];
        loops.members[0] = block(0);
        assert_eq!(
            loops.replay(inventory, Default::default(), budget),
            Err(Error::ReplayMismatch)
        );
        loops.members[0] = member;
        let bits = loops.recurrences[0].step_bits;
        loops.recurrences[0].step_bits ^= 1;
        assert_eq!(
            loops.replay(inventory, Default::default(), budget),
            Err(Error::ReplayMismatch)
        );
        loops.recurrences[0].step_bits = bits;
        let edge = loops.edges[0];
        loops.edges[0] = Edge {
            source: block(0),
            successor: 0,
        };
        assert_eq!(
            loops.replay(inventory, Default::default(), budget),
            Err(Error::ReplayMismatch)
        );
        loops.edges[0] = edge;
        loops.replay(inventory, Default::default(), budget).unwrap();
        drop(loops);
        drop(foreign);
        budget
            .release_storage(ls.retained_storage() + fs.retained_storage())
            .unwrap();
    });
}

#[test]
fn v18_loops_preserve_duplicate_edge_occurrences_and_zero_step_nonmatches() {
    for duplicate in [false, true] {
        let mut source = storage_fixture(false);
        let body = source.functions[0].body.as_mut().unwrap();
        if duplicate {
            body.blocks[2].terminator = Some(Terminator::ConditionalBranch {
                condition: ValueId(1),
                then_target: BlockId(11),
                then_arguments: vec![ValueId(4)],
                else_target: BlockId(11),
                else_arguments: vec![ValueId(4)],
            });
        } else {
            body.blocks[0].operations[0].kind = OperationKind::Constant(Constant::U32(0));
        }
        with_inventory_v18(source, |inventory, budget| {
            let (loops, ls) =
                CanonicalKirLoopsV18::derive_v18(inventory, Default::default(), budget).unwrap();
            budget.reserve_storage(ls.retained_storage()).unwrap();
            assert_eq!(loops.loop_count(), 1);
            assert_eq!(
                loops.latch_edges(0, budget).unwrap().len(),
                if duplicate { 2 } else { 1 }
            );
            assert!(loops.recurrences(0, budget).unwrap().is_empty());
            loops.replay(inventory, Default::default(), budget).unwrap();
            drop(loops);
            budget.release_storage(ls.retained_storage()).unwrap();
        });
    }
}

#[test]
fn v18_loop_derivation_and_replay_have_exact_and_one_short_limits() {
    with_inventory_v18(storage_fixture(true), |inventory, outer| {
        let floor = outer.storage() + FLOOR;
        let run = |work_limit, storage_limit| {
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(floor).unwrap();
            let result = (|| {
                let (loops, ls) =
                    CanonicalKirLoopsV18::derive_v18(inventory, Default::default(), &mut budget)?;
                budget.reserve_storage(ls.retained_storage())?;
                let result = loops.replay(inventory, Default::default(), &mut budget);
                drop(loops);
                budget.release_storage(ls.retained_storage())?;
                result
            })();
            let kind = match &result {
                Ok(()) => 0,
                Err(Error::Resource(Resource::Work(_))) => 1,
                Err(Error::Resource(Resource::Storage(_))) => 2,
                other => panic!("unexpected loop result: {other:?}"),
            };
            assert_eq!(budget.storage(), floor);
            let observed = (
                kind,
                budget.work(),
                budget.peak_storage(),
                budget.failed_storage(),
            );
            drop(budget);
            (observed, work.failed_work())
        };
        let (full, failed) = run(LIMIT, LIMIT);
        assert_eq!(full.0, 0);
        assert_eq!(failed, None);
        assert_eq!(run(full.1, full.2).0, full);
        let work = run(full.1 - 1, full.2);
        assert_eq!(work.0.0, 1);
        assert!(work.1.is_some());
        assert_eq!(work.0.3, None);
        let storage = run(full.1, full.2 - 1);
        assert_eq!(storage.0.0, 2);
        assert!(storage.0.3.is_some());
        assert_eq!(storage.1, None);
    });
}
