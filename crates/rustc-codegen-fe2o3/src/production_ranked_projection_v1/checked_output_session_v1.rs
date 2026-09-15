//! Private assertion projection over one actual source-qualified checked O.
//! Direct private-array writes use exact checked O operands. Other memory
//! rules and backend activation remain separate.

use super::{
    ProductionRankedProjectionErrorV1 as ProjectionError,
    canonical_assertion_facts_v1::{
        CanonicalAssertionErrorV1, ProjectedAssertionConditionV1, ProjectedAssertionFactsV1,
    },
};
use fe2o3_amd_target::ProductionAmdTargetProfileV1;
use fe2o3_kernel_analysis::{CanonicalKirSparseV1, CanonicalKirSparseValueV1};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, ScalarType,
    VerifiedCanonicalKernelIrModuleV12,
};
use fe2o3_lower_mir_kernel::{
    ProductionPreRankedKirOwnerV1, ProductionSourceOutputBlockV1, ProductionSourceOutputErrorV1,
    ProductionSourceOutputOccurrencesV1, ProductionSourceOutputPrivateArrayAccessV1,
    SemanticKirOptimizedAssertOutcomeV1, derive_source_output_occurrences_v1,
};
use fe2o3_mir_model::semantic_mir_v1::{SemanticBlockIdV1, SemanticFunctionIdV1};
use fe2o3_pliron::{CheckedNeutralKernelIrOwnerV1, with_canonical_analysis_scope_v1};

mod ranked_private_v1 {
    include!("checked_output_ranked_private_v1.rs");
}

fn resource(error: Resource) -> ProjectionError {
    ProjectionError::CanonicalAssertions(CanonicalAssertionErrorV1::Resource(error))
}
fn source_output(error: ProductionSourceOutputErrorV1) -> ProjectionError {
    ProjectionError::CanonicalAssertions(CanonicalAssertionErrorV1::Output(error))
}
fn binding(detail: &'static str) -> ProjectionError {
    ProjectionError::CanonicalAssertions(CanonicalAssertionErrorV1::Binding(detail))
}

pub(super) struct CheckedOutputAssertionSessionV1<'view, 'inventory, 'owners, 'budget, 'work> {
    occurrences: &'view ProductionSourceOutputOccurrencesV1<'owners, 'owners>,
    report: &'view CanonicalKirSparseV1<'inventory, 'owners>,
    budget: &'budget mut Budget<'work>,
}
impl CheckedOutputAssertionSessionV1<'_, '_, '_, '_, '_> {
    #[cfg(test)]
    pub(super) fn for_source_with_query_budget_v1<'a>(
        &'a self,
        budget: &'a mut Budget<'_>,
        owner: SemanticFunctionIdV1,
        function: SemanticFunctionIdV1,
    ) -> impl ProjectedAssertionFactsV1 + 'a {
        CheckedOutputSourceAssertionFactsV1 {
            occurrences: self.occurrences,
            report: self.report,
            budget,
            owner,
            function,
        }
    }

    pub(super) fn with_output_occurrences_v1<T>(
        &mut self,
        body: impl FnOnce(
            &ProductionSourceOutputOccurrencesV1<'_, '_>,
            &mut Budget<'_>,
        ) -> Result<T, ProjectionError>,
    ) -> Result<T, ProjectionError> {
        // One scoped consumer dispatch; each occurrence lookup is charged below.
        self.budget.charge_work(1).map_err(resource)?;
        body(self.occurrences, self.budget)
    }

    #[allow(
        dead_code,
        reason = "Checked-output backend staging is not activated by this library packet."
    )]
    pub(super) fn for_source(
        &mut self,
        owner: SemanticFunctionIdV1,
        function: SemanticFunctionIdV1,
    ) -> impl ProjectedAssertionFactsV1 + '_ {
        CheckedOutputSourceAssertionFactsV1 {
            occurrences: self.occurrences,
            report: self.report,
            budget: &mut *self.budget,
            owner,
            function,
        }
    }
}

