use super::*;
use fe2o3_kernel_ir::{AccessMode, AddressSpace, BinaryOp, CheckedBinaryOperator, MemoryAccess};

fn bits(ty: ScalarType, value: u64) -> Constant {
    match ty {
        ScalarType::I8 => Constant::I8(value as i8),
        ScalarType::I16 => Constant::I16(value as i16),
        ScalarType::I32 => Constant::I32(value as i32),
        ScalarType::I64 => Constant::I64(value as i64),
        ScalarType::U8 => Constant::U8(value as u8),
        ScalarType::U16 => Constant::U16(value as u16),
        ScalarType::U32 => Constant::U32(value as u32),
        ScalarType::U64 => Constant::U64(value),
        _ => panic!("fixed integer fixture"),
    }
}

fn store(value: u32) -> Operation {
    Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(99),
            value: ValueId(value),
            access: MemoryAccess::new(AddressSpace::Global, 1),
        },
    )
}

struct Fixture {
    input: Module,
    output: Module,
    plan: Plan,
}

fn make_fixture(
    ty: ScalarType,
    operator: BinaryOp,
    literal: u64,
    reverse: bool,
    overflow: bool,
    target: u32,
    keep_store: bool,
) -> Fixture {
    let ty = Type::Scalar(ty);
    let parameters = vec![
        ty.clone(),
        ty.clone(),
        Type::pointer(ty.clone(), AddressSpace::Global, AccessMode::ReadWrite),
    ];
    let checked = matches!(operator, BinaryOp::Checked(_));
    let results = if checked {
        vec![ty.clone(), ty.clone(), Type::BOOL, Type::BOOL]
    } else {
        vec![ty.clone(), ty.clone()]
    };
    let literal = bits(ty.as_scalar().unwrap(), literal);
    let (lhs, rhs) = if reverse { (2, 1) } else { (1, 2) };
    let binary = if let BinaryOp::Checked(operator) = operator {
        Operation::checked_binary(
            ValueDef::new(ValueId(3), ty.clone()),
            ValueDef::new(ValueId(4), Type::BOOL),
            operator,
            ValueId(lhs),
            ValueId(rhs),
        )
    } else {
        value(
            3,
            ty.clone(),
            OperationKind::Binary {
                op: operator,
                lhs: ValueId(lhs),
                rhs: ValueId(rhs),
            },
        )
    };
    let input = module(
        parameters.clone(),
        results.clone(),
        vec![1, 9, 99],
        vec![returning(
            17,
            vec![constant(2, literal.clone()), binary, store(3)],
            if checked { &[3, 3, 4, 4] } else { &[3, 3] },
        )],
    );
    let mut operations = vec![constant(2, literal)];
    let mut origins = vec![Origin::Retained(op(0, 0))];
    let mut relations = vec![(1, 1, R), (9, 9, R), (99, 99, R), (2, 2, R), (3, target, S)];
    if checked {
        operations.push(constant(40, Constant::Bool(overflow)));
        origins.push(Origin::ConstantFrom(result(0, 1, 1)));
        relations.push((4, 40, S));
    }
    let mut uses = Vec::new();
    if keep_store {
        operations.push(store(target));
        origins.push(Origin::Retained(op(0, 2)));
        uses.extend([operand(0, 2, 0), operand(0, 2, 1)]);
    }
    let returned = if checked {
        vec![target, target, 40, 40]
    } else {
        vec![target, target]
    };
    uses.extend((0..returned.len()).map(|slot| term(0, slot as u32)));
    let output = module(
        parameters,
        results,
        vec![1, 9, 99],
        vec![returning(17, operations, &returned)],
    );
    Fixture {
        input,
        output,
        plan: Plan {
            chains: vec![vec![(0, None)]],
            operations: origins,
            relations,
            uses,
            ..Plan::default()
        },
    }
}

