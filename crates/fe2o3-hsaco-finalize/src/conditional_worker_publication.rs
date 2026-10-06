//! Conditional source retention over the existing opaque artifact journal.
use super::*;
use crate::{
    ConditionalWorkerCompactFinalizerReplayV5 as Transcript,
    NativeWorkerEvidenceCustodyV1 as Custody,
    PreparedFinalizedConditionalWorkerHsacoV5 as Artifact,
    first_build_worker_v3::extract_worker_v3_request_replay_parts_v1 as extract,
    revalidate_conditional_worker_finalizer_v5 as replay,
};
use fe2o3_compiler_ffi::{
    INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_DECODE_METADATA_STORAGE_V5 as METADATA,
    InertSemanticCompilerModuleHandoffV5 as Handoff,
    inert_semantic_compiler_module_handoff_decode_work_v5 as decode_work,
};
use fe2o3_verifier::{
    NativeConditionalRootPolicyV2, native_conditional_root_policy_input_storage_v2,
    recover_compiler_conditional_native_semantic_handoff_in_original_account_v5 as recover_source_original,
    recover_compiler_conditional_native_semantic_handoff_v5 as recover_source,
};

const DOMAINS: plan::Domains = plan::Domains {
    context: b"FE2O3/CONDITIONAL-WORKER-DURABLE-CONTEXT/V5\0",
    request: b"FE2O3/CONDITIONAL-WORKER-DURABLE-REQUEST/V5\0",
    plan: b"FE2O3/CONDITIONAL-WORKER-DURABLE-PLAN/V5\0",
    intent: b"FE2O3/CONDITIONAL-WORKER-DURABLE-PUBLICATION-INTENT/V5\0",
    kernel: b"FE2O3/CONDITIONAL-WORKER-DURABLE-KERNEL-SET/V5\0",
    target: b"FE2O3/CONDITIONAL-WORKER-DURABLE-TARGET/V5\0",
    worker: b"FE2O3/CONDITIONAL-WORKER-DURABLE-WORKER/V5\0",
    response: b"FE2O3/CONDITIONAL-WORKER-DURABLE-RESPONSE/V5\0",
    finalization: b"FE2O3/CONDITIONAL-WORKER-DURABLE-FINALIZATION/V5\0",
    publication: b"FE2O3/CONDITIONAL-WORKER-DURABLE-PUBLICATION/V5\0",
};

/// Independently accepted policy views, not policy admission or authority. The
/// caller establishes provenance and prepays their complete backing throughout
/// recovery. Never derive accepted signers or limits from the stored handoff.
pub struct ConditionalWorkerRecoveryPolicyV5<'a> {
    pub roots: &'a [NativeConditionalRootPolicyV2<'a>],
    pub history_limits: fe2o3_kernel_opt::CanonicalRefinedForwardingHistoryLimitsV1,
    pub target: fe2o3_amd_target::ProductionAmdTargetProfileV1,
}

