//! Genuine checked continuation of MIR36 source ownership into KIR19.
//! The ranked graph is a safety-analysis projection, never the executable.
//! The original immutable KIR19 owner remains the only compilation subject.
use super::*;
use fe2o3_kernel_ir::{
    ExplicitLaunchExtent, FormalIndexWidth, FormalMemoryObligationAnalysis,
    FormalMemoryObligations, VerifiedCanonicalKernelIrModuleV19,
    derive_complete_body_memory_obligations_v19,
};
use fe2o3_pliron::{
    ProductionConstructionV1, ProductionMiddleEndEvidenceV5, ProductionRankedKernelLoweringInputV1,
    ProductionSessionLimitsV1, compile_ranked_kernel_for_gfx942_lowering_v1,
};

#[path = "production_complete_body_ranked_v19.rs"]
mod ranked;
use ranked::complete_body_ranked_recipe_v19;

const RANKED_TEXT_LIMIT_V19: usize = 64 * 1024;
const FORMAL_RETAINED_LIMIT_V19: usize = 64 * 1024;
const EVIDENCE_RETAINED_LIMIT_V19: usize =
    fe2o3_pliron::MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V5;

/// A source, analysis-relation or mandatory verifier failure.
#[derive(Debug)]
pub enum ProductionCompleteBodyCheckErrorV19 {
    /// The original retained source relation failed.
    Source(ProductionCompleteBodySourceErrorVNext),
    /// The common source/projection resource ledger refused.
    Resource(ArgumentResourceV1),
    /// An exact source, executable or analysis projection relation failed.
    Relation(&'static str),
    /// The closed ranked analysis recipe was structurally invalid.
    RankedRecipe(fe2o3_pliron::ProductionRankedKernelErrorV1),
    /// The existing mandatory ranked checks refused the actual projection.
    Ranked(fe2o3_pliron::ProductionRankedCompileErrorV1),
    /// Existing checked middle-end evidence could not be retained or replayed.
    Evidence(fe2o3_pliron::ProductionMiddleEndEvidenceCodecErrorV5),
    /// Existing formal-memory derivation failed on the actual KIR19 graph.
    Formal(fe2o3_kernel_ir::FormalMemoryObligationError),
}
impl From<ArgumentResourceV1> for ProductionCompleteBodyCheckErrorV19 {
    fn from(error: ArgumentResourceV1) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for ProductionCompleteBodyCheckErrorV19 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "complete-body checked continuation: {self:?}")
    }
}
impl Error for ProductionCompleteBodyCheckErrorV19 {}

