use core::fmt;

use crate::{Expr, KernelCase, ModelError};

/// Hard limit on predicate evaluations in one reduction.
pub const MAX_REDUCTION_ATTEMPTS: usize = 1_000_000;

/// A lexicographic, strictly decreasing progress measure for reduction.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CaseComplexity {
    pub expression_nodes: usize,
    pub expression_weight: u64,
    pub work_items: usize,
    pub input_buffers: usize,
    pub nonzero_input_values: usize,
    pub input_magnitude: u64,
}

impl CaseComplexity {
    pub fn measure(case: &KernelCase) -> Self {
        let mut nonzero_input_values = 0;
        let mut input_magnitude = 0_u64;
        for value in case.inputs().iter().flatten() {
            nonzero_input_values += usize::from(*value != 0);
            input_magnitude = input_magnitude.saturating_add(u64::from(value.unsigned_abs()));
        }
        Self {
            expression_nodes: case.program().expression().node_count(),
            expression_weight: expression_weight(case.program().expression()),
            work_items: usize::from(case.program().work_items()),
            input_buffers: case.inputs().len(),
            nonzero_input_values,
            input_magnitude,
        }
    }
}

/// A reduced case and deterministic accounting for the reduction search.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReductionResult {
    pub case: KernelCase,
    pub initial_complexity: CaseComplexity,
    pub final_complexity: CaseComplexity,
    pub predicate_evaluations: usize,
    pub accepted_reductions: usize,
}

/// Reduces `case` while `mismatch` remains true.
///
/// Candidates are visited in a stable order. The result is locally minimal for
/// this reducer's expression, shape, and input-value transformations.
pub fn reduce_case<F>(case: &KernelCase, mut mismatch: F) -> Result<ReductionResult, ReduceError>
where
    F: FnMut(&KernelCase) -> bool,
{
    case.validate().map_err(ReduceError::InvalidCase)?;
    let initial_complexity = CaseComplexity::measure(case);
    if !mismatch(case) {
        return Err(ReduceError::InitialMismatchAbsent);
    }

    let mut current = case.clone();
    let mut predicate_evaluations = 1;
    let mut accepted_reductions = 0;
    loop {
        let candidates = reduction_candidates(&current)?;
        let mut accepted = None;
        for candidate in candidates {
            if predicate_evaluations == MAX_REDUCTION_ATTEMPTS {
                return Err(ReduceError::AttemptLimitExceeded);
            }
            predicate_evaluations += 1;
            if mismatch(&candidate) {
                accepted = Some(candidate);
                accepted_reductions += 1;
                break;
            }
        }
        match accepted {
            Some(candidate) => current = candidate,
            None => break,
        }
    }

    Ok(ReductionResult {
        final_complexity: CaseComplexity::measure(&current),
        case: current,
        initial_complexity,
        predicate_evaluations,
        accepted_reductions,
    })
}

fn reduction_candidates(case: &KernelCase) -> Result<Vec<KernelCase>, ReduceError> {
    let current_complexity = CaseComplexity::measure(case);
    let mut candidates = Vec::new();

    let mut paths = Vec::new();
    expression_paths(case.program().expression(), &mut Vec::new(), &mut paths);
    for path in paths {
        let expression = expression_at(case.program().expression(), &path);
        for replacement in expression_replacements(expression) {
            let replaced = replace_expression(case.program().expression(), &path, &replacement);
            if let Ok(program) = case.program().with_expression(replaced)
                && let Ok(candidate) = case.rebuild(program, case.inputs().to_vec())
            {
                push_if_smaller(&mut candidates, candidate, current_complexity);
            }
        }
    }

    let work_items = usize::from(case.program().work_items());
    for reduced_work_items in 1..work_items {
        let reduced = u16::try_from(reduced_work_items).expect("work-item bound fits u16");
        let program = case
            .program()
            .with_shape(case.program().input_count(), reduced)
            .map_err(ReduceError::InvalidCase)?;
        let inputs = case
            .inputs()
            .iter()
            .map(|values| values[..reduced_work_items].to_vec())
            .collect();
        let candidate = case
            .rebuild(program, inputs)
            .map_err(ReduceError::InvalidCase)?;
        push_if_smaller(&mut candidates, candidate, current_complexity);
    }

    let required_inputs = case
        .program()
        .expression()
        .maximum_input()
        .map_or(0, |input| usize::from(input) + 1);
    for input_count in required_inputs..case.inputs().len() {
        let program = case
            .program()
            .with_shape(
                u8::try_from(input_count).expect("input bound fits u8"),
                case.program().work_items(),
            )
            .map_err(ReduceError::InvalidCase)?;
        let candidate = case
            .rebuild(program, case.inputs()[..input_count].to_vec())
            .map_err(ReduceError::InvalidCase)?;
        push_if_smaller(&mut candidates, candidate, current_complexity);
    }

    let mut all_zero = case.inputs().to_vec();
    for values in &mut all_zero {
        values.fill(0);
    }
    let all_zero_case = case
        .rebuild(case.program().clone(), all_zero)
        .map_err(ReduceError::InvalidCase)?;
    push_if_smaller(&mut candidates, all_zero_case, current_complexity);

    for input in 0..case.inputs().len() {
        let mut zero_buffer = case.inputs().to_vec();
        zero_buffer[input].fill(0);
        let zero_buffer_case = case
            .rebuild(case.program().clone(), zero_buffer)
            .map_err(ReduceError::InvalidCase)?;
        push_if_smaller(&mut candidates, zero_buffer_case, current_complexity);

        for index in 0..case.inputs()[input].len() {
            for replacement in [0, 1, -1] {
                if case.inputs()[input][index] == replacement {
                    continue;
                }
                let mut inputs = case.inputs().to_vec();
                inputs[input][index] = replacement;
                let candidate = case
                    .rebuild(case.program().clone(), inputs)
                    .map_err(ReduceError::InvalidCase)?;
                push_if_smaller(&mut candidates, candidate, current_complexity);
            }
        }
    }

    Ok(candidates)
}

