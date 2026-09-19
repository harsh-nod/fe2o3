/// Failure of a descriptive source-to-pending-recipe query. No failure grants
/// partial production authority.
#[derive(Debug)]
pub enum ProductionConditionalSourceTranslationErrorV1 {
    /// The candidate borrows another owner or a detached recipe copy.
    ForeignRecipe,
    /// The root, selected body, symbol, or retained launch does not agree.
    SourceAssociation,
    /// Source rows are duplicate, incomplete, or point to the wrong operation kind.
    SourceRows,
    /// The frozen pending owner could not authenticate its retained recipe.
    Pending,
    /// Source policy, resource accounting, or the effect/value relation failed.
    Correspondence(ProductionSemanticKirErrorV1),
}

impl fmt::Display for ProductionConditionalSourceTranslationErrorV1 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForeignRecipe => out.write_str("candidate does not borrow this pending recipe"),
            Self::SourceAssociation => {
                out.write_str("pending recipe source or launch association changed")
            }
            Self::SourceRows => {
                out.write_str("pending recipe has invalid source correspondence rows")
            }
            Self::Pending => out.write_str("pending recipe custody could not be authenticated"),
            Self::Correspondence(error) => error.fmt(out),
        }
    }
}

impl std::error::Error for ProductionConditionalSourceTranslationErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Correspondence(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ProductionSemanticKirErrorV1> for ProductionConditionalSourceTranslationErrorV1 {
    fn from(error: ProductionSemanticKirErrorV1) -> Self {
        Self::Correspondence(error)
    }
}

impl From<ArgumentResourceV1> for ProductionConditionalSourceTranslationErrorV1 {
    fn from(error: ArgumentResourceV1) -> Self {
        Self::Correspondence(error.into())
    }
}

/// Descriptive effect/value correspondence retaining both actual owners.
///
/// Mandatory safety checks remain pending. This is not indexed-address
/// equivalence, complete operational equivalence, a clean lowering input,
/// refinement evidence, or artifact/launch authority.
/// Non-private memory attribution is complete; private accesses retain the
/// existing producer-specific omissions and checks, not an all-memory proof.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{ProductionConditionalSourceTranslationV1, ProductionPreRankedKirOwnerV1};
/// fn substitute<'a>(result: &mut ProductionConditionalSourceTranslationV1<'a>, owner: &'a ProductionPreRankedKirOwnerV1) {
///     result.source = owner;
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionConditionalSourceTranslationV1;
/// use fe2o3_pliron::ProductionRankedKernelLoweringInputV1;
/// fn promote(result: ProductionConditionalSourceTranslationV1<'_>) -> ProductionRankedKernelLoweringInputV1 {
///     result.into()
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionConditionalSourceTranslationV1;
/// fn escape<'a>(result: ProductionConditionalSourceTranslationV1<'a>) -> ProductionConditionalSourceTranslationV1<'static> {
///     result
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionConditionalSourceTranslationV1;
/// use fe2o3_pliron::ProductionConditionalRankedAnalysisV1;
/// fn substitute<'a>(result: &mut ProductionConditionalSourceTranslationV1<'a>, pending: &'a ProductionConditionalRankedAnalysisV1) {
///     result.pending = pending;
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{NativeRankedSourceCandidateV1, ProductionConditionalSourceTranslationV1, ProductionPreRankedKirOwnerV1};
/// use fe2o3_pliron::ProductionConditionalRankedAnalysisV1;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn escape_source<'a>(source: ProductionPreRankedKirOwnerV1, pending: &'a ProductionConditionalRankedAnalysisV1,
///     candidate: NativeRankedSourceCandidateV1<'_>, budget: &mut Budget<'_>) -> ProductionConditionalSourceTranslationV1<'a> {
///     source.check_conditional_source_translation_v1(pending, candidate, budget).unwrap()
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{NativeRankedSourceCandidateV1, ProductionConditionalSourceTranslationV1, ProductionPreRankedKirOwnerV1};
/// use fe2o3_pliron::ProductionConditionalRankedAnalysisV1;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn escape_pending<'a>(source: &'a ProductionPreRankedKirOwnerV1, pending: ProductionConditionalRankedAnalysisV1,
///     candidate: NativeRankedSourceCandidateV1<'_>, budget: &mut Budget<'_>) -> ProductionConditionalSourceTranslationV1<'a> {
///     source.check_conditional_source_translation_v1(&pending, candidate, budget).unwrap()
/// }
/// ```
pub struct ProductionConditionalSourceTranslationV1<'owner> {
    source: &'owner ProductionPreRankedKirOwnerV1,
    pending: &'owner fe2o3_pliron::ProductionConditionalRankedAnalysisV1,
    validation: ProductionMirPlironTranslationValidationV1,
}

impl fmt::Debug for ProductionConditionalSourceTranslationV1<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("ProductionConditionalSourceTranslationV1")
            .field("validation", &self.validation)
            .finish_non_exhaustive()
    }
}

impl<'owner> ProductionConditionalSourceTranslationV1<'owner> {
    /// The actual immutable source/SSA/canonical executable owner checked.
    pub const fn source(&self) -> &'owner ProductionPreRankedKirOwnerV1 {
        self.source
    }

    /// The actual pending graph owner, with every pipeline obligation unchanged.
    pub const fn pending(&self) -> &'owner fe2o3_pliron::ProductionConditionalRankedAnalysisV1 {
        self.pending
    }

    /// Matched represented memory effects, not a dynamic coverage proof.
    pub const fn memory_effects(&self) -> usize {
        self.validation.memory_effects
    }

    /// Independently reconstructed scalar write roots.
    pub const fn value_expressions(&self) -> usize {
        self.validation.value_expressions
    }
}

