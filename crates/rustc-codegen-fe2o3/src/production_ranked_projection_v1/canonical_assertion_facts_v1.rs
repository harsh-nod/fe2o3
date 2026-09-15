//! One lexical, phase-local graph analysis for ranked assertion decisions.
//! Source/ranked allocations retain their existing separate limits.

#[cfg(test)]
use super::ranked_projection_source_v1::with_projection_source_budget_v1;
use super::{
    ProductionRankedProjectionErrorV1 as ProjectionError,
    ranked_projection_source_v1::RankedProjectionSourceV1,
};
use fe2o3_kernel_analysis::{
    CanonicalKirInventoryErrorV1, CanonicalKirSparseErrorV1, CanonicalKirSparseV1,
    CanonicalKirSparseValueV1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, ScalarType,
};
use fe2o3_lower_mir_kernel::{ProductionPreRankedKirOwnerV1, SemanticKirPrivateArrayQueryErrorV1};
use fe2o3_lower_mir_kernel::{
    SemanticKirAssertConditionOutcomeV1, SemanticKirAssertOriginErrorV1, SemanticKirAssertOriginsV1,
};
use fe2o3_mir_model::semantic_mir_v1::{SemanticBlockIdV1, SemanticFunctionIdV1};
use fe2o3_pliron::{CanonicalAnalysisScopeErrorV1, with_canonical_analysis_scope_v1};
use std::{error::Error, fmt};

#[derive(Debug)]
pub(crate) enum CanonicalAssertionErrorV1 {
    Resource(Resource),
    Inventory(CanonicalKirInventoryErrorV1),
    Sparse(CanonicalKirSparseErrorV1),
    Origin(SemanticKirAssertOriginErrorV1),
    Output(fe2o3_lower_mir_kernel::ProductionSourceOutputErrorV1),
    Target(dialect_amdgcn::ProductionTargetCoordinateErrorV1),
    PrivateArray(SemanticKirPrivateArrayQueryErrorV1),
    Binding(&'static str),
}
impl fmt::Display for CanonicalAssertionErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Inventory(error) => error.fmt(f),
            Self::Sparse(error) => error.fmt(f),
            Self::Origin(error) => error.fmt(f),
            Self::Output(error) => error.fmt(f),
            Self::Target(error) => error.fmt(f),
            Self::PrivateArray(error) => error.fmt(f),
            Self::Binding(detail) => f.write_str(detail),
        }
    }
}
impl Error for CanonicalAssertionErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Inventory(error) => Some(error),
            Self::Sparse(error) => Some(error),
            Self::Origin(error) => Some(error),
            Self::Output(error) => Some(error),
            Self::Target(error) => Some(error),
            Self::PrivateArray(error) => Some(error),
            Self::Binding(_) => None,
        }
    }
}
fn reject(error: CanonicalAssertionErrorV1) -> ProjectionError {
    ProjectionError::CanonicalAssertions(error)
}
fn resource(error: Resource) -> ProjectionError {
    reject(CanonicalAssertionErrorV1::Resource(error))
}

