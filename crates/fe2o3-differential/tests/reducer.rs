use fe2o3_differential::{BinaryOp, Expr, KernelCase, Program, ReduceError, reduce_case};

fn contains_mul(expression: &Expr) -> bool {
    match expression {
        Expr::Binary {
            op: BinaryOp::Mul, ..
        } => true,
        Expr::Unary { value, .. } => contains_mul(value),
        Expr::Binary { left, right, .. } => contains_mul(left) || contains_mul(right),
        Expr::Select {
            condition,
            then_value,
            else_value,
        } => contains_mul(condition) || contains_mul(then_value) || contains_mul(else_value),
        Expr::Const(_) | Expr::GlobalId | Expr::Load { .. } => false,
    }
}

fn injected_mismatch(case: &KernelCase) -> bool {
    // Models a backend mutant that miscompiles multiplication whenever one is present.
    contains_mul(case.program().expression())
}

fn fixture() -> KernelCase {
    let mul = Expr::Binary {
        op: BinaryOp::Mul,
        left: Box::new(Expr::Load { input: 0 }),
        right: Box::new(Expr::Const(123)),
    };
    let expression = Expr::Select {
        condition: Box::new(Expr::GlobalId),
        then_value: Box::new(mul.clone()),
        else_value: Box::new(mul),
    };
    KernelCase::new(
        42,
        Program::new(1, 8, expression).unwrap(),
        vec![vec![91, -7, 4, 6, 2, 8, 3, 5]],
    )
    .unwrap()
}

#[test]
fn reducer_preserves_predicate_and_reaches_a_deterministic_local_minimum() {
    let first = reduce_case(&fixture(), injected_mismatch).unwrap();
    let second = reduce_case(&fixture(), injected_mismatch).unwrap();
    assert_eq!(first, second);
    assert!(injected_mismatch(&first.case));
    assert!(first.final_complexity < first.initial_complexity);
    assert_eq!(first.case.program().work_items(), 1);
    assert_eq!(first.case.program().input_count(), 0);
    assert!(first.case.inputs().is_empty());
    assert_eq!(
        first.case.program().expression(),
        &Expr::Binary {
            op: BinaryOp::Mul,
            left: Box::new(Expr::Const(0)),
            right: Box::new(Expr::Const(0)),
        }
    );

    let fixed_point = reduce_case(&first.case, injected_mismatch).unwrap();
    assert_eq!(fixed_point.case, first.case);
    assert_eq!(fixed_point.accepted_reductions, 0);
}

#[test]
fn reducer_rejects_a_case_without_the_injected_mismatch() {
    assert_eq!(
        reduce_case(&fixture(), |_| false),
        Err(ReduceError::InitialMismatchAbsent)
    );
}
