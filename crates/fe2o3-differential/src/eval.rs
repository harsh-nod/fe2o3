use crate::{BinaryOp, Expr, KernelCase, UnaryOp};

/// Maximum number of per-lane details retained in one mismatch report.
pub const MAX_REPORTED_MISMATCHES: usize = 32;

/// Evaluates the reference semantics for every work-item in a case.
pub fn evaluate_case(case: &KernelCase) -> Vec<i32> {
    (0..usize::from(case.program().work_items()))
        .map(|lane| evaluate_lane(case, lane))
        .collect()
}

/// Evaluates one lane. `lane` must be less than the case's work-item count.
pub fn evaluate_lane(case: &KernelCase, lane: usize) -> i32 {
    assert!(lane < usize::from(case.program().work_items()));
    evaluate_expr(case.program().expression(), case.inputs(), lane)
}

fn evaluate_expr(expression: &Expr, inputs: &[Vec<i32>], lane: usize) -> i32 {
    match expression {
        Expr::Const(value) => *value,
        Expr::GlobalId => lane as i32,
        Expr::Load { input } => inputs[usize::from(*input)][lane],
        Expr::Unary { op, value } => {
            let value = evaluate_expr(value, inputs, lane);
            match op {
                UnaryOp::Neg => value.wrapping_neg(),
                UnaryOp::Not => !value,
            }
        }
        Expr::Binary { op, left, right } => {
            let left = evaluate_expr(left, inputs, lane);
            let right = evaluate_expr(right, inputs, lane);
            match op {
                BinaryOp::Add => left.wrapping_add(right),
                BinaryOp::Sub => left.wrapping_sub(right),
                BinaryOp::Mul => left.wrapping_mul(right),
                BinaryOp::BitAnd => left & right,
                BinaryOp::BitOr => left | right,
                BinaryOp::BitXor => left ^ right,
                BinaryOp::Eq => i32::from(left == right),
                BinaryOp::Lt => i32::from(left < right),
            }
        }
        Expr::Select {
            condition,
            then_value,
            else_value,
        } => {
            if evaluate_expr(condition, inputs, lane) != 0 {
                evaluate_expr(then_value, inputs, lane)
            } else {
                evaluate_expr(else_value, inputs, lane)
            }
        }
    }
}

/// One differing or missing lane value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaneMismatch {
    pub lane: usize,
    pub expected: Option<i32>,
    pub observed: Option<i32>,
}

/// A bounded, structured comparison against the CPU reference evaluator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MismatchReport {
    pub seed: u64,
    pub expected_len: usize,
    pub observed_len: usize,
    pub total_mismatches: usize,
    pub mismatches: Vec<LaneMismatch>,
    pub truncated: bool,
}

impl MismatchReport {
    pub fn is_mismatch(&self) -> bool {
        self.total_mismatches != 0
    }
}

/// Compares arbitrary observed output with the CPU reference result.
pub fn compare_outputs(case: &KernelCase, observed: &[i32]) -> MismatchReport {
    let expected = evaluate_case(case);
    let compared_len = expected.len().max(observed.len());
    let mut total_mismatches = 0;
    let mut mismatches = Vec::new();
    for lane in 0..compared_len {
        let expected_value = expected.get(lane).copied();
        let observed_value = observed.get(lane).copied();
        if expected_value != observed_value {
            total_mismatches += 1;
            if mismatches.len() < MAX_REPORTED_MISMATCHES {
                mismatches.push(LaneMismatch {
                    lane,
                    expected: expected_value,
                    observed: observed_value,
                });
            }
        }
    }
    MismatchReport {
        seed: case.seed(),
        expected_len: expected.len(),
        observed_len: observed.len(),
        total_mismatches,
        truncated: total_mismatches > mismatches.len(),
        mismatches,
    }
}
