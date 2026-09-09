//! Live original-source / execution-view / KIR join. Decoders remain inert.

use super::ProductionSemanticLineageErrorV3;
use fe2o3_compiler_lineage::{
    ExpandedSourceRootV2, InertExpandedSourceEvidenceV2, MultiRootCorrespondenceInputsV3,
    MultiRootCorrespondencePayloadV3, MultiRootInductionKindV3,
};
use fe2o3_lower_mir_kernel::{
    InertCanonicalMirToKirCorrespondenceEvidenceV6, MirToKirInductionEvidenceV6,
    ProductionSemanticKirOwnerV1, ReplayedMirToKirCorrespondenceV6,
};
use fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1;
use fe2o3_mir_model::{
    SemanticU32InductionAnalysisLimitsV1, SemanticU32InductionNoOverflowReportV1,
};

type Result<T> = std::result::Result<T, ProductionSemanticLineageErrorV3>;

fn live_error(error: impl std::fmt::Display) -> ProductionSemanticLineageErrorV3 {
    ProductionSemanticLineageErrorV3::LiveOwner(error.to_string())
}

// Move-only custody, constructed only by replay against the still-live compiler owner.
pub(super) struct PreparedExpandedSourceV2 {
    source: InertExpandedSourceEvidenceV2,
}

impl PreparedExpandedSourceV2 {
    pub(super) fn from_live_owner(
        owner: &ProductionSemanticKirOwnerV1,
        reports: &[(
            SemanticFunctionIdV1,
            &SemanticU32InductionNoOverflowReportV1,
        )],
    ) -> Result<Self> {
        let limits = SemanticU32InductionAnalysisLimitsV1::default();
        let correspondence =
            InertCanonicalMirToKirCorrespondenceEvidenceV6::from_live_owner(owner, reports, limits)
                .map_err(live_error)?;
        Self::replay_and_bind(&correspondence, owner)
    }

    fn replay_and_bind(
        correspondence: &InertCanonicalMirToKirCorrespondenceEvidenceV6,
        owner: &ProductionSemanticKirOwnerV1,
    ) -> Result<Self> {
        let replayed = correspondence
            .verify_replay(owner, SemanticU32InductionAnalysisLimitsV1::default())
            .map_err(live_error)?;
        Ok(Self {
            source: bind_replayed_source(&replayed)?,
        })
    }

    pub(super) fn source(&self) -> &InertExpandedSourceEvidenceV2 {
        &self.source
    }

    pub(super) fn revalidate(
        &self,
        lineage: &fe2o3_compiler_lineage::InertMultiRootProofLineageV3,
    ) -> Result<()> {
        let decoded = InertExpandedSourceEvidenceV2::decode(self.source.canonical_bytes())
            .map_err(live_error)?;
        if decoded != self.source {
            return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
                "expanded source custody changed",
            ));
        }
        decoded
            .validate_against_lineage(lineage)
            .map_err(live_error)
    }
}

fn bind_replayed_source(
    replayed: &ReplayedMirToKirCorrespondenceV6<'_>,
) -> Result<InertExpandedSourceEvidenceV2> {
    let owner = replayed.owner();
    let source = owner.semantic().semantic();
    let evidence = replayed.evidence();
    let mut roots = Vec::with_capacity(evidence.roots().len());
    for (ordinal, (root, function)) in evidence
        .roots()
        .iter()
        .zip(evidence.functions())
        .enumerate()
    {
        let physical = &source.functions()[root.root().index() as usize];
        let body = &source.functions()[root.source_body().index() as usize];
        let view = owner
            .semantic_ssa()
            .execution_view_for_root(root.root())
            .ok_or(ProductionSemanticLineageErrorV3::AxisMismatch(
                "replayed root lost its execution view",
            ))?;
        let entry =
            physical
                .kernel_entry()
                .ok_or(ProductionSemanticLineageErrorV3::AxisMismatch(
                    "replayed physical root lost its export",
                ))?;
        let symbol = std::str::from_utf8(entry.export_symbol().as_bytes()).map_err(live_error)?;
        if function.root() != root.root()
            || function.source_body() != root.source_body()
            || function.kernel_ir_function().as_str() != symbol
            || view.identity() != root.execution_view_identity()
            || evidence.roots().len() != evidence.functions().len()
        {
            return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
                "replayed root/function correspondence differs",
            ));
        }
        let (induction_kind, induction_identity) = match root.induction() {
            MirToKirInductionEvidenceV6::Original(value) => {
                (MultiRootInductionKindV3::OriginalV1, *value.identity())
            }
            MirToKirInductionEvidenceV6::Expanded(value) => {
                (MultiRootInductionKindV3::ExpandedV2, *value.identity())
            }
        };
        let coordinates = MultiRootCorrespondencePayloadV3::new(MultiRootCorrespondenceInputsV3 {
            root_ordinal: u32::try_from(ordinal).map_err(live_error)?,
            semantic_root: root.root().index(),
            selected_body: root.source_body().index(),
            kernel_function_ordinal: function.kernel_ir_function_ordinal(),
            semantic_mir_sha256: *source.semantic_sha256().as_bytes(),
            correspondence_identity: *evidence.identity(),
            expansion_evidence_identity: *evidence.expansion().identity(),
            semantic_root_identity: *physical.identity().as_bytes(),
            selected_body_identity: *body.identity().as_bytes(),
            execution_view_identity: *view.identity(),
            execution_function_identity: *view.body().identity().as_bytes(),
            induction_identity,
            induction_kind,
        })
        .map_err(live_error)?;
        roots.push(
            ExpandedSourceRootV2::new(
                coordinates,
                *entry.kernel_binding_identity().as_bytes(),
                symbol,
            )
            .map_err(live_error)?,
        );
    }
    let kir = evidence.canonical_kernel_ir();
    InertExpandedSourceEvidenceV2::new(
        *kir.digest(),
        kir.canonical_length(),
        roots,
        evidence.canonical_bytes(),
    )
    .map_err(live_error)
}

#[cfg(test)]
mod tests;