fn push_if_smaller(
    candidates: &mut Vec<KernelCase>,
    candidate: KernelCase,
    current: CaseComplexity,
) {
    if CaseComplexity::measure(&candidate) < current && !candidates.contains(&candidate) {
        candidates.push(candidate);
    }
}

fn expression_weight(expression: &Expr) -> u64 {
    match expression {
        Expr::Const(value) => u64::from(value.unsigned_abs()),
        Expr::GlobalId => 1,
        Expr::Load { input } => 2 + u64::from(*input),
        Expr::Unary { value, .. } => 4_u64.saturating_add(expression_weight(value)),
        Expr::Binary { left, right, .. } => 8_u64
            .saturating_add(expression_weight(left))
            .saturating_add(expression_weight(right)),
        Expr::Select {
            condition,
            then_value,
            else_value,
        } => 16_u64
            .saturating_add(expression_weight(condition))
            .saturating_add(expression_weight(then_value))
            .saturating_add(expression_weight(else_value)),
    }
}

fn expression_paths(expression: &Expr, path: &mut Vec<u8>, paths: &mut Vec<Vec<u8>>) {
    paths.push(path.clone());
    match expression {
        Expr::Const(_) | Expr::GlobalId | Expr::Load { .. } => {}
        Expr::Unary { value, .. } => {
            path.push(0);
            expression_paths(value, path, paths);
            path.pop();
        }
        Expr::Binary { left, right, .. } => {
            path.push(0);
            expression_paths(left, path, paths);
            path.pop();
            path.push(1);
            expression_paths(right, path, paths);
            path.pop();
        }
        Expr::Select {
            condition,
            then_value,
            else_value,
        } => {
            for (child, value) in [(0, condition), (1, then_value), (2, else_value)] {
                path.push(child);
                expression_paths(value, path, paths);
                path.pop();
            }
        }
    }
}

fn expression_at<'a>(mut expression: &'a Expr, path: &[u8]) -> &'a Expr {
    for child in path {
        expression = match (expression, child) {
            (Expr::Unary { value, .. }, 0) => value,
            (Expr::Binary { left, .. }, 0) => left,
            (Expr::Binary { right, .. }, 1) => right,
            (Expr::Select { condition, .. }, 0) => condition,
            (Expr::Select { then_value, .. }, 1) => then_value,
            (Expr::Select { else_value, .. }, 2) => else_value,
            _ => unreachable!("paths are derived from the same expression"),
        };
    }
    expression
}

fn expression_replacements(expression: &Expr) -> Vec<Expr> {
    match expression {
        Expr::Const(value) => [0, 1, -1]
            .into_iter()
            .filter(|replacement| replacement != value)
            .map(Expr::Const)
            .collect(),
        Expr::GlobalId => vec![Expr::Const(0)],
        Expr::Load { .. } => vec![Expr::Const(0), Expr::GlobalId],
        Expr::Unary { value, .. } => vec![(**value).clone(), Expr::Const(0)],
        Expr::Binary { left, right, .. } => {
            vec![(**left).clone(), (**right).clone(), Expr::Const(0)]
        }
        Expr::Select {
            then_value,
            else_value,
            ..
        } => vec![
            (**then_value).clone(),
            (**else_value).clone(),
            Expr::Const(0),
        ],
    }
}

fn replace_expression(expression: &Expr, path: &[u8], replacement: &Expr) -> Expr {
    let Some((&child, remaining)) = path.split_first() else {
        return replacement.clone();
    };
    match expression {
        Expr::Unary { op, value } => {
            debug_assert_eq!(child, 0);
            Expr::Unary {
                op: *op,
                value: Box::new(replace_expression(value, remaining, replacement)),
            }
        }
        Expr::Binary { op, left, right } => Expr::Binary {
            op: *op,
            left: if child == 0 {
                Box::new(replace_expression(left, remaining, replacement))
            } else {
                left.clone()
            },
            right: if child == 1 {
                Box::new(replace_expression(right, remaining, replacement))
            } else {
                right.clone()
            },
        },
        Expr::Select {
            condition,
            then_value,
            else_value,
        } => Expr::Select {
            condition: if child == 0 {
                Box::new(replace_expression(condition, remaining, replacement))
            } else {
                condition.clone()
            },
            then_value: if child == 1 {
                Box::new(replace_expression(then_value, remaining, replacement))
            } else {
                then_value.clone()
            },
            else_value: if child == 2 {
                Box::new(replace_expression(else_value, remaining, replacement))
            } else {
                else_value.clone()
            },
        },
        Expr::Const(_) | Expr::GlobalId | Expr::Load { .. } => {
            unreachable!("paths are derived from the same expression")
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReduceError {
    InvalidCase(ModelError),
    InitialMismatchAbsent,
    AttemptLimitExceeded,
}

impl fmt::Display for ReduceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCase(error) => write!(formatter, "cannot reduce an invalid case: {error}"),
            Self::InitialMismatchAbsent => {
                formatter.write_str("the initial case does not satisfy the mismatch predicate")
            }
            Self::AttemptLimitExceeded => {
                formatter.write_str("reduction predicate-evaluation limit was exceeded")
            }
        }
    }
}

impl std::error::Error for ReduceError {}
