/// Move-only versioned custody. Ordinary kernels retain their historical V5
/// bytes; only a live static-publication proof selects the V6 policy.
#[must_use = "dropping middle-end custody abandons ranked verification"]
pub(crate) enum CompilerMiddleEndEvidenceV1 {
    LegacyV5(fe2o3_pliron::ProductionMiddleEndEvidenceV5),
    PublicationV6(fe2o3_pliron::ProductionMiddleEndEvidenceV6),
}

impl CompilerMiddleEndEvidenceV1 {
    fn try_new(
        semantic_owner: &ProductionSemanticMirOwnerV1,
        lowering: &ProductionRankedKernelLoweringInputV1,
        ranked_ir: &str,
    ) -> Result<Self, ProductionRankedVerificationErrorV1> {
        if lowering.race_report().static_publication().is_some() {
            fe2o3_pliron::ProductionMiddleEndEvidenceV6::try_new(
                semantic_owner,
                lowering,
                ranked_ir,
            )
            .map(Self::PublicationV6)
            .map_err(ProductionRankedVerificationErrorV1::PublicationMiddleEndEvidence)
        } else {
            fe2o3_pliron::ProductionMiddleEndEvidenceV5::try_new(
                semantic_owner,
                lowering,
                ranked_ir,
            )
            .map(Self::LegacyV5)
            .map_err(ProductionRankedVerificationErrorV1::MiddleEndEvidence)
        }
    }

    pub(crate) fn view(&self) -> &dyn fe2o3_pliron::ProductionMiddleEndEvidenceViewV1 {
        match self {
            Self::LegacyV5(evidence) => evidence,
            Self::PublicationV6(evidence) => evidence,
        }
    }

    pub(crate) fn identity(&self) -> fe2o3_pliron::ProductionMiddleEndLiveIdentityV1 {
        self.view().identity()
    }

    pub(crate) fn canonical_bytes(&self) -> &[u8] {
        self.view().canonical_bytes()
    }

    pub(crate) const fn static_publication(
        &self,
    ) -> Option<&fe2o3_pliron::ProductionMiddleEndPublicationSummaryV6> {
        match self {
            Self::LegacyV5(_) => None,
            Self::PublicationV6(evidence) => evidence.static_publication(),
        }
    }
}
