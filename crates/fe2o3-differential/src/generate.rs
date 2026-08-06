use core::fmt;

use crate::{
    BinaryOp, Expr, KernelCase, MAX_EXPR_DEPTH, MAX_EXPR_NODES, MAX_INPUTS, MAX_WORK_ITEMS,
    Program, UnaryOp,
};

/// Bounded controls for deterministic case generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GenerateConfig {
    input_count: u8,
    work_items: u16,
    max_nodes: u8,
    max_depth: u8,
}

impl GenerateConfig {
    pub fn new(
        input_count: u8,
        work_items: u16,
        max_nodes: u8,
        max_depth: u8,
    ) -> Result<Self, GenerateError> {
        if usize::from(input_count) > MAX_INPUTS {
            return Err(GenerateError::InvalidInputCount {
                actual: usize::from(input_count),
            });
        }
        if work_items == 0 || usize::from(work_items) > MAX_WORK_ITEMS {
            return Err(GenerateError::InvalidWorkItemCount {
                actual: usize::from(work_items),
            });
        }
        if max_nodes == 0 || usize::from(max_nodes) > MAX_EXPR_NODES {
            return Err(GenerateError::InvalidNodeBound {
                actual: usize::from(max_nodes),
            });
        }
        if max_depth == 0 || usize::from(max_depth) > MAX_EXPR_DEPTH {
            return Err(GenerateError::InvalidDepthBound {
                actual: usize::from(max_depth),
            });
        }
        Ok(Self {
            input_count,
            work_items,
            max_nodes,
            max_depth,
        })
    }

    pub fn input_count(self) -> u8 {
        self.input_count
    }

    pub fn work_items(self) -> u16 {
        self.work_items
    }

    pub fn max_nodes(self) -> u8 {
        self.max_nodes
    }

    pub fn max_depth(self) -> u8 {
        self.max_depth
    }
}

impl Default for GenerateConfig {
    fn default() -> Self {
        Self {
            input_count: 2,
            work_items: 32,
            max_nodes: 31,
            max_depth: 7,
        }
    }
}

/// Generates one case. Equal seeds and configuration always produce equal cases.
pub fn generate_case(seed: u64, config: GenerateConfig) -> KernelCase {
    let mut random = SplitMix64::new(seed);
    let expression = generate_expr(
        &mut random,
        config.input_count,
        usize::from(config.max_nodes),
        usize::from(config.max_depth),
    );
    let program = Program::new(config.input_count, config.work_items, expression)
        .expect("generator configuration and node accounting are valid");
    let mut inputs = Vec::with_capacity(usize::from(config.input_count));
    for _ in 0..config.input_count {
        let mut values = Vec::with_capacity(usize::from(config.work_items));
        for _ in 0..config.work_items {
            values.push(random.next_u64() as i32);
        }
        inputs.push(values);
    }
    KernelCase::new(seed, program, inputs).expect("generator emits a valid bounded case")
}

fn generate_expr(random: &mut SplitMix64, input_count: u8, budget: usize, depth: usize) -> Expr {
    if budget <= 1 || depth <= 1 {
        return generate_leaf(random, input_count);
    }

    let choice = random.bounded(100);
    if choice < 25 && budget >= 2 {
        Expr::Unary {
            op: if random.bounded(2) == 0 {
                UnaryOp::Neg
            } else {
                UnaryOp::Not
            },
            value: Box::new(generate_expr(random, input_count, budget - 1, depth - 1)),
        }
    } else if choice < 80 && budget >= 3 {
        let remaining = budget - 1;
        let left_budget = 1 + random.bounded(remaining - 1);
        let right_budget = remaining - left_budget;
        Expr::Binary {
            op: binary_op(random.bounded(8)),
            left: Box::new(generate_expr(random, input_count, left_budget, depth - 1)),
            right: Box::new(generate_expr(random, input_count, right_budget, depth - 1)),
        }
    } else if budget >= 4 {
        let remaining = budget - 1;
        let condition_budget = 1 + random.bounded(remaining - 2);
        let after_condition = remaining - condition_budget;
        let then_budget = 1 + random.bounded(after_condition - 1);
        let else_budget = after_condition - then_budget;
        Expr::Select {
            condition: Box::new(generate_expr(
                random,
                input_count,
                condition_budget,
                depth - 1,
            )),
            then_value: Box::new(generate_expr(random, input_count, then_budget, depth - 1)),
            else_value: Box::new(generate_expr(random, input_count, else_budget, depth - 1)),
        }
    } else {
        generate_leaf(random, input_count)
    }
}

fn generate_leaf(random: &mut SplitMix64, input_count: u8) -> Expr {
    match random.bounded(if input_count == 0 { 2 } else { 3 }) {
        0 => Expr::Const(random.next_u64() as i32),
        1 => Expr::GlobalId,
        _ => Expr::Load {
            input: random.bounded(usize::from(input_count)) as u8,
        },
    }
}

fn binary_op(value: usize) -> BinaryOp {
    match value {
        0 => BinaryOp::Add,
        1 => BinaryOp::Sub,
        2 => BinaryOp::Mul,
        3 => BinaryOp::BitAnd,
        4 => BinaryOp::BitOr,
        5 => BinaryOp::BitXor,
        6 => BinaryOp::Eq,
        _ => BinaryOp::Lt,
    }
}

#[derive(Clone, Copy, Debug)]
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    fn bounded(&mut self, exclusive_upper: usize) -> usize {
        debug_assert!(exclusive_upper > 0);
        (self.next_u64() % exclusive_upper as u64) as usize
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GenerateError {
    InvalidInputCount { actual: usize },
    InvalidWorkItemCount { actual: usize },
    InvalidNodeBound { actual: usize },
    InvalidDepthBound { actual: usize },
}

impl fmt::Display for GenerateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInputCount { actual } => {
                write!(
                    formatter,
                    "input count {actual} exceeds the generator bound"
                )
            }
            Self::InvalidWorkItemCount { actual } => {
                write!(
                    formatter,
                    "work-item count {actual} is outside the generator bound"
                )
            }
            Self::InvalidNodeBound { actual } => {
                write!(
                    formatter,
                    "node bound {actual} is outside the generator bound"
                )
            }
            Self::InvalidDepthBound { actual } => {
                write!(
                    formatter,
                    "depth bound {actual} is outside the generator bound"
                )
            }
        }
    }
}

impl std::error::Error for GenerateError {}
