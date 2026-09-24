//! Source-retaining native restart through the existing opaque artifact journal.
//!
//! The WorkerV3 storage types below are byte owners and atomic I/O mechanics,
//! never a V3 outer decoder or a publication-authority bridge. Native source/F,
//! Worker and finalizer replay are required on both fresh and restarted paths.
//! Shared I/O, attachment copies/hashes, producer-package hashing and artifact
//! replay retain their existing bounded artifact domains. Only new native
//! decode/source/transcript/header ownership is charged to the caller ledger.
//! This is not whole-filesystem, process, allocation-capacity or RSS accounting.

use std::{fmt, mem::size_of, path::Path};

use fe2o3_artifact_transaction::{
    AtomicPublicationIdentityV1, BuildAttempt, CanonicalLinkRequestIdentityV1,
    DurableLinkPublicationPlanV1, FinalizationIdentityV1, FinalizedOutputIdentityV1,
    KernelSetIdentityV1, LinkPublicationScopeV1, LinkedOutputIdentityV1,
    MAX_COMPILER_MODULE_HANDOFF_BYTES_V3, MAX_WORKER_V3_FINALIZER_REPLAY_TRANSCRIPT_BYTES_V1,
    MAX_WORKER_V3_PUBLICATION_INTENT_OUTPUT_BYTES_V1,
    MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1,
    MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_PAYLOADS_V1, PackageIdentityV1, PinnedWorkerIdentityV1,
    ProducerIdentity, RecoveredWorkerV3PublicationIntentV1, TargetIdentityV1,
    ValidatedResponseIdentityV1, WorkerV3FinalizerReplayAttachmentsV1,
    WorkerV3PublicationIntentOutcomeV1, WorkerV3PublicationIntentRecordV1,
    persist_worker_v3_publication_intent_v1, producer_package_identity_v1,
    recover_worker_v3_publication_intent_v1,
};
use fe2o3_compiler_ffi::{
    INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_DECODE_METADATA_STORAGE_V4 as DECODE_METADATA,
    InertSemanticCompilerModuleHandoffV4, MAX_INERT_REFINED_FORWARDING_STORAGE_V1,
    inert_semantic_compiler_module_handoff_decode_work_v4,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_verifier::{
    CompilerRefinedForwardingOutputErrorV1, recover_compiler_native_semantic_handoff_v4,
};
use sha2::{Digest, Sha256};

use crate::{
    ContentIdentityV1, NativeWorkerDiagnosticV1,
    native_worker_compact_replay::{
        NativeWorkerCompactFinalizerReplayV1, NativeWorkerCompactReplayErrorV1,
        extract_native_worker_external_providers_v1,
        prepare_native_worker_compact_finalizer_replay_v1,
    },
    native_worker_finalization::PreparedFinalizedNativeWorkerHsacoV1,
    native_worker_replay::{NativeWorkerReplayErrorV1, revalidate_native_worker_finalizer_v1},
};

const CONTEXT_DOMAIN: &[u8] = b"FE2O3/NATIVE-WORKER-DURABLE-CONTEXT/V1\0";
const REQUEST_DOMAIN: &[u8] = b"FE2O3/NATIVE-WORKER-DURABLE-REQUEST/V1\0";
const PLAN_DOMAIN: &[u8] = b"FE2O3/NATIVE-WORKER-DURABLE-PLAN/V1\0";
const INTENT_DOMAIN: &[u8] = b"FE2O3/NATIVE-WORKER-DURABLE-PUBLICATION-INTENT/V1\0";
const KERNEL_DOMAIN: &[u8] = b"FE2O3/NATIVE-WORKER-DURABLE-KERNEL-SET/V1\0";
const TARGET_DOMAIN: &[u8] = b"FE2O3/NATIVE-WORKER-DURABLE-TARGET/V1\0";
const WORKER_DOMAIN: &[u8] = b"FE2O3/NATIVE-WORKER-DURABLE-WORKER/V1\0";
const RESPONSE_DOMAIN: &[u8] = b"FE2O3/NATIVE-WORKER-DURABLE-RESPONSE/V1\0";
const FINALIZATION_DOMAIN: &[u8] = b"FE2O3/NATIVE-WORKER-DURABLE-FINALIZATION/V1\0";
const PUBLICATION_DOMAIN: &[u8] = b"FE2O3/NATIVE-WORKER-DURABLE-PUBLICATION/V1\0";

/// Native domain binding of the entire durable plan, never publication authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeWorkerPublicationPlanIdentityV1([u8; 32]);
impl NativeWorkerPublicationPlanIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeWorkerPublicationIntentIdentityV1([u8; 32]);
impl NativeWorkerPublicationIntentIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Privately derived inert identities; the shared plan is only a storage input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeWorkerPublicationIntentV1 {
    identity: NativeWorkerPublicationIntentIdentityV1,
    plan_identity: NativeWorkerPublicationPlanIdentityV1,
    plan: DurableLinkPublicationPlanV1,
}
impl NativeWorkerPublicationIntentV1 {
    pub const fn identity(self) -> NativeWorkerPublicationIntentIdentityV1 {
        self.identity
    }
    pub const fn plan_identity(self) -> NativeWorkerPublicationPlanIdentityV1 {
        self.plan_identity
    }
    pub const fn durable_plan(self) -> DurableLinkPublicationPlanV1 {
        self.plan
    }
    pub const fn grants_publication_authority(self) -> bool {
        false
    }
    pub const fn grants_load_authority(self) -> bool {
        false
    }
    pub const fn grants_launch_authority(self) -> bool {
        false
    }
}

/// Additional native storage to reserve before retaining the returned owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeWorkerHsacoPublicationStorageV1(usize);
impl NativeWorkerHsacoPublicationStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Retains the complete finalized native source/F owner and its native codec.
/// Preparation neither writes durable files nor supplies publication authority.
///
/// ```compile_fail
/// use fe2o3_hsaco_finalize::PreparedNativeWorkerHsacoPublicationV1 as Owner;
/// fn duplicate(owner: Owner) { let _ = owner.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_hsaco_finalize::PreparedNativeWorkerHsacoPublicationV1 as Owner;
/// fn forge() -> Owner { Owner::default() }
/// ```
pub struct PreparedNativeWorkerHsacoPublicationV1 {
    finalized: PreparedFinalizedNativeWorkerHsacoV1,
    transcript: NativeWorkerCompactFinalizerReplayV1,
    intent: NativeWorkerPublicationIntentV1,
    retained_storage: usize,
}
impl PreparedNativeWorkerHsacoPublicationV1 {
    pub const fn finalized(&self) -> &PreparedFinalizedNativeWorkerHsacoV1 {
        &self.finalized
    }
    pub const fn transcript(&self) -> &NativeWorkerCompactFinalizerReplayV1 {
        &self.transcript
    }
    pub const fn intent(&self) -> NativeWorkerPublicationIntentV1 {
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

/// Independent native replay retaining actual source/F, transcript, record and
/// rederived plan. No currentness, signing, machine proof or launch authority.
///
/// ```compile_fail
/// use fe2o3_hsaco_finalize::RecoveredNativeWorkerHsacoPublicationV1 as Owner;
/// fn duplicate(owner: Owner) { let _ = owner.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_hsaco_finalize::RecoveredNativeWorkerHsacoPublicationV1 as Owner;
/// fn forge() -> Owner { Owner::default() }
/// ```
pub struct RecoveredNativeWorkerHsacoPublicationV1 {
    finalized: PreparedFinalizedNativeWorkerHsacoV1,
    transcript: NativeWorkerCompactFinalizerReplayV1,
    intent: NativeWorkerPublicationIntentV1,
    record: WorkerV3PublicationIntentRecordV1,
    outcome: WorkerV3PublicationIntentOutcomeV1,
    retained_storage: usize,
}
impl RecoveredNativeWorkerHsacoPublicationV1 {
    pub const fn finalized(&self) -> &PreparedFinalizedNativeWorkerHsacoV1 {
        &self.finalized
    }
    pub const fn transcript(&self) -> &NativeWorkerCompactFinalizerReplayV1 {
        &self.transcript
    }
    pub const fn intent(&self) -> NativeWorkerPublicationIntentV1 {
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

#[derive(Debug)]
pub enum NativeWorkerHsacoPublicationErrorV1 {
    Resource(Resource),
    StorageCap,
    Limit(&'static str),
    Mismatch(&'static str),
    Stage {
        phase: &'static str,
        diagnostic: NativeWorkerDiagnosticV1,
    },
}
type Error = NativeWorkerHsacoPublicationErrorV1;
type Result<T> = std::result::Result<T, Error>;
impl From<Resource> for Error {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl From<NativeWorkerCompactReplayErrorV1> for Error {
    fn from(value: NativeWorkerCompactReplayErrorV1) -> Self {
        match value {
            NativeWorkerCompactReplayErrorV1::Resource(value) => Self::Resource(value),
            other => failure("native compact replay", other),
        }
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(value) => value.fmt(f),
            Self::StorageCap => f.write_str("native publication ledger exceeds native storage cap"),
            Self::Limit(field) => write!(
                f,
                "native publication exceeds shared storage limit: {field}"
            ),
            Self::Mismatch(field) => write!(f, "native publication mismatch: {field}"),
            Self::Stage { phase, diagnostic } => {
                write!(f, "native publication {phase}: {diagnostic}")
            }
        }
    }
}
impl std::error::Error for Error {}
fn failure(phase: &'static str, value: impl fmt::Display) -> Error {
    Error::Stage {
        phase,
        diagnostic: NativeWorkerDiagnosticV1::from_display(value),
    }
}

// Fixed native hashes/joins and <=128 plan-input accounting, not a quote for
// the producer's strings, attachment payloads, journal or artifact replay.
const ENTRY_WORK: usize = 16_384;
const FRAME: usize = 2 * size_of::<PreparedNativeWorkerHsacoPublicationV1>()
    + 2 * size_of::<RecoveredNativeWorkerHsacoPublicationV1>()
    + 3 * size_of::<NativeWorkerPublicationIntentV1>()
    + 2 * size_of::<NativePlanInputs>()
    + size_of::<Sha256>()
    + size_of::<Error>()
    + 2048;

fn scoped<T>(
    budget: &mut Budget<'_>,
    floor: usize,
    operation: impl FnOnce(&mut Budget<'_>) -> Result<T>,
) -> Result<T> {
    budget.with_prepaid_scope(floor, 8, ENTRY_WORK, FRAME, |budget| {
        if budget.storage_limit() > MAX_INERT_REFINED_FORWARDING_STORAGE_V1 {
            return Err(Error::StorageCap);
        }
        operation(budget)
    })
}

/// Retains the finalized source while deriving the native compact transcript and
/// durable plan. The finalized owner's original floor must already be reserved.
/// Returns only the additional transcript/header charge, unreserved. All entry
/// reservations survive success, refusal and unwind unchanged.
pub fn prepare_native_worker_hsaco_publication_v1(
    producer: &ProducerIdentity,
    finalized: PreparedFinalizedNativeWorkerHsacoV1,
    budget: &mut Budget<'_>,
) -> Result<(
    PreparedNativeWorkerHsacoPublicationV1,
    NativeWorkerHsacoPublicationStorageV1,
)> {
    let floor = finalized.required_retained_storage();
    scoped(budget, floor, |budget| {
        precheck_finalized_storage(&finalized)?;
        let (transcript, codec_storage) =
            prepare_native_worker_compact_finalizer_replay_v1(&finalized, budget)?;
        budget.reserve_storage(codec_storage.retained_storage())?;
        check_length(
            transcript.canonical_bytes().len(),
            MAX_WORKER_V3_FINALIZER_REPLAY_TRANSCRIPT_BYTES_V1,
            "transcript",
        )?;
        let intent = derive_intent(
            producer_package_identity_v1(producer),
            &finalized,
            &transcript,
        )?;
        let delta = add(
            codec_storage.retained_storage(),
            size_of::<PreparedNativeWorkerHsacoPublicationV1>(),
        )?;
        budget.reserve_storage(size_of::<PreparedNativeWorkerHsacoPublicationV1>())?;
        Ok((
            PreparedNativeWorkerHsacoPublicationV1 {
                finalized,
                transcript,
                intent,
                retained_storage: add(floor, delta)?,
            },
            NativeWorkerHsacoPublicationStorageV1(delta),
        ))
    })
}

/// Copies only bounded storage attachments while retaining `prepared` through
/// the same native validator used on restart. After exact comparison it drops
/// the original owner; its caller reservation is unchanged and can be retired
/// by the caller. The returned delta is the FULL independently reconstructed
/// native owner, not merely another header. Reserve it before retaining it.
///
/// A failure after durable commit may leave an inert restart record. This wrapper
/// never rolls back the shared journal or claims publication/load authority.
pub fn persist_prepared_native_worker_hsaco_publication_v1(
    output_dir: &Path,
    producer: &ProducerIdentity,
    prepared: PreparedNativeWorkerHsacoPublicationV1,
    budget: &mut Budget<'_>,
) -> Result<(
    RecoveredNativeWorkerHsacoPublicationV1,
    NativeWorkerHsacoPublicationStorageV1,
)> {
    scoped(budget, prepared.required_retained_storage(), |budget| {
        let package = producer_package_identity_v1(producer);
        if package != prepared.intent.plan.scope().package() {
            return Err(Error::Mismatch("producer"));
        }
        let (attachments, output) = storage_attachments(&prepared)?;
        let attempt = prepared.intent.plan.attempt();
        let stored = persist_worker_v3_publication_intent_v1(
            output_dir,
            producer,
            attempt,
            prepared.intent.plan,
            attachments,
            output,
        )
        .map_err(|e| failure("persist", e))?;
        let expected_record = stored.record();
        let (recovered, storage) = validate_recovered(producer, attempt, stored, budget)?;
        budget.reserve_storage(storage.retained_storage())?;
        compare_fresh(&prepared, expected_record, &recovered)?;
        drop(prepared);
        Ok((recovered, storage))
    })
}

/// Recovers opaque byte storage, then requires V4-only source/F and native Worker
/// replay. No legacy decoder fallback or V3 binding is used. The entire returned
/// source/codec/Worker/finalizer/header charge is additional and unreserved;
/// inherited reservations and work history are preserved on all exits.
pub fn recover_native_worker_hsaco_publication_v1(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    budget: &mut Budget<'_>,
) -> Result<(
    RecoveredNativeWorkerHsacoPublicationV1,
    NativeWorkerHsacoPublicationStorageV1,
)> {
    let floor = budget.storage();
    scoped(budget, floor, |budget| {
        let stored = recover_worker_v3_publication_intent_v1(output_dir, producer, attempt)
            .map_err(|e| failure("recover storage", e))?;
        validate_recovered(producer, attempt, stored, budget)
    })
}

fn validate_recovered(
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    stored: RecoveredWorkerV3PublicationIntentV1,
    budget: &mut Budget<'_>,
) -> Result<(
    RecoveredNativeWorkerHsacoPublicationV1,
    NativeWorkerHsacoPublicationStorageV1,
)> {
    let floor = budget.storage();
    scoped(budget, floor, |budget| {
        check_record_inputs(producer, attempt, &stored)?;
        let outcome = stored.outcome();
        let (record, attachments, exact_output) = stored.into_parts();
        let (outer_bytes, providers, transcript_bytes) = attachments.into_parts();
        // The shared storage domain transfers this existing allocation into the
        // native domain. Pay its entire capacity, not just the canonical range.
        let outer_storage = add(outer_bytes.capacity(), DECODE_METADATA)?;
        budget.reserve_storage(outer_storage)?;
        let work = inert_semantic_compiler_module_handoff_decode_work_v4(outer_bytes.len())
            .map_err(|e| failure("V4 decode quote", format_args!("{e:?}")))?;
        budget.charge_work(work)?;
        let outer = InertSemanticCompilerModuleHandoffV4::decode_owned(outer_bytes)
            .map_err(|e| failure("V4 decode", format_args!("{e:?}")))?;
        let (source, source_storage) = recover_compiler_native_semantic_handoff_v4(outer, budget)
            .map_err(|e| match e {
            CompilerRefinedForwardingOutputErrorV1::Resource(e) => Error::Resource(e),
            other => failure("native source/F", other),
        })?;
        budget.reserve_storage(source_storage.retained_storage())?;
        let source_floor = add(outer_storage, source_storage.retained_storage())?;

        let input_charge = transcript_bytes.len();
        budget.reserve_storage(input_charge)?;
        let (transcript, codec_storage) =
            NativeWorkerCompactFinalizerReplayV1::decode_canonical(&transcript_bytes, budget)?;
        budget.reserve_storage(codec_storage.retained_storage())?;
        drop(transcript_bytes);
        budget.release_storage(input_charge)?;
        if transcript.attempt() != attempt {
            return Err(Error::Mismatch("transcript attempt"));
        }
        transcript.verify_outer_identity(source.handoff().identity())?;
        let (finalized, replay_storage) = revalidate_native_worker_finalizer_v1(
            producer,
            attempt,
            source,
            &transcript,
            providers,
            &exact_output,
            budget,
        )
        .map_err(|e| match e {
            NativeWorkerReplayErrorV1::Resource(e) => Error::Resource(e),
            other => failure("native finalizer replay", other),
        })?;
        budget.reserve_storage(replay_storage.retained_storage())?;
        if finalized.exact_finalized_bytes() != exact_output {
            return Err(Error::Mismatch("exact finalized artifact"));
        }
        let intent = derive_intent(
            producer_package_identity_v1(producer),
            &finalized,
            &transcript,
        )?;
        if intent.plan != record.plan() {
            return Err(Error::Mismatch("derived durable plan"));
        }
        let delta = [
            source_floor,
            codec_storage.retained_storage(),
            replay_storage.retained_storage(),
            size_of::<RecoveredNativeWorkerHsacoPublicationV1>(),
        ]
        .into_iter()
        .try_fold(0, add)?;
        budget.reserve_storage(size_of::<RecoveredNativeWorkerHsacoPublicationV1>())?;
        Ok((
            RecoveredNativeWorkerHsacoPublicationV1 {
                finalized,
                transcript,
                intent,
                record,
                outcome,
                retained_storage: delta,
            },
            NativeWorkerHsacoPublicationStorageV1(delta),
        ))
    })
}

fn check_record_inputs(
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    stored: &RecoveredWorkerV3PublicationIntentV1,
) -> Result<()> {
    check_shape(
        stored.outer_handoff().len(),
        stored.external_providers().len(),
        stored.external_providers().payload_length(),
        Some(stored.finalizer_replay_transcript().len()),
        stored.exact_output().len(),
    )?;
    let record = stored.record();
    // The opaque store already rederived producer/occurrence keys under its
    // journal lock. Check every exposed record/input coordinate again; the
    // native replay rederives the producer-specific V4 transaction separately.
    if record.attempt() != attempt
        || record.plan().attempt() != attempt
        || record.plan().scope().package() != producer_package_identity_v1(producer)
    {
        return Err(Error::Mismatch("record producer/attempt"));
    }
    if record.outer_handoff_length() != stored.outer_handoff().len()
        || record.outer_handoff_sha256() != raw_hash(stored.outer_handoff())
        || record.transcript_length() != stored.finalizer_replay_transcript().len()
        || record.transcript_sha256() != raw_hash(stored.finalizer_replay_transcript())
        || record.output_length() != stored.exact_output().len()
        || record.output_sha256() != raw_hash(stored.exact_output())
        || record.external_provider_count() != stored.external_providers().len()
        || record.external_provider_payload_length() != stored.external_providers().payload_length()
        || record.external_provider_archive_length()
            != stored.external_providers().canonical_length()
        || record.external_provider_archive_sha256()
            != stored.external_providers().canonical_sha256()
    {
        return Err(Error::Mismatch("exact storage record"));
    }
    Ok(())
}

fn compare_fresh(
    prepared: &PreparedNativeWorkerHsacoPublicationV1,
    expected_record: WorkerV3PublicationIntentRecordV1,
    recovered: &RecoveredNativeWorkerHsacoPublicationV1,
) -> Result<()> {
    if recovered.record != expected_record
        || recovered.intent != prepared.intent
        || recovered.record.plan() != prepared.intent.plan
    {
        return Err(Error::Mismatch("persisted record/derived plan"));
    }
    if prepared.transcript.canonical_bytes() != recovered.transcript.canonical_bytes()
        || prepared.finalized.exact_finalized_bytes() != recovered.finalized.exact_finalized_bytes()
        || prepared.finalized.identity() != recovered.finalized.identity()
        || prepared.finalized.source_evidence().identity()
            != recovered.finalized.source_evidence().identity()
        || prepared.finalized.source_evidence().binding()
            != recovered.finalized.source_evidence().binding()
        || prepared
            .finalized
            .source_evidence()
            .recovered_handoff()
            .handoff()
            .canonical_bytes()
            != recovered
                .finalized
                .source_evidence()
                .recovered_handoff()
                .handoff()
                .canonical_bytes()
    {
        return Err(Error::Mismatch("exact prepared/recovered native evidence"));
    }
    Ok(())
}

fn precheck_finalized_storage(finalized: &PreparedFinalizedNativeWorkerHsacoV1) -> Result<()> {
    let source = finalized.source_evidence();
    let outer = source.recovered_handoff().handoff();
    let module = outer.module_handoff().module_identity();
    let module = ContentIdentityV1::from_parts(*module.sha256(), module.byte_len());
    let mut modules = 0;
    let mut count = 0;
    let mut bytes = 0;
    for input in source.plan().inputs() {
        if input.identity() == module {
            modules += 1;
        } else {
            count += 1;
            let n =
                usize::try_from(input.identity().byte_len()).map_err(|_| Resource::Arithmetic)?;
            bytes = add(bytes, n)?;
        }
    }
    if modules != 1 {
        return Err(Error::Mismatch("exact module in Worker plan"));
    }
    check_shape(
        outer.canonical_bytes().len(),
        count,
        bytes,
        None,
        finalized.exact_finalized_bytes().len(),
    )
}

fn check_shape(
    outer: usize,
    providers: usize,
    provider_bytes: usize,
    transcript: Option<usize>,
    output: usize,
) -> Result<()> {
    check_length(outer, MAX_COMPILER_MODULE_HANDOFF_BYTES_V3, "outer")?;
    check_length(
        output,
        MAX_WORKER_V3_PUBLICATION_INTENT_OUTPUT_BYTES_V1,
        "output",
    )?;
    if providers > MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_PAYLOADS_V1
        || provider_bytes > MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1
        || (providers == 0) != (provider_bytes == 0)
    {
        return Err(Error::Limit("external providers"));
    }
    if let Some(n) = transcript {
        check_length(
            n,
            MAX_WORKER_V3_FINALIZER_REPLAY_TRANSCRIPT_BYTES_V1,
            "transcript",
        )?;
    }
    Ok(())
}

fn storage_attachments(
    prepared: &PreparedNativeWorkerHsacoPublicationV1,
) -> Result<(WorkerV3FinalizerReplayAttachmentsV1, Vec<u8>)> {
    precheck_finalized_storage(&prepared.finalized)?;
    let transcript = prepared.transcript.canonical_bytes();
    check_length(
        transcript.len(),
        MAX_WORKER_V3_FINALIZER_REPLAY_TRANSCRIPT_BYTES_V1,
        "transcript",
    )?;
    let parts = extract_native_worker_external_providers_v1(&prepared.finalized)?;
    let mut providers = Vec::new();
    providers
        .try_reserve_exact(parts.external_providers.len())
        .map_err(|_| Resource::Allocation)?;
    if providers.capacity() > MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_PAYLOADS_V1 {
        return Err(Error::Limit("provider list capacity"));
    }
    for part in parts.external_providers {
        providers.push(part.bytes);
    }
    // Shared constructors recheck actual provider capacities and aggregate owner
    // capacity; no write occurs before they and the store's input checks pass.
    let outer = copy_attachment(
        prepared
            .finalized
            .source_evidence()
            .recovered_handoff()
            .handoff()
            .canonical_bytes(),
        MAX_COMPILER_MODULE_HANDOFF_BYTES_V3,
        "outer",
    )?;
    let transcript = copy_attachment(
        transcript,
        MAX_WORKER_V3_FINALIZER_REPLAY_TRANSCRIPT_BYTES_V1,
        "transcript",
    )?;
    let output = copy_attachment(
        prepared.finalized.exact_finalized_bytes(),
        MAX_WORKER_V3_PUBLICATION_INTENT_OUTPUT_BYTES_V1,
        "output",
    )?;
    let attachments = WorkerV3FinalizerReplayAttachmentsV1::new(outer, providers, transcript)
        .map_err(|e| failure("storage attachment limits", e))?;
    Ok((attachments, output))
}

fn copy_attachment(bytes: &[u8], maximum: usize, field: &'static str) -> Result<Vec<u8>> {
    check_length(bytes.len(), maximum, field)?;
    let mut copy = Vec::new();
    copy.try_reserve_exact(bytes.len())
        .map_err(|_| Resource::Allocation)?;
    if copy.capacity() > maximum {
        return Err(Error::Limit(field));
    }
    copy.extend_from_slice(bytes);
    Ok(copy)
}
fn check_length(n: usize, maximum: usize, field: &'static str) -> Result<()> {
    if n == 0 || n > maximum {
        return Err(Error::Limit(field));
    }
    Ok(())
}
fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b).ok_or(Resource::Arithmetic.into())
}
fn raw_hash(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

struct NativePlanInputs {
    package: PackageIdentityV1,
    attempt: BuildAttempt,
    slot: u8,
    transaction: [u8; 32],
    outer: ContentIdentityV1,
    binding: [u8; 32],
    source: [u8; 32],
    worker: [u8; 32],
    finalized: [u8; 32],
    transcript: [u8; 32],
    link_plan: [u8; 32],
    manifest: ContentIdentityV1,
    policy: [u8; 32],
    raw: ContentIdentityV1,
    output: ContentIdentityV1,
    descriptor: ContentIdentityV1,
    canonical_digest: [u8; 32],
}

fn derive_intent(
    package: PackageIdentityV1,
    finalized: &PreparedFinalizedNativeWorkerHsacoV1,
    transcript: &NativeWorkerCompactFinalizerReplayV1,
) -> Result<NativeWorkerPublicationIntentV1> {
    let source = finalized.source_evidence();
    let binding = source.binding();
    let receipt = binding.receipt();
    let outer = source.recovered_handoff().handoff();
    transcript.verify_outer_identity(outer.identity())?;
    if transcript.attempt() != receipt.attempt()
        || transcript.handoff_slot() != receipt.slot()
        || transcript.transaction_identity() != receipt.transaction_identity()
        || transcript.binding_identity() != binding.identity().as_bytes()
        || transcript.source_evidence_identity() != source.identity().as_bytes()
        || transcript.expected_finalization_identity() != finalized.identity().as_bytes()
    {
        return Err(Error::Mismatch("native plan/transcript coordinates"));
    }
    let manifest = outer.module_handoff().symbol_manifest().identity();
    let measurement = source.worker_measurement();
    let executable = measurement.executable();
    let worker = hash_parts(
        WORKER_DOMAIN,
        &[
            executable.sha256(),
            &executable.byte_len().to_le_bytes(),
            &(measurement.worker_build_identity().len() as u64).to_le_bytes(),
            measurement.worker_build_identity().as_bytes(),
            &(measurement.llvm_build_identity().len() as u64).to_le_bytes(),
            measurement.llvm_build_identity().as_bytes(),
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
        binding: *binding.identity().as_bytes(),
        source: *source.identity().as_bytes(),
        worker,
        finalized: *finalized.identity().as_bytes(),
        transcript: *transcript.identity().as_bytes(),
        link_plan: *source.plan().identity().as_bytes(),
        manifest: ContentIdentityV1::from_parts(*manifest.sha256(), manifest.byte_len()),
        policy: *finalized.raw_policy().identity().as_bytes(),
        raw: source.output_identity(),
        output: finalized.output_identity(),
        descriptor: finalized.descriptor_identity(),
        canonical_digest: *finalized.canonical_digest().as_bytes(),
    }))
}

fn derive_plan(input: NativePlanInputs) -> NativeWorkerPublicationIntentV1 {
    // Complete native source/Worker identity already commits all response,
    // measurement, provider and option axes. Include it with the finalization,
    // transcript, producer and full occurrence in every new native identity.
    let mut hash = Sha256::new();
    hash.update(CONTEXT_DOMAIN);
    hash.update(input.package.as_bytes());
    hash_attempt(&mut hash, input.attempt);
    hash.update([input.slot]);
    for identity in [
        input.transaction,
        input.binding,
        input.source,
        input.worker,
        input.finalized,
        input.transcript,
        input.link_plan,
        input.policy,
        input.canonical_digest,
    ] {
        hash.update(identity);
    }
    for content in [
        input.outer,
        input.manifest,
        input.raw,
        input.output,
        input.descriptor,
    ] {
        hash.update(content.sha256());
        hash.update(content.byte_len().to_le_bytes());
    }
    let context: [u8; 32] = hash.finalize().into();
    let domain = |domain: &[u8]| hash_parts(domain, &[&context]);
    // Scope and worker coordinates keep their meaning across attempts; the
    // complete request/plan/intent identities, not the scope, bind occurrence.
    let scope = LinkPublicationScopeV1::new(
        input.package,
        KernelSetIdentityV1::from_bytes(hash_parts(
            KERNEL_DOMAIN,
            &[
                input.manifest.sha256(),
                &input.manifest.byte_len().to_le_bytes(),
            ],
        )),
        TargetIdentityV1::from_bytes(hash_parts(TARGET_DOMAIN, &[&input.policy])),
    );
    let plan = DurableLinkPublicationPlanV1::new(
        input.attempt,
        scope,
        CanonicalLinkRequestIdentityV1::from_bytes(domain(REQUEST_DOMAIN)),
        PinnedWorkerIdentityV1::from_bytes(input.worker),
        ValidatedResponseIdentityV1::from_bytes(hash_parts(
            RESPONSE_DOMAIN,
            &[
                &input.source,
                input.raw.sha256(),
                &input.raw.byte_len().to_le_bytes(),
            ],
        )),
        LinkedOutputIdentityV1::from_bytes(*input.raw.sha256()),
        FinalizationIdentityV1::from_bytes(hash_parts(
            FINALIZATION_DOMAIN,
            &[
                &input.finalized,
                &input.canonical_digest,
                input.descriptor.sha256(),
                &input.descriptor.byte_len().to_le_bytes(),
                input.output.sha256(),
                &input.output.byte_len().to_le_bytes(),
            ],
        )),
        FinalizedOutputIdentityV1::from_bytes(*input.output.sha256()),
        AtomicPublicationIdentityV1::from_bytes(domain(PUBLICATION_DOMAIN)),
    );
    let plan_identity = NativeWorkerPublicationPlanIdentityV1(hash_parts(
        PLAN_DOMAIN,
        &[
            &context,
            scope.package().as_bytes(),
            scope.kernel_set().as_bytes(),
            scope.target().as_bytes(),
            plan.request().as_bytes(),
            plan.worker().as_bytes(),
            plan.response().as_bytes(),
            plan.linked_output().as_bytes(),
            plan.finalization().as_bytes(),
            plan.finalized_output().as_bytes(),
            plan.publication().as_bytes(),
        ],
    ));
    let identity = NativeWorkerPublicationIntentIdentityV1(hash_parts(
        INTENT_DOMAIN,
        &[&context, plan_identity.as_bytes()],
    ));
    NativeWorkerPublicationIntentV1 {
        identity,
        plan_identity,
        plan,
    }
}
fn hash_attempt(hash: &mut Sha256, attempt: BuildAttempt) {
    hash.update(attempt.generation().to_le_bytes());
    hash.update(attempt.session().as_bytes());
    hash.update(attempt.invocation().as_bytes());
}
fn hash_parts(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(domain);
    for part in parts {
        hash.update(part);
    }
    hash.finalize().into()
}

#[cfg(test)]
#[path = "native_worker_publication_tests.rs"]
mod tests;
