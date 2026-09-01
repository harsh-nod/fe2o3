use std::{error::Error, fmt};

use fe2o3_pliron::{
    ProductionRankedKernelLoweringInputV1, ProductionRankedKernelV1, ProductionRankedValueIdV1,
    ProductionRankedValueV1, ProductionSemanticExpressionV2,
};

/// A GPU write projected independently from frontend reference effects.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RankedGpuWriteV2 {
    pub block: usize,
    pub operation: usize,
    pub allocation_origin: u64,
    pub view: ProductionRankedValueV1,
    pub indices: Vec<ProductionRankedValueV1>,
    pub value: Result<ProductionSemanticExpressionV2, &'static str>,
}

/// Stable error boundary for frontend-owned reference-effect implementations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionReferenceEffectErrorV1 {
    detail: String,
}

impl ProductionReferenceEffectErrorV1 {
    pub fn new(detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
        }
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl fmt::Display for ProductionReferenceEffectErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.detail)
    }
}

impl Error for ProductionReferenceEffectErrorV1 {}

/// Frontend adapter for optional, independently authenticated reference effects.
///
/// The middle end owns root partitioning and ranked projection. A frontend owns
/// the concrete reference IR and proof executor, and supplies only these bounded
/// operations. Implementations returned by `select` must own their custody so
/// projection cannot retain a borrow into frontend collection state.
pub trait AuthenticatedReferenceEffectsV1 {
    fn logical_kernel_names(&self) -> Box<[&str]>;

    fn select(
        &self,
        indices: &[usize],
    ) -> Result<Box<dyn AuthenticatedReferenceEffectsV1>, ProductionReferenceEffectErrorV1>;

    fn is_empty(&self) -> bool;

    fn reserved_output_ranks(&self) -> Result<Vec<usize>, ProductionReferenceEffectErrorV1>;

    fn prove_and_compile(
        &self,
        kernel: ProductionRankedKernelV1,
        writes: &[RankedGpuWriteV2],
        reserved_values: Vec<ProductionRankedValueIdV1>,
    ) -> Result<ProductionRankedKernelLoweringInputV1, ProductionReferenceEffectErrorV1>;
}