impl From<CanonicalAnalysisScopeErrorV1> for ProjectionError {
    fn from(error: CanonicalAnalysisScopeErrorV1) -> Self {
        reject(match error {
            CanonicalAnalysisScopeErrorV1::Resource(error) => {
                CanonicalAssertionErrorV1::Resource(error)
            }
            CanonicalAnalysisScopeErrorV1::Inventory(error) => {
                CanonicalAssertionErrorV1::Inventory(error)
            }
            CanonicalAnalysisScopeErrorV1::Sparse(error) => {
                CanonicalAssertionErrorV1::Sparse(error)
            }
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ProjectedAssertionConditionV1 {
    Bool(bool),
    Dormant,
    Unknown,
    Dynamic,
    ElidedByExistingRule,
}

/// Private consumer contract. Production implements it only with the sealed
/// origin view and exact borrowed graph report below. Tests must identify any
/// isolated synthetic decision inputs explicitly.
pub(super) trait ProjectedAssertionFactsV1 {
    fn with_checked_control_scope_v1<T>(
        &mut self,
        body: impl FnOnce(&mut Self) -> Result<T, ProjectionError>,
    ) -> Result<T, ProjectionError>
    where
        Self: Sized,
    {
        body(self)
    }

    fn checked_control_enabled_v1(&self) -> bool {
        false
    }

    fn reserve_checked_control_storage_v1(&mut self, _bytes: usize) -> Result<(), ProjectionError> {
        Err(ProjectionError::Incomplete(
            "checked control storage requires checked output facts",
        ))
    }

    fn checked_block_coverage_v1(
        &mut self,
        _block: usize,
    ) -> Result<fe2o3_lower_mir_kernel::ProductionSourceOutputBlockCoverageV1, ProjectionError>
    {
        Err(ProjectionError::Incomplete(
            "checked control coverage requires checked output facts",
        ))
    }

    fn checked_selected_successor_v1(
        &mut self,
        _block: usize,
    ) -> Result<
        Option<fe2o3_lower_mir_kernel::ProductionSourceOutputSelectedSuccessorV1>,
        ProjectionError,
    > {
        Err(ProjectionError::Incomplete(
            "checked control selection requires checked output facts",
        ))
    }

    // N keeps its existing final attachment; this extra O check does no N work.
    fn check_ranked_private_array_sources(
        &mut self,
        _blocks: &[super::ProductionRankedBlockV1],
        _sources: &[super::ProjectedAccessSourceV1],
    ) -> Result<(), ProjectionError> {
        Ok(())
    }

    fn charge_private_array_work(&mut self, amount: usize) -> Result<(), ProjectionError>;
    fn private_array_access(
        &mut self,
        site: fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1,
        role: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
    ) -> Result<bool, ProjectionError>;
    fn private_array_constant_index(
        &mut self,
        site: fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1,
        role: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
    ) -> Result<Option<u64>, ProjectionError>;
    fn is_materialized_block(&mut self, block: usize) -> Result<bool, ProjectionError>;

    fn condition(
        &mut self,
        block: usize,
        expected: bool,
        semantic_success: SemanticBlockIdV1,
    ) -> Result<ProjectedAssertionConditionV1, ProjectionError>;
}

pub(super) fn projected_assertion_is_proved_v1(
    condition: ProjectedAssertionConditionV1,
    expected: bool,
    existing_source_proof: bool,
) -> bool {
    match condition {
        ProjectedAssertionConditionV1::Bool(actual) => actual == expected,
        ProjectedAssertionConditionV1::Dormant
        | ProjectedAssertionConditionV1::Unknown
        | ProjectedAssertionConditionV1::Dynamic
        | ProjectedAssertionConditionV1::ElidedByExistingRule => existing_source_proof,
    }
}

pub(super) struct CanonicalAssertionSessionV1<'r, 'i, 'g, 'b, 'w> {
    materialized: &'g ProductionPreRankedKirOwnerV1,
    origins: SemanticKirAssertOriginsV1<'g>,
    report: &'r CanonicalKirSparseV1<'i, 'g>,
    budget: &'b mut Budget<'w>,
}
impl CanonicalAssertionSessionV1<'_, '_, '_, '_, '_> {
    #[cfg(test)]
    pub(super) fn retained_floor_for_test_v1(&self) -> usize {
        self.budget.storage()
    }

    #[cfg(test)]
    pub(super) fn for_source_with_query_budget_v1<'a>(
        &'a self,
        budget: &'a mut Budget<'_>,
        correspondence_owner: SemanticFunctionIdV1,
        semantic_function: SemanticFunctionIdV1,
    ) -> impl ProjectedAssertionFactsV1 + 'a {
        CanonicalSourceAssertionFactsV1 {
            materialized: self.materialized,
            origins: self.origins,
            report: self.report,
            budget,
            correspondence_owner,
            semantic_function,
        }
    }

    pub(super) fn for_source(
        &mut self,
        correspondence_owner: SemanticFunctionIdV1,
        semantic_function: SemanticFunctionIdV1,
    ) -> impl ProjectedAssertionFactsV1 + '_ {
        CanonicalSourceAssertionFactsV1 {
            materialized: self.materialized,
            origins: self.origins,
            report: self.report,
            budget: &mut *self.budget,
            correspondence_owner,
            semantic_function,
        }
    }
}

