//! Non-default local Global-read conditions on the actual source scalar candidate.
//! This entry preserves every remaining source, allocation and launch obligation.
use super::context_handoff_v29::tile_scalar_source_v29::ProductionTileScalarPipelineErrorV29;
use super::{CollectedRustStage, ProductionCompilation};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use fe2o3_lower_mir_kernel::{
    ProductionCheckedTileGlobalReadsV29 as Checked, ProductionTileGlobalReadErrorV29 as ReadError,
    ProductionTileScalarOrderV29,
};

#[derive(Debug)]
pub(crate) enum ProductionTileGlobalReadPipelineErrorV29 {
    Source(ProductionTileScalarPipelineErrorV29),
    LocalRead(ReadError),
}
impl std::fmt::Display for ProductionTileGlobalReadPipelineErrorV29 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source(error) => error.fmt(formatter),
            Self::LocalRead(error) => error.fmt(formatter),
        }
    }
}
impl std::error::Error for ProductionTileGlobalReadPipelineErrorV29 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            Self::LocalRead(error) => Some(error),
        }
    }
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    /// Consumes authenticated collection through the existing source transport.
    /// Success is local read conditions only, never final compilation/activation.
    pub(crate) fn consume_checked_tile_global_reads_v29<T>(
        self,
        order: ProductionTileScalarOrderV29,
        consume: impl for<'scope, 'w> FnOnce(&Checked<'scope>, &mut Budget<'w>) -> Result<T, ReadError>,
    ) -> Result<T, ProductionTileGlobalReadPipelineErrorV29> {
        let nested = self
            .consume_checked_tile_scalar_source_v29(order, |transport, budget| {
                Ok(fe2o3_lower_mir_kernel::with_checked_tile_global_reads_v29(
                    transport, budget, consume,
                ))
            })
            .map_err(ProductionTileGlobalReadPipelineErrorV29::Source)?;
        nested.map_err(ProductionTileGlobalReadPipelineErrorV29::LocalRead)
    }
}