struct CheckedOutputSourceAssertionFactsV1<'view, 'inventory, 'owners, 'budget, 'work> {
    occurrences: &'view ProductionSourceOutputOccurrencesV1<'owners, 'owners>,
    report: &'view CanonicalKirSparseV1<'inventory, 'owners>,
    budget: &'budget mut Budget<'work>,
    owner: SemanticFunctionIdV1,
    function: SemanticFunctionIdV1,
}
impl ProjectedAssertionFactsV1 for CheckedOutputSourceAssertionFactsV1<'_, '_, '_, '_, '_> {
    fn with_checked_control_scope_v1<T>(
        &mut self,
        body: impl FnOnce(&mut Self) -> Result<T, ProjectionError>,
    ) -> Result<T, ProjectionError> {
        let floor = self.budget.storage();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body(self)));
        let released = self
            .budget
            .storage()
            .checked_sub(floor)
            .ok_or_else(|| resource(Resource::Accounting))?;
        self.budget.release_storage(released).map_err(resource)?;
        match result {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    fn checked_control_enabled_v1(&self) -> bool {
        true
    }

    fn reserve_checked_control_storage_v1(&mut self, bytes: usize) -> Result<(), ProjectionError> {
        self.budget.reserve_storage(bytes).map_err(resource)
    }

    fn checked_block_coverage_v1(
        &mut self,
        block: usize,
    ) -> Result<fe2o3_lower_mir_kernel::ProductionSourceOutputBlockCoverageV1, ProjectionError>
    {
        self.budget.charge_work(1).map_err(resource)?;
        let block = u32::try_from(block)
            .map_err(|_| binding("checked control source block index overflows"))?;
        self.occurrences
            .block_coverage(
                self.owner,
                self.function,
                SemanticBlockIdV1::from_index(block),
                self.budget,
            )
            .map_err(source_output)
    }

    fn checked_selected_successor_v1(
        &mut self,
        block: usize,
    ) -> Result<
        Option<fe2o3_lower_mir_kernel::ProductionSourceOutputSelectedSuccessorV1>,
        ProjectionError,
    > {
        self.budget.charge_work(1).map_err(resource)?;
        let block = u32::try_from(block)
            .map_err(|_| binding("checked control source block index overflows"))?;
        self.occurrences
            .selected_successor(
                self.owner,
                self.function,
                SemanticBlockIdV1::from_index(block),
                self.budget,
            )
            .map_err(source_output)
    }

    fn check_ranked_private_array_sources(
        &mut self,
        blocks: &[super::ProductionRankedBlockV1],
        sources: &[super::ProjectedAccessSourceV1],
    ) -> Result<(), ProjectionError> {
        // Exact source-root lookup and export lookup; this is the root whose name
        // the shared caller gives to the immediately subsequent ranked constructor.
        self.budget.charge_work(2).map_err(resource)?;
        let root = self
            .occurrences
            .source()
            .semantic_ssa()
            .source_semantic()
            .functions()
            .get(self.owner.index() as usize)
            .ok_or_else(|| binding("checked output ranked source root is absent"))?;
        let entry = root
            .kernel_entry()
            .ok_or_else(|| binding("checked output ranked source export is absent"))?;
        self.budget
            .charge_work(entry.export_symbol().as_bytes().len())
            .map_err(resource)?;
        let name = std::str::from_utf8(entry.export_symbol().as_bytes())
            .map_err(|_| binding("checked output ranked source export is not UTF-8"))?;
        ranked_private_v1::check(
            self.occurrences,
            self.owner,
            self.function,
            name,
            blocks,
            sources,
            self.budget,
        )
    }

    fn charge_private_array_work(&mut self, amount: usize) -> Result<(), ProjectionError> {
        self.budget.charge_work(amount).map_err(resource)
    }
    fn private_array_access(
        &mut self,
        site: fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1,
        role: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
    ) -> Result<bool, ProjectionError> {
        self.budget.charge_work(1).map_err(resource)?;
        self.private_array_constant_index(site, role)
            .map(|index| index.is_some())
    }
    fn private_array_constant_index(
        &mut self,
        site: fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1,
        role: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
    ) -> Result<Option<u64>, ProjectionError> {
        // Outcome dispatch and independent executable-placement guard.
        self.budget.charge_work(2).map_err(resource)?;
        match self
            .occurrences
            .private_array_write(self.owner, self.function, site, role, self.budget)
            .map_err(source_output)?
        {
            ProductionSourceOutputPrivateArrayAccessV1::ProvenUnretained => Ok(None),
            ProductionSourceOutputPrivateArrayAccessV1::Retained {
                index,
                executable: true,
                ..
            } => Ok(Some(index)),
            ProductionSourceOutputPrivateArrayAccessV1::Retained {
                executable: false, ..
            }
            | ProductionSourceOutputPrivateArrayAccessV1::OmittedUnreachable => Err(
                ProjectionError::Incomplete("checked output private-array write is not executable"),
            ),
        }
    }
    fn is_materialized_block(&mut self, block: usize) -> Result<bool, ProjectionError> {
        self.budget.charge_work(2).map_err(resource)?;
        let block = u32::try_from(block)
            .map_err(|_| binding("checked output source block index overflows"))?;
        Ok(
            match self
                .occurrences
                .block(
                    self.owner,
                    self.function,
                    SemanticBlockIdV1::from_index(block),
                    self.budget,
                )
                .map_err(source_output)?
            {
                ProductionSourceOutputBlockV1::NotMaterialized => false,
                ProductionSourceOutputBlockV1::Materialized { executable, .. } => executable,
            },
        )
    }
    fn condition(
        &mut self,
        block: usize,
        expected: bool,
        semantic_success: SemanticBlockIdV1,
    ) -> Result<ProjectedAssertionConditionV1, ProjectionError> {
        // Three source-frame checks, outcome dispatch, and four bounded
        // Boolean-literal decision checks; sparse lookup has its own charge.
        self.budget.charge_work(8).map_err(resource)?;
        let block = u32::try_from(block)
            .map_err(|_| binding("checked output assertion block index overflows"))?;
        let assertion = self
            .occurrences
            .assertion(
                self.owner,
                self.function,
                SemanticBlockIdV1::from_index(block),
                self.budget,
            )
            .map_err(source_output)?;
        if assertion.expected() != expected || assertion.semantic_success() != semantic_success {
            return Err(binding("checked output assertion source contract differs"));
        }
        Ok(match assertion.outcome() {
            SemanticKirOptimizedAssertOutcomeV1::SelectedSuccess { .. } => {
                ProjectedAssertionConditionV1::Bool(expected)
            }
            SemanticKirOptimizedAssertOutcomeV1::SelectedFailure { .. } => {
                ProjectedAssertionConditionV1::Bool(!expected)
            }
            SemanticKirOptimizedAssertOutcomeV1::SourceRuleElision { .. } => {
                ProjectedAssertionConditionV1::ElidedByExistingRule
            }
            SemanticKirOptimizedAssertOutcomeV1::RemovedUnreachable { .. } => {
                return Err(ProjectionError::Incomplete(
                    "checked-unreachable assertion has no live output predicate",
                ));
            }
            SemanticKirOptimizedAssertOutcomeV1::Conditional { condition, .. } => {
                let value = self
                    .report
                    .value_at_use(condition.coordinate, self.budget)
                    .map_err(|error| {
                        ProjectionError::CanonicalAssertions(CanonicalAssertionErrorV1::Sparse(
                            error,
                        ))
                    })?;
                match value {
                    CanonicalKirSparseValueV1::Constant(value) => {
                        if value.ty() != ScalarType::Bool || value.bits() > 1 {
                            return Err(binding("checked output assertion is not Boolean"));
                        }
                        ProjectedAssertionConditionV1::Bool(value.bits() == 1)
                    }
                    CanonicalKirSparseValueV1::Unreachable => {
                        return Err(ProjectionError::Incomplete(
                            "retained assertion predicate has no executable sparse value",
                        ));
                    }
                    CanonicalKirSparseValueV1::Unknown => ProjectedAssertionConditionV1::Unknown,
                    CanonicalKirSparseValueV1::Dynamic => ProjectedAssertionConditionV1::Dynamic,
                }
            }
        })
    }
}

