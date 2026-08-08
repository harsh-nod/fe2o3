//! Extension boundary for lowering authenticated source semantics.
//!
//! Imported callees enter this module only after rustc `DefId` recognition has
//! produced an [`AuthenticatedSemanticItem`]. Feature modules must dispatch on
//! that semantic identity, never on diagnostic paths.

mod control_flow;
mod general_v3;

use super::{FunctionLowerer, TranslationDiagnostic, TranslationLocation};
use crate::mir_import::{MirCallee, MirOperandRef, MirPlaceRef, MirRvalueKind, MirTerminatorKind};
use crate::semantic_features::AuthenticatedSemanticItem;
use fe2o3_kernel_ir::{BasicBlock, Terminator};

#[derive(Clone, Copy)]
pub(super) struct AuthenticatedSemanticCall<'call> {
    item: AuthenticatedSemanticItem,
    callee: &'call MirCallee,
    target: usize,
    destination: &'call MirPlaceRef,
    operands: &'call [MirOperandRef],
    location: &'call TranslationLocation,
}

/// An assignment imported from structured rustc MIR.
///
/// Unlike calls, assignments do not need a `DefId` authority. Their operation
/// and operands are authenticated by the typed MIR importer rather than by a
/// source spelling.
#[derive(Clone, Copy)]
pub(super) struct SemanticAssignment<'assignment> {
    rvalue: MirRvalueKind,
    destination: &'assignment MirPlaceRef,
    operands: &'assignment [MirOperandRef],
    location: &'assignment TranslationLocation,
}

impl<'assignment> SemanticAssignment<'assignment> {
    pub(super) fn new(
        rvalue: MirRvalueKind,
        destination: &'assignment MirPlaceRef,
        operands: &'assignment [MirOperandRef],
        location: &'assignment TranslationLocation,
    ) -> Self {
        Self {
            rvalue,
            destination,
            operands,
            location,
        }
    }
}

/// A control-flow operation imported from structured rustc MIR.
#[derive(Clone, Copy)]
pub(super) struct SemanticTerminator<'terminator> {
    kind: &'terminator MirTerminatorKind,
    location: &'terminator TranslationLocation,
}

impl<'terminator> SemanticTerminator<'terminator> {
    pub(super) fn new(
        kind: &'terminator MirTerminatorKind,
        location: &'terminator TranslationLocation,
    ) -> Self {
        Self { kind, location }
    }
}

impl<'call> AuthenticatedSemanticCall<'call> {
    pub(super) fn new(
        callee: &'call MirCallee,
        target: usize,
        destination: &'call MirPlaceRef,
        operands: &'call [MirOperandRef],
        location: &'call TranslationLocation,
    ) -> Option<Self> {
        Some(Self {
            item: callee.authenticated_item()?,
            callee,
            target,
            destination,
            operands,
            location,
        })
    }
}

type LoweringResult<T> = Result<T, TranslationDiagnostic>;

type CallHandler = for<'function, 'declarations, 'call> fn(
    &mut FunctionLowerer<'function, 'declarations>,
    AuthenticatedSemanticCall<'call>,
    &mut BasicBlock,
) -> Option<LoweringResult<Terminator>>;

type AssignmentHandler =
    for<'function, 'declarations, 'assignment> fn(
        &mut FunctionLowerer<'function, 'declarations>,
        SemanticAssignment<'assignment>,
        &mut BasicBlock,
    ) -> Option<LoweringResult<()>>;

type TerminatorHandler =
    for<'function, 'declarations, 'terminator> fn(
        &mut FunctionLowerer<'function, 'declarations>,
        SemanticTerminator<'terminator>,
        &mut BasicBlock,
    ) -> Option<LoweringResult<Terminator>>;

const CALL_HANDLERS: &[CallHandler] = &[general_v3::try_lower_call];
const ASSIGNMENT_HANDLERS: &[AssignmentHandler] = &[general_v3::try_lower_assignment];
const TERMINATOR_HANDLERS: &[TerminatorHandler] = &[control_flow::try_lower_terminator];

pub(super) fn try_lower_call(
    lowerer: &mut FunctionLowerer<'_, '_>,
    call: AuthenticatedSemanticCall<'_>,
    block: &mut BasicBlock,
) -> Option<LoweringResult<Terminator>> {
    CALL_HANDLERS
        .iter()
        .find_map(|handler| handler(lowerer, call, block))
}

pub(super) fn try_lower_assignment(
    lowerer: &mut FunctionLowerer<'_, '_>,
    assignment: SemanticAssignment<'_>,
    block: &mut BasicBlock,
) -> Option<LoweringResult<()>> {
    ASSIGNMENT_HANDLERS
        .iter()
        .find_map(|handler| handler(lowerer, assignment, block))
}

pub(super) fn try_lower_terminator(
    lowerer: &mut FunctionLowerer<'_, '_>,
    terminator: SemanticTerminator<'_>,
    block: &mut BasicBlock,
) -> Option<LoweringResult<Terminator>> {
    TERMINATOR_HANDLERS
        .iter()
        .find_map(|handler| handler(lowerer, terminator, block))
}
