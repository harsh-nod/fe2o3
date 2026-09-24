//! Genuine fixed-policy ranked/formal continuation of source-owned KIR20.
//! The analysis is conditional on the retained compiler-ABI memory obligations;
//! it does not discharge runtime kernarg immutability or output disjointness.
use super::*;
use fe2o3_kernel_ir::{
    ExplicitLaunchExtent, FormalIndexWidth, PhysicalEntryMemoryErrorV20,
    PhysicalEntryMemoryObligationsV20, PhysicalEntryMemoryStorageV20,
    VerifiedCanonicalKernelIrModuleV20, derive_physical_entry_memory_obligations_v20,
};
use fe2o3_pliron::{
    ProductionConstructionV1, ProductionMiddleEndEvidenceV5, ProductionRankedKernelLoweringInputV1,
    ProductionSessionLimitsV1, compile_ranked_kernel_for_gfx942_lowering_v1,
};
#[path = "production_physical_entry_ranked_v20.rs"]
mod ranked;
const TEXT_LIMIT: usize = 512 * 1024;
const EVIDENCE_LIMIT: usize = fe2o3_pliron::MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V5;

/// Complete source, physical-memory, resource or mandatory-analysis failure.
#[derive(Debug)]
pub enum ProductionPhysicalEntryCheckErrorV20 {
    /// Original retained source/executable correspondence was refused.
    Source(ProductionPhysicalEntrySourceErrorV20),
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
    /// Exact combined output and compiler-ABI memory derivation failed.
    Formal(PhysicalEntryMemoryErrorV20),
}
impl From<ArgumentResourceV1> for ProductionPhysicalEntryCheckErrorV20 {
    fn from(error: ArgumentResourceV1) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for ProductionPhysicalEntryCheckErrorV20 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "physical-entry checked continuation: {self:?}")
    }
}
impl Error for ProductionPhysicalEntryCheckErrorV20 {}
// Helpers do not carry the large report-bearing session error. The exhaustive
// move conversion retains all typed diagnostic data, with no allocating box.
#[derive(Debug)]
enum PhysicalEntryAuxErrorV20 {
    Resource(ArgumentResourceV1),
    Relation(&'static str),
    RankedRecipe(fe2o3_pliron::ProductionRankedKernelErrorV1),
    Formal(PhysicalEntryMemoryErrorV20),
}
impl From<ArgumentResourceV1> for PhysicalEntryAuxErrorV20 {
    fn from(error: ArgumentResourceV1) -> Self {
        Self::Resource(error)
    }
}
impl From<PhysicalEntryAuxErrorV20> for ProductionPhysicalEntryCheckErrorV20 {
    fn from(error: PhysicalEntryAuxErrorV20) -> Self {
        match error {
            PhysicalEntryAuxErrorV20::Resource(e) => Self::Resource(e),
            PhysicalEntryAuxErrorV20::Relation(e) => Self::Relation(e),
            PhysicalEntryAuxErrorV20::RankedRecipe(e) => Self::RankedRecipe(e),
            PhysicalEntryAuxErrorV20::Formal(e) => Self::Formal(e),
        }
    }
}
const _: () = assert!(std::mem::size_of::<PhysicalEntryAuxErrorV20>() < 128);

/// Move-only source, canonical graph, mandatory ranked reports and memory custody.
///
/// This owner consumes the real source preowner; it cannot be made from model
/// bytes, a caller-selected recipe, detached evidence or a descriptor. The
/// immutable authored body remains the executable; the ranked graph is only
/// its safety-analysis projection. The fixed existing eight-pass production
/// session runs normally and its complete typed reports remain retained.
///
/// The compiler-ABI allocation is not a fabricated third/extra source argument.
/// Its read views rely on explicit retained immutability/output-disjoint
/// preconditions. Those remain unresolved here and must be consumed by the
/// normal compiler descriptor/runtime continuation. Clean conditional reports
/// are not proof that any real pointer allocation or launch satisfies them.
///
/// Added logical owner/recipe/text/formal/evidence retention and exact replay
/// use the original cumulative canonical ledger. The established semantic SSA
/// and ranked PLIRON session keep their independently bounded domains; the
/// receipt is not total compiler allocation/RSS or runtime authority.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionPhysicalEntryCheckedKirOwnerV20;
/// fn copy<T: Clone>() {}
/// copy::<ProductionPhysicalEntryCheckedKirOwnerV20>();
/// ```
#[must_use]
pub struct ProductionPhysicalEntryCheckedKirOwnerV20 {
    materialized: ProductionPhysicalEntryPreRankedKirOwnerV20,
    ranked: ProductionRankedKernelLoweringInputV1,
    middle_end: ProductionMiddleEndEvidenceV5,
    ranked_text: String,
    formal: PhysicalEntryMemoryObligationsV20,
    retained_storage: usize,
}
impl ProductionPhysicalEntryCheckedKirOwnerV20 {
    /// Runs fixed mandatory checks while consuming the retained actual source.
    ///
    /// Input storage must already be reserved. Work remains cumulative; incoming
    /// storage is restored on success/error/unwind. On success reserve the
    /// returned total receipt minus the consumed preowner receipt while held.
    pub fn try_check(
        materialized: ProductionPhysicalEntryPreRankedKirOwnerV20,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionPhysicalEntryCheckErrorV20> {
        let input_storage = materialized.retained_storage();
        if budget.storage() < input_storage {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        materialized
            .verify_equivalence(budget)
            .map_err(ProductionPhysicalEntryCheckErrorV20::Source)?;
        let mut scope = PhysicalEntryLedgerScopeV20::new(budget);
        let fixed = argument_sum_v1(&[std::mem::size_of::<Self>(), TEXT_LIMIT, EVIDENCE_LIMIT])?;
        scope.reserve(fixed)?;
        let (formal, formal_storage) = formal_v20(&materialized, scope.budget)?;
        scope.reserve(formal_storage.retained_storage())?;
        let recipe = ranked::physical_entry_ranked_recipe_v20(
            materialized.executable(),
            materialized.source_launch(),
            &formal,
            scope.budget,
        )?;
        let ranked_text = inspection_text(&recipe)?;
        let construction = ProductionConstructionV1::ranked_kernel("physical_entry_v20", recipe)
            .map_err(|_| {
                ProductionPhysicalEntryCheckErrorV20::Relation("physical ranked construction name")
            })?;
        let ranked = compile_ranked_kernel_for_gfx942_lowering_v1(
            construction,
            ProductionSessionLimitsV1::default(),
            std::iter::empty(),
        )
        .map_err(ProductionPhysicalEntryCheckErrorV20::Ranked)?;
        if !ranked.all_mandatory_reports_are_clean()
            || ranked.has_retained_policy_checked_refinement_staging()
        {
            return Err(ProductionPhysicalEntryCheckErrorV20::Relation(
                "physical ranked mandatory policy differs",
            ));
        }
        let middle_end = ProductionMiddleEndEvidenceV5::try_new(
            materialized.semantic_ssa().source_owner(),
            &ranked,
            &ranked_text,
        )
        .map_err(ProductionPhysicalEntryCheckErrorV20::Evidence)?;
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
    ) -> Result<(), ProductionPhysicalEntryCheckErrorV20> {
        if budget.storage() < self.retained_storage {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        self.materialized
            .verify_equivalence(budget)
            .map_err(ProductionPhysicalEntryCheckErrorV20::Source)?;
        let mut scope = PhysicalEntryLedgerScopeV20::new(budget);
        scope.reserve(argument_sum_v1(&[TEXT_LIMIT, EVIDENCE_LIMIT])?)?;
        let (formal, formal_storage) = formal_v20(&self.materialized, scope.budget)?;
        scope.reserve(formal_storage.retained_storage())?;
        if formal != self.formal {
            return Err(ProductionPhysicalEntryCheckErrorV20::Relation(
                "physical complete memory report changed",
            ));
        }
        let recipe = ranked::physical_entry_ranked_recipe_v20(
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
            return Err(ProductionPhysicalEntryCheckErrorV20::Relation(
                "physical ranked graph/report relation changed",
            ));
        }
        let evidence = ProductionMiddleEndEvidenceV5::try_new(
            self.materialized.semantic_ssa().source_owner(),
            &self.ranked,
            &self.ranked_text,
        )
        .map_err(ProductionPhysicalEntryCheckErrorV20::Evidence)?;
        if evidence.canonical_bytes() != self.middle_end.canonical_bytes() {
            return Err(ProductionPhysicalEntryCheckErrorV20::Relation(
                "physical source/ranked evidence changed",
            ));
        }
        Ok(())
    }
    /// Real source-to-executable custody and correspondence, not imported bytes.
    pub const fn materialized(&self) -> &ProductionPhysicalEntryPreRankedKirOwnerV20 {
        &self.materialized
    }
    /// Original retained semantic SSA owner.
    pub fn semantic_ssa(&self) -> &ProductionSemanticSsaOwnerV1 {
        self.materialized.semantic_ssa()
    }
    /// The actual immutable executable with all authored setup and termination.
    pub fn executable(&self) -> &VerifiedCanonicalKernelIrModuleV20 {
        self.materialized.executable()
    }
    /// Retained actual source launch envelope, not a runtime launch observation.
    pub fn source_launch(&self) -> &crate::ProductionSourceLaunchRosterV1 {
        self.materialized.source_launch()
    }
    /// Combined output AND compiler-ABI obligations. Consuming output alone is
    /// not a complete physical-entry memory admission.
    pub const fn memory_obligations(&self) -> &PhysicalEntryMemoryObligationsV20 {
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
fn formal_v20(
    materialized: &ProductionPhysicalEntryPreRankedKirOwnerV20,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<
    (
        PhysicalEntryMemoryObligationsV20,
        PhysicalEntryMemoryStorageV20,
    ),
    PhysicalEntryAuxErrorV20,
> {
    let [kernel] = materialized.executable().module().kernels.as_slice() else {
        return Err(PhysicalEntryAuxErrorV20::Relation(
            "physical formal actual kernel roster",
        ));
    };
    let [root] = materialized.source_launch().roots() else {
        return Err(PhysicalEntryAuxErrorV20::Relation(
            "physical formal actual launch roster",
        ));
    };
    let output = derive_physical_entry_memory_obligations_v20(
        materialized.executable(),
        &kernel.id,
        ExplicitLaunchExtent::Exact {
            rank: 1,
            extents: root.layout().global_extents(),
        },
        FormalIndexWidth::Bits64,
        budget,
    )
    .map_err(PhysicalEntryAuxErrorV20::Formal)?;
    if output.0.output().allocations().len() != 1
        || output.0.output().accesses().len() != 1
        || output.0.output().bounds_requirements().len() != 1
        || output.0.output().bounds_requirements()[0].minimum_byte_len() != Some(512)
        || !output.0.output().runtime_alias_requirements().is_empty()
        || !output.0.output().inter_invocation_conflicts().is_empty()
        || output.0.kernarg_reads().is_empty()
    {
        return Err(PhysicalEntryAuxErrorV20::Relation(
            "physical complete memory census differs",
        ));
    }
    Ok(output)
}
fn inspection_text(
    recipe: &fe2o3_pliron::ProductionRankedKernelV1,
) -> Result<String, PhysicalEntryAuxErrorV20> {
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
        "physical-entry-v20-safety-projection conditional-kernarg-immutable-and-output-disjoint {recipe:#?}\n"))
        .map_err(|_|PhysicalEntryAuxErrorV20::Relation("physical ranked inspection text bound"))?;
    Ok(text.0)
}