impl ProductionPreRankedKirOwnerV1 {
    /// Checks the existing effect/value relation before mandatory ranked checks
    /// are complete, without constructing a clean input or attachment receipt.
    ///
    /// Reserve `retained_analysis_storage_v1()` on the caller ledger first.
    /// Association traversal and native helper expansion use that same ledger;
    /// correlation retains its separate `max_operations * 64` work cap. Existing
    /// correlation/source-row allocations and recipe/arena storage are not
    /// accounted here. This is not complete allocation or RSS accounting.
    /// Diagnostic text is not interpreted as semantic evidence.
    pub fn check_conditional_source_translation_v1<'owner>(
        &'owner self,
        pending: &'owner fe2o3_pliron::ProductionConditionalRankedAnalysisV1,
        candidate: crate::NativeRankedSourceCandidateV1<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<
        ProductionConditionalSourceTranslationV1<'owner>,
        ProductionConditionalSourceTranslationErrorV1,
    > {
        use ProductionConditionalSourceTranslationErrorV1 as E;
        budget.charge_work(4)?;
        if budget.storage() < self.retained_analysis_storage_v1() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        self.require_legacy_helper_policy_v1("conditional source translation")?;
        let recipe = pending.kernel().map_err(|_| E::Pending)?;
        if !std::ptr::eq(recipe, candidate.kernel()) {
            return Err(E::ForeignRecipe);
        }
        let root = SemanticFunctionIdV1::from_index(candidate.semantic_root());
        let mut association = None;
        budget.charge_work(argument_product_v1(
            self.correspondence.lowered_functions().len(),
            3,
        )?)?;
        for row in self.correspondence.lowered_functions() {
            if row.role() == SemanticKirFunctionRoleV1::KernelEntry
                && row.correspondence_owner() == root
                && association.replace(row).is_some()
            {
                return Err(E::SourceAssociation);
            }
        }
        let association = association.ok_or(E::SourceAssociation)?;
        let semantic = self.semantic_ssa.source_semantic();
        charge_conditional_body_selection_v1(semantic, association, budget).map_err(|error| {
            match error {
                ProductionConditionalOutputBindingErrorV1::Correspondence(error) => {
                    E::Correspondence(error)
                }
                _ => E::SourceAssociation,
            }
        })?;
        let selection = semantic
            .select_kernel_body_for_root_v1(root)
            .ok_or(E::SourceAssociation)?;
        if selection.body() != association.semantic_function() {
            return Err(E::SourceAssociation);
        }
        let declaration = semantic
            .functions()
            .get(root.index() as usize)
            .ok_or(E::SourceAssociation)?;
        let symbol = declaration
            .kernel_entry()
            .ok_or(E::SourceAssociation)?
            .export_symbol()
            .as_bytes();
        budget.charge_work(argument_sum_v1(&[
            symbol.len(),
            recipe.function_name().len(),
        ])?)?;
        if symbol != recipe.function_name().as_bytes() {
            return Err(E::SourceAssociation);
        }
        let mut launch = None;
        budget.charge_work(argument_product_v1(self.source_launch.roots().len(), 3)?)?;
        for row in self.source_launch.roots() {
            if row.selected_root() == root && launch.replace(row).is_some() {
                return Err(E::SourceAssociation);
            }
        }
        let launch = launch.ok_or(E::SourceAssociation)?;
        let layout = launch.layout();
        let expected = ProductionRankedOperationV1::ExecutionLayout {
            grid_identity: layout.grid_identity(),
            global_extents: layout.global_extents(),
            workgroup_extents: layout.workgroup_extents(),
            subgroup_size: layout.subgroup_size(),
            full_physical_workgroups: layout.full_physical_workgroups(),
        };
        if launch.source_rank() != candidate.launch_rank()
            || launch.semantic_root_identity() != declaration.identity()
            || recipe
                .blocks()
                .first()
                .and_then(|block| block.operations().first())
                != Some(&expected)
        {
            return Err(E::SourceAssociation);
        }
        let module = self.executable.module();
        let mut kernel = None;
        for row in &module.kernels {
            budget.charge_work(argument_sum_v1(&[
                3,
                row.entry.as_str().len(),
                association.kernel_ir_function().as_str().len(),
            ])?)?;
            if &row.entry == association.kernel_ir_function() && kernel.replace(row).is_some() {
                return Err(E::SourceAssociation);
            }
        }
        let kernel = kernel.ok_or(E::SourceAssociation)?;
        budget.charge_work(argument_product_v1(
            argument_sum_v1(&[
                candidate.access_sources().len(),
                candidate.executable_effect_sources().len(),
            ])?,
            4,
        )?)?;
        if !ranked_access_sources_are_well_formed(recipe, candidate.access_sources())
            || !ranked_executable_effect_sources_are_well_formed(
                recipe,
                candidate.executable_effect_sources(),
            )
        {
            return Err(E::SourceRows);
        }
        let validation = validate_mir_pliron_recipe_translation_with_semantic_and_budget_v1(
            Some(semantic),
            module,
            &self.correspondence,
            kernel.id.as_str(),
            recipe,
            candidate.access_sources(),
            candidate.executable_effect_sources(),
            self.limits.max_operations,
            budget,
        )
        .map_err(ProductionSemanticKirErrorV1::MirPlironTranslation)?;
        Ok(ProductionConditionalSourceTranslationV1 {
            source: self,
            pending,
            validation,
        })
    }
}
