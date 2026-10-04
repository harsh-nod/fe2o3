//! Explicit create-new source action through the retained live compiler owner.
use super::*;
use crate::production_ordered_composition_source_v1::{
    OrderedCompositionSourcePublishEffectV1, OrderedCompositionSourcePublishErrorV1,
    OrderedCompositionSourcePublishRequestV1, PublishedOrderedCompositionSourceV1,
    publish_ordered_composition_source_v1,
};
impl AuthenticatedOrderedCompositionDiagnosticV1<'_> {
    pub(crate) fn publish_source_candidate_v1(
        &mut self,
        request: &SourcePromotionRequestV1,
    ) -> Result<PublishedOrderedCompositionSourceV1, String> {
        // Request hashes select within this actual live owner; they cannot build
        // one, replace its source seed or bypass any frontend admission.
        if self.source_seed.semantic_sha256() != &request.semantic
            || self.materialized.executable().identity().digest() != &request.canonical
        {
            return Err("NotAttempted: stale promotion semantic/canonical selection".into());
        }
        let definition = self
            .materialized
            .composition()
            .definitions()
            .get(usize::from(request.definition))
            .filter(|d| d.key().ordinal() == u32::from(request.definition))
            .ok_or("NotAttempted: promotion definition is absent in the actual owner")?
            .key();
        let mut possible_publication = false;
        let result = self.ledger.with_budget(|budget| {
            // The bounded request reader/parser is a separate UI input domain.
            // The actual HIR/SSA inspection and publisher reuse this same ledger.
            let selected = OrderedCompositionSourcePublishRequestV1 {
                definition,
                expected_canonical: self.materialized.executable().identity(),
                expected_semantic: request.semantic,
                original_path: &request.original,
                original_sha256: request.original_sha256,
                candidate_path: &request.candidate,
                helper_name: &request.helper,
                edit: request.edit,
            };
            let nested = fe2o3_lower_mir_kernel::with_ordered_composition_inspection_v1(
                &self.materialized,
                budget,
                |view, budget| {
                    let published: Result<
                        PublishedOrderedCompositionSourceV1,
                        OrderedCompositionSourcePublishErrorV1,
                    > = publish_ordered_composition_source_v1(
                        &self.source_seed,
                        view,
                        &selected,
                        budget,
                    );
                    possible_publication = published.as_ref().map_or_else(
                        |error| {
                            error.effect
                                == OrderedCompositionSourcePublishEffectV1::MayHaveCreatedCandidate
                        },
                        |_| true,
                    );
                    Ok(published)
                },
            )
            .map_err(|e| format!("same-owner inspection: {e}"))?;
            let published = nested.map_err(|e| e.to_string())?;
            budget
                .reserve_storage(published.retained_storage_bytes())
                .map_err(|e| format!("retained publication facts: {e}"))?;
            Ok::<_, String>(published)
        });
        result.map_err(|error| {
            format!(
                "{}: {error}",
                if possible_publication {
                    "MayHaveCreatedCandidate"
                } else {
                    "NotAttempted"
                }
            )
        })
    }
}
