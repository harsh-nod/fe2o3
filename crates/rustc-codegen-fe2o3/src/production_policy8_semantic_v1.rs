//! Actual retained P7 history and independently replayed J/K, not authority.
use super::*;
use crate::production_pipeline::checked_output_policy7_v1::semantic::{
    PortablePolicy7ClaimsV1, check_portable_history_v1,
};
use fe2o3_kernel_analysis::CanonicalKirLoadForwardingRowV1;
use fe2o3_kernel_ir::InertCanonicalKirTransitionGraphIdentityV1;
use fe2o3_kernel_opt::{
    CanonicalPolicy6ContinuationClaimsV1, CanonicalPolicy8ContinuationClaimsV1,
    CanonicalPolicy8SemanticErrorV1, CheckedCanonicalPolicy6ExecutionRelationV1,
    CheckedCanonicalPolicy7ContinuationRelationV1, CheckedCanonicalPolicy8ContinuationRelationV1,
    POLICY8_COMMUTATIVE_PASS_NAME_V1, check_canonical_policy8_continuation_relation_v1,
};

fn portable(error: CanonicalPolicy8SemanticErrorV1) -> ProductionPipelineError {
    super::error(CheckedOutputPolicy8StageErrorV1::Portable(Box::new(error)))
}

/// Authenticates one actual source-owned stage's P6/P7/J/K execution relation.
/// The private retained stage supplies J/K and all commutative rows. The inner
/// portable J/K relation remains semantic-only. No native/signed receipt or
/// artifact, publication, default or launch authority is issued.
pub(crate) struct AuthenticatedPolicy8ExecutionRelationV1<'a> {
    prefix: CheckedCanonicalPolicy6ExecutionRelationV1<'a>,
    policy7: CheckedCanonicalPolicy7ContinuationRelationV1<'a>,
    continuation: CheckedCanonicalPolicy8ContinuationRelationV1<'a, 'a, 'a>,
    actual: &'a PreparedPolicy8ArtifactsV1,
    retained: usize,
}

impl<'a> AuthenticatedPolicy8ExecutionRelationV1<'a> {
    pub(crate) const fn policy6_execution(
        &self,
    ) -> &CheckedCanonicalPolicy6ExecutionRelationV1<'a> {
        &self.prefix
    }
    pub(crate) const fn policy7_continuation(
        &self,
    ) -> &CheckedCanonicalPolicy7ContinuationRelationV1<'a> {
        &self.policy7
    }
    pub(crate) const fn continuation(
        &self,
    ) -> &CheckedCanonicalPolicy8ContinuationRelationV1<'a, 'a, 'a> {
        &self.continuation
    }
    pub(crate) const fn actual_stage(&self) -> &'a PreparedPolicy8ArtifactsV1 {
        self.actual
    }
    pub(crate) const fn retained_storage(&self) -> usize {
        self.retained
    }
    pub(crate) const fn authenticates_execution(&self) -> bool {
        true
    }
    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

impl Admitted8 {
    fn portable_tail(&self) -> &fe2o3_pliron::OwnedCommutativeBitwiseContinuationV1 {
        match self {
            Self::Direct(value) => value.continuation(),
            Self::Erased(value) => value.continuation(),
        }
    }
}

fn wrapper_storage() -> Result8<usize> {
    size_of::<AuthenticatedPolicy8ExecutionRelationV1<'_>>()
        .checked_sub(size_of::<CheckedCanonicalPolicy6ExecutionRelationV1<'_>>())
        .and_then(|n| n.checked_sub(size_of::<CheckedCanonicalPolicy7ContinuationRelationV1<'_>>()))
        .and_then(|n| {
            n.checked_sub(size_of::<
                CheckedCanonicalPolicy8ContinuationRelationV1<'_, '_, '_>,
            >())
        })
        .ok_or_else(|| resource(Resource::Arithmetic))
}

impl PreparedPolicy8ArtifactsV1 {
    /// Borrows the actual retained history once; no Prepared7, graph or row copy
    /// is constructed. Caller claim backing stays prepaid once or explicitly
    /// external. J/K and occurrence claims cannot be supplied by the caller.
    ///
    /// The inherited P7 sequence and complete source-to-K replay both remain
    /// mandatory, including their repeated source/formal checks and inherited
    /// metering exclusions. This is not globally deduplicated replay or complete
    /// allocator/RSS accounting. All new scratch uses the cumulative caller
    /// budget; the full surviving stage floor remains required throughout.
    /// Success returns only the added receipt unreserved, with entry storage
    /// restored. Reserve it before further work and release it after it drops.
    pub(crate) fn check_portable_execution_relation_v1<'a>(
        &'a self,
        policy4_wire: &'a [u8],
        policy5_record: &'a [u8],
        load_rows: &'a [CanonicalKirLoadForwardingRowV1],
        policy6: CanonicalPolicy6ContinuationClaimsV1<'a>,
        policy7_record: &'a [u8],
        budget: &mut Budget<'_>,
    ) -> Result8<AuthenticatedPolicy8ExecutionRelationV1<'a>> {
        scoped(self.retained_floor, budget, |budget| {
            let wrapper = wrapper_storage()?;
            budget.reserve_storage(wrapper).map_err(resource)?;
            let (prefix, policy7) = check_portable_history_v1(
                self.admitted.history(),
                &self.prefix_execution,
                PortablePolicy7ClaimsV1 {
                    policy4_wire,
                    policy5_record,
                    load_rows,
                    policy6,
                    policy7_record,
                },
                budget,
            )?;
            // This independently retains source-site/trap transport and fresh K
            // formal checks; a portable graph relation cannot replace them.
            self.admitted.replay(budget)?;
            let tail = self.admitted.portable_tail();
            let continuation = check_canonical_policy8_continuation_relation_v1(
                self.admitted.historical_j(),
                tail.output(),
                CanonicalPolicy8ContinuationClaimsV1 {
                    pass_name: POLICY8_COMMUTATIVE_PASS_NAME_V1,
                    input: InertCanonicalKirTransitionGraphIdentityV1::from_verified(
                        tail.input_identity(),
                    ),
                    output: InertCanonicalKirTransitionGraphIdentityV1::from_verified(
                        tail.output().canonical().identity(),
                    ),
                    occurrences: tail.occurrences().candidate(),
                },
                budget,
            )
            .map_err(portable)?;
            let continuation_storage = continuation.storage().retained_storage();
            budget
                .reserve_storage(continuation_storage)
                .map_err(resource)?;
            budget.charge_work(3).map_err(resource)?;
            if continuation.proved_pairs() != tail.proved_pairs()
                || continuation.has_substitutions() != tail.execution().changed()
            {
                return Err(execution_error(
                    "exact actual Policy8 continuation observation",
                ));
            }
            let retained = wrapper
                .checked_add(prefix.storage().retained_storage())
                .and_then(|n| n.checked_add(policy7.storage().retained_storage()))
                .and_then(|n| n.checked_add(continuation_storage))
                .ok_or_else(|| resource(Resource::Arithmetic))?;
            Ok(AuthenticatedPolicy8ExecutionRelationV1 {
                prefix,
                policy7,
                continuation,
                actual: self,
                retained,
            })
        })
    }
}

#[cfg(test)]
#[path = "production_policy8_semantic_resources_v1_tests.rs"]
mod resource_tests;
#[cfg(test)]
#[path = "production_policy8_semantic_v1_tests.rs"]
pub(crate) mod tests;
