//! Distinct optimized physical endpoint over the existing original-source facts.
use super::*;
use canonical_assertion_facts_v1::{
    ProjectedAssertionConditionV1, ProjectedSourceSiteControlV18, checked_slice_projection_v18,
};
use canonical_source_facts_v18::{
    SourceAssertionFactsV18, SourceCallObligationV18, retain_resource_error, retained_resource,
    source_error,
};
use fe2o3_kernel_analysis::{
    CanonicalKirCallEffectsV18, CanonicalKirSparseV18, CanonicalKirSparseValueV1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_lower_mir_kernel::{
    ProductionOptimizedSourceBlockControlV18, ProductionOptimizedSourceCorrespondenceV18,
    ProductionOptimizedSourceSiteControlV18, ProductionSliceAccessSiteV1,
    ProductionSourceCorrespondenceV18, ProductionSourceOwnedViewErrorV18,
    SemanticKirOptimizedAssertOutcomeV1,
};

fn resource(error: Resource) -> ProductionRankedProjectionErrorV1 {
    source_error(error.into())
}
fn binding(detail: &'static str) -> ProductionRankedProjectionErrorV1 {
    source_error(ProductionSourceOwnedViewErrorV18::Binding(detail))
}

pub(super) struct OptimizedSourceFactsV18<'a, 'r, 'i, 'g, 'b, 'w> {
    original: &'a mut SourceAssertionFactsV18<'r, 'i, 'g, 'b, 'w>,
    optimized: &'a ProductionOptimizedSourceCorrespondenceV18<'g>,
    sparse: &'a CanonicalKirSparseV18<'i, 'g>,
    effects: &'a CanonicalKirCallEffectsV18<'i, 'g>,
    block: Option<(
        SemanticBlockIdV1,
        ProductionOptimizedSourceBlockControlV18<'a, 'g>,
    )>,
}

fn optimized_facts_headers_v18<T>() -> Result<usize, ProductionRankedProjectionErrorV1> {
    let mut bytes = 0usize;
    for header in [
        std::mem::size_of::<OptimizedSourceFactsV18<'_, '_, '_, '_, '_, '_>>(),
        std::mem::size_of::<SourceAssertionFactsV18<'_, '_, '_, '_, '_>>(),
        std::mem::size_of::<
            Result<
                ProductionOptimizedSourceBlockControlV18<'_, '_>,
                ProductionSourceOwnedViewErrorV18,
            >,
        >(),
        std::mem::size_of::<Result<T, ProductionRankedProjectionErrorV1>>(),
        std::mem::size_of::<std::thread::Result<Result<T, ProductionRankedProjectionErrorV1>>>(),
        std::mem::size_of::<std::panic::AssertUnwindSafe<T>>(),
    ] {
        bytes = bytes
            .checked_add(header)
            .ok_or_else(|| resource(Resource::Arithmetic))?;
    }
    Ok(bytes)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn with_optimized_source_projection_facts_v18<'g, 'i, 'work, T>(
    relation: &ProductionSourceCorrespondenceV18<'g>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'g>,
    input_sparse: &CanonicalKirSparseV18<'i, 'g>,
    output_sparse: &CanonicalKirSparseV18<'i, 'g>,
    input_effects: &CanonicalKirCallEffectsV18<'i, 'g>,
    output_effects: &CanonicalKirCallEffectsV18<'i, 'g>,
    cleanup: &fe2o3_pliron::CanonicalAnalysisCleanupV1<'_>,
    root: usize,
    selection: SemanticKernelBodySelectionV1,
    pending: &mut [Option<SourceCallObligationV18>],
    budget: &mut Budget<'work>,
    consume: impl FnOnce(
        &mut OptimizedSourceFactsV18<'_, '_, 'i, 'g, '_, 'work>,
    ) -> Result<T, ProductionRankedProjectionErrorV1>,
) -> Result<T, ProductionRankedProjectionErrorV1> {
    with_optimized_source_projection_scope_v18(relation, optimized, input_sparse,
        output_sparse, input_effects, output_effects, cleanup, root,
        canonical_source_facts_v18::SourceProjectionScopeV18::SelectedBody(selection),
        pending, budget, consume)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn with_optimized_source_projection_scope_v18<'g, 'i, 'work, T>(
    relation: &ProductionSourceCorrespondenceV18<'g>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'g>,
    input_sparse: &CanonicalKirSparseV18<'i, 'g>,
    output_sparse: &CanonicalKirSparseV18<'i, 'g>,
    input_effects: &CanonicalKirCallEffectsV18<'i, 'g>,
    output_effects: &CanonicalKirCallEffectsV18<'i, 'g>,
    cleanup: &fe2o3_pliron::CanonicalAnalysisCleanupV1<'_>,
    root: usize,
    scope: canonical_source_facts_v18::SourceProjectionScopeV18<'_, 'g>,
    pending: &mut [Option<SourceCallObligationV18>],
    budget: &mut Budget<'work>,
    consume: impl FnOnce(&mut OptimizedSourceFactsV18<'_, '_, 'i, 'g, '_, 'work>)
        -> Result<T, ProductionRankedProjectionErrorV1>,
) -> Result<T, ProductionRankedProjectionErrorV1> {
    relation.check_query_v18(budget).map_err(source_error)?;
    // Both nominal facts adapters coexist; the original table's own header and
    // backing remain charged by its unchanged scoped constructor.
    let header = optimized_facts_headers_v18::<T>()
        .map_err(|error| retain_resource_error(relation, error))?;
    budget
        .reserve_storage(header)
        .map_err(|error| retained_resource(relation, error))?;
    let result = canonical_source_facts_v18::with_source_projection_scope_v18(
        relation,
        input_sparse,
        input_effects,
        cleanup,
        root,
        scope,
        pending,
        budget,
        |original| {
            let mut facts = OptimizedSourceFactsV18 {
                original,
                optimized,
                sparse: output_sparse,
                effects: output_effects,
                block: None,
            };
            facts.check()?;
            let result = consume(&mut facts);
            let postflight = if result.is_ok() {
                facts.check()
            } else {
                facts.original.observe_custody()
            };
            match result {
                Err(error) => Err(error),
                Ok(value) => match postflight {
                    Ok(()) => Ok(value),
                    Err(error) => {
                        drop(value);
                        Err(error)
                    }
                },
            }
        },
    );
    match result {
        Ok(value) => {
            budget
                .release_storage(header)
                .map_err(|error| retained_resource(relation, error))?;
            Ok(value)
        }
        Err(error) => Err(error),
    }
}

impl<'w> OptimizedSourceFactsV18<'_, '_, '_, '_, '_, 'w> {
    pub(super) fn check_original_analysis_input_v18(
        &mut self,
        semantic: &AdmittedInertSemanticMirV1,
        function: SemanticFunctionIdV1,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        self.check()?;
        let source = self
            .original
            .relation
            .source(self.original.budget)
            .map_err(source_error)?;
        let actual = source
            .source_semantic(self.original.budget)
            .map_err(source_error)?;
        self.original
            .budget
            .charge_work(2)
            .map_err(|error| retained_resource(self.original.relation, error))?;
        if !std::ptr::eq(actual, semantic) || function != self.original.semantic_function {
            return Err(binding(
                "backend analysis substituted its original admitted source or invocation function",
            ));
        }
        Ok(())
    }

    pub(super) fn check_constant_function_v18(
        &mut self,
        function: &SemanticFunctionDeclV1,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        self.check()?;
        let source = self
            .original
            .relation
            .source(self.original.budget)
            .map_err(source_error)?;
        let semantic = source
            .source_semantic(self.original.budget)
            .map_err(source_error)?;
        self.original
            .budget
            .charge_work(1)
            .map_err(|error| retained_resource(self.original.relation, error))?;
        if semantic
            .functions()
            .get(self.original.semantic_function.index() as usize)
            .is_none_or(|actual| !std::ptr::eq(actual, function))
        {
            return Err(binding(
                "constant preparation substituted its original source function",
            ));
        }
        Ok(())
    }

    pub(super) fn constant_budget_v18(&mut self) -> &mut Budget<'w> {
        self.original.budget
    }

    pub(super) fn retain_resource_error_v18(
        &self,
        error: ProductionRankedProjectionErrorV1,
    ) -> ProductionRankedProjectionErrorV1 {
        retain_resource_error(self.original.relation, error)
    }

    pub(super) fn constant_custody_v18(
        &self,
        slot: usize,
        ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
        floor: usize,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        optimized_source_projection_v18::observe_constant_custody_v18(
            self.original.budget,
            self.original.cleanup,
            slot,
            ledger,
            floor,
        )
    }

    pub(super) fn deny_constant_refund_v18(&self) {
        self.original.cleanup.deny_refund();
    }

    fn check(&mut self) -> Result<(), ProductionRankedProjectionErrorV1> {
        // The original adapter still checks the actual original function,
        // source table, account, inventory and original analysis pointers.
        self.original.charge_private_array_work(0)?;
        let input = self
            .optimized
            .input_inventory(self.original.budget)
            .map_err(source_error)?;
        let output = self
            .optimized
            .output_inventory(self.original.budget)
            .map_err(source_error)?;
        let source = self
            .optimized
            .original_source(self.original.budget)
            .map_err(source_error)?;
        self.original
            .budget
            .charge_work(4)
            .map_err(|error| retained_resource(self.original.relation, error))?;
        if !std::ptr::eq(
            input,
            self.original
                .relation
                .inventory(self.original.budget)
                .map_err(source_error)?,
        ) || !std::ptr::eq(
            source,
            self.original
                .relation
                .source(self.original.budget)
                .map_err(source_error)?,
        ) || !self.sparse.belongs_to(output)
            || !self.effects.belongs_to(output)
        {
            return Err(binding(
                "optimized projection substituted source or output reports",
            ));
        }
        Ok(())
    }

    fn select_block(
        &mut self,
        block: SemanticBlockIdV1,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        self.check()?;
        if self
            .block
            .as_ref()
            .is_none_or(|(current, _)| *current != block)
        {
            let control = self
                .optimized
                .source_block_control(
                    self.original.root,
                    self.original.instance,
                    block,
                    self.original.budget,
                )
                .map_err(source_error)?;
            self.block = Some((block, control));
        }
        Ok(())
    }

    fn block_disposition(
        &mut self,
        block: SemanticBlockIdV1,
    ) -> Result<ProductionOptimizedSourceSiteControlV18, ProductionRankedProjectionErrorV1> {
        self.select_block(block)?;
        self.block
            .as_ref()
            .ok_or_else(|| binding("missing optimized block census"))?
            .1
            .disposition(self.original.budget)
            .map_err(source_error)
    }

    fn site_disposition(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
    ) -> Result<ProductionOptimizedSourceSiteControlV18, ProductionRankedProjectionErrorV1> {
        self.select_block(block)?;
        self.block
            .as_ref()
            .ok_or_else(|| binding("missing optimized block census"))?
            .1
            .site(statement, self.original.budget)
            .map_err(source_error)
    }
}

