//! One lexical, phase-local graph analysis for ranked assertion decisions.
//! Source/ranked allocations retain their existing separate limits.

#[path = "bf16_nominal_facts_observation_v1.rs"]
mod bf16_nominal_facts_observation_v1;
#[allow(unused_imports)]
pub(super) use bf16_nominal_facts_observation_v1::with_nominal_canonical_facts_observation_v1;
#[cfg(test)]
pub(crate) use bf16_nominal_facts_observation_v1::{
    inspect_foreign_nominal_facts_refusal_for_test_v1, inspect_nominal_routing_genuine_for_test_v1,
};

#[path = "bf16_nominal_capability_consumer_v1.rs"]
mod bf16_nominal_capability_consumer_v1;
#[allow(unused_imports)]
pub(super) use bf16_nominal_capability_consumer_v1::with_nominal_capability_consumer_v1;

// Private reservation-only seam; no owning recipe or strict origin producer yet.
#[allow(dead_code)]
#[path = "bf16_nominal_recipe_resources_v1.rs"]
mod bf16_nominal_recipe_resources_v1;
#[allow(unused_imports)]
pub(super) use bf16_nominal_recipe_resources_v1::with_nominal_recipe_resources_v1;

#[cfg(test)]
#[path = "bf16_nominal_recipe_resources_genuine_v1_tests.rs"]
mod bf16_nominal_recipe_resources_genuine_v1_tests;
#[cfg(test)]
pub(super) use bf16_nominal_recipe_resources_genuine_v1_tests::{
    nominal_recipe_resources_controls_for_test_v1, observe_nominal_recipe_resources_for_test_v1,
};

#[cfg(test)]
pub(super) use bf16_nominal_recipe_resources_v1::{
    nominal_initial_graph_controls_for_test_v1, observe_nominal_initial_graph_for_test_v1,
};

use super::bf16_nominal_call_routing_v1::NominalCallVisitorV1;
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
use fe2o3_lower_mir_kernel::{
    ProductionArgumentCoverageV1, ProductionPreRankedKirOwnerV1, ProductionSliceAccessSiteV1,
    SemanticKirAssertConditionOutcomeV1, SemanticKirAssertOriginErrorV1,
    SemanticKirAssertOriginsV1, SemanticKirPrivateArrayQueryErrorV1,
};
use fe2o3_mir_model::semantic_mir_v1::{SemanticBlockIdV1, SemanticFunctionIdV1};
use fe2o3_pliron::{CanonicalAnalysisScopeErrorV1, with_canonical_analysis_scope_v1};
use std::{error::Error, fmt};

