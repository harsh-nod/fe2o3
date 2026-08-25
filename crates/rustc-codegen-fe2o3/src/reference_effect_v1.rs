//! Workload-neutral safe-Rust reference binding and effect IR.
//!
//! This IR is deliberately bounded and fail-closed. It retains exact rustc
//! identities and represents only semantics translated without workload names.
//! A point reference may add up to three leading `usize` coordinate arguments;
//! the remaining arguments map to the kernel ABI. Its extracted write events
//! describe coordinates, path predicates, and RHS expressions independently
//! of the GPU projection. Matching those events proves only per-effect partial
//! correctness, not total coverage of a dynamic output domain.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use rustc_abi::ExternAbi;
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{BlockCheckMode, ExprKind, Mutability, Safety, UnsafeSource};
use rustc_middle::mir::{
    AssertMessage, BinOp, Body, CastKind, Operand, Place, ProjectionElem, Rvalue, StatementKind,
    TerminatorKind, UnOp, UnwindAction,
};
use rustc_middle::ty::{Instance, Ty, TyCtxt, TyKind, TypingEnv};
use rustc_span::Spanned;
use sha2::{Digest as _, Sha256};

use crate::rustc_semantic_adapter_v1::{
    canonical_function_identities_v1, rustc_mir_body_sha256_v1,
};
use crate::trusted_device_items::{self, TrustedDeviceItem};

pub(crate) const MAX_REFERENCE_BLOCKS_V1: usize = 4_096;
pub(crate) const MAX_REFERENCE_STATEMENTS_V1: usize = 65_536;
pub(crate) const MAX_REFERENCE_POINT_AXES_V1: usize = 3;
pub(crate) const MAX_REFERENCE_GUARD_CLAUSES_V1: usize = 65_536;
pub(crate) const MAX_REFERENCE_GUARD_ATOMS_V1: usize = 262_144;
pub(crate) const MAX_REFERENCE_EXPRESSION_NODES_V1: usize = 8_192;
pub(crate) const MAX_REFERENCE_SYMBOLIC_STEPS_V2: usize = 65_536;
pub(crate) const MAX_REFERENCE_SYMBOLIC_WORK_NODES_V2: usize = 1_048_576;
pub(crate) const MAX_REFERENCE_LOOP_ITERATIONS_V2: usize = 4_096;
pub(crate) const MAX_REFERENCE_HELPER_ARGUMENTS_V2: usize = 64;