impl ProjectedAssertionFactsV1 for OptimizedSourceFactsV18<'_, '_, '_, '_, '_, '_> {
    fn check_projection_source_v18(
        &mut self,
        semantic: &AdmittedInertSemanticMirV1,
        function: SemanticFunctionIdV1,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        self.check_original_analysis_input_v18(semantic, function)
    }

    fn projection_meter_v18(
        &mut self,
    ) -> Option<
        &mut dyn fe2o3_mir_model::SemanticAssertionMeterV1<
            Error = ProductionRankedProjectionErrorV1,
        >,
    > {
        Some(self)
    }

    fn finish_ranked_projection_v18(
        &mut self,
        name: &str,
        arguments: usize,
        blocks: Vec<ProductionRankedBlockV1>,
    ) -> Result<ProductionRankedKernelV1, ProductionRankedProjectionErrorV1> {
        self.check()?;
        ProductionRankedKernelV1::from_exact_projection_v18(
            name,
            arguments,
            blocks,
            self.original.budget,
        )
        .map_err(|error| match error {
            fe2o3_pliron::ProductionRankedProjectionValidationErrorV18::Kernel(error) => {
                ProductionRankedProjectionErrorV1::Recipe(error)
            }
            fe2o3_pliron::ProductionRankedProjectionValidationErrorV18::Resource(error) => {
                retained_resource(self.original.relation, error)
            }
        })
    }

