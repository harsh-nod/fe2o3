//! Actual-stage authentication of borrowed portable claims, not publication.
use super::*;
use fe2o3_kernel_analysis::CanonicalKirLoadForwardingRowV1;
use fe2o3_kernel_opt::{
    CanonicalPolicy6ContinuationClaimsV1, CanonicalPolicy7ContinuationClaimsV1,
    CanonicalPolicy7SemanticErrorV1, CheckedCanonicalPolicy6ExecutionRelationV1,
    CheckedCanonicalPolicy7ContinuationRelationV1, check_canonical_policy6_execution_relation_v1,
    check_canonical_policy7_continuation_relation_v1,
};

fn portable(error: CanonicalPolicy7SemanticErrorV1) -> ProductionPipelineError {
    super::error(CheckedOutputPolicy7StageErrorV1::Portable(Box::new(error)))
}

/// One portable authenticated P6 receipt and one independent I/J relation,
/// borrowing the actual source-owned stage. This does not replace Native7's
/// mandatory signed-source gate or issue artifact/publication authority.
pub(crate) struct AuthenticatedPolicy7ExecutionRelationV1<'a> {
    prefix: CheckedCanonicalPolicy6ExecutionRelationV1<'a>,
    continuation: CheckedCanonicalPolicy7ContinuationRelationV1<'a>,
    actual: &'a PreparedPolicy7ArtifactsV1,
    retained: usize,
}
impl<'a> AuthenticatedPolicy7ExecutionRelationV1<'a> {
    pub(crate) const fn policy6_execution(
        &self,
    ) -> &CheckedCanonicalPolicy6ExecutionRelationV1<'a> {
        &self.prefix
    }
    pub(crate) const fn continuation(&self) -> &CheckedCanonicalPolicy7ContinuationRelationV1<'a> {
        &self.continuation
    }
    pub(crate) const fn actual_stage(&self) -> &'a PreparedPolicy7ArtifactsV1 {
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

impl Admitted7 {
    fn portable_prefix(
        &self,
    ) -> (
        &Graph,
        &fe2o3_kernel_opt::CheckedCanonicalKernelIrOwnerPolicy6V1,
    ) {
        self.borrowed().portable_prefix()
    }
}

impl<'a> Admitted7Ref<'a> {
    pub(in crate::production_pipeline) fn portable_prefix(
        self,
    ) -> (
        &'a Graph,
        &'a fe2o3_kernel_opt::CheckedCanonicalKernelIrOwnerPolicy6V1,
    ) {
        match self {
            Self::Direct(value) => (value.prefix().bound(), value.prefix().checked_output()),
            Self::Erased(value) => (value.prefix().bound(), value.prefix().checked_output()),
        }
    }

    fn portable_replay(self, budget: &mut Budget<'_>) -> Result7<()> {
        match self {
            Self::Direct(value) => value.verify_equivalence(budget),
            Self::Erased(value) => value.verify_equivalence(budget),
        }
        .map_err(admission)
    }
}

/// Borrowed caller claims, not another history owner or retained allocation.
#[derive(Clone, Copy)]
pub(in crate::production_pipeline) struct PortablePolicy7ClaimsV1<'a> {
    pub policy4_wire: &'a [u8],
    pub policy5_record: &'a [u8],
    pub load_rows: &'a [CanonicalKirLoadForwardingRowV1],
    pub policy6: CanonicalPolicy6ContinuationClaimsV1<'a>,
    pub policy7_record: &'a [u8],
}

type PortablePolicy7PartsV1<'a> = (
    CheckedCanonicalPolicy6ExecutionRelationV1<'a>,
    CheckedCanonicalPolicy7ContinuationRelationV1<'a>,
);

