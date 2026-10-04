//! Model-only controls, including the prior BlockId/roster and switch pitfalls.
use super::*;

fn one(id: u32, kind: OperationKind) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), Type::Scalar(ScalarType::U32)),
        kind,
    )
}
fn switch(cases: usize, target: u32, default: u32) -> Terminator {
    Terminator::Switch {
        selector: ValueId(2),
        cases: (0..cases)
            .map(|value| SwitchCase {
                value: value as u64,
                target: BlockId(target),
                arguments: vec![],
            })
            .collect(),
        default_target: BlockId(default),
        default_arguments: vec![],
    }
}
fn graph() -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let mut call = BasicBlock::new(BlockId(17));
    call.operations = vec![one(
        3,
        OperationKind::Call {
            callee: "helper".into(),
            arguments: vec![ValueId(0), ValueId(1)],
        },
    )];
    call.terminator = Some(switch(2, 42, 17));
    let mut end = BasicBlock::new(BlockId(42));
    end.terminator = Some(Terminator::Return { values: vec![] });
    let entry = Function::kernel_entry(
        "entry",
        Signature::new(
            vec![
                scalar.clone(),
                scalar.clone(),
                Type::Scalar(ScalarType::U64),
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![call, end],
    );
    let mut helper = BasicBlock::new(BlockId(99));
    helper.operations = vec![
        one(2, OperationKind::Constant(Constant::U32(0xffff))),
        one(
            3,
            OperationKind::Binary {
                op: BinaryOp::BitXor,
                lhs: ValueId(0),
                rhs: ValueId(1),
            },
        ),
        one(
            4,
            OperationKind::Binary {
                op: BinaryOp::BitAnd,
                lhs: ValueId(3),
                rhs: ValueId(2),
            },
        ),
    ];
    helper.terminator = Some(Terminator::Return {
        values: vec![ValueId(4)],
    });
    let helper = Function::internal_helper(
        "helper",
        Signature::new(vec![scalar.clone(), scalar.clone()], vec![scalar]),
        vec![ValueId(0), ValueId(1)],
        vec![helper],
    );
    let mut module = Module::new("synthetic-source-cursor-topology");
    module.functions = vec![entry, helper];
    module.kernels.push(Kernel::new(
        "loop_helper",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}

#[test]
fn positive_owner_has_nonpositional_ids_and_real_case_or_default_cycle() {
    for (target, default) in [(42, 17), (17, 42)] {
        let mut module = graph();
        module.functions[0].body.as_mut().unwrap().blocks[0].terminator =
            Some(switch(2, target, default));
        VerifiedCanonicalKernelIrV11::from_module(module.clone()).unwrap();
        let selected = select(&module).unwrap();
        assert_eq!(selected.call.block, BlockId(17));
        assert_eq!(selected.helper.block, BlockId(99));
        assert_eq!(selected.call.function_ordinal, 0);
        assert_eq!(selected.helper.function_ordinal, 1);
    }
}

#[test]
fn missing_targets_acyclic_and_unreachable_calls_refuse() {
    for (target, default) in [(100, 17), (17, 100), (42, 42)] {
        let mut module = graph();
        module.functions[0].body.as_mut().unwrap().blocks[0].terminator =
            Some(switch(2, target, default));
        assert!(select(&module).is_err());
    }
    let mut module = graph();
    let mut entry = BasicBlock::new(BlockId(88));
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(42),
        arguments: vec![],
    });
    module.functions[0]
        .body
        .as_mut()
        .unwrap()
        .blocks
        .insert(0, entry);
    assert!(select(&module).is_err());
    let mut missing = graph();
    missing.functions[0].body.as_mut().unwrap().blocks[0].terminator = None;
    assert!(select(&missing).is_err());
}

#[test]
fn bounded_cases_edges_and_pure_helper_profile_are_not_relaxed() {
    let mut module = graph();
    module.functions[0].body.as_mut().unwrap().blocks[0].terminator = Some(switch(64, 17, 42));
    assert!(select(&module).is_ok());
    module.functions[0].body.as_mut().unwrap().blocks[0].terminator = Some(switch(65, 17, 42));
    assert_eq!(select(&module).unwrap_err(), "case cap");
    module.functions[0].body.as_mut().unwrap().blocks[0].terminator = Some(switch(63, 17, 42));
    for id in [90, 91, 92] {
        let mut block = BasicBlock::new(BlockId(id));
        block.terminator = Some(switch(63, 17, 42));
        module.functions[0]
            .body
            .as_mut()
            .unwrap()
            .blocks
            .push(block);
    }
    assert!(select(&module).is_ok()); // exactly four times (63 cases + default)
    module.functions[0].body.as_mut().unwrap().blocks[0].terminator = Some(switch(64, 17, 42));
    assert_eq!(select(&module).unwrap_err(), "edge cap");
    let mut impure = graph();
    impure.functions[1].body.as_mut().unwrap().blocks[0].operations[0].kind =
        OperationKind::Constant(Constant::U32(3));
    assert!(select(&impure).is_err());
}
