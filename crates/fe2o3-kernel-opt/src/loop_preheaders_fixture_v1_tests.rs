use super::*;

pub(super) const WORK: usize = 500_000_000;
pub(super) const STORAGE: usize = 128 << 20;
pub(super) const SIBLING: usize = 43;
#[derive(Clone, Copy, Debug)]
pub(super) enum Incoming {
    Branch,
    Conditional,
    Switch,
    IntegerSwitch,
    Distinct,
}
pub(super) fn branch(target: u32, arguments: &[u32]) -> Terminator {
    Terminator::Branch {
        target: BlockId(target),
        arguments: arguments.iter().copied().map(ValueId).collect(),
    }
}
pub(super) fn conditional(condition: u32, yes: u32, no: u32) -> Terminator {
    Terminator::ConditionalBranch {
        condition: ValueId(condition),
        then_target: BlockId(yes),
        then_arguments: vec![],
        else_target: BlockId(no),
        else_arguments: vec![],
    }
}
pub(super) fn block(id: u32, terminator: Terminator) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(id));
    block.terminator = Some(terminator);
    block
}
pub(super) fn fixture(incoming: Incoming) -> Module {
    let ty = Type::Scalar(ScalarType::U32);
    let mut entry = block(
        10,
        match incoming {
            Incoming::Branch => branch(50, &[1, 2]),
            Incoming::Conditional => Terminator::ConditionalBranch {
                condition: ValueId(0),
                then_target: BlockId(50),
                then_arguments: vec![ValueId(1), ValueId(2)],
                else_target: BlockId(50),
                else_arguments: vec![ValueId(2), ValueId(1)],
            },
            Incoming::Switch => Terminator::Switch {
                selector: ValueId(1),
                cases: vec![
                    SwitchCase {
                        value: 0,
                        target: BlockId(50),
                        arguments: vec![ValueId(1), ValueId(2)],
                    },
                    SwitchCase {
                        value: 1,
                        target: BlockId(50),
                        arguments: vec![ValueId(2), ValueId(1)],
                    },
                ],
                default_target: BlockId(50),
                default_arguments: vec![ValueId(1), ValueId(2)],
            },
            Incoming::IntegerSwitch => Terminator::IntegerSwitch {
                selector: ValueId(1),
                cases: vec![
                    IntegerSwitchCase {
                        value: Constant::U32(0),
                        target: BlockId(50),
                        arguments: vec![ValueId(1), ValueId(2)],
                    },
                    IntegerSwitchCase {
                        value: Constant::U32(1),
                        target: BlockId(50),
                        arguments: vec![ValueId(2), ValueId(1)],
                    },
                ],
                default_target: BlockId(50),
                default_arguments: vec![ValueId(1), ValueId(2)],
            },
            Incoming::Distinct => conditional(0, 110, 120),
        },
    );
    entry.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(100), ty.clone()),
        Kind::Constant(Constant::U32(1)),
    ));
    let mut header = block(50, conditional(202, 80, 90));
    header.parameters = vec![
        ValueDef::new(ValueId(200), ty.clone()),
        ValueDef::new(ValueId(201), ty.clone()),
    ];
    header.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(202), Type::BOOL),
        Kind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs: ValueId(200),
            rhs: ValueId(3),
        },
    ));
    let mut latch = block(80, branch(50, &[203, 201]));
    latch.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(203), ty.clone()),
        Kind::Binary {
            op: BinaryOp::Add,
            lhs: ValueId(200),
            rhs: ValueId(100),
        },
    ));
    let mut exit = block(90, Terminator::Return { values: vec![] });
    exit.operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(204), ty.clone()),
            Kind::Binary {
                op: BinaryOp::BitXor,
                lhs: ValueId(200),
                rhs: ValueId(201),
            },
        ),
        Operation::new(
            vec![],
            Kind::Store {
                pointer: ValueId(4),
                value: ValueId(204),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    let mut blocks = vec![entry, header, latch, exit];
    if matches!(incoming, Incoming::Distinct) {
        blocks.extend([
            block(110, branch(50, &[1, 2])),
            block(120, branch(50, &[2, 1])),
        ]);
    }
    let mut module = Module::new("neutral-loop-preheader-fixture");
    module.functions.push(Function::kernel_entry(
        "loop_impl",
        Signature::new(
            vec![
                Type::BOOL,
                ty.clone(),
                ty.clone(),
                ty.clone(),
                Type::pointer(ty, AddressSpace::Global, AccessMode::ReadWrite),
            ],
            vec![],
        ),
        (0..5).map(ValueId).collect(),
        blocks,
    ));
    module.kernels.push(Kernel::new(
        "loop",
        "loop_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}
pub(super) fn empty_loop() -> Module {
    let mut module = Module::new("empty-loop-preheader-fixture");
    module.functions.push(Function::internal_helper(
        "empty",
        Signature::new(vec![Type::BOOL], vec![]),
        vec![ValueId(u32::MAX)],
        vec![
            block(4, conditional(u32::MAX, 9, 9)),
            block(9, conditional(u32::MAX, 12, 30)),
            block(12, branch(9, &[])),
            block(30, Terminator::Return { values: vec![] }),
        ],
    ));
    module
}
pub(super) fn nested() -> Module {
    let mut module = Module::new("nested-neutral-loops");
    module.functions.push(Function::internal_helper(
        "nested",
        Signature::new(vec![Type::BOOL], vec![]),
        vec![ValueId(0)],
        vec![
            block(10, conditional(0, 20, 20)),
            block(20, conditional(0, 30, 30)),
            block(30, conditional(0, 40, 50)),
            block(40, branch(30, &[])),
            block(50, conditional(0, 20, 60)),
            block(60, Terminator::Return { values: vec![] }),
        ],
    ));
    module
}
pub(super) fn coordinate(function: u32, block: u32) -> Block {
    Block {
        function: FunctionCoordinate(function),
        block,
    }
}
pub(super) fn blocks(module: &mut Module) -> &mut Vec<BasicBlock> {
    &mut module.functions[0].body.as_mut().unwrap().blocks
}
pub(super) fn admit(module: &Module) -> (Owner, OutputStorage) {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    Owner::from_module_ref_with_verification_budget_v12(module, &mut budget).unwrap()
}
pub(super) fn with_input(module: Module, run: impl FnOnce(&Owner, &mut Budget<'_>)) {
    let (input, receipt) = admit(&module);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget
        .reserve_storage(SIBLING + receipt.retained_storage())
        .unwrap();
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    run(&input, &mut budget);
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(input.module(), &module);
    drop(input);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), SIBLING);
}
pub(super) fn replay(
    owner: &OwnedLoopPreheadersContinuationV1,
    input: &Owner,
    budget: &mut Budget<'_>,
) {
    let floor = budget.storage();
    let retained = {
        let (pair, ps) = owner.replay_against(input, budget).unwrap();
        assert_eq!(budget.storage(), floor);
        budget.reserve_storage(ps.retained_storage()).unwrap();
        assert!(std::ptr::eq(pair.input(), input));
        assert!(std::ptr::eq(pair.output(), owner.output()));
        assert_eq!(pair.preheaders(), owner.preheaders());
        assert!(!pair.grants_authority());
        ps.retained_storage()
    };
    budget.release_storage(retained).unwrap();
}
pub(super) fn release(owner: OwnedLoopPreheadersContinuationV1, budget: &mut Budget<'_>) {
    let bytes = owner.retained_storage();
    drop(owner);
    budget.release_storage(bytes).unwrap();
}
