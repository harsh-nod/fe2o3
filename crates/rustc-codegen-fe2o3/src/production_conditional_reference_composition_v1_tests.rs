//! Inert operand refusals. Matching subjects in a request are not a signed proof.
use super::*;
use dialect_kernel::AccessKindAttr;
use fe2o3_pliron::{
    ProductionGpuWriteSiteV2, ProductionRankedTerminatorV1 as Term,
    ProductionRankedValueIdV1 as Id, ProductionReferenceOutputSiteV2, ProductionSemanticUnaryOpV2,
};

const U32: Ty = Ty::Integer {
    signed: false,
    bits: 32,
};
const U64: Ty = Ty::Integer {
    signed: false,
    bits: 64,
};

fn value(id: u32) -> Value {
    Value::Local(Id::new(id))
}

fn sample(gpu_bits: u32, cpu_bits: u32) -> Vec<Op> {
    let point = Expr::Symbol {
        symbol: 0,
        scalar: U64,
    };
    let yes = Expr::Constant {
        scalar: Ty::Bool,
        bits: 1,
    };
    let expressions = [
        point.clone(),
        yes.clone(),
        yes.clone(),
        Expr::Constant {
            scalar: U32,
            bits: u64::from(gpu_bits),
        },
        point,
        yes.clone(),
        yes,
        Expr::Constant {
            scalar: U32,
            bits: u64::from(cpu_bits),
        },
    ];
    let mut operations: Vec<_> = expressions
        .into_iter()
        .enumerate()
        .map(|(index, expression)| Op::SemanticExpression {
            result: Id::new(index as u32),
            expression,
            numerical_contract: Numerical::ExactBitVectorOperatorCongruence,
        })
        .collect();
    operations.push(Op::ValueAccess {
        kind: AccessKindAttr::Write,
        view: Value::Argument(0),
        indices: vec![Value::Argument(1)],
        value: value(3),
    });
    operations.push(Op::RequestEffectRefinement {
        contract: Contract::new(
            1,
            ProductionGpuWriteSiteV2::new(0, 8),
            ProductionReferenceOutputSiteV2::new(0, 0, 0),
            Value::Argument(0),
            vec![Value::Argument(1)],
            vec![value(0)],
            vec![value(4)],
            value(1),
            value(5),
            value(2),
            value(6),
            value(3),
            value(7),
        )
        .unwrap(),
        subjects: FunctionalRefinementSubjectsV2::new(
            SafeReferenceKindV2::Mir,
            DigestV1::from_untrusted_bytes([2; 32]),
            DigestV1::ZERO,
            DigestV1::from_untrusted_bytes([7; 32]),
            DigestV1::from_untrusted_bytes([2; 32]),
            DigestV1::from_untrusted_bytes([7; 32]),
        )
        .unwrap(),
    });
    operations
}

fn run(operations: Vec<Op>, bits: u32, limit: usize) -> (Result<(), Error>, usize, Option<usize>) {
    let cpu = fixture(u128::from(bits));
    let mut work = Work::new(limit);
    let mut budget = Budget::new(&mut work, 31);
    budget.reserve_storage(31).unwrap();
    budget.charge_work(17).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let (_, checked) = check_constant_point_effect(&cpu, &mut budget).unwrap();
    let blocks = [Block::new(operations, Term::Return)];
    let Some(Op::RequestEffectRefinement { contract, subjects }) = blocks[0].operations().last()
    else {
        panic!("fixture must end with an unproved request");
    };
    let result =
        check_constant_u32_operands_v1(&blocks, contract, *subjects, checked as u32, &mut budget);
    assert_eq!(budget.storage(), 31);
    assert!(budget.work_ledger_identity_v1() == ledger);
    let used = budget.work();
    (result, used, work.failed_work())
}

fn refusal(result: Result<(), Error>, expected: &'static str) {
    assert!(
        matches!(result, Err(Error::Reference(reason)) if reason == expected),
        "{result:?}"
    );
}

#[test]
fn gpu_roles_reject_value_type_model_and_definition_substitutions() {
    for role in 0..4 {
        for change in 0..7 {
            let mut operations = sample(17, 17);
            let Op::SemanticExpression {
                expression,
                numerical_contract,
                ..
            } = &mut operations[role]
            else {
                unreachable!();
            };
            match change {
                0 => {
                    *expression = if role == 0 {
                        Expr::Symbol {
                            symbol: 1,
                            scalar: U64,
                        }
                    } else {
                        Expr::Constant {
                            scalar: if role == 3 { U32 } else { Ty::Bool },
                            bits: if role == 3 { 18 } else { 0 },
                        }
                    }
                }
                1 | 2 => match expression {
                    Expr::Constant { scalar, .. } | Expr::Symbol { scalar, .. } => {
                        *scalar = if change == 2 {
                            Ty::Integer {
                                signed: true,
                                bits: 32,
                            }
                        } else if role == 0 {
                            U32
                        } else {
                            U64
                        };
                    }
                    _ => unreachable!(),
                },
                3 => *numerical_contract = Numerical::Relaxed,
                4 => {
                    *expression = Expr::Unary {
                        operation: ProductionSemanticUnaryOpV2::Not,
                        scalar: U32,
                        operand: Box::new(expression.clone()),
                    }
                }
                5 | 6 => {}
                _ => unreachable!(),
            }
            if change == 5 {
                operations.insert(9, operations[role].clone());
            } else if change == 6 {
                operations[role] = Op::IndexUnknown {
                    result: Id::new(role as u32),
                };
            }
            refusal(
                run(operations, 17, 100_000).0,
                if change == 6 {
                    "missing contract operand"
                } else if change >= 4 {
                    "GPU definition"
                } else {
                    "GPU expression"
                },
            );
        }
    }
}

#[test]
fn selected_gpu_write_requires_its_exact_value_bearing_operands() {
    for change in 0..6 {
        let mut operations = sample(17, 17);
        // A correct write at a different occurrence must not rescue the selected one.
        operations.insert(9, operations[8].clone());
        if change == 0 {
            operations[8] = Op::Access {
                kind: AccessKindAttr::Write,
                view: Value::Argument(0),
                indices: vec![Value::Argument(1)],
            };
        } else if let Op::ValueAccess {
            kind,
            view,
            indices,
            value: stored,
        } = &mut operations[8]
        {
            match change {
                1 => *kind = AccessKindAttr::Read,
                2 => *view = Value::Argument(2),
                3 => indices[0] = Value::Argument(2),
                4 => *stored = value(7),
                5 => indices.push(Value::Argument(1)),
                _ => unreachable!(),
            }
        }
        refusal(run(operations, 17, 100_000).0, "GPU write");
    }
}

#[test]
fn matching_operands_and_subjects_do_not_promote_an_unproved_request() {
    for bits in [0, 1, 17, u32::MAX] {
        let operations = sample(bits, bits);
        let (result, exact, failed) = run(operations.clone(), bits, 100_000);
        refusal(result, "missing required proof");
        assert_eq!(failed, None);
        refusal(
            run(operations.clone(), bits, exact).0,
            "missing required proof",
        );
        let (short, _, failed) = run(operations, bits, exact - 1);
        assert!(
            matches!(short, Err(Error::Resource(Resource::Work(_)))),
            "{short:?}"
        );
        assert_eq!(failed, Some(exact));
    }
    refusal(run(sample(18, 17), 17, 100_000).0, "GPU expression");
}