#[derive(Debug)]
pub(crate) enum CanonicalAssertionErrorV1 {
    Resource(Resource),
    Inventory(CanonicalKirInventoryErrorV1),
    Sparse(CanonicalKirSparseErrorV1),
    MemorySsa(fe2o3_kernel_analysis::CanonicalKirMemorySsaErrorV1),
    Origin(SemanticKirAssertOriginErrorV1),
    PrivateArray(SemanticKirPrivateArrayQueryErrorV1),
    CallEffects(fe2o3_kernel_analysis::CanonicalKirCallEffectErrorV1),
    MaskedAssertion(fe2o3_lower_mir_kernel::ProductionSemanticMaskedShiftQueryErrorV1),
    GuardedProgress(Box<fe2o3_lower_mir_kernel::ProductionScalarSsaEmissionErrorV1>),
    NominalCall(fe2o3_lower_mir_kernel::Bf16NominalCallQueryErrorV1),
    Binding(&'static str),
}
impl fmt::Display for CanonicalAssertionErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Inventory(error) => error.fmt(f),
            Self::Sparse(error) => error.fmt(f),
            Self::MemorySsa(error) => error.fmt(f),
            Self::Origin(error) => error.fmt(f),
            Self::PrivateArray(error) => error.fmt(f),
            Self::CallEffects(error) => error.fmt(f),
            Self::MaskedAssertion(error) => error.fmt(f),
            Self::GuardedProgress(error) => error.fmt(f),
            Self::NominalCall(error) => error.fmt(f),
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
            Self::MemorySsa(error) => Some(error),
            Self::Origin(error) => Some(error),
            Self::PrivateArray(error) => Some(error),
            Self::CallEffects(error) => Some(error),
            Self::MaskedAssertion(error) => Some(error),
            Self::GuardedProgress(error) => Some(error.as_ref()),
            Self::NominalCall(error) => Some(error),
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
            CanonicalAnalysisScopeErrorV1::MemorySsa(error) => {
                CanonicalAssertionErrorV1::MemorySsa(error)
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
    #[cfg(test)]
    fn observe_conditional_bound_for_test_v1(
        &mut self,
        _bound: crate::production_reference_effect_join_v2::CompilerOwnedBoundReferenceEffectV2,
        _source: super::conditional_bound_observation_v1_tests::BoundSourceV1<'_>,
    ) -> Result<(), ProjectionError> {
        Err(ProjectionError::Incomplete(
            "post-bind observation requires a canonical owner",
        ))
    }

    #[cfg(test)]
    fn observe_conditional_prepared_for_test_v1(
        &mut self,
        _candidate: fe2o3_lower_mir_kernel::NativeRankedSourceCandidateV1<'_>,
    ) -> Result<(), ProjectionError> {
        Err(ProjectionError::Incomplete(
            "conditional observation requires a canonical owner",
        ))
    }

    // The historical route has no added guard contract. The distinct strict
    // decorator consumes its exact source/N row before the existing proof mark.
    fn require_guarded_source_progress_v1(
        &mut self,
        _types: &[super::SemanticTypeDeclV1],
        _function: &super::SemanticFunctionDeclV1,
        _induction: &super::ProjectedUniformInductionV1,
        _entry_operations: &[super::ProductionRankedOperationV1],
    ) -> Result<(), ProjectionError> {
        Ok(())
    }

    fn masked_assertion_source_proved_v1(
        &mut self,
        _function: &super::SemanticFunctionDeclV1,
        _block: usize,
        _expected: bool,
        _successor: SemanticBlockIdV1,
    ) -> Result<bool, ProjectionError> {
        Ok(false)
    }

    fn with_nominal_call_v1(
        &mut self,
        _block: usize,
        _call: &super::SemanticDirectCallV1,
        _source: super::SemanticSourceProvenanceV1,
        _visit: &mut NominalCallVisitorV1<'_>,
    ) -> Result<(), ProjectionError> {
        Err(ProjectionError::Incomplete(
            "nominal call visitor requires a canonical source owner",
        ))
    }

    fn require_unit_local_call(
        &mut self,
        block: usize,
        call: &super::SemanticDirectCallV1,
        source: super::SemanticSourceProvenanceV1,
    ) -> Result<(), ProjectionError> {
        Err(ProjectionError::UnresolvedCallableEffect {
            block,
            source: Box::new(source),
            callee: call.callee().index(),
            tail: false,
        })
    }

    fn charge_private_array_work(&mut self, amount: usize) -> Result<(), ProjectionError>;

    fn helper_value_ledger_v1(
        &self,
    ) -> Result<
        (
            usize,
            fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
        ),
        ProjectionError,
    > {
        Err(ProjectionError::Incomplete(
            "helper value projection requires canonical ledger custody",
        ))
    }

    fn scalar_private_storage_v1(&self) -> Result<usize, ProjectionError> {
        Err(ProjectionError::Incomplete(
            "scalar private projection requires canonical storage custody",
        ))
    }
    fn reserve_scalar_private_storage_v1(&mut self, _amount: usize) -> Result<(), ProjectionError> {
        Err(ProjectionError::Incomplete(
            "scalar private projection requires canonical storage custody",
        ))
    }
    fn release_scalar_private_storage_v1(&mut self, _amount: usize) -> Result<(), ProjectionError> {
        Err(ProjectionError::Incomplete(
            "scalar private projection requires canonical storage custody",
        ))
    }

    fn private_array_initializer_count(
        &mut self,
        block: usize,
        statement: usize,
    ) -> Result<Option<u64>, ProjectionError>;

    fn private_array_access_index_v1(
        &mut self,
        _site: super::ProjectedSemanticAccessSiteV1,
        _role: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
    ) -> Result<Option<u64>, ProjectionError> {
        Err(ProjectionError::Incomplete(
            "private array read requires live canonical correspondence",
        ))
    }

    fn slice_access(
        &mut self,
        site: super::ProjectedSemanticAccessSiteV1,
        ordinal: u32,
        assertion: u32,
    ) -> Result<super::slice_projection_v1::ProjectedSliceInputV1, ProjectionError> {
        let _ = (site, ordinal, assertion);
        Err(ProjectionError::Incomplete(
            "slice access requires live canonical correspondence",
        ))
    }

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
    owner: &'g ProductionPreRankedKirOwnerV1,
    origins: SemanticKirAssertOriginsV1<'g>,
    report: &'r CanonicalKirSparseV1<'i, 'g>,
    budget: &'b mut Budget<'w>,
}
impl CanonicalAssertionSessionV1<'_, '_, '_, '_, '_> {
    #[cfg(test)]
    pub(super) fn callable_effect_summaries_with_query_budget_v1(
        &self,
        source: &RankedProjectionSourceV1<'_>,
        budget: &mut Budget<'_>,
    ) -> Result<super::DefinedCallableEmptyEffectSummariesV1, ProjectionError> {
        super::derive_materialized_callable_effect_summaries_v1(
            source,
            self.report.inventory(),
            budget,
        )
    }

