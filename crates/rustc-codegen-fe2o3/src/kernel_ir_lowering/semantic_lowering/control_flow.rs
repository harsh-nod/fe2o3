//! Baseline structured control-flow lowering.

use super::{FunctionLowerer, HandlerClaim, SemanticTerminator, TranslationDiagnostic};
use crate::mir_import::MirTerminatorKind;
use fe2o3_kernel_ir::{BasicBlock, Terminator};

pub(super) fn claim_terminator(
    _lowerer: &FunctionLowerer<'_, '_>,
    terminator: SemanticTerminator<'_>,
) -> HandlerClaim {
    if matches!(terminator.kind, MirTerminatorKind::Goto { .. }) {
        HandlerClaim::Owned
    } else {
        HandlerClaim::NotOwned
    }
}

pub(super) fn lower_terminator(
    lowerer: &mut FunctionLowerer<'_, '_>,
    terminator: SemanticTerminator<'_>,
    _block: &mut BasicBlock,
) -> Result<Terminator, TranslationDiagnostic> {
    let MirTerminatorKind::Goto { target } = terminator.kind else {
        unreachable!("only claimed goto terminators may be lowered");
    };

    Ok(Terminator::Branch {
        target: lowerer.block_id(*target, terminator.location.clone())?,
        arguments: lowerer.edge_arguments(*target, terminator.location)?,
    })
}
