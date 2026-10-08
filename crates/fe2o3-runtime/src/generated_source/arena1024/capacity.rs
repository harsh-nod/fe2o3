//! Private count-branded controls; public sources never expose a family conversion.

use super::*;

pub(crate) enum ArenaPacketsV1 {
    Original(fe2o3_kfd::Gfx942NativeFillArenaPacketsV1),
    Independent2048(fe2o3_kfd::Gfx942IndependentFillArena2048PacketsV1),
}

impl ArenaPacketsV1 {
    #[cfg(test)]
    pub(crate) fn packets(&self) -> &[fe2o3_kfd::Gfx942FixedDispatchPacketV1] {
        match self {
            Self::Original(packets) => packets.packets(),
            Self::Independent2048(packets) => packets.packets(),
        }
    }

    pub(crate) fn matches(&self, profile: GeneratedProfileV1) -> bool {
        matches!(
            (self, profile),
            (
                Self::Original(_),
                GeneratedProfileV1::NativeFillArena1024
                    | GeneratedProfileV1::IndependentFillArena1024
            ) | (
                Self::Independent2048(_),
                GeneratedProfileV1::IndependentFillArena2048
            )
        )
    }
}

/// Exactly 2048 original independent-WO sources, not a scalar carrier or a
/// 1024-family source. This inert preparation does not qualify native depth.
///
/// ```compile_fail
/// use fe2o3_runtime::{RuntimeGfx942GeneratedIndependentArena2048V1 as Large,
///                    RuntimeGfx942GeneratedIndependentArena1024V1 as Original};
/// fn refuse<P>(source: Large<P>) -> Original<P> { source }
/// ```
pub struct RuntimeGfx942GeneratedIndependentArena2048V1<P>(
    pub(crate) RuntimeGfx942GeneratedArena1024V1<P>,
);

impl<P: RuntimeGfx942GeneratedCarrierV1> RuntimeGfx942GeneratedIndependentArena2048V1<P> {
    pub fn try_new<E>(
        metadata: &ResourceCreditAccountV1,
        device_unique_id: u64,
        prepare: impl FnMut(usize) -> Result<P, E>,
    ) -> Result<Self, RuntimeGfx942ArenaPreparationErrorV1<E>> {
        RuntimeGfx942GeneratedArena1024V1::try_new_with_profile(
            metadata,
            device_unique_id,
            prepare,
            GeneratedProfileV1::IndependentFillArena2048,
        )
        .map(Self)
    }
}
