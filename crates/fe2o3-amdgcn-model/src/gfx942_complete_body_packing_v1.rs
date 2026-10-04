//! Adapter from inert packing into the existing single complete-body checker.
use super::*;
use fe2o3_kernel_ir::{
    GFX942_COMPLETE_BODY_PACKING_WORK_V1, Gfx942CompleteBodyPackedV1,
    Gfx942CompleteBodyPackingErrorV1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942CompleteBodyPackedCheckErrorV1 {
    Packing(Gfx942CompleteBodyPackingErrorV1),
    Body(Gfx942CompleteBodyErrorV1),
}

impl fmt::Display for Gfx942CompleteBodyPackedCheckErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Packing(error) => error.fmt(formatter),
            Self::Body(error) => error.fmt(formatter),
        }
    }
}
impl std::error::Error for Gfx942CompleteBodyPackedCheckErrorV1 {}

impl Gfx942CompleteBodyPlanV1 {
    /// Checks one structurally packed intent using the unchanged body validator.
    /// Prepays a separate 64-unit decode/projection debit, then the existing
    /// 512-unit body debit. Fixed stack temporaries are not heap reservations;
    /// a retained packed/decoded/plan owner must account its logical storage
    /// separately. No source, executable, ABI-emission or artifact authority.
    pub fn check_packed(
        boundary: Gfx942CompleteBodyBoundaryV1,
        registers: Registers,
        resources: Gfx942CompleteBodyResourcesV1,
        packed: Gfx942CompleteBodyPackedV1,
        work: &mut CanonicalKernelIrWorkBudgetV1,
    ) -> Result<Self, Gfx942CompleteBodyPackedCheckErrorV1> {
        use Gfx942CompleteBodyPackedCheckErrorV1 as E;
        work.charge_work(GFX942_COMPLETE_BODY_PACKING_WORK_V1)
            .map_err(|error| E::Body(Gfx942CompleteBodyErrorV1::Work(error)))?;
        let decoded = packed.decode().map_err(E::Packing)?;
        let mut blocks = [Gfx942CompleteBodyBlockV1 {
            label: Gfx942CompleteBodyLabelV1(0),
            instructions: &[],
            terminator: Gfx942CompleteBodyTerminatorV1::GuardedStoreOutputAndEnd,
        }; GFX942_COMPLETE_BODY_MAX_BLOCKS_V1];
        for (slot, block) in blocks.iter_mut().zip(decoded.blocks()) {
            *slot = block;
        }
        Self::check(
            boundary,
            registers,
            resources,
            &blocks[..decoded.block_count()],
            work,
        )
        .map_err(E::Body)
    }
}

#[cfg(test)]
#[path = "gfx942_complete_body_packing_v1_tests.rs"]
mod tests;
