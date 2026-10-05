use super::*;
use fe2o3_kernel_ir::{
    AccessMode, Axis, BasicBlock, BinaryOp, BlockId, CanonicalKernelIrWorkBudgetV1 as Work,
    ComparePredicate, Constant, ExecutionRoleV15 as Role, Function as IrFunction,
    IntrinsicOperation, Kernel, LaunchDomain, LaunchExtent, MemoryAccess, Module, Operation,
    ScalarType, Signature, StorageLayoutLimitsV1, Type, ValueDef, ValueId,
    VerifiedCanonicalKernelIrModuleV18 as Owner,
};

const LAYOUTS: StorageLayoutLimitsV1 = StorageLayoutLimitsV1 {
    rows: 16,
    edges: 32,
    containment_depth: 8,
    object_bytes: 4096,
};
fn value(id: u32, ty: Type, kind: Kind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(id), ty), kind)
}
fn index(id: u32, kind: IndexKind) -> Operation {
    value(
        id,
        Type::INDEX,
        Kind::Intrinsic(IntrinsicOperation::new(
            IntrinsicKind::InvocationIndex {
                kind,
                axis: Axis::X,
            },
            Type::INDEX,
        )),
    )
}
fn tile_block(id: u32, base: u32) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(id));
    block.operations = vec![
        value(
            10,
            Type::Execution(Role::Context),
            Kind::Execution(Execution::ContextIssue),
        ),
        value(
            11,
            Type::Execution(Role::Workgroup),
            Kind::Execution(Execution::WorkgroupDerive {
                context: ValueId(10),
            }),
        ),
        value(
            12,
            Type::Execution(Role::MaskedTileU32 {
                lanes: 64,
                elements: 3,
            }),
            Kind::Execution(Execution::MaskedTileLoadU32 {
                workgroup: ValueId(11),
                input: ValueId(0),
                base: ValueId(base),
                lanes: 64,
                elements: 3,
            }),
        ),
        Operation::new(
            vec![],
            Kind::Execution(Execution::ScopeEnd {
                workgroup: ValueId(11),
                discarded: vec![ValueId(12)],
            }),
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    block
}
fn module(blocks: Vec<BasicBlock>) -> Module {
    let mut module = Module::new("uniform-tile");
    module.functions.push(IrFunction::kernel_entry(
        "entry",
        Signature::new(
            vec![
                Type::slice(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::ReadOnly,
                ),
                Type::INDEX,
                Type::BOOL,
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        blocks,
    ));
    let mut kernel = Kernel::new(
        "entry",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    module.kernels.push(kernel);
    module
}
fn with_inventory<T>(module: &Module, run: impl FnOnce(&CanonicalKirInventoryV18<'_>) -> T) -> T {
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let (owner, receipt) =
        Owner::from_module_ref_with_verification_budget_v18(module, LAYOUTS, &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let (inventory, retained) = CanonicalKirInventoryV18::derive_v18(&owner, &mut budget).unwrap();
    budget.reserve_storage(retained.retained_storage()).unwrap();
    let result = run(&inventory);
    drop(inventory);
    budget.release_storage(retained.retained_storage()).unwrap();
    drop(owner);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 0);
    result
}
fn check(module: &Module) -> Result<()> {
    with_inventory(module, |inventory| {
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        budget.reserve_storage(31).unwrap();
        let result = check_canonical_tile_convergence_v160(inventory, Function(0), &mut budget);
        assert_eq!(budget.storage(), 31);
        result
    })
}

#[test]
fn tile_convergence_authenticates_kernel_abi_and_launch() {
    let mut input = module(vec![tile_block(7, 1)]);
    check(&input).unwrap();
    input.kernels[0].workgroup_size = None;
    assert_eq!(check(&input), Err(Error::KernelEntry));
    input.kernels[0].workgroup_size = Some(WorkgroupSize::new(32, 1, 1));
    assert!(matches!(check(&input), Err(Error::LaunchMismatch(_))));
    input.kernels.clear();
    input.functions[0].role = fe2o3_kernel_ir::FunctionRole::InternalHelper;
    input.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .clear();
    assert_eq!(check(&input), Err(Error::KernelEntry));
}

#[test]
fn tile_convergence_distinguishes_workgroup_and_lane_indices() {
    for kind in [
        IndexKind::Workgroup,
        IndexKind::WorkgroupSize,
        IndexKind::WorkgroupCount,
        IndexKind::Local,
        IndexKind::Global,
    ] {
        let mut block = tile_block(7, 3);
        block.operations.insert(0, index(3, kind));
        let result = check(&module(vec![block]));
        if matches!(kind, IndexKind::Local | IndexKind::Global) {
            assert!(
                matches!(result, Err(Error::VaryingTileInput(_))),
                "{result:?}"
            );
        } else {
            result.unwrap();
        }
    }
}

#[test]
fn tile_convergence_loaded_bases_are_varying_even_with_uniform_addresses() {
    let mut block = tile_block(7, 6);
    block.operations.splice(
        0..0,
        [
            value(
                3,
                Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::ReadOnly,
                ),
                Kind::SliceData { slice: ValueId(0) },
            ),
            value(
                4,
                Type::Scalar(ScalarType::U32),
                Kind::Load {
                    pointer: ValueId(3),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            ),
            value(
                6,
                Type::INDEX,
                Kind::Cast {
                    kind: fe2o3_kernel_ir::CastKind::ZeroExtend,
                    value: ValueId(4),
                    to: Type::INDEX,
                },
            ),
        ],
    );
    assert!(matches!(
        check(&module(vec![block])),
        Err(Error::VaryingTileInput(_))
    ));
}

fn diamond(varying_selector: bool, varying_argument: bool) -> Module {
    let mut entry = BasicBlock::new(BlockId(91));
    entry.operations = vec![
        index(3, IndexKind::Local),
        value(
            4,
            Type::BOOL,
            Kind::Compare {
                predicate: ComparePredicate::Equal,
                lhs: ValueId(3),
                rhs: ValueId(1),
            },
        ),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(if varying_selector { 4 } else { 2 }),
        then_target: BlockId(17),
        then_arguments: vec![ValueId(if varying_argument { 3 } else { 1 })],
        else_target: BlockId(17),
        else_arguments: vec![ValueId(1)],
    });
    let mut join = tile_block(17, 20);
    join.parameters
        .push(ValueDef::new(ValueId(20), Type::INDEX));
    module(vec![entry, join])
}

#[test]
fn tile_convergence_checks_parallel_edge_control_and_each_incoming_value() {
    check(&diamond(false, false)).unwrap();
    assert!(matches!(
        check(&diamond(true, false)),
        Err(Error::VaryingTileInput(_))
    ));
    assert!(matches!(
        check(&diamond(false, true)),
        Err(Error::VaryingTileInput(_))
    ));
}

fn loop_module(varying_selector: bool) -> Module {
    let mut entry = BasicBlock::new(BlockId(91));
    entry.operations = vec![
        index(3, IndexKind::Local),
        value(4, Type::INDEX, Kind::Constant(Constant::Index(0))),
    ];
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(22),
        arguments: vec![ValueId(4)],
    });
    let mut header = BasicBlock::new(BlockId(22));
    header
        .parameters
        .push(ValueDef::new(ValueId(21), Type::INDEX));
    header.operations = vec![value(
        5,
        Type::BOOL,
        Kind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs: ValueId(21),
            rhs: ValueId(if varying_selector { 3 } else { 1 }),
        },
    )];
    header.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(5),
        then_target: BlockId(23),
        then_arguments: vec![],
        else_target: BlockId(17),
        else_arguments: vec![],
    });
    let mut body = BasicBlock::new(BlockId(23));
    body.operations = vec![
        value(6, Type::INDEX, Kind::Constant(Constant::Index(1))),
        value(
            7,
            Type::INDEX,
            Kind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(21),
                rhs: ValueId(6),
            },
        ),
    ];
    body.terminator = Some(Terminator::Branch {
        target: BlockId(22),
        arguments: vec![ValueId(7)],
    });
    // Physical order deliberately differs from both DFS and execution order.
    module(vec![entry, tile_block(17, 1), body, header])
}

