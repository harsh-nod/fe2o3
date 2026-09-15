use super::*;

/// Stream the complete canonical incoming roster. No caller vector, singleton
/// assumption, source-derived sort, or last-caller replacement is involved.
pub(super) fn reconstruct(
    target: SemanticFunctionIdV1,
    functions: &[SemanticFunctionDeclV1],
    callables: &[SemanticCallableDeclV1],
    work: &mut u64,
) -> Result<SemanticPhaseIncomingCommitmentV1, SemanticMirErrorV1> {
    use sha2::{Digest, Sha256};
    let mut digest = Sha256::new();
    digest.update(b"FE2O3/SEMANTIC-REUSABLE-PHASE-INCOMING/V26\0");
    digest.update(target.index().to_le_bytes());
    let mut count = 0u32;
    for (index, caller) in functions.iter().enumerate() {
        spend(work, 1)?;
        let mut committed_body = false;
        for (block, data) in caller.blocks().iter().enumerate() {
            spend(work, 1)?;
            let SemanticTerminatorKindV1::Call(call) = data.terminator().kind() else {
                continue;
            };
            if callables.get(call.callee().index() as usize)
                != Some(&SemanticCallableDeclV1::Defined { function: target })
            {
                continue;
            }
            require(
                index != target.index() as usize
                    && call.unwind() == SemanticUnwindActionV1::Unreachable,
            )?;
            let destination = call
                .destination()
                .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?;
            require(
                destination.edge().role() == SemanticEdgeRoleV1::CallReturn
                    && destination.place().projections().is_empty(),
            )?;
            if !committed_body {
                let (hash, bytes) = canonical_semantic_source_body_sha256_v25(caller, *work / 2)?;
                spend(
                    work,
                    bytes
                        .checked_mul(2)
                        .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?,
                )?;
                digest.update([0]);
                digest.update((index as u32).to_le_bytes());
                digest.update(hash);
                committed_body = true;
            }
            count = count
                .checked_add(1)
                .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?;
            require(u64::from(count) <= HARD_MAX_BLOCKS_V1)?;
            digest.update([1]);
            digest.update((block as u32).to_le_bytes());
            digest.update(call.callee().index().to_le_bytes());
            digest.update(destination.edge().target().index().to_le_bytes());
            digest.update(destination.place().local().index().to_le_bytes());
        }
    }
    require(count != 0)?;
    digest.update([2]);
    digest.update(count.to_le_bytes());
    Ok(SemanticPhaseIncomingCommitmentV1 {
        count,
        digest: digest.finalize().into(),
    })
}
