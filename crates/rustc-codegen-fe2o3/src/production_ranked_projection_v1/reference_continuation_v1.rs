//! Resolve proof requests only after projection scratch and guard scopes close.
use super::*;
use crate::production_reference_effect_join_v2::conditional::{
    ReferenceRootV1 as Verification, ReferenceSourceV1, continue_reference_v1,
};
use crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1 as References;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use guarded_source_progress_v1::{resource, resources};

// This is a move-only batch of existing requests, not another executable graph.
// Only `finish` exposes a completed roster to the ordinary or guarded caller.
pub(super) struct ProjectedReferenceRootsV1 {
    pub(super) roots: Box<[ProductionRankedRootProgramV1]>,
    pub(super) references: Vec<References>,
}

impl ProjectedReferenceRootsV1 {
    pub(super) fn finish(
        self,
        source: &RankedProjectionSourceV1<'_>,
        budget: &mut Budget<'_>,
    ) -> Result<Box<[ProductionRankedRootProgramV1]>, ProductionRankedProjectionErrorV1> {
        source.require_floor(budget)?;
        resources::owned(
            budget,
            0,
            resource,
            || ProductionRankedProjectionErrorV1::Incomplete("reference continuation panicked"),
            |budget| {
                if self.roots.len() != self.references.len() {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "reference continuation lost its complete root roster",
                    ));
                }
                let mut retained = 0usize;
                let roots = self
                    .roots
                    .into_vec()
                    .into_iter()
                    .zip(self.references.iter())
                    .map(|(root, references)| {
                        let root = finish_root(root, references, source, budget)?;
                        let storage = match &root.verification {
                            Verification::Conditional(root) => {
                                root.input().retained_storage_v1().map_err(resource)?
                            }
                            Verification::Ordinary { .. } => 0,
                            Verification::Pending(_) => {
                                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                                    "reference continuation left an unproved root",
                                ));
                            }
                        };
                        retained = retained
                            .checked_add(storage)
                            .ok_or_else(|| resource(Resource::Arithmetic))?;
                        Ok(root)
                    })
                    .collect::<Result<Vec<_>, ProductionRankedProjectionErrorV1>>()?;
                Ok((roots.into_boxed_slice(), retained))
            },
        )
    }
}

fn finish_root(
    mut root: ProductionRankedRootProgramV1,
    references: &References,
    source: &RankedProjectionSourceV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<ProductionRankedRootProgramV1, ProductionRankedProjectionErrorV1> {
    let semantic = source.semantic_ssa().source_semantic();
    let body = semantic
        .select_kernel_body_for_root_v1(root.semantic_root)
        .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
            "reference continuation source body",
        ))?
        .body();
    root.verification = match root.verification {
        Verification::Pending(request) => {
            let (verification, returned) = continue_reference_v1(
                source.owner(),
                request,
                ReferenceSourceV1 {
                    root: root.semantic_root.index(),
                    rank: root.source_rank,
                    references,
                    access: root.access_sources,
                    effects: root.executable_effect_sources,
                    ranked_ir: root.ranked_ir,
                },
                budget,
            )
            .map_err(|error| {
                ProductionRankedProjectionErrorV1::ReferenceEffectJoin(error)
                    .with_deterministic_root_context(
                        root.semantic_root,
                        body,
                        root.logical_name.as_bytes(),
                    )
            })?;
            root.access_sources = returned.access;
            root.executable_effect_sources = returned.effects;
            root.ranked_ir = returned.ranked_ir;
            verification
        }
        verification => verification,
    };
    if let Some(lowering) = root.verification.ordinary() {
        fe2o3_lower_mir_kernel::validate_borrowed_ranked_semantic_projection_candidate_with_generated_effects_v1(
            source.semantic_ssa().source_owner(), root.semantic_root, lowering,
            &root.ranked_ir, &root.access_sources, &root.executable_effect_sources,
        ).map_err(|error| ProductionRankedProjectionErrorV1::StructuralValidation(error)
            .with_deterministic_root_context(root.semantic_root, body, root.logical_name.as_bytes()))?;
    }
    Ok(root)
}
