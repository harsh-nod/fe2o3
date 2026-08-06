use core::fmt;

/// Maximum number of input buffers in one generated case.
pub const MAX_INPUTS: usize = 4;
/// Maximum number of work-items evaluated by one case.
pub const MAX_WORK_ITEMS: usize = 256;
/// Maximum number of nodes in one output expression.
pub const MAX_EXPR_NODES: usize = 127;
/// Maximum root-to-leaf expression depth, including both endpoints.
pub const MAX_EXPR_DEPTH: usize = 12;

/// A deterministic unary operation with wrapping `i32` semantics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnaryOp {
    Neg,
    Not,
}

/// A deterministic binary operation.
///
/// Arithmetic wraps in two's-complement `i32`; comparisons return zero or one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    BitAnd,
    BitOr,
    BitXor,
    Eq,
    Lt,
}

/// A scalar expression evaluated independently for each global invocation ID.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Expr {
    Const(i32),
    GlobalId,
    /// Loads the current invocation's element from an input buffer.
    Load {
        input: u8,
    },
    Unary {
        op: UnaryOp,
        value: Box<Self>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Self>,
        right: Box<Self>,
    },
    /// Evaluates exactly one arm. Zero is false and every other value is true.
    Select {
        condition: Box<Self>,
        then_value: Box<Self>,
        else_value: Box<Self>,
    },
}

impl Expr {
    /// Returns the exact node count of this expression.
    pub fn node_count(&self) -> usize {
        match self {
            Self::Const(_) | Self::GlobalId | Self::Load { .. } => 1,
            Self::Unary { value, .. } => 1 + value.node_count(),
            Self::Binary { left, right, .. } => 1 + left.node_count() + right.node_count(),
            Self::Select {
                condition,
                then_value,
                else_value,
            } => 1 + condition.node_count() + then_value.node_count() + else_value.node_count(),
        }
    }

    /// Returns the root-to-leaf depth of this expression.
    pub fn depth(&self) -> usize {
        match self {
            Self::Const(_) | Self::GlobalId | Self::Load { .. } => 1,
            Self::Unary { value, .. } => 1 + value.depth(),
            Self::Binary { left, right, .. } => 1 + left.depth().max(right.depth()),
            Self::Select {
                condition,
                then_value,
                else_value,
            } => {
                1 + condition
                    .depth()
                    .max(then_value.depth())
                    .max(else_value.depth())
            }
        }
    }

    pub(crate) fn maximum_input(&self) -> Option<u8> {
        match self {
            Self::Const(_) | Self::GlobalId => None,
            Self::Load { input } => Some(*input),
            Self::Unary { value, .. } => value.maximum_input(),
            Self::Binary { left, right, .. } => left.maximum_input().max(right.maximum_input()),
            Self::Select {
                condition,
                then_value,
                else_value,
            } => condition
                .maximum_input()
                .max(then_value.maximum_input())
                .max(else_value.maximum_input()),
        }
    }
}

/// A bounded independent-work-item kernel program.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Program {
    input_count: u8,
    work_items: u16,
    expression: Expr,
}

impl Program {
    pub fn new(input_count: u8, work_items: u16, expression: Expr) -> Result<Self, ModelError> {
        let program = Self {
            input_count,
            work_items,
            expression,
        };
        program.validate()?;
        Ok(program)
    }

    pub fn input_count(&self) -> u8 {
        self.input_count
    }

    pub fn work_items(&self) -> u16 {
        self.work_items
    }

    pub fn expression(&self) -> &Expr {
        &self.expression
    }

