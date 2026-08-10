//! Extension boundary for lowering compiler-recognized source semantics.
//!
//! Imported callees enter this module only after rustc `DefId` recognition has
//! produced a [`SessionRecognizedSemanticItem`]. That identity is local to one
//! compilation session and grants no proof, provider, or artifact authority.
//! Feature modules must dispatch on it, never on diagnostic paths.

mod collective_v1;
mod control_flow;
mod diagnostics;
mod general_v3;
mod memory_v1;

use super::{
    FunctionLowerer, TranslationDiagnostic, TranslationDiagnosticCode, TranslationLocation,
    diagnostic,
};
use crate::mir_import::{MirCallee, MirOperandRef, MirPlaceRef, MirRvalueKind, MirTerminatorKind};
use crate::semantic_features::SessionRecognizedSemanticItem;
use fe2o3_kernel_ir::{BasicBlock, Terminator};

#[derive(Clone, Copy)]
pub(super) struct SessionRecognizedSemanticCall<'call> {
    item: SessionRecognizedSemanticItem,
    callee: &'call MirCallee,
    target: usize,
    destination: &'call MirPlaceRef,
    operands: &'call [MirOperandRef],
    location: &'call TranslationLocation,
}

/// An assignment imported from structured rustc MIR.
///
/// Unlike calls, assignments do not need a `DefId` authority. Their operation
/// and operands are provided by the typed MIR importer rather than by a
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

impl<'call> SessionRecognizedSemanticCall<'call> {
    pub(super) fn new(
        callee: &'call MirCallee,
        target: usize,
        destination: &'call MirPlaceRef,
        operands: &'call [MirOperandRef],
        location: &'call TranslationLocation,
    ) -> Option<Self> {
        Some(Self {
            item: callee.session_recognized_item()?,
            callee,
            target,
            destination,
            operands,
            location,
        })
    }
}

/// Read-only ownership decision made before any handler may mutate lowering state.
#[derive(Debug, Eq, PartialEq)]
pub(super) enum HandlerClaim {
    NotOwned,
    Owned,
    Reject(TranslationDiagnostic),
}

pub(super) enum LoweringOutcome<T> {
    NotOwned,
    Lowered(T),
    Reject(TranslationDiagnostic),
}

type LoweringResult<T> = Result<T, TranslationDiagnostic>;

type CallClaim = for<'function, 'declarations, 'call> fn(
    &FunctionLowerer<'function, 'declarations>,
    SessionRecognizedSemanticCall<'call>,
) -> HandlerClaim;
type CallLowering = for<'function, 'declarations, 'call> fn(
    &mut FunctionLowerer<'function, 'declarations>,
    SessionRecognizedSemanticCall<'call>,
    &mut BasicBlock,
) -> LoweringResult<Terminator>;

type AssignmentClaim = for<'function, 'declarations, 'assignment> fn(
    &FunctionLowerer<'function, 'declarations>,
    SemanticAssignment<'assignment>,
) -> HandlerClaim;
type AssignmentLowering = for<'function, 'declarations, 'assignment> fn(
    &mut FunctionLowerer<'function, 'declarations>,
    SemanticAssignment<'assignment>,
    &mut BasicBlock,
) -> LoweringResult<()>;

type TerminatorClaim = for<'function, 'declarations, 'terminator> fn(
    &FunctionLowerer<'function, 'declarations>,
    SemanticTerminator<'terminator>,
) -> HandlerClaim;
type TerminatorLowering =
    for<'function, 'declarations, 'terminator> fn(
        &mut FunctionLowerer<'function, 'declarations>,
        SemanticTerminator<'terminator>,
        &mut BasicBlock,
    ) -> LoweringResult<Terminator>;

struct CallHandler {
    name: &'static str,
    claim: CallClaim,
    lower: CallLowering,
}

struct AssignmentHandler {
    name: &'static str,
    claim: AssignmentClaim,
    lower: AssignmentLowering,
}

struct TerminatorHandler {
    name: &'static str,
    claim: TerminatorClaim,
    lower: TerminatorLowering,
}

