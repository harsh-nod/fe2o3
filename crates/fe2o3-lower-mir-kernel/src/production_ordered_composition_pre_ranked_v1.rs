// Additive same-source ordered composition custody. Ordinary source lowering
// remains the only producer; no raw Module or detached emission constructor.

/// A composition source, call correspondence or canonical-admission refusal.
#[derive(Debug)]
pub enum ProductionOrderedCompositionErrorV1 {
    /// Existing source lowering/correspondence or cumulative-resource refusal.
    Lowering(ProductionSemanticKirErrorV1),
    /// Exact V17 inverse/verification refusal.
    Canonical(fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV17),
}
impl fmt::Display for ProductionOrderedCompositionErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lowering(e) => e.fmt(f),
            Self::Canonical(e) => e.fmt(f),
        }
    }
}
impl Error for ProductionOrderedCompositionErrorV1 {}
impl From<ProductionSemanticKirErrorV1> for ProductionOrderedCompositionErrorV1 {
    fn from(value: ProductionSemanticKirErrorV1) -> Self {
        Self::Lowering(value)
    }
}
impl From<ArgumentResourceV1> for ProductionOrderedCompositionErrorV1 {
    fn from(value: ArgumentResourceV1) -> Self {
        Self::Lowering(value.into())
    }
}

fn ordered_composition_structural_error_v1(
    error: fe2o3_kernel_ir::OrderedProgramCompositionErrorV1,
) -> ProductionSemanticKirErrorV1 {
    match error {
        fe2o3_kernel_ir::OrderedProgramCompositionErrorV1::Resource(e) => e.into(),
        _ => ordered_composition_refusal_v1(
            "ordered composition canonical structural profile refused",
        ),
    }
}

/// Additional canonical, call-transport, composition-roster and owner storage.
/// Existing source SSA and ordinary lowering/correspondence allocations retain
/// their separate source limits. This is not total RSS or source authentication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrderedCompositionRetainedStorageV1 {
    bytes: usize,
}
impl OrderedCompositionRetainedStorageV1 {
    /// Reserve for the whole lifetime of the returned owner.
    pub const fn retained_storage(self) -> usize {
        self.bytes
    }
}