    pub fn validate(&self) -> Result<(), ModelError> {
        if usize::from(self.input_count) > MAX_INPUTS {
            return Err(ModelError::TooManyInputs {
                actual: usize::from(self.input_count),
            });
        }
        if self.work_items == 0 || usize::from(self.work_items) > MAX_WORK_ITEMS {
            return Err(ModelError::InvalidWorkItemCount {
                actual: usize::from(self.work_items),
            });
        }
        let nodes = self.expression.node_count();
        if nodes > MAX_EXPR_NODES {
            return Err(ModelError::ExpressionTooLarge { actual: nodes });
        }
        let depth = self.expression.depth();
        if depth > MAX_EXPR_DEPTH {
            return Err(ModelError::ExpressionTooDeep { actual: depth });
        }
        if let Some(input) = self.expression.maximum_input()
            && input >= self.input_count
        {
            return Err(ModelError::UnknownInput {
                input,
                input_count: self.input_count,
            });
        }
        Ok(())
    }

    pub(crate) fn with_expression(&self, expression: Expr) -> Result<Self, ModelError> {
        Self::new(self.input_count, self.work_items, expression)
    }

    pub(crate) fn with_shape(&self, input_count: u8, work_items: u16) -> Result<Self, ModelError> {
        Self::new(input_count, work_items, self.expression.clone())
    }
}

/// One reproducible differential-test input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KernelCase {
    seed: u64,
    program: Program,
    inputs: Vec<Vec<i32>>,
}

impl KernelCase {
    pub fn new(seed: u64, program: Program, inputs: Vec<Vec<i32>>) -> Result<Self, ModelError> {
        let case = Self {
            seed,
            program,
            inputs,
        };
        case.validate()?;
        Ok(case)
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    pub fn program(&self) -> &Program {
        &self.program
    }

    pub fn inputs(&self) -> &[Vec<i32>] {
        &self.inputs
    }

    pub fn validate(&self) -> Result<(), ModelError> {
        self.program.validate()?;
        if self.inputs.len() != usize::from(self.program.input_count) {
            return Err(ModelError::InputCountMismatch {
                declared: usize::from(self.program.input_count),
                actual: self.inputs.len(),
            });
        }
        let expected = usize::from(self.program.work_items);
        for (input, values) in self.inputs.iter().enumerate() {
            if values.len() != expected {
                return Err(ModelError::InputLengthMismatch {
                    input,
                    expected,
                    actual: values.len(),
                });
            }
        }
        Ok(())
    }

    pub(crate) fn rebuild(
        &self,
        program: Program,
        inputs: Vec<Vec<i32>>,
    ) -> Result<Self, ModelError> {
        Self::new(self.seed, program, inputs)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModelError {
    TooManyInputs {
        actual: usize,
    },
    InvalidWorkItemCount {
        actual: usize,
    },
    ExpressionTooLarge {
        actual: usize,
    },
    ExpressionTooDeep {
        actual: usize,
    },
    UnknownInput {
        input: u8,
        input_count: u8,
    },
    InputCountMismatch {
        declared: usize,
        actual: usize,
    },
    InputLengthMismatch {
        input: usize,
        expected: usize,
        actual: usize,
    },
}

impl fmt::Display for ModelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyInputs { actual } => {
                write!(formatter, "{actual} input buffers exceed the bound")
            }
            Self::InvalidWorkItemCount { actual } => {
                write!(
                    formatter,
                    "work-item count {actual} is outside the supported bound"
                )
            }
            Self::ExpressionTooLarge { actual } => {
                write!(
                    formatter,
                    "expression has {actual} nodes and exceeds the bound"
                )
            }
            Self::ExpressionTooDeep { actual } => {
                write!(formatter, "expression depth {actual} exceeds the bound")
            }
            Self::UnknownInput { input, input_count } => write!(
                formatter,
                "expression references input {input}, but only {input_count} inputs are declared"
            ),
            Self::InputCountMismatch { declared, actual } => write!(
                formatter,
                "program declares {declared} inputs, but the case contains {actual}"
            ),
            Self::InputLengthMismatch {
                input,
                expected,
                actual,
            } => write!(
                formatter,
                "input {input} contains {actual} elements; expected {expected}"
            ),
        }
    }
}

impl std::error::Error for ModelError {}