    fn source_site_control_v18(
        &mut self,
        block: usize,
        statement: Option<usize>,
    ) -> Result<Option<ProjectedSourceSiteControlV18>, ProductionRankedProjectionErrorV1> {
        let block = SemanticBlockIdV1::from_index(
            u32::try_from(block)
                .map_err(|_| retained_resource(self.original.relation, Resource::Arithmetic))?,
        );
        let statement = statement
            .map(u32::try_from)
            .transpose()
            .map_err(|_| retained_resource(self.original.relation, Resource::Arithmetic))?;
        Ok(Some(match self.site_disposition(block, statement)? {
            ProductionOptimizedSourceSiteControlV18::Retained => {
                ProjectedSourceSiteControlV18::Retained
            }
            ProductionOptimizedSourceSiteControlV18::RemovedUnreachable => {
                ProjectedSourceSiteControlV18::RemovedUnreachable
            }
            ProductionOptimizedSourceSiteControlV18::Mixed => ProjectedSourceSiteControlV18::Mixed,
            ProductionOptimizedSourceSiteControlV18::OriginalUnmaterialized => {
                ProjectedSourceSiteControlV18::OriginalUnmaterialized
            }
        }))
    }

    fn masked_assertion_source_proved_v1(
        &mut self,
        function: &SemanticFunctionDeclV1,
        block: usize,
        expected: bool,
        successor: SemanticBlockIdV1,
    ) -> Result<bool, ProductionRankedProjectionErrorV1> {
        self.check()?;
        let proved = self
            .original
            .masked_assertion_source_proved_v1(function, block, expected, successor)?;
        if proved
            && matches!(self.condition(block, expected, successor)?,
            ProjectedAssertionConditionV1::Bool(value) if value != expected)
        {
            return Err(binding(
                "optimized assertion selected failure after source proof",
            ));
        }
        Ok(proved)
    }

