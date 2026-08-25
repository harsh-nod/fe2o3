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
    BinOp, Body, CastKind, Operand, Place, ProjectionElem, Rvalue, StatementKind, TerminatorKind,
    UnOp, UnwindAction,
};
use rustc_middle::ty::{Instance, Ty, TyCtxt, TyKind, TypingEnv};
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
    /// Compiler-derived point effects. This is per-effect partial correctness
    /// evidence; it does not assert that a dynamic output view is totally
    /// covered by the kernel.
    pub(crate) observable_output_effects: Box<[ReferenceOutputWriteV1]>,
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

#[derive(Clone, Debug, Eq, PartialEq)]
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
        put_len(&mut digest, self.observable_output_effects.len());
        for effect in &self.observable_output_effects {
            digest_output_effect_v1(&mut digest, effect);
        }
        digest.finalize().into()
    }
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
    reject_cycles_v1(tcx, body)?;
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
                unwind,
                ..
            } if matches!(unwind, UnwindAction::Unreachable) => ReferenceTerminatorV1::Assert {
                condition: lower_operand_v1(tcx, body, cond, block_index)?,
                expected: *expected,
                success: target.as_u32(),
            },
            TerminatorKind::Call { .. } => {
                return Err(ReferenceBindingErrorV1::at(
                    tcx,
                    body,
                    block_index,
                    "function calls are outside reference-effect V1; inline the operation or use a supported scalar expression",
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
        observable_output_effects: Box::default(),
    };
    effect_ir.observable_output_effects =
        effect_ir.observable_output_writes_v1()?.into_boxed_slice();
    Ok(effect_ir)
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
        }
    }
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
        } => Ok(vec![(
            *success,
            Some(ReferenceGuardAtomV1::Assert {
                condition: resolver.resolve_operand_inner_v1(
                    condition,
                    &mut BTreeSet::new(),
                    &mut 0,
                    1,
                )?,
                expected: *expected,
            }),
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
            "loops or backedges are outside reference-effect V1; counted affine reference loops are not yet authenticated",
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
        Rvalue::UnaryOp(operation, operand) => Ok(ReferenceValueV1::Unary {
            operation: match operation {
                UnOp::Not => ReferenceUnaryOpV1::Not,
                UnOp::Neg => ReferenceUnaryOpV1::Negate,
                unsupported => {
                    return Err(ReferenceBindingErrorV1::at(
                        tcx,
                        body,
                        block,
                        format_args!(
                            "unary operation '{unsupported:?}' is outside reference-effect V1"
                        ),
                    ));
                }
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
    }
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
        } => {
            digest.update([3, u8::from(*expected)]);
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
}
