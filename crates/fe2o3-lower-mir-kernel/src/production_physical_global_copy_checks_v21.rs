//! Genuine fixed-policy ranked/formal continuation of source-owned KIR21.
//! The analysis is conditional on the retained compiler-ABI memory obligations;
//! It does not discharge full-launch input bounds/initialization/readability,
//! output writability/bounds, input/output alias or kernarg lifetime conditions.
use super::*;
use fe2o3_kernel_ir::{
    ExplicitLaunchExtent, FormalIndexWidth, PhysicalGlobalCopyMemoryErrorV21,
    PhysicalGlobalCopyMemoryObligationsV21, PhysicalGlobalCopyMemoryStorageV21,
    VerifiedCanonicalKernelIrModuleV21, derive_physical_global_copy_memory_obligations_v21,
};
use fe2o3_pliron::{
    ProductionConstructionV1, ProductionMiddleEndEvidenceV5, ProductionRankedKernelLoweringInputV1,
    ProductionSessionLimitsV1, compile_ranked_kernel_for_gfx942_lowering_v1,
};
#[path = "production_physical_global_copy_ranked_v21.rs"]
mod ranked;
const TEXT_LIMIT: usize = 512 * 1024;
const EVIDENCE_LIMIT: usize = fe2o3_pliron::MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V5;

