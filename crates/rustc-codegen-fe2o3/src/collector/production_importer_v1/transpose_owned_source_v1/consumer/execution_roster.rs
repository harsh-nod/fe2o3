//! Full expanded terminal coverage, reusing the original source roster ledger.
//! Block identities select occurrences, never capability issuers or SSA values.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticCallableDeclV1, SemanticCompilerIntrinsicOperationV1,
    SemanticExecutionCapabilityOperationV1, SemanticGfx950TransposeOperationV1,
};

pub(super) fn expected(
    owner: &ProductionSemanticSsaOwnerV1,
    view: &SemanticExpandedRootV1,
    work: &mut usize,
) -> PlanResult<protocol::Roster<u32>> {
    let mut entries = Vec::new();
    for (index, block) in view.body().blocks().iter().enumerate() {
        bounded::charge(work, 1)?;
        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            continue;
        };
        let Some(SemanticCallableDeclV1::CompilerIntrinsic {
            operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
            ..
        }) = owner
            .source_semantic()
            .callables()
            .get(call.callee().index() as usize)
        else {
            continue;
        };
        let SemanticExecutionCapabilityOperationV1::Gfx950Transpose(transpose) =
            contract.operation()
        else {
            continue;
        };
        let role = match transpose.operation() {
            SemanticGfx950TransposeOperationV1::Issue { .. } => protocol::Role::Issue,
            SemanticGfx950TransposeOperationV1::Stage { .. } => protocol::Role::Stage,
            SemanticGfx950TransposeOperationV1::Publish { .. } => protocol::Role::Publish,
            SemanticGfx950TransposeOperationV1::Read { .. } => continue,
        };
        let origin = view.block_origins().get(index).ok_or(Error::Source(
            "transpose execution occurrence has no original source",
        ))?;
        let original = owner
            .source_semantic()
            .functions()
            .get(origin.function().index() as usize)
            .and_then(|function| function.blocks().get(origin.block().index() as usize));
        if origin.terminator() != TerminatorOrigin::Source
            || !matches!(original.map(|block| block.terminator().kind()),
                Some(SemanticTerminatorKindV1::Call(original)) if original.callee() == call.callee())
        {
            return Err(
                Error::Source("transpose terminal changed original call attribution").into(),
            );
        }
        bounded::push(
            &mut entries,
            (role, u32::try_from(index).map_err(|_| Error::Work)?),
            work,
        )?;
    }
    protocol::Roster::new(entries, work).map_err(Into::into)
}
