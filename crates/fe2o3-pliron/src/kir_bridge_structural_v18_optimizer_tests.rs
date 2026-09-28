use super::*;
use fe2o3_kernel_ir as kir;
use kir::{
    BasicBlock as Block, BlockId, Function, Operation as Instruction, OperationKind as Kind,
    Signature, Terminator, ValueDef, ValueId,
};

const U32: Type = Type::Scalar(ScalarType::U32);

fn binary(id: u32, a: u32, b: u32) -> Instruction {
    Instruction::effect_free(
        ValueDef::new(ValueId(id), U32),
        Kind::Binary {
            op: kir::BinaryOp::Add,
            lhs: ValueId(a),
            rhs: ValueId(b),
        },
    )
}

fn fanout(count: u32) -> Module {
    let mut block = Block::new(BlockId(0));
    for (id, n) in [(1, 7), (2, 9)] {
        block.operations.push(Instruction::effect_free(
            ValueDef::new(ValueId(id), U32),
            Kind::Constant(kir::Constant::U32(n)),
        ));
    }
    block.operations.push(binary(3, 1, 2));
    for index in 0..count {
        block.operations.push(binary(4 + index, 0, 3));
    }
    let mut sum = 4;
    for index in 1..count {
        let next = 4 + count + index;
        block.operations.push(binary(next, sum, 4 + index));
        sum = next;
    }
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(sum)],
    });
    let mut module = Module::new("fanout");
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(vec![U32], vec![U32]),
        vec![ValueId(0)],
        vec![block],
    ));
    module
}

fn wide_phi(width: u32) -> Module {
    let mut entry = Block::new(BlockId(0));
    let then_arguments = vec![ValueId(1); width as usize];
    let mut else_arguments = then_arguments.clone();
    else_arguments[0] = ValueId(2);
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(0),
        then_target: BlockId(1),
        then_arguments,
        else_target: BlockId(1),
        else_arguments,
    });
    let mut join = Block::new(BlockId(1));
    join.parameters = (0..width)
        .map(|index| ValueDef::new(ValueId(3 + index), U32))
        .collect();
    join.terminator = Some(Terminator::Return {
        values: vec![ValueId(3)],
    });
    let mut module = Module::new("wide-phi");
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(vec![Type::BOOL, U32, U32], vec![U32]),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![entry, join],
    ));
    module
}

fn successor_fanin(count: u32) -> Module {
    let mut blocks = Vec::new();
    for index in 0..count {
        let mut block = Block::new(BlockId(index));
        block.terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(0),
            then_target: BlockId(count),
            then_arguments: vec![ValueId(1)],
            else_target: BlockId(index + 1),
            else_arguments: if index + 1 == count {
                vec![ValueId(2)]
            } else {
                vec![]
            },
        });
        blocks.push(block);
    }
    let mut join = Block::new(BlockId(count));
    join.parameters = vec![ValueDef::new(ValueId(3), U32)];
    join.terminator = Some(Terminator::Return {
        values: vec![ValueId(3)],
    });
    blocks.push(join);
    let mut module = Module::new("successor-fanin");
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(vec![Type::BOOL, U32, U32], vec![U32]),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        blocks,
    ));
    module
}

fn selected_switch(integer: bool, selector: Option<u32>, default_only: bool) -> Module {
    let mut entry = Block::new(BlockId(0));
    let selector = if let Some(value) = selector {
        entry.operations.push(Instruction::effect_free(
            ValueDef::new(ValueId(3), U32),
            Kind::Constant(kir::Constant::U32(value)),
        ));
        ValueId(3)
    } else {
        ValueId(0)
    };
    entry.terminator = Some(if integer {
        Terminator::IntegerSwitch {
            selector,
            cases: if default_only {
                vec![]
            } else {
                vec![kir::IntegerSwitchCase {
                    value: kir::Constant::U32(7),
                    target: BlockId(1),
                    arguments: vec![ValueId(1)],
                }]
            },
            default_target: BlockId(1),
            default_arguments: vec![ValueId(2)],
        }
    } else {
        Terminator::Switch {
            selector,
            cases: if default_only {
                vec![]
            } else {
                vec![kir::SwitchCase {
                    value: 7,
                    target: BlockId(1),
                    arguments: vec![ValueId(1)],
                }]
            },
            default_target: BlockId(1),
            default_arguments: vec![ValueId(2)],
        }
    });
    let mut join = Block::new(BlockId(1));
    join.parameters = vec![ValueDef::new(ValueId(4), U32)];
    join.terminator = Some(Terminator::Return {
        values: vec![ValueId(4)],
    });
    let mut module = Module::new("selected-switch");
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(vec![U32; 3], vec![U32]),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![entry, join],
    ));
    module
}

fn checked_work(module: &Module, selected_switch: bool) -> usize {
    use crate::neutral_optimization_v1::storage_v18::tests::{LIMITS, input};
    let input = input(module);
    let mut work = kir::CanonicalKernelIrWorkBudgetV1::new(500_000_000);
    work.charge_work(11).unwrap();
    let mut budget = Budget::new(&mut work, 20_000_000);
    budget.reserve_storage(17 + input.storage).unwrap();
    let floor = budget.storage();
    let observed =
        crate::optimize_neutral_kernel_ir_v18(&input.owner, LIMITS, &mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
    assert_eq!(observed.report().passes().len(), 8);
    budget
        .reserve_storage(observed.storage().retained_storage())
        .unwrap();
    if selected_switch {
        let blocks = &observed.owner().module().functions[0]
            .body
            .as_ref()
            .unwrap()
            .blocks;
        assert_eq!(
            blocks.len(),
            1,
            "selected switch and successor were actually folded"
        );
        assert!(matches!(
            blocks[0].terminator,
            Some(Terminator::Return { .. })
        ));
    }
    let mut called = false;
    let (checked, (), receipt) = observed
        .try_check_and_finish_with_v18(&mut budget, |relation, _| {
            called = true;
            assert!(relation.input().belongs_to(&input.owner));
            assert!(!relation.grants_authority());
            Ok::<_, std::convert::Infallible>(((), 0))
        })
        .unwrap();
    assert!(called);
    assert_eq!(budget.storage(), floor);
    assert_eq!(
        receipt.retained_storage(),
        std::mem::size_of::<crate::KirNeutralOwnedOriginStorageV1>()
    );
    drop(checked);
    assert!(budget.failed_storage().is_none());
    budget.work()
}

#[test]
fn actual_fanout_phi_and_successor_growth_use_the_same_fixed_pipeline_and_limits() {
    for size in [1, 8, 32] {
        checked_work(&fanout(size), false);
        checked_work(&wide_phi(size), false);
        checked_work(&successor_fanin(size), false);
    }
}

#[test]
fn actual_metadata_padding_has_affine_work_without_changing_graph_shape() {
    let mut costs = Vec::new();
    for extra in [0, 1024, 2048] {
        let mut source = fanout(8);
        source.id = format!("fanout{}", "x".repeat(extra)).into();
        costs.push(checked_work(&source, false));
    }
    assert!(costs[1] > costs[0]);
    assert_eq!(costs[2] - costs[1], costs[1] - costs[0]);
}

#[test]
fn actual_switch_and_integer_switch_synthesis_cover_case_default_and_default_only() {
    for integer in [false, true] {
        for (selector, default_only) in [(Some(7), false), (Some(9), false), (None, true)] {
            checked_work(&selected_switch(integer, selector, default_only), true);
        }
    }
}