/// Complete source, physical-memory, resource or mandatory-analysis failure.
#[derive(Debug)]
pub enum ProductionPhysicalGlobalCopyCheckErrorV21 {
    /// Original retained source/executable correspondence was refused.
    Source(ProductionPhysicalGlobalCopySourceErrorV21),
    /// Common cumulative source/projection resource admission failed.
    Resource(ArgumentResourceV1),
    /// Exact source, canonical, report or analysis relation differs.
    Relation(&'static str),
    /// Fixed ranked recipe was structurally invalid.
    RankedRecipe(fe2o3_pliron::ProductionRankedKernelErrorV1),
    /// Complete report-bearing mandatory ranked-check failure, never reduced.
    Ranked(fe2o3_pliron::ProductionRankedCompileErrorV1),
    /// Existing typed middle-end evidence construction failed.
    Evidence(fe2o3_pliron::ProductionMiddleEndEvidenceCodecErrorV5),
    /// Exact combined input/output and compiler-ABI memory derivation failed.
    Formal(PhysicalGlobalCopyMemoryErrorV21),
}
impl From<ArgumentResourceV1> for ProductionPhysicalGlobalCopyCheckErrorV21 {
    fn from(error: ArgumentResourceV1) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for ProductionPhysicalGlobalCopyCheckErrorV21 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "physical-global-copy checked continuation: {self:?}")
    }
}
impl Error for ProductionPhysicalGlobalCopyCheckErrorV21 {}
// Helpers do not carry the large report-bearing session error. The exhaustive
// move conversion retains all typed diagnostic data, with no allocating box.
#[derive(Debug)]
enum PhysicalGlobalCopyAuxErrorV21 {
    Resource(ArgumentResourceV1),
    Relation(&'static str),
    RankedRecipe(fe2o3_pliron::ProductionRankedKernelErrorV1),
    Formal(PhysicalGlobalCopyMemoryErrorV21),
}
impl From<ArgumentResourceV1> for PhysicalGlobalCopyAuxErrorV21 {
    fn from(error: ArgumentResourceV1) -> Self {
        Self::Resource(error)
    }
}
impl From<PhysicalGlobalCopyAuxErrorV21> for ProductionPhysicalGlobalCopyCheckErrorV21 {
    fn from(error: PhysicalGlobalCopyAuxErrorV21) -> Self {
        match error {
            PhysicalGlobalCopyAuxErrorV21::Resource(e) => Self::Resource(e),
            PhysicalGlobalCopyAuxErrorV21::Relation(e) => Self::Relation(e),
            PhysicalGlobalCopyAuxErrorV21::RankedRecipe(e) => Self::RankedRecipe(e),
            PhysicalGlobalCopyAuxErrorV21::Formal(e) => Self::Formal(e),
        }
    }
}
const _: () = assert!(std::mem::size_of::<PhysicalGlobalCopyAuxErrorV21>() < 128);

/// Move-only source, canonical graph, mandatory ranked reports and memory custody.
///
/// This owner consumes the real source preowner; it cannot be made from model
/// bytes, a caller-selected recipe, detached evidence or a descriptor. The
/// immutable authored body remains the executable; the ranked graph is only
/// its safety-analysis projection. The full-EXEC input read is unguarded;
/// its finite readable prefix is conditional on the retained runtime bounds.
/// Loaded data stays opaque, never a deterministic function of its address.
/// The fixed existing eight-pass production session runs normally and its
/// complete typed reports remain retained.
///
/// The compiler-ABI allocation is not a fabricated third/extra source argument.
/// Its read views and the full-EXEC input prefix rely on explicit retained
/// readability/initialization/bounds and immutability/disjoint preconditions.
/// Those remain unresolved here and must be consumed by the
/// later compiler descriptor/runtime continuation, which is not implemented
/// by this owner. No successful checks erase these unresolved conditions.
/// Clean conditional reports are not proof that any real pointer allocation
/// or launch satisfies them.
///
/// Added logical owner/recipe/text/formal/evidence retention and exact replay
/// use the original cumulative canonical ledger. The established semantic SSA
/// and ranked PLIRON session keep their independently bounded domains; the
/// receipt is not total compiler allocation/RSS or runtime authority.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionPhysicalGlobalCopyCheckedKirOwnerV21;
/// fn copy<T: Clone>() {}
/// copy::<ProductionPhysicalGlobalCopyCheckedKirOwnerV21>();
/// ```
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionPhysicalGlobalCopyCheckedKirOwnerV21;
/// fn mutate(x: &mut ProductionPhysicalGlobalCopyCheckedKirOwnerV21) {
///     x.executable().module().functions.clear();
/// }
/// ```
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionPhysicalGlobalCopyCheckedKirOwnerV21;
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV21, CanonicalKernelIrVerificationResourceBudgetV1};
/// fn forge(x: VerifiedCanonicalKernelIrModuleV21, b: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
///     ProductionPhysicalGlobalCopyCheckedKirOwnerV21::try_check(x, b);
/// }
/// ```
#[must_use]
pub struct ProductionPhysicalGlobalCopyCheckedKirOwnerV21 {
    materialized: ProductionPhysicalGlobalCopyPreRankedKirOwnerV21,
    ranked: ProductionRankedKernelLoweringInputV1,
    middle_end: ProductionMiddleEndEvidenceV5,
    ranked_text: String,
    formal: PhysicalGlobalCopyMemoryObligationsV21,
    retained_storage: usize,
}
impl ProductionPhysicalGlobalCopyCheckedKirOwnerV21 {
    /// Runs fixed mandatory checks while consuming the retained actual source.
    ///
    /// Input storage must already be reserved. Work remains cumulative; incoming
    /// storage is restored on success/error/unwind. On success reserve the
    /// returned total receipt minus the consumed preowner receipt while held.
    pub fn try_check(
        materialized: ProductionPhysicalGlobalCopyPreRankedKirOwnerV21,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionPhysicalGlobalCopyCheckErrorV21> {
        budget.charge_work(1)?;
        let input_storage = materialized.retained_storage();
        if budget.storage() < input_storage {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        materialized
            .verify_equivalence(budget)
            .map_err(ProductionPhysicalGlobalCopyCheckErrorV21::Source)?;
        let mut scope = PhysicalGlobalCopyLedgerScopeV21::new(budget);
        let fixed = argument_sum_v1(&[std::mem::size_of::<Self>(), TEXT_LIMIT, EVIDENCE_LIMIT])?;
        scope.reserve(fixed)?;
        let (formal, formal_storage) = formal_v21(&materialized, scope.budget)?;
        scope.reserve(formal_storage.retained_storage())?;
        let recipe = ranked::physical_global_copy_ranked_recipe_v21(
            materialized.executable(),
            materialized.source_launch(),
            &formal,
            scope.budget,
        )?;
        let ranked_text = inspection_text(&recipe)?;
        let construction = ProductionConstructionV1::ranked_kernel(
            "physical_global_copy_v21",
            recipe,
        )
        .map_err(|_| {
            ProductionPhysicalGlobalCopyCheckErrorV21::Relation("physical ranked construction name")
        })?;
        let ranked = compile_ranked_kernel_for_gfx942_lowering_v1(
            construction,
            ProductionSessionLimitsV1::default(),
            std::iter::empty(),
        )
        .map_err(ProductionPhysicalGlobalCopyCheckErrorV21::Ranked)?;
        if !ranked.all_mandatory_reports_are_clean()
            || ranked.has_retained_policy_checked_refinement_staging()
        {
            return Err(ProductionPhysicalGlobalCopyCheckErrorV21::Relation(
                "physical ranked mandatory policy differs",
            ));
        }
        let middle_end = ProductionMiddleEndEvidenceV5::try_new(
            materialized.semantic_ssa().source_owner(),
            &ranked,
            &ranked_text,
        )
        .map_err(ProductionPhysicalGlobalCopyCheckErrorV21::Evidence)?;
        let retained_storage = argument_sum_v1(&[
            input_storage,
            fixed,
            formal_storage.retained_storage(),
            ranked::STORAGE,
        ])?;
        if scope.budget.storage()
            != argument_sum_v1(&[
                scope.floor,
                fixed,
                formal_storage.retained_storage(),
                ranked::STORAGE,
            ])?
        {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        let output = Self {
            materialized,
            ranked,
            middle_end,
            ranked_text,
            formal,
            retained_storage,
        };
        output.verify_equivalence(scope.budget)?;
        Ok(output)
    }
    /// Reconstructs the source relation, complete typed memory records and exact
    /// ranked recipe. Identity-only equality cannot replace these full joins.
    pub fn verify_equivalence(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionPhysicalGlobalCopyCheckErrorV21> {
        budget.charge_work(1)?;
        if budget.storage() < self.retained_storage {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        self.materialized
            .verify_equivalence(budget)
            .map_err(ProductionPhysicalGlobalCopyCheckErrorV21::Source)?;
        let mut scope = PhysicalGlobalCopyLedgerScopeV21::new(budget);
        scope.reserve(argument_sum_v1(&[TEXT_LIMIT, EVIDENCE_LIMIT])?)?;
        let (formal, formal_storage) = formal_v21(&self.materialized, scope.budget)?;
        scope.reserve(formal_storage.retained_storage())?;
        if formal != self.formal {
            return Err(ProductionPhysicalGlobalCopyCheckErrorV21::Relation(
                "physical complete memory report changed",
            ));
        }
        let recipe = ranked::physical_global_copy_ranked_recipe_v21(
            self.executable(),
            self.source_launch(),
            &formal,
            scope.budget,
        )?;
        scope.budget.charge_work(131_072)?;
        if &recipe != self.ranked.kernel()
            || inspection_text(&recipe)? != self.ranked_text
            || !self.ranked.all_mandatory_reports_are_clean()
            || self.ranked.has_retained_policy_checked_refinement_staging()
        {
            return Err(ProductionPhysicalGlobalCopyCheckErrorV21::Relation(
                "physical ranked graph/report relation changed",
            ));
        }
        let evidence = ProductionMiddleEndEvidenceV5::try_new(
            self.materialized.semantic_ssa().source_owner(),
            &self.ranked,
            &self.ranked_text,
        )
        .map_err(ProductionPhysicalGlobalCopyCheckErrorV21::Evidence)?;
        if evidence.canonical_bytes() != self.middle_end.canonical_bytes() {
            return Err(ProductionPhysicalGlobalCopyCheckErrorV21::Relation(
                "physical source/ranked evidence changed",
            ));
        }
        Ok(())
    }
    /// Real source-to-executable custody and correspondence, not imported bytes.
    pub const fn materialized(&self) -> &ProductionPhysicalGlobalCopyPreRankedKirOwnerV21 {
        &self.materialized
    }
    /// Original retained semantic SSA owner.
    pub fn semantic_ssa(&self) -> &ProductionSemanticSsaOwnerV1 {
        self.materialized.semantic_ssa()
    }
    /// The actual immutable executable with all authored setup and termination.
    pub fn executable(&self) -> &VerifiedCanonicalKernelIrModuleV21 {
        self.materialized.executable()
    }
    /// Retained actual source launch envelope, not a runtime launch observation.
    pub fn source_launch(&self) -> &crate::ProductionSourceLaunchRosterV1 {
        self.materialized.source_launch()
    }
    /// Combined input/output AND compiler-ABI obligations. Consuming output alone is
    /// not a complete physical-global-copy memory admission.
    pub const fn memory_obligations(&self) -> &PhysicalGlobalCopyMemoryObligationsV21 {
        &self.formal
    }
    /// Typed fixed-policy internal-analysis evidence, not functional certification.
    pub const fn middle_end_evidence(&self) -> &ProductionMiddleEndEvidenceV5 {
        &self.middle_end
    }
    /// Total logical retained payload, including consumed source/canonical receipt.
    pub const fn retained_storage(&self) -> usize {
        self.retained_storage
    }
    /// Existing independent session's retained analysis resource upper bound.
    pub fn ranked_analysis_retained_storage_upper_bound(&self) -> usize {
        self.ranked
            .production_analysis_retained_storage_upper_bound_v1()
    }
    /// No descriptor, artifact, protected publication or launch authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}
fn formal_v21(
    materialized: &ProductionPhysicalGlobalCopyPreRankedKirOwnerV21,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<
    (
        PhysicalGlobalCopyMemoryObligationsV21,
        PhysicalGlobalCopyMemoryStorageV21,
    ),
    PhysicalGlobalCopyAuxErrorV21,
> {
    let [kernel] = materialized.executable().module().kernels.as_slice() else {
        return Err(PhysicalGlobalCopyAuxErrorV21::Relation(
            "physical formal actual kernel roster",
        ));
    };
    let [root] = materialized.source_launch().roots() else {
        return Err(PhysicalGlobalCopyAuxErrorV21::Relation(
            "physical formal actual launch roster",
        ));
    };
    let output = derive_physical_global_copy_memory_obligations_v21(
        materialized.executable(),
        &kernel.id,
        ExplicitLaunchExtent::Exact {
            rank: 1,
            extents: root.layout().global_extents(),
        },
        FormalIndexWidth::Bits64,
        budget,
    )
    .map_err(PhysicalGlobalCopyAuxErrorV21::Formal)?;
    if output.0.global().allocations().len() != 2
        || output.0.global().accesses().len() != 2
        || output.0.global().bounds_requirements().len() != 2
        || output
            .0
            .global()
            .bounds_requirements()
            .iter()
            .any(|b| b.minimum_byte_len() != Some(512))
        || output.0.global().runtime_alias_requirements().len() != 1
        || !output.0.global().inter_invocation_conflicts().is_empty()
        || !ranked::required_conditions(&output.0)
    {
        return Err(PhysicalGlobalCopyAuxErrorV21::Relation(
            "physical complete memory census differs",
        ));
    }
    Ok(output)
}
fn inspection_text(
    recipe: &fe2o3_pliron::ProductionRankedKernelV1,
) -> Result<String, PhysicalGlobalCopyAuxErrorV21> {
    struct Text(String);
    impl fmt::Write for Text {
        fn write_str(&mut self, value: &str) -> fmt::Result {
            if self
                .0
                .len()
                .checked_add(value.len())
                .is_none_or(|n| n > TEXT_LIMIT)
            {
                return Err(fmt::Error);
            }
            self.0.push_str(value);
            Ok(())
        }
    }
    let mut value = String::new();
    value
        .try_reserve_exact(TEXT_LIMIT)
        .map_err(|_| ArgumentResourceV1::Allocation)?;
    if value.capacity() > TEXT_LIMIT {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    let mut text = Text(value);
    fmt::write(&mut text,format_args!(
        "physical-global-copy-v21-safety-projection conditional-full-input-prefix512-readable-initialized-output-writable-nonalias-kernarg-live-immutable {recipe:#?}\n"))
        .map_err(|_|PhysicalGlobalCopyAuxErrorV21::Relation("physical ranked inspection text bound"))?;
    Ok(text.0)
}

#[cfg(test)]
mod error_transport_tests {
    use super::*;
    #[test]
    fn global_copy_auxiliary_errors_move_without_boxing_or_erasing_diagnostics() {
        let public: ProductionPhysicalGlobalCopyCheckErrorV21 =
            PhysicalGlobalCopyAuxErrorV21::Relation("actual retained relation").into();
        assert!(matches!(
            public,
            ProductionPhysicalGlobalCopyCheckErrorV21::Relation("actual retained relation")
        ));
        let public: ProductionPhysicalGlobalCopyCheckErrorV21 =
            PhysicalGlobalCopyAuxErrorV21::Resource(ArgumentResourceV1::Accounting).into();
        assert!(matches!(
            public,
            ProductionPhysicalGlobalCopyCheckErrorV21::Resource(ArgumentResourceV1::Accounting)
        ));
        let public: ProductionPhysicalGlobalCopyCheckErrorV21 =
            PhysicalGlobalCopyAuxErrorV21::Formal(PhysicalGlobalCopyMemoryErrorV21::Profile(
                "actual read site",
            ))
            .into();
        assert!(matches!(
            public,
            ProductionPhysicalGlobalCopyCheckErrorV21::Formal(
                PhysicalGlobalCopyMemoryErrorV21::Profile("actual read site")
            )
        ));
    }
}