#[test]
fn independently_checks_fixed_integer_identity_family_and_checked_false_results() {
    for ty in [
        ScalarType::I8,
        ScalarType::I16,
        ScalarType::I32,
        ScalarType::I64,
        ScalarType::U8,
        ScalarType::U16,
        ScalarType::U32,
        ScalarType::U64,
    ] {
        for operator in [
            BinaryOp::Add,
            BinaryOp::Subtract,
            BinaryOp::Multiply,
            BinaryOp::BitAnd,
            BinaryOp::BitOr,
            BinaryOp::BitXor,
            BinaryOp::Checked(CheckedBinaryOperator::Add),
            BinaryOp::Checked(CheckedBinaryOperator::Subtract),
            BinaryOp::Checked(CheckedBinaryOperator::Multiply),
        ] {
            for reverse in [false, true] {
                if reverse
                    && matches!(
                        operator,
                        BinaryOp::Subtract | BinaryOp::Checked(CheckedBinaryOperator::Subtract)
                    )
                {
                    continue;
                }
                let neutral = match operator {
                    BinaryOp::Multiply | BinaryOp::Checked(CheckedBinaryOperator::Multiply) => 1,
                    BinaryOp::BitAnd => u64::MAX >> (64 - ty.bit_width().unwrap()),
                    _ => 0,
                };
                let fixture = make_fixture(ty, operator, neutral, reverse, false, 1, true);
                inspect(
                    fixture.input,
                    fixture.output,
                    |a, b| fixture.plan.rows(a, b),
                    |a, b, rows, floor| accepted(a, b, rows, floor),
                );
            }
        }
    }
}

#[test]
fn near_miss_constant_orientation_operator_and_wrong_operand_are_not_proofs() {
    for (operator, literal, reverse) in [
        (BinaryOp::Add, 1, false),
        (BinaryOp::Subtract, 0, true),
        (BinaryOp::Multiply, 0, false),
        (BinaryOp::BitAnd, 0xffff_fffe, false),
        (BinaryOp::BitOr, 1, false),
        (BinaryOp::BitXor, 1, false),
        (BinaryOp::Checked(CheckedBinaryOperator::Add), 1, false),
        (BinaryOp::Checked(CheckedBinaryOperator::Subtract), 0, true),
        (BinaryOp::Checked(CheckedBinaryOperator::Multiply), 0, false),
        (BinaryOp::ShiftLeft, 0, false),
        (BinaryOp::ShiftRight, 0, false),
        (BinaryOp::Divide, 1, false),
        (BinaryOp::Divide, 0, false),
        (BinaryOp::Remainder, 1, false),
    ] {
        let fixture = make_fixture(ScalarType::U32, operator, literal, reverse, false, 1, true);
        inspect(
            fixture.input,
            fixture.output,
            |a, b| fixture.plan.rows(a, b),
            |a, b, rows, floor| {
                assert_eq!(
                    rejected(a, b, rows, floor),
                    Error::Rule("unproved typed substitution")
                );
            },
        );
    }
    let fixture = make_fixture(ScalarType::U32, BinaryOp::Add, 0, false, false, 9, true);
    inspect(
        fixture.input,
        fixture.output,
        |a, b| fixture.plan.rows(a, b),
        |a, b, rows, floor| {
            assert_eq!(
                rejected(a, b, rows, floor),
                Error::Rule("unproved typed substitution")
            );
        },
    );
}

#[test]
fn checked_overflow_and_slot_corruption_are_rejected_independently() {
    let fixture = make_fixture(
        ScalarType::I32,
        BinaryOp::Checked(CheckedBinaryOperator::Multiply),
        1,
        false,
        true,
        1,
        true,
    );
    inspect(
        fixture.input,
        fixture.output,
        |a, b| fixture.plan.rows(a, b),
        |a, b, rows, floor| {
            assert_eq!(
                rejected(a, b, rows, floor),
                Error::Rule("constant synthesis bits")
            );
        },
    );
    let fixture = make_fixture(
        ScalarType::U32,
        BinaryOp::Checked(CheckedBinaryOperator::Add),
        0,
        false,
        false,
        1,
        true,
    );
    inspect(
        fixture.input,
        fixture.output,
        |a, b| fixture.plan.rows(a, b),
        |a, b, rows, floor| {
            accepted(a, b, rows, floor);
            rows.operations[1].origin = Origin::ConstantFrom(result(0, 1, 0));
            assert_eq!(
                rejected(a, b, rows, floor),
                Error::Rule("synthesized constant type")
            );
        },
    );
}