    fn call_projection_disposition_v18<'call>(
        &mut self,
        block: usize,
        call: &'call SemanticDirectCallV1,
        source: SemanticSourceProvenanceV1,
    ) -> Result<CallProjectionDispositionV18<'call>, ProductionRankedProjectionErrorV1> {
        let block_id = SemanticBlockIdV1::from_index(
            u32::try_from(block)
                .map_err(|_| retained_resource(self.original.relation, Resource::Arithmetic))?,
        );
        if self.site_disposition(block_id, None)?
            != ProductionOptimizedSourceSiteControlV18::Retained
        {
            return Err(binding(
                "unreachable optimized call cannot become a pending or empty call",
            ));
        }
        let disposition = self
            .original
            .call_projection_disposition_v18(block, call, source)?;
        if let CallProjectionDispositionV18::PendingStorage(pending) = &disposition {
            let decision = self
                .optimized
                .instance_effects(
                    self.original.root,
                    pending.callee_instance,
                    self.original.effects,
                    self.effects,
                    self.original.budget,
                )
                .map_err(source_error)?;
            if decision.output
                == fe2o3_kernel_analysis::CanonicalKirCallEffectDecisionV1::Incomplete
            {
                return Err(binding("optimized callee effect summary is incomplete"));
            }
        }
        Ok(disposition)
    }

    fn accept_pending_source_call_v18(
        &mut self,
        pending: PendingSourceCallV18<'_>,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        if self.site_disposition(pending.source_block, None)?
            != ProductionOptimizedSourceSiteControlV18::Retained
        {
            return Err(binding("optimized pending call became unreachable"));
        }
        self.original.accept_pending_source_call_v18(pending)
    }

    fn charge_private_array_work(
        &mut self,
        amount: usize,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        self.check()?;
        self.original.charge_private_array_work(amount)
    }
    fn helper_value_ledger_v1(
        &self,
    ) -> Result<
        (
            usize,
            fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
        ),
        ProductionRankedProjectionErrorV1,
    > {
        self.original.helper_value_ledger_v1()
    }
    fn scalar_private_storage_v1(&self) -> Result<usize, ProductionRankedProjectionErrorV1> {
        self.original.scalar_private_storage_v1()
    }
    fn reserve_scalar_private_storage_v1(
        &mut self,
        amount: usize,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        self.check()?;
        self.original.reserve_scalar_private_storage_v1(amount)
    }
    fn release_scalar_private_storage_v1(
        &mut self,
        amount: usize,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        self.check()?;
        self.original.release_scalar_private_storage_v1(amount)
    }
    fn private_array_initializer_count(
        &mut self,
        block: usize,
        statement: usize,
    ) -> Result<Option<u64>, ProductionRankedProjectionErrorV1> {
        self.check()?;
        self.original
            .private_array_initializer_count(block, statement)
    }
    fn slice_access(
        &mut self,
        site: ProjectedSemanticAccessSiteV1,
        ordinal: u32,
        assertion: u32,
    ) -> Result<slice_projection_v1::ProjectedSliceInputV1, ProductionRankedProjectionErrorV1> {
        self.check()?;
        let block = u32::try_from(site.block)
            .map_err(|_| retained_resource(self.original.relation, Resource::Arithmetic))?;
        let statement = site
            .statement
            .map(u32::try_from)
            .transpose()
            .map_err(|_| retained_resource(self.original.relation, Resource::Arithmetic))?;
        let site = ProductionSliceAccessSiteV1::new(
            self.original.original_root,
            self.original.semantic_function,
            SemanticBlockIdV1::from_index(block),
            statement,
            ordinal,
            SemanticBlockIdV1::from_index(assertion),
        );
        self.original
            .relation
            .with_optimized_checked_slice_access_v18(
                self.optimized,
                self.original.root,
                self.original.instance,
                site,
                self.original.budget,
                |view, local| checked_slice_projection_v18(view.original(), local),
            )
            .map_err(source_error)
    }
    fn is_materialized_block(
        &mut self,
        block: usize,
    ) -> Result<bool, ProductionRankedProjectionErrorV1> {
        let disposition = self.block_disposition(SemanticBlockIdV1::from_index(
            u32::try_from(block)
                .map_err(|_| retained_resource(self.original.relation, Resource::Arithmetic))?,
        ))?;
        Ok(matches!(
            disposition,
            ProductionOptimizedSourceSiteControlV18::Retained
                | ProductionOptimizedSourceSiteControlV18::Mixed
        ))
    }
    fn condition(
        &mut self,
        block: usize,
        expected: bool,
        semantic_success: SemanticBlockIdV1,
    ) -> Result<ProjectedAssertionConditionV1, ProductionRankedProjectionErrorV1> {
        self.check()?;
        let block = SemanticBlockIdV1::from_index(
            u32::try_from(block)
                .map_err(|_| retained_resource(self.original.relation, Resource::Arithmetic))?,
        );
        let original = self
            .original
            .relation
            .assertion(
                self.original.root,
                self.original.instance,
                block,
                self.original.budget,
            )
            .map_err(source_error)?;
        if original.expected() != expected || original.semantic_success() != semantic_success {
            return Err(binding(
                "optimized assertion changed original polarity or successor",
            ));
        }
        let outcome = self
            .optimized
            .assertion(
                self.original.root,
                self.original.instance,
                block,
                self.original.budget,
            )
            .map_err(source_error)?;
        Ok(match outcome {
            SemanticKirOptimizedAssertOutcomeV1::Conditional { condition, .. } => {
                let value = self
                    .sparse
                    .value_at_use(condition.coordinate, self.original.budget)
                    .map_err(|error| {
                        retain_resource_error(
                            self.original.relation,
                            ProductionRankedProjectionErrorV1::CanonicalAssertions(
                                CanonicalAssertionErrorV1::Sparse(error),
                            ),
                        )
                    })?;
                match value {
                    CanonicalKirSparseValueV1::Constant(value)
                        if value.ty() == fe2o3_kernel_ir::ScalarType::Bool && value.bits() <= 1 =>
                    {
                        ProjectedAssertionConditionV1::Bool(value.bits() == 1)
                    }
                    CanonicalKirSparseValueV1::Constant(_) => {
                        return Err(binding("optimized assertion is not an exact Boolean"));
                    }
                    CanonicalKirSparseValueV1::Unreachable => {
                        ProjectedAssertionConditionV1::Dormant
                    }
                    CanonicalKirSparseValueV1::Unknown => ProjectedAssertionConditionV1::Unknown,
                    CanonicalKirSparseValueV1::Dynamic => ProjectedAssertionConditionV1::Dynamic,
                }
            }
            SemanticKirOptimizedAssertOutcomeV1::SelectedSuccess { .. } => {
                ProjectedAssertionConditionV1::Bool(expected)
            }
            SemanticKirOptimizedAssertOutcomeV1::SelectedFailure { .. } => {
                ProjectedAssertionConditionV1::Bool(!expected)
            }
            SemanticKirOptimizedAssertOutcomeV1::RemovedUnreachable { .. } => {
                ProjectedAssertionConditionV1::Dormant
            }
            SemanticKirOptimizedAssertOutcomeV1::SourceRuleElision { .. } => {
                ProjectedAssertionConditionV1::ElidedByExistingRule
            }
        })
    }
}