/// Keeps logical kernel-scalar arguments disjoint from the three point-axis
/// symbols used by the functional-refinement formula.
pub(crate) fn kernel_scalar_symbol_v2(argument: u32) -> Option<u32> {
    (1_u32 << 30).checked_add(argument)
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ReferenceScalarTypeV1 {
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
pub(crate) enum ReferenceArgumentRelationV1 {
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
pub(crate) struct ReferenceFunctionIdentityV1 {
    pub(crate) def_path_hash: [u8; 16],
    pub(crate) function_sha256: [u8; 32],
    pub(crate) item_definition_sha256: [u8; 32],
    pub(crate) monomorphization_sha256: [u8; 32],
    pub(crate) generic_type_arguments_sha256: [u8; 32],
    pub(crate) const_generic_arguments_sha256: [u8; 32],
    pub(crate) rustc_mir_body_sha256: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ReferencePlaceProjectionV1 {
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
pub(crate) struct ReferencePlaceV1 {
    pub(crate) local: u32,
    pub(crate) projection: Box<[ReferencePlaceProjectionV1]>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ReferenceConstantV1 {
    ZeroSized,
    Scalar {
        scalar: ReferenceScalarTypeV1,
        bits: u128,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ReferenceOperandV1 {
    Copy(ReferencePlaceV1),
    Move(ReferencePlaceV1),
    Constant(ReferenceConstantV1),
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ReferenceBinaryOpV1 {
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
pub(crate) enum ReferenceUnaryOpV1 {
    Not,
    Negate,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ReferenceCastKindV1 {
    Integer,
    IntegerToFloat,
    FloatToFloat,
    FloatToIntegerSaturating,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ReferenceValueV1 {
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
pub(crate) struct ReferenceAssignmentV1 {
    pub(crate) statement: u32,
    pub(crate) destination: ReferencePlaceV1,
    pub(crate) value: ReferenceValueV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ReferenceTerminatorV1 {
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
        bounds_check: bool,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReferenceBlockV1 {
    pub(crate) block: u32,
    pub(crate) assignments: Box<[ReferenceAssignmentV1]>,
    pub(crate) terminator: ReferenceTerminatorV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReferenceEffectIrV1 {
    pub(crate) argument_count: u32,
    pub(crate) local_count: u32,
    pub(crate) relations: Box<[ReferenceArgumentRelationV1]>,
    pub(crate) blocks: Box<[ReferenceBlockV1]>,
    /// Exact finite recurrences encountered while deriving output effects.
    /// These records supplement, rather than replace, the final unrolled
    /// expression used by the current scalar semantic join.
    pub(crate) loop_summaries: Box<[ReferenceLoopSummaryV2]>,
    /// Compiler-derived point effects. This is per-effect partial correctness
    /// evidence; it does not assert that a dynamic output view is totally
    /// covered by the kernel.
    pub(crate) observable_output_effects: Box<[ReferenceOutputWriteV1]>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct ReferenceLoopSummaryV2 {
    pub(crate) header: u32,
    pub(crate) latch: u32,
    pub(crate) exit: u32,
    pub(crate) exact_iterations: Option<u64>,
    pub(crate) maximum_iterations: u64,
    pub(crate) carried_locals: Box<[u32]>,
    pub(crate) initial_state_sha256: [u8; 32],
    pub(crate) transition_sha256: [u8; 32],
    pub(crate) variant_sha256: [u8; 32],
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ReferenceEffectExpressionV1 {
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
pub(crate) enum ReferenceGuardAtomV1 {
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
pub(crate) struct ReferenceGuardClauseV1 {
    pub(crate) atoms: Box<[ReferenceGuardAtomV1]>,
}

/// A canonical disjunction of conjunctions. No clauses means false; one empty
/// clause means true.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct ReferencePathPredicateV1 {
    pub(crate) clauses: Box<[ReferenceGuardClauseV1]>,
}

impl ReferencePathPredicateV1 {
    pub(crate) fn unconditional_v1() -> Self {
        Self {
            clauses: vec![ReferenceGuardClauseV1 {
                atoms: Box::default(),
            }]
            .into_boxed_slice(),
        }
    }

    fn unreachable_v1() -> Self {
        Self {
            clauses: Box::default(),
        }
    }

    fn is_unreachable_v1(&self) -> bool {
        self.clauses.is_empty()
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ReferenceOutputCoordinateV1 {
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
pub(crate) struct ReferenceOutputWriteV1 {
    pub(crate) argument: u32,
    pub(crate) block: u32,
    pub(crate) statement: u32,
    pub(crate) coordinate: ReferenceOutputCoordinateV1,
    pub(crate) guard: ReferencePathPredicateV1,
    pub(crate) rhs: ReferenceEffectExpressionV1,
    pub(crate) value: ReferenceValueV1,
}

impl ReferenceEffectIrV1 {
    fn observable_output_writes_v1(
        &self,
    ) -> Result<Vec<ReferenceOutputWriteV1>, ReferenceBindingErrorV1> {
        let guards = reference_block_path_predicates_v1(self)?;
        let resolver = ReferenceExpressionResolverV1::new(self)?;
        let point_coordinates = self
            .relations
            .iter()
            .filter_map(|relation| match relation {
                ReferenceArgumentRelationV1::PointCoordinate { axis, .. } => {
                    Some(ReferenceEffectExpressionV1::PointCoordinate { axis: *axis })
                }
                _ => None,
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let mut writes = Vec::new();
        for relation in &self.relations {
            let (argument, coordinate_output) = match relation {
                ReferenceArgumentRelationV1::DisjointOutputSlice { argument, .. } => {
                    (*argument, false)
                }
                ReferenceArgumentRelationV1::DisjointOutputCoordinate { argument, .. } => {
                    (*argument, true)
                }
                ReferenceArgumentRelationV1::ScalarInput { .. }
                | ReferenceArgumentRelationV1::SharedSliceInput { .. }
                | ReferenceArgumentRelationV1::PointCoordinate { .. } => continue,
            };
            let local = self
                .reference_argument_for_kernel_argument_v1(argument)?
                .checked_add(1)
                .ok_or_else(|| ReferenceBindingErrorV1::new("reference local index overflowed"))?;
            for block in &self.blocks {
                let guard = guards
                    .get(block.block as usize)
                    .ok_or_else(|| {
                        ReferenceBindingErrorV1::new("reference block identity is out of bounds")
                    })?
                    .clone();
                if guard.is_unreachable_v1() {
                    continue;
                }
                for assignment in &block.assignments {
                    if assignment.destination.local != local {
                        continue;
                    }
                    let projection = assignment.destination.projection.as_ref();
                    let coordinate = match projection {
                        [ReferencePlaceProjectionV1::Dereference] if coordinate_output => {
                            if point_coordinates.is_empty() {
                                ReferenceOutputCoordinateV1::SingleCoordinate
                            } else {
                                ReferenceOutputCoordinateV1::LogicalPoint(point_coordinates.clone())
                            }
                        }
                        [
                            ReferencePlaceProjectionV1::Dereference,
                            ReferencePlaceProjectionV1::Index(index),
                        ] if !coordinate_output => {
                            ReferenceOutputCoordinateV1::Dynamic(resolver.resolve_local_v1(*index)?)
                        }
                        [
                            ReferencePlaceProjectionV1::Dereference,
                            ReferencePlaceProjectionV1::ConstantIndex {
                                offset,
                                minimum_length,
                                from_end,
                            },
                        ] if !coordinate_output => ReferenceOutputCoordinateV1::Constant {
                            offset: *offset,
                            minimum_length: *minimum_length,
                            from_end: *from_end,
                        },
                        _ => {
                            return Err(ReferenceBindingErrorV1::new(format!(
                                "observable output argument {} uses unsupported write projection {:?}; reference-effect V1 cannot omit a global output write",
                                argument + 1,
                                assignment.destination.projection,
                            )));
                        }
                    };
                    writes.push(ReferenceOutputWriteV1 {
                        argument,
                        block: block.block,
                        statement: assignment.statement,
                        coordinate,
                        guard: guard.clone(),
                        rhs: resolver.resolve_value_v1(&assignment.value)?,
                        value: assignment.value.clone(),
                    });
                }
            }
        }
        Ok(writes)
    }

    fn point_coordinate_count_v1(&self) -> Result<u32, ReferenceBindingErrorV1> {
        u32::try_from(
            self.relations
                .iter()
                .filter(|relation| {
                    matches!(
                        relation,
                        ReferenceArgumentRelationV1::PointCoordinate { .. }
                    )
                })
                .count(),
        )
        .map_err(|_| ReferenceBindingErrorV1::new("point coordinate count exceeds u32"))
    }

    fn reference_argument_for_kernel_argument_v1(
        &self,
        kernel_argument: u32,
    ) -> Result<u32, ReferenceBindingErrorV1> {
        self.point_coordinate_count_v1()?
            .checked_add(kernel_argument)
            .ok_or_else(|| ReferenceBindingErrorV1::new("reference argument index overflowed"))
    }

    pub(crate) fn canonical_sha256_v1(&self) -> [u8; 32] {
        let mut digest = Sha256::new();
        digest.update(b"fe2o3/reference-effect-ir/v1\0");
        digest.update(self.argument_count.to_le_bytes());
        digest.update(self.local_count.to_le_bytes());
        put_len(&mut digest, self.relations.len());
        for relation in &self.relations {
            match relation {
                ReferenceArgumentRelationV1::PointCoordinate {
                    reference_argument,
                    axis,
                } => {
                    digest.update([4]);
                    digest.update(reference_argument.to_le_bytes());
                    digest.update(axis.to_le_bytes());
                }
                ReferenceArgumentRelationV1::ScalarInput { argument, scalar } => {
                    digest.update([0, scalar_tag(*scalar)]);
                    digest.update(argument.to_le_bytes());
                }
                ReferenceArgumentRelationV1::SharedSliceInput { argument, element } => {
                    digest.update([1, scalar_tag(*element)]);
                    digest.update(argument.to_le_bytes());
                }
                ReferenceArgumentRelationV1::DisjointOutputSlice { argument, element } => {
                    digest.update([2, scalar_tag(*element)]);
                    digest.update(argument.to_le_bytes());
                }
                ReferenceArgumentRelationV1::DisjointOutputCoordinate { argument, element } => {
                    digest.update([3, scalar_tag(*element)]);
                    digest.update(argument.to_le_bytes());
                }
            }
        }
        put_len(&mut digest, self.blocks.len());
        for block in &self.blocks {
            digest.update(block.block.to_le_bytes());
            put_len(&mut digest, block.assignments.len());
            for assignment in &block.assignments {
                digest.update(assignment.statement.to_le_bytes());
                digest_place(&mut digest, &assignment.destination);
                digest_value(&mut digest, &assignment.value);
            }
            digest_terminator(&mut digest, &block.terminator);
        }
        put_len(&mut digest, self.loop_summaries.len());
        for summary in &self.loop_summaries {
            digest.update(summary.header.to_le_bytes());
            digest.update(summary.latch.to_le_bytes());
            digest.update(summary.exit.to_le_bytes());
            match summary.exact_iterations {
                Some(iterations) => {
                    digest.update([1]);
                    digest.update(iterations.to_le_bytes());
                }
                None => digest.update([0]),
            }
            digest.update(summary.maximum_iterations.to_le_bytes());
            put_len(&mut digest, summary.carried_locals.len());
            for local in &summary.carried_locals {
                digest.update(local.to_le_bytes());
            }
            digest.update(summary.initial_state_sha256);
            digest.update(summary.transition_sha256);
            digest.update(summary.variant_sha256);
        }
        put_len(&mut digest, self.observable_output_effects.len());
        for effect in &self.observable_output_effects {
            digest_output_effect_v1(&mut digest, effect);
        }
        digest.finalize().into()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ReferenceSymbolicValueV2 {
    Scalar(ReferenceEffectExpressionV1),
    CheckedPair {
        value: ReferenceEffectExpressionV1,
        overflowed: Option<bool>,
    },
}

type ReferenceSymbolicEnvironmentV2 = BTreeMap<u32, ReferenceSymbolicValueV2>;

#[derive(Clone, Debug)]
struct ReferenceLoopTraceV2 {
    header: u32,
    latch: u32,
    exit: Option<u32>,
    initial: ReferenceSymbolicEnvironmentV2,
    transitions: Vec<ReferenceSymbolicEnvironmentV2>,
    variants: Vec<ReferenceEffectExpressionV1>,
    exact_iterations: Option<u64>,
    maximum_iterations: Option<u64>,
}

#[derive(Clone, Debug)]
struct ReferenceSymbolicStateV2 {
    block: u32,
    environment: ReferenceSymbolicEnvironmentV2,
    guard: ReferencePathPredicateV1,
    traces: BTreeMap<(u32, u32), ReferenceLoopTraceV2>,
}

#[derive(Default)]
struct ReferenceSymbolicWorkBudgetV2 {
    charged_nodes: usize,
}

impl ReferenceSymbolicWorkBudgetV2 {
    fn charge_v2(&mut self, nodes: usize) -> Result<(), ReferenceBindingErrorV1> {
        self.charged_nodes = self.charged_nodes.checked_add(nodes).ok_or_else(|| {
            ReferenceBindingErrorV1::new("reference symbolic work-node accounting overflowed")
        })?;
        if self.charged_nodes > MAX_REFERENCE_SYMBOLIC_WORK_NODES_V2 {
            return Err(ReferenceBindingErrorV1::new(format!(
                "reference symbolic execution exceeds {MAX_REFERENCE_SYMBOLIC_WORK_NODES_V2} cumulative expression work nodes",
            )));
        }
        Ok(())
    }

    fn charge_expression_v2(
        &mut self,
        expression: &ReferenceEffectExpressionV1,
    ) -> Result<(), ReferenceBindingErrorV1> {
        self.charge_v2(symbolic_expression_nodes_v2(expression)?)
    }

    fn charge_environment_v2(
        &mut self,
        environment: &ReferenceSymbolicEnvironmentV2,
    ) -> Result<(), ReferenceBindingErrorV1> {
        self.charge_v2(symbolic_environment_nodes_v2(environment)?)
    }

    fn charge_predicate_v2(
        &mut self,
        predicate: &ReferencePathPredicateV1,
    ) -> Result<(), ReferenceBindingErrorV1> {
        self.charge_v2(symbolic_predicate_nodes_v2(predicate)?)
    }

    fn charge_state_clone_v2(
        &mut self,
        state: &ReferenceSymbolicStateV2,
    ) -> Result<(), ReferenceBindingErrorV1> {
        self.charge_v2(symbolic_state_nodes_v2(state)?)
    }
}

impl ReferenceEffectIrV1 {
    fn observable_output_writes_with_loops_v2(
        &self,
        backedges: &BTreeSet<(u32, u32)>,
    ) -> Result<(Vec<ReferenceOutputWriteV1>, Vec<ReferenceLoopSummaryV2>), ReferenceBindingErrorV1>
    {
        let loop_nodes = reference_natural_loop_nodes_v2(self, backedges)?;
        let loop_headers = backedges
            .iter()
            .map(|(_, header)| *header)
            .collect::<BTreeSet<_>>();
        let mut initial_environment = BTreeMap::new();
        let point_count = self.point_coordinate_count_v1()?;
        for relation in &self.relations {
            match relation {
                ReferenceArgumentRelationV1::PointCoordinate {
                    reference_argument,
                    axis,
                } => {
                    initial_environment.insert(
                        reference_argument + 1,
                        ReferenceSymbolicValueV2::Scalar(
                            ReferenceEffectExpressionV1::PointCoordinate { axis: *axis },
                        ),
                    );
                }
                ReferenceArgumentRelationV1::ScalarInput { argument, .. } => {
                    let reference_argument =
                        point_count.checked_add(*argument).ok_or_else(|| {
                            ReferenceBindingErrorV1::new(
                                "reference scalar argument local index overflowed",
                            )
                        })?;
                    initial_environment.insert(
                        reference_argument + 1,
                        ReferenceSymbolicValueV2::Scalar(
                            ReferenceEffectExpressionV1::KernelScalarArgument {
                                argument: *argument,
                            },
                        ),
                    );
                }
                ReferenceArgumentRelationV1::SharedSliceInput { .. }
                | ReferenceArgumentRelationV1::DisjointOutputSlice { .. }
                | ReferenceArgumentRelationV1::DisjointOutputCoordinate { .. } => {}
            }
        }
        let mut pending = VecDeque::from([ReferenceSymbolicStateV2 {
            block: 0,
            environment: initial_environment,
            guard: ReferencePathPredicateV1::unconditional_v1(),
            traces: BTreeMap::new(),
        }]);
        let mut writes = Vec::new();
        let mut completed_traces = Vec::new();
        let mut steps = 0_usize;
        let mut work_budget = ReferenceSymbolicWorkBudgetV2::default();
        work_budget.charge_environment_v2(&pending[0].environment)?;
        while let Some(mut state) = pending.pop_front() {
            steps = steps.checked_add(1).ok_or_else(|| {
                ReferenceBindingErrorV1::new("reference symbolic execution step count overflowed")
            })?;
            if steps > MAX_REFERENCE_SYMBOLIC_STEPS_V2 {
                return Err(ReferenceBindingErrorV1::new(format!(
                    "reference symbolic execution exceeds {MAX_REFERENCE_SYMBOLIC_STEPS_V2} steps",
                )));
            }
            if loop_headers.contains(&state.block) {
                for (latch, header) in backedges {
                    if *header == state.block && !state.traces.contains_key(&(*header, *latch)) {
                        work_budget.charge_environment_v2(&state.environment)?;
                        state.traces.insert(
                            (*header, *latch),
                            ReferenceLoopTraceV2 {
                                header: *header,
                                latch: *latch,
                                exit: None,
                                initial: state.environment.clone(),
                                transitions: Vec::new(),
                                variants: Vec::new(),
                                exact_iterations: None,
                                maximum_iterations: None,
                            },
                        );
                    }
                }
            }
            let block = self.blocks.get(state.block as usize).ok_or_else(|| {
                ReferenceBindingErrorV1::new(format!(
                    "reference symbolic execution reached missing block {}",
                    state.block,
                ))
            })?;
            for assignment in &block.assignments {
                if let Some((argument, coordinate_output)) =
                    self.output_relation_for_local_v2(assignment.destination.local)?
                {
                    let coordinate = self.symbolic_output_coordinate_v2(
                        &state.environment,
                        &assignment.destination,
                        coordinate_output,
                    )?;
                    let rhs = symbolic_scalar_v2(symbolic_value_v2(
                        &state.environment,
                        &assignment.value,
                    )?)?;
                    require_symbolic_expression_budget_v2(&rhs)?;
                    work_budget.charge_expression_v2(&rhs)?;
                    work_budget.charge_predicate_v2(&state.guard)?;
                    if writes.len() >= MAX_REFERENCE_STATEMENTS_V1 {
                        return Err(ReferenceBindingErrorV1::new(format!(
                            "reference symbolic execution exceeds {MAX_REFERENCE_STATEMENTS_V1} retained output writes",
                        )));
                    }
                    writes.push(ReferenceOutputWriteV1 {
                        argument,
                        block: block.block,
                        statement: assignment.statement,
                        coordinate,
                        guard: state.guard.clone(),
                        rhs,
                        value: assignment.value.clone(),
                    });
                    continue;
                }
                if !assignment.destination.projection.is_empty() {
                    return Err(ReferenceBindingErrorV1::new(format!(
                        "loop-carried reference assignment to _{} uses unsupported projection {:?}",
                        assignment.destination.local, assignment.destination.projection,
                    )));
                }
                let value = symbolic_value_v2(&state.environment, &assignment.value)?;
                require_symbolic_value_budget_v2(&value)?;
                work_budget.charge_v2(symbolic_value_nodes_v2(&value)?)?;
                state
                    .environment
                    .insert(assignment.destination.local, value);
            }
            match &block.terminator {
                ReferenceTerminatorV1::Return => {
                    completed_traces.extend(state.traces.into_values());
                }
                ReferenceTerminatorV1::Goto { target } => {
                    dispatch_symbolic_edge_v2(
                        &mut pending,
                        state,
                        block.block,
                        *target,
                        backedges,
                        &loop_nodes,
                        &mut work_budget,
                    )?;
                }
                ReferenceTerminatorV1::Assert {
                    condition,
                    expected,
                    success,
                    bounds_check,
                } => {
                    if *bounds_check {
                        dispatch_symbolic_edge_v2(
                            &mut pending,
                            state,
                            block.block,
                            *success,
                            backedges,
                            &loop_nodes,
                            &mut work_budget,
                        )?;
                        continue;
                    }
                    let expression =
                        symbolic_scalar_v2(symbolic_operand_v2(&state.environment, condition)?)?;
                    let actual = reference_constant_bits_v2(&expression)
                        .filter(|(scalar, _)| *scalar == ReferenceScalarTypeV1::Bool)
                        .map(|(_, bits)| bits != 0)
                        .ok_or_else(|| {
                            ReferenceBindingErrorV1::new(format!(
                                "loop-carried reference assertion in block {} is not statically proved; checked arithmetic requires a range fact",
                                block.block,
                            ))
                        })?;
                    if actual != *expected {
                        return Err(ReferenceBindingErrorV1::new(format!(
                            "loop-carried reference assertion in block {} is statically false",
                            block.block,
                        )));
                    }
                    dispatch_symbolic_edge_v2(
                        &mut pending,
                        state,
                        block.block,
                        *success,
                        backedges,
                        &loop_nodes,
                        &mut work_budget,
                    )?;
                }
                ReferenceTerminatorV1::Switch {
                    discriminant,
                    values,
                    otherwise,
                } => {
                    let expression =
                        symbolic_scalar_v2(symbolic_operand_v2(&state.environment, discriminant)?)?;
                    if loop_headers.contains(&block.block) {
                        for trace in state
                            .traces
                            .values_mut()
                            .filter(|trace| trace.header == block.block)
                        {
                            work_budget.charge_expression_v2(&expression)?;
                            trace.variants.push(expression.clone());
                        }
                    }
                    if let Some((_, bits)) = reference_constant_bits_v2(&expression) {
                        let target = values
                            .iter()
                            .find_map(|(value, target)| (*value == bits).then_some(*target))
                            .unwrap_or(*otherwise);
                        dispatch_symbolic_edge_v2(
                            &mut pending,
                            state,
                            block.block,
                            target,
                            backedges,
                            &loop_nodes,
                            &mut work_budget,
                        )?;
                    } else {
                        if loop_headers.contains(&block.block) {
                            if let Some(summary) = self.summarize_dynamic_counted_loop_v2(
                                block,
                                discriminant,
                                values,
                                *otherwise,
                                &state,
                                &loop_nodes,
                            )? {
                                state.environment.insert(
                                    summary.induction_local,
                                    ReferenceSymbolicValueV2::Scalar(
                                        summary.final_induction.clone(),
                                    ),
                                );
                                let trace = state
                                    .traces
                                    .get_mut(&(block.block, summary.latch))
                                    .ok_or_else(|| {
                                        ReferenceBindingErrorV1::new(
                                            "dynamic reference loop lost its compiler trace",
                                        )
                                    })?;
                                trace.exit = Some(summary.exit);
                                trace.exact_iterations = None;
                                trace.maximum_iterations = Some(summary.maximum_iterations);
                                trace.transitions.push(state.environment.clone());
                                trace.variants.push(expression);
                                state.block = summary.exit;
                                work_budget.charge_state_clone_v2(&state)?;
                                pending.push_back(state);
                                continue;
                            }
                            return Err(ReferenceBindingErrorV1::new(format!(
                                "dynamic reference loop at header {} has no authenticated finite maximum and canonical unit-step variant",
                                block.block,
                            )));
                        }
                        let mut by_target = BTreeMap::<u32, Vec<u128>>::new();
                        let mut all_values = Vec::with_capacity(values.len());
                        for (value, target) in values {
                            by_target.entry(*target).or_default().push(*value);
                            all_values.push(*value);
                        }
                        for (target, accepted) in by_target {
                            work_budget.charge_state_clone_v2(&state)?;
                            let mut branch = state.clone();
                            branch.guard = reference_predicate_and_atom_v1(
                                &branch.guard,
                                ReferenceGuardAtomV1::SwitchValueSet {
                                    discriminant: expression.clone(),
                                    values: accepted.into_boxed_slice(),
                                    inside_set: true,
                                },
                            )?;
                            work_budget.charge_predicate_v2(&branch.guard)?;
                            dispatch_symbolic_edge_v2(
                                &mut pending,
                                branch,
                                block.block,
                                target,
                                backedges,
                                &loop_nodes,
                                &mut work_budget,
                            )?;
                        }
                        state.guard = reference_predicate_and_atom_v1(
                            &state.guard,
                            ReferenceGuardAtomV1::SwitchValueSet {
                                discriminant: expression,
                                values: all_values.into_boxed_slice(),
                                inside_set: false,
                            },
                        )?;
                        work_budget.charge_predicate_v2(&state.guard)?;
                        dispatch_symbolic_edge_v2(
                            &mut pending,
                            state,
                            block.block,
                            *otherwise,
                            backedges,
                            &loop_nodes,
                            &mut work_budget,
                        )?;
                    }
                }
            }
        }
        if completed_traces.is_empty() {
            return Err(ReferenceBindingErrorV1::new(
                "bounded reference loop has no successful return path",
            ));
        }
        let mut summaries = completed_traces
            .iter()
            .map(reference_loop_summary_v2)
            .collect::<Result<Vec<_>, _>>()?;
        summaries.sort();
        summaries.dedup();
        writes.sort_by(|lhs, rhs| {
            (
                lhs.argument,
                lhs.block,
                lhs.statement,
                &lhs.coordinate,
                &lhs.guard,
                &lhs.rhs,
            )
                .cmp(&(
                    rhs.argument,
                    rhs.block,
                    rhs.statement,
                    &rhs.coordinate,
                    &rhs.guard,
                    &rhs.rhs,
                ))
        });
        writes.dedup_by(|lhs, rhs| {
            lhs.argument == rhs.argument
                && lhs.block == rhs.block
                && lhs.statement == rhs.statement
                && lhs.coordinate == rhs.coordinate
                && lhs.guard == rhs.guard
                && lhs.rhs == rhs.rhs
        });
        Ok((writes, summaries))
    }

    fn output_relation_for_local_v2(
        &self,
        local: u32,
    ) -> Result<Option<(u32, bool)>, ReferenceBindingErrorV1> {
        let point_count = self.point_coordinate_count_v1()?;
        Ok(self.relations.iter().find_map(|relation| {
            let (argument, coordinate_output) = match relation {
                ReferenceArgumentRelationV1::DisjointOutputSlice { argument, .. } => {
                    (*argument, false)
                }
                ReferenceArgumentRelationV1::DisjointOutputCoordinate { argument, .. } => {
                    (*argument, true)
                }
                _ => return None,
            };
            let reference_argument = point_count.checked_add(argument)?;
            (local == reference_argument + 1).then_some((argument, coordinate_output))
        }))
    }

    fn summarize_dynamic_counted_loop_v2(
        &self,
        header: &ReferenceBlockV1,
        discriminant: &ReferenceOperandV1,
        values: &[(u128, u32)],
        otherwise: u32,
        state: &ReferenceSymbolicStateV2,
        loop_nodes: &BTreeMap<(u32, u32), BTreeSet<u32>>,
    ) -> Result<Option<DynamicReferenceLoopSummaryV2>, ReferenceBindingErrorV1> {
        let (ReferenceOperandV1::Copy(discriminant) | ReferenceOperandV1::Move(discriminant)) =
            discriminant
        else {
            return Ok(None);
        };
        if !discriminant.projection.is_empty() {
            return Ok(None);
        }
        let assignments = header
            .assignments
            .iter()
            .filter(|assignment| {
                assignment.destination.local == discriminant.local
                    && assignment.destination.projection.is_empty()
            })
            .collect::<Vec<_>>();
        let [assignment] = assignments.as_slice() else {
            return Ok(None);
        };
        let ReferenceValueV1::Binary {
            operation: ReferenceBinaryOpV1::LessThan,
            lhs,
            rhs: bound,
            checked: false,
        } = &assignment.value
        else {
            return Ok(None);
        };
        let (ReferenceOperandV1::Copy(induction) | ReferenceOperandV1::Move(induction)) = lhs
        else {
            return Ok(None);
        };
        if !induction.projection.is_empty() {
            return Ok(None);
        }
        let mut induction = induction.clone();
        for _ in 0..fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 {
            let aliases = header
                .assignments
                .iter()
                .filter(|assignment| {
                    assignment.destination.local == induction.local
                        && assignment.destination.projection.is_empty()
                })
                .collect::<Vec<_>>();
            let [alias] = aliases.as_slice() else {
                break;
            };
            let ReferenceValueV1::Use(
                ReferenceOperandV1::Copy(source) | ReferenceOperandV1::Move(source),
            ) = &alias.value
            else {
                break;
            };
            if !source.projection.is_empty() || source.local == induction.local {
                break;
            }
            induction = source.clone();
        }
        let [(0, exit)] = values else {
            return Ok(None);
        };
        let mut matching_loops = loop_nodes.iter().filter(|((_, loop_header), nodes)| {
            *loop_header == header.block && nodes.contains(&otherwise) && !nodes.contains(exit)
        });
        let Some((&(latch, _), nodes)) = matching_loops.next() else {
            return Ok(None);
        };
        if matching_loops.next().is_some() {
            return Ok(None);
        }
        let initial = state.environment.get(&induction.local);
        let Some(ReferenceSymbolicValueV2::Scalar(initial)) = initial else {
            return Err(ReferenceBindingErrorV1::new(
                "dynamic counted loop has no scalar initial induction value",
            ));
        };
        let Some((initial_scalar, 0)) = reference_constant_bits_v2(initial) else {
            return Err(ReferenceBindingErrorV1::new(
                "dynamic counted loop induction is not initialized to unsigned zero",
            ));
        };
        let final_induction = symbolic_scalar_v2(symbolic_operand_v2(&state.environment, bound)?)?;
        let maximum_iterations = match &final_induction {
            ReferenceEffectExpressionV1::KernelScalarArgument { argument } => {
                let scalar = self.relations.iter().find_map(|relation| match relation {
                    ReferenceArgumentRelationV1::ScalarInput {
                        argument: actual,
                        scalar,
                    } if actual == argument => Some(*scalar),
                    _ => None,
                });
                let Some(scalar) = scalar else {
                    return Err(ReferenceBindingErrorV1::new(
                        "dynamic counted loop bound has no scalar ABI relation",
                    ));
                };
                let Some(maximum) = unsigned_reference_scalar_maximum_v2(scalar) else {
                    return Err(ReferenceBindingErrorV1::new(
                        "dynamic counted loop bound is not an unsigned finite machine scalar",
                    ));
                };
                if scalar != initial_scalar {
                    return Err(ReferenceBindingErrorV1::new(
                        "dynamic counted loop induction and bound types differ",
                    ));
                }
                maximum
            }
            _ => return Ok(None),
        };
        let mut induction_assignments = Vec::new();
        for node in nodes {
            for assignment in &self.blocks[*node as usize].assignments {
                if !assignment.destination.projection.is_empty() {
                    return Err(ReferenceBindingErrorV1::new(
                        "dynamic counted loop contains a projected memory effect",
                    ));
                }
                if state
                    .traces
                    .get(&(header.block, latch))
                    .is_some_and(|trace| trace.initial.contains_key(&assignment.destination.local))
                    && assignment.destination.local != induction.local
                {
                    return Err(ReferenceBindingErrorV1::new(
                        "dynamic counted loop mutates another loop-carried local",
                    ));
                }
                if assignment.destination.local == induction.local {
                    induction_assignments.push(assignment);
                }
            }
        }
        let [induction_assignment] = induction_assignments.as_slice() else {
            return Err(ReferenceBindingErrorV1::new(
                "dynamic counted loop does not have one induction assignment",
            ));
        };
        let ReferenceValueV1::Use(
            ReferenceOperandV1::Copy(increment_value) | ReferenceOperandV1::Move(increment_value),
        ) = &induction_assignment.value
        else {
            return Err(ReferenceBindingErrorV1::new(
                "dynamic counted loop induction is not assigned from checked arithmetic",
            ));
        };
        let [ReferencePlaceProjectionV1::Field(0)] = increment_value.projection.as_ref() else {
            return Err(ReferenceBindingErrorV1::new(
                "dynamic counted loop induction does not select the checked value field",
            ));
        };
        let increment_pair = increment_value.local;
        let increment = nodes
            .iter()
            .flat_map(|node| self.blocks[*node as usize].assignments.iter())
            .find(|assignment| {
                assignment.destination.local == increment_pair
                    && assignment.destination.projection.is_empty()
            });
        let Some(ReferenceAssignmentV1 {
            value:
                ReferenceValueV1::Binary {
                    operation: ReferenceBinaryOpV1::Add,
                    lhs: increment_lhs,
                    rhs: increment_rhs,
                    checked: true,
                },
            ..
        }) = increment
        else {
            return Err(ReferenceBindingErrorV1::new(
                "dynamic counted loop has no checked additive transition",
            ));
        };
        let increment_matches = [
            (increment_lhs, increment_rhs),
            (increment_rhs, increment_lhs),
        ]
        .into_iter()
        .any(|(candidate_induction, candidate_step)| {
            matches!(
                candidate_induction,
                ReferenceOperandV1::Copy(place) | ReferenceOperandV1::Move(place)
                    if place.local == induction.local && place.projection.is_empty()
            ) && matches!(
                candidate_step,
                ReferenceOperandV1::Constant(ReferenceConstantV1::Scalar { bits: 1, .. })
            )
        });
        if !increment_matches {
            return Err(ReferenceBindingErrorV1::new(
                "dynamic counted loop transition is not induction plus one",
            ));
        }
        let overflow_checked = nodes.iter().any(|node| {
            matches!(
                &self.blocks[*node as usize].terminator,
                ReferenceTerminatorV1::Assert {
                    condition: ReferenceOperandV1::Copy(place) | ReferenceOperandV1::Move(place),
                    expected: false,
                    ..
                } if place.local == increment_pair
                    && matches!(place.projection.as_ref(), [ReferencePlaceProjectionV1::Field(1)])
            )
        });
        if !overflow_checked {
            return Err(ReferenceBindingErrorV1::new(
                "dynamic counted loop does not retain its overflow assertion",
            ));
        }
        Ok(Some(DynamicReferenceLoopSummaryV2 {
            latch,
            exit: *exit,
            induction_local: induction.local,
            final_induction,
            maximum_iterations,
        }))
    }

    fn symbolic_output_coordinate_v2(
        &self,
        environment: &ReferenceSymbolicEnvironmentV2,
        destination: &ReferencePlaceV1,
        coordinate_output: bool,
    ) -> Result<ReferenceOutputCoordinateV1, ReferenceBindingErrorV1> {
        match destination.projection.as_ref() {
            [ReferencePlaceProjectionV1::Dereference] if coordinate_output => {
                let axes = self
                    .relations
                    .iter()
                    .filter_map(|relation| match relation {
                        ReferenceArgumentRelationV1::PointCoordinate { axis, .. } => {
                            Some(ReferenceEffectExpressionV1::PointCoordinate { axis: *axis })
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice();
                if axes.is_empty() {
                    Ok(ReferenceOutputCoordinateV1::SingleCoordinate)
                } else {
                    Ok(ReferenceOutputCoordinateV1::LogicalPoint(axes))
                }
            }
            [
                ReferencePlaceProjectionV1::Dereference,
                ReferencePlaceProjectionV1::Index(index),
            ] if !coordinate_output => Ok(ReferenceOutputCoordinateV1::Dynamic(
                symbolic_scalar_v2(environment.get(index).cloned().ok_or_else(|| {
                    ReferenceBindingErrorV1::new(format!(
                        "reference output index local _{index} has no loop-carried value",
                    ))
                })?)?,
            )),
            [
                ReferencePlaceProjectionV1::Dereference,
                ReferencePlaceProjectionV1::ConstantIndex {
                    offset,
                    minimum_length,
                    from_end,
                },
            ] if !coordinate_output => Ok(ReferenceOutputCoordinateV1::Constant {
                offset: *offset,
                minimum_length: *minimum_length,
                from_end: *from_end,
            }),
            projection => Err(ReferenceBindingErrorV1::new(format!(
                "observable loop output uses unsupported projection {projection:?}",
            ))),
        }
    }
}

struct DynamicReferenceLoopSummaryV2 {
    latch: u32,
    exit: u32,
    induction_local: u32,
    final_induction: ReferenceEffectExpressionV1,
    maximum_iterations: u64,
}

fn unsigned_reference_scalar_maximum_v2(scalar: ReferenceScalarTypeV1) -> Option<u64> {
    Some(match scalar {
        ReferenceScalarTypeV1::U8 => u64::from(u8::MAX),
        ReferenceScalarTypeV1::U16 => u64::from(u16::MAX),
        ReferenceScalarTypeV1::U32 => u64::from(u32::MAX),
        ReferenceScalarTypeV1::U64 | ReferenceScalarTypeV1::Usize => u64::MAX,
        _ => return None,
    })
}

fn symbolic_value_v2(
    environment: &ReferenceSymbolicEnvironmentV2,
    value: &ReferenceValueV1,
) -> Result<ReferenceSymbolicValueV2, ReferenceBindingErrorV1> {
    match value {
        ReferenceValueV1::Use(operand) => symbolic_operand_v2(environment, operand),
        ReferenceValueV1::Binary {
            operation,
            lhs,
            rhs,
            checked,
        } => {
            let lhs = symbolic_scalar_v2(symbolic_operand_v2(environment, lhs)?)?;
            let rhs = symbolic_scalar_v2(symbolic_operand_v2(environment, rhs)?)?;
            let expression = ReferenceEffectExpressionV1::Binary {
                operation: *operation,
                lhs: Box::new(lhs.clone()),
                rhs: Box::new(rhs.clone()),
                checked: *checked,
            };
            let folded = reference_fold_constant_v2(&expression).unwrap_or(expression);
            if *checked {
                Ok(ReferenceSymbolicValueV2::CheckedPair {
                    value: folded,
                    overflowed: reference_checked_overflow_v2(*operation, &lhs, &rhs),
                })
            } else {
                Ok(ReferenceSymbolicValueV2::Scalar(folded))
            }
        }
        ReferenceValueV1::Unary { operation, operand } => {
            let operand = symbolic_scalar_v2(symbolic_operand_v2(environment, operand)?)?;
            let expression = ReferenceEffectExpressionV1::Unary {
                operation: *operation,
                operand: Box::new(operand),
            };
            Ok(ReferenceSymbolicValueV2::Scalar(
                reference_fold_constant_v2(&expression).unwrap_or(expression),
            ))
        }
        ReferenceValueV1::Cast {
            kind,
            source,
            target,
            operand,
        } => {
            let operand = symbolic_scalar_v2(symbolic_operand_v2(environment, operand)?)?;
            Ok(ReferenceSymbolicValueV2::Scalar(
                ReferenceEffectExpressionV1::Cast {
                    kind: *kind,
                    source: *source,
                    target: *target,
                    operand: Box::new(operand),
                },
            ))
        }
        ReferenceValueV1::InputLength { reference_argument } => Ok(
            ReferenceSymbolicValueV2::Scalar(ReferenceEffectExpressionV1::InputLength {
                reference_argument: *reference_argument,
            }),
        ),
        ReferenceValueV1::SafeHelperCall {
            parameters,
            arguments,
            summary,
            ..
        } => {
            if parameters.len() != arguments.len() {
                return Err(ReferenceBindingErrorV1::new(
                    "authenticated helper summary argument count changed",
                ));
            }
            let arguments = arguments
                .iter()
                .map(|argument| symbolic_scalar_v2(symbolic_operand_v2(environment, argument)?))
                .collect::<Result<Vec<_>, _>>()?;
            let mut work = 0;
            let expression = substitute_helper_summary_v2(summary, &arguments, &mut work, 0)?;
            Ok(ReferenceSymbolicValueV2::Scalar(
                reference_fold_constant_v2(&expression).unwrap_or(expression),
            ))
        }
    }
}

fn symbolic_operand_v2(
    environment: &ReferenceSymbolicEnvironmentV2,
    operand: &ReferenceOperandV1,
) -> Result<ReferenceSymbolicValueV2, ReferenceBindingErrorV1> {
    match operand {
        ReferenceOperandV1::Constant(constant) => Ok(ReferenceSymbolicValueV2::Scalar(
            ReferenceEffectExpressionV1::Constant(constant.clone()),
        )),
        ReferenceOperandV1::Copy(place) | ReferenceOperandV1::Move(place)
            if place.projection.is_empty() =>
        {
            environment.get(&place.local).cloned().ok_or_else(|| {
                ReferenceBindingErrorV1::new(format!(
                    "loop-carried reference local _{} has no symbolic value",
                    place.local,
                ))
            })
        }
        ReferenceOperandV1::Copy(place) | ReferenceOperandV1::Move(place)
            if matches!(
                place.projection.as_ref(),
                [ReferencePlaceProjectionV1::Field(0)]
            ) =>
        {
            match environment.get(&place.local) {
                Some(ReferenceSymbolicValueV2::CheckedPair { value, .. }) => {
                    Ok(ReferenceSymbolicValueV2::Scalar(value.clone()))
                }
                _ => Err(ReferenceBindingErrorV1::new(format!(
                    "reference _{}.0 is not one checked scalar value",
                    place.local,
                ))),
            }
        }
        ReferenceOperandV1::Copy(place) | ReferenceOperandV1::Move(place)
            if matches!(
                place.projection.as_ref(),
                [
                    ReferencePlaceProjectionV1::Dereference,
                    ReferencePlaceProjectionV1::Index(_)
                ]
            ) =>
        {
            let [
                ReferencePlaceProjectionV1::Dereference,
                ReferencePlaceProjectionV1::Index(index),
            ] = place.projection.as_ref()
            else {
                unreachable!()
            };
            let reference_argument = place.local.checked_sub(1).ok_or_else(|| {
                ReferenceBindingErrorV1::new(
                    "safe reference load uses the return-place local as its base",
                )
            })?;
            let index = environment.get(index).cloned().ok_or_else(|| {
                ReferenceBindingErrorV1::new(format!(
                    "safe reference load index _{index} has no symbolic value",
                ))
            })?;
            Ok(ReferenceSymbolicValueV2::Scalar(
                ReferenceEffectExpressionV1::InputLoad {
                    reference_argument,
                    index: Box::new(symbolic_scalar_v2(index)?),
                },
            ))
        }
        ReferenceOperandV1::Copy(place) | ReferenceOperandV1::Move(place)
            if matches!(
                place.projection.as_ref(),
                [ReferencePlaceProjectionV1::Field(1)]
            ) =>
        {
            match environment.get(&place.local) {
                Some(ReferenceSymbolicValueV2::CheckedPair {
                    overflowed: Some(overflowed),
                    ..
                }) => Ok(ReferenceSymbolicValueV2::Scalar(
                    ReferenceEffectExpressionV1::Constant(ReferenceConstantV1::Scalar {
                        scalar: ReferenceScalarTypeV1::Bool,
                        bits: u128::from(*overflowed),
                    }),
                )),
                Some(ReferenceSymbolicValueV2::CheckedPair {
                    overflowed: None, ..
                }) => Err(ReferenceBindingErrorV1::new(
                    "checked loop arithmetic overflow is not statically proved; provide an authenticated range fact",
                )),
                _ => Err(ReferenceBindingErrorV1::new(format!(
                    "reference _{}.1 is not one checked scalar overflow flag",
                    place.local,
                ))),
            }
        }
        ReferenceOperandV1::Copy(place) | ReferenceOperandV1::Move(place) => {
            Err(ReferenceBindingErrorV1::new(format!(
                "reference scalar expression reads projection {:?}; slice and pointer reads require an independently bound GPU load symbol",
                place.projection,
            )))
        }
    }
}

fn symbolic_scalar_v2(
    value: ReferenceSymbolicValueV2,
) -> Result<ReferenceEffectExpressionV1, ReferenceBindingErrorV1> {
    match value {
        ReferenceSymbolicValueV2::Scalar(expression) => Ok(expression),
        ReferenceSymbolicValueV2::CheckedPair { .. } => Err(ReferenceBindingErrorV1::new(
            "checked arithmetic pair is used without selecting its value or overflow field",
        )),
    }
}

fn require_symbolic_value_budget_v2(
    value: &ReferenceSymbolicValueV2,
) -> Result<(), ReferenceBindingErrorV1> {
    match value {
        ReferenceSymbolicValueV2::Scalar(expression)
        | ReferenceSymbolicValueV2::CheckedPair {
            value: expression, ..
        } => require_symbolic_expression_budget_v2(expression),
    }
}

fn require_symbolic_expression_budget_v2(
    expression: &ReferenceEffectExpressionV1,
) -> Result<(), ReferenceBindingErrorV1> {
    symbolic_expression_nodes_v2(expression).map(|_| ())
}

fn symbolic_expression_nodes_v2(
    expression: &ReferenceEffectExpressionV1,
) -> Result<usize, ReferenceBindingErrorV1> {
    let mut pending = vec![(expression, 0_usize)];
    let mut nodes = 0_usize;
    while let Some((expression, depth)) = pending.pop() {
        if depth > fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 {
            return Err(ReferenceBindingErrorV1::new(format!(
                "reference symbolic expression exceeds depth {}",
                fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2,
            )));
        }
        nodes = nodes.checked_add(1).ok_or_else(|| {
            ReferenceBindingErrorV1::new("reference symbolic expression node count overflowed")
        })?;
        if nodes > MAX_REFERENCE_EXPRESSION_NODES_V1 {
            return Err(ReferenceBindingErrorV1::new(format!(
                "reference symbolic expression exceeds {MAX_REFERENCE_EXPRESSION_NODES_V1} nodes",
            )));
        }
        match expression {
            ReferenceEffectExpressionV1::Binary { lhs, rhs, .. } => {
                pending.push((lhs, depth + 1));
                pending.push((rhs, depth + 1));
            }
            ReferenceEffectExpressionV1::Unary { operand, .. }
            | ReferenceEffectExpressionV1::Cast { operand, .. }
            | ReferenceEffectExpressionV1::InputLoad { index: operand, .. } => {
                pending.push((operand, depth + 1));
            }
            ReferenceEffectExpressionV1::PointCoordinate { .. }
            | ReferenceEffectExpressionV1::KernelScalarArgument { .. }
            | ReferenceEffectExpressionV1::InputLength { .. }
            | ReferenceEffectExpressionV1::Constant(_) => {}
        }
    }
    Ok(nodes)
}

fn symbolic_value_nodes_v2(
    value: &ReferenceSymbolicValueV2,
) -> Result<usize, ReferenceBindingErrorV1> {
    match value {
        ReferenceSymbolicValueV2::Scalar(expression)
        | ReferenceSymbolicValueV2::CheckedPair {
            value: expression, ..
        } => symbolic_expression_nodes_v2(expression),
    }
}

fn symbolic_environment_nodes_v2(
    environment: &ReferenceSymbolicEnvironmentV2,
) -> Result<usize, ReferenceBindingErrorV1> {
    environment.values().try_fold(0_usize, |total, value| {
        total
            .checked_add(symbolic_value_nodes_v2(value)?)
            .ok_or_else(|| {
                ReferenceBindingErrorV1::new("reference symbolic environment node count overflowed")
            })
    })
}

fn symbolic_predicate_nodes_v2(
    predicate: &ReferencePathPredicateV1,
) -> Result<usize, ReferenceBindingErrorV1> {
    predicate.clauses.iter().try_fold(0_usize, |total, clause| {
        clause.atoms.iter().try_fold(total, |total, atom| {
            let expression = match atom {
                ReferenceGuardAtomV1::SwitchValueSet { discriminant, .. } => discriminant,
                ReferenceGuardAtomV1::Assert { condition, .. } => condition,
            };
            total
                .checked_add(symbolic_expression_nodes_v2(expression)?)
                .ok_or_else(|| {
                    ReferenceBindingErrorV1::new(
                        "reference symbolic predicate node count overflowed",
                    )
                })
        })
    })
}

fn symbolic_trace_nodes_v2(trace: &ReferenceLoopTraceV2) -> Result<usize, ReferenceBindingErrorV1> {
    let mut nodes = symbolic_environment_nodes_v2(&trace.initial)?;
    for environment in &trace.transitions {
        nodes = nodes
            .checked_add(symbolic_environment_nodes_v2(environment)?)
            .ok_or_else(|| {
                ReferenceBindingErrorV1::new("reference symbolic trace node count overflowed")
            })?;
    }
    for expression in &trace.variants {
        nodes = nodes
            .checked_add(symbolic_expression_nodes_v2(expression)?)
            .ok_or_else(|| {
                ReferenceBindingErrorV1::new("reference symbolic trace node count overflowed")
            })?;
    }
    Ok(nodes)
}

fn symbolic_state_nodes_v2(
    state: &ReferenceSymbolicStateV2,
) -> Result<usize, ReferenceBindingErrorV1> {
    let mut nodes = symbolic_environment_nodes_v2(&state.environment)?
        .checked_add(symbolic_predicate_nodes_v2(&state.guard)?)
        .ok_or_else(|| {
            ReferenceBindingErrorV1::new("reference symbolic state node count overflowed")
        })?;
    for trace in state.traces.values() {
        nodes = nodes
            .checked_add(symbolic_trace_nodes_v2(trace)?)
            .ok_or_else(|| {
                ReferenceBindingErrorV1::new("reference symbolic state node count overflowed")
            })?;
    }
    Ok(nodes)
}

fn dispatch_symbolic_edge_v2(
    pending: &mut VecDeque<ReferenceSymbolicStateV2>,
    mut state: ReferenceSymbolicStateV2,
    source: u32,
    target: u32,
    backedges: &BTreeSet<(u32, u32)>,
    loop_nodes: &BTreeMap<(u32, u32), BTreeSet<u32>>,
    work_budget: &mut ReferenceSymbolicWorkBudgetV2,
) -> Result<(), ReferenceBindingErrorV1> {
    if backedges.contains(&(source, target)) {
        let trace = state.traces.get_mut(&(target, source)).ok_or_else(|| {
            ReferenceBindingErrorV1::new(format!(
                "reference backedge {source}->{target} has no canonical loop trace",
            ))
        })?;
        work_budget.charge_environment_v2(&state.environment)?;
        trace.transitions.push(state.environment.clone());
        if trace.transitions.len() > MAX_REFERENCE_LOOP_ITERATIONS_V2 {
            return Err(ReferenceBindingErrorV1::new(format!(
                "reference loop <header={target}, latch={source}> exceeds {MAX_REFERENCE_LOOP_ITERATIONS_V2} iterations",
            )));
        }
    }
    if let Some(trace) = state
        .traces
        .values_mut()
        .find(|trace| trace.header == source)
    {
        let nodes = loop_nodes
            .get(&(trace.latch, trace.header))
            .ok_or_else(|| ReferenceBindingErrorV1::new("reference natural-loop nodes vanished"))?;
        if !nodes.contains(&target) {
            trace.exit = Some(target);
        }
    }
    state.block = target;
    pending.push_back(state);
    Ok(())
}

fn reference_cfg_backedges_v2(
    effect_ir: &ReferenceEffectIrV1,
) -> Result<BTreeSet<(u32, u32)>, ReferenceBindingErrorV1> {
    let count = effect_ir.blocks.len();
    if count == 0 {
        return Err(ReferenceBindingErrorV1::new(
            "reference effect IR has no entry block",
        ));
    }
    let all = (0..count).collect::<BTreeSet<_>>();
    let mut dominators = vec![all.clone(); count];
    dominators[0] = BTreeSet::from([0]);
    let mut predecessors = vec![Vec::new(); count];
    for block in &effect_ir.blocks {
        for successor in reference_successors_v1(&block.terminator) {
            let successor = successor as usize;
            if successor >= count {
                return Err(ReferenceBindingErrorV1::new(
                    "reference CFG successor is outside the block table",
                ));
            }
            predecessors[successor].push(block.block as usize);
        }
    }
    loop {
        let mut changed = false;
        for block in 1..count {
            let mut next = if let Some(first) = predecessors[block].first() {
                dominators[*first].clone()
            } else {
                BTreeSet::new()
            };
            for predecessor in predecessors[block].iter().skip(1) {
                next = next
                    .intersection(&dominators[*predecessor])
                    .copied()
                    .collect();
            }
            next.insert(block);
            if next != dominators[block] {
                dominators[block] = next;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let mut backedges = BTreeSet::new();
    for block in &effect_ir.blocks {
        for successor in reference_successors_v1(&block.terminator) {
            if dominators[block.block as usize].contains(&(successor as usize)) {
                backedges.insert((block.block, successor));
            }
        }
    }
    Ok(backedges)
}

fn reference_natural_loop_nodes_v2(
    effect_ir: &ReferenceEffectIrV1,
    backedges: &BTreeSet<(u32, u32)>,
) -> Result<BTreeMap<(u32, u32), BTreeSet<u32>>, ReferenceBindingErrorV1> {
    let mut predecessors = vec![Vec::new(); effect_ir.blocks.len()];
    for block in &effect_ir.blocks {
        for successor in reference_successors_v1(&block.terminator) {
            predecessors
                .get_mut(successor as usize)
                .ok_or_else(|| {
                    ReferenceBindingErrorV1::new(
                        "reference loop successor is outside the block table",
                    )
                })?
                .push(block.block);
        }
    }
    let mut result = BTreeMap::new();
    for (latch, header) in backedges {
        let mut nodes = BTreeSet::from([*header, *latch]);
        let mut pending = vec![*latch];
        while let Some(block) = pending.pop() {
            for predecessor in &predecessors[block as usize] {
                if nodes.insert(*predecessor) && *predecessor != *header {
                    pending.push(*predecessor);
                }
            }
        }
        result.insert((*latch, *header), nodes);
    }
    Ok(result)
}

fn validate_reference_loop_shapes_v2(
    effect_ir: &ReferenceEffectIrV1,
    backedges: &BTreeSet<(u32, u32)>,
) -> Result<(), ReferenceBindingErrorV1> {
    let mut headers = BTreeSet::new();
    let loop_nodes = reference_natural_loop_nodes_v2(effect_ir, backedges)?;
    reject_overlapping_reference_loops_v2(&loop_nodes)?;
    for (latch, header) in backedges {
        if !headers.insert(*header) {
            return Err(ReferenceBindingErrorV1::new(format!(
                "reference loop header {header} has multiple latches; only one canonical recurrence is supported",
            )));
        }
        let header_block = effect_ir.blocks.get(*header as usize).ok_or_else(|| {
            ReferenceBindingErrorV1::new("reference loop header is outside the block table")
        })?;
        if !matches!(
            header_block.terminator,
            ReferenceTerminatorV1::Switch { .. }
        ) {
            return Err(ReferenceBindingErrorV1::new(format!(
                "reference loop header {header} does not end in one canonical condition switch",
            )));
        }
        let latch_block = effect_ir.blocks.get(*latch as usize).ok_or_else(|| {
            ReferenceBindingErrorV1::new("reference loop latch is outside the block table")
        })?;
        if !matches!(
            latch_block.terminator,
            ReferenceTerminatorV1::Goto { target } if target == *header
        ) {
            return Err(ReferenceBindingErrorV1::new(format!(
                "reference loop latch {latch} is not one unconditional edge to header {header}",
            )));
        }
        let nodes = &loop_nodes[&(*latch, *header)];
        let exits = nodes
            .iter()
            .flat_map(|node| {
                reference_successors_v1(&effect_ir.blocks[*node as usize].terminator)
                    .into_iter()
                    .filter(|target| !nodes.contains(target))
                    .map(|target| (*node, target))
            })
            .collect::<BTreeSet<_>>();
        let Some((exit_source, _)) = exits.iter().next().copied() else {
            return Err(ReferenceBindingErrorV1::new(format!(
                "reference loop <header={header}, latch={latch}> has no finite exit",
            )));
        };
        if exits.len() != 1 || exit_source != *header {
            return Err(ReferenceBindingErrorV1::new(format!(
                "reference loop <header={header}, latch={latch}> must have exactly one header exit",
            )));
        }
    }
    Ok(())
}

fn reject_overlapping_reference_loops_v2(
    loop_nodes: &BTreeMap<(u32, u32), BTreeSet<u32>>,
) -> Result<(), ReferenceBindingErrorV1> {
    let loops = loop_nodes.iter().collect::<Vec<_>>();
    for (left_index, (left_identity, left_nodes)) in loops.iter().enumerate() {
        for (right_identity, right_nodes) in loops.iter().skip(left_index + 1) {
            if !left_nodes.is_disjoint(right_nodes) {
                return Err(ReferenceBindingErrorV1::new(format!(
                    "reference loops <header={}, latch={}> and <header={}, latch={}> overlap or nest; activation-specific recurrence summaries are not implemented",
                    left_identity.1, left_identity.0, right_identity.1, right_identity.0,
                )));
            }
        }
    }
    Ok(())
}

fn reference_loop_summary_v2(
    trace: &ReferenceLoopTraceV2,
) -> Result<ReferenceLoopSummaryV2, ReferenceBindingErrorV1> {
    let exit = trace.exit.ok_or_else(|| {
        ReferenceBindingErrorV1::new(format!(
            "reference loop <header={}, latch={}> has no authenticated exit",
            trace.header, trace.latch,
        ))
    })?;
    let mut carried_locals = trace
        .initial
        .iter()
        .filter_map(|(local, initial)| {
            trace
                .transitions
                .iter()
                .filter_map(|transition| transition.get(local))
                .any(|next| next != initial)
                .then_some(*local)
        })
        .collect::<Vec<_>>();
    carried_locals.sort_unstable();
    let mut initial_digest = Sha256::new();
    initial_digest.update(b"fe2o3/reference-loop-initial/v2\0");
    let mut transition_digest = Sha256::new();
    transition_digest.update(b"fe2o3/reference-loop-transition/v2\0");
    for local in &carried_locals {
        initial_digest.update(local.to_le_bytes());
        digest_symbolic_value_v2(
            &mut initial_digest,
            trace.initial.get(local).ok_or_else(|| {
                ReferenceBindingErrorV1::new("reference carried local has no initial value")
            })?,
        );
        transition_digest.update(local.to_le_bytes());
        for transition in &trace.transitions {
            digest_symbolic_value_v2(
                &mut transition_digest,
                transition.get(local).ok_or_else(|| {
                    ReferenceBindingErrorV1::new("reference carried local has no transition value")
                })?,
            );
        }
    }
    let mut variant_digest = Sha256::new();
    variant_digest.update(b"fe2o3/reference-loop-variant/v2\0");
    for variant in &trace.variants {
        digest_effect_expression_v1(&mut variant_digest, variant);
    }
    let iterations = u64::try_from(trace.transitions.len())
        .map_err(|_| ReferenceBindingErrorV1::new("reference loop iteration count exceeds u64"))?;
    let exact_iterations = if trace.maximum_iterations.is_some() {
        trace.exact_iterations
    } else {
        Some(iterations)
    };
    let maximum_iterations = trace.maximum_iterations.unwrap_or(iterations);
    Ok(ReferenceLoopSummaryV2 {
        header: trace.header,
        latch: trace.latch,
        exit,
        exact_iterations,
        maximum_iterations,
        carried_locals: carried_locals.into_boxed_slice(),
        initial_state_sha256: initial_digest.finalize().into(),
        transition_sha256: transition_digest.finalize().into(),
        variant_sha256: variant_digest.finalize().into(),
    })
}

fn digest_symbolic_value_v2(digest: &mut Sha256, value: &ReferenceSymbolicValueV2) {
    match value {
        ReferenceSymbolicValueV2::Scalar(expression) => {
            digest.update([0]);
            digest_effect_expression_v1(digest, expression);
        }
        ReferenceSymbolicValueV2::CheckedPair { value, overflowed } => {
            digest.update([1, overflowed.map(u8::from).unwrap_or(2)]);
            digest_effect_expression_v1(digest, value);
        }
    }
}

fn reference_constant_bits_v2(
    expression: &ReferenceEffectExpressionV1,
) -> Option<(ReferenceScalarTypeV1, u128)> {
    let folded = reference_fold_constant_v2(expression)?;
    let ReferenceEffectExpressionV1::Constant(ReferenceConstantV1::Scalar { scalar, bits }) =
        folded
    else {
        return None;
    };
    Some((scalar, bits))
}

fn reference_fold_constant_v2(
    expression: &ReferenceEffectExpressionV1,
) -> Option<ReferenceEffectExpressionV1> {
    match expression {
        ReferenceEffectExpressionV1::Constant(_) => Some(expression.clone()),
        ReferenceEffectExpressionV1::Binary {
            operation,
            lhs,
            rhs,
            ..
        } => {
            let (lhs_scalar, lhs) = reference_constant_bits_v2(lhs)?;
            let (rhs_scalar, rhs) = reference_constant_bits_v2(rhs)?;
            if lhs_scalar != rhs_scalar {
                return None;
            }
            if !matches!(
                lhs_scalar,
                ReferenceScalarTypeV1::Bool
                    | ReferenceScalarTypeV1::U8
                    | ReferenceScalarTypeV1::U16
                    | ReferenceScalarTypeV1::U32
                    | ReferenceScalarTypeV1::U64
                    | ReferenceScalarTypeV1::Usize
            ) {
                return None;
            }
            let comparison = matches!(
                operation,
                ReferenceBinaryOpV1::Equal
                    | ReferenceBinaryOpV1::LessThan
                    | ReferenceBinaryOpV1::LessEqual
                    | ReferenceBinaryOpV1::NotEqual
                    | ReferenceBinaryOpV1::GreaterEqual
                    | ReferenceBinaryOpV1::GreaterThan
            );
            let bits = match operation {
                ReferenceBinaryOpV1::Add => lhs.wrapping_add(rhs),
                ReferenceBinaryOpV1::Subtract => lhs.wrapping_sub(rhs),
                ReferenceBinaryOpV1::Multiply => lhs.wrapping_mul(rhs),
                ReferenceBinaryOpV1::Divide if rhs != 0 => lhs / rhs,
                ReferenceBinaryOpV1::Remainder if rhs != 0 => lhs % rhs,
                ReferenceBinaryOpV1::BitXor => lhs ^ rhs,
                ReferenceBinaryOpV1::BitAnd => lhs & rhs,
                ReferenceBinaryOpV1::BitOr => lhs | rhs,
                ReferenceBinaryOpV1::ShiftLeft if rhs < 128 => lhs << rhs,
                ReferenceBinaryOpV1::ShiftRight if rhs < 128 => lhs >> rhs,
                ReferenceBinaryOpV1::Equal => u128::from(lhs == rhs),
                ReferenceBinaryOpV1::LessThan => u128::from(lhs < rhs),
                ReferenceBinaryOpV1::LessEqual => u128::from(lhs <= rhs),
                ReferenceBinaryOpV1::NotEqual => u128::from(lhs != rhs),
                ReferenceBinaryOpV1::GreaterEqual => u128::from(lhs >= rhs),
                ReferenceBinaryOpV1::GreaterThan => u128::from(lhs > rhs),
                _ => return None,
            };
            let scalar = if comparison {
                ReferenceScalarTypeV1::Bool
            } else {
                lhs_scalar
            };
            let bits = bits & reference_scalar_mask_v2(scalar)?;
            Some(ReferenceEffectExpressionV1::Constant(
                ReferenceConstantV1::Scalar { scalar, bits },
            ))
        }
        ReferenceEffectExpressionV1::Unary { operation, operand } => {
            let (scalar, bits) = reference_constant_bits_v2(operand)?;
            let mask = reference_scalar_mask_v2(scalar)?;
            let bits = match operation {
                ReferenceUnaryOpV1::Not => (!bits) & mask,
                ReferenceUnaryOpV1::Negate => (!bits).wrapping_add(1) & mask,
            };
            Some(ReferenceEffectExpressionV1::Constant(
                ReferenceConstantV1::Scalar { scalar, bits },
            ))
        }
        ReferenceEffectExpressionV1::PointCoordinate { .. }
        | ReferenceEffectExpressionV1::KernelScalarArgument { .. }
        | ReferenceEffectExpressionV1::InputLoad { .. }
        | ReferenceEffectExpressionV1::InputLength { .. }
        | ReferenceEffectExpressionV1::Cast { .. } => None,
    }
}

fn reference_checked_overflow_v2(
    operation: ReferenceBinaryOpV1,
    lhs: &ReferenceEffectExpressionV1,
    rhs: &ReferenceEffectExpressionV1,
) -> Option<bool> {
    let (lhs_scalar, lhs) = reference_constant_bits_v2(lhs)?;
    let (rhs_scalar, rhs) = reference_constant_bits_v2(rhs)?;
    if lhs_scalar != rhs_scalar {
        return None;
    }
    if !matches!(
        lhs_scalar,
        ReferenceScalarTypeV1::U8
            | ReferenceScalarTypeV1::U16
            | ReferenceScalarTypeV1::U32
            | ReferenceScalarTypeV1::U64
            | ReferenceScalarTypeV1::Usize
    ) {
        return None;
    }
    let mask = reference_scalar_mask_v2(lhs_scalar)?;
    match operation {
        ReferenceBinaryOpV1::Add => Some(lhs.checked_add(rhs).is_none_or(|value| value > mask)),
        ReferenceBinaryOpV1::Subtract => Some(lhs < rhs),
        ReferenceBinaryOpV1::Multiply => {
            Some(lhs.checked_mul(rhs).is_none_or(|value| value > mask))
        }
        _ => None,
    }
}

fn reference_scalar_mask_v2(scalar: ReferenceScalarTypeV1) -> Option<u128> {
    let bits = match scalar {
        ReferenceScalarTypeV1::Bool => 1,
        ReferenceScalarTypeV1::U8 | ReferenceScalarTypeV1::I8 => 8,
        ReferenceScalarTypeV1::U16 | ReferenceScalarTypeV1::I16 => 16,
        ReferenceScalarTypeV1::U32 | ReferenceScalarTypeV1::I32 | ReferenceScalarTypeV1::F32 => 32,
        ReferenceScalarTypeV1::U64
        | ReferenceScalarTypeV1::Usize
        | ReferenceScalarTypeV1::I64
        | ReferenceScalarTypeV1::Isize
        | ReferenceScalarTypeV1::F64 => 64,
    };
    Some((1_u128 << bits) - 1)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedReferenceEffectBindingV1 {
    pub(crate) registration_path: String,
    pub(crate) logical_kernel_name: String,
    pub(crate) kernel: ReferenceFunctionIdentityV1,
    pub(crate) reference: ReferenceFunctionIdentityV1,
    pub(crate) effect_ir_sha256: [u8; 32],
    pub(crate) effect_ir: ReferenceEffectIrV1,
    pub(crate) observable_output_writes: Box<[ReferenceOutputWriteV1]>,
}

#[derive(Debug, Default)]
pub(crate) struct AuthenticatedReferenceEffectBindingsV1 {
    bindings: Box<[AuthenticatedReferenceEffectBindingV1]>,
}

impl AuthenticatedReferenceEffectBindingsV1 {
    pub(crate) fn new(bindings: Vec<AuthenticatedReferenceEffectBindingV1>) -> Self {
        Self {
            bindings: bindings.into_boxed_slice(),
        }
    }

    pub(crate) fn as_slice(&self) -> &[AuthenticatedReferenceEffectBindingV1] {
        &self.bindings
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReferenceBindingErrorV1(String);

impl ReferenceBindingErrorV1 {
    fn new(reason: impl Into<String>) -> Self {
        Self(reason.into())
    }

    fn at(tcx: TyCtxt<'_>, body: &Body<'_>, block: usize, reason: impl fmt::Display) -> Self {
        let span = body.basic_blocks[rustc_middle::mir::BasicBlock::from_usize(block)]
            .terminator()
            .source_info
            .span;
        Self::new(format!(
            "unsupported safe Rust reference MIR at {}: {reason}",
            tcx.sess.source_map().span_to_diagnostic_string(span),
        ))
    }
}

impl fmt::Display for ReferenceBindingErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ReferenceBindingErrorV1 {}

pub(crate) fn authenticate_reference_binding_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    registration_path: String,
    logical_kernel_name: String,
    kernel: Instance<'tcx>,
    reference: Instance<'tcx>,
) -> Result<AuthenticatedReferenceEffectBindingV1, ReferenceBindingErrorV1> {
    authenticate_safe_local_reference(tcx, reference)?;
    let relations = logical_abi_relation_v1(tcx, kernel, reference)?;
    let effect_ir = lower_reference_effect_ir_v1(tcx, reference, relations)?;
    let effect_ir_sha256 = effect_ir.canonical_sha256_v1();
    let observable_output_writes = effect_ir.observable_output_effects.clone();
    if effect_ir.relations.iter().any(|relation| {
        matches!(
            relation,
            ReferenceArgumentRelationV1::DisjointOutputSlice { .. }
                | ReferenceArgumentRelationV1::DisjointOutputCoordinate { .. }
        )
    }) && observable_output_writes.is_empty()
    {
        return Err(ReferenceBindingErrorV1::new(
            "safe Rust reference has a logical output but reference-effect V1 found no observable output write",
        ));
    }
    Ok(AuthenticatedReferenceEffectBindingV1 {
        registration_path,
        logical_kernel_name,
        kernel: function_identity_v1(tcx, kernel),
        reference: function_identity_v1(tcx, reference),
        effect_ir_sha256,
        effect_ir,
        observable_output_writes,
    })
}

fn function_identity_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> ReferenceFunctionIdentityV1 {
    let identities = canonical_function_identities_v1(tcx, instance);
    ReferenceFunctionIdentityV1 {
        def_path_hash: tcx.def_path_hash(instance.def_id()).0.to_le_bytes(),
        function_sha256: *identities.function().as_bytes(),
        item_definition_sha256: *identities.item_definition().as_bytes(),
        monomorphization_sha256: *identities.monomorphization().as_bytes(),
        generic_type_arguments_sha256: *identities.generic_type_arguments().as_bytes(),
        const_generic_arguments_sha256: *identities.const_generic_arguments().as_bytes(),
        rustc_mir_body_sha256: rustc_mir_body_sha256_v1(tcx, instance),
    }
}

#[derive(Default)]
struct UnsafeBlockVisitorV1 {
    first_span: Option<rustc_span::Span>,
}

impl<'tcx> Visitor<'tcx> for UnsafeBlockVisitorV1 {
    fn visit_block(&mut self, block: &'tcx rustc_hir::Block<'tcx>) {
        if matches!(
            block.rules,
            BlockCheckMode::UnsafeBlock(UnsafeSource::UserProvided)
        ) {
            self.first_span.get_or_insert(block.span);
        }
        intravisit::walk_block(self, block);
    }

    fn visit_expr(&mut self, expression: &'tcx rustc_hir::Expr<'tcx>) {
        if !matches!(expression.kind, ExprKind::Closure(_)) {
            intravisit::walk_expr(self, expression);
        }
    }
}

fn authenticate_safe_local_reference<'tcx>(
    tcx: TyCtxt<'tcx>,
    reference: Instance<'tcx>,
) -> Result<(), ReferenceBindingErrorV1> {
    let signature = instantiated_signature(tcx, reference);
    if signature.safety != Safety::Safe {
        return Err(ReferenceBindingErrorV1::new(format!(
            "safe Rust reference '{}' is declared unsafe",
            tcx.def_path_str(reference.def_id()),
        )));
    }
    if signature.abi != ExternAbi::Rust || signature.c_variadic {
        return Err(ReferenceBindingErrorV1::new(format!(
            "safe Rust reference '{}' must use the non-variadic Rust ABI",
            tcx.def_path_str(reference.def_id()),
        )));
    }
    let Some(local) = reference.def_id().as_local() else {
        return Err(ReferenceBindingErrorV1::new(format!(
            "safe Rust reference '{}' must be local so unsafe-block absence is authenticated",
            tcx.def_path_str(reference.def_id()),
        )));
    };
    let Some(body) = tcx.hir_maybe_body_owned_by(local) else {
        return Err(ReferenceBindingErrorV1::new(
            "safe Rust reference has no local HIR body",
        ));
    };
    let mut visitor = UnsafeBlockVisitorV1::default();
    visitor.visit_body(body);
    if let Some(span) = visitor.first_span {
        return Err(ReferenceBindingErrorV1::new(format!(
            "safe Rust reference '{}' contains a user-provided unsafe block at {}",
            tcx.def_path_str(reference.def_id()),
            tcx.sess.source_map().span_to_diagnostic_string(span),
        )));
    }
    Ok(())
}

fn instantiated_signature<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> rustc_middle::ty::FnSig<'tcx> {
    tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    )
}

fn logical_abi_relation_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    kernel: Instance<'tcx>,
    reference: Instance<'tcx>,
) -> Result<Vec<ReferenceArgumentRelationV1>, ReferenceBindingErrorV1> {
    let kernel_signature = instantiated_signature(tcx, kernel);
    let reference_signature = instantiated_signature(tcx, reference);
    if reference_signature.output() != tcx.types.unit {
        return Err(ReferenceBindingErrorV1::new(
            "safe Rust reference must return unit in V1; use explicit mutable outputs",
        ));
    }
    if reference_signature.inputs().len() < kernel_signature.inputs().len() {
        return Err(ReferenceBindingErrorV1::new(format!(
            "safe Rust reference logical ABI has {} arguments but kernel has {}; a point reference may only add leading usize coordinate arguments",
            reference_signature.inputs().len(),
            kernel_signature.inputs().len(),
        )));
    }
    let point_axis_count = reference_signature.inputs().len() - kernel_signature.inputs().len();
    if point_axis_count > MAX_REFERENCE_POINT_AXES_V1 {
        return Err(ReferenceBindingErrorV1::new(format!(
            "safe Rust point reference has {point_axis_count} coordinate axes; maximum is {MAX_REFERENCE_POINT_AXES_V1}",
        )));
    }
    let mut relations = Vec::with_capacity(reference_signature.inputs().len());
    for (axis, reference_ty) in reference_signature
        .inputs()
        .iter()
        .copied()
        .take(point_axis_count)
        .enumerate()
    {
        if !matches!(
            reference_ty.kind(),
            TyKind::Uint(rustc_middle::ty::UintTy::Usize)
        ) {
            return Err(ReferenceBindingErrorV1::new(format!(
                "safe Rust point-reference coordinate argument {} must be usize, found '{reference_ty}'",
                axis + 1,
            )));
        }
        relations.push(ReferenceArgumentRelationV1::PointCoordinate {
            reference_argument: u32::try_from(axis).map_err(|_| {
                ReferenceBindingErrorV1::new("reference coordinate argument exceeds u32")
            })?,
            axis: u32::try_from(axis).map_err(|_| {
                ReferenceBindingErrorV1::new("reference coordinate axis exceeds u32")
            })?,
        });
    }
    for (index, (kernel_ty, reference_ty)) in kernel_signature
        .inputs()
        .iter()
        .copied()
        .zip(
            reference_signature
                .inputs()
                .iter()
                .copied()
                .skip(point_axis_count),
        )
        .enumerate()
    {
        let argument = u32::try_from(index)
            .map_err(|_| ReferenceBindingErrorV1::new("reference argument index exceeds u32"))?;
        if let Some(scalar) = scalar_type_v1(kernel_ty) {
            if kernel_ty != reference_ty {
                return Err(logical_abi_mismatch(index, kernel_ty, reference_ty));
            }
            relations.push(ReferenceArgumentRelationV1::ScalarInput { argument, scalar });
            continue;
        }
        if let Some(element) = shared_slice_element_v1(kernel_ty) {
            if kernel_ty != reference_ty {
                return Err(logical_abi_mismatch(index, kernel_ty, reference_ty));
            }
            relations.push(ReferenceArgumentRelationV1::SharedSliceInput { argument, element });
            continue;
        }
        if let Some((element_ty, element)) = disjoint_slice_element_v1(tcx, kernel_ty) {
            match *reference_ty.kind() {
                TyKind::Ref(_, pointee, Mutability::Mut) if matches!(*pointee.kind(), TyKind::Slice(actual) if actual == element_ty) =>
                {
                    relations.push(ReferenceArgumentRelationV1::DisjointOutputSlice {
                        argument,
                        element,
                    });
                    continue;
                }
                TyKind::Ref(_, pointee, Mutability::Mut) if pointee == element_ty => {
                    relations.push(ReferenceArgumentRelationV1::DisjointOutputCoordinate {
                        argument,
                        element,
                    });
                    continue;
                }
                _ => return Err(logical_abi_mismatch(index, kernel_ty, reference_ty)),
            }
        }
        return Err(ReferenceBindingErrorV1::new(format!(
            "kernel argument {} type '{kernel_ty}' has no reference ABI relation",
            index + 1,
        )));
    }
    Ok(relations)
}

fn logical_abi_mismatch(
    index: usize,
    kernel_ty: Ty<'_>,
    reference_ty: Ty<'_>,
) -> ReferenceBindingErrorV1 {
    ReferenceBindingErrorV1::new(format!(
        "safe Rust reference logical ABI mismatch at argument {}: kernel '{kernel_ty}', reference '{reference_ty}'",
        index + 1,
    ))
}

fn scalar_type_v1(ty: Ty<'_>) -> Option<ReferenceScalarTypeV1> {
    use rustc_middle::ty::{FloatTy, IntTy, UintTy};
    Some(match *ty.kind() {
        TyKind::Bool => ReferenceScalarTypeV1::Bool,
        TyKind::Uint(UintTy::U8) => ReferenceScalarTypeV1::U8,
        TyKind::Uint(UintTy::U16) => ReferenceScalarTypeV1::U16,
        TyKind::Uint(UintTy::U32) => ReferenceScalarTypeV1::U32,
        TyKind::Uint(UintTy::U64) => ReferenceScalarTypeV1::U64,
        TyKind::Uint(UintTy::Usize) => ReferenceScalarTypeV1::Usize,
        TyKind::Int(IntTy::I8) => ReferenceScalarTypeV1::I8,
        TyKind::Int(IntTy::I16) => ReferenceScalarTypeV1::I16,
        TyKind::Int(IntTy::I32) => ReferenceScalarTypeV1::I32,
        TyKind::Int(IntTy::I64) => ReferenceScalarTypeV1::I64,
        TyKind::Int(IntTy::Isize) => ReferenceScalarTypeV1::Isize,
        TyKind::Float(FloatTy::F32) => ReferenceScalarTypeV1::F32,
        TyKind::Float(FloatTy::F64) => ReferenceScalarTypeV1::F64,
        _ => return None,
    })
}

fn shared_slice_element_v1(ty: Ty<'_>) -> Option<ReferenceScalarTypeV1> {
    let TyKind::Ref(_, pointee, Mutability::Not) = *ty.kind() else {
        return None;
    };
    let TyKind::Slice(element) = *pointee.kind() else {
        return None;
    };
    scalar_type_v1(element)
}

fn disjoint_slice_element_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Option<(Ty<'tcx>, ReferenceScalarTypeV1)> {
    let TyKind::Adt(definition, arguments) = *ty.kind() else {
        return None;
    };
    if trusted_device_items::classify(tcx, definition.did())
        != Some(TrustedDeviceItem::DisjointSlice)
    {
        return None;
    }
    let element = arguments.first()?.as_type()?;
    Some((element, scalar_type_v1(element)?))
}

fn lower_reference_effect_ir_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    reference: Instance<'tcx>,
    relations: Vec<ReferenceArgumentRelationV1>,
) -> Result<ReferenceEffectIrV1, ReferenceBindingErrorV1> {
    let body = tcx.instance_mir(reference.def);
    if body.basic_blocks.len() > MAX_REFERENCE_BLOCKS_V1 {
        return Err(ReferenceBindingErrorV1::new(format!(
            "safe Rust reference has {} MIR blocks; maximum is {MAX_REFERENCE_BLOCKS_V1}",
            body.basic_blocks.len(),
        )));
    }
    let statement_count = body
        .basic_blocks
        .iter()
        .map(|block| block.statements.len())
        .try_fold(0_usize, usize::checked_add)
        .ok_or_else(|| ReferenceBindingErrorV1::new("reference statement count overflowed"))?;
    if statement_count > MAX_REFERENCE_STATEMENTS_V1 {
        return Err(ReferenceBindingErrorV1::new(format!(
            "safe Rust reference has {statement_count} MIR statements; maximum is {MAX_REFERENCE_STATEMENTS_V1}",
        )));
    }
    for (local, declaration) in body.local_decls.iter_enumerated() {
        if !supported_local_type_v1(declaration.ty) {
            return Err(ReferenceBindingErrorV1::new(format!(
                "unsupported safe Rust reference local _{} type '{}'; V1 accepts unit, scalar values, scalar tuples, and references or slices of scalars",
                local.as_usize(),
                declaration.ty,
            )));
        }
    }
    let mut blocks = Vec::with_capacity(body.basic_blocks.len());
    for (block_id, block) in body.basic_blocks.iter_enumerated() {
        let block_index = block_id.as_usize();
        let mut assignments = Vec::new();
        for (statement_index, statement) in block.statements.iter().enumerate() {
            match &statement.kind {
                StatementKind::Assign(assignment) => {
                    let (destination, value) = &**assignment;
                    assignments.push(ReferenceAssignmentV1 {
                        statement: u32::try_from(statement_index).map_err(|_| {
                            ReferenceBindingErrorV1::new("reference statement index exceeds u32")
                        })?,
                        destination: lower_place_v1(tcx, body, *destination, block_index)?,
                        value: lower_rvalue_v1(tcx, body, value, block_index)?,
                    });
                }
                StatementKind::StorageLive(_)
                | StatementKind::StorageDead(_)
                | StatementKind::Nop => {}
                unsupported => {
                    return Err(ReferenceBindingErrorV1::at(
                        tcx,
                        body,
                        block_index,
                        format_args!(
                            "statement operation '{unsupported:?}' is outside reference-effect V1"
                        ),
                    ));
                }
            }
        }
        let terminator = match &block.terminator().kind {
            TerminatorKind::Return => ReferenceTerminatorV1::Return,
            TerminatorKind::Goto { target } => ReferenceTerminatorV1::Goto {
                target: target.as_u32(),
            },
            TerminatorKind::SwitchInt { discr, targets } => ReferenceTerminatorV1::Switch {
                discriminant: lower_operand_v1(tcx, body, discr, block_index)?,
                values: targets
                    .iter()
                    .map(|(value, target)| (value, target.as_u32()))
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
                otherwise: targets.otherwise().as_u32(),
            },
            TerminatorKind::Assert {
                cond,
                expected,
                target,
                unwind: UnwindAction::Unreachable,
                msg,
            } => ReferenceTerminatorV1::Assert {
                condition: lower_operand_v1(tcx, body, cond, block_index)?,
                expected: *expected,
                success: target.as_u32(),
                bounds_check: matches!(&**msg, AssertMessage::BoundsCheck { .. }),
            },
            TerminatorKind::Call {
                func,
                args,
                destination,
                target: Some(target),
                unwind: UnwindAction::Continue | UnwindAction::Unreachable,
                ..
            } => {
                assignments.push(lower_safe_scalar_helper_call_v2(
                    tcx,
                    reference,
                    ReferenceHelperCallSiteV2 {
                        body,
                        block: block_index,
                        statement: u32::try_from(block.statements.len()).map_err(|_| {
                            ReferenceBindingErrorV1::new(
                                "synthetic helper-call statement index exceeds u32",
                            )
                        })?,
                        destination: *destination,
                    },
                    func,
                    args,
                )?);
                ReferenceTerminatorV1::Goto {
                    target: target.as_u32(),
                }
            }
            TerminatorKind::Call { .. } => {
                return Err(ReferenceBindingErrorV1::at(
                    tcx,
                    body,
                    block_index,
                    "only a returning direct call to one safe local pure scalar helper is supported",
                ));
            }
            unsupported => {
                return Err(ReferenceBindingErrorV1::at(
                    tcx,
                    body,
                    block_index,
                    format_args!("terminator '{unsupported:?}' is outside reference-effect V1"),
                ));
            }
        };
        blocks.push(ReferenceBlockV1 {
            block: block_id.as_u32(),
            assignments: assignments.into_boxed_slice(),
            terminator,
        });
    }
    let mut effect_ir = ReferenceEffectIrV1 {
        argument_count: u32::try_from(body.arg_count)
            .map_err(|_| ReferenceBindingErrorV1::new("reference argument count exceeds u32"))?,
        local_count: u32::try_from(body.local_decls.len())
            .map_err(|_| ReferenceBindingErrorV1::new("reference local count exceeds u32"))?,
        relations: relations.into_boxed_slice(),
        blocks: blocks.into_boxed_slice(),
        loop_summaries: Box::default(),
        observable_output_effects: Box::default(),
    };
    let backedges = reference_cfg_backedges_v2(&effect_ir)?;
    if backedges.is_empty() {
        effect_ir.observable_output_effects =
            effect_ir.observable_output_writes_v1()?.into_boxed_slice();
    } else {
        validate_reference_loop_shapes_v2(&effect_ir, &backedges)?;
        let (effects, summaries) = effect_ir.observable_output_writes_with_loops_v2(&backedges)?;
        effect_ir.observable_output_effects = effects.into_boxed_slice();
        effect_ir.loop_summaries = summaries.into_boxed_slice();
    }
    Ok(effect_ir)
}

struct ReferenceHelperCallSiteV2<'a, 'tcx> {
    body: &'a Body<'tcx>,
    block: usize,
    statement: u32,
    destination: Place<'tcx>,
}

fn lower_safe_scalar_helper_call_v2<'tcx>(
    tcx: TyCtxt<'tcx>,
    caller: Instance<'tcx>,
    site: ReferenceHelperCallSiteV2<'_, 'tcx>,
    function: &Operand<'tcx>,
    arguments: &[Spanned<Operand<'tcx>>],
) -> Result<ReferenceAssignmentV1, ReferenceBindingErrorV1> {
    let ReferenceHelperCallSiteV2 {
        body,
        block,
        statement,
        destination,
    } = site;
    let Operand::Constant(function) = function else {
        return Err(ReferenceBindingErrorV1::at(
            tcx,
            body,
            block,
            "indirect helper calls are outside authenticated reference semantics",
        ));
    };
    let TyKind::FnDef(definition, generic_arguments) = function.const_.ty().kind() else {
        return Err(ReferenceBindingErrorV1::at(
            tcx,
            body,
            block,
            "helper call target is not one statically resolved function item",
        ));
    };
    let helper = Instance::try_resolve(
        tcx,
        TypingEnv::fully_monomorphized(),
        *definition,
        generic_arguments,
    )
    .map_err(|_| {
        ReferenceBindingErrorV1::at(tcx, body, block, "safe helper instance resolution failed")
    })?
    .ok_or_else(|| {
        ReferenceBindingErrorV1::at(tcx, body, block, "safe helper instance is not monomorphic")
    })?;
    if helper == caller {
        return Err(ReferenceBindingErrorV1::at(
            tcx,
            body,
            block,
            "recursive safe reference helpers are unsupported",
        ));
    }
    authenticate_safe_local_reference(tcx, helper)?;
    let signature = instantiated_signature(tcx, helper);
    if signature.inputs().len() > MAX_REFERENCE_HELPER_ARGUMENTS_V2 {
        return Err(ReferenceBindingErrorV1::at(
            tcx,
            body,
            block,
            format_args!(
                "safe helper has {} arguments; maximum is {MAX_REFERENCE_HELPER_ARGUMENTS_V2}",
                signature.inputs().len(),
            ),
        ));
    }
    if signature.inputs().len() != arguments.len() {
        return Err(ReferenceBindingErrorV1::at(
            tcx,
            body,
            block,
            "safe helper MIR argument count disagrees with its instantiated signature",
        ));
    }
    let parameters = signature
        .inputs()
        .iter()
        .copied()
        .zip(arguments)
        .enumerate()
        .map(|(index, (expected, argument))| {
            let scalar = scalar_type_v1(expected).ok_or_else(|| {
                ReferenceBindingErrorV1::at(
                    tcx,
                    body,
                    block,
                    format_args!(
                        "safe helper argument {} type '{expected}' is not a supported scalar",
                        index + 1,
                    ),
                )
            })?;
            let actual = argument.node.ty(body, tcx);
            if actual != expected {
                return Err(ReferenceBindingErrorV1::at(
                    tcx,
                    body,
                    block,
                    format_args!(
                        "safe helper argument {} has type '{actual}', expected '{expected}'",
                        index + 1,
                    ),
                ));
            }
            Ok(scalar)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let result_ty = signature.output();
    let result = scalar_type_v1(result_ty).ok_or_else(|| {
        ReferenceBindingErrorV1::at(
            tcx,
            body,
            block,
            format_args!("safe helper result type '{result_ty}' is not a supported scalar"),
        )
    })?;
    if destination.ty(body, tcx).ty != result_ty {
        return Err(ReferenceBindingErrorV1::at(
            tcx,
            body,
            block,
            "safe helper return destination type disagrees with its instantiated signature",
        ));
    }
    let summary = lower_safe_scalar_helper_summary_v2(tcx, helper, &parameters)?;
    Ok(ReferenceAssignmentV1 {
        statement,
        destination: lower_place_v1(tcx, body, destination, block)?,
        value: ReferenceValueV1::SafeHelperCall {
            helper: function_identity_v1(tcx, helper),
            parameters: parameters.into_boxed_slice(),
            result,
            arguments: arguments
                .iter()
                .map(|argument| lower_operand_v1(tcx, body, &argument.node, block))
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice(),
            summary: Box::new(summary),
        },
    })
}

fn lower_safe_scalar_helper_summary_v2<'tcx>(
    tcx: TyCtxt<'tcx>,
    helper: Instance<'tcx>,
    parameters: &[ReferenceScalarTypeV1],
) -> Result<ReferenceEffectExpressionV1, ReferenceBindingErrorV1> {
    let body = tcx.instance_mir(helper.def);
    reject_cycles_v1(tcx, body).map_err(|_| {
        ReferenceBindingErrorV1::new(format!(
            "safe helper '{}' contains a loop; helper summaries must be acyclic",
            tcx.def_path_str(helper.def_id()),
        ))
    })?;
    if body.basic_blocks.len() > MAX_REFERENCE_BLOCKS_V1 {
        return Err(ReferenceBindingErrorV1::new(format!(
            "safe helper '{}' has {} MIR blocks; maximum is {MAX_REFERENCE_BLOCKS_V1}",
            tcx.def_path_str(helper.def_id()),
            body.basic_blocks.len(),
        )));
    }
    let statement_count = body
        .basic_blocks
        .iter()
        .map(|block| block.statements.len())
        .try_fold(0_usize, usize::checked_add)
        .ok_or_else(|| ReferenceBindingErrorV1::new("safe helper statement count overflowed"))?;
    if statement_count > MAX_REFERENCE_STATEMENTS_V1 {
        return Err(ReferenceBindingErrorV1::new(format!(
            "safe helper '{}' has {statement_count} MIR statements; maximum is {MAX_REFERENCE_STATEMENTS_V1}",
            tcx.def_path_str(helper.def_id()),
        )));
    }
    for (local, declaration) in body.local_decls.iter_enumerated() {
        if !supported_local_type_v1(declaration.ty) {
            return Err(ReferenceBindingErrorV1::new(format!(
                "safe helper '{}' local _{} type '{}' is unsupported",
                tcx.def_path_str(helper.def_id()),
                local.as_usize(),
                declaration.ty,
            )));
        }
    }
    let mut blocks = Vec::with_capacity(body.basic_blocks.len());
    for (block_id, block) in body.basic_blocks.iter_enumerated() {
        let block_index = block_id.as_usize();
        let mut assignments = Vec::new();
        for (statement_index, statement) in block.statements.iter().enumerate() {
            match &statement.kind {
                StatementKind::Assign(assignment) => {
                    let (destination, value) = &**assignment;
                    assignments.push(ReferenceAssignmentV1 {
                        statement: u32::try_from(statement_index).map_err(|_| {
                            ReferenceBindingErrorV1::new("safe helper statement index exceeds u32")
                        })?,
                        destination: lower_place_v1(tcx, body, *destination, block_index)?,
                        value: lower_rvalue_v1(tcx, body, value, block_index)?,
                    });
                }
                StatementKind::StorageLive(_)
                | StatementKind::StorageDead(_)
                | StatementKind::Nop => {}
                unsupported => {
                    return Err(ReferenceBindingErrorV1::at(
                        tcx,
                        body,
                        block_index,
                        format_args!("safe helper statement '{unsupported:?}' is unsupported"),
                    ));
                }
            }
        }
        let terminator = match &block.terminator().kind {
            TerminatorKind::Return => ReferenceTerminatorV1::Return,
            TerminatorKind::Goto { target } => ReferenceTerminatorV1::Goto {
                target: target.as_u32(),
            },
            TerminatorKind::Assert {
                cond,
                expected,
                target,
                unwind,
                ..
            } if matches!(unwind, UnwindAction::Continue | UnwindAction::Unreachable)
                && !*expected
                && matches!(
                    cond,
                    Operand::Copy(place) | Operand::Move(place)
                        if matches!(
                            place.projection.as_ref(),
                            [ProjectionElem::Field(field, _)] if field.as_u32() == 1
                        )
                ) =>
            {
                ReferenceTerminatorV1::Assert {
                    condition: lower_operand_v1(tcx, body, cond, block_index)?,
                    expected: *expected,
                    success: target.as_u32(),
                    bounds_check: false,
                }
            }
            TerminatorKind::SwitchInt { .. } | TerminatorKind::Assert { .. } => {
                return Err(ReferenceBindingErrorV1::at(
                    tcx,
                    body,
                    block_index,
                    "safe scalar helper control flow is unsupported; only compiler checked-overflow assertions may be summarized",
                ));
            }
            TerminatorKind::Call { func, .. } => {
                if let Operand::Constant(function) = func
                    && let TyKind::FnDef(definition, generic_arguments) =
                        function.const_.ty().kind()
                    && let Ok(Some(callee)) = Instance::try_resolve(
                        tcx,
                        TypingEnv::fully_monomorphized(),
                        *definition,
                        generic_arguments,
                    )
                    && callee == helper
                {
                    return Err(ReferenceBindingErrorV1::at(
                        tcx,
                        body,
                        block_index,
                        "recursive safe scalar helper is unsupported",
                    ));
                }
                return Err(ReferenceBindingErrorV1::at(
                    tcx,
                    body,
                    block_index,
                    "nested safe helper calls are unsupported; make the helper one pure scalar expression",
                ));
            }
            unsupported => {
                return Err(ReferenceBindingErrorV1::at(
                    tcx,
                    body,
                    block_index,
                    format_args!("safe helper terminator '{unsupported:?}' is unsupported"),
                ));
            }
        };
        blocks.push(ReferenceBlockV1 {
            block: block_id.as_u32(),
            assignments: assignments.into_boxed_slice(),
            terminator,
        });
    }
    let relations = parameters
        .iter()
        .copied()
        .enumerate()
        .map(
            |(argument, scalar)| ReferenceArgumentRelationV1::ScalarInput {
                argument: u32::try_from(argument).unwrap_or(u32::MAX),
                scalar,
            },
        )
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let effect_ir = ReferenceEffectIrV1 {
        argument_count: u32::try_from(body.arg_count)
            .map_err(|_| ReferenceBindingErrorV1::new("safe helper argument count exceeds u32"))?,
        local_count: u32::try_from(body.local_decls.len())
            .map_err(|_| ReferenceBindingErrorV1::new("safe helper local count exceeds u32"))?,
        relations,
        blocks: blocks.into_boxed_slice(),
        loop_summaries: Box::default(),
        observable_output_effects: Box::default(),
    };
    ReferenceExpressionResolverV1::new(&effect_ir)?.resolve_local_v1(0)
}

struct ReferenceExpressionResolverV1<'a> {
    effect_ir: &'a ReferenceEffectIrV1,
    definitions: BTreeMap<u32, &'a ReferenceValueV1>,
    ambiguous_definitions: BTreeSet<u32>,
}

impl<'a> ReferenceExpressionResolverV1<'a> {
    fn new(effect_ir: &'a ReferenceEffectIrV1) -> Result<Self, ReferenceBindingErrorV1> {
        let mut definitions = BTreeMap::new();
        let mut ambiguous_definitions = BTreeSet::new();
        for block in &effect_ir.blocks {
            for assignment in &block.assignments {
                if !assignment.destination.projection.is_empty() {
                    continue;
                }
                if assignment.destination.local > 0
                    && assignment.destination.local <= effect_ir.argument_count
                {
                    return Err(ReferenceBindingErrorV1::new(format!(
                        "reference effect reassigns logical argument {}; mutable argument-local normalization is outside reference-effect V1",
                        assignment.destination.local,
                    )));
                }
                if definitions
                    .insert(assignment.destination.local, &assignment.value)
                    .is_some()
                {
                    ambiguous_definitions.insert(assignment.destination.local);
                }
            }
        }
        Ok(Self {
            effect_ir,
            definitions,
            ambiguous_definitions,
        })
    }

    fn resolve_local_v1(
        &self,
        local: u32,
    ) -> Result<ReferenceEffectExpressionV1, ReferenceBindingErrorV1> {
        self.resolve_local_inner_v1(local, &mut BTreeSet::new(), &mut 0, 0)
    }

    fn resolve_value_v1(
        &self,
        value: &ReferenceValueV1,
    ) -> Result<ReferenceEffectExpressionV1, ReferenceBindingErrorV1> {
        self.resolve_value_inner_v1(value, &mut BTreeSet::new(), &mut 0, 0)
    }

    fn charge_node_v1(work: &mut usize) -> Result<(), ReferenceBindingErrorV1> {
        *work = work
            .checked_add(1)
            .ok_or_else(|| ReferenceBindingErrorV1::new("reference expression work overflowed"))?;
        if *work > MAX_REFERENCE_EXPRESSION_NODES_V1 {
            return Err(ReferenceBindingErrorV1::new(format!(
                "reference effect expression exceeds {MAX_REFERENCE_EXPRESSION_NODES_V1} nodes",
            )));
        }
        Ok(())
    }

    fn require_depth_v1(depth: usize) -> Result<(), ReferenceBindingErrorV1> {
        if depth > fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 {
            return Err(ReferenceBindingErrorV1::new(format!(
                "reference effect expression exceeds {} resolution levels",
                fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2,
            )));
        }
        Ok(())
    }

    fn resolve_local_inner_v1(
        &self,
        local: u32,
        visiting: &mut BTreeSet<u32>,
        work: &mut usize,
        depth: usize,
    ) -> Result<ReferenceEffectExpressionV1, ReferenceBindingErrorV1> {
        Self::require_depth_v1(depth)?;
        Self::charge_node_v1(work)?;
        if local > 0 && local <= self.effect_ir.argument_count {
            let reference_argument = local - 1;
            if let Some((axis, _)) =
                self.effect_ir
                    .relations
                    .iter()
                    .find_map(|relation| match relation {
                        ReferenceArgumentRelationV1::PointCoordinate {
                            reference_argument: actual,
                            axis,
                        } if *actual == reference_argument => Some((*axis, *actual)),
                        _ => None,
                    })
            {
                return Ok(ReferenceEffectExpressionV1::PointCoordinate { axis });
            }
            let point_count = self.effect_ir.point_coordinate_count_v1()?;
            let kernel_argument = reference_argument.checked_sub(point_count).ok_or_else(|| {
                ReferenceBindingErrorV1::new("reference argument has no logical ABI relation")
            })?;
            return match self
                .effect_ir
                .relations
                .iter()
                .find(|relation| match relation {
                    ReferenceArgumentRelationV1::ScalarInput { argument, .. }
                    | ReferenceArgumentRelationV1::SharedSliceInput { argument, .. }
                    | ReferenceArgumentRelationV1::DisjointOutputSlice { argument, .. }
                    | ReferenceArgumentRelationV1::DisjointOutputCoordinate { argument, .. } => {
                        *argument == kernel_argument
                    }
                    ReferenceArgumentRelationV1::PointCoordinate { .. } => false,
                }) {
                Some(ReferenceArgumentRelationV1::ScalarInput { .. }) => {
                    Ok(ReferenceEffectExpressionV1::KernelScalarArgument {
                        argument: kernel_argument,
                    })
                }
                Some(_) => Err(ReferenceBindingErrorV1::new(format!(
                    "reference effect expression reads non-scalar logical argument {}",
                    kernel_argument + 1,
                ))),
                None => Err(ReferenceBindingErrorV1::new(format!(
                    "reference argument {} has no logical ABI relation",
                    reference_argument + 1,
                ))),
            };
        }
        if self.ambiguous_definitions.contains(&local) {
            return Err(ReferenceBindingErrorV1::new(format!(
                "reference effect local _{local} has multiple definitions; path-sensitive scalar phi normalization is outside reference-effect V1",
            )));
        }
        let value = self.definitions.get(&local).ok_or_else(|| {
            ReferenceBindingErrorV1::new(format!(
                "reference effect local _{local} has no unique scalar definition",
            ))
        })?;
        if !visiting.insert(local) {
            return Err(ReferenceBindingErrorV1::new(format!(
                "reference effect local _{local} has a cyclic scalar definition",
            )));
        }
        let resolved = self.resolve_value_inner_v1(value, visiting, work, depth);
        visiting.remove(&local);
        resolved
    }

    fn resolve_operand_inner_v1(
        &self,
        operand: &ReferenceOperandV1,
        visiting: &mut BTreeSet<u32>,
        work: &mut usize,
        depth: usize,
    ) -> Result<ReferenceEffectExpressionV1, ReferenceBindingErrorV1> {
        Self::require_depth_v1(depth)?;
        match operand {
            ReferenceOperandV1::Constant(constant) => {
                Self::charge_node_v1(work)?;
                Ok(ReferenceEffectExpressionV1::Constant(constant.clone()))
            }
            ReferenceOperandV1::Copy(place) | ReferenceOperandV1::Move(place)
                if place.projection.is_empty() =>
            {
                self.resolve_local_inner_v1(place.local, visiting, work, depth)
            }
            ReferenceOperandV1::Copy(place) | ReferenceOperandV1::Move(place)
                if matches!(
                    place.projection.as_ref(),
                    [ReferencePlaceProjectionV1::Field(0)]
                ) =>
            {
                let value = self.definitions.get(&place.local).ok_or_else(|| {
                    ReferenceBindingErrorV1::new(format!(
                        "reference checked scalar pair _{} has no unique definition",
                        place.local,
                    ))
                })?;
                match value {
                    ReferenceValueV1::Binary { checked: true, .. } => {
                        self.resolve_value_inner_v1(value, visiting, work, depth + 1)
                    }
                    _ => Err(ReferenceBindingErrorV1::new(format!(
                        "reference field projection {:?} is not the value field of one checked scalar operation",
                        place.projection,
                    ))),
                }
            }
            ReferenceOperandV1::Copy(place) | ReferenceOperandV1::Move(place)
                if matches!(
                    place.projection.as_ref(),
                    [
                        ReferencePlaceProjectionV1::Dereference,
                        ReferencePlaceProjectionV1::Index(_)
                    ]
                ) =>
            {
                let [
                    ReferencePlaceProjectionV1::Dereference,
                    ReferencePlaceProjectionV1::Index(index),
                ] = place.projection.as_ref()
                else {
                    unreachable!()
                };
                let reference_argument = place.local.checked_sub(1).ok_or_else(|| {
                    ReferenceBindingErrorV1::new(
                        "safe reference load uses the return-place local as its base",
                    )
                })?;
                let point_count = self.effect_ir.point_coordinate_count_v1()?;
                let kernel_argument =
                    reference_argument.checked_sub(point_count).ok_or_else(|| {
                        ReferenceBindingErrorV1::new(
                            "safe reference load base has no logical kernel argument",
                        )
                    })?;
                if !self.effect_ir.relations.iter().any(|relation| {
                    matches!(
                        relation,
                        ReferenceArgumentRelationV1::SharedSliceInput { argument, .. }
                            if *argument == kernel_argument
                    )
                }) {
                    return Err(ReferenceBindingErrorV1::new(
                        "safe reference load base is not a shared-slice input",
                    ));
                }
                Self::charge_node_v1(work)?;
                Ok(ReferenceEffectExpressionV1::InputLoad {
                    reference_argument,
                    index: Box::new(self.resolve_local_inner_v1(
                        *index,
                        visiting,
                        work,
                        depth + 1,
                    )?),
                })
            }
            ReferenceOperandV1::Copy(place) | ReferenceOperandV1::Move(place) => {
                Err(ReferenceBindingErrorV1::new(format!(
                    "reference effect scalar operand uses unsupported place projection {:?}",
                    place.projection,
                )))
            }
        }
    }

    fn resolve_value_inner_v1(
        &self,
        value: &ReferenceValueV1,
        visiting: &mut BTreeSet<u32>,
        work: &mut usize,
        depth: usize,
    ) -> Result<ReferenceEffectExpressionV1, ReferenceBindingErrorV1> {
        Self::require_depth_v1(depth)?;
        Self::charge_node_v1(work)?;
        match value {
            ReferenceValueV1::Use(operand) => {
                self.resolve_operand_inner_v1(operand, visiting, work, depth + 1)
            }
            ReferenceValueV1::Binary {
                operation,
                lhs,
                rhs,
                checked,
            } => Ok(ReferenceEffectExpressionV1::Binary {
                operation: *operation,
                lhs: Box::new(self.resolve_operand_inner_v1(lhs, visiting, work, depth + 1)?),
                rhs: Box::new(self.resolve_operand_inner_v1(rhs, visiting, work, depth + 1)?),
                checked: *checked,
            }),
            ReferenceValueV1::Unary { operation, operand } => {
                Ok(ReferenceEffectExpressionV1::Unary {
                    operation: *operation,
                    operand: Box::new(self.resolve_operand_inner_v1(
                        operand,
                        visiting,
                        work,
                        depth + 1,
                    )?),
                })
            }
            ReferenceValueV1::Cast {
                kind,
                source,
                target,
                operand,
            } => Ok(ReferenceEffectExpressionV1::Cast {
                kind: *kind,
                source: *source,
                target: *target,
                operand: Box::new(self.resolve_operand_inner_v1(
                    operand,
                    visiting,
                    work,
                    depth + 1,
                )?),
            }),
            ReferenceValueV1::InputLength { reference_argument } => {
                Ok(ReferenceEffectExpressionV1::InputLength {
                    reference_argument: *reference_argument,
                })
            }
            ReferenceValueV1::SafeHelperCall {
                parameters,
                arguments,
                summary,
                ..
            } => {
                if parameters.len() != arguments.len() {
                    return Err(ReferenceBindingErrorV1::new(
                        "authenticated safe helper summary argument count changed",
                    ));
                }
                let arguments = arguments
                    .iter()
                    .map(|argument| {
                        self.resolve_operand_inner_v1(argument, visiting, work, depth + 1)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                substitute_helper_summary_v2(summary, &arguments, work, depth + 1)
            }
        }
    }
}

fn substitute_helper_summary_v2(
    expression: &ReferenceEffectExpressionV1,
    arguments: &[ReferenceEffectExpressionV1],
    work: &mut usize,
    depth: usize,
) -> Result<ReferenceEffectExpressionV1, ReferenceBindingErrorV1> {
    ReferenceExpressionResolverV1::require_depth_v1(depth)?;
    ReferenceExpressionResolverV1::charge_node_v1(work)?;
    Ok(match expression {
        ReferenceEffectExpressionV1::KernelScalarArgument { argument } => {
            arguments.get(*argument as usize).cloned().ok_or_else(|| {
                ReferenceBindingErrorV1::new(format!(
                    "safe helper summary refers to missing argument {}",
                    argument + 1,
                ))
            })?
        }
        ReferenceEffectExpressionV1::PointCoordinate { .. } => {
            return Err(ReferenceBindingErrorV1::new(
                "safe scalar helper summary unexpectedly contains a point-coordinate symbol",
            ));
        }
        ReferenceEffectExpressionV1::InputLoad { .. } => {
            return Err(ReferenceBindingErrorV1::new(
                "safe scalar helper summaries cannot capture reference loads",
            ));
        }
        ReferenceEffectExpressionV1::InputLength { .. } => {
            return Err(ReferenceBindingErrorV1::new(
                "safe scalar helper summaries cannot capture slice lengths",
            ));
        }
        ReferenceEffectExpressionV1::Constant(constant) => {
            ReferenceEffectExpressionV1::Constant(constant.clone())
        }
        ReferenceEffectExpressionV1::Binary {
            operation,
            lhs,
            rhs,
            checked,
        } => ReferenceEffectExpressionV1::Binary {
            operation: *operation,
            lhs: Box::new(substitute_helper_summary_v2(
                lhs,
                arguments,
                work,
                depth + 1,
            )?),
            rhs: Box::new(substitute_helper_summary_v2(
                rhs,
                arguments,
                work,
                depth + 1,
            )?),
            checked: *checked,
        },
        ReferenceEffectExpressionV1::Unary { operation, operand } => {
            ReferenceEffectExpressionV1::Unary {
                operation: *operation,
                operand: Box::new(substitute_helper_summary_v2(
                    operand,
                    arguments,
                    work,
                    depth + 1,
                )?),
            }
        }
        ReferenceEffectExpressionV1::Cast {
            kind,
            source,
            target,
            operand,
        } => ReferenceEffectExpressionV1::Cast {
            kind: *kind,
            source: *source,
            target: *target,
            operand: Box::new(substitute_helper_summary_v2(
                operand,
                arguments,
                work,
                depth + 1,
            )?),
        },
    })
}

fn reference_block_path_predicates_v1(
    effect_ir: &ReferenceEffectIrV1,
) -> Result<Vec<ReferencePathPredicateV1>, ReferenceBindingErrorV1> {
    let block_count = effect_ir.blocks.len();
    if block_count == 0 {
        return Err(ReferenceBindingErrorV1::new(
            "reference effect IR has no entry block",
        ));
    }
    for (index, block) in effect_ir.blocks.iter().enumerate() {
        if block.block as usize != index {
            return Err(ReferenceBindingErrorV1::new(
                "reference effect block identities are not contiguous",
            ));
        }
    }
    let resolver = ReferenceExpressionResolverV1::new(effect_ir)?;
    let mut successors = vec![BTreeSet::new(); block_count];
    let mut indegree = vec![0_usize; block_count];
    for block in &effect_ir.blocks {
        for target in reference_successors_v1(&block.terminator) {
            let target_index = target as usize;
            if target_index >= block_count {
                return Err(ReferenceBindingErrorV1::new(
                    "reference effect terminator target is out of bounds",
                ));
            }
            if successors[block.block as usize].insert(target_index) {
                indegree[target_index] =
                    indegree[target_index].checked_add(1).ok_or_else(|| {
                        ReferenceBindingErrorV1::new("reference effect CFG indegree overflowed")
                    })?;
            }
        }
    }
    let mut pending = indegree
        .iter()
        .enumerate()
        .filter_map(|(block, degree)| (*degree == 0).then_some(block))
        .collect::<VecDeque<_>>();
    let mut predicates = vec![ReferencePathPredicateV1::unreachable_v1(); block_count];
    predicates[0] = ReferencePathPredicateV1::unconditional_v1();
    let mut visited = 0_usize;
    while let Some(block_index) = pending.pop_front() {
        visited += 1;
        let source = predicates[block_index].clone();
        for (target, atom) in
            reference_guarded_edges_v1(&effect_ir.blocks[block_index].terminator, &resolver)?
        {
            let contribution = match atom {
                Some(atom) => reference_predicate_and_atom_v1(&source, atom)?,
                None => source.clone(),
            };
            reference_predicate_or_assign_v1(&mut predicates[target as usize], contribution)?;
        }
        for target in &successors[block_index] {
            indegree[*target] -= 1;
            if indegree[*target] == 0 {
                pending.push_back(*target);
            }
        }
    }
    if visited != block_count {
        return Err(ReferenceBindingErrorV1::new(
            "reference effect CFG contains a cycle after MIR authentication",
        ));
    }
    Ok(predicates)
}

fn reference_successors_v1(terminator: &ReferenceTerminatorV1) -> Vec<u32> {
    match terminator {
        ReferenceTerminatorV1::Return => Vec::new(),
        ReferenceTerminatorV1::Goto { target } => vec![*target],
        ReferenceTerminatorV1::Switch {
            values, otherwise, ..
        } => values
            .iter()
            .map(|(_, target)| *target)
            .chain(std::iter::once(*otherwise))
            .collect(),
        ReferenceTerminatorV1::Assert { success, .. } => vec![*success],
    }
}

fn reference_guarded_edges_v1(
    terminator: &ReferenceTerminatorV1,
    resolver: &ReferenceExpressionResolverV1<'_>,
) -> Result<Vec<(u32, Option<ReferenceGuardAtomV1>)>, ReferenceBindingErrorV1> {
    match terminator {
        ReferenceTerminatorV1::Return => Ok(Vec::new()),
        ReferenceTerminatorV1::Goto { target } => Ok(vec![(*target, None)]),
        ReferenceTerminatorV1::Assert {
            condition,
            expected,
            success,
            bounds_check,
        } => Ok(vec![(
            *success,
            if *bounds_check {
                None
            } else {
                Some(ReferenceGuardAtomV1::Assert {
                    condition: resolver.resolve_operand_inner_v1(
                        condition,
                        &mut BTreeSet::new(),
                        &mut 0,
                        1,
                    )?,
                    expected: *expected,
                })
            },
        )]),
        ReferenceTerminatorV1::Switch {
            discriminant,
            values,
            otherwise,
        } => {
            let expression =
                resolver.resolve_operand_inner_v1(discriminant, &mut BTreeSet::new(), &mut 0, 1)?;
            let mut by_target = BTreeMap::<u32, Vec<u128>>::new();
            let mut all_values = Vec::with_capacity(values.len());
            for (value, target) in values {
                by_target.entry(*target).or_default().push(*value);
                all_values.push(*value);
            }
            all_values.sort_unstable();
            all_values.dedup();
            let mut edges = Vec::with_capacity(by_target.len() + 1);
            for (target, mut accepted) in by_target {
                accepted.sort_unstable();
                accepted.dedup();
                edges.push((
                    target,
                    Some(ReferenceGuardAtomV1::SwitchValueSet {
                        discriminant: expression.clone(),
                        values: accepted.into_boxed_slice(),
                        inside_set: true,
                    }),
                ));
            }
            edges.push((
                *otherwise,
                Some(ReferenceGuardAtomV1::SwitchValueSet {
                    discriminant: expression,
                    values: all_values.into_boxed_slice(),
                    inside_set: false,
                }),
            ));
            Ok(edges)
        }
    }
}

fn reference_predicate_and_atom_v1(
    predicate: &ReferencePathPredicateV1,
    atom: ReferenceGuardAtomV1,
) -> Result<ReferencePathPredicateV1, ReferenceBindingErrorV1> {
    let mut clauses = Vec::with_capacity(predicate.clauses.len());
    for clause in &predicate.clauses {
        let mut atoms = clause.atoms.to_vec();
        atoms.push(atom.clone());
        atoms.sort();
        atoms.dedup();
        clauses.push(ReferenceGuardClauseV1 {
            atoms: atoms.into_boxed_slice(),
        });
    }
    reference_normalize_predicate_v1(clauses)
}

fn reference_predicate_or_assign_v1(
    target: &mut ReferencePathPredicateV1,
    source: ReferencePathPredicateV1,
) -> Result<(), ReferenceBindingErrorV1> {
    let mut clauses = target.clauses.to_vec();
    clauses.extend(source.clauses);
    *target = reference_normalize_predicate_v1(clauses)?;
    Ok(())
}

fn reference_normalize_predicate_v1(
    mut clauses: Vec<ReferenceGuardClauseV1>,
) -> Result<ReferencePathPredicateV1, ReferenceBindingErrorV1> {
    clauses.sort();
    clauses.dedup();
    if clauses.len() > MAX_REFERENCE_GUARD_CLAUSES_V1 {
        return Err(ReferenceBindingErrorV1::new(format!(
            "reference path predicate has {} clauses; maximum is {MAX_REFERENCE_GUARD_CLAUSES_V1}",
            clauses.len(),
        )));
    }
    let atoms = clauses.iter().try_fold(0_usize, |total, clause| {
        total.checked_add(clause.atoms.len())
    });
    if atoms.is_none_or(|atoms| atoms > MAX_REFERENCE_GUARD_ATOMS_V1) {
        return Err(ReferenceBindingErrorV1::new(format!(
            "reference path predicate exceeds {MAX_REFERENCE_GUARD_ATOMS_V1} total atoms",
        )));
    }
    Ok(ReferencePathPredicateV1 {
        clauses: clauses.into_boxed_slice(),
    })
}

fn reject_cycles_v1(tcx: TyCtxt<'_>, body: &Body<'_>) -> Result<(), ReferenceBindingErrorV1> {
    let mut indegree = vec![0_usize; body.basic_blocks.len()];
    for block in body.basic_blocks.iter() {
        for successor in block.terminator().successors() {
            indegree[successor.as_usize()] = indegree[successor.as_usize()]
                .checked_add(1)
                .ok_or_else(|| ReferenceBindingErrorV1::new("reference CFG indegree overflowed"))?;
        }
    }
    let mut pending = indegree
        .iter()
        .enumerate()
        .filter_map(|(block, degree)| (*degree == 0).then_some(block))
        .collect::<VecDeque<_>>();
    let mut visited = 0_usize;
    while let Some(block) = pending.pop_front() {
        visited += 1;
        for successor in body.basic_blocks[rustc_middle::mir::BasicBlock::from_usize(block)]
            .terminator()
            .successors()
        {
            let degree = &mut indegree[successor.as_usize()];
            *degree -= 1;
            if *degree == 0 {
                pending.push_back(successor.as_usize());
            }
        }
    }
    if visited != body.basic_blocks.len() {
        let cyclic = indegree
            .iter()
            .enumerate()
            .find_map(|(block, degree)| (*degree != 0).then_some(block))
            .unwrap_or(0);
        return Err(ReferenceBindingErrorV1::at(
            tcx,
            body,
            cyclic,
            "cyclic control flow is outside the acyclic helper-summary boundary",
        ));
    }
    Ok(())
}

fn supported_local_type_v1(ty: Ty<'_>) -> bool {
    if ty.is_unit() || scalar_type_v1(ty).is_some() {
        return true;
    }
    match *ty.kind() {
        TyKind::Ref(_, pointee, _) => match *pointee.kind() {
            TyKind::Slice(element) => scalar_type_v1(element).is_some(),
            _ => scalar_type_v1(pointee).is_some(),
        },
        TyKind::Tuple(fields) => fields.iter().all(|field| scalar_type_v1(field).is_some()),
        _ => false,
    }
}

fn lower_place_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    place: Place<'tcx>,
    block: usize,
) -> Result<ReferencePlaceV1, ReferenceBindingErrorV1> {
    let mut projection = Vec::with_capacity(place.projection.len());
    for element in place.projection {
        projection.push(match element {
            ProjectionElem::Deref => ReferencePlaceProjectionV1::Dereference,
            ProjectionElem::Field(field, _) => ReferencePlaceProjectionV1::Field(field.as_u32()),
            ProjectionElem::Index(index) => ReferencePlaceProjectionV1::Index(index.as_u32()),
            ProjectionElem::ConstantIndex {
                offset,
                min_length,
                from_end,
            } => ReferencePlaceProjectionV1::ConstantIndex {
                offset,
                minimum_length: min_length,
                from_end,
            },
            unsupported => {
                return Err(ReferenceBindingErrorV1::at(
                    tcx,
                    body,
                    block,
                    format_args!(
                        "place projection '{unsupported:?}' is outside reference-effect V1"
                    ),
                ));
            }
        });
    }
    Ok(ReferencePlaceV1 {
        local: place.local.as_u32(),
        projection: projection.into_boxed_slice(),
    })
}

fn lower_operand_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    operand: &Operand<'tcx>,
    block: usize,
) -> Result<ReferenceOperandV1, ReferenceBindingErrorV1> {
    match operand {
        Operand::Copy(place) => Ok(ReferenceOperandV1::Copy(lower_place_v1(
            tcx, body, *place, block,
        )?)),
        Operand::Move(place) => Ok(ReferenceOperandV1::Move(lower_place_v1(
            tcx, body, *place, block,
        )?)),
        Operand::Constant(constant) => {
            let ty = constant.const_.ty();
            if ty.is_unit() {
                return Ok(ReferenceOperandV1::Constant(ReferenceConstantV1::ZeroSized));
            }
            let Some(scalar) = scalar_type_v1(ty) else {
                return Err(ReferenceBindingErrorV1::at(
                    tcx,
                    body,
                    block,
                    format_args!("constant type '{ty}' is outside reference-effect V1"),
                ));
            };
            let bits = constant
                .const_
                .try_eval_bits(tcx, TypingEnv::fully_monomorphized())
                .ok_or_else(|| {
                    ReferenceBindingErrorV1::at(tcx, body, block, "constant is not an exact scalar")
                })?;
            Ok(ReferenceOperandV1::Constant(ReferenceConstantV1::Scalar {
                scalar,
                bits,
            }))
        }
        Operand::RuntimeChecks(_) => Err(ReferenceBindingErrorV1::at(
            tcx,
            body,
            block,
            "runtime-check pseudo-operands are outside reference-effect V1",
        )),
    }
}

fn lower_rvalue_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    value: &Rvalue<'tcx>,
    block: usize,
) -> Result<ReferenceValueV1, ReferenceBindingErrorV1> {
    match value {
        Rvalue::Use(operand) => Ok(ReferenceValueV1::Use(lower_operand_v1(
            tcx, body, operand, block,
        )?)),
        Rvalue::BinaryOp(operation, operands) => {
            let checked = matches!(
                operation,
                BinOp::AddWithOverflow | BinOp::SubWithOverflow | BinOp::MulWithOverflow
            );
            let (lhs, rhs) = &**operands;
            Ok(ReferenceValueV1::Binary {
                operation: lower_binary_op_v1(*operation).ok_or_else(|| {
                    ReferenceBindingErrorV1::at(
                        tcx,
                        body,
                        block,
                        format_args!(
                            "binary operation '{operation:?}' is outside reference-effect V1"
                        ),
                    )
                })?,
                lhs: lower_operand_v1(tcx, body, lhs, block)?,
                rhs: lower_operand_v1(tcx, body, rhs, block)?,
                checked,
            })
        }
        Rvalue::UnaryOp(UnOp::PtrMetadata, operand) => {
            let (Operand::Copy(place) | Operand::Move(place)) = operand else {
                return Err(ReferenceBindingErrorV1::at(
                    tcx,
                    body,
                    block,
                    "slice metadata source is not one reference argument",
                ));
            };
            if !place.projection.is_empty() || place.local.as_u32() == 0 {
                return Err(ReferenceBindingErrorV1::at(
                    tcx,
                    body,
                    block,
                    "slice metadata source is not one direct reference argument",
                ));
            }
            Ok(ReferenceValueV1::InputLength {
                reference_argument: place.local.as_u32() - 1,
            })
        }
        Rvalue::UnaryOp(operation, operand) => Ok(ReferenceValueV1::Unary {
            operation: match operation {
                UnOp::Not => ReferenceUnaryOpV1::Not,
                UnOp::Neg => ReferenceUnaryOpV1::Negate,
                UnOp::PtrMetadata => unreachable!(),
            },
            operand: lower_operand_v1(tcx, body, operand, block)?,
        }),
        Rvalue::Cast(kind, operand, target) => {
            let source = scalar_type_v1(operand.ty(body, tcx)).ok_or_else(|| {
                ReferenceBindingErrorV1::at(
                    tcx,
                    body,
                    block,
                    "cast source is outside reference-effect scalar semantics",
                )
            })?;
            let target = scalar_type_v1(*target).ok_or_else(|| {
                ReferenceBindingErrorV1::at(
                    tcx,
                    body,
                    block,
                    "cast target is outside reference-effect scalar semantics",
                )
            })?;
            let kind = match kind {
                CastKind::IntToInt => ReferenceCastKindV1::Integer,
                CastKind::IntToFloat => ReferenceCastKindV1::IntegerToFloat,
                CastKind::FloatToFloat => ReferenceCastKindV1::FloatToFloat,
                CastKind::FloatToInt => ReferenceCastKindV1::FloatToIntegerSaturating,
                _ => {
                    return Err(ReferenceBindingErrorV1::at(
                        tcx,
                        body,
                        block,
                        format_args!(
                            "cast operation '{kind:?}' is outside reference-effect V2 numeric semantics"
                        ),
                    ));
                }
            };
            Ok(ReferenceValueV1::Cast {
                kind,
                source,
                target,
                operand: lower_operand_v1(tcx, body, operand, block)?,
            })
        }
        Rvalue::Ref(..) => Err(ReferenceBindingErrorV1::at(
            tcx,
            body,
            block,
            "references constructed for helper memory access are outside pure scalar helper summaries",
        )),
        unsupported => Err(ReferenceBindingErrorV1::at(
            tcx,
            body,
            block,
            format_args!("rvalue '{unsupported:?}' is outside reference-effect V1"),
        )),
    }
}

fn lower_binary_op_v1(operation: BinOp) -> Option<ReferenceBinaryOpV1> {
    Some(match operation {
        BinOp::Add | BinOp::AddWithOverflow => ReferenceBinaryOpV1::Add,
        BinOp::Sub | BinOp::SubWithOverflow => ReferenceBinaryOpV1::Subtract,
        BinOp::Mul | BinOp::MulWithOverflow => ReferenceBinaryOpV1::Multiply,
        BinOp::Div => ReferenceBinaryOpV1::Divide,
        BinOp::Rem => ReferenceBinaryOpV1::Remainder,
        BinOp::BitXor => ReferenceBinaryOpV1::BitXor,
        BinOp::BitAnd => ReferenceBinaryOpV1::BitAnd,
        BinOp::BitOr => ReferenceBinaryOpV1::BitOr,
        BinOp::Shl => ReferenceBinaryOpV1::ShiftLeft,
        BinOp::Shr => ReferenceBinaryOpV1::ShiftRight,
        BinOp::Eq => ReferenceBinaryOpV1::Equal,
        BinOp::Lt => ReferenceBinaryOpV1::LessThan,
        BinOp::Le => ReferenceBinaryOpV1::LessEqual,
        BinOp::Ne => ReferenceBinaryOpV1::NotEqual,
        BinOp::Ge => ReferenceBinaryOpV1::GreaterEqual,
        BinOp::Gt => ReferenceBinaryOpV1::GreaterThan,
        _ => return None,
    })
}

fn put_len(digest: &mut Sha256, length: usize) {
    digest.update(u64::try_from(length).unwrap_or(u64::MAX).to_le_bytes());
}

fn scalar_tag(scalar: ReferenceScalarTypeV1) -> u8 {
    match scalar {
        ReferenceScalarTypeV1::Bool => 0,
        ReferenceScalarTypeV1::U8 => 1,
        ReferenceScalarTypeV1::U16 => 2,
        ReferenceScalarTypeV1::U32 => 3,
        ReferenceScalarTypeV1::U64 => 4,
        ReferenceScalarTypeV1::Usize => 5,
        ReferenceScalarTypeV1::I8 => 6,
        ReferenceScalarTypeV1::I16 => 7,
        ReferenceScalarTypeV1::I32 => 8,
        ReferenceScalarTypeV1::I64 => 9,
        ReferenceScalarTypeV1::Isize => 10,
        ReferenceScalarTypeV1::F32 => 11,
        ReferenceScalarTypeV1::F64 => 12,
    }
}

fn digest_place(digest: &mut Sha256, place: &ReferencePlaceV1) {
    digest.update(place.local.to_le_bytes());
    put_len(digest, place.projection.len());
    for projection in &place.projection {
        match projection {
            ReferencePlaceProjectionV1::Dereference => digest.update([0]),
            ReferencePlaceProjectionV1::Field(field) => {
                digest.update([1]);
                digest.update(field.to_le_bytes());
            }
            ReferencePlaceProjectionV1::Index(local) => {
                digest.update([2]);
                digest.update(local.to_le_bytes());
            }
            ReferencePlaceProjectionV1::ConstantIndex {
                offset,
                minimum_length,
                from_end,
            } => {
                digest.update([3, u8::from(*from_end)]);
                digest.update(offset.to_le_bytes());
                digest.update(minimum_length.to_le_bytes());
            }
        }
    }
}

fn digest_operand(digest: &mut Sha256, operand: &ReferenceOperandV1) {
    match operand {
        ReferenceOperandV1::Copy(place) => {
            digest.update([0]);
            digest_place(digest, place);
        }
        ReferenceOperandV1::Move(place) => {
            digest.update([1]);
            digest_place(digest, place);
        }
        ReferenceOperandV1::Constant(ReferenceConstantV1::ZeroSized) => {
            digest.update([2]);
        }
        ReferenceOperandV1::Constant(ReferenceConstantV1::Scalar { scalar, bits }) => {
            digest.update([3, scalar_tag(*scalar)]);
            digest.update(bits.to_le_bytes());
        }
    }
}

fn digest_value(digest: &mut Sha256, value: &ReferenceValueV1) {
    match value {
        ReferenceValueV1::Use(operand) => {
            digest.update([0]);
            digest_operand(digest, operand);
        }
        ReferenceValueV1::Binary {
            operation,
            lhs,
            rhs,
            checked,
        } => {
            digest.update([1, u8::from(*checked), binary_tag(*operation)]);
            digest_operand(digest, lhs);
            digest_operand(digest, rhs);
        }
        ReferenceValueV1::Unary { operation, operand } => {
            digest.update([
                2,
                match operation {
                    ReferenceUnaryOpV1::Not => 0,
                    ReferenceUnaryOpV1::Negate => 1,
                },
            ]);
            digest_operand(digest, operand);
        }
        ReferenceValueV1::Cast {
            kind,
            source,
            target,
            operand,
        } => {
            digest.update([3, *kind as u8, scalar_tag(*source), scalar_tag(*target)]);
            digest_operand(digest, operand);
        }
        ReferenceValueV1::SafeHelperCall {
            helper,
            parameters,
            result,
            arguments,
            summary,
        } => {
            digest.update([4]);
            digest_function_identity_v2(digest, helper);
            put_len(digest, parameters.len());
            for parameter in parameters {
                digest.update([scalar_tag(*parameter)]);
            }
            digest.update([scalar_tag(*result)]);
            put_len(digest, arguments.len());
            for argument in arguments {
                digest_operand(digest, argument);
            }
            digest_effect_expression_v1(digest, summary);
        }
        ReferenceValueV1::InputLength { reference_argument } => {
            digest.update([5]);
            digest.update(reference_argument.to_le_bytes());
        }
    }
}

fn digest_function_identity_v2(digest: &mut Sha256, identity: &ReferenceFunctionIdentityV1) {
    digest.update(identity.def_path_hash);
    digest.update(identity.function_sha256);
    digest.update(identity.item_definition_sha256);
    digest.update(identity.monomorphization_sha256);
    digest.update(identity.generic_type_arguments_sha256);
    digest.update(identity.const_generic_arguments_sha256);
    digest.update(identity.rustc_mir_body_sha256);
}

fn digest_effect_expression_v1(digest: &mut Sha256, expression: &ReferenceEffectExpressionV1) {
    match expression {
        ReferenceEffectExpressionV1::PointCoordinate { axis } => {
            digest.update([0]);
            digest.update(axis.to_le_bytes());
        }
        ReferenceEffectExpressionV1::KernelScalarArgument { argument } => {
            digest.update([1]);
            digest.update(argument.to_le_bytes());
        }
        ReferenceEffectExpressionV1::Constant(constant) => {
            digest.update([2]);
            digest_operand(digest, &ReferenceOperandV1::Constant(constant.clone()));
        }
        ReferenceEffectExpressionV1::Binary {
            operation,
            lhs,
            rhs,
            checked,
        } => {
            digest.update([3, binary_tag(*operation), u8::from(*checked)]);
            digest_effect_expression_v1(digest, lhs);
            digest_effect_expression_v1(digest, rhs);
        }
        ReferenceEffectExpressionV1::Unary { operation, operand } => {
            digest.update([
                4,
                match operation {
                    ReferenceUnaryOpV1::Not => 0,
                    ReferenceUnaryOpV1::Negate => 1,
                },
            ]);
            digest_effect_expression_v1(digest, operand);
        }
        ReferenceEffectExpressionV1::InputLoad {
            reference_argument,
            index,
        } => {
            digest.update([6]);
            digest.update(reference_argument.to_le_bytes());
            digest_effect_expression_v1(digest, index);
        }
        ReferenceEffectExpressionV1::InputLength { reference_argument } => {
            digest.update([7]);
            digest.update(reference_argument.to_le_bytes());
        }
        ReferenceEffectExpressionV1::Cast {
            kind,
            source,
            target,
            operand,
        } => {
            digest.update([5, *kind as u8, scalar_tag(*source), scalar_tag(*target)]);
            digest_effect_expression_v1(digest, operand);
        }
    }
}

fn digest_path_predicate_v1(digest: &mut Sha256, predicate: &ReferencePathPredicateV1) {
    put_len(digest, predicate.clauses.len());
    for clause in &predicate.clauses {
        put_len(digest, clause.atoms.len());
        for atom in &clause.atoms {
            match atom {
                ReferenceGuardAtomV1::SwitchValueSet {
                    discriminant,
                    values,
                    inside_set,
                } => {
                    digest.update([0, u8::from(*inside_set)]);
                    digest_effect_expression_v1(digest, discriminant);
                    put_len(digest, values.len());
                    for value in values {
                        digest.update(value.to_le_bytes());
                    }
                }
                ReferenceGuardAtomV1::Assert {
                    condition,
                    expected,
                } => {
                    digest.update([1, u8::from(*expected)]);
                    digest_effect_expression_v1(digest, condition);
                }
            }
        }
    }
}

fn digest_output_effect_v1(digest: &mut Sha256, effect: &ReferenceOutputWriteV1) {
    digest.update(effect.argument.to_le_bytes());
    digest.update(effect.block.to_le_bytes());
    digest.update(effect.statement.to_le_bytes());
    match &effect.coordinate {
        ReferenceOutputCoordinateV1::LogicalPoint(axes) => {
            digest.update([0]);
            put_len(digest, axes.len());
            for axis in axes {
                digest_effect_expression_v1(digest, axis);
            }
        }
        ReferenceOutputCoordinateV1::SingleCoordinate => digest.update([1]),
        ReferenceOutputCoordinateV1::Dynamic(expression) => {
            digest.update([2]);
            digest_effect_expression_v1(digest, expression);
        }
        ReferenceOutputCoordinateV1::Constant {
            offset,
            minimum_length,
            from_end,
        } => {
            digest.update([3, u8::from(*from_end)]);
            digest.update(offset.to_le_bytes());
            digest.update(minimum_length.to_le_bytes());
        }
    }
    digest_path_predicate_v1(digest, &effect.guard);
    digest_effect_expression_v1(digest, &effect.rhs);
}

fn binary_tag(operation: ReferenceBinaryOpV1) -> u8 {
    match operation {
        ReferenceBinaryOpV1::Add => 0,
        ReferenceBinaryOpV1::Subtract => 1,
        ReferenceBinaryOpV1::Multiply => 2,
        ReferenceBinaryOpV1::Divide => 3,
        ReferenceBinaryOpV1::Remainder => 4,
        ReferenceBinaryOpV1::BitXor => 5,
        ReferenceBinaryOpV1::BitAnd => 6,
        ReferenceBinaryOpV1::BitOr => 7,
        ReferenceBinaryOpV1::ShiftLeft => 8,
        ReferenceBinaryOpV1::ShiftRight => 9,
        ReferenceBinaryOpV1::Equal => 10,
        ReferenceBinaryOpV1::LessThan => 11,
        ReferenceBinaryOpV1::LessEqual => 12,
        ReferenceBinaryOpV1::NotEqual => 13,
        ReferenceBinaryOpV1::GreaterEqual => 14,
        ReferenceBinaryOpV1::GreaterThan => 15,
    }
}

fn digest_terminator(digest: &mut Sha256, terminator: &ReferenceTerminatorV1) {
    match terminator {
        ReferenceTerminatorV1::Return => digest.update([0]),
        ReferenceTerminatorV1::Goto { target } => {
            digest.update([1]);
            digest.update(target.to_le_bytes());
        }
        ReferenceTerminatorV1::Switch {
            discriminant,
            values,
            otherwise,
        } => {
            digest.update([2]);
            digest_operand(digest, discriminant);
            put_len(digest, values.len());
            for (value, target) in values {
                digest.update(value.to_le_bytes());
                digest.update(target.to_le_bytes());
            }
            digest.update(otherwise.to_le_bytes());
        }
        ReferenceTerminatorV1::Assert {
            condition,
            expected,
            success,
            bounds_check,
        } => {
            digest.update([3, u8::from(*expected), u8::from(*bounds_check)]);
            digest_operand(digest, condition);
            digest.update(success.to_le_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scalar_constant(bits: u128) -> ReferenceConstantV1 {
        ReferenceConstantV1::Scalar {
            scalar: ReferenceScalarTypeV1::U32,
            bits,
        }
    }

    fn scalar_operand(bits: u128) -> ReferenceOperandV1 {
        ReferenceOperandV1::Constant(scalar_constant(bits))
    }

    fn output_assignment(projection: Vec<ReferencePlaceProjectionV1>) -> ReferenceAssignmentV1 {
        ReferenceAssignmentV1 {
            statement: 0,
            destination: ReferencePlaceV1 {
                local: 3,
                projection: projection.into_boxed_slice(),
            },
            value: ReferenceValueV1::Use(scalar_operand(17)),
        }
    }

    fn guarded_point_reference_ir(
        output_projection: Vec<ReferencePlaceProjectionV1>,
    ) -> ReferenceEffectIrV1 {
        ReferenceEffectIrV1 {
            argument_count: 3,
            local_count: 4,
            relations: vec![
                ReferenceArgumentRelationV1::PointCoordinate {
                    reference_argument: 0,
                    axis: 0,
                },
                ReferenceArgumentRelationV1::ScalarInput {
                    argument: 0,
                    scalar: ReferenceScalarTypeV1::Bool,
                },
                ReferenceArgumentRelationV1::DisjointOutputCoordinate {
                    argument: 1,
                    element: ReferenceScalarTypeV1::U32,
                },
            ]
            .into_boxed_slice(),
            blocks: vec![
                ReferenceBlockV1 {
                    block: 0,
                    assignments: Box::default(),
                    terminator: ReferenceTerminatorV1::Switch {
                        discriminant: ReferenceOperandV1::Copy(ReferencePlaceV1 {
                            local: 2,
                            projection: Box::default(),
                        }),
                        values: vec![(0, 2)].into_boxed_slice(),
                        otherwise: 1,
                    },
                },
                ReferenceBlockV1 {
                    block: 1,
                    assignments: vec![output_assignment(output_projection)].into_boxed_slice(),
                    terminator: ReferenceTerminatorV1::Return,
                },
                ReferenceBlockV1 {
                    block: 2,
                    assignments: Box::default(),
                    terminator: ReferenceTerminatorV1::Return,
                },
            ]
            .into_boxed_slice(),
            loop_summaries: Box::default(),
            observable_output_effects: Box::default(),
        }
    }

    #[test]
    fn derives_point_coordinate_guard_and_rhs_from_reference_ir() {
        let effect_ir = guarded_point_reference_ir(vec![ReferencePlaceProjectionV1::Dereference]);
        let writes = effect_ir.observable_output_writes_v1().unwrap();
        assert_eq!(writes.len(), 1);
        assert_eq!(
            writes[0].coordinate,
            ReferenceOutputCoordinateV1::LogicalPoint(
                vec![ReferenceEffectExpressionV1::PointCoordinate { axis: 0 }].into_boxed_slice(),
            )
        );
        assert_eq!(
            writes[0].rhs,
            ReferenceEffectExpressionV1::Constant(scalar_constant(17))
        );
        assert_eq!(
            writes[0].guard,
            ReferencePathPredicateV1 {
                clauses: vec![ReferenceGuardClauseV1 {
                    atoms: vec![ReferenceGuardAtomV1::SwitchValueSet {
                        discriminant: ReferenceEffectExpressionV1::KernelScalarArgument {
                            argument: 0,
                        },
                        values: vec![0].into_boxed_slice(),
                        inside_set: false,
                    }]
                    .into_boxed_slice(),
                }]
                .into_boxed_slice(),
            }
        );
    }

    #[test]
    fn refuses_to_omit_an_unsupported_observable_output_projection() {
        let effect_ir = guarded_point_reference_ir(vec![
            ReferencePlaceProjectionV1::Dereference,
            ReferencePlaceProjectionV1::Field(0),
        ]);
        let error = effect_ir.observable_output_writes_v1().unwrap_err();
        assert!(
            error
                .to_string()
                .contains("cannot omit a global output write")
        );
    }

    #[test]
    fn derives_multiple_observable_output_effects_without_collapsing_arguments() {
        let effect_ir = ReferenceEffectIrV1 {
            argument_count: 3,
            local_count: 4,
            relations: vec![
                ReferenceArgumentRelationV1::PointCoordinate {
                    reference_argument: 0,
                    axis: 0,
                },
                ReferenceArgumentRelationV1::DisjointOutputCoordinate {
                    argument: 0,
                    element: ReferenceScalarTypeV1::U32,
                },
                ReferenceArgumentRelationV1::DisjointOutputCoordinate {
                    argument: 1,
                    element: ReferenceScalarTypeV1::U32,
                },
            ]
            .into_boxed_slice(),
            blocks: vec![ReferenceBlockV1 {
                block: 0,
                assignments: vec![
                    ReferenceAssignmentV1 {
                        statement: 0,
                        destination: ReferencePlaceV1 {
                            local: 2,
                            projection: vec![ReferencePlaceProjectionV1::Dereference]
                                .into_boxed_slice(),
                        },
                        value: ReferenceValueV1::Use(scalar_operand(17)),
                    },
                    ReferenceAssignmentV1 {
                        statement: 1,
                        destination: ReferencePlaceV1 {
                            local: 3,
                            projection: vec![ReferencePlaceProjectionV1::Dereference]
                                .into_boxed_slice(),
                        },
                        value: ReferenceValueV1::Use(scalar_operand(23)),
                    },
                ]
                .into_boxed_slice(),
                terminator: ReferenceTerminatorV1::Return,
            }]
            .into_boxed_slice(),
            loop_summaries: Box::default(),
            observable_output_effects: Box::default(),
        };
        let writes = effect_ir.observable_output_writes_v1().unwrap();
        assert_eq!(writes.len(), 2);
        assert_eq!(
            writes
                .iter()
                .map(|write| write.argument)
                .collect::<Vec<_>>(),
            [0, 1],
        );
        assert!(writes.iter().all(|write| matches!(
            write.coordinate,
            ReferenceOutputCoordinateV1::LogicalPoint(_)
        )));
    }

    fn alias_chain_reference_ir(local_count: u32) -> ReferenceEffectIrV1 {
        let assignments = (1..=local_count)
            .map(|local| ReferenceAssignmentV1 {
                statement: local - 1,
                destination: ReferencePlaceV1 {
                    local,
                    projection: Box::default(),
                },
                value: if local == local_count {
                    ReferenceValueV1::Use(scalar_operand(17))
                } else {
                    ReferenceValueV1::Use(ReferenceOperandV1::Copy(ReferencePlaceV1 {
                        local: local + 1,
                        projection: Box::default(),
                    }))
                },
            })
            .collect::<Vec<_>>();
        ReferenceEffectIrV1 {
            argument_count: 0,
            local_count: local_count + 1,
            relations: Box::default(),
            blocks: vec![ReferenceBlockV1 {
                block: 0,
                assignments: assignments.into_boxed_slice(),
                terminator: ReferenceTerminatorV1::Return,
            }]
            .into_boxed_slice(),
            loop_summaries: Box::default(),
            observable_output_effects: Box::default(),
        }
    }

    #[test]
    fn reference_expression_resolution_enforces_128_level_depth_budget() {
        let boundary = alias_chain_reference_ir(128);
        assert_eq!(
            ReferenceExpressionResolverV1::new(&boundary)
                .unwrap()
                .resolve_local_v1(1)
                .unwrap(),
            ReferenceEffectExpressionV1::Constant(scalar_constant(17)),
        );
        for depth in [129, 4_096] {
            let overdeep = alias_chain_reference_ir(depth);
            let error = ReferenceExpressionResolverV1::new(&overdeep)
                .unwrap()
                .resolve_local_v1(1)
                .unwrap_err();
            assert!(error.to_string().contains("exceeds 128 resolution levels"));
        }
    }

    fn local(local: u32) -> ReferenceOperandV1 {
        ReferenceOperandV1::Copy(ReferencePlaceV1 {
            local,
            projection: Box::default(),
        })
    }

    fn checked_field(local: u32, field: u32) -> ReferenceOperandV1 {
        ReferenceOperandV1::Move(ReferencePlaceV1 {
            local,
            projection: vec![ReferencePlaceProjectionV1::Field(field)].into_boxed_slice(),
        })
    }

    fn assign(local: u32, statement: u32, value: ReferenceValueV1) -> ReferenceAssignmentV1 {
        ReferenceAssignmentV1 {
            statement,
            destination: ReferencePlaceV1 {
                local,
                projection: Box::default(),
            },
            value,
        }
    }

    fn counted_loop_reference_ir(upper_bound: ReferenceOperandV1) -> ReferenceEffectIrV1 {
        ReferenceEffectIrV1 {
            argument_count: if matches!(upper_bound, ReferenceOperandV1::Copy(_)) {
                2
            } else {
                1
            },
            local_count: 8,
            relations: if matches!(upper_bound, ReferenceOperandV1::Copy(_)) {
                vec![
                    ReferenceArgumentRelationV1::ScalarInput {
                        argument: 0,
                        scalar: ReferenceScalarTypeV1::U32,
                    },
                    ReferenceArgumentRelationV1::DisjointOutputCoordinate {
                        argument: 1,
                        element: ReferenceScalarTypeV1::U32,
                    },
                ]
            } else {
                vec![ReferenceArgumentRelationV1::DisjointOutputCoordinate {
                    argument: 0,
                    element: ReferenceScalarTypeV1::U32,
                }]
            }
            .into_boxed_slice(),
            blocks: vec![
                ReferenceBlockV1 {
                    block: 0,
                    assignments: vec![
                        assign(3, 0, ReferenceValueV1::Use(scalar_operand(0))),
                        assign(4, 1, ReferenceValueV1::Use(scalar_operand(11))),
                    ]
                    .into_boxed_slice(),
                    terminator: ReferenceTerminatorV1::Goto { target: 1 },
                },
                ReferenceBlockV1 {
                    block: 1,
                    assignments: vec![assign(
                        5,
                        0,
                        ReferenceValueV1::Binary {
                            operation: ReferenceBinaryOpV1::LessThan,
                            lhs: local(3),
                            rhs: upper_bound.clone(),
                            checked: false,
                        },
                    )]
                    .into_boxed_slice(),
                    terminator: ReferenceTerminatorV1::Switch {
                        discriminant: local(5),
                        values: vec![(0, 5)].into_boxed_slice(),
                        otherwise: 2,
                    },
                },
                ReferenceBlockV1 {
                    block: 2,
                    assignments: vec![assign(
                        6,
                        0,
                        ReferenceValueV1::Binary {
                            operation: ReferenceBinaryOpV1::Add,
                            lhs: local(4),
                            rhs: local(3),
                            checked: true,
                        },
                    )]
                    .into_boxed_slice(),
                    terminator: ReferenceTerminatorV1::Assert {
                        condition: checked_field(6, 1),
                        expected: false,
                        success: 3,
                        bounds_check: false,
                    },
                },
                ReferenceBlockV1 {
                    block: 3,
                    assignments: vec![
                        assign(4, 0, ReferenceValueV1::Use(checked_field(6, 0))),
                        assign(
                            7,
                            1,
                            ReferenceValueV1::Binary {
                                operation: ReferenceBinaryOpV1::Add,
                                lhs: local(3),
                                rhs: scalar_operand(1),
                                checked: true,
                            },
                        ),
                    ]
                    .into_boxed_slice(),
                    terminator: ReferenceTerminatorV1::Assert {
                        condition: checked_field(7, 1),
                        expected: false,
                        success: 4,
                        bounds_check: false,
                    },
                },
                ReferenceBlockV1 {
                    block: 4,
                    assignments: vec![assign(3, 0, ReferenceValueV1::Use(checked_field(7, 0)))]
                        .into_boxed_slice(),
                    terminator: ReferenceTerminatorV1::Goto { target: 1 },
                },
                ReferenceBlockV1 {
                    block: 5,
                    assignments: vec![ReferenceAssignmentV1 {
                        statement: 0,
                        destination: ReferencePlaceV1 {
                            local: if matches!(upper_bound, ReferenceOperandV1::Copy(_)) {
                                2
                            } else {
                                1
                            },
                            projection: vec![ReferencePlaceProjectionV1::Dereference]
                                .into_boxed_slice(),
                        },
                        value: ReferenceValueV1::Use(local(4)),
                    }]
                    .into_boxed_slice(),
                    terminator: ReferenceTerminatorV1::Return,
                },
            ]
            .into_boxed_slice(),
            loop_summaries: Box::default(),
            observable_output_effects: Box::default(),
        }
    }

    #[test]
    fn exact_counted_loop_derives_loop_carried_value_and_recurrence_identity() {
        let effect_ir = counted_loop_reference_ir(scalar_operand(4));
        let backedges = reference_cfg_backedges_v2(&effect_ir).unwrap();
        assert_eq!(backedges, BTreeSet::from([(4, 1)]));
        let (writes, summaries) = effect_ir
            .observable_output_writes_with_loops_v2(&backedges)
            .unwrap();
        assert_eq!(writes.len(), 1);
        assert_eq!(
            writes[0].rhs,
            ReferenceEffectExpressionV1::Constant(scalar_constant(17)),
        );
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].exact_iterations, Some(4));
        assert_eq!(summaries[0].maximum_iterations, 4);
        assert_eq!(summaries[0].carried_locals.as_ref(), &[3, 4]);
        assert_ne!(summaries[0].transition_sha256, [0; 32]);
        assert_ne!(summaries[0].variant_sha256, [0; 32]);
    }

    #[test]
    fn dynamic_loop_with_additional_carried_state_fails_closed() {
        let effect_ir = counted_loop_reference_ir(local(1));
        let backedges = reference_cfg_backedges_v2(&effect_ir).unwrap();
        let error = effect_ir
            .observable_output_writes_with_loops_v2(&backedges)
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("mutates another loop-carried local"),
            "{error}"
        );
    }

    #[test]
    fn dynamic_induction_only_loop_uses_the_unsigned_type_bound() {
        let effect_ir = ReferenceEffectIrV1 {
            argument_count: 2,
            local_count: 6,
            relations: vec![
                ReferenceArgumentRelationV1::ScalarInput {
                    argument: 0,
                    scalar: ReferenceScalarTypeV1::U32,
                },
                ReferenceArgumentRelationV1::DisjointOutputCoordinate {
                    argument: 1,
                    element: ReferenceScalarTypeV1::U32,
                },
            ]
            .into_boxed_slice(),
            blocks: vec![
                ReferenceBlockV1 {
                    block: 0,
                    assignments: vec![assign(3, 0, ReferenceValueV1::Use(scalar_operand(0)))]
                        .into_boxed_slice(),
                    terminator: ReferenceTerminatorV1::Goto { target: 1 },
                },
                ReferenceBlockV1 {
                    block: 1,
                    assignments: vec![assign(
                        4,
                        0,
                        ReferenceValueV1::Binary {
                            operation: ReferenceBinaryOpV1::LessThan,
                            lhs: local(3),
                            rhs: local(1),
                            checked: false,
                        },
                    )]
                    .into_boxed_slice(),
                    terminator: ReferenceTerminatorV1::Switch {
                        discriminant: local(4),
                        values: vec![(0, 4)].into_boxed_slice(),
                        otherwise: 2,
                    },
                },
                ReferenceBlockV1 {
                    block: 2,
                    assignments: vec![assign(
                        5,
                        0,
                        ReferenceValueV1::Binary {
                            operation: ReferenceBinaryOpV1::Add,
                            lhs: local(3),
                            rhs: scalar_operand(1),
                            checked: true,
                        },
                    )]
                    .into_boxed_slice(),
                    terminator: ReferenceTerminatorV1::Assert {
                        condition: checked_field(5, 1),
                        expected: false,
                        success: 3,
                        bounds_check: false,
                    },
                },
                ReferenceBlockV1 {
                    block: 3,
                    assignments: vec![assign(3, 0, ReferenceValueV1::Use(checked_field(5, 0)))]
                        .into_boxed_slice(),
                    terminator: ReferenceTerminatorV1::Goto { target: 1 },
                },
                ReferenceBlockV1 {
                    block: 4,
                    assignments: vec![ReferenceAssignmentV1 {
                        statement: 0,
                        destination: ReferencePlaceV1 {
                            local: 2,
                            projection: vec![ReferencePlaceProjectionV1::Dereference]
                                .into_boxed_slice(),
                        },
                        value: ReferenceValueV1::Use(local(3)),
                    }]
                    .into_boxed_slice(),
                    terminator: ReferenceTerminatorV1::Return,
                },
            ]
            .into_boxed_slice(),
            loop_summaries: Box::default(),
            observable_output_effects: Box::default(),
        };
        let backedges = reference_cfg_backedges_v2(&effect_ir).unwrap();
        let (writes, summaries) = effect_ir
            .observable_output_writes_with_loops_v2(&backedges)
            .unwrap();
        assert_eq!(
            writes[0].rhs,
            ReferenceEffectExpressionV1::KernelScalarArgument { argument: 0 }
        );
        assert_eq!(summaries[0].exact_iterations, None);
        assert_eq!(summaries[0].maximum_iterations, u64::from(u32::MAX));
    }

    #[test]
    fn helper_summary_substitutes_exact_call_arguments() {
        let mut effect_ir = alias_chain_reference_ir(2);
        effect_ir.blocks[0].assignments[0].value = ReferenceValueV1::SafeHelperCall {
            helper: ReferenceFunctionIdentityV1 {
                def_path_hash: [1; 16],
                function_sha256: [2; 32],
                item_definition_sha256: [3; 32],
                monomorphization_sha256: [4; 32],
                generic_type_arguments_sha256: [5; 32],
                const_generic_arguments_sha256: [6; 32],
                rustc_mir_body_sha256: [7; 32],
            },
            parameters: vec![ReferenceScalarTypeV1::U32].into_boxed_slice(),
            result: ReferenceScalarTypeV1::U32,
            arguments: vec![scalar_operand(16)].into_boxed_slice(),
            summary: Box::new(ReferenceEffectExpressionV1::Binary {
                operation: ReferenceBinaryOpV1::Add,
                lhs: Box::new(ReferenceEffectExpressionV1::KernelScalarArgument { argument: 0 }),
                rhs: Box::new(ReferenceEffectExpressionV1::Constant(scalar_constant(1))),
                checked: true,
            }),
        };
        assert_eq!(
            ReferenceExpressionResolverV1::new(&effect_ir)
                .unwrap()
                .resolve_local_v1(1)
                .unwrap(),
            ReferenceEffectExpressionV1::Binary {
                operation: ReferenceBinaryOpV1::Add,
                lhs: Box::new(ReferenceEffectExpressionV1::Constant(scalar_constant(16))),
                rhs: Box::new(ReferenceEffectExpressionV1::Constant(scalar_constant(1))),
                checked: true,
            },
        );
    }

    #[test]
    fn projected_slice_read_retains_its_exact_argument_and_index() {
        let environment = BTreeMap::from([(
            2,
            ReferenceSymbolicValueV2::Scalar(ReferenceEffectExpressionV1::PointCoordinate {
                axis: 0,
            }),
        )]);
        let value = symbolic_operand_v2(
            &environment,
            &ReferenceOperandV1::Copy(ReferencePlaceV1 {
                local: 1,
                projection: vec![
                    ReferencePlaceProjectionV1::Dereference,
                    ReferencePlaceProjectionV1::Index(2),
                ]
                .into_boxed_slice(),
            }),
        )
        .unwrap();
        assert_eq!(
            value,
            ReferenceSymbolicValueV2::Scalar(ReferenceEffectExpressionV1::InputLoad {
                reference_argument: 0,
                index: Box::new(ReferenceEffectExpressionV1::PointCoordinate { axis: 0 }),
            })
        );
    }

    #[test]
    fn unproved_checked_overflow_is_rejected() {
        let mut environment = BTreeMap::new();
        environment.insert(
            3,
            ReferenceSymbolicValueV2::CheckedPair {
                value: ReferenceEffectExpressionV1::KernelScalarArgument { argument: 0 },
                overflowed: None,
            },
        );
        let error = symbolic_operand_v2(&environment, &checked_field(3, 1)).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("provide an authenticated range fact")
        );
    }

    #[test]
    fn loop_iteration_resource_bound_fails_closed() {
        let mut state = ReferenceSymbolicStateV2 {
            block: 4,
            environment: BTreeMap::new(),
            guard: ReferencePathPredicateV1::unconditional_v1(),
            traces: BTreeMap::from([(
                (1, 4),
                ReferenceLoopTraceV2 {
                    header: 1,
                    latch: 4,
                    exit: None,
                    initial: BTreeMap::new(),
                    transitions: vec![BTreeMap::new(); MAX_REFERENCE_LOOP_ITERATIONS_V2],
                    variants: Vec::new(),
                    exact_iterations: None,
                    maximum_iterations: None,
                },
            )]),
        };
        let error = dispatch_symbolic_edge_v2(
            &mut VecDeque::new(),
            state.clone(),
            4,
            1,
            &BTreeSet::from([(4, 1)]),
            &BTreeMap::from([((4, 1), BTreeSet::from([1, 4]))]),
            &mut ReferenceSymbolicWorkBudgetV2::default(),
        )
        .unwrap_err();
        assert!(error.to_string().contains("exceeds 4096 iterations"));
        state.traces.clear();
    }

    #[test]
    fn overlapping_or_nested_loop_regions_fail_before_symbolic_execution() {
        let mut overlapping = BTreeMap::new();
        overlapping.insert((2, 1), BTreeSet::from([1, 2, 3]));
        overlapping.insert((4, 3), BTreeSet::from([3, 4]));
        let error = reject_overlapping_reference_loops_v2(&overlapping).unwrap_err();
        assert!(error.to_string().contains("overlap or nest"));

        let disjoint = BTreeMap::from([
            ((2, 1), BTreeSet::from([1, 2])),
            ((4, 3), BTreeSet::from([3, 4])),
        ]);
        reject_overlapping_reference_loops_v2(&disjoint).unwrap();
    }

    #[test]
    fn symbolic_unrolling_checks_depth_before_recursive_hashing() {
        let mut expression = ReferenceEffectExpressionV1::Constant(ReferenceConstantV1::Scalar {
            scalar: ReferenceScalarTypeV1::U64,
            bits: 1,
        });
        for _ in 0..=fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 {
            expression = ReferenceEffectExpressionV1::Unary {
                operation: ReferenceUnaryOpV1::Not,
                operand: Box::new(expression),
            };
        }
        let error = require_symbolic_expression_budget_v2(&expression).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("symbolic expression exceeds depth")
        );
    }

    #[test]
    fn symbolic_execution_has_one_cumulative_expression_work_budget() {
        let mut budget = ReferenceSymbolicWorkBudgetV2 {
            charged_nodes: MAX_REFERENCE_SYMBOLIC_WORK_NODES_V2,
        };
        let error = budget
            .charge_expression_v2(&ReferenceEffectExpressionV1::Constant(scalar_constant(1)))
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("cumulative expression work nodes")
        );
    }
}
