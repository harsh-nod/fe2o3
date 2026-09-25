//! Private conditional N-through-J custody at the production F entry only.
//! No ordinary receipt, final F, native artifact or launch authority is created.
use super::{
    Budget, DirectPolicy6PreparationV1, Owner, ProductionPipelineError, Profile,
    RankedVerifiedProductionCompilation, Resource, resource,
};
use crate::production_pipeline::checked_output_policy7_v1::scoped;
use crate::production_ranked_projection_v1::ProductionRankedVerificationErrorV1 as RankedError;
use crate::production_reference_effect_join_v2::ProductionReferenceEffectJoinErrorV2 as JoinError;
use fe2o3_kernel_opt::{
    CheckedCanonicalKernelIrOwnerPolicy6V1 as Prefix, OwnedRedundantStoreContinuationV1 as Tail,
    prepare_owned_redundant_store_continuation_v1,
};
use fe2o3_lower_mir_kernel::{
    ProductionConditionalCheckedTailErrorV1, ProductionHelperSourcePolicyV1,
    ProductionSourceBoundConditionalAggregateRequestV1 as Request,
};
use std::{
    fmt,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
};

/// Constructed only after genuine replay, contract retention and both original
/// account postchecks. Fields never escape separately or become ordinary V5.
pub(in crate::production_pipeline) struct ConditionalPrefixForFV1 {
    preparation: DirectPolicy6PreparationV1,
    tail: Tail,
    retained_floor: usize,
}

impl RankedVerifiedProductionCompilation {
    pub(in crate::production_pipeline) fn prepare_conditional_prefix_for_f_v1(
        self,
        budget: &mut Budget<'_>,
    ) -> Result<ConditionalPrefixForFV1, ProductionPipelineError> {
        if !self.ranked.has_conditional_roots_v1()
            || self.ranked.materialized().helper_source_policy_v1()
                != ProductionHelperSourcePolicyV1::RawEmpty
        {
            return Err(ProductionPipelineError::RankedVerification(
                RankedError::RosterMetadata("conditional F prefix requires original Direct roots"),
            ));
        }
        let floor = budget.storage();
        let (preparation, tail, retained) = scoped(floor, budget, move |budget| {
            let DirectPolicy6PreparationV1 {
                ranked,
                bindings,
                bound,
                checked,
            } = self.prepare_direct_policy6_v1(budget)?;
            // Preserve B and the single I prefix while the existing checked
            // rewrite detaches its actual J. Its returned receipt is unreserved.
            let tail = prepare_owned_redundant_store_continuation_v1(checked.owner(), budget)
                .map_err(join_error)?;
            budget
                .reserve_storage(tail.retained_storage())
                .map_err(resource)?;
            // Conservatively include the wrapper; source/bindings retain their
            // inherited bounded domains, not a claim of exact process heap use.
            budget
                .reserve_storage(size_of::<ConditionalPrefixForFV1>())
                .map_err(resource)?;
            let ranked = crate::production_pipeline::conditional_generated_fields_v1::replay_conditional_prefix_for_f_v1(
                ranked, &bindings, &bound, &checked, &tail, budget,
            )
            .map_err(ProductionPipelineError::RankedVerification)?;
            let retained = budget
                .storage()
                .checked_sub(floor)
                .ok_or_else(|| resource(Resource::Accounting))?;
            Ok((
                DirectPolicy6PreparationV1 {
                    ranked,
                    bindings,
                    bound,
                    checked,
                },
                tail,
                retained,
            ))
        })?;
        // The enclosing target postcheck must also finish before private custody
        // is installed. Re-reserve the transferred receipt on that same account.
        budget.reserve_storage(retained).map_err(resource)?;
        let value = ConditionalPrefixForFV1 {
            preparation,
            tail,
            retained_floor: budget.storage(),
        };
        #[cfg(test)]
        tests::installed(&value);
        Ok(value)
    }
}

impl ConditionalPrefixForFV1 {
    pub(in crate::production_pipeline) fn into_finalizer_error_v1(
        self,
        budget: &mut Budget<'_>,
    ) -> ProductionPipelineError {
        if budget.storage() < self.retained_floor {
            return resource(Resource::Accounting);
        }
        let Self {
            preparation,
            tail: _tail,
            ..
        } = self;
        let DirectPolicy6PreparationV1 {
            ranked,
            bindings: _bindings,
            bound: _bound,
            checked: _checked,
        } = preparation;
        // Consume the actual original ranked owner at its unchanged gate while
        // keeping the graph history and collector bindings alive beside it.
        match ranked.into_verified_roster_receipt() {
            Err(error) => ProductionPipelineError::RankedVerification(error),
            Ok(_) => ProductionPipelineError::RankedVerification(RankedError::RosterMetadata(
                "conditional F prefix cannot yield an ordinary receipt",
            )),
        }
    }
}

fn join_error(error: impl fmt::Display) -> ProductionPipelineError {
    ProductionPipelineError::RankedVerification(RankedError::ConditionalReplay(
        JoinError::ProofExecution(error.to_string()),
    ))
}

#[derive(Debug)]
pub(in crate::production_pipeline) enum Error {
    Resource(Resource),
    Target(dialect_amdgcn::ProductionTargetCoordinateErrorV1),
    Agreement(ProductionConditionalCheckedTailErrorV1),
}
impl From<Resource> for Error {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(out),
            Self::Target(error) => error.fmt(out),
            Self::Agreement(error) => error.fmt(out),
        }
    }
}

pub(in crate::production_pipeline) fn check(
    request: &Request<'_>,
    bound: &Owner,
    checked: &Prefix,
    tail: &Tail,
    profile: Profile,
    target: &mut Budget<'_>,
    source: &mut Budget<'_>,
) -> Result<(), Error> {
    let result = scoped_target(target, |target| {
        target.reserve_storage(size_of::<Error>() + 256)?;
        let (coordinates, storage) =
            dialect_amdgcn::check_production_target_coordinate_preservation_v1(
                request.source().executable(),
                bound,
                profile,
                target,
            )
            .map_err(Error::Target)?;
        target.reserve_storage(storage.retained_storage())?;
        // This lower method calls the existing N-to-I checker exactly once,
        // followed by independent J relation/coverage/occurrence/premise checks.
        request
            .check_policy6_redundant_store_output_v1(&coordinates, checked, tail, target, source)
            .map_err(Error::Agreement)
    });
    #[cfg(test)]
    if result.is_ok() {
        tests::agreement_checked(profile);
    }
    result
}

fn scoped_target<'w>(
    target: &mut Budget<'w>,
    run: impl FnOnce(&mut Budget<'w>) -> Result<(), Error>,
) -> Result<(), Error> {
    let floor = target.storage();
    let account = target.work_ledger_identity_v1();
    let address = std::ptr::from_mut(target);
    let result = catch_unwind(AssertUnwindSafe(|| run(target)));
    let cleanup =
        if std::ptr::from_mut(target) != address || target.work_ledger_identity_v1() != account {
            Err(Resource::Accounting)
        } else {
            target
                .storage()
                .checked_sub(floor)
                .ok_or(Resource::Accounting)
                .and_then(|delta| target.release_storage(delta))
        };
    match result {
        Ok(result) => {
            cleanup?;
            result
        }
        Err(payload) => {
            let _ = cleanup;
            resume_unwind(payload)
        }
    }
}

#[cfg(test)]
#[path = "production_pipeline_conditional_prefix_v1_tests.rs"]
mod tests;