impl fe2o3_mir_model::SemanticAssertionMeterV1 for OptimizedSourceFactsV18<'_, '_, '_, '_, '_, '_> {
    type Error = ProductionRankedProjectionErrorV1;

    fn charge_work(&mut self, amount: usize) -> Result<(), Self::Error> {
        self.check()?;
        self.original
            .budget
            .charge_work(amount)
            .map_err(|error| retained_resource(self.original.relation, error))
    }

    fn reserve_storage(&mut self, bytes: usize) -> Result<(), Self::Error> {
        self.check()?;
        self.original
            .budget
            .reserve_storage(bytes)
            .map_err(|error| retained_resource(self.original.relation, error))
    }
}

#[cfg(test)]
mod header_tests {
    use super::*;
    use std::{mem::size_of, panic::AssertUnwindSafe};

    struct LegacyFacts;

    impl ProjectedAssertionFactsV1 for LegacyFacts {
        fn charge_private_array_work(
            &mut self,
            _: usize,
        ) -> Result<(), ProductionRankedProjectionErrorV1> {
            panic!("legacy site control added a work charge")
        }
        fn private_array_initializer_count(
            &mut self,
            _: usize,
            _: usize,
        ) -> Result<Option<u64>, ProductionRankedProjectionErrorV1> {
            panic!("legacy query")
        }
        fn is_materialized_block(
            &mut self,
            _: usize,
        ) -> Result<bool, ProductionRankedProjectionErrorV1> {
            panic!("legacy query")
        }
        fn condition(
            &mut self,
            _: usize,
            _: bool,
            _: SemanticBlockIdV1,
        ) -> Result<ProjectedAssertionConditionV1, ProductionRankedProjectionErrorV1> {
            panic!("legacy query")
        }
    }

