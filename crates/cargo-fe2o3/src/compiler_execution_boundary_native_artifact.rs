//! Original-account acquisition of the exact completed root's V5 publication.
use super::*;
use fe2o3_artifact_transaction::{
    ArtifactLockRetirementBarrierV1 as Barrier,
    CompilerModuleHandoffCustodyResourcesV5 as Resources,
    quote_compiler_module_handoff_currentness_custody_v5 as quote,
    try_acquire_artifact_lock_retirement_barrier_v1 as retirement,
    try_recover_compiler_module_handoff_receipt_in_root_budget_v5 as recover_original,
};

// Field order matters on every partial-construction and ordinary return path.
// The actual lock owners drop before the original process-retirement barrier.
pub(super) struct CurrentPublication {
    pub(super) lease: Lease,
    pub(super) token: Token,
    pub(super) retirement: Option<Barrier>,
}

pub(super) fn acquire_original(
    readiness: &mut ParentCompilerExecutionReadinessCustodyV3<'_, '_>,
    output: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
) -> Result<CurrentPublication> {
    readiness.origin.require_original_root()?;
    readiness.require_completion()?;
    readiness.revalidate()?;
    readiness.origin.require_output(output, readiness.budget)?;
    // Declared before all later lock owners; constructor refusal drops them first.
    let barrier = retirement()?;
    let (lease, token) =
        readiness
            .budget
            .with_prepaid_scope(readiness.retained_storage(), 0, 0, FRAME, |b| {
                let receipt = recover_original(
                    output,
                    producer,
                    attempt,
                    fe2o3_compiler_ffi::MAX_INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_BYTES_V5,
                    &barrier,
                    b,
                )?;
                if receipt.attempt() != attempt {
                    return Err(Failure::Mismatch(
                        "publication differs from the selected attempt",
                    ));
                }
                let quote = quote(output, producer, receipt)?;
                b.reserve_storage(
                    quote
                        .retained_storage()
                        .checked_add(size_of::<Resources>())
                        .ok_or(Resource::Arithmetic)?,
                )?;
                let mut resources = Resources::prepare(quote, b)?;
                let mut lease = None;
                let mut token = None;
                resources.with_acquisition(&barrier, b, |scope| {
                    let (owner, storage) = scope.acquire_lease(output, producer)?;
                    lease = Some(owner);
                    scope.reserve_retained(storage)?;
                    let (owner, storage) =
                        scope.acquire_token(lease.as_ref().ok_or(Resource::Accounting)?)?;
                    token = Some(owner);
                    scope.reserve_retained(storage)?;
                    scope.revalidate(
                        lease.as_ref().ok_or(Resource::Accounting)?,
                        token.as_ref().ok_or(Resource::Accounting)?,
                    )
                })?;
                Ok::<_, Failure>((
                    lease.ok_or(Resource::Accounting)?,
                    token.ok_or(Resource::Accounting)?,
                ))
            })?;
    let pair = CurrentPublication {
        lease,
        token,
        retirement: Some(barrier),
    };
    let storage = pair
        .lease
        .storage()
        .retained_storage()
        .checked_add(pair.token.storage().retained_storage())
        .ok_or(Resource::Arithmetic)?;
    readiness.budget.reserve_storage(storage)?;
    Ok(pair)
}
