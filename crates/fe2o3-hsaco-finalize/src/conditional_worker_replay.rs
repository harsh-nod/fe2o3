//! Independent conditional Worker/artifact replay, never fresh process custody.
use crate::{
    ConditionalWorkerCompactFinalizerReplayV5 as Transcript,
    InertConditionalFirstBuildWorkerEvidenceV2 as Evidence, MAX_WORKER_OUTPUT_BYTES,
    NativeWorkerReplayErrorV1 as Error, NativeWorkerReplayStorageV1 as Storage,
    PreparedFinalizedConditionalWorkerHsacoV5 as Artifact, WorkerOutputConstraintsV1,
    conditional_worker_finalization::{
        finalize_conditional_worker_hsaco_v5 as finalize, reconstruct_artifact,
    },
    first_build_worker_conditional::{
        ConditionalWorkerReplaySource, recover_prepaid_conditional_worker_evidence_v2 as recover,
    },
    first_build_worker_conditional_binding::{
        AccountMode, ProtectedCompilerConditionalHandoffBindingV2 as Binding, replay_input_storage,
        storage_floor,
    },
    first_build_worker_v3::enforce_worker_working_set_budget,
    native_worker_replay::{failure, prepare_providers},
    native_worker_replay_resources::{
        NativeWorkerReplayResourceQuote as Quote, NativeWorkerReplayResponseInputs,
    },
    request_construction::decode_compiler_module_handoff_v2,
    worker_finalizer_replay_engine::{ReconstructedWorkerExchanges, reconstruct_worker_exchanges},
};
use fe2o3_artifact_transaction::{
    BuildAttempt, CompilerModuleHandoffErrorV5, ProducerIdentity,
    rederive_compiler_module_handoff_receipt_for_replay_v5 as rederive,
    rederive_compiler_module_handoff_receipt_in_original_account_v5 as rederive_original,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_verifier::RecoveredCompilerConditionalNativeSemanticHandoffV5 as Source;
use std::mem::size_of;

const FRAME: usize = 2 * size_of::<ConditionalWorkerReplaySource>()
    + 2 * size_of::<Binding>()
    + size_of::<ReconstructedWorkerExchanges>()
    + size_of::<Evidence>()
    + size_of::<Quote>()
    + size_of::<Error>()
    + 4096;

/// Reconstruct both canonical Worker exchanges and rerun V5 finalization while
/// retaining the actual independently recovered source/final graph/catalog.
/// The caller must first recover `source` against independently admitted policy,
/// outside any refund scope around that terminal admission. No source is forged
/// from the transcript, and no Worker process or filesystem transaction starts.
///
/// Prepay complete source backing/metadata/recovery, transcript storage and the
/// borrowed finalized bytes on this original account. Returned storage is an
/// additional unreserved charge. Provider construction and Worker reconstruction
/// are charged here; artifact parsing/payload allocations keep their separately
/// bounded finalizer domain. This is not an RSS or caller-spare-capacity bound.
///
/// The returned source has `RecoveredTranscript` custody, not fresh consumption
/// or currentness. Deterministic replay proves neither protected origin nor
/// machine refinement and grants no publication, load or launch authority.
/// ```compile_fail
/// use fe2o3_hsaco_finalize::PreparedFinalizedConditionalWorkerHsacoV5 as A;
/// use fe2o3_artifact_transaction::ConsumedCompilerModuleHandoffV5 as C;
/// fn promote(artifact: A) -> C { artifact.into() }
/// ```
#[allow(clippy::too_many_arguments)]
pub fn revalidate_conditional_worker_finalizer_v5(
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    source: Source,
    transcript: &Transcript,
    provider_payloads: Vec<Vec<u8>>,
    exact_finalized_hsaco: &[u8],
    b: &mut Budget<'_>,
) -> Result<(Artifact, Storage), Error> {
    replay_using(
        producer,
        attempt,
        source,
        transcript,
        provider_payloads,
        exact_finalized_hsaco,
        b,
        AccountMode::LEGACY,
    )
}

/// Same canonical reconstruction on an original owned account with the actual
/// complete inputs charged inside a non-widening local window. The returned
/// evidence retains the original accounting association, not fresh authority.
#[allow(clippy::too_many_arguments)]
pub fn revalidate_conditional_worker_finalizer_in_original_account_v5(
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    source: Source,
    transcript: &Transcript,
    provider_payloads: Vec<Vec<u8>>,
    exact_finalized_hsaco: &[u8],
    b: &mut Budget<'_>,
) -> Result<(Artifact, Storage), Error> {
    let account = AccountMode::original(b)?;
    replay_using(
        producer,
        attempt,
        source,
        transcript,
        provider_payloads,
        exact_finalized_hsaco,
        b,
        account,
    )
}

#[allow(clippy::too_many_arguments)]
fn replay_using(
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    source: Source,
    transcript: &Transcript,
    provider_payloads: Vec<Vec<u8>>,
    exact_finalized_hsaco: &[u8],
    b: &mut Budget<'_>,
    account: AccountMode,
) -> Result<(Artifact, Storage), Error> {
    let floor = storage_floor(&source)?
        .checked_add(transcript.storage().retained_storage())
        .and_then(|n| n.checked_add(exact_finalized_hsaco.len()))
        .ok_or(Resource::Arithmetic)?;
    let inputs = if account.is_original() {
        floor
            .checked_add(replay_input_storage(&provider_payloads, b)?)
            .ok_or(Resource::Arithmetic)?
    } else {
        0
    };
    account.run(b, inputs, |b| {
        b.with_prepaid_scope(floor, 8, 4096, FRAME, |b| {
            let coordinates = transcript.coordinates();
            if coordinates.attempt() != attempt {
                return Err(Error::Mismatch("conditional attempt"));
            }
            transcript
                .verify_outer_identity(source.handoff().identity())
                .map_err(|e| failure("conditional outer identity", e))?;
            let receipt = (if account.is_original() {
                rederive_original
            } else {
                rederive
            })(
                producer,
                attempt,
                coordinates.slot(),
                coordinates.transaction_identity(),
                source.handoff(),
                b,
            )
            .map_err(|e| match e {
                CompilerModuleHandoffErrorV5::Resource(e) => Error::Resource(e),
                other => failure("conditional occurrence", other),
            })?;
            let binding = Binding::from_handoff_using(
                &source,
                receipt,
                *source.handoff().capsule().invocation().compiler_closure(),
                b,
                account,
            )?;
            if binding.identity() != transcript.binding_identity() {
                return Err(Error::Mismatch("conditional Worker binding"));
            }
            let replay = transcript.replay_view();
            let providers = prepare_providers(&replay, provider_payloads, b)?;
            enforce_worker_working_set_budget(
                source.handoff().canonical_bytes().len(),
                source.handoff().module_handoff(),
                &providers,
                replay.link_options,
            )
            .map_err(|e| failure("conditional working set", e))?;
            if exact_finalized_hsaco.is_empty()
                || exact_finalized_hsaco.len() > MAX_WORKER_OUTPUT_BYTES
            {
                return Err(Error::Mismatch("conditional artifact byte bound"));
            }
            let raw = reconstruct_artifact(exact_finalized_hsaco, b)?;
            let output = WorkerOutputConstraintsV1::new(replay.bootstrap_output_bound)
                .map_err(|e| failure("conditional output bound", e))?;
            let quote = Quote::new(
                source.handoff().module_handoff(),
                &providers,
                replay.link_options,
                &output,
                replay.execution_limits,
                NativeWorkerReplayResponseInputs {
                    raw_output_bytes: raw.len(),
                    worker_build_identity_bytes: replay.worker.worker_build_identity().len(),
                    bootstrap_metadata: replay.bootstrap_metadata,
                    replay_metadata: replay.replay_metadata,
                },
            )
            .and_then(Quote::for_evidence::<Evidence>)
            .map_err(|e| failure("conditional resource quote", e))?;
            let (evidence, worker_storage) = b.with_prepaid_scope(
                b.storage(),
                0,
                quote.reconstruction_work,
                quote.reconstruction_storage,
                |_| {
                    let decoded = decode_compiler_module_handoff_v2(
                        source.handoff().module_handoff().canonical_bytes(),
                    )
                    .map_err(|e| failure("conditional module decode", e))?;
                    let exchanges = reconstruct_worker_exchanges(
                        (&binding).into(),
                        &decoded,
                        providers,
                        &replay,
                        &raw,
                    )
                    .map_err(|e| failure("conditional Worker reconstruction", e))?;
                    recover(
                        ConditionalWorkerReplaySource {
                            source,
                            binding,
                            worker: replay.worker.clone(),
                            limits: replay.execution_limits,
                            account,
                        },
                        &decoded,
                        exchanges,
                        &quote.first_build,
                    )
                    .map_err(Error::from)
                },
            )?;
            b.reserve_storage(worker_storage.retained_storage())?;
            if evidence.identity() != transcript.source_evidence_identity() {
                return Err(Error::Mismatch("conditional source evidence"));
            }
            let (artifact, artifact_storage) = finalize(evidence, b)?;
            b.reserve_storage(artifact_storage.retained_storage())?;
            if artifact.identity() != transcript.expected_finalization_identity() {
                return Err(Error::Mismatch("conditional finalization identity"));
            }
            b.charge_work(exact_finalized_hsaco.len())?;
            if artifact.finalized().as_bytes() != exact_finalized_hsaco {
                return Err(Error::Mismatch("conditional finalized artifact bytes"));
            }
            let storage = Storage(
                worker_storage
                    .retained_storage()
                    .checked_add(artifact_storage.retained_storage())
                    .ok_or(Resource::Arithmetic)?,
            );
            Ok((artifact, storage))
        })
    })
}
