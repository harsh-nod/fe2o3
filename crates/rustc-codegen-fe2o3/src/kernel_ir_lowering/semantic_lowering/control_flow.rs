//! Baseline structured control-flow lowering.

use super::{FunctionLowerer, SemanticTerminator, TranslationDiagnostic};
use crate::mir_import::MirTerminatorKind;
use fe2o3_kernel_ir::{BasicBlock, Terminator};

pub(super) fn try_lower_terminator(
    lowerer: &mut FunctionLowerer<'_, '_>,
    terminator: SemanticTerminator<'_>,
    _block: &mut BasicBlock,
) -> Option<Result<Terminator, TranslationDiagnostic>> {
    let MirTerminatorKind::Goto { target } = terminator.kind else {
        return None;
    };

    Some(
        lowerer
            .block_id(*target, terminator.location.clone())
            .map(|target| Terminator::Branch {
                target,
                arguments: Vec::new(),
            }),
    )
}
