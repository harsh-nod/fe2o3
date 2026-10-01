//! Storage-only checkpoint through the ORIGINAL import, then unchanged continuation.
//! Not called by the existing warm timing loop. No authenticated owner is built
//! in these tests; the signature control is compile-time coverage only.
use super::super::{
    CollectedRustStage, ProductionCompilation, ProductionPipelineError,
    RankedVerifiedProductionCompilation,
};
use super::{BindingsRetainedStorageV1, BindingsStorageErrorV1};
use fe2o3_kernel_ir::LogicalStorageLimitsV1;
use std::{error::Error, fmt};

#[derive(Debug)]
pub(crate) enum BindingsStorageCheckpointErrorV1 {
    Pipeline(ProductionPipelineError),
    Storage(BindingsStorageErrorV1),
}
impl From<ProductionPipelineError> for BindingsStorageCheckpointErrorV1 {
    fn from(value: ProductionPipelineError) -> Self {
        Self::Pipeline(value)
    }
}
impl From<BindingsStorageErrorV1> for BindingsStorageCheckpointErrorV1 {
    fn from(value: BindingsStorageErrorV1) -> Self {
        Self::Storage(value)
    }
}
impl fmt::Display for BindingsStorageCheckpointErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pipeline(error) => write!(f, "original compilation: {error}"),
            Self::Storage(error) => fmt::Display::fmt(error, f),
        }
    }
}
impl Error for BindingsStorageCheckpointErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Pipeline(error) => Some(error),
            Self::Storage(error) => Some(error),
        }
    }
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    /// At one genuine after_analysis session, consume the original transaction
    /// through original import once, observe its retained ExtractionOnly bindings,
    /// and continue the exact existing general-verification sequence. The live
    /// semantic MIR and original TcX are adjacent owners, NOT included in this
    /// bindings-only report. Only four primitive counts cross this checkpoint.
    pub(crate) fn verify_general_kernel_checks_with_bindings_storage_v1(
        self,
        limits: LogicalStorageLimitsV1,
    ) -> Result<
        (
            RankedVerifiedProductionCompilation,
            BindingsRetainedStorageV1,
        ),
        BindingsStorageCheckpointErrorV1,
    > {
        let admitted = self.import_semantic_mir()?;
        let report = admitted
            .stage
            .bindings
            .logical_retained_storage_v1(limits)?;
        let ranked = admitted
            .construct_semantic_middle_end()?
            .construct_semantic_ssa()?
            .materialize_target_neutral()
            .map_err(ProductionPipelineError::from)?
            .verify_general_kernel_checks()?;
        Ok((ranked, report))
    }
}

#[test]
fn original_import_checkpoint_signature_without_fabricated_authenticated_owner() {
    fn signature<'tcx>() {
        let _: fn(
            ProductionCompilation<'tcx, CollectedRustStage<'tcx>>,
            LogicalStorageLimitsV1,
        ) -> Result<
            (
                RankedVerifiedProductionCompilation,
                BindingsRetainedStorageV1,
            ),
            BindingsStorageCheckpointErrorV1,
        > = ProductionCompilation::verify_general_kernel_checks_with_bindings_storage_v1;
    }
    signature();
}
