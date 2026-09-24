use super::*;
pub(super) use fe2o3_kernel_ir::{
    BinaryOp, CanonicalKernelIrWorkBudgetV1 as Work, CheckedBinaryOperator, ComparePredicate,
    Constant, Function, Signature,
};
pub(super) const W: usize = 1_000_000_000;
pub(super) const S: usize = 1024 * 1024 * 1024;
pub(super) const FLOOR: usize = 43;
pub(super) fn scalar(s: ScalarType) -> Type {
    Type::Scalar(s)
}
pub(super) fn constant(s: ScalarType, n: u64) -> Constant {
    match s {
        ScalarType::U8 => Constant::U8(n.try_into().unwrap()),
        ScalarType::U16 => Constant::U16(n.try_into().unwrap()),
        ScalarType::U32 => Constant::U32(n.try_into().unwrap()),
        ScalarType::U64 => Constant::U64(n),
        ScalarType::I32 => Constant::I32(n.try_into().unwrap()),
        ScalarType::Index => Constant::Index(n),
        _ => panic!("fixture scalar"),
    }
}
pub(super) fn literal(id: u32, s: ScalarType, n: u64) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), scalar(s)),
        OperationKind::Constant(constant(s, n)),
    )
}
pub(super) fn branch(b: u32, args: &[u32]) -> Terminator {
    Terminator::Branch {
        target: BlockId(b),
        arguments: args.iter().map(|v| ValueId(*v)).collect(),
    }
}
pub(super) fn block(
    id: u32,
    parameters: Vec<ValueDef>,
    operations: Vec<Operation>,
    term: Terminator,
) -> BasicBlock {
    BasicBlock {
        id: BlockId(id),
        parameters,
        operations,
        terminator: Some(term),
    }
}
pub(super) fn fixture(
    s: ScalarType,
    literals: Option<(u64, u64)>,
    step: u64,
    checked: bool,
) -> Module {
    let mut entry = vec![literal(10, s, step)];
    let (initial, bound) = if let Some((a, b)) = literals {
        entry.extend([literal(11, s, a), literal(12, s, b)]);
        (11, 12)
    } else {
        (0, 1)
    };
    let update = if checked {
        Operation::checked_binary(
            ValueDef::new(ValueId(30), scalar(s)),
            ValueDef::new(ValueId(31), Type::BOOL),
            CheckedBinaryOperator::Add,
            ValueId(20),
            ValueId(10),
        )
    } else {
        Operation::effect_free(
            ValueDef::new(ValueId(30), scalar(s)),
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(20),
                rhs: ValueId(10),
            },
        )
    };
    let mut m = Module::new("bounded-loop-unroll");
    m.functions.push(Function::internal_helper(
        "unroll",
        Signature::new(vec![scalar(s), scalar(s), Type::BOOL], vec![]),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![
            block(13, vec![], entry, branch(41, &[initial])),
            block(
                41,
                vec![ValueDef::new(ValueId(20), scalar(s))],
                vec![Operation::effect_free(
                    ValueDef::new(ValueId(21), Type::BOOL),
                    OperationKind::Compare {
                        predicate: ComparePredicate::LessThan,
                        lhs: ValueId(20),
                        rhs: ValueId(bound),
                    },
                )],
                Terminator::ConditionalBranch {
                    condition: ValueId(21),
                    then_target: BlockId(97),
                    then_arguments: vec![ValueId(20)],
                    else_target: BlockId(701),
                    else_arguments: vec![ValueId(20)],
                },
            ),
            block(
                97,
                vec![ValueDef::new(ValueId(25), scalar(s))],
                vec![update],
                branch(41, &[30]),
            ),
            block(
                701,
                vec![ValueDef::new(ValueId(40), scalar(s))],
                vec![],
                Terminator::Return { values: vec![] },
            ),
        ],
    ));
    m
}
pub(super) fn diamond(s: ScalarType, n: u64, checked: bool) -> Module {
    let mut m = fixture(s, Some((0, n)), 1, checked);
    let b = &mut m.functions[0].body.as_mut().unwrap().blocks;
    let update = b[2].operations.remove(0);
    b[2].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(2),
        then_target: BlockId(100),
        then_arguments: vec![ValueId(25)],
        else_target: BlockId(101),
        else_arguments: vec![ValueId(25)],
    });
    b.extend([
        block(
            100,
            vec![ValueDef::new(ValueId(26), scalar(s))],
            vec![],
            branch(103, &[26]),
        ),
        block(
            101,
            vec![ValueDef::new(ValueId(27), scalar(s))],
            vec![],
            branch(103, &[27]),
        ),
        block(
            103,
            vec![ValueDef::new(ValueId(28), scalar(s))],
            vec![update],
            branch(41, &[30]),
        ),
    ]);
    m
}
pub(super) fn admit(m: &Module) -> (Owner, usize) {
    let mut w = Work::new(W);
    let mut b = Budget::new(&mut w, S);
    let (o, s) = Owner::from_module_ref_with_verification_budget_v12(m, &mut b).unwrap();
    (o, s.retained_storage())
}
pub(super) fn with_input(m: Module, run: impl FnOnce(&Owner, &mut Budget<'_>)) {
    let (input, bytes) = admit(&m);
    let sibling = vec![0x85u8; FLOOR];
    let floor = bytes + sibling.capacity() + size_of::<Vec<u8>>();
    let mut w = Work::new(W);
    let mut b = Budget::new(&mut w, S);
    b.reserve_storage(floor).unwrap();
    let ledger = b.work_ledger_identity_v1();
    run(&input, &mut b);
    assert_eq!(b.storage(), floor);
    assert!(b.work_ledger_identity_v1() == ledger);
    assert_eq!(input.module(), &m);
    assert_eq!(sibling, [0x85; FLOOR]);
}
pub(super) fn run(input: &Owner, b: &mut Budget<'_>) -> OwnedLoopUnrollV1 {
    let (o, s) = unroll_canonical_kir_loops_v1(input, Limits::default(), b).unwrap();
    assert_eq!(o.retained, s.retained_storage());
    b.reserve_storage(o.retained).unwrap();
    o
}
pub(super) fn release(o: OwnedLoopUnrollV1, b: &mut Budget<'_>) {
    let bytes = o.retained;
    drop(o);
    b.release_storage(bytes).unwrap();
}
pub(super) fn replay(o: &OwnedLoopUnrollV1, input: &Owner, b: &mut Budget<'_>) {
    let floor = b.storage();
    let bytes = {
        let (p, s) = o.replay(input, o.limits(), b).unwrap();
        b.reserve_storage(s.retained_storage()).unwrap();
        assert!(std::ptr::eq(p.input(), input));
        assert!(std::ptr::eq(p.output(), o.output()));
        assert!(!p.grants_authority());
        assert_eq!(p.origins().blocks, o.origins().blocks);
        assert_eq!(p.limits(), o.limits());
        s.retained_storage()
    };
    b.release_storage(bytes).unwrap();
    assert_eq!(b.storage(), floor);
}
pub(super) fn no_op(m: Module) {
    with_input(m, |input, b| {
        let o = run(input, b);
        assert_eq!(o.origins().selection, None);
        assert_eq!(
            o.output().canonical().canonical_bytes(),
            input.canonical().canonical_bytes()
        );
        replay(&o, input, b);
        release(o, b);
    });
}
