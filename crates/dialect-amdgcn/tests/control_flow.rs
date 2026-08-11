use dialect_amdgcn::lower_device_module_to_gfx942_llvm_ir;
use fe2o3_kernel_ir::{
    BasicBlock, BinaryOp, BlockId, ComparePredicate, Constant, Function, IntegerSwitchCase, Module,
    Operation, OperationKind, ScalarType, Signature, TargetCapability, Terminator, Type, ValueDef,
    ValueId, WaveWidth,
};

// These identify the source of manually transcribed modules below; they do not
// establish a structured MIR-to-kernel-IR correspondence.
const BRANCHING_FILL_MIR: &str =
    include_str!("../../dialect-mir/tests/fixtures/branching-fill.mir.json");
const INTEGER_MATCH_MIR: &str =
    include_str!("../../dialect-mir/tests/fixtures/integer-match.mir.json");
const NESTED_LOOP_MIR: &str = include_str!("../../dialect-mir/tests/fixtures/nested-loop.mir.json");

const U32: Type = Type::Scalar(ScalarType::U32);

fn value(id: u32) -> ValueId {
    ValueId(id)
}

fn constant(id: u32, constant: Constant) -> Operation {
    Operation::effect_free(
        ValueDef::new(value(id), constant.ty()),
        OperationKind::Constant(constant),
    )
}

fn binary(id: u32, op: BinaryOp, lhs: u32, rhs: u32) -> Operation {
    Operation::effect_free(
        ValueDef::new(value(id), U32),
        OperationKind::Binary {
            op,
            lhs: value(lhs),
            rhs: value(rhs),
        },
    )
}

fn compare(id: u32, predicate: ComparePredicate, lhs: u32, rhs: u32) -> Operation {
    Operation::effect_free(
        ValueDef::new(value(id), Type::BOOL),
        OperationKind::Compare {
            predicate,
            lhs: value(lhs),
            rhs: value(rhs),
        },
    )
}

fn block(
    id: u32,
    parameters: &[u32],
    operations: Vec<Operation>,
    terminator: Terminator,
) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(id));
    block.parameters = parameters
        .iter()
        .map(|id| ValueDef::new(value(*id), U32))
        .collect();
    block.operations = operations;
    block.terminator = Some(terminator);
    block
}

fn helper_module(name: &str, blocks: Vec<BasicBlock>) -> Module {
    let mut function = Function::device_ffi_export(
        name,
        Signature::new(vec![U32], vec![U32]),
        vec![value(0)],
        blocks,
    );
    function
        .required_capabilities
        .insert(TargetCapability::WaveWidth(WaveWidth::Wave64));
    let mut module = Module::new(format!("tests::{name}"));
    module.functions.push(function);
    module
}

fn branching_fill_module() -> Module {
    helper_module(
        "branching_fill",
        vec![
            block(
                0,
                &[],
                vec![
                    constant(1, Constant::U32(10)),
                    compare(2, ComparePredicate::LessThan, 0, 1),
                ],
                Terminator::ConditionalBranch {
                    condition: value(2),
                    then_target: BlockId(1),
                    then_arguments: vec![],
                    else_target: BlockId(2),
                    else_arguments: vec![],
                },
            ),
            block(
                1,
                &[],
                vec![constant(3, Constant::U32(7))],
                Terminator::Branch {
                    target: BlockId(3),
                    arguments: vec![value(3)],
                },
            ),
            block(
                2,
                &[],
                vec![constant(4, Constant::U32(0))],
                Terminator::Branch {
                    target: BlockId(3),
                    arguments: vec![value(4)],
                },
            ),
            block(
                3,
                &[5],
                vec![],
                Terminator::Return {
                    values: vec![value(5)],
                },
            ),
        ],
    )
}

fn integer_match_module() -> Module {
    let case = |value, target| IntegerSwitchCase {
        value: Constant::U32(value),
        target: BlockId(target),
        arguments: vec![],
    };
    helper_module(
        "integer_match",
        vec![
            block(
                0,
                &[],
                vec![],
                Terminator::IntegerSwitch {
                    selector: value(0),
                    cases: vec![case(0, 1), case(7, 2), case(42, 3)],
                    default_target: BlockId(4),
                    default_arguments: vec![],
                },
            ),
            block(
                1,
                &[],
                vec![constant(1, Constant::U32(10))],
                Terminator::Branch {
                    target: BlockId(5),
                    arguments: vec![value(1)],
                },
            ),
            block(
                2,
                &[],
                vec![constant(2, Constant::U32(20))],
                Terminator::Branch {
                    target: BlockId(5),
                    arguments: vec![value(2)],
                },
            ),
            block(
                3,
                &[],
                vec![constant(3, Constant::U32(30))],
                Terminator::Branch {
                    target: BlockId(5),
                    arguments: vec![value(3)],
                },
            ),
            block(
                4,
                &[],
                vec![constant(4, Constant::U32(99))],
                Terminator::Branch {
                    target: BlockId(5),
                    arguments: vec![value(4)],
                },
            ),
            block(
                5,
                &[5],
                vec![],
                Terminator::Return {
                    values: vec![value(5)],
                },
            ),
        ],
    )
}

