use super::*;
use fe2o3_mir_model::semantic_mir_v1::SemanticBasicBlockV1;

// Only the existing interpreter's empty, operand-free transfer can share an
// entry unchanged. This does not remove blocks, edges or any predecessor meet.
pub(super) fn is_identity(block: &SemanticBasicBlockV1, budget: &mut Budget) -> Result<bool> {
    budget.charge(1)?;
    if !block.statements().is_empty() {
        return Ok(false);
    }
    Ok(match block.terminator().kind() {
        SemanticTerminatorKindV1::Goto(_)
        | SemanticTerminatorKindV1::Return
        | SemanticTerminatorKindV1::UnwindResume
        | SemanticTerminatorKindV1::UnwindTerminate
        | SemanticTerminatorKindV1::Abort
        | SemanticTerminatorKindV1::Unreachable => true,
        SemanticTerminatorKindV1::Call(_)
        | SemanticTerminatorKindV1::SwitchInt { .. }
        | SemanticTerminatorKindV1::Assert { .. }
        | SemanticTerminatorKindV1::Drop { .. }
        | SemanticTerminatorKindV1::TailCall(_)
        | SemanticTerminatorKindV1::FalseEdge { .. } => false,
    })
}

#[cfg(test)]
#[path = "neutral_transfer_tests.rs"]
mod tests;