#[test]
fn tile_convergence_carries_divergent_backedge_history_to_later_collectives() {
    check(&loop_module(false)).unwrap();
    assert!(matches!(
        check(&loop_module(true)),
        Err(Error::VaryingArrival(_))
    ));
}

#[test]
fn tile_convergence_refuses_opaque_arrival_even_without_results() {
    let mut block = tile_block(7, 1);
    block.operations.insert(
        0,
        Operation::new(
            vec![],
            Kind::Call {
                callee: "opaque".into(),
                arguments: vec![],
            },
        ),
    );
    let mut input = module(vec![block]);
    input.functions.push(IrFunction::external_import(
        "opaque",
        Signature::new(vec![], vec![]),
    ));
    assert!(matches!(check(&input), Err(Error::UnsupportedArrival(_))));
}

#[test]
fn tile_convergence_ignores_unreachable_edges() {
    let mut input = diamond(true, false);
    let body = input.functions[0].body.as_mut().unwrap();
    let mut dead = BasicBlock::new(BlockId(99));
    dead.operations.push(index(90, IndexKind::Local));
    dead.terminator = Some(Terminator::Branch {
        target: BlockId(17),
        arguments: vec![ValueId(90)],
    });
    body.blocks.push(dead);
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(17),
        arguments: vec![ValueId(1)],
    });
    check(&input).unwrap();
}