    pub(super) fn callable_effect_summaries(
        &mut self,
        source: &RankedProjectionSourceV1<'_>,
    ) -> Result<super::DefinedCallableEmptyEffectSummariesV1, ProjectionError> {
        super::derive_materialized_callable_effect_summaries_v1(
            source,
            self.report.inventory(),
            self.budget,
        )
    }

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
            owner: self.owner,
            origins: self.origins,
            report: self.report,
            budget,
            correspondence_owner,
            semantic_function,
            masked: None,
        }
    }

    pub(super) fn for_source(
        &mut self,
        correspondence_owner: SemanticFunctionIdV1,
        semantic_function: SemanticFunctionIdV1,
    ) -> impl ProjectedAssertionFactsV1 + '_ {
        CanonicalSourceAssertionFactsV1 {
            owner: self.owner,
            origins: self.origins,
            report: self.report,
            budget: &mut *self.budget,
            correspondence_owner,
            semantic_function,
            masked: None,
        }
    }
}

pub(super) struct CanonicalSourceAssertionFactsV1<'r, 'i, 'g, 'b, 'w> {
    owner: &'g ProductionPreRankedKirOwnerV1,
    origins: SemanticKirAssertOriginsV1<'g>,
    report: &'r CanonicalKirSparseV1<'i, 'g>,
    budget: &'b mut Budget<'w>,
    correspondence_owner: SemanticFunctionIdV1,
    semantic_function: SemanticFunctionIdV1,
    masked: Option<&'r MaskedSourceAssertionTableV1<'g>>,
}
impl ProjectedAssertionFactsV1 for CanonicalSourceAssertionFactsV1<'_, '_, '_, '_, '_> {
    fn private_array_access_index_v1(
        &mut self,
        site: super::ProjectedSemanticAccessSiteV1,
        role: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
    ) -> Result<Option<u64>, ProjectionError> {
        self.budget.charge_work(4).map_err(resource)?;
        let block = u32::try_from(site.block).map_err(|_| resource(Resource::Arithmetic))?;
        let statement = site.statement.ok_or(ProjectionError::Incomplete(
            "private array access requires a statement",
        ))?;
        let statement = u32::try_from(statement).map_err(|_| resource(Resource::Arithmetic))?;
        self.owner
            .materialized_private_array_constant_index(
                self.correspondence_owner,
                self.semantic_function,
                fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1::Statement {
                    block: fe2o3_mir_model::SsaBlockIdV1::new(block),
                    statement,
                },
                role,
                self.budget,
            )
            .map_err(|error| reject(CanonicalAssertionErrorV1::PrivateArray(error)))
    }

    #[cfg(test)]
    fn observe_conditional_bound_for_test_v1(
        &mut self,
        bound: crate::production_reference_effect_join_v2::CompilerOwnedBoundReferenceEffectV2,
        source: super::conditional_bound_observation_v1_tests::BoundSourceV1<'_>,
    ) -> Result<(), ProjectionError> {
        super::conditional_bound_observation_v1_tests::observe_bound(
            self.owner,
            bound,
            source,
            self.budget,
        )
    }

    #[cfg(test)]
    fn observe_conditional_prepared_for_test_v1(
        &mut self,
        candidate: fe2o3_lower_mir_kernel::NativeRankedSourceCandidateV1<'_>,
    ) -> Result<(), ProjectionError> {
        super::conditional_output_observation_v1_tests::observe_candidate(
            self.owner,
            candidate,
            self.budget,
        )
    }

    fn masked_assertion_source_proved_v1(
        &mut self,
        function: &super::SemanticFunctionDeclV1,
        block: usize,
        expected: bool,
        successor: SemanticBlockIdV1,
    ) -> Result<bool, ProjectionError> {
        match self.masked {
            Some(table) => table.proves(
                self.owner.semantic_ssa().source_semantic(),
                self.semantic_function,
                function,
                block,
                expected,
                successor,
                self.budget,
            ),
            None => Ok(false),
        }
    }

    fn with_nominal_call_v1(
        &mut self,
        block: usize,
        call: &super::SemanticDirectCallV1,
        source: super::SemanticSourceProvenanceV1,
        visit: &mut NominalCallVisitorV1<'_>,
    ) -> Result<(), ProjectionError> {
        let source_block = u32::try_from(block).map_err(|_| resource(Resource::Arithmetic))?;
        let owner = self.owner;
        let caller = self.semantic_function;
        super::bf16_nominal_call_projection_v1::with_bf16_nominal_call_projection_v1(
            owner,
            self.report.inventory(),
            self.correspondence_owner,
            caller,
            SemanticBlockIdV1::from_index(source_block),
            call,
            self.budget,
            |candidate, budget| {
                budget.charge_work(2)?;
                let actual = owner
                    .semantic_ssa()
                    .source_semantic()
                    .functions()
                    .get(caller.index() as usize)
                    .and_then(|function| function.blocks().get(block))
                    .ok_or(
                        fe2o3_lower_mir_kernel::Bf16NominalCallQueryErrorV1::Unavailable(
                            "nominal facts source block absent",
                        ),
                    )?;
                if actual.terminator().source() != source {
                    return Err(
                        fe2o3_lower_mir_kernel::Bf16NominalCallQueryErrorV1::Unavailable(
                            "nominal facts source provenance differs",
                        ),
                    );
                }
                visit(candidate, budget)
            },
        )
        .map_err(super::bf16_nominal_call_routing_v1::query_error)
    }

    fn require_unit_local_call(
        &mut self,
        block: usize,
        call: &super::SemanticDirectCallV1,
        source: super::SemanticSourceProvenanceV1,
    ) -> Result<(), ProjectionError> {
        self.budget.charge_work(1).map_err(resource)?;
        let source_block = u32::try_from(block).map_err(|_| resource(Resource::Arithmetic))?;
        self.owner
            .with_checked_unit_local_source_v1(
                self.report.inventory(),
                self.budget,
                |view, budget| {
                    let checked = view.bounds_neutral_call_v1(
                        self.correspondence_owner,
                        self.semantic_function,
                        SemanticBlockIdV1::from_index(source_block),
                        call,
                        budget,
                    )?;
                    Ok(match checked {
                        Some(_checked) => Ok(()),
                        None => Err(()),
                    })
                },
            )
            .map_err(ProjectionError::StructuralValidation)?
            .map_err(|()| ProjectionError::UnresolvedCallableEffect {
                block,
                source: Box::new(source),
                callee: call.callee().index(),
                tail: false,
            })
    }

    fn charge_private_array_work(&mut self, amount: usize) -> Result<(), ProjectionError> {
        self.budget.charge_work(amount).map_err(resource)
    }

    fn scalar_private_storage_v1(&self) -> Result<usize, ProjectionError> {
        Ok(self.budget.storage())
    }
    fn helper_value_ledger_v1(
        &self,
    ) -> Result<
        (
            usize,
            fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
        ),
        ProjectionError,
    > {
        Ok((
            self.budget as *const Budget<'_> as usize,
            self.budget.work_ledger_identity_v1(),
        ))
    }
    fn reserve_scalar_private_storage_v1(&mut self, amount: usize) -> Result<(), ProjectionError> {
        self.budget.reserve_storage(amount).map_err(resource)
    }
    fn release_scalar_private_storage_v1(&mut self, amount: usize) -> Result<(), ProjectionError> {
        self.budget.release_storage(amount).map_err(resource)
    }

    fn private_array_initializer_count(
        &mut self,
        block: usize,
        statement: usize,
    ) -> Result<Option<u64>, ProjectionError> {
        self.budget.charge_work(3).map_err(resource)?;
        let block = u32::try_from(block).map_err(|_| resource(Resource::Arithmetic))?;
        let statement = u32::try_from(statement).map_err(|_| resource(Resource::Arithmetic))?;
        self.owner
            .materialized_private_array_initializer_count(
                self.correspondence_owner,
                self.semantic_function,
                fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1::Statement {
                    block: fe2o3_mir_model::SsaBlockIdV1::new(block),
                    statement,
                },
                self.budget,
            )
            .map_err(|error| reject(CanonicalAssertionErrorV1::PrivateArray(error)))
    }

    fn slice_access(
        &mut self,
        site: super::ProjectedSemanticAccessSiteV1,
        ordinal: u32,
        assertion: u32,
    ) -> Result<super::slice_projection_v1::ProjectedSliceInputV1, ProjectionError> {
        use fe2o3_kernel_ir::{AccessMode, AddressSpace, Type};
        let block = u32::try_from(site.block).map_err(|_| resource(Resource::Arithmetic))?;
        let statement = site
            .statement
            .map(u32::try_from)
            .transpose()
            .map_err(|_| resource(Resource::Arithmetic))?;
        let site = ProductionSliceAccessSiteV1::new(
            self.correspondence_owner,
            self.semantic_function,
            SemanticBlockIdV1::from_index(block),
            statement,
            ordinal,
            SemanticBlockIdV1::from_index(assertion),
        );
        self.owner
            .with_checked_slice_access_v1(self.report.inventory(), site, self.budget, |view| {
                let mismatch =
                    || fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1::CorrespondenceMismatch;
                let ProductionArgumentCoverageV1::Parameter(parameter) = view.source().coverage()
                else {
                    return Err(mismatch());
                };
                let Type::Slice(slice) = parameter.ty() else {
                    return Err(mismatch());
                };
                if slice.address_space != AddressSpace::Global
                    || slice.access != AccessMode::ReadOnly
                    || slice.element.as_ref() != view.loaded_type()
                {
                    return Err(mismatch());
                }
                let element_width = view
                    .loaded_type()
                    .as_scalar()
                    .and_then(|scalar| scalar.bit_width())
                    .map(|bits| u32::from(bits).max(8))
                    .ok_or_else(mismatch)?;
                let direct_local = view
                    .source()
                    .local_binding()
                    .filter(|(_, path)| path.is_empty() && view.source().source_path().is_empty())
                    .map(|(local, _)| local);
                Ok(super::slice_projection_v1::ProjectedSliceInputV1 {
                    source_argument: view.source().source_argument(),
                    direct_local,
                    element_width,
                })
            })
            .map_err(ProjectionError::StructuralValidation)
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

include!("canonical_masked_assertion_facts_v1.rs");

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
                    owner: source.owner(),
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

#[cfg(test)]
pub(crate) use bf16_nominal_recipe_resources_v1::observe_actual_root_guarded_accesses_for_test_v1;
#[cfg(test)]
pub(crate) use bf16_nominal_recipe_resources_v1::observe_actual_root_prefix_indices_for_test_v1;
