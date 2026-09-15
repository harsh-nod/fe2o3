//! Frontend-owned typed join. The entry caller has already checked the source
//! root, launch identity and the original Context entry-transfer receipt.
use super::*;
use fe2o3_lower_mir_kernel::{
    ProductionKernelContextEntrySsaRelationV1, ProductionKernelContextLoweringInputV1,
    ProductionScopedMatrixSourceSessionV1, ProductionTransposeOwnedSourceUsesV1,
};
use fe2o3_pliron::ProductionSemanticSsaOwnerV1;

#[cfg(test)]
#[path = "typed_tests.rs"]
pub(super) mod tests;

pub(in super::super) fn check<'a>(
    contexts: &AuthenticatedProductionKernelContextsV1,
    root: &AuthenticatedProductionKernelContextRootV1,
    owner: &'a ProductionSemanticSsaOwnerV1,
    entry: Option<&ProductionKernelContextEntrySsaRelationV1<'a>>,
    max_work: usize,
) -> PlanResult<()> {
    let rows = owner.source_semantic().transpose_owned_flows();
    if rows.is_empty() {
        if contexts.transpose_source.is_some() {
            return Err(
                Error::Source("transpose live seal lost its canonical source roster").into(),
            );
        }
        return Ok(());
    }
    if contexts.roots.len() != 1 || !std::ptr::eq(&contexts.roots[0], root) {
        return Err(
            Error::Source("transpose typed custody requires its exact single source root").into(),
        );
    }
    let seal = contexts.transpose_source.as_ref().ok_or(Error::Source(
        "transpose typed custody has no live frontend source seal",
    ))?;
    let mut remaining = max_work;
    let flows = seal
        .rebind(owner, root.selected_root, &mut remaining)?
        .into_flows(owner)?;
    if flows.is_empty() {
        return Err(Error::Source(
            "transpose typed custody lost its complete expanded source flows",
        )
        .into());
    }
    let mut input = ProductionKernelContextLoweringInputV1::new(
        root.selected_root,
        contexts.frontend_unit_identity,
        root.kernel_marker_identity,
        contexts.target_brand_identity,
        root.launch_brand_identity,
        root.issuance_identity,
    );
    if let Some(transfer) = root.entry_transfer {
        input = input.with_entry_transfer(transfer);
    }
    ProductionScopedMatrixSourceSessionV1::check_transpose_owned_uses(
        owner,
        &input,
        entry,
        flows.iter().map(|flow| {
            ProductionTransposeOwnedSourceUsesV1::new(
                &flow.issue_partition,
                &flow.matrix_subgroup,
                &flow.matrix_epoch,
                &flow.workgroup,
                &flow.workgroup_borrows,
            )
        }),
        remaining,
    )
    .map_err(PlanError::Typed)
}