struct CanonicalSourceAssertionFactsV1<'r, 'i, 'g, 'b, 'w> {
    materialized: &'g ProductionPreRankedKirOwnerV1,
    origins: SemanticKirAssertOriginsV1<'g>,
    report: &'r CanonicalKirSparseV1<'i, 'g>,
    budget: &'b mut Budget<'w>,
    correspondence_owner: SemanticFunctionIdV1,
    semantic_function: SemanticFunctionIdV1,
}
impl ProjectedAssertionFactsV1 for CanonicalSourceAssertionFactsV1<'_, '_, '_, '_, '_> {
    fn charge_private_array_work(&mut self, amount: usize) -> Result<(), ProjectionError> {
        self.budget.charge_work(amount).map_err(resource)
    }

    fn private_array_access(
        &mut self,
        site: fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1,
        role: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
    ) -> Result<bool, ProjectionError> {
        self.materialized
            .has_materialized_private_array_access(
                self.correspondence_owner,
                self.semantic_function,
                site,
                role,
                self.budget,
            )
            .map_err(|error| reject(CanonicalAssertionErrorV1::PrivateArray(error)))
    }
    fn private_array_constant_index(
        &mut self,
        site: fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1,
        role: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
    ) -> Result<Option<u64>, ProjectionError> {
        self.materialized
            .materialized_private_array_constant_index(
                self.correspondence_owner,
                self.semantic_function,
                site,
                role,
                self.budget,
            )
            .map_err(|error| reject(CanonicalAssertionErrorV1::PrivateArray(error)))
    }

    fn is_materialized_block(&mut self, block: usize) -> Result<bool, ProjectionError> {
        self.budget.charge_work(1).map_err(resource)?;
        let block = u32::try_from(block).map_err(|_| resource(Resource::Arithmetic))?;
        self.origins
            .is_materialized_block(
                self.correspondence_owner,
                self.semantic_function,
                SemanticBlockIdV1::from_index(block),
                self.budget,
            )
            .map_err(|error| reject(CanonicalAssertionErrorV1::Origin(error)))
    }

    fn condition(
        &mut self,
        block: usize,
        expected: bool,
        semantic_success: SemanticBlockIdV1,
    ) -> Result<ProjectedAssertionConditionV1, ProjectionError> {
        // Fixed site conversion, polarity/success comparisons, and scalar
        // interpretation. Origin lookup and graph-use query charge separately.
        self.budget.charge_work(4).map_err(resource)?;
        let block = u32::try_from(block).map_err(|_| resource(Resource::Arithmetic))?;
        let binding = self
            .origins
            .assert_condition(
                self.correspondence_owner,
                self.semantic_function,
                SemanticBlockIdV1::from_index(block),
                self.budget,
            )
            .map_err(|error| reject(CanonicalAssertionErrorV1::Origin(error)))?;
        if binding.expected() != expected || binding.semantic_success() != semantic_success {
            return Err(reject(CanonicalAssertionErrorV1::Binding(
                "canonical assertion source polarity or success occurrence changed",
            )));
        }
        match binding.outcome() {
            SemanticKirAssertConditionOutcomeV1::ElidedByExistingRule { .. } => {
                Ok(ProjectedAssertionConditionV1::ElidedByExistingRule)
            }
            SemanticKirAssertConditionOutcomeV1::Emitted { condition_use, .. } => {
                // The lowerer's sealed binding independently checks the full
                // definition, both edge occurrences/payloads and failure block.
                let value = self
                    .report
                    .value_at_use(condition_use, self.budget)
                    .map_err(|error| reject(CanonicalAssertionErrorV1::Sparse(error)))?;
                Ok(match value {
                    CanonicalKirSparseValueV1::Constant(value)
                        if value.ty() == ScalarType::Bool && value.bits() <= 1 =>
                    {
                        ProjectedAssertionConditionV1::Bool(value.bits() == 1)
                    }
                    CanonicalKirSparseValueV1::Constant(_) => {
                        return Err(reject(CanonicalAssertionErrorV1::Binding(
                            "canonical assertion condition is not an exact Boolean",
                        )));
                    }
                    CanonicalKirSparseValueV1::Unreachable => {
                        ProjectedAssertionConditionV1::Dormant
                    }
                    CanonicalKirSparseValueV1::Unknown => ProjectedAssertionConditionV1::Unknown,
                    CanonicalKirSparseValueV1::Dynamic => ProjectedAssertionConditionV1::Dynamic,
                })
            }
        }
    }
}

/// The caller reserves the complete borrowed source payload. Only canonical
/// assertion/inventory/sparse work is on this ledger, not the Rust/projector
/// phases. Inner owners drop before cleanup. With valid ledger accounting,
/// cleanup restores the entry floor and resumes the original unwind payload;
/// corrupted accounting instead returns its resource error.
pub(super) fn with_canonical_assertions_source_budget_v1<T>(
    source: &RankedProjectionSourceV1<'_>,
    budget: &mut Budget<'_>,
    body: impl FnOnce(
        &mut CanonicalAssertionSessionV1<'_, '_, '_, '_, '_>,
    ) -> Result<T, ProjectionError>,
) -> Result<T, ProjectionError> {
    source.require_floor(budget)?;
    budget.charge_work(1).map_err(resource)?;
    if !std::ptr::eq(source.origins().executable(), source.executable()) {
        return Err(reject(CanonicalAssertionErrorV1::Binding(
            "canonical assertion origins belong to a different executable owner",
        )));
    }
    let floor = budget.storage();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_canonical_analysis_scope_v1(source.executable(), budget, |scope| {
            scope.with_sparse_v1(|report, budget| {
                body(&mut CanonicalAssertionSessionV1 {
                    materialized: source.materialized(),
                    origins: source.origins(),
                    report,
                    budget,
                })
            })
        })
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

#[cfg(test)]
pub(super) fn with_canonical_assertions_v1<T>(
    materialized: &ProductionPreRankedKirOwnerV1,
    body: impl FnOnce(
        &mut CanonicalAssertionSessionV1<'_, '_, '_, '_, '_>,
    ) -> Result<T, ProjectionError>,
) -> Result<T, ProjectionError> {
    let source = RankedProjectionSourceV1::from_legacy(materialized)?;
    with_projection_source_budget_v1(&source, |budget| {
        with_canonical_assertions_source_budget_v1(&source, budget, body)
    })
}

#[cfg(test)]
pub(super) fn with_canonical_assertions_budget_v1<T>(
    materialized: &ProductionPreRankedKirOwnerV1,
    budget: &mut Budget<'_>,
    body: impl FnOnce(
        &mut CanonicalAssertionSessionV1<'_, '_, '_, '_, '_>,
    ) -> Result<T, ProjectionError>,
) -> Result<T, ProjectionError> {
    let source = RankedProjectionSourceV1::from_legacy(materialized)?;
    with_canonical_assertions_source_budget_v1(&source, budget, body)
}
