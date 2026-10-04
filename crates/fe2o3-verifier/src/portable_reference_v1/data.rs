//! Inert CPU reference data; construction confers no source or proof authority.

use std::fmt;

pub const MAX_REFERENCE_BLOCKS_V1: usize = 4_096;
pub const MAX_REFERENCE_STATEMENTS_V1: usize = 65_536;
pub const MAX_REFERENCE_POINT_AXES_V1: usize = 3;
pub const MAX_REFERENCE_GUARD_CLAUSES_V1: usize = 65_536;
pub const MAX_REFERENCE_GUARD_ATOMS_V1: usize = 262_144;
pub const MAX_REFERENCE_EXPRESSION_NODES_V1: usize = 8_192;
pub const MAX_REFERENCE_SYMBOLIC_STEPS_V2: usize = 65_536;
pub const MAX_REFERENCE_SYMBOLIC_WORK_NODES_V2: usize = 1_048_576;
pub const MAX_REFERENCE_LOOP_ITERATIONS_V2: usize = 4_096;
pub const MAX_REFERENCE_HELPER_ARGUMENTS_V2: usize = 64;

/// Keeps logical kernel-scalar arguments disjoint from the three point-axis
/// symbols used by the functional-refinement formula.
pub fn kernel_scalar_symbol_v2(argument: u32) -> Option<u32> {
    let symbol = fe2o3_pliron::PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2.checked_add(argument)?;
    (symbol < fe2o3_pliron::PRODUCTION_SEMANTIC_LOAD_SYMBOL_BASE_V2).then_some(symbol)
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ReferenceScalarTypeV1 {
    Bool,
    U8,
    U16,
    U32,
    U64,
    Usize,
    I8,
    I16,
    I32,
    I64,
    Isize,
    F32,
    F64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReferenceArgumentRelationV1 {
    PointCoordinate {
        reference_argument: u32,
        axis: u32,
    },
    ScalarInput {
        argument: u32,
        scalar: ReferenceScalarTypeV1,
    },
    SharedSliceInput {
        argument: u32,
        element: ReferenceScalarTypeV1,
    },
    DisjointOutputSlice {
        argument: u32,
        element: ReferenceScalarTypeV1,
    },
    DisjointOutputCoordinate {
        argument: u32,
        element: ReferenceScalarTypeV1,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReferenceFunctionIdentityV1 {
    pub def_path_hash: [u8; 16],
    pub function_sha256: [u8; 32],
    pub item_definition_sha256: [u8; 32],
    pub monomorphization_sha256: [u8; 32],
    pub generic_type_arguments_sha256: [u8; 32],
    pub const_generic_arguments_sha256: [u8; 32],
    pub rustc_mir_body_sha256: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReferencePlaceProjectionV1 {
    Dereference,
    Field(u32),
    Index(u32),
    ConstantIndex {
        offset: u64,
        minimum_length: u64,
        from_end: bool,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferencePlaceV1 {
    pub local: u32,
    pub projection: Box<[ReferencePlaceProjectionV1]>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ReferenceConstantV1 {
    ZeroSized,
    Scalar {
        scalar: ReferenceScalarTypeV1,
        bits: u128,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReferenceOperandV1 {
    Copy(ReferencePlaceV1),
    Move(ReferencePlaceV1),
    Constant(ReferenceConstantV1),
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ReferenceBinaryOpV1 {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    BitXor,
    BitAnd,
    BitOr,
    ShiftLeft,
    ShiftRight,
    Equal,
    LessThan,
    LessEqual,
    NotEqual,
    GreaterEqual,
    GreaterThan,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ReferenceUnaryOpV1 {
    Not,
    Negate,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ReferenceCastKindV1 {
    Integer,
    IntegerToFloat,
    FloatToFloat,
    FloatToIntegerSaturating,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReferenceValueV1 {
    Use(ReferenceOperandV1),
    Binary {
        operation: ReferenceBinaryOpV1,
        lhs: ReferenceOperandV1,
        rhs: ReferenceOperandV1,
        checked: bool,
    },
    Unary {
        operation: ReferenceUnaryOpV1,
        operand: ReferenceOperandV1,
    },
    Cast {
        kind: ReferenceCastKindV1,
        source: ReferenceScalarTypeV1,
        target: ReferenceScalarTypeV1,
        operand: ReferenceOperandV1,
    },
    InputLength {
        reference_argument: u32,
    },
    /// Exact, compiler-derived summary of one direct safe local scalar helper.
    /// The summary uses `KernelScalarArgument` leaves as helper-formal symbols;
    /// the resolver substitutes the independently lowered call operands.
    SafeHelperCall {
        helper: ReferenceFunctionIdentityV1,
        parameters: Box<[ReferenceScalarTypeV1]>,
        result: ReferenceScalarTypeV1,
        arguments: Box<[ReferenceOperandV1]>,
        summary: Box<ReferenceEffectExpressionV1>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferenceAssignmentV1 {
    pub statement: u32,
    pub destination: ReferencePlaceV1,
    pub value: ReferenceValueV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReferenceTerminatorV1 {
    Return,
    Goto {
        target: u32,
    },
    Switch {
        discriminant: ReferenceOperandV1,
        values: Box<[(u128, u32)]>,
        otherwise: u32,
    },
    Assert {
        condition: ReferenceOperandV1,
        expected: bool,
        success: u32,
        bounds_check: Option<ReferenceBoundsCheckV1>,
    },
}

/// Exact operands retained from one compiler-generated safe-slice bounds
/// assertion. The assertion condition remains independently retained on the
/// terminator and must normalize to `index < length` before it can be erased.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferenceBoundsCheckV1 {
    pub index: ReferenceOperandV1,
    pub length: ReferenceOperandV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedReferenceBoundsCheckV1 {
    pub block: u32,
    pub expected: bool,
    pub condition: ReferenceEffectExpressionV1,
    pub index: ReferenceEffectExpressionV1,
    pub length: ReferenceEffectExpressionV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferenceBlockV1 {
    pub block: u32,
    pub assignments: Box<[ReferenceAssignmentV1]>,
    pub terminator: ReferenceTerminatorV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferenceEffectIrV1 {
    pub argument_count: u32,
    pub local_count: u32,
    pub relations: Box<[ReferenceArgumentRelationV1]>,
    pub blocks: Box<[ReferenceBlockV1]>,
    /// Exact finite recurrences encountered while deriving output effects.
    /// These records supplement, rather than replace, the final unrolled
    /// expression used by the current scalar semantic join.
    pub loop_summaries: Box<[ReferenceLoopSummaryV2]>,
    /// Compiler-derived point effects. This is per-effect partial correctness
    /// evidence; it does not assert that a dynamic output view is totally
    /// covered by the kernel.
    pub observable_output_effects: Box<[ReferenceOutputWriteV1]>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ReferenceLoopSummaryV2 {
    pub header: u32,
    pub latch: u32,
    pub exit: u32,
    pub exact_iterations: Option<u64>,
    pub maximum_iterations: u64,
    pub carried_locals: Box<[u32]>,
    pub initial_state_sha256: [u8; 32],
    pub transition_sha256: [u8; 32],
    pub variant_sha256: [u8; 32],
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ReferenceEffectExpressionV1 {
    PointCoordinate {
        axis: u32,
    },
    KernelScalarArgument {
        argument: u32,
    },
    Constant(ReferenceConstantV1),
    /// Safe reference load from one exact logical input argument. The
    /// production join must bind it to a unique live ranked GPU read.
    InputLoad {
        reference_argument: u32,
        index: Box<Self>,
    },
    InputLength {
        reference_argument: u32,
    },
    Binary {
        operation: ReferenceBinaryOpV1,
        lhs: Box<Self>,
        rhs: Box<Self>,
        checked: bool,
    },
    Unary {
        operation: ReferenceUnaryOpV1,
        operand: Box<Self>,
    },
    Cast {
        kind: ReferenceCastKindV1,
        source: ReferenceScalarTypeV1,
        target: ReferenceScalarTypeV1,
        operand: Box<Self>,
    },
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ReferenceGuardAtomV1 {
    SwitchValueSet {
        discriminant: ReferenceEffectExpressionV1,
        values: Box<[u128]>,
        inside_set: bool,
    },
    Assert {
        condition: ReferenceEffectExpressionV1,
        expected: bool,
    },
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ReferenceGuardClauseV1 {
    pub atoms: Box<[ReferenceGuardAtomV1]>,
}

/// A canonical disjunction of conjunctions. No clauses means false; one empty
/// clause means true.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ReferencePathPredicateV1 {
    pub clauses: Box<[ReferenceGuardClauseV1]>,
}

impl ReferencePathPredicateV1 {
    pub fn unconditional_v1() -> Self {
        Self {
            clauses: vec![ReferenceGuardClauseV1 {
                atoms: Box::default(),
            }]
            .into_boxed_slice(),
        }
    }

    pub fn unreachable_v1() -> Self {
        Self {
            clauses: Box::default(),
        }
    }

    pub fn is_unreachable_v1(&self) -> bool {
        self.clauses.is_empty()
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ReferenceOutputCoordinateV1 {
    LogicalPoint(Box<[ReferenceEffectExpressionV1]>),
    SingleCoordinate,
    Dynamic(ReferenceEffectExpressionV1),
    Constant {
        offset: u64,
        minimum_length: u64,
        from_end: bool,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferenceOutputWriteV1 {
    pub argument: u32,
    pub block: u32,
    pub statement: u32,
    pub coordinate: ReferenceOutputCoordinateV1,
    pub guard: ReferencePathPredicateV1,
    pub rhs: ReferenceEffectExpressionV1,
    pub value: ReferenceValueV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferenceBindingErrorV1(String);

impl ReferenceBindingErrorV1 {
    pub fn new(reason: impl Into<String>) -> Self {
        Self(reason.into())
    }
}

impl fmt::Display for ReferenceBindingErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ReferenceBindingErrorV1 {}
