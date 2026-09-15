//! Mandatory production source/SSA join. This does not discharge typed
//! Workgroup/epoch, initialization, barrier, numerical or physical LDS checks.
use super::*;
use fe2o3_pliron::ProductionSemanticSsaOwnerV1;

impl AuthenticatedProductionKernelContextsV1 {
    pub(crate) fn validate_transpose_source_ssa(
        contexts: Option<&Self>,
        owner: &ProductionSemanticSsaOwnerV1,
    ) -> std::result::Result<(), ProductionSemanticImportErrorV1> {
        validate(contexts, owner)
            .map_err(|error| ProductionSemanticImportErrorV1::TransposeOwnedSource(Box::new(error)))
    }
}

fn validate(
    contexts: Option<&AuthenticatedProductionKernelContextsV1>,
    owner: &ProductionSemanticSsaOwnerV1,
) -> PlanResult<()> {
    let rows = owner.source_semantic().transpose_owned_flows();
    if rows.is_empty() {
        if contexts.is_some_and(|contexts| contexts.transpose_source.is_some()) {
            return Err(
                Error::Source("transpose live seal lost its canonical source roster").into(),
            );
        }
        return Ok(());
    }
    let contexts = contexts.ok_or(Error::Source(
        "transpose source footer has no authenticated frontend owner",
    ))?;
    let [root] = contexts.roots.as_ref() else {
        return Err(Error::Source(
            "transpose source requires a complete single authenticated root partition",
        )
        .into());
    };
    let seal = contexts.transpose_source.as_ref().ok_or(Error::Source(
        "transpose source footer has no live source seal",
    ))?;
    let mut work = usize::try_from(
        SemanticMirLimitsV1::default().limit(SemanticMirResourceV1::ValidationWork),
    )
    .map_err(|_| Error::Work)?;
    let flows = seal
        .rebind(owner, root.selected_root, &mut work)?
        .into_flows(owner)?;
    // Rebinding includes the complete expanded roster and both actual Move
    // ReturnTransfers. Nothing here creates a value at a raw Constant input.
    if flows.is_empty() {
        return Err(Error::Source("transpose sealed source has no expanded flow").into());
    }
    Ok(())
}
