use super::*;
use dialect_kernel::{OwnershipCoverageAttr, OwnershipPartitionAttr};
use fe2o3_pliron::{
    ProductionConditionalOwnershipSiteV1 as Site, ProductionRankedKernelV1,
    ProductionRankedOperationV1,
};

/// Selects the existing single conditional output's exact ownership operation.
/// This scans typed operations, never a kernel/profile name, and grants no
/// ordinary proof or launch authority. Debits match the live selector's scans.
pub fn select_native_conditional_ownership_site_v2(
    kernel: &ProductionRankedKernelV1,
    budget: &mut Budget<'_>,
) -> Result<Site, E> {
    let mut output = None;
    for block in kernel.blocks() {
        budget.charge_work(1)?;
        for op in block.operations() {
            budget.charge_work(1)?;
            if let ProductionRankedOperationV1::RequireEffectRefinement { contract, .. } = op {
                if output.replace(contract.view()).is_some() {
                    return Err(E::invalid("conditional output roster"));
                }
            }
        }
    }
    let output = output.ok_or_else(|| E::invalid("missing bound output"))?;
    let mut selected = None;
    for (b, block) in kernel.blocks().iter().enumerate() {
        budget.charge_work(1)?;
        for (o, op) in block.operations().iter().enumerate() {
            budget.charge_work(1)?;
            if let ProductionRankedOperationV1::OwnershipContract {
                view,
                coverage,
                partition,
            } = op
                && *view == output
            {
                if selected.is_some()
                    || *coverage != OwnershipCoverageAttr::TotalView
                    || *partition != OwnershipPartitionAttr::ExactSets
                {
                    return Err(E::invalid("ambiguous conditional ownership"));
                }
                selected = Some(Site {
                    block: u32::try_from(b).map_err(|_| Resource::Arithmetic)?,
                    operation: u32::try_from(o).map_err(|_| Resource::Arithmetic)?,
                    view: *view,
                });
            }
        }
    }
    selected.ok_or_else(|| E::invalid("missing conditional ownership"))
}