fn nested_loop_module() -> Module {
    helper_module(
        "nested_loop",
        vec![
            block(
                0,
                &[],
                vec![constant(1, Constant::U32(0)), constant(2, Constant::U32(0))],
                Terminator::Branch {
                    target: BlockId(1),
                    arguments: vec![value(1), value(2)],
                },
            ),
            block(
                1,
                &[3, 4],
                vec![compare(5, ComparePredicate::LessThan, 3, 0)],
                Terminator::ConditionalBranch {
                    condition: value(5),
                    then_target: BlockId(2),
                    then_arguments: vec![],
                    else_target: BlockId(8),
                    else_arguments: vec![value(4)],
                },
            ),
            block(
                2,
                &[],
                vec![constant(6, Constant::U32(0))],
                Terminator::Branch {
                    target: BlockId(3),
                    arguments: vec![value(6), value(4)],
                },
            ),
            block(
                3,
                &[7, 8],
                vec![compare(9, ComparePredicate::LessThan, 7, 0)],
                Terminator::ConditionalBranch {
                    condition: value(9),
                    then_target: BlockId(4),
                    then_arguments: vec![],
                    else_target: BlockId(7),
                    else_arguments: vec![],
                },
            ),
            block(
                4,
                &[],
                vec![
                    constant(10, Constant::U32(2)),
                    compare(11, ComparePredicate::Equal, 7, 10),
                ],
                Terminator::ConditionalBranch {
                    condition: value(11),
                    then_target: BlockId(6),
                    then_arguments: vec![value(8)],
                    else_target: BlockId(5),
                    else_arguments: vec![],
                },
            ),
            block(
                5,
                &[],
                vec![binary(12, BinaryOp::Add, 8, 7)],
                Terminator::Branch {
                    target: BlockId(6),
                    arguments: vec![value(12)],
                },
            ),
            block(
                6,
                &[13],
                vec![
                    constant(14, Constant::U32(1)),
                    binary(15, BinaryOp::Add, 7, 14),
                ],
                Terminator::Branch {
                    target: BlockId(3),
                    arguments: vec![value(15), value(13)],
                },
            ),
            block(
                7,
                &[],
                vec![
                    constant(16, Constant::U32(1)),
                    binary(17, BinaryOp::Add, 3, 16),
                ],
                Terminator::Branch {
                    target: BlockId(1),
                    arguments: vec![value(17), value(8)],
                },
            ),
            block(
                8,
                &[18],
                vec![],
                Terminator::Return {
                    values: vec![value(18)],
                },
            ),
        ],
    )
}

#[test]
fn manually_transcribed_mir_sources_remain_identifiable() {
    assert!(BRANCHING_FILL_MIR.contains("fixture::branching_fill"));
    assert!(BRANCHING_FILL_MIR.contains("SwitchInt"));
    assert!(INTEGER_MATCH_MIR.contains("fixture::integer_match"));
    assert!(INTEGER_MATCH_MIR.contains("[[0,"));
    assert!(INTEGER_MATCH_MIR.contains("[42,"));
    assert!(NESTED_LOOP_MIR.contains("fixture::nested_loop"));
    assert!(NESTED_LOOP_MIR.matches("SwitchInt").count() >= 3);
}

#[test]
fn branching_fill_matches_the_gfx942_golden() {
    let llvm = lower_device_module_to_gfx942_llvm_ir(&branching_fill_module()).unwrap();
    assert_eq!(
        llvm.trim_end(),
        include_str!("fixtures/branching_fill_gfx942.ll").trim_end()
    );
    assert!(llvm.contains("%v5 = phi i32 [ 7, %bb1 ], [ 0, %bb2 ]"));
}

#[test]
fn integer_match_matches_the_gfx942_golden() {
    let llvm = lower_device_module_to_gfx942_llvm_ir(&integer_match_module()).unwrap();
    assert_eq!(
        llvm.trim_end(),
        include_str!("fixtures/integer_match_gfx942.ll").trim_end()
    );
    assert!(llvm.contains("i32 42, label %bb3"));
}

#[test]
fn nested_loop_break_continue_and_critical_edge_match_the_gfx942_golden() {
    let llvm = lower_device_module_to_gfx942_llvm_ir(&nested_loop_module()).unwrap();
    assert_eq!(
        llvm.trim_end(),
        include_str!("fixtures/nested_loop_gfx942.ll").trim_end()
    );
    assert!(llvm.contains("edge_bb4_0_bb6"));
    assert!(llvm.contains("%v3 = phi i32 [ 0, %bb0 ], [ %v17, %bb7 ]"));
    assert!(llvm.contains("%v7 = phi i32 [ 0, %bb2 ], [ %v15, %bb6 ]"));
}

#[test]
fn all_control_flow_goldens_are_deterministic() {
    for module in [
        branching_fill_module(),
        integer_match_module(),
        nested_loop_module(),
    ] {
        let first = lower_device_module_to_gfx942_llvm_ir(&module).unwrap();
        let second = lower_device_module_to_gfx942_llvm_ir(&module).unwrap();
        assert_eq!(first, second);
    }
}
