//! Source-only action over the authentic P0 callback; never resumes a candidate.
use super::*;
use crate::production_tiled_region_source_v1::{
    Bf16SourcePublicationProgressV1, Bf16TileSourcePublishErrorV1, Bf16TileSourcePublishRequestV1,
    PublishedBf16TileSourceV1, publish_bf16_tile_helper_source_v1,
};

pub(crate) struct Bf16TileSourcePromotionOutcomeV1 {
    // None means the callback did not complete the publisher. Returned outcomes
    // retain progress across Result/postflight failures, not a propagated panic:
    // unwinding this convenience action returns no outcome.
    pub(crate) publication: Option<Result<PublishedBf16TileSourceV1, Bf16TileSourcePublishErrorV1>>,
    pub(crate) source_postflight: Result<(), Box<ProductionPipelineError>>,
    pub(crate) progress: Bf16SourcePublicationProgressV1,
}
impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    /// The existing P0 entry still requires extraction-only custody and all
    /// original collection/source/SSA/target joins. The returned ordinary owner
    /// is dropped, not sent through a substitute success path or serialized.
    pub(crate) fn publish_bf16_tile_source_candidate_v1(
        self,
        request: &Bf16TileSourcePublishRequestV1<'_>,
    ) -> Bf16TileSourcePromotionOutcomeV1 {
        let mut progress = Bf16SourcePublicationProgressV1::new();
        let mut publication = None;
        let result = self.materialize_with_bf16_mfma_inspection_v1(|source, budget| {
            // Fixed caller-owned outcome remains live across all P0 postflight.
            // Its extra reservation is preserved by the original phase contract.
            budget.reserve_storage(std::mem::size_of::<Bf16TileSourcePromotionOutcomeV1>())?;
            let result = publish_bf16_tile_helper_source_v1(source, request, budget, &mut progress);
            let accepted = result.is_ok();
            publication = Some(result);
            if accepted {
                Ok(())
            } else {
                Err(
                    fe2o3_lower_mir_kernel::ProductionTiledRegionInspectionErrorV1::Unavailable(
                        "BF16 source publication refused; retained outcome",
                    ),
                )
            }
        });
        let source_postflight = result.map(|(ordinary, ())| drop(ordinary));
        Bf16TileSourcePromotionOutcomeV1 {
            publication,
            source_postflight,
            progress,
        }
    }
}

/// Inert point-in-time selection facts, not a source owner or production token.
#[derive(Clone, Copy)]
pub(crate) struct Bf16TileSourceSelectionV1 {
    pub(crate) semantic_sha256: [u8; 32],
    pub(crate) canonical_sha256: [u8; 32],
    pub(crate) mir_sha256: [u8; 32],
    pub(crate) original_sha256: [u8; 32],
    pub(crate) original_bytes: usize,
}
impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    /// Read-only inspection of the original direct source profile. Publishing
    /// redoes the live source admission and additional eligibility checks.
    pub(crate) fn inspect_bf16_tile_source_selection_v1(
        self,
    ) -> Result<Bf16TileSourceSelectionV1, Box<ProductionPipelineError>> {
        let (ordinary, selection) =
            self.materialize_with_bf16_mfma_inspection_v1(|source, budget| {
                // Retain only fixed primitive facts through original postflight.
                // No whole-action storage or timing measurement is introduced.
                budget.reserve_storage(std::mem::size_of::<Bf16TileSourceSelectionV1>())?;
                budget.charge_work(256)?;
                let owner = source.emission().original();
                Ok(Bf16TileSourceSelectionV1 {
                    semantic_sha256: *owner
                        .semantic_ssa()
                        .source_semantic()
                        .semantic_sha256()
                        .as_bytes(),
                    canonical_sha256: *owner.executable().canonical().identity().digest(),
                    mir_sha256: *source.mir_sha256(),
                    original_sha256: *source.source().sha256(),
                    original_bytes: source.source().bytes().len(),
                })
            })?;
        drop(ordinary);
        Ok(selection)
    }
}