    #[test]
    fn original_projection_site_control_is_default_inert() {
        let mut facts = LegacyFacts;
        for site in [(0, None), (5, Some(3)), (usize::MAX, Some(usize::MAX))] {
            assert!(
                canonical_assertion_facts_v1::project_source_site_v18(&mut facts, site.0, site.1)
                    .unwrap()
            );
            assert_eq!(facts.source_site_control_v18(site.0, site.1).unwrap(), None);
        }
    }

    // Isolated consumer dispatch test, not a checked source-control producer.
    struct SyntheticControl(ProjectedSourceSiteControlV18);

    impl ProjectedAssertionFactsV1 for SyntheticControl {
        fn source_site_control_v18(
            &mut self,
            _: usize,
            _: Option<usize>,
        ) -> Result<Option<ProjectedSourceSiteControlV18>, ProductionRankedProjectionErrorV1>
        {
            Ok(Some(self.0))
        }
        fn charge_private_array_work(
            &mut self,
            _: usize,
        ) -> Result<(), ProductionRankedProjectionErrorV1> {
            panic!("unrelated query")
        }
        fn private_array_initializer_count(
            &mut self,
            _: usize,
            _: usize,
        ) -> Result<Option<u64>, ProductionRankedProjectionErrorV1> {
            panic!("unrelated query")
        }
        fn is_materialized_block(
            &mut self,
            _: usize,
        ) -> Result<bool, ProductionRankedProjectionErrorV1> {
            panic!("unrelated query")
        }
        fn condition(
            &mut self,
            _: usize,
            _: bool,
            _: SemanticBlockIdV1,
        ) -> Result<ProjectedAssertionConditionV1, ProductionRankedProjectionErrorV1> {
            panic!("unrelated query")
        }
    }