/// Inert conditional-domain coordinates, never publication or launch authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConditionalWorkerPublicationIntentV5 {
    identity: [u8; 32],
    plan_identity: [u8; 32],
    plan: DurableLinkPublicationPlanV1,
}
impl ConditionalWorkerPublicationIntentV5 {
    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }
    pub const fn plan_identity(&self) -> &[u8; 32] {
        &self.plan_identity
    }
    pub const fn durable_plan(self) -> DurableLinkPublicationPlanV1 {
        self.plan
    }
    pub const fn grants_publication_authority(&self) -> bool {
        false
    }
    pub const fn grants_load_authority(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Move-only fresh source, transcript and derived plan. No currentness or
/// execution authority is created by preparing opaque journal storage.
/// ```no_run
/// use fe2o3_hsaco_finalize::PreparedConditionalWorkerHsacoPublicationV5 as P;
/// fn inspect(value: &P) {
///     let _ = (value.finalized(), value.transcript(), value.intent());
///     assert!(!value.authenticates_compiler_origin());
///     assert!(!value.grants_publication_authority());
///     assert!(!value.grants_load_authority());
///     assert!(!value.grants_launch_authority());
/// }
/// ```
/// ```compile_fail
/// use fe2o3_hsaco_finalize::PreparedConditionalWorkerHsacoPublicationV5 as P;
/// fn duplicate(value: P) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_hsaco_finalize::{PreparedConditionalWorkerHsacoPublicationV5 as C,
///     PreparedNativeWorkerHsacoPublicationV1 as N};
/// fn downgrade(value: C) -> N { value.into() }
/// ```
pub struct PreparedConditionalWorkerHsacoPublicationV5 {
    finalized: Artifact,
    transcript: Transcript,
    intent: ConditionalWorkerPublicationIntentV5,
    retained_storage: usize,
}
type Prepared = PreparedConditionalWorkerHsacoPublicationV5;
impl Prepared {
    pub const fn finalized(&self) -> &Artifact {
        &self.finalized
    }
    pub const fn transcript(&self) -> &Transcript {
        &self.transcript
    }
    pub const fn intent(&self) -> ConditionalWorkerPublicationIntentV5 {
        self.intent
    }
    pub const fn required_retained_storage(&self) -> usize {
        self.retained_storage
    }
    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }
    pub const fn grants_publication_authority(&self) -> bool {
        false
    }
    pub const fn grants_load_authority(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Independently recovered conditional source/Worker/artifact content. Neither
/// a journal outcome nor reproducibility supplies protected compiler origin.
/// ```no_run
/// use fe2o3_hsaco_finalize::RecoveredConditionalWorkerHsacoPublicationV5 as R;
/// fn inspect(value: &R) {
///     let _ = (value.finalized(), value.transcript(), value.intent());
///     let _ = (value.record(), value.outcome(), value.required_retained_storage());
///     assert!(!value.authenticates_compiler_origin());
///     assert!(!value.grants_publication_authority());
///     assert!(!value.grants_load_authority());
///     assert!(!value.grants_launch_authority());
/// }
/// ```
/// ```compile_fail
/// use fe2o3_hsaco_finalize::{RecoveredConditionalWorkerHsacoPublicationV5 as R,
///     PreparedConditionalWorkerHsacoPublicationV5 as P};
/// fn promote(value: R) -> P { value.into() }
/// ```
/// ```compile_fail
/// use fe2o3_hsaco_finalize::RecoveredConditionalWorkerHsacoPublicationV5 as R;
/// fn forge() -> R { R::default() }
/// ```
pub struct RecoveredConditionalWorkerHsacoPublicationV5 {
    finalized: Artifact,
    transcript: Transcript,
    intent: ConditionalWorkerPublicationIntentV5,
    record: WorkerV3PublicationIntentRecordV1,
    outcome: WorkerV3PublicationIntentOutcomeV1,
    retained_storage: usize,
}
type Recovered = RecoveredConditionalWorkerHsacoPublicationV5;
impl Recovered {
    pub const fn finalized(&self) -> &Artifact {
        &self.finalized
    }
    pub const fn transcript(&self) -> &Transcript {
        &self.transcript
    }
    pub const fn intent(&self) -> ConditionalWorkerPublicationIntentV5 {
        self.intent
    }
    pub const fn record(&self) -> WorkerV3PublicationIntentRecordV1 {
        self.record
    }
    pub const fn outcome(&self) -> WorkerV3PublicationIntentOutcomeV1 {
        self.outcome
    }
    pub const fn required_retained_storage(&self) -> usize {
        self.retained_storage
    }
    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }
    pub const fn grants_publication_authority(&self) -> bool {
        false
    }
    pub const fn grants_load_authority(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Opaque terminal refusal. No source chain exposes a nested refundable error.
#[derive(Debug)]
pub struct ConditionalWorkerHsacoPublicationErrorV5(Error);
impl fmt::Display for ConditionalWorkerHsacoPublicationErrorV5 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl std::error::Error for ConditionalWorkerHsacoPublicationErrorV5 {}
type PublicResult<T> = std::result::Result<
    (T, NativeWorkerHsacoPublicationStorageV1),
    ConditionalWorkerHsacoPublicationErrorV5,
>;
const FRAME: usize = 2 * size_of::<Prepared>()
    + 2 * size_of::<Recovered>()
    + 2 * size_of::<NativePlanInputs>()
    + size_of::<RecoveredWorkerV3PublicationIntentV1>()
    + size_of::<Sha256>()
    + size_of::<Error>()
    + 4096;

/// Reuses the already retained transcript. Caller prepays artifact and transcript
/// storage; the returned additional header charge is unreserved. This preparation
/// does not run terminal source recovery and preserves the caller's input floor.
pub fn prepare_conditional_worker_hsaco_publication_v5(
    producer: &ProducerIdentity,
    finalized: Artifact,
    transcript: Transcript,
    b: &mut Budget<'_>,
) -> PublicResult<Prepared> {
    prepare_using(producer, finalized, transcript, b, false)
}

/// Same preparation on an original owned account. Full actual artifact and
/// transcript owners are duplicated inside an additional <=256 MiB window.
/// The original account and ordinary entry's strict cap stay unchanged.
pub fn prepare_conditional_worker_hsaco_publication_in_original_account_v5(
    producer: &ProducerIdentity,
    finalized: Artifact,
    transcript: Transcript,
    b: &mut Budget<'_>,
) -> PublicResult<Prepared> {
    (|| {
        let inputs = add(
            finalized.required_retained_storage(),
            transcript.storage().retained_storage(),
        )?;
        original_terminal(b, inputs, |b| {
            prepare_using(producer, finalized, transcript, b, true).map_err(|e| e.0)
        })
    })()
    .map_err(ConditionalWorkerHsacoPublicationErrorV5)
}

fn prepare_using(
    producer: &ProducerIdentity,
    finalized: Artifact,
    transcript: Transcript,
    b: &mut Budget<'_>,
    original: bool,
) -> PublicResult<Prepared> {
    (|| {
        let floor = add(
            finalized.required_retained_storage(),
            transcript.storage().retained_storage(),
        )?;
        b.with_prepaid_scope(floor, 8, ENTRY_WORK, FRAME, |b| {
            if !original {
                check_storage_cap(b)?;
            }
            require_custody(finalized.source().custody(), Custody::ConsumedPublication)?;
            precheck(&finalized, &transcript)?;
            let intent = derive_intent(
                producer_package_identity_v1(producer),
                &finalized,
                &transcript,
                b,
            )?;
            let delta = size_of::<Prepared>();
            b.reserve_storage(delta)?;
            Ok((
                Prepared {
                    finalized,
                    transcript,
                    intent,
                    retained_storage: add(floor, delta)?,
                },
                NativeWorkerHsacoPublicationStorageV1(delta),
            ))
        })
    })()
    .map_err(ConditionalWorkerHsacoPublicationErrorV5)
}

/// Persist through the shared journal, then independently recover and compare
/// against the retained fresh owner. Failure or unwind is terminal: do not wrap
/// this call in a refund scope or retry this attempt with a fresh resource meter.
/// A committed inert record is not rolled back on later source/proof refusal.
/// The returned full recovered charge is unreserved; the original prepared
/// reservation remains the caller's responsibility even after its owner drops.
pub fn persist_prepared_conditional_worker_hsaco_publication_v5(
    output_dir: &Path,
    producer: &ProducerIdentity,
    prepared: Prepared,
    policy: ConditionalWorkerRecoveryPolicyV5<'_>,
    b: &mut Budget<'_>,
) -> PublicResult<Recovered> {
    persist_using(output_dir, producer, prepared, policy, b, false)
}

/// Same journal, strict source replay and fresh/recovered comparison on the
/// original owned account. The complete prepared owner and actual policy views
/// are counted again in a nonrefunding <=256 MiB window. A committed record is
/// not undone by later refusal, and partial terminal reservations are retained.
pub fn persist_prepared_conditional_worker_hsaco_publication_in_original_account_v5(
    output_dir: &Path,
    producer: &ProducerIdentity,
    prepared: Prepared,
    policy: ConditionalWorkerRecoveryPolicyV5<'_>,
    b: &mut Budget<'_>,
) -> PublicResult<Recovered> {
    (|| {
        let policies = native_conditional_root_policy_input_storage_v2(policy.roots, b)?;
        let inputs = add(prepared.required_retained_storage(), policies)?;
        original_terminal(b, inputs, |b| {
            persist_using(output_dir, producer, prepared, policy, b, true).map_err(|e| e.0)
        })
    })()
    .map_err(ConditionalWorkerHsacoPublicationErrorV5)
}

fn persist_using(
    output_dir: &Path,
    producer: &ProducerIdentity,
    prepared: Prepared,
    policy: ConditionalWorkerRecoveryPolicyV5<'_>,
    b: &mut Budget<'_>,
    original: bool,
) -> PublicResult<Recovered> {
    terminal_using(b, prepared.required_retained_storage(), original, |b| {
        if producer_package_identity_v1(producer) != prepared.intent.plan.scope().package() {
            return Err(Error::Mismatch("conditional producer"));
        }
        require_custody(
            prepared.finalized.source().custody(),
            Custody::ConsumedPublication,
        )?;
        precheck(&prepared.finalized, &prepared.transcript)?;
        let source = prepared.finalized.source();
        let parts = extract(
            source.bootstrap_request_bytes(),
            source.replay_request_bytes(),
        )
        .map_err(|e| failure("conditional providers", e))?;
        let (attachments, output) = copy_storage_attachments(
            source.recovered_handoff().handoff().canonical_bytes(),
            prepared.transcript.canonical_bytes(),
            prepared.finalized.finalized().as_bytes(),
            parts,
        )?;
        let attempt = prepared.intent.plan.attempt();
        let stored = persist_worker_v3_publication_intent_v1(
            output_dir,
            producer,
            attempt,
            prepared.intent.plan,
            attachments,
            output,
        )
        .map_err(|e| failure("conditional persist", e))?;
        let expected_record = stored.record();
        let (recovered, storage) =
            validate_recovered_using(producer, attempt, stored, policy, b, original)?;
        compare_fresh(&prepared, expected_record, &recovered, b)?;
        drop(prepared);
        Ok((recovered, storage))
    })
}

/// Recover journal bytes, strictly admit the conditional V5 source under the
/// independent policy, then replay Worker/finalizer content on this account.
/// There is no ordinary/V4 fallback. Failure/unwind retains terminal charges;
/// never enclose this call in a blanket-refund scope. Success returns the full
/// additional recovered charge unreserved. Caller policy backing stays prepaid.
/// Journal I/O, bounded attachment copies/hashes and artifact payloads keep the
/// existing artifact-domain bounds, not whole-process allocation/RSS accounting.
pub fn recover_conditional_worker_hsaco_publication_v5(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    policy: ConditionalWorkerRecoveryPolicyV5<'_>,
    b: &mut Budget<'_>,
) -> PublicResult<Recovered> {
    terminal(b, b.storage(), |b| {
        let stored = recover_worker_v3_publication_intent_v1(output_dir, producer, attempt)
            .map_err(|e| failure("conditional recover storage", e))?;
        validate_recovered(producer, attempt, stored, policy, b)
    })
}

fn check_storage_cap(b: &Budget<'_>) -> Result<()> {
    if b.storage_limit() > MAX_INERT_REFINED_FORWARDING_STORAGE_V1 {
        return Err(Error::StorageCap);
    }
    Ok(())
}
fn require_custody(actual: Custody, expected: Custody) -> Result<()> {
    if actual != expected {
        return Err(Error::Mismatch("conditional source custody"));
    }
    Ok(())
}

// No restoring guard surrounds opaque recovery. Only the exact success transfer
// below releases charges; error and panic preserve even partly acquired custody.
fn terminal<'w, T>(
    b: &mut Budget<'w>,
    floor: usize,
    run: impl FnOnce(&mut Budget<'w>) -> Result<(T, usize)>,
) -> PublicResult<T> {
    terminal_using(b, floor, false, run)
}

fn original_terminal<'w, T>(
    b: &mut Budget<'w>,
    inputs: usize,
    run: impl FnOnce(&mut Budget<'w>) -> Result<T>,
) -> Result<T> {
    let floor = b.storage();
    let overlap = add(inputs, Budget::STORAGE_WINDOW_SCRATCH_V1)?;
    b.with_additional_storage_window_v1(MAX_INERT_REFINED_FORWARDING_STORAGE_V1, |b| {
        if floor < inputs {
            return Err(Resource::Accounting.into());
        }
        b.reserve_storage(overlap)?;
        let result = run(b)?;
        if b.storage() != add(floor, overlap)? {
            return Err(Resource::Accounting.into());
        }
        b.release_storage(overlap)?;
        Ok(result)
    })
}

fn terminal_using<'w, T>(
    b: &mut Budget<'w>,
    floor: usize,
    original_account: bool,
    run: impl FnOnce(&mut Budget<'w>) -> Result<(T, usize)>,
) -> PublicResult<T> {
    (|| {
        let original = b.storage();
        let address = b as *const Budget<'_> as usize;
        let ledger = b.work_ledger_identity_v1();
        b.charge_work(ENTRY_WORK)?;
        if !original_account {
            check_storage_cap(b)?;
        }
        if original < floor {
            return Err(Resource::Accounting.into());
        }
        b.reserve_storage(FRAME)?;
        let (owner, storage) = run(b)?;
        let release = add(FRAME, storage)?;
        if b as *const Budget<'_> as usize != address
            || b.work_ledger_identity_v1() != ledger
            || b.storage() != add(original, release)?
        {
            drop(owner);
            return Err(Resource::Accounting.into());
        }
        b.release_storage(release)?;
        Ok((owner, NativeWorkerHsacoPublicationStorageV1(storage)))
    })()
    .map_err(ConditionalWorkerHsacoPublicationErrorV5)
}

fn validate_recovered(
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    stored: RecoveredWorkerV3PublicationIntentV1,
    policy: ConditionalWorkerRecoveryPolicyV5<'_>,
    b: &mut Budget<'_>,
) -> Result<(Recovered, usize)> {
    validate_recovered_using(producer, attempt, stored, policy, b, false)
}

fn validate_recovered_using(
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    stored: RecoveredWorkerV3PublicationIntentV1,
    policy: ConditionalWorkerRecoveryPolicyV5<'_>,
    b: &mut Budget<'_>,
    original: bool,
) -> Result<(Recovered, usize)> {
    check_record_inputs(producer, attempt, &stored)?;
    let outcome = stored.outcome();
    let (record, attachments, exact_output) = stored.into_parts();
    let (outer_bytes, providers, transcript_bytes) = attachments.into_parts();
    let outer_storage = add(outer_bytes.capacity(), METADATA)?;
    b.reserve_storage(outer_storage)?;
    b.charge_work(
        decode_work(outer_bytes.len())
            .map_err(|e| failure("V5 decode quote", format_args!("{e:?}")))?,
    )?;
    let outer = Handoff::decode_owned(outer_bytes)
        .map_err(|e| failure("V5 decode", format_args!("{e:?}")))?;
    // Deliberately outside every ordinary restoring scope.
    let (source, source_storage) = if original {
        recover_source_original(outer, policy.roots, policy.history_limits, policy.target, b)
    } else {
        recover_source(outer, policy.roots, policy.history_limits, policy.target, b)
    }
    .map_err(|e| failure("conditional source/F", e))?;
    b.reserve_storage(source_storage.retained_storage())?;
    let input_storage = transcript_bytes.capacity();
    b.reserve_storage(input_storage)?;
    let (transcript, codec_storage) = Transcript::decode_canonical(&transcript_bytes, b)?;
    b.reserve_storage(codec_storage.retained_storage())?;
    drop(transcript_bytes);
    b.release_storage(input_storage)?;
    let output_storage = exact_output.capacity();
    b.reserve_storage(output_storage)?;
    let (finalized, replay_storage) = replay(
        producer,
        attempt,
        source,
        &transcript,
        providers,
        &exact_output,
        b,
    )
    .map_err(|e| match e {
        NativeWorkerReplayErrorV1::Resource(e) => Error::Resource(e),
        other => failure("conditional finalizer replay", other),
    })?;
    b.reserve_storage(replay_storage.retained_storage())?;
    require_custody(finalized.source().custody(), Custody::RecoveredTranscript)?;
    drop(exact_output);
    b.release_storage(output_storage)?;
    let intent = derive_intent(
        producer_package_identity_v1(producer),
        &finalized,
        &transcript,
        b,
    )?;
    if intent.plan != record.plan() {
        return Err(Error::Mismatch("conditional derived durable plan"));
    }
    let delta = [
        outer_storage,
        source_storage.retained_storage(),
        codec_storage.retained_storage(),
        replay_storage.retained_storage(),
        size_of::<Recovered>(),
    ]
    .into_iter()
    .try_fold(0, add)?;
    b.reserve_storage(size_of::<Recovered>())?;
    Ok((
        Recovered {
            finalized,
            transcript,
            intent,
            record,
            outcome,
            retained_storage: delta,
        },
        delta,
    ))
}

fn precheck(artifact: &Artifact, transcript: &Transcript) -> Result<()> {
    let source = artifact.source();
    let outer = source.recovered_handoff().handoff();
    let module = outer.module_handoff().module_identity();
    precheck_plan_storage(
        ContentIdentityV1::from_parts(*module.sha256(), module.byte_len()),
        source.plan(),
        outer.canonical_bytes().len(),
        artifact.finalized().as_bytes().len(),
    )?;
    check_length(
        transcript.canonical_bytes().len(),
        MAX_WORKER_V3_FINALIZER_REPLAY_TRANSCRIPT_BYTES_V1,
        "transcript",
    )?;
    transcript.verify_finalized_coordinates(artifact)?;
    Ok(())
}

fn derive_intent(
    package: PackageIdentityV1,
    artifact: &Artifact,
    transcript: &Transcript,
    b: &mut Budget<'_>,
) -> Result<ConditionalWorkerPublicationIntentV5> {
    transcript.verify_finalized_coordinates(artifact)?;
    let source = artifact.source();
    let binding = source.binding();
    let receipt = binding.receipt();
    let outer = source.recovered_handoff().handoff();
    let manifest = outer.module_handoff().symbol_manifest().identity();
    let worker = source.worker_measurement();
    let executable = worker.executable();
    let finalized = artifact.finalized();
    let work = [
        finalized.as_bytes().len(),
        finalized.descriptor_bytes().len(),
        worker.worker_build_identity().len(),
        worker.llvm_build_identity().len(),
    ]
    .into_iter()
    .try_fold(0, add)?;
    b.charge_work(work)?;
    let worker_identity = hash_parts(
        DOMAINS.worker,
        &[
            executable.sha256(),
            &executable.byte_len().to_le_bytes(),
            &(worker.worker_build_identity().len() as u64).to_le_bytes(),
            worker.worker_build_identity().as_bytes(),
            &(worker.llvm_build_identity().len() as u64).to_le_bytes(),
            worker.llvm_build_identity().as_bytes(),
        ],
    );
    Ok(derive_plan(NativePlanInputs {
        package,
        attempt: receipt.attempt(),
        slot: receipt.slot() as u8,
        transaction: *receipt.transaction_identity().as_bytes(),
        outer: ContentIdentityV1::from_parts(
            *outer.identity().sha256(),
            outer.identity().byte_len(),
        ),
        binding: *binding.identity(),
        source: *source.identity(),
        worker: worker_identity,
        finalized: *artifact.identity(),
        transcript: *transcript.identity().as_bytes(),
        link_plan: *source.plan().identity().as_bytes(),
        manifest: ContentIdentityV1::from_parts(*manifest.sha256(), manifest.byte_len()),
        policy: *artifact.policy().identity().as_bytes(),
        raw: source.output_identity(),
        output: ContentIdentityV1::calculate(finalized.as_bytes()),
        descriptor: ContentIdentityV1::calculate(finalized.descriptor_bytes()),
        canonical_digest: *finalized.digest().as_bytes(),
    }))
}
fn derive_plan(input: NativePlanInputs) -> ConditionalWorkerPublicationIntentV5 {
    let (identity, plan_identity, plan) = plan::derive(input, &DOMAINS);
    ConditionalWorkerPublicationIntentV5 {
        identity,
        plan_identity,
        plan,
    }
}

fn compare_fresh(
    prepared: &Prepared,
    expected_record: WorkerV3PublicationIntentRecordV1,
    recovered: &Recovered,
    b: &mut Budget<'_>,
) -> Result<()> {
    let source = prepared.finalized.source();
    let actual = recovered.finalized.source();
    let outer = source.recovered_handoff().handoff().canonical_bytes();
    let transcript = prepared.transcript.canonical_bytes();
    let output = prepared.finalized.finalized().as_bytes();
    b.charge_work(
        [outer.len(), transcript.len(), output.len()]
            .into_iter()
            .try_fold(0, add)?,
    )?;
    if recovered.record != expected_record
        || recovered.intent != prepared.intent
        || recovered.record.plan() != prepared.intent.plan
        || recovered.finalized.identity() != prepared.finalized.identity()
        || actual.binding() != source.binding()
        || actual.identity() != source.identity()
        || actual.recovered_handoff().handoff().canonical_bytes() != outer
        || recovered.transcript.canonical_bytes() != transcript
        || recovered.finalized.finalized().as_bytes() != output
    {
        return Err(Error::Mismatch(
            "exact prepared/recovered conditional evidence",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "conditional_worker_publication_tests.rs"]
mod tests;
