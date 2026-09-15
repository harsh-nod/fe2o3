use super::*;
use fe2o3_mir_model::semantic_direct_call_expansion_v1::SemanticExpandedDefinedCapabilityV1;

pub(super) fn origins(
    view: &SemanticExpandedRootV1,
    binding: &SemanticExpandedDefinedCapabilityV1,
    parameter: (SemanticBlockIdV1, u32),
    returned: (SemanticBlockIdV1, u32),
) -> Result<(), ProductionSemanticSsaErrorV1> {
    let frame = view
        .instances()
        .get(binding.callee_instance().index() as usize)
        .ok_or_else(mismatch)?;
    let entry = view
        .block_origins()
        .get(parameter.0.index() as usize)
        .ok_or_else(mismatch)?;
    let exit = view
        .block_origins()
        .get(returned.0.index() as usize)
        .ok_or_else(mismatch)?;
    if frame.local_count() != 2
        || parameter.1 != 2
        || returned.1 != 0
        || entry.statements().len() != 3
        || exit.statements().len() != 3
        || entry.terminator()
            != (T::CallEntry {
                callee: binding.callee_instance(),
            })
        || exit.terminator()
            != (T::CallReturn {
                callee: binding.callee_instance(),
            })
    {
        return Err(mismatch());
    }
    for index in 0..2 {
        let local = SemanticLocalIdV1::from_index(index as u32);
        if entry.statements()[index]
            != (S::FrameStorageLive {
                callee: binding.callee_instance(),
                local,
            })
            || exit.statements()[index + 1]
                != (S::FrameStorageDead {
                    callee: binding.callee_instance(),
                    local,
                })
        {
            return Err(mismatch());
        }
    }
    Ok(())
}

pub(super) fn statements(
    function: &SemanticFunctionDeclV1,
    row: &ProductionSemanticReusableLdsResultV1,
) -> Result<(), ProductionSemanticSsaErrorV1> {
    let entry = function
        .blocks()
        .get(row.parameter_block.index() as usize)
        .ok_or_else(mismatch)?;
    let exit = function
        .blocks()
        .get(row.return_block.index() as usize)
        .ok_or_else(mismatch)?;
    let mut locals = [row.parameter, row.return_local];
    locals.sort_unstable();
    if row.parameter_statement != 2
        || row.return_statement != 0
        || locals[0].index().checked_add(1) != Some(locals[1].index())
        || entry.statements().len() != 3
        || exit.statements().len() != 3
    {
        return Err(mismatch());
    }
    for (index, local) in locals.into_iter().enumerate() {
        if entry.statements()[index].kind() != &SemanticStatementKindV1::StorageLive(local)
            || exit.statements()[index + 1].kind() != &SemanticStatementKindV1::StorageDead(local)
        {
            return Err(mismatch());
        }
    }
    Ok(())
}