    #[test]
    fn projection_omits_only_complete_dead_sites_and_refuses_partial_sites() {
        use ProjectedSourceSiteControlV18 as D;
        for disposition in [
            D::Retained,
            D::RemovedUnreachable,
            D::Mixed,
            D::OriginalUnmaterialized,
        ] {
            let result = canonical_assertion_facts_v1::project_source_site_v18(
                &mut SyntheticControl(disposition),
                0,
                None,
            );
            match disposition {
                D::Retained | D::OriginalUnmaterialized => assert!(result.unwrap()),
                D::RemovedUnreachable => assert!(!result.unwrap()),
                D::Mixed => assert!(matches!(
                    result,
                    Err(ProductionRankedProjectionErrorV1::Incomplete(
                        "partially surviving source site requires exact ranked segment projection"
                    ))
                )),
            }
        }
    }

    #[test]
    fn original_and_optimized_facts_coexist_under_independent_header_oracle() {
        type Output = [u128; 3];
        let expected = size_of::<OptimizedSourceFactsV18<'_, '_, '_, '_, '_, '_>>()
            + size_of::<SourceAssertionFactsV18<'_, '_, '_, '_, '_>>()
            + size_of::<
                Result<
                    ProductionOptimizedSourceBlockControlV18<'_, '_>,
                    ProductionSourceOwnedViewErrorV18,
                >,
            >()
            + size_of::<Result<Output, ProductionRankedProjectionErrorV1>>()
            + size_of::<std::thread::Result<Result<Output, ProductionRankedProjectionErrorV1>>>()
            + size_of::<AssertUnwindSafe<Output>>();
        assert_eq!(optimized_facts_headers_v18::<Output>().unwrap(), expected);
        for one_short in [false, true] {
            let limit = 5 + expected - usize::from(one_short);
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(0);
            let mut budget = Budget::new(&mut work, limit);
            budget.reserve_storage(5).unwrap();
            let result = budget.reserve_storage(optimized_facts_headers_v18::<Output>().unwrap());
            if one_short {
                let Err(Resource::Storage(error)) = result else {
                    panic!("one-short facts header");
                };
                assert_eq!((error.actual(), error.limit()), (5 + expected, limit));
                assert_eq!(budget.storage(), 5);
                assert_eq!(budget.peak_storage(), 5);
                assert_eq!(budget.failed_storage(), Some(error.actual()));
            } else {
                result.unwrap();
                assert_eq!(budget.storage(), limit);
                assert_eq!(budget.peak_storage(), limit);
                assert_eq!(budget.failed_storage(), None);
            }
            assert_eq!(budget.work(), 0);
        }
    }
}