#[test]
fn identity_totality_does_not_waive_store_use_type_or_descendant_obligations() {
    let fixture = make_fixture(ScalarType::U32, BinaryOp::Add, 0, false, false, 1, false);
    inspect(
        fixture.input,
        fixture.output,
        |a, b| fixture.plan.rows(a, b),
        |a, b, rows, floor| {
            assert_eq!(
                rejected(a, b, rows, floor),
                Error::Rule("executable ordered operation removed")
            );
        },
    );
    let mut fixture = make_fixture(ScalarType::U32, BinaryOp::Add, 0, false, false, 1, true);
    fixture.plan.relations.retain(|(source, _, _)| *source != 3);
    inspect(
        fixture.input,
        fixture.output,
        |a, b| fixture.plan.rows(a, b),
        |a, b, rows, floor| {
            assert_eq!(
                rejected(a, b, rows, floor),
                Error::Rule("final operand has no exact descendant")
            );
        },
    );
    let fixture = make_fixture(ScalarType::U32, BinaryOp::Add, 0, false, false, 1, true);
    inspect(
        fixture.input,
        fixture.output,
        |a, b| fixture.plan.rows(a, b),
        |a, b, rows, floor| {
            accepted(a, b, rows, floor);
            let row = a
                .definitions()
                .iter()
                .position(|row| row.value == Some(ValueId(3)))
                .unwrap();
            let descendant = rows.definitions[row].outputs.start as usize;
            rows.definition_outputs[descendant].output = b
                .definitions()
                .iter()
                .find(|row| row.value == Some(ValueId(99)))
                .unwrap()
                .coordinate;
            assert_eq!(
                rejected(a, b, rows, floor),
                Error::Rule("descendant owner or type")
            );
        },
    );
    let fixture = make_fixture(ScalarType::U32, BinaryOp::Add, 0, false, false, 1, true);
    inspect(
        fixture.input,
        fixture.output,
        |a, b| fixture.plan.rows(a, b),
        |a, b, rows, floor| {
            rows.uses[1].input = operand(0, 1, 0);
            assert_eq!(
                rejected(a, b, rows, floor),
                Error::Rule("final operation operand origin")
            );
        },
    );
}

#[test]
fn removing_an_identity_does_not_allow_reordering_two_live_stores() {
    let mut fixture = make_fixture(ScalarType::U32, BinaryOp::Add, 0, false, false, 1, true);
    fixture.input.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .push(store(9));
    fixture.output.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .insert(1, store(9));
    fixture
        .plan
        .operations
        .insert(1, Origin::Retained(op(0, 3)));
    fixture
        .plan
        .uses
        .splice(0..0, [operand(0, 3, 0), operand(0, 3, 1)]);
    inspect(
        fixture.input,
        fixture.output,
        |a, b| fixture.plan.rows(a, b),
        |a, b, rows, floor| {
            assert_eq!(
                rejected(a, b, rows, floor),
                Error::Rule("ordered operation order")
            );
        },
    );
}

#[test]
fn floating_boolean_and_index_identity_claims_stay_outside_the_closed_checker() {
    for (ty, literal, operator) in [
        (Type::BOOL, Constant::Bool(false), BinaryOp::BitOr),
        (
            Type::Scalar(ScalarType::Index),
            Constant::Index(0),
            BinaryOp::Add,
        ),
        (
            Type::Scalar(ScalarType::F32),
            Constant::F32Bits(0),
            BinaryOp::Add,
        ),
        (
            Type::Scalar(ScalarType::F32),
            Constant::F32Bits(0x8000_0000),
            BinaryOp::Add,
        ),
        (
            Type::Scalar(ScalarType::F32),
            Constant::F32Bits(0x7fc0_0000),
            BinaryOp::Add,
        ),
    ] {
        let input = module(
            vec![ty.clone()],
            vec![ty.clone()],
            vec![1],
            vec![returning(
                17,
                vec![
                    constant(2, literal.clone()),
                    value(
                        3,
                        ty.clone(),
                        OperationKind::Binary {
                            op: operator,
                            lhs: ValueId(1),
                            rhs: ValueId(2),
                        },
                    ),
                ],
                &[3],
            )],
        );
        let output = module(
            vec![ty.clone()],
            vec![ty],
            vec![1],
            vec![returning(17, vec![constant(2, literal)], &[1])],
        );
        inspect(
            input,
            output,
            |a, b| {
                Plan {
                    chains: vec![vec![(0, None)]],
                    operations: vec![Origin::Retained(op(0, 0))],
                    relations: vec![(1, 1, R), (2, 2, R), (3, 1, S)],
                    uses: vec![term(0, 0)],
                    ..Plan::default()
                }
                .rows(a, b)
            },
            |a, b, rows, floor| {
                assert_eq!(
                    rejected(a, b, rows, floor),
                    Error::Rule("unproved typed substitution")
                );
            },
        );
    }
}