/// Move-only source, canonical, ranked-check and formal-memory custody.
///
/// This owner cannot be made from canonical bytes, a model plan or detached
/// report. It consumes the retained source owner and independently reconstructs
/// the complete safety projection from the one actual executable graph.
/// It proves neither a user algorithm nor hardware execution.
///
/// The common ledger covers source replay and added bounded recipe/text/report
/// payload. The existing ranked PLIRON session and formal analyzer retain their
/// own independently bounded resource domains, exactly as in the ordinary
/// compiler path. The receipt is not an aggregate compiler heap/RSS bound.
#[must_use]
pub struct ProductionCompleteBodyCheckedKirOwnerV19 {
    materialized: ProductionCompleteBodyPreRankedKirOwnerVNext,
    ranked: ProductionRankedKernelLoweringInputV1,
    middle_end: ProductionMiddleEndEvidenceV5,
    ranked_text: String,
    formal: FormalMemoryObligations,
    retained_storage: usize,
}
impl ProductionCompleteBodyCheckedKirOwnerV19 {
    /// Consumes the source owner and runs the fixed mandatory general checks.
    ///
    /// The input owner's receipt must already be reserved. On every exit the
    /// incoming storage floor is restored; work is cumulative. On success the
    /// caller reserves the difference between this owner's total receipt and
    /// the consumed input receipt. No caller chooses a verifier/refinement policy.
    pub fn try_check(
        materialized: ProductionCompleteBodyPreRankedKirOwnerVNext,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionCompleteBodyCheckErrorV19> {
        let input_storage = materialized.retained_storage();
        if budget.storage() < input_storage {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        materialized
            .verify_equivalence(budget)
            .map_err(ProductionCompleteBodyCheckErrorV19::Source)?;
        let mut scope = CompleteBodyLedgerScopeVNext::new(budget);
        let added = argument_sum_v1(&[
            std::mem::size_of::<Self>(),
            ranked::PROJECTION_STORAGE_V19,
            RANKED_TEXT_LIMIT_V19,
            FORMAL_RETAINED_LIMIT_V19,
            EVIDENCE_RETAINED_LIMIT_V19,
        ])?;
        // Projection reserves its own fixed output payload before allocation.
        scope.reserve(added - ranked::PROJECTION_STORAGE_V19)?;
        let recipe = complete_body_ranked_recipe_v19(
            materialized.executable(),
            materialized.source_launch(),
            scope.budget,
        )?;
        let ranked_text = ranked_text_v19(&recipe)?;
        let construction = ProductionConstructionV1::ranked_kernel("complete_body_v19", recipe)
            .map_err(|_| {
                ProductionCompleteBodyCheckErrorV19::Relation("ranked construction name")
            })?;
        // This is the existing fixed production checker, with its existing
        // independent session/resource admission, not a new analysis engine.
        let ranked = compile_ranked_kernel_for_gfx942_lowering_v1(
            construction,
            ProductionSessionLimitsV1::default(),
            std::iter::empty(),
        )
        .map_err(ProductionCompleteBodyCheckErrorV19::Ranked)?;
        if !ranked.all_mandatory_reports_are_clean()
            || ranked.has_retained_policy_checked_refinement_staging()
        {
            return Err(ProductionCompleteBodyCheckErrorV19::Relation(
                "unclean or substituted ranked policy",
            ));
        }
        let middle_end = ProductionMiddleEndEvidenceV5::try_new(
            materialized.semantic_ssa().source_owner(),
            &ranked,
            &ranked_text,
        )
        .map_err(ProductionCompleteBodyCheckErrorV19::Evidence)?;
        let formal = complete_body_formal_v19(&materialized)?;
        let retained_storage = argument_sum_v1(&[input_storage, added])?;
        let owner = Self {
            materialized,
            ranked,
            middle_end,
            ranked_text,
            formal,
            retained_storage,
        };
        // Input + all added receipts are live in this scope during exact replay.
        owner.verify_equivalence(scope.budget)?;
        Ok(owner)
    }

    /// Reconciles the actual retained source, projection, reports and full
    /// runtime memory-obligation records. Identity-only comparisons do not
    /// substitute for the full reconstructed analysis recipe and obligations.
    pub fn verify_equivalence(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionCompleteBodyCheckErrorV19> {
        if budget.storage() < self.retained_storage {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        self.materialized
            .verify_equivalence(budget)
            .map_err(ProductionCompleteBodyCheckErrorV19::Source)?;
        let mut scope = CompleteBodyLedgerScopeVNext::new(budget);
        scope.reserve(argument_sum_v1(&[
            RANKED_TEXT_LIMIT_V19,
            FORMAL_RETAINED_LIMIT_V19,
            EVIDENCE_RETAINED_LIMIT_V19,
        ])?)?;
        let recipe = complete_body_ranked_recipe_v19(
            self.executable(),
            self.materialized.source_launch(),
            scope.budget,
        )?;
        scope.budget.charge_work(16_384)?;
        if &recipe != self.ranked.kernel()
            || ranked_text_v19(&recipe)? != self.ranked_text
            || !self.ranked.all_mandatory_reports_are_clean()
            || self.ranked.has_retained_policy_checked_refinement_staging()
        {
            return Err(ProductionCompleteBodyCheckErrorV19::Relation(
                "ranked graph/report relation changed",
            ));
        }
        let middle_end = ProductionMiddleEndEvidenceV5::try_new(
            self.materialized.semantic_ssa().source_owner(),
            &self.ranked,
            &self.ranked_text,
        )
        .map_err(ProductionCompleteBodyCheckErrorV19::Evidence)?;
        if middle_end.canonical_bytes() != self.middle_end.canonical_bytes()
            || complete_body_formal_v19(&self.materialized)? != self.formal
        {
            return Err(ProductionCompleteBodyCheckErrorV19::Relation(
                "middle-end or formal obligations changed",
            ));
        }
        Ok(())
    }
    /// Actual source-to-executable owner, including exact launch and correspondence.
    pub const fn materialized(&self) -> &ProductionCompleteBodyPreRankedKirOwnerVNext {
        &self.materialized
    }
    /// Retained semantic SSA, not a regenerated or diagnostic source model.
    pub fn semantic_ssa(&self) -> &ProductionSemanticSsaOwnerV1 {
        self.materialized.semantic_ssa()
    }
    /// One immutable exact KIR19 executable; no V12 projection is provided.
    pub fn executable(&self) -> &VerifiedCanonicalKernelIrModuleV19 {
        self.materialized.executable()
    }
    /// Complete actual-body formal obligations; runtime allocation facts remain obligations.
    pub const fn formal_obligations(&self) -> &FormalMemoryObligations {
        &self.formal
    }
    /// Current source-derived launch; this does not authenticate a native launch.
    pub fn source_launch(&self) -> &crate::ProductionSourceLaunchRosterV1 {
        self.materialized.source_launch()
    }
    /// Total logical source/projection/text/report reservation, excluding the
    /// separately bounded ranked-session/formal-analyzer allocation domains.
    pub const fn retained_storage(&self) -> usize {
        self.retained_storage
    }
    /// Mandatory production-analysis resource bound retained by the real session.
    pub fn ranked_analysis_retained_storage_upper_bound(&self) -> usize {
        self.ranked
            .production_analysis_retained_storage_upper_bound_v1()
    }
    /// Internal analysis evidence, not a functional proof or machine certificate.
    pub const fn middle_end_evidence(&self) -> &ProductionMiddleEndEvidenceV5 {
        &self.middle_end
    }
    /// No artifact, protected publication or hardware launch authority is granted.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

fn complete_body_formal_v19(
    materialized: &ProductionCompleteBodyPreRankedKirOwnerVNext,
) -> Result<FormalMemoryObligations, ProductionCompleteBodyCheckErrorV19> {
    let [kernel] = materialized.executable().module().kernels.as_slice() else {
        return Err(ProductionCompleteBodyCheckErrorV19::Relation(
            "formal kernel roster",
        ));
    };
    let [root] = materialized.source_launch().roots() else {
        return Err(ProductionCompleteBodyCheckErrorV19::Relation(
            "formal source launch roster",
        ));
    };
    let report = derive_complete_body_memory_obligations_v19(
        materialized.executable(),
        &kernel.id,
        ExplicitLaunchExtent::Exact {
            rank: 1,
            extents: root.layout().global_extents(),
        },
        FormalIndexWidth::Bits64,
    )
    .map_err(ProductionCompleteBodyCheckErrorV19::Formal)?;
    let FormalMemoryObligationAnalysis::Complete(obligations) = report else {
        return Err(ProductionCompleteBodyCheckErrorV19::Relation(
            "actual body formal analysis remains incomplete",
        ));
    };
    if obligations.allocations().len() != 1
        || obligations.accesses().len() != 1
        || !obligations.inter_invocation_conflicts().is_empty()
        || !obligations.runtime_alias_requirements().is_empty()
        || obligations.bounds_requirements().len() > 1
    {
        return Err(ProductionCompleteBodyCheckErrorV19::Relation(
            "unexpected formal effect/obligation roster",
        ));
    }
    Ok(obligations)
}

/// Bounded deterministic inspection text for the actual safety recipe. Its
/// explicit domain labels it analysis-only; it is not LLVM or executable KIR.
fn ranked_text_v19(
    recipe: &fe2o3_pliron::ProductionRankedKernelV1,
) -> Result<String, ProductionCompleteBodyCheckErrorV19> {
    struct Text(String);
    impl fmt::Write for Text {
        fn write_str(&mut self, text: &str) -> fmt::Result {
            if self
                .0
                .len()
                .checked_add(text.len())
                .is_none_or(|n| n > RANKED_TEXT_LIMIT_V19)
            {
                return Err(fmt::Error);
            }
            self.0.push_str(text);
            Ok(())
        }
    }
    let mut value = String::new();
    value
        .try_reserve_exact(RANKED_TEXT_LIMIT_V19)
        .map_err(|_| ArgumentResourceV1::Allocation)?;
    if value.capacity() > RANKED_TEXT_LIMIT_V19 {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    let mut text = Text(value);
    fmt::write(
        &mut text,
        format_args!("complete-body-v19-safety-projection {recipe:#?}\n"),
    )
    .map_err(|_| ProductionCompleteBodyCheckErrorV19::Relation("ranked inspection text bound"))?;
    Ok(text.0)
}