/// Borrows the exact already-produced B/O pair. Production staging creates B
/// with the existing binder and O with the existing checked optimizer; this
/// entry independently validates the full pair and accepts no trusted relation
/// or alternate executable callback. The callback can only inspect/project
/// borrowed source-qualified facts; it cannot construct an executable owner.
#[allow(
    dead_code,
    reason = "Checked-output backend staging is not activated by this library packet."
)]
pub(super) fn with_checked_output_assertions_budget_v1<'owners, T>(
    source: &'owners ProductionPreRankedKirOwnerV1,
    bound: &'owners VerifiedCanonicalKernelIrModuleV12,
    checked: &'owners CheckedNeutralKernelIrOwnerV1,
    profile: ProductionAmdTargetProfileV1,
    budget: &mut Budget<'_>,
    body: impl FnOnce(
        &mut CheckedOutputAssertionSessionV1<'_, '_, 'owners, '_, '_>,
    ) -> Result<T, ProjectionError>,
) -> Result<T, ProjectionError> {
    let floor = budget.storage();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let (result, coordinate_storage) = {
            let (coordinates, coordinate_storage) =
                dialect_amdgcn::check_production_target_coordinate_preservation_v1(
                    source.executable(),
                    bound,
                    profile,
                    budget,
                )
                .map_err(|error| {
                    ProjectionError::CanonicalAssertions(CanonicalAssertionErrorV1::Target(error))
                })?;
            budget
                .reserve_storage(coordinate_storage.retained_storage())
                .map_err(resource)?;
            let (occurrences, occurrence_storage) =
                derive_source_output_occurrences_v1(source, &coordinates, checked, budget)
                    .map_err(source_output)?;
            budget
                .reserve_storage(occurrence_storage.retained_storage())
                .map_err(resource)?;
            let result = with_canonical_analysis_scope_v1(checked.owner(), budget, |scope| {
                scope.with_sparse_v1(|report, budget| {
                    body(&mut CheckedOutputAssertionSessionV1 {
                        occurrences: &occurrences,
                        report,
                        budget,
                    })
                })
            });
            drop(occurrences);
            budget
                .release_storage(occurrence_storage.retained_storage())
                .map_err(resource)?;
            (result, coordinate_storage)
        };
        budget
            .release_storage(coordinate_storage.retained_storage())
            .map_err(resource)?;
        result
    }));
    let released = budget
        .storage()
        .checked_sub(floor)
        .ok_or_else(|| resource(Resource::Accounting))?;
    budget.release_storage(released).map_err(resource)?;
    match result {
        Ok(result) => result,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}