#[test]
fn tile_convergence_exact_and_one_short_work_and_storage() {
    for module in [loop_module(false), reconverging_diamond(false, false)] {
        with_inventory(&module, |inventory| {
            let mut work = Work::new(usize::MAX);
            let mut budget = Budget::new(&mut work, usize::MAX);
            budget.reserve_storage(31).unwrap();
            check_canonical_tile_convergence_v160(inventory, Function(0), &mut budget).unwrap();
            let used = budget.work();
            let peak = budget.peak_storage();
            assert_eq!(budget.storage(), 31);
            for (work_limit, storage_limit) in [(used, peak), (used - 1, peak), (used, peak - 1)] {
                let mut work = Work::new(work_limit);
                let mut budget = Budget::new(&mut work, storage_limit);
                budget.reserve_storage(31).unwrap();
                let result =
                    check_canonical_tile_convergence_v160(inventory, Function(0), &mut budget);
                if work_limit == used && storage_limit == peak {
                    result.unwrap();
                } else {
                    assert!(matches!(result, Err(Error::Resource(_))), "{result:?}");
                }
                assert_eq!(budget.storage(), 31);
            }
        });
    }
}

fn reconverging_diamond(selected_base: bool, early_exit: bool) -> Module {
    let mut entry = BasicBlock::new(BlockId(91));
    entry.operations = vec![
        index(3, IndexKind::Local),
        value(
            4,
            Type::BOOL,
            Kind::Compare {
                predicate: ComparePredicate::Equal,
                lhs: ValueId(3),
                rhs: ValueId(1),
            },
        ),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(4),
        then_target: BlockId(22),
        then_arguments: vec![],
        else_target: BlockId(23),
        else_arguments: vec![],
    });
    let mut left = BasicBlock::new(BlockId(22));
    left.terminator = Some(Terminator::Branch {
        target: BlockId(17),
        arguments: if selected_base {
            vec![ValueId(1)]
        } else {
            vec![]
        },
    });
    let mut right = BasicBlock::new(BlockId(23));
    right
        .operations
        .push(value(5, Type::INDEX, Kind::Constant(Constant::Index(0))));
    right.terminator = Some(if early_exit {
        Terminator::Return { values: vec![] }
    } else {
        Terminator::Branch {
            target: BlockId(17),
            arguments: if selected_base {
                vec![ValueId(5)]
            } else {
                vec![]
            },
        }
    });
    let mut join = tile_block(17, if selected_base { 20 } else { 1 });
    if selected_base {
        join.parameters
            .push(ValueDef::new(ValueId(20), Type::INDEX));
    }
    // A postdominator can precede its predecessors in physical block order.
    module(vec![entry, join, right, left])
}

#[test]
fn tile_convergence_reconverges_acyclic_arrival_without_clearing_selected_values() {
    check(&reconverging_diamond(false, false)).unwrap();
    assert!(matches!(
        check(&reconverging_diamond(true, false)),
        Err(Error::VaryingTileInput(_))
    ));
    assert!(matches!(
        check(&reconverging_diamond(false, true)),
        Err(Error::VaryingArrival(_))
    ));
}

#[test]
fn tile_convergence_nested_diamonds_retain_outer_control_until_its_postdominator() {
    let mut input = reconverging_diamond(false, false);
    let blocks = &mut input.functions[0].body.as_mut().unwrap().blocks;
    let mut nested = BasicBlock::new(BlockId(30));
    nested.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(2),
        then_target: BlockId(31),
        then_arguments: vec![],
        else_target: BlockId(32),
        else_arguments: vec![],
    });
    let mut a = BasicBlock::new(BlockId(31));
    a.terminator = Some(Terminator::Branch {
        target: BlockId(17),
        arguments: vec![],
    });
    let mut b = BasicBlock::new(BlockId(32));
    b.terminator = Some(Terminator::Branch {
        target: BlockId(17),
        arguments: vec![],
    });
    blocks[3].terminator = Some(Terminator::Branch {
        target: BlockId(30),
        arguments: vec![],
    });
    blocks.extend([nested, a, b]);
    check(&input).unwrap();
    // Move the collective into only one nested arm: its uniform selector is
    // insufficient because the enclosing branch is lane-varying.
    let blocks = &mut input.functions[0].body.as_mut().unwrap().blocks;
    blocks[5].operations = std::mem::take(&mut blocks[1].operations);
    // The arm issues its own context; terminate it without merging that
    // ownership history with the paths that never issued a context.
    blocks[5].terminator = Some(Terminator::Return { values: vec![] });
    assert!(matches!(check(&input), Err(Error::VaryingArrival(_))));
}
