//! Mandatory checked continuation for source-owned ordered compositions.
//! The canonical executable/call graph is never replaced by its safety view.
use super::*;
use fe2o3_kernel_ir::{
    ExplicitLaunchExtent, FormalIndexWidth, FormalMemoryObligationAnalysis,
    FormalMemoryObligations, OrderedCompositionFormalErrorV1,
    derive_ordered_composition_memory_obligations_v1,
};
use fe2o3_pliron::{
    ProductionConstructionV1, ProductionMiddleEndEvidenceV5, ProductionRankedKernelLoweringInputV1,
    ProductionSessionLimitsV1, compile_ranked_kernel_for_gfx942_lowering_v1,
};
#[path = "production_ordered_composition_ranked_v1.rs"]
mod ranked;
pub use ranked::{
    OrderedCompositionLaunchEnvelopeRequirementV1, OrderedCompositionRankedDependencyV1,
};

const TEXT_LIMIT: usize = 512 * 1024;
const EVIDENCE_LIMIT: usize = fe2o3_pliron::MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V5;

/// Source, mandatory-analysis or exact retained-relation refusal.
#[derive(Debug)]
pub enum ProductionOrderedCompositionCheckErrorV1 {
    /// Source/canonical materialization or replay failed.
    Source(ProductionOrderedCompositionErrorV1),
    /// The cumulative verification resource budget was exhausted or inconsistent.
    Resource(ArgumentResourceV1),
    /// The exact retained source/analysis relationship or supported profile failed.
    Relation(&'static str),
    /// Memory-obligation extraction failed; no condition is discharged.
    Formal(OrderedCompositionFormalErrorV1),
    /// Constructing the conservative safety recipe failed.
    RankedRecipe(fe2o3_pliron::ProductionRankedKernelErrorV1),
    /// Mandatory ranked analysis refused the safety projection.
    Ranked(fe2o3_pliron::ProductionRankedCompileErrorV1),
    /// Binding the mandatory analysis to the source owner failed.
    Evidence(fe2o3_pliron::ProductionMiddleEndEvidenceCodecErrorV5),
}
impl From<ArgumentResourceV1> for ProductionOrderedCompositionCheckErrorV1 {
    fn from(e: ArgumentResourceV1) -> Self {
        Self::Resource(e)
    }
}
impl fmt::Display for ProductionOrderedCompositionCheckErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ordered composition checked continuation: {self:?}")
    }
}
impl Error for ProductionOrderedCompositionCheckErrorV1 {}
#[derive(Debug)]
enum OrderedCompositionCheckAuxV1 {
    Resource(ArgumentResourceV1),
    Relation(&'static str),
    Formal(OrderedCompositionFormalErrorV1),
    RankedRecipe(fe2o3_pliron::ProductionRankedKernelErrorV1),
}
impl From<ArgumentResourceV1> for OrderedCompositionCheckAuxV1 {
    fn from(e: ArgumentResourceV1) -> Self {
        Self::Resource(e)
    }
}
impl From<OrderedCompositionCheckAuxV1> for ProductionOrderedCompositionCheckErrorV1 {
    fn from(e: OrderedCompositionCheckAuxV1) -> Self {
        match e {
            OrderedCompositionCheckAuxV1::Resource(e) => Self::Resource(e),
            OrderedCompositionCheckAuxV1::Relation(e) => Self::Relation(e),
            OrderedCompositionCheckAuxV1::Formal(e) => Self::Formal(e),
            OrderedCompositionCheckAuxV1::RankedRecipe(e) => Self::RankedRecipe(e),
        }
    }
}
const _: () = assert!(std::mem::size_of::<OrderedCompositionCheckAuxV1>() < 128);

/// Move-only source/canonical/mandatory-analysis custody. No caller supplies
/// a projection, policy, report or helper effect declaration.
///
/// The ranked graph is a safety over-approximation, not functional equivalence.
/// Actual formal conditions AND additional full-launch buffer requirements stay
/// unresolved. The owner grants no artifact, launch, runtime safety or native
/// physical-register authority. Source authentication still belongs to the
/// dedicated backend constructor, not identities or diagnostic byte loaders.
///
/// The receipt covers additional logical retained projection/text/report
/// payload. Existing ranked/formal analyzer auxiliary domains and source SSA
/// domains remain independently bounded; it is not an aggregate heap/RSS cap.
///
/// try_check preserves the input storage floor on all exits. Its input receipt
/// plus attached source-occurrence receipt must already be live. On success,
/// reserve this receipt minus the consumed pre-owner's receipt; keep the
/// occurrence receipt independently reserved as before.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionOrderedCompositionCheckedKirOwnerV1;
/// fn clone_owner(owner: ProductionOrderedCompositionCheckedKirOwnerV1) { let _ = owner.clone(); }
/// ```
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionOrderedCompositionCheckedKirOwnerV1;
/// fn mutate(owner: &mut ProductionOrderedCompositionCheckedKirOwnerV1) {
///     owner.executable().module().functions.clear();
/// }
/// ```
#[must_use]
pub struct ProductionOrderedCompositionCheckedKirOwnerV1 {
    materialized: ProductionOrderedCompositionPreRankedKirOwnerV1,
    ranked: ProductionRankedKernelLoweringInputV1,
    evidence: ProductionMiddleEndEvidenceV5,
    text: String,
    formal: FormalMemoryObligations,
    dependencies: Vec<OrderedCompositionRankedDependencyV1>,
    bounds: Vec<OrderedCompositionLaunchEnvelopeRequirementV1>,
    retained: usize,
}
impl ProductionOrderedCompositionCheckedKirOwnerV1 {
    /// Consumes the materialized owner and runs mandatory formal and ranked checks.
    pub fn try_check(
        materialized: ProductionOrderedCompositionPreRankedKirOwnerV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionOrderedCompositionCheckErrorV1> {
        materialized
            .verify_equivalence(budget)
            .map_err(ProductionOrderedCompositionCheckErrorV1::Source)?;
        let floor = budget.storage();
        let added = added_storage()?;
        budget.with_prepaid_scope(floor, 1, 1, added, move |budget| {
            let formal = formal(&materialized, budget)?;
            let projection = ranked::project(
                materialized.composition(),
                materialized.source_launch(),
                &formal,
                budget,
            )?;
            let text = projection_text(&projection.recipe)?;
            let construction = ProductionConstructionV1::ranked_kernel(
                "ordered_composition_v1",
                projection.recipe,
            )
            .map_err(|_| {
                ProductionOrderedCompositionCheckErrorV1::Relation("ranked construction")
            })?;
            let ranked = compile_ranked_kernel_for_gfx942_lowering_v1(
                construction,
                ProductionSessionLimitsV1::default(),
                std::iter::empty(),
            )
            .map_err(ProductionOrderedCompositionCheckErrorV1::Ranked)?;
            if !ranked.all_mandatory_reports_are_clean()
                || ranked.has_retained_policy_checked_refinement_staging()
            {
                return Err(ProductionOrderedCompositionCheckErrorV1::Relation(
                    "mandatory ranked report/policy",
                ));
            }
            let evidence = ProductionMiddleEndEvidenceV5::try_new(
                materialized.semantic_ssa().source_owner(),
                &ranked,
                &text,
            )
            .map_err(ProductionOrderedCompositionCheckErrorV1::Evidence)?;
            let retained =
                argument_sum_v1(&[materialized.retained_storage().retained_storage(), added])?;
            let owner = Self {
                materialized,
                ranked,
                evidence,
                text,
                formal,
                dependencies: projection.dependencies,
                bounds: projection.bounds,
                retained,
            };
            owner.verify_equivalence(budget)?;
            Ok(owner)
        })
    }
    /// Replays the retained source, memory conditions, dependency graph and evidence.
    pub fn verify_equivalence(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionOrderedCompositionCheckErrorV1> {
        let floor = argument_sum_v1(&[
            self.retained,
            self.materialized
                .semantic_ssa()
                .occurrence_storage()
                .ok_or(ArgumentResourceV1::Accounting)?
                .retained_storage(),
        ])?;
        if budget.storage() < floor {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        self.materialized
            .verify_equivalence(budget)
            .map_err(ProductionOrderedCompositionCheckErrorV1::Source)?;
        budget.with_prepaid_scope(floor, 1, 131_072, added_storage()?, |budget| {
            let formal = formal(&self.materialized, budget)?;
            let projection =
                ranked::project(self.composition(), self.source_launch(), &formal, budget)?;
            let text = projection_text(&projection.recipe)?;
            if formal != self.formal
                || projection.recipe != *self.ranked.kernel()
                || projection.dependencies != self.dependencies
                || projection.bounds != self.bounds
                || text != self.text
                || !self.ranked.all_mandatory_reports_are_clean()
                || self.ranked.has_retained_policy_checked_refinement_staging()
            {
                return Err(ProductionOrderedCompositionCheckErrorV1::Relation(
                    "same-source full formal/projection/report relation changed",
                ));
            }
            let evidence = ProductionMiddleEndEvidenceV5::try_new(
                self.semantic_ssa().source_owner(),
                &self.ranked,
                &text,
            )
            .map_err(ProductionOrderedCompositionCheckErrorV1::Evidence)?;
            if evidence.canonical_bytes() != self.evidence.canonical_bytes() {
                return Err(ProductionOrderedCompositionCheckErrorV1::Relation(
                    "middle-end evidence changed",
                ));
            }
            Ok(())
        })
    }
    /// Borrows the original materialization owner, retaining its source custody.
    pub const fn materialized(&self) -> &ProductionOrderedCompositionPreRankedKirOwnerV1 {
        &self.materialized
    }
    /// Borrows the unchanged source-owned semantic SSA.
    pub fn semantic_ssa(&self) -> &ProductionSemanticSsaOwnerV1 {
        self.materialized.semantic_ssa()
    }
    /// Borrows the structural composition of the original canonical executable.
    pub fn composition(&self) -> &fe2o3_kernel_ir::VerifiedOrderedProgramCompositionV1 {
        self.materialized.composition()
    }
    /// Borrows the original complete V17 executable, not the safety projection.
    pub fn executable(&self) -> &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV17 {
        self.materialized.executable()
    }
    /// Borrows the exact launch roster retained from source.
    pub fn source_launch(&self) -> &crate::ProductionSourceLaunchRosterV1 {
        self.materialized.source_launch()
    }
    /// Returns actual root memory obligations, including unresolved host conditions.
    pub const fn formal_obligations(&self) -> &FormalMemoryObligations {
        &self.formal
    }
    /// Returns additional unresolved full-launch buffer requirements.
    pub fn launch_envelope_requirements(&self) -> &[OrderedCompositionLaunchEnvelopeRequirementV1] {
        &self.bounds
    }
    /// Returns call-specific source attribution for the safety dependency graph.
    pub fn ranked_dependencies(&self) -> &[OrderedCompositionRankedDependencyV1] {
        &self.dependencies
    }
    /// Borrows the source-bound mandatory middle-end analysis evidence.
    pub const fn middle_end_evidence(&self) -> &ProductionMiddleEndEvidenceV5 {
        &self.evidence
    }
    /// Returns the logical retained receipt, including the consumed pre-owner receipt.
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
    /// Returns the separately bounded retained ranked-analysis allocation domain.
    pub fn ranked_analysis_retained_storage_upper_bound(&self) -> usize {
        self.ranked
            .production_analysis_retained_storage_upper_bound_v1()
    }
    /// Always false: analysis custody alone cannot authorize an artifact or launch.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}
fn added_storage() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        std::mem::size_of::<ProductionOrderedCompositionCheckedKirOwnerV1>(),
        ranked::PROJECTION_STORAGE,
        TEXT_LIMIT,
        EVIDENCE_LIMIT,
        fe2o3_kernel_ir::ORDERED_COMPOSITION_FORMAL_RETAINED_BYTES_V1,
    ])
}
fn formal(
    owner: &ProductionOrderedCompositionPreRankedKirOwnerV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<FormalMemoryObligations, OrderedCompositionCheckAuxV1> {
    let [root] = owner.source_launch().roots() else {
        return Err(OrderedCompositionCheckAuxV1::Relation(
            "one actual source launch",
        ));
    };
    let [kernel] = owner.executable().module().kernels.as_slice() else {
        return Err(OrderedCompositionCheckAuxV1::Relation("one actual kernel"));
    };
    let (report, _storage) = derive_ordered_composition_memory_obligations_v1(
        owner.composition(),
        &kernel.id,
        ExplicitLaunchExtent::Exact {
            rank: 1,
            extents: root.layout().global_extents(),
        },
        FormalIndexWidth::Bits64,
        budget,
    )
    .map_err(OrderedCompositionCheckAuxV1::Formal)?;
    let FormalMemoryObligationAnalysis::Complete(report) = report else {
        return Err(OrderedCompositionCheckAuxV1::Relation(
            "actual root formal effects remain incomplete",
        ));
    };
    if !report.inter_invocation_conflicts().is_empty() {
        return Err(OrderedCompositionCheckAuxV1::Relation(
            "actual root has inter-invocation conflicts",
        ));
    }
    Ok(report)
}
fn projection_text(
    recipe: &fe2o3_pliron::ProductionRankedKernelV1,
) -> Result<String, OrderedCompositionCheckAuxV1> {
    struct Text(String);
    impl fmt::Write for Text {
        fn write_str(&mut self, text: &str) -> fmt::Result {
            if self
                .0
                .len()
                .checked_add(text.len())
                .is_none_or(|n| n > TEXT_LIMIT)
            {
                return Err(fmt::Error);
            }
            self.0.push_str(text);
            Ok(())
        }
    }
    let mut s = String::new();
    s.try_reserve_exact(TEXT_LIMIT)
        .map_err(|_| ArgumentResourceV1::Allocation)?;
    if s.capacity() != TEXT_LIMIT {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    let mut text = Text(s);
    fmt::write(
        &mut text,
        format_args!("ordered-composition-v1-safety-overapproximation {recipe:#?}\n"),
    )
    .map_err(|_| OrderedCompositionCheckAuxV1::Relation("ranked text bound"))?;
    Ok(text.0)
}

#[cfg(test)]
pub(crate) fn substitute_dependency_for_test(
    owner: &mut ProductionOrderedCompositionCheckedKirOwnerV1,
) {
    owner.dependencies[0] = ranked::substituted_row_for_test(owner.dependencies[0]);
}

#[cfg(test)]
pub(crate) fn project_for_test(
    owner: &fe2o3_kernel_ir::VerifiedOrderedProgramCompositionV1,
    launch: &crate::ProductionSourceLaunchRosterV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<
    (
        fe2o3_pliron::ProductionRankedKernelV1,
        Vec<OrderedCompositionLaunchEnvelopeRequirementV1>,
    ),
    String,
> {
    let (formal, _) = derive_ordered_composition_memory_obligations_v1(
        owner,
        &owner.canonical().module().kernels[0].id,
        ExplicitLaunchExtent::Exact {
            rank: 1,
            extents: launch.roots()[0].layout().global_extents(),
        },
        FormalIndexWidth::Bits64,
        budget,
    )
    .map_err(|e| format!("{e:?}"))?;
    let FormalMemoryObligationAnalysis::Complete(formal) = formal else {
        return Err("incomplete".into());
    };
    let projection =
        ranked::project(owner, launch, &formal, budget).map_err(|e| format!("{e:?}"))?;
    Ok((projection.recipe, projection.bounds))
}
