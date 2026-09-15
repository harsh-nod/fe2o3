use super::*;
use fe2o3_pliron::{
    ProductionSemanticSsaSourceOperandV1 as SourceOperand,
    ProductionSemanticSsaSourceUseV1 as SourceUse,
};

/// Inert selection of actual replayed source occurrences. The owned Workgroup
/// may remain retained storage and supplies no SSA value. This is neither a
/// source-authentication receipt nor an initialization or lifetime proof.
pub struct ProductionTransposeOwnedSourceUsesV1<'a, 'u> {
    pub(super) partition: &'u SourceUse<'a>,
    pub(super) subgroup: &'u SourceUse<'a>,
    pub(super) epoch: &'u SourceUse<'a>,
    pub(super) workgroup: &'u SourceOperand<'a>,
    pub(super) borrows: &'u [SourceUse<'a>],
}

impl<'a, 'u> ProductionTransposeOwnedSourceUsesV1<'a, 'u> {
    /// Selects existing uses; the batch consumer checks their exact owner,
    /// source/footer coordinates, complete roster, type and lifetime relations.
    pub const fn new(
        partition: &'u SourceUse<'a>,
        subgroup: &'u SourceUse<'a>,
        epoch: &'u SourceUse<'a>,
        workgroup: &'u SourceOperand<'a>,
        borrows: &'u [SourceUse<'a>],
    ) -> Self {
        Self {
            partition,
            subgroup,
            epoch,
            workgroup,
            borrows,
        }
    }
}
