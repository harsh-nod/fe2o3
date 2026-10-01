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
