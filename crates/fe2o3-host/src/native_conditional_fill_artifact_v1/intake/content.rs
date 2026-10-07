//! Real source recovery and exact finalizer replay, without publication promotion.
use super::*;
use fe2o3_artifact_transaction::{
    InertCompilerExecutionSubjectV3 as Subject, WorkerV3ExternalProviderPayloadsV1 as Providers,
};
use fe2o3_compiler_ffi::{
    INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_DECODE_METADATA_STORAGE_V5 as METADATA,
    InertSemanticCompilerModuleHandoffV5 as Handoff,
    inert_semantic_compiler_module_handoff_decode_work_v5 as decode_work,
};
use fe2o3_hsaco_finalize::{
    derive_recovered_conditional_worker_publication_intent_in_original_account_v5 as derive_intent,
    revalidate_conditional_worker_finalizer_in_original_account_v5 as replay,
};
use fe2o3_verifier::recover_native_conditional_handoff_under_policy_file_v1 as recover_source;

#[allow(clippy::too_many_arguments)]
pub(super) fn recover<'work>(
    wire: &Wire<'_>,
    record: &Record,
    claim: &Claim,
    publication: &Publication<'work>,
    producer: &ProducerIdentity,
    profile: &Profile<'work>,
    budget: &mut Budget<'work>,
) -> Result<(Finalized, Transcript, Carriage)> {
    exact(
        wire.outer_handoff_bytes(),
        record.outer_handoff_sha256(),
        record.outer_handoff_length(),
        budget,
    )?;
    exact(
        wire.transcript_bytes(),
        record.transcript_sha256(),
        record.transcript_length(),
        budget,
    )?;
    exact(
        publication.exact_artifact_bytes(),
        record.output_sha256(),
        record.output_length(),
        budget,
    )?;

    budget.charge_work(
        decode_work(wire.outer_handoff_bytes().len())
            .map_err(|error| failure(format_args!("native V5 decode bound: {error:?}")))?,
    )?;
    budget.reserve_storage(METADATA)?;
    let bytes = copy(wire.outer_handoff_bytes(), budget)?;
    let handoff = Handoff::decode_owned(bytes)
        .map_err(|error| failure(format_args!("native V5 handoff: {error:?}")))?;
    let (source, charge) =
        recover_source(profile.semantic_policy_bytes(), handoff, budget).map_err(failure)?;
    budget.reserve_storage(charge.retained_storage())?;

    let (transcript, charge) =
        Transcript::decode_canonical(wire.transcript_bytes(), budget).map_err(failure)?;
    budget.reserve_storage(charge.retained_storage())?;
    let (carriage, charge) =
        Carriage::decode_in_original_account_v3(wire.compiler_execution_bytes(), budget)
            .map_err(failure)?;
    budget.reserve_storage(charge.additional_storage())?;
    require(
        *carriage.policy().identity().as_bytes()
            == profile.configuration().parts().compiler_policy_identity,
        "native independently pinned compiler policy",
    )?;

    let count = wire.providers().len();
    budget.reserve_storage(
        count
            .checked_mul(size_of::<Vec<u8>>())
            .and_then(|n| n.checked_add(size_of::<Providers>() + size_of::<Vec<Vec<u8>>>()))
            .ok_or(Resource::Arithmetic)?,
    )?;
    let mut providers = Vec::new();
    providers.try_reserve_exact(count).map_err(failure)?;
    require(providers.capacity() == count, "native provider capacity")?;
    for bytes in wire.providers() {
        providers.push(copy(bytes, budget)?);
    }
    let providers = Providers::new(providers).map_err(failure)?;
    require(
        providers.len() == record.external_provider_count()
            && providers.payload_length() == record.external_provider_payload_length()
            && providers.canonical_length() == record.external_provider_archive_length()
            && providers.canonical_sha256() == record.external_provider_archive_sha256(),
        "native exact provider archive",
    )?;
    let (finalized, charge) = replay(
        producer,
        record.attempt(),
        source,
        &transcript,
        providers.into_payloads(),
        publication.exact_artifact_bytes(),
        budget,
    )
    .map_err(failure)?;
    budget.reserve_storage(charge.retained_storage())?;

    // This adapter only derives the inert plan from actual recovered custody.
    // It cannot rebrand the source as fresh consumption or publish a transaction.
    let (intent, charge) =
        derive_intent(producer, &finalized, &transcript, budget).map_err(failure)?;
    budget.reserve_storage(charge.retained_storage())?;
    require(
        intent.durable_plan() == record.plan() && intent.durable_plan() == claim.plan(),
        "native exact replay-derived publication plan",
    )?;
    budget.release_storage(charge.retained_storage())?;
    let source = finalized.source();
    let (subject, charge) = Subject::from_publication_in_original_account_v3(
        source.binding().receipt(),
        source.recovered_handoff().handoff(),
        budget,
    )
    .map_err(failure)?;
    budget.reserve_storage(charge.retained_storage())?;
    budget.charge_work(subject.canonical_bytes().len())?;
    require(
        subject.canonical_bytes() == carriage.request().subject().canonical_bytes(),
        "native actual V5 compiler-execution subject",
    )?;
    drop(subject);
    budget.release_storage(charge.retained_storage())?;
    Ok((finalized, transcript, carriage))
}

fn exact(bytes: &[u8], sha: [u8; 32], length: usize, budget: &mut Budget<'_>) -> Result<()> {
    budget.charge_work(bytes.len().checked_add(64).ok_or(Resource::Arithmetic)?)?;
    require(
        bytes.len() == length && <[u8; 32]>::from(Sha256::digest(bytes)) == sha,
        "native exact readiness component",
    )
}

fn copy(bytes: &[u8], budget: &mut Budget<'_>) -> Result<Vec<u8>> {
    budget.charge_work(bytes.len().checked_mul(4).ok_or(Resource::Arithmetic)?)?;
    budget.reserve_storage(
        bytes
            .len()
            .checked_add(size_of::<Vec<u8>>())
            .ok_or(Resource::Arithmetic)?,
    )?;
    let mut owned = Vec::new();
    owned.try_reserve_exact(bytes.len()).map_err(failure)?;
    require(owned.capacity() == bytes.len(), "native component capacity")?;
    owned.extend_from_slice(bytes);
    Ok(owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

    #[test]
    fn exact_component_rejects_independent_length_and_digest_substitution() {
        let bytes = b"exact original component";
        let hash: [u8; 32] = Sha256::digest(bytes).into();
        for (sha, length, ok) in [
            (hash, bytes.len(), true),
            ([0; 32], bytes.len(), false),
            (hash, bytes.len() - 1, false),
        ] {
            let mut work = Work::new(usize::MAX);
            let mut budget = Budget::new(&mut work, 4096);
            assert_eq!(exact(bytes, sha, length, &mut budget).is_ok(), ok);
            assert_eq!(budget.storage(), 0);
            assert_eq!(budget.work(), bytes.len() + 64);
        }
        let mut work = Work::new(0);
        let mut budget = Budget::new(&mut work, 4096);
        assert!(matches!(
            exact(bytes, hash, bytes.len(), &mut budget),
            Err(Error::Resource(_))
        ));
    }
}