/// Run inside the caller's existing scope after its own wrapper prepayment.
/// Both returned component headers remain reserved until that scope ends.
pub(in crate::production_pipeline) fn check_portable_history_v1<'a>(
    actual: Admitted7Ref<'a>,
    execution: &'a Policy7ExecutionWitnessV1,
    claims: PortablePolicy7ClaimsV1<'a>,
    budget: &mut Budget<'_>,
) -> Result7<PortablePolicy7PartsV1<'a>> {
    actual.portable_replay(budget)?;
    // Existing witness replay constructs a fixed header in addition to the
    // actual retained record; preserve its historical scratch and charge order.
    budget
        .reserve_storage(fe2o3_kernel_opt::POLICY7_EXECUTION_HEADER_BYTES_V1)
        .map_err(resource)?;
    let checked_record = execution.check_history_v1(actual, budget);
    budget
        .release_storage(fe2o3_kernel_opt::POLICY7_EXECUTION_HEADER_BYTES_V1)
        .map_err(resource)?;
    checked_record?;
    let (bound, checked) = actual.portable_prefix();
    let prefix = check_canonical_policy6_execution_relation_v1(
        bound,
        checked,
        claims.policy4_wire,
        claims.policy5_record,
        claims.load_rows,
        claims.policy6,
        budget,
    )
    .map_err(|e| portable(CanonicalPolicy7SemanticErrorV1::Policy6(Box::new(e))))?;
    let prefix_storage = prefix.storage().retained_storage();
    budget.reserve_storage(prefix_storage).map_err(resource)?;
    let continuation = actual.continuation();
    let continuation = check_canonical_policy7_continuation_relation_v1(
        prefix.authenticated_composition_record(),
        actual.input(),
        actual.output(),
        CanonicalPolicy7ContinuationClaimsV1 {
            execution_record: claims.policy7_record,
            deletion_rows: continuation.rows(),
            retained_operations: continuation.retained_operations(),
        },
        budget,
    )
    .map_err(portable)?;
    let continuation_storage = continuation.storage().retained_storage();
    budget
        .reserve_storage(continuation_storage)
        .map_err(resource)?;
    budget
        .charge_work(
            claims
                .policy7_record
                .len()
                .checked_add(execution.canonical_bytes().len())
                .ok_or_else(|| resource(Resource::Arithmetic))?,
        )
        .map_err(resource)?;
    if claims.policy7_record != execution.canonical_bytes() {
        return Err(execution_error("exact portable Policy7 execution record"));
    }
    Ok((prefix, continuation))
}

impl PreparedPolicy7ArtifactsV1 {
    /// Retains the actual stage borrow and validates exactly one portable P6
    /// prefix plus complete I/J rows. Existing source replay still checks the
    /// sealed prefix and fresh J reports; that charged work is not skipped or
    /// claimed to be globally deduplicated. Caller backing remains prepaid once
    /// or explicitly external. Success returns its added receipt unreserved.
    pub(crate) fn check_portable_execution_relation_v1<'a>(
        &'a self,
        policy4_wire: &'a [u8],
        policy5_record: &'a [u8],
        load_rows: &'a [CanonicalKirLoadForwardingRowV1],
        policy6: CanonicalPolicy6ContinuationClaimsV1<'a>,
        policy7_record: &'a [u8],
        budget: &mut Budget<'_>,
    ) -> Result7<AuthenticatedPolicy7ExecutionRelationV1<'a>> {
        scoped(self.retained_floor, budget, |budget| {
            let wrapper = size_of::<AuthenticatedPolicy7ExecutionRelationV1<'_>>()
                .checked_sub(size_of::<CheckedCanonicalPolicy6ExecutionRelationV1<'_>>())
                .and_then(|n| {
                    n.checked_sub(size_of::<CheckedCanonicalPolicy7ContinuationRelationV1<'_>>())
                })
                .ok_or_else(|| resource(Resource::Arithmetic))?;
            budget.reserve_storage(wrapper).map_err(resource)?;
            let (prefix, continuation) = check_portable_history_v1(
                self.admitted.borrowed(),
                &self.execution,
                PortablePolicy7ClaimsV1 {
                    policy4_wire,
                    policy5_record,
                    load_rows,
                    policy6,
                    policy7_record,
                },
                budget,
            )?;
            let prefix_storage = prefix.storage().retained_storage();
            let continuation_storage = continuation.storage().retained_storage();
            let retained = wrapper
                .checked_add(prefix_storage)
                .and_then(|n| n.checked_add(continuation_storage))
                .ok_or_else(|| resource(Resource::Arithmetic))?;
            Ok(AuthenticatedPolicy7ExecutionRelationV1 {
                prefix,
                continuation,
                actual: self,
                retained,
            })
        })
    }
}

#[cfg(test)]
#[path = "production_policy7_semantic_v1_tests.rs"]
pub(crate) mod tests;