/// Move-only same-source pre-ranked custody for the finite MIR32/V17 composition.
/// No ranked/formal proof, optimizer permission, protected artifact or launch
/// authority is established. Backend authentication is a separate typed producer.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionOrderedCompositionPreRankedKirOwnerV1;
/// fn clone_owner(owner: ProductionOrderedCompositionPreRankedKirOwnerV1) { let _ = owner.clone(); }
/// ```
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionOrderedCompositionPreRankedKirOwnerV1;
/// fn mutate(owner: &mut ProductionOrderedCompositionPreRankedKirOwnerV1) {
///     owner.executable().module().functions.clear();
/// }
/// ```
#[derive(Debug)]
#[must_use = "source and call correspondence must remain attached to the canonical composition"]
pub struct ProductionOrderedCompositionPreRankedKirOwnerV1 {
    semantic_ssa: ProductionSemanticSsaOwnerV1,
    source_launch: crate::ProductionSourceLaunchRosterV1,
    composition: fe2o3_kernel_ir::VerifiedOrderedProgramCompositionV1,
    correspondence: SemanticKirCorrespondenceV1,
    sources: OrderedCompositionSourceRowsV1,
    limits: ProductionSemanticKirLimitsV1,
    retained: OrderedCompositionRetainedStorageV1,
}
struct DerivedOrderedCompositionV1 {
    composition: fe2o3_kernel_ir::VerifiedOrderedProgramCompositionV1,
    correspondence: SemanticKirCorrespondenceV1,
    sources: OrderedCompositionSourceRowsV1,
    retained: OrderedCompositionRetainedStorageV1,
}
impl ProductionOrderedCompositionPreRankedKirOwnerV1 {
    fn live_storage_floor_v1(&self) -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            self.retained.bytes,
            self.semantic_ssa
                .occurrence_storage()
                .ok_or(ArgumentResourceV1::Accounting)?
                .retained_storage(),
        ])
    }

    /// Uses actual normal semantic lowering, then exact V17 admission and
    /// structural composition verification. Restores the caller's storage floor
    /// on all Result exits, preserving cumulative work/peak/denials. Reserve the
    /// returned receipt while retaining this owner. No source authority is
    /// reconstructed from decoded bytes or from a supplied call roster.
    pub fn try_materialize_with_budget(
        semantic_ssa: ProductionSemanticSsaOwnerV1,
        source_launch: crate::ProductionSourceLaunchRosterV1,
        limits: ProductionSemanticKirLimitsV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionOrderedCompositionErrorV1> {
        let floor = budget.storage();
        let result = (|| {
            semantic_ssa
                .verify_replay()
                .map_err(ProductionSemanticKirErrorV1::SemanticSsa)?;
            let derived =
                derive_ordered_composition_v1(&semantic_ssa, &source_launch, limits, budget)?;
            let owner = Self {
                semantic_ssa,
                source_launch,
                composition: derived.composition,
                correspondence: derived.correspondence,
                sources: derived.sources,
                limits,
                retained: derived.retained,
            };
            owner.with_checked_canonical_calls_v17(budget, |_, _| Ok(()))?;
            Ok(owner)
        })();
        budget.release_storage(
            budget
                .storage()
                .checked_sub(floor)
                .ok_or(ArgumentResourceV1::Accounting)?,
        )?;
        result
    }
    /// Exact retained source SSA owner; identities alone cannot replace it.
    pub const fn semantic_ssa(&self) -> &ProductionSemanticSsaOwnerV1 {
        &self.semantic_ssa
    }
    /// Actual detached launch agreement retained from the frontend.
    pub const fn source_launch(&self) -> &crate::ProductionSourceLaunchRosterV1 {
        &self.source_launch
    }
    /// Structural owner of the unchanged complete V17 executable graph.
    pub const fn composition(&self) -> &fe2o3_kernel_ir::VerifiedOrderedProgramCompositionV1 {
        &self.composition
    }
    /// Exact canonical V17 owner, never a V12 projection.
    pub const fn executable(&self) -> &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV17 {
        self.composition.canonical()
    }
    /// Actual operation/call/result attribution retained by normal lowering.
    pub const fn correspondence(&self) -> &SemanticKirCorrespondenceV1 {
        &self.correspondence
    }
    /// Additional storage reservation, excluding legacy source-owned payload.
    pub const fn retained_storage(&self) -> OrderedCompositionRetainedStorageV1 {
        self.retained
    }
    /// Root-qualified source occurrences, bound to this canonical owner.
    pub fn occurrences(&self) -> impl Iterator<Item = &OrderedCompositionSourceOccurrenceV1> {
        self.sources.occurrences.iter().flatten()
    }
    /// Definition source rows, not cloned helper implementations.
    pub fn definitions(&self) -> impl Iterator<Item = &OrderedCompositionSourceDefinitionV1> {
        self.sources.definitions.iter().flatten()
    }
    /// This pre-ranked owner grants no artifact or launch authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
    /// Replays the actual immutable semantic owner, normal lowering, full
    /// canonical bytes, call/result correspondence and occurrence joins. A
    /// distinct equal-valued SSA substitution is not treated as equivalence.
    pub fn verify_equivalence(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionOrderedCompositionErrorV1> {
        if budget.storage() < self.live_storage_floor_v1()? {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        let floor = budget.storage();
        let result = (|| {
            self.semantic_ssa
                .verify_replay()
                .map_err(ProductionSemanticKirErrorV1::SemanticSsa)?;
            let replay = derive_ordered_composition_v1(
                &self.semantic_ssa,
                &self.source_launch,
                self.limits,
                budget,
            )?;
            budget.charge_work(argument_sum_v1(&[
                self.executable().canonical_bytes().len(),
                replay.composition.canonical().canonical_bytes().len(),
                ordered_composition_correspondence_work_v1(&self.correspondence)?,
                ordered_composition_correspondence_work_v1(&replay.correspondence)?,
                4096,
            ])?)?;
            if replay.composition != self.composition
                || replay.correspondence != self.correspondence
                || replay.sources != self.sources
                || replay.retained != self.retained
                || replay.composition.canonical().canonical_bytes()
                    != self.executable().canonical_bytes()
            {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch.into());
            }
            self.with_checked_canonical_calls_v17(budget, |_, _| Ok(()))?;
            Ok(())
        })();
        budget.release_storage(
            budget
                .storage()
                .checked_sub(floor)
                .ok_or(ArgumentResourceV1::Accounting)?,
        )?;
        result
    }
}

fn ordered_composition_correspondence_work_v1(
    rows: &SemanticKirCorrespondenceV1,
) -> Result<usize, ArgumentResourceV1> {
    let mut work = argument_sum_v1(&[
        std::mem::size_of_val(rows),
        std::mem::size_of_val(rows.lowered_functions.as_ref()),
        std::mem::size_of_val(rows.blocks.as_ref()),
        std::mem::size_of_val(rows.statement_operation_spans.as_ref()),
        std::mem::size_of_val(rows.terminator_operation_spans.as_ref()),
        std::mem::size_of_val(rows.generated_terminator_values.as_ref()),
        std::mem::size_of_val(rows.call_returns.as_ref()),
        std::mem::size_of_val(rows.call_result_components.as_ref()),
        std::mem::size_of_val(rows.synthetic_operation_spans.as_ref()),
        std::mem::size_of_val(rows.parameter_bindings.as_ref()),
        std::mem::size_of_val(rows.parameter_component_bindings.as_ref()),
        std::mem::size_of_val(rows.ignored_parameter_bindings.as_ref()),
    ])?;
    for row in &rows.lowered_functions {
        work = argument_sum_v1(&[work, row.kernel_ir_function.as_str().len()])?;
    }
    for row in &rows.parameter_component_bindings {
        work = argument_sum_v1(&[work, std::mem::size_of_val(row.projection.as_ref())])?;
    }
    Ok(work)
}

fn derive_ordered_composition_v1(
    semantic_ssa: &ProductionSemanticSsaOwnerV1,
    source_launch: &crate::ProductionSourceLaunchRosterV1,
    limits: ProductionSemanticKirLimitsV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<DerivedOrderedCompositionV1, ProductionOrderedCompositionErrorV1> {
    let permit = validate_ordered_composition_context_v1(
        semantic_ssa.source_semantic(),
        source_launch,
        limits,
        budget,
    )?;
    if semantic_ssa.occurrences_v1().is_none() {
        return Err(ordered_composition_refusal_v1(
            "ordered composition requires actual source SSA occurrence capture",
        )
        .into());
    }
    if budget.storage()
        < semantic_ssa
            .occurrence_storage()
            .ok_or(ArgumentResourceV1::Accounting)?
            .retained_storage()
    {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    let owner_bytes = std::mem::size_of::<ProductionOrderedCompositionPreRankedKirOwnerV1>();
    budget.reserve_storage(owner_bytes)?;
    let root_bytes = std::mem::size_of::<RetainedRankedLaunchRootV1>();
    budget.charge_work(1)?;
    budget.reserve_storage(argument_product_v1(root_bytes, 2)?)?;
    let roots = materialization_launch_roots_v1(semantic_ssa, source_launch)?;
    if roots.len() != 1 || std::mem::size_of_val(roots.as_ref()) != root_bytes {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    budget.release_storage(root_bytes)?;
    let (module, correspondence) = lower_module_with_call_budget_for_helper_admission_v1(
        semantic_ssa,
        limits,
        Some(&roots),
        None,
        budget,
        &mut HelperLoweringAdmissionV1::PendingOrderedComposition(permit),
    )?;
    drop(roots);
    budget.release_storage(root_bytes)?;
    if correspondence.private_arrays.active {
        return Err(ordered_composition_refusal_v1(
            "ordered composition has private-array effects",
        )
        .into());
    }
    let call_bytes = CallReturnBufferV1::bytes(
        correspondence.call_returns.len(),
        correspondence.call_result_components.len(),
    )?;
    let (canonical, canonical_storage) = fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV17::
        from_module_ref_with_verification_budget_v17(&module, budget)
        .map_err(ProductionOrderedCompositionErrorV1::Canonical)?;
    drop(module);
    budget.reserve_storage(canonical_storage.retained_storage())?;
    let (composition, composition_storage) =
        fe2o3_kernel_ir::VerifiedOrderedProgramCompositionV1::try_from_canonical_v17(
            canonical, budget,
        )
        .map_err(ordered_composition_structural_error_v1)?;
    budget.reserve_storage(composition_storage.retained_storage())?;
    let sources =
        bind_ordered_composition_sources_v1(semantic_ssa, &composition, &correspondence, budget)?;
    let retained = OrderedCompositionRetainedStorageV1 {
        bytes: argument_sum_v1(&[
            owner_bytes,
            call_bytes,
            canonical_storage.retained_storage(),
            composition_storage.retained_storage(),
        ])?,
    };
    Ok(DerivedOrderedCompositionV1 {
        composition,
        correspondence,
        sources,
        retained,
    })
}

include!("production_ordered_composition_context_v1.rs");
include!("production_ordered_composition_calls_v17.rs");
include!("production_ordered_composition_sources_v1.rs");
include!("production_ordered_composition_inspection_v1.rs");