const CALL_HANDLERS: &[CallHandler] = &[
    CallHandler {
        name: "gfx942-diagnostics",
        claim: diagnostics::claim_call,
        lower: diagnostics::lower_call,
    },
    CallHandler {
        name: "gfx942-collective-v1",
        claim: collective_v1::claim_call,
        lower: collective_v1::lower_call,
    },
    CallHandler {
        name: "gfx942-memory-v1",
        claim: memory_v1::claim_call,
        lower: memory_v1::lower_call,
    },
    CallHandler {
        name: "general-v3",
        claim: general_v3::claim_call,
        lower: general_v3::lower_call,
    },
];
const ASSIGNMENT_HANDLERS: &[AssignmentHandler] = &[AssignmentHandler {
    name: "general-v3",
    claim: general_v3::claim_assignment,
    lower: general_v3::lower_assignment,
}];
const TERMINATOR_HANDLERS: &[TerminatorHandler] = &[TerminatorHandler {
    name: "structured-control-flow",
    claim: control_flow::claim_terminator,
    lower: control_flow::lower_terminator,
}];

#[derive(Debug, Eq, PartialEq)]
enum ClaimSelection {
    NotOwned,
    Handler(usize),
    Reject(TranslationDiagnostic),
}

fn select_claim(
    operation: &'static str,
    location: &TranslationLocation,
    claims: impl IntoIterator<Item = (usize, &'static str, HandlerClaim)>,
) -> ClaimSelection {
    let mut claimants = claims
        .into_iter()
        .filter(|(_, _, claim)| !matches!(claim, HandlerClaim::NotOwned))
        .collect::<Vec<_>>();

    if claimants.len() > 1 {
        let mut names = claimants
            .iter()
            .map(|(_, name, _)| *name)
            .collect::<Vec<_>>();
        names.sort_unstable();
        let names = names
            .into_iter()
            .map(|name| format!("`{name}`"))
            .collect::<Vec<_>>()
            .join(", ");
        return ClaimSelection::Reject(diagnostic(
            TranslationDiagnosticCode::MalformedMir,
            location.clone(),
            format!("ambiguous semantic {operation} lowering ownership: {names}"),
        ));
    }

    match claimants.pop() {
        None => ClaimSelection::NotOwned,
        Some((index, _, HandlerClaim::Owned)) => ClaimSelection::Handler(index),
        Some((_, _, HandlerClaim::Reject(diagnostic))) => ClaimSelection::Reject(diagnostic),
        Some((_, _, HandlerClaim::NotOwned)) => {
            unreachable!("NotOwned claims were filtered before selection")
        }
    }
}

fn finish<T>(result: LoweringResult<T>) -> LoweringOutcome<T> {
    match result {
        Ok(value) => LoweringOutcome::Lowered(value),
        Err(diagnostic) => LoweringOutcome::Reject(diagnostic),
    }
}

pub(super) fn lower_call(
    lowerer: &mut FunctionLowerer<'_, '_>,
    call: SessionRecognizedSemanticCall<'_>,
    block: &mut BasicBlock,
) -> LoweringOutcome<Terminator> {
    let selection = select_claim(
        "call",
        call.location,
        CALL_HANDLERS
            .iter()
            .enumerate()
            .map(|(index, handler)| (index, handler.name, (handler.claim)(lowerer, call))),
    );
    match selection {
        ClaimSelection::NotOwned => LoweringOutcome::NotOwned,
        ClaimSelection::Handler(index) => {
            finish((CALL_HANDLERS[index].lower)(lowerer, call, block))
        }
        ClaimSelection::Reject(diagnostic) => LoweringOutcome::Reject(diagnostic),
    }
}

pub(super) fn lower_assignment(
    lowerer: &mut FunctionLowerer<'_, '_>,
    assignment: SemanticAssignment<'_>,
    block: &mut BasicBlock,
) -> LoweringOutcome<()> {
    let selection = select_claim(
        "assignment",
        assignment.location,
        ASSIGNMENT_HANDLERS
            .iter()
            .enumerate()
            .map(|(index, handler)| (index, handler.name, (handler.claim)(lowerer, assignment))),
    );
    match selection {
        ClaimSelection::NotOwned => LoweringOutcome::NotOwned,
        ClaimSelection::Handler(index) => finish((ASSIGNMENT_HANDLERS[index].lower)(
            lowerer, assignment, block,
        )),
        ClaimSelection::Reject(diagnostic) => LoweringOutcome::Reject(diagnostic),
    }
}

pub(super) fn lower_terminator(
    lowerer: &mut FunctionLowerer<'_, '_>,
    terminator: SemanticTerminator<'_>,
    block: &mut BasicBlock,
) -> LoweringOutcome<Terminator> {
    let selection = select_claim(
        "terminator",
        terminator.location,
        TERMINATOR_HANDLERS
            .iter()
            .enumerate()
            .map(|(index, handler)| (index, handler.name, (handler.claim)(lowerer, terminator))),
    );
    match selection {
        ClaimSelection::NotOwned => LoweringOutcome::NotOwned,
        ClaimSelection::Handler(index) => finish((TERMINATOR_HANDLERS[index].lower)(
            lowerer, terminator, block,
        )),
        ClaimSelection::Reject(diagnostic) => LoweringOutcome::Reject(diagnostic),
    }
}

#[cfg(test)]
mod tests {
    use super::{ClaimSelection, HandlerClaim, select_claim};
    use crate::kernel_ir_lowering::{TranslationDiagnosticCode, TranslationLocation};

    fn location() -> TranslationLocation {
        TranslationLocation {
            function: Some("tests::claim".to_owned()),
            block: Some(0),
            statement: None,
            terminator: true,
            operation: None,
            source: None,
        }
    }

    #[test]
    fn ambiguous_ownership_is_rejected_deterministically_before_lowering() {
        let location = location();
        let forward = select_claim(
            "call",
            &location,
            [
                (0, "zeta-handler", HandlerClaim::Owned),
                (1, "alpha-handler", HandlerClaim::Owned),
            ],
        );
        let reverse = select_claim(
            "call",
            &location,
            [
                (0, "alpha-handler", HandlerClaim::Owned),
                (1, "zeta-handler", HandlerClaim::Owned),
            ],
        );

        let ClaimSelection::Reject(forward) = forward else {
            panic!("ambiguous forward claims must reject");
        };
        let ClaimSelection::Reject(reverse) = reverse else {
            panic!("ambiguous reverse claims must reject");
        };
        assert_eq!(forward, reverse);
        assert_eq!(forward.code, TranslationDiagnosticCode::MalformedMir);
        assert_eq!(
            forward.message,
            "ambiguous semantic call lowering ownership: `alpha-handler`, `zeta-handler`"
        );
    }

    #[test]
    fn a_claimed_rejection_is_not_treated_as_unowned() {
        let rejection = crate::kernel_ir_lowering::diagnostic(
            TranslationDiagnosticCode::UnsupportedCall,
            location(),
            "recognized but unsupported",
        );
        let selected = select_claim(
            "call",
            &location(),
            [(
                0,
                "rejecting-handler",
                HandlerClaim::Reject(rejection.clone()),
            )],
        );

        assert_eq!(selected, ClaimSelection::Reject(rejection));
    }

    #[test]
    fn disjoint_multiplication_claims_select_one_owner_but_overlap_rejects() {
        let location = location();
        let general_v3_only = select_claim(
            "assignment",
            &location,
            [
                (0, "general-v3-f32-multiply", HandlerClaim::Owned),
                (1, "integer-multiply", HandlerClaim::NotOwned),
            ],
        );
        let integer_only = select_claim(
            "assignment",
            &location,
            [
                (0, "general-v3-f32-multiply", HandlerClaim::NotOwned),
                (1, "integer-multiply", HandlerClaim::Owned),
            ],
        );

        assert_eq!(general_v3_only, ClaimSelection::Handler(0));
        assert_eq!(integer_only, ClaimSelection::Handler(1));

        let overlap = select_claim(
            "assignment",
            &location,
            [
                (0, "general-v3-f32-multiply", HandlerClaim::Owned),
                (1, "integer-multiply", HandlerClaim::Owned),
            ],
        );
        let ClaimSelection::Reject(overlap) = overlap else {
            panic!("overlapping multiplication claims must reject");
        };
        assert_eq!(overlap.code, TranslationDiagnosticCode::MalformedMir);
        assert_eq!(
            overlap.message,
            "ambiguous semantic assignment lowering ownership: `general-v3-f32-multiply`, `integer-multiply`"
        );
    }
}
