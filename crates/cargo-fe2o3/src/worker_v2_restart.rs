use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(feature = "worker-v2-fault-injection-test-only")]
use std::ffi::OsStr;

use fe2o3_artifact_transaction::{
    BackendPublicationReceiptV1, BuildAttempt, BuildSession, DurableLinkPublicationPlanV1,
    ProducerIdentity, RecoveredWorkerV2PublicationIntentV1, UpstreamCodeObjectEvidenceIdentityV1,
    WorkerV2PublicationIntentErrorV1, WorkerV2PublicationIntentIdentityV1,
    persist_worker_v2_publication_intent_v1, producer_package_identity_v1,
    recover_worker_v2_publication_intent_v1,
};
use fe2o3_compiler_ffi::CodeObjectVersion;
use fe2o3_hsaco_finalize::{
    CanonicalDescriptorSectionObservationV1, InspectedRawWorkerV2HsacoV1,
    PreparedFinalizedWorkerV2HsacoPublicationV1, PreparedWorkerV2HsacoPublicationV1,
    SealedWorkerV2HsacoPublicationIntentV1, WorkerV2HsacoFinalizationError,
    WorkerV2HsacoPublicationError, finalize_inspected_worker_v2_hsaco_v1,
    prepare_finalized_worker_v2_hsaco_publication_v1, prepare_worker_v2_hsaco_publication_v1,
};
use fe2o3_worker_v2_bundle::{
    MAX_WORKER_V2_ENVELOPE_INPUTS_BYTES, MAX_WORKER_V2_LOAD_ENVELOPE_BYTES,
    WORKER_V2_LOAD_ENVELOPE_NAME_PREFIX_V1 as ENVELOPE_PREFIX,
    WORKER_V2_LOAD_ENVELOPE_NAME_SUFFIX_V1 as ENVELOPE_SUFFIX, WorkerV2EnvelopeInputsIdentityV1,
    WorkerV2EnvelopeInputsV1, WorkerV2LoadEnvelopeIdentityV1, WorkerV2LoadEnvelopeV1,
    worker_v2_load_envelope_name_v1,
};
use rustix::fd::{FromRawFd, OwnedFd};
use rustix::fs::{
    AtFlags, FileType, FlockOperation, Mode, OFlags, RenameFlags, flock, fstat, fsync, open,
    openat, renameat, renameat_with, statat, unlinkat,
};
use sha2::{Digest, Sha256};

use crate::worker_v2_artifact_container::{
    WorkerV2ArtifactContainerAssemblyErrorV1, derive_required_worker_v2_publication_plan_v1,
};

const MARKER_MAGIC: &[u8] = b"FE2O3-CARGO-WORKER-V2-RESUME-V1\0";
const LEGACY_MARKER_VERSION: u16 = 1;
const PREVIOUS_MARKER_VERSION: u16 = 2;
const MARKER_VERSION: u16 = 3;
const MARKER_CHECKSUM_DOMAIN: &[u8] = b"FE2O3/CARGO-WORKER-V2-RESUME-CHECKSUM/V1\0";
const ADMISSION_COMMITMENT_DOMAIN: &[u8] = b"FE2O3/CARGO-WORKER-V2-ADMISSION-COMMITMENT/V1\0";
const MARKER_PREFIX: &str = ".fe2o3-cargo-worker-v2-resume-v1-";
const LOCK_SUFFIX: &str = ".lock";
const RECORD_SUFFIX: &str = ".record";
const TEMP_SUFFIX: &str = ".tmp-";
const ENVELOPE_INPUTS_PREFIX: &str = ".fe2o3-worker-v2-envelope-inputs-v1-";
const ENVELOPE_INPUTS_SUFFIX: &str = ".capsule";
const ENVELOPE_INPUTS_NAME_DOMAIN: &[u8] = b"FE2O3/WORKER-V2-ENVELOPE-INPUTS-NAME/V1\0";
const MAX_ENVELOPE_INPUT_RESIDUE_ENTRIES: usize = 256;
const MAX_ENVELOPE_TEMP_RESIDUE_ENTRIES: usize = 256;
const RECEIPT_FIELDS: usize = 7;
const PREVIOUS_MARKER_BYTES: usize =
    MARKER_MAGIC.len() + 2 + 1 + 1 + 32 + 8 + 16 + 32 + 32 + 32 + RECEIPT_FIELDS * 32 + 32;
const MARKER_BYTES: usize = PREVIOUS_MARKER_BYTES + 32 + 32;
const LEGACY_MARKER_BYTES: usize =
    MARKER_MAGIC.len() + 2 + 1 + 32 + 8 + 16 + 32 + 32 + RECEIPT_FIELDS * 32 + 32;

static NEXT_TEMP: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorkerV2EnvelopeModeV1 {
    /// Preserve the inert HSACO publication flow without claiming load or launch authority.
    NonAuthoritative,
    /// Require a canonical inert envelope before the attempt can complete.
    Required,
}

impl WorkerV2EnvelopeModeV1 {
    pub(crate) const fn is_required(self) -> bool {
        matches!(self, Self::Required)
    }

    pub(crate) const fn grants_load_authority(self) -> bool {
        false
    }

    pub(crate) const fn grants_launch_authority(self) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ReceiptRecordV1([[u8; 32]; RECEIPT_FIELDS]);

impl ReceiptRecordV1 {
    pub(crate) fn from_receipt(receipt: BackendPublicationReceiptV1) -> Self {
        Self([
            receipt.attempt_identity(),
            receipt.producer_identity(),
            receipt.scope_identity(),
            receipt.plan_commitment(),
            receipt.upstream_evidence_identity(),
            receipt.finalized_output_identity(),
            receipt.publication_identity(),
        ])
    }

    pub(crate) fn matches(self, receipt: BackendPublicationReceiptV1) -> bool {
        self == Self::from_receipt(receipt)
    }

    fn encode(self, bytes: &mut Vec<u8>) {
        for field in self.0 {
            bytes.extend_from_slice(&field);
        }
    }

    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, &'static str> {
        let mut fields = [[0_u8; 32]; RECEIPT_FIELDS];
        for field in &mut fields {
            *field = decoder.array()?;
        }
        Ok(Self(fields))
    }

    fn is_zero(self) -> bool {
        self.0.iter().all(|field| *field == [0; 32])
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorkerV2PublicationKindV1 {
    Raw,
    /// Ordinary inert COV6 publication; no load envelope was requested.
    Finalized,
    /// COV6 publication whose persisted transaction cannot complete without its load envelope.
    FinalizedEnvelopeRequired,
}

impl WorkerV2PublicationKindV1 {
    const fn tag(self) -> u8 {
        match self {
            Self::Raw => 1,
            Self::Finalized => 2,
            Self::FinalizedEnvelopeRequired => 3,
        }
    }

    const fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::Raw),
            2 => Some(Self::Finalized),
            3 => Some(Self::FinalizedEnvelopeRequired),
            _ => None,
        }
    }

    #[allow(dead_code)] // Retained for fixture and state-machine assertions.
    pub(crate) const fn is_finalized(self) -> bool {
        matches!(self, Self::Finalized | Self::FinalizedEnvelopeRequired)
    }

    pub(crate) const fn requires_envelope(self) -> bool {
        matches!(self, Self::FinalizedEnvelopeRequired)
    }
}

#[allow(clippy::large_enum_variant)] // The fixed receipt is kept inline for exact marker equality.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResumeMarkerStateV1 {
    Pending {
        legacy: bool,
        publication: WorkerV2PublicationKindV1,
        attempt: BuildAttempt,
        admission: [u8; 32],
        envelope_inputs: [u8; 32],
    },
    Ready {
        legacy: bool,
        publication: WorkerV2PublicationKindV1,
        attempt: BuildAttempt,
        admission: [u8; 32],
        envelope_inputs: [u8; 32],
        intent: WorkerV2PublicationIntentIdentityV1,
    },
    Completed {
        legacy: bool,
        publication: WorkerV2PublicationKindV1,
        attempt: BuildAttempt,
        admission: [u8; 32],
        envelope_inputs: [u8; 32],
        envelope: [u8; 32],
        intent: WorkerV2PublicationIntentIdentityV1,
        receipt: ReceiptRecordV1,
    },
}

#[derive(Debug)]
pub(crate) enum RestartIntentErrorV1 {
    Marker(ResumeMarkerErrorV1),
    Intent(WorkerV2PublicationIntentErrorV1),
    Finalization(WorkerV2HsacoFinalizationError),
    PublicationIntent(WorkerV2HsacoPublicationError),
    EnvelopeAssembly(WorkerV2ArtifactContainerAssemblyErrorV1),
    UnsupportedPublicationRoute {
        code_object_version: CodeObjectVersion,
        descriptor: CanonicalDescriptorSectionObservationV1,
    },
    IntentIdentityMismatch,
    MissingEnvelopeInputs,
    EnvelopeInputMismatch(&'static str),
}

impl fmt::Display for RestartIntentErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Marker(error) => write!(formatter, "Worker V2 resume marker failed: {error}"),
            Self::Intent(error) => {
                write!(formatter, "Worker V2 publication intent failed: {error}")
            }
            Self::Finalization(error) => {
                write!(formatter, "Worker V2 HSACO finalization failed: {error}")
            }
            Self::PublicationIntent(error) => {
                write!(
                    formatter,
                    "Worker V2 HSACO publication intent failed: {error}"
                )
            }
            Self::EnvelopeAssembly(error) => {
                write!(formatter, "Worker V2 envelope input validation failed: {error}")
            }
            Self::UnsupportedPublicationRoute {
                code_object_version,
                descriptor,
            } => write!(
                formatter,
                "Worker V2 publication rejects {code_object_version:?} with descriptor observation {descriptor:?}; only descriptor-missing COV5 raw compatibility and descriptor-bearing COV6 canonical finalization are supported"
            ),
            Self::IntentIdentityMismatch => formatter.write_str(
                "recovered Worker V2 publication intent does not match its resume marker",
            ),
            Self::MissingEnvelopeInputs => formatter.write_str(
                "canonical Worker V2 envelope inputs are missing; the compiler handoff must provide sealed direct-link evidence and per-kernel proof records",
            ),
            Self::EnvelopeInputMismatch(field) => write!(
                formatter,
                "canonical Worker V2 envelope input {field} does not match the measured worker result"
            ),
        }
    }
}

impl Error for RestartIntentErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Marker(error) => Some(error),
            Self::Intent(error) => Some(error),
            Self::Finalization(error) => Some(error),
            Self::PublicationIntent(error) => Some(error),
            Self::EnvelopeAssembly(error) => Some(error),
            Self::UnsupportedPublicationRoute { .. } => None,
            Self::IntentIdentityMismatch => None,
            Self::MissingEnvelopeInputs => None,
            Self::EnvelopeInputMismatch(_) => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorkerV2EnvelopePublicationOutcomeV1 {
    Published,
    AlreadyPublished,
}

pub(crate) struct PersistedAdmittedWorkerV2IntentV1 {
    pub(crate) intent: RecoveredWorkerV2PublicationIntentV1,
    pub(crate) publication: WorkerV2PublicationKindV1,
}

#[derive(Debug)]
pub(crate) enum PreparedWorkerV2PublicationV1 {
    Raw(Box<PreparedWorkerV2HsacoPublicationV1>),
    Finalized(Box<PreparedFinalizedWorkerV2HsacoPublicationV1>),
}

impl PreparedWorkerV2PublicationV1 {
    #[allow(dead_code)] // Retained for fixture and state-machine assertions.
    pub(crate) const fn kind(&self) -> WorkerV2PublicationKindV1 {
        match self {
            Self::Raw(_) => WorkerV2PublicationKindV1::Raw,
            Self::Finalized(_) => WorkerV2PublicationKindV1::Finalized,
        }
    }

    fn attempt(&self) -> BuildAttempt {
        match self {
            Self::Raw(prepared) => prepared.attempt(),
            Self::Finalized(prepared) => prepared.attempt(),
        }
    }

    fn intent(&self) -> SealedWorkerV2HsacoPublicationIntentV1 {
        match self {
            Self::Raw(prepared) => prepared.publication_intent(),
            Self::Finalized(prepared) => prepared.publication_intent(),
        }
    }
}

impl From<ResumeMarkerErrorV1> for RestartIntentErrorV1 {
    fn from(error: ResumeMarkerErrorV1) -> Self {
        Self::Marker(error)
    }
}

impl From<WorkerV2PublicationIntentErrorV1> for RestartIntentErrorV1 {
    fn from(error: WorkerV2PublicationIntentErrorV1) -> Self {
        Self::Intent(error)
    }
}

pub(crate) fn persist_admitted_worker_v2_intent_v1(
    store: &WorkerV2ResumeStoreV1,
    producer: &ProducerIdentity,
    inspected: InspectedRawWorkerV2HsacoV1,
    envelope_mode: WorkerV2EnvelopeModeV1,
    envelope_inputs: Option<&WorkerV2EnvelopeInputsV1>,
) -> Result<PersistedAdmittedWorkerV2IntentV1, RestartIntentErrorV1> {
    let publication = select_publication_kind_v1(
        inspected.code_object_version(),
        inspected.canonical_descriptor_section(),
        envelope_mode,
    )?;
    let (attempt, plan, upstream, exact_bytes) = match publication {
        WorkerV2PublicationKindV1::Raw => {
            if envelope_inputs.is_some() {
                return Err(RestartIntentErrorV1::MissingEnvelopeInputs);
            }
            let prepared = PreparedWorkerV2PublicationV1::Raw(Box::new(
                prepare_worker_v2_hsaco_publication_v1(producer, inspected)
                    .map_err(RestartIntentErrorV1::PublicationIntent)?,
            ));
            let intent = prepared.intent();
            (
                prepared.attempt(),
                intent.durable_plan(),
                intent.upstream_evidence(),
                prepared.exact_bytes().to_vec(),
            )
        }
        WorkerV2PublicationKindV1::Finalized => {
            if envelope_inputs.is_some() {
                return Err(RestartIntentErrorV1::MissingEnvelopeInputs);
            }
            let finalized = finalize_inspected_worker_v2_hsaco_v1(inspected)
                .map_err(RestartIntentErrorV1::Finalization)?;
            let prepared = PreparedWorkerV2PublicationV1::Finalized(Box::new(
                prepare_finalized_worker_v2_hsaco_publication_v1(producer, finalized)
                    .map_err(RestartIntentErrorV1::PublicationIntent)?,
            ));
            let intent = prepared.intent();
            (
                prepared.attempt(),
                intent.durable_plan(),
                intent.upstream_evidence(),
                prepared.exact_bytes().to_vec(),
            )
        }
        WorkerV2PublicationKindV1::FinalizedEnvelopeRequired => {
            let inputs = envelope_inputs.ok_or(RestartIntentErrorV1::MissingEnvelopeInputs)?;
            let finalized = finalize_inspected_worker_v2_hsaco_v1(inspected)
                .map_err(RestartIntentErrorV1::Finalization)?;
            if !finalized
                .raw_output_identity()
                .matches(inputs.raw_hsaco().bytes())
            {
                return Err(RestartIntentErrorV1::EnvelopeInputMismatch("raw HSACO"));
            }
            let attempt = finalized.attempt();
            let exact_bytes = finalized.exact_finalized_bytes().to_vec();
            let (plan, upstream) = derive_required_worker_v2_publication_plan_v1(
                producer,
                attempt,
                &exact_bytes,
                inputs,
            )
            .map_err(RestartIntentErrorV1::EnvelopeAssembly)?;
            (attempt, plan, upstream, exact_bytes)
        }
    };
    let envelope_inputs_identity = match (publication.requires_envelope(), envelope_inputs) {
        (true, Some(inputs)) => Some(inputs.identity()),
        (true, None) | (false, Some(_)) => return Err(RestartIntentErrorV1::MissingEnvelopeInputs),
        (false, None) => None,
    };
    let admission = restart_admission_commitment_with_inputs_v1(
        publication,
        plan,
        upstream,
        &exact_bytes,
        envelope_inputs_identity,
    );
    // A required marker is recoverable only after its exact capsule name and bytes are durable.
    if let Some(inputs) = envelope_inputs {
        store.persist_envelope_inputs(attempt, inputs)?;
        #[cfg(feature = "worker-v2-fault-injection-test-only")]
        injected_fault_point_v1("envelope-inputs-persisted");
    }
    store.persist_pending_with_envelope_inputs(
        publication,
        attempt,
        admission,
        envelope_inputs_identity,
    )?;
    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    injected_fault_point_v1("pending-marker");
    store.verify_output_path()?;
    let persisted = persist_worker_v2_publication_intent_v1(
        &store.display_path,
        producer,
        attempt,
        plan,
        upstream,
        &exact_bytes,
    )?;
    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    injected_fault_point_v1("pending-intent");
    store.verify_output_path()?;
    store.persist_ready(publication, attempt, persisted.record().identity())?;
    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    injected_fault_point_v1("ready");
    Ok(PersistedAdmittedWorkerV2IntentV1 {
        intent: persisted,
        publication,
    })
}

#[cfg(feature = "worker-v2-fault-injection-test-only")]
pub(crate) fn injected_fault_point_v1(point: &str) {
    const ENV: &str = "FE2O3_TEST_WORKER_V2_FAULT_POINT_V1";
    if std::env::var_os(ENV).as_deref() == Some(OsStr::new(point)) {
        std::process::exit(86);
    }
}

fn select_publication_kind_v1(
    code_object_version: CodeObjectVersion,
    descriptor: CanonicalDescriptorSectionObservationV1,
    envelope_mode: WorkerV2EnvelopeModeV1,
) -> Result<WorkerV2PublicationKindV1, RestartIntentErrorV1> {
    match (code_object_version, descriptor) {
        (CodeObjectVersion::V5, CanonicalDescriptorSectionObservationV1::Missing)
            if !envelope_mode.is_required() =>
        {
            Ok(WorkerV2PublicationKindV1::Raw)
        }
        (
            CodeObjectVersion::V6,
            CanonicalDescriptorSectionObservationV1::PresentButNotFinalizedByThisInspection,
        ) if envelope_mode.is_required() => {
            Ok(WorkerV2PublicationKindV1::FinalizedEnvelopeRequired)
        }
        (
            CodeObjectVersion::V6,
            CanonicalDescriptorSectionObservationV1::PresentButNotFinalizedByThisInspection,
        ) => Ok(WorkerV2PublicationKindV1::Finalized),
        (code_object_version, descriptor) => {
            Err(RestartIntentErrorV1::UnsupportedPublicationRoute {
                code_object_version,
                descriptor,
            })
        }
    }
}

pub(crate) fn recover_worker_v2_intent_v1(
    store: &WorkerV2ResumeStoreV1,
    producer: &ProducerIdentity,
    state: ResumeMarkerStateV1,
) -> Result<RecoveredWorkerV2PublicationIntentV1, RestartIntentErrorV1> {
    let attempt = state.attempt();
    let envelope_inputs_identity = if state.publication().requires_envelope() {
        let identity = store.recover_envelope_inputs(attempt)?.identity();
        if state.envelope_inputs() != identity.as_bytes() {
            return Err(RestartIntentErrorV1::IntentIdentityMismatch);
        }
        Some(identity)
    } else {
        None
    };
    store.verify_output_path()?;
    let recovered =
        recover_worker_v2_publication_intent_v1(&store.display_path, producer, attempt)?;
    store.verify_output_path()?;
    if let Some(expected) = state.intent()
        && recovered.record().identity() != expected
    {
        return Err(RestartIntentErrorV1::IntentIdentityMismatch);
    }
    let current_admission = restart_admission_commitment_with_inputs_v1(
        state.publication(),
        recovered.record().plan(),
        recovered.record().upstream_evidence(),
        recovered.exact_output(),
        envelope_inputs_identity,
    );
    if state.is_legacy() {
        if state.publication() != WorkerV2PublicationKindV1::Raw {
            return Err(RestartIntentErrorV1::IntentIdentityMismatch);
        }
        if matches!(state, ResumeMarkerStateV1::Pending { .. })
            && legacy_restart_admission_commitment_v1(
                recovered.record().plan(),
                recovered.record().upstream_evidence(),
                recovered.exact_output(),
            ) != state.admission()
        {
            return Err(RestartIntentErrorV1::IntentIdentityMismatch);
        }
        if !matches!(state, ResumeMarkerStateV1::Completed { .. }) {
            store.migrate_legacy_to_ready(
                state,
                current_admission,
                recovered.record().identity(),
            )?;
        }
    } else if current_admission != state.admission() {
        return Err(RestartIntentErrorV1::IntentIdentityMismatch);
    } else if matches!(state, ResumeMarkerStateV1::Pending { .. }) {
        store.persist_ready(state.publication(), attempt, recovered.record().identity())?;
    }
    Ok(recovered)
}

#[allow(dead_code)] // Ordinary fixture staging uses the no-capsule contract directly.
pub(crate) fn restart_admission_commitment_v1(
    publication: WorkerV2PublicationKindV1,
    plan: DurableLinkPublicationPlanV1,
    upstream: UpstreamCodeObjectEvidenceIdentityV1,
    output: &[u8],
) -> [u8; 32] {
    restart_admission_commitment_with_inputs_v1(publication, plan, upstream, output, None)
}

pub(crate) fn restart_admission_commitment_with_inputs_v1(
    publication: WorkerV2PublicationKindV1,
    plan: DurableLinkPublicationPlanV1,
    upstream: UpstreamCodeObjectEvidenceIdentityV1,
    output: &[u8],
    envelope_inputs: Option<WorkerV2EnvelopeInputsIdentityV1>,
) -> [u8; 32] {
    hash_identity(ADMISSION_COMMITMENT_DOMAIN, |digest| {
        digest.update([publication.tag()]);
        let attempt = plan.attempt();
        digest.update(attempt.generation().to_le_bytes());
        digest.update(attempt.session().as_bytes());
        digest.update(attempt.invocation().as_bytes());
        digest.update(plan.scope().package().as_bytes());
        digest.update(plan.scope().kernel_set().as_bytes());
        digest.update(plan.scope().target().as_bytes());
        digest.update(plan.request().as_bytes());
        digest.update(plan.worker().as_bytes());
        digest.update(plan.response().as_bytes());
        digest.update(plan.linked_output().as_bytes());
        digest.update(plan.finalization().as_bytes());
        digest.update(plan.finalized_output().as_bytes());
        digest.update(plan.publication().as_bytes());
        digest.update(upstream.as_bytes());
        digest.update(Sha256::digest(output));
        digest.update((output.len() as u64).to_le_bytes());
        if let Some(identity) = envelope_inputs {
            digest.update([1]);
            digest.update(identity.as_bytes());
        }
    })
}

fn legacy_restart_admission_commitment_v1(
    plan: DurableLinkPublicationPlanV1,
    upstream: UpstreamCodeObjectEvidenceIdentityV1,
    output: &[u8],
) -> [u8; 32] {
    hash_identity(ADMISSION_COMMITMENT_DOMAIN, |digest| {
        let attempt = plan.attempt();
        digest.update(attempt.generation().to_le_bytes());
        digest.update(attempt.session().as_bytes());
        digest.update(attempt.invocation().as_bytes());
        digest.update(plan.scope().package().as_bytes());
        digest.update(plan.scope().kernel_set().as_bytes());
        digest.update(plan.scope().target().as_bytes());
        digest.update(plan.request().as_bytes());
        digest.update(plan.worker().as_bytes());
        digest.update(plan.response().as_bytes());
        digest.update(plan.linked_output().as_bytes());
        digest.update(plan.finalization().as_bytes());
        digest.update(plan.finalized_output().as_bytes());
        digest.update(plan.publication().as_bytes());
        digest.update(upstream.as_bytes());
        digest.update(Sha256::digest(output));
        digest.update((output.len() as u64).to_le_bytes());
    })
}

impl PreparedWorkerV2PublicationV1 {
    pub(crate) fn exact_bytes(&self) -> &[u8] {
        match self {
            Self::Raw(prepared) => prepared.exact_bytes(),
            Self::Finalized(prepared) => prepared.exact_finalized_bytes(),
        }
    }
}

fn hash_identity(domain: &[u8], update: impl FnOnce(&mut Sha256)) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    update(&mut digest);
    digest.finalize().into()
}

pub(super) fn envelope_name(publication_identity: [u8; 32]) -> String {
    worker_v2_load_envelope_name_v1(publication_identity)
}

fn envelope_inputs_name(package: [u8; 32], attempt: BuildAttempt) -> String {
    let identity = hash_identity(ENVELOPE_INPUTS_NAME_DOMAIN, |digest| {
        digest.update(package);
        digest.update(attempt.generation().to_le_bytes());
        digest.update(attempt.session().as_bytes());
        digest.update(attempt.invocation().as_bytes());
    });
    format!(
        "{ENVELOPE_INPUTS_PREFIX}{}-{}{ENVELOPE_INPUTS_SUFFIX}",
        hex(&package),
        hex(&identity)
    )
}

fn envelope_inputs_package_prefix(package: [u8; 32]) -> String {
    format!("{ENVELOPE_INPUTS_PREFIX}{}-", hex(&package))
}

fn envelope_temp_package_prefix(package: [u8; 32]) -> String {
    format!("{ENVELOPE_PREFIX}{}-", hex(&package))
}

fn envelope_temp_name(
    package: [u8; 32],
    publication_identity: [u8; 32],
    process: u32,
    counter: u64,
) -> String {
    format!(
        "{}{}{ENVELOPE_SUFFIX}{TEMP_SUFFIX}{process}-{counter}",
        envelope_temp_package_prefix(package),
        hex(&publication_identity)
    )
}

fn is_canonical_envelope_inputs_name(name: &str, package_prefix: &str) -> bool {
    is_canonical_envelope_inputs_name_bytes(name.as_bytes(), package_prefix)
}

fn is_canonical_envelope_inputs_name_bytes(bytes: &[u8], package_prefix: &str) -> bool {
    let digest_start = package_prefix.len();
    let digest_end = digest_start + 64;
    bytes.len() == digest_end + ENVELOPE_INPUTS_SUFFIX.len()
        && bytes.starts_with(package_prefix.as_bytes())
        && bytes[digest_start..digest_end]
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        && &bytes[digest_end..] == ENVELOPE_INPUTS_SUFFIX.as_bytes()
}

fn is_envelope_inputs_temp_name(name: &str, package_prefix: &str) -> bool {
    let bytes = name.as_bytes();
    let canonical_len = package_prefix.len() + 64 + ENVELOPE_INPUTS_SUFFIX.len();
    if bytes.len() <= canonical_len + TEMP_SUFFIX.len()
        || !is_canonical_envelope_inputs_name_bytes(&bytes[..canonical_len], package_prefix)
        || &bytes[canonical_len..canonical_len + TEMP_SUFFIX.len()] != TEMP_SUFFIX.as_bytes()
    {
        return false;
    }
    has_decimal_temp_counters(&bytes[canonical_len + TEMP_SUFFIX.len()..])
}

fn is_envelope_temp_name(name: &str, package_prefix: &str) -> bool {
    let bytes = name.as_bytes();
    let identity_start = package_prefix.len();
    let identity_end = identity_start + 64;
    let suffix_end = identity_end + ENVELOPE_SUFFIX.len();
    let temp_end = suffix_end + TEMP_SUFFIX.len();
    bytes.len() > temp_end
        && bytes.starts_with(package_prefix.as_bytes())
        && bytes[identity_start..identity_end]
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        && &bytes[identity_end..suffix_end] == ENVELOPE_SUFFIX.as_bytes()
        && &bytes[suffix_end..temp_end] == TEMP_SUFFIX.as_bytes()
        && has_decimal_temp_counters(&bytes[temp_end..])
}

fn has_decimal_temp_counters(counters: &[u8]) -> bool {
    let Some(separator) = counters.iter().position(|byte| *byte == b'-') else {
        return false;
    };
    is_canonical_decimal(&counters[..separator]) && is_canonical_decimal(&counters[separator + 1..])
}

fn is_canonical_decimal(bytes: &[u8]) -> bool {
    !bytes.is_empty()
        && bytes.iter().all(u8::is_ascii_digit)
        && (bytes.len() == 1 || bytes[0] != b'0')
}

impl ResumeMarkerStateV1 {
    pub(crate) const fn attempt(self) -> BuildAttempt {
        match self {
            Self::Pending { attempt, .. }
            | Self::Ready { attempt, .. }
            | Self::Completed { attempt, .. } => attempt,
        }
    }

    pub(crate) const fn intent(self) -> Option<WorkerV2PublicationIntentIdentityV1> {
        match self {
            Self::Pending { .. } => None,
            Self::Ready { intent, .. } | Self::Completed { intent, .. } => Some(intent),
        }
    }

    pub(crate) const fn publication(self) -> WorkerV2PublicationKindV1 {
        match self {
            Self::Pending { publication, .. }
            | Self::Ready { publication, .. }
            | Self::Completed { publication, .. } => publication,
        }
    }

    pub(crate) const fn admission(self) -> [u8; 32] {
        match self {
            Self::Pending { admission, .. }
            | Self::Ready { admission, .. }
            | Self::Completed { admission, .. } => admission,
        }
    }

    pub(crate) const fn envelope_inputs(self) -> [u8; 32] {
        match self {
            Self::Pending {
                envelope_inputs, ..
            }
            | Self::Ready {
                envelope_inputs, ..
            }
            | Self::Completed {
                envelope_inputs, ..
            } => envelope_inputs,
        }
    }

    pub(crate) const fn envelope(self) -> [u8; 32] {
        match self {
            Self::Pending { .. } | Self::Ready { .. } => [0; 32],
            Self::Completed { envelope, .. } => envelope,
        }
    }

    pub(crate) const fn is_legacy(self) -> bool {
        match self {
            Self::Pending { legacy, .. }
            | Self::Ready { legacy, .. }
            | Self::Completed { legacy, .. } => legacy,
        }
    }
}

#[derive(Debug)]
pub(crate) enum ResumeMarkerErrorV1 {
    Io(std::io::Error),
    OutputDirectoryChanged(PathBuf),
    InvalidMarker { path: PathBuf, reason: String },
    ConflictingMarker,
    InvalidTransition,
    StaleInvocation,
}

impl fmt::Display for ResumeMarkerErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::OutputDirectoryChanged(path) => write!(
                formatter,
                "Worker V2 resume output directory changed: {}",
                path.display()
            ),
            Self::InvalidMarker { path, reason } => write!(
                formatter,
                "invalid Worker V2 resume marker {}: {reason}",
                path.display()
            ),
            Self::ConflictingMarker => formatter
                .write_str("a different Worker V2 resume marker already exists for this producer"),
            Self::InvalidTransition => {
                formatter.write_str("invalid Worker V2 resume marker state transition")
            }
            Self::StaleInvocation => formatter.write_str(
                "Worker V2 resume marker belongs to a different build session or invocation",
            ),
        }
    }
}

impl Error for ResumeMarkerErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<std::io::Error> for ResumeMarkerErrorV1 {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

pub(crate) struct WorkerV2ResumeStoreV1 {
    directory: OwnedFd,
    _lock: OwnedFd,
    display_path: PathBuf,
    device: u64,
    inode: u64,
    package: [u8; 32],
    marker_name: String,
}

impl WorkerV2ResumeStoreV1 {
    pub(crate) fn open(
        output_dir: &Path,
        producer: &ProducerIdentity,
    ) -> Result<Self, ResumeMarkerErrorV1> {
        let directory = open_output_directory(output_dir, true)?;
        let directory_stat = fstat(&directory).map_err(std::io::Error::from)?;
        if FileType::from_raw_mode(directory_stat.st_mode) != FileType::Directory {
            return Err(Self::invalid_at(
                output_dir,
                "output path is not a directory",
            ));
        }

        let package = *producer_package_identity_v1(producer).as_bytes();
        let stem = format!("{MARKER_PREFIX}{}", hex(&package));
        let lock_name = format!("{stem}{LOCK_SUFFIX}");
        let marker_name = format!("{stem}{RECORD_SUFFIX}");
        let lock = openat(
            &directory,
            &lock_name,
            OFlags::RDWR | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )
        .map_err(std::io::Error::from)?;
        validate_private_file(&directory, &lock, &lock_name, output_dir, None)?;
        flock(&lock, FlockOperation::LockExclusive).map_err(std::io::Error::from)?;
        validate_private_file(&directory, &lock, &lock_name, output_dir, None)?;

        let store = Self {
            directory,
            _lock: lock,
            display_path: output_dir.to_path_buf(),
            device: directory_stat.st_dev,
            inode: directory_stat.st_ino,
            package,
            marker_name,
        };
        store.verify_output_path()?;
        store.cleanup_envelope_input_residue()?;
        store.cleanup_envelope_temp_residue()?;
        Ok(store)
    }

    fn cleanup_envelope_input_residue(&self) -> Result<(), ResumeMarkerErrorV1> {
        let retained = self.load()?.and_then(|state| {
            state
                .publication()
                .requires_envelope()
                .then(|| envelope_inputs_name(self.package, state.attempt()))
        });
        let package_prefix = envelope_inputs_package_prefix(self.package);
        let scan =
            rustix::io::fcntl_dupfd_cloexec(&self.directory, 0).map_err(std::io::Error::from)?;
        let mut directory = rustix::fs::Dir::read_from(&scan).map_err(std::io::Error::from)?;
        let mut residue = Vec::new();
        for entry in &mut directory {
            let entry = entry.map_err(std::io::Error::from)?;
            let bytes = entry.file_name().to_bytes();
            if !bytes.starts_with(package_prefix.as_bytes()) {
                continue;
            }
            let name = std::str::from_utf8(bytes)
                .map_err(|_| self.invalid("envelope input residue name is not UTF-8"))?;
            let canonical = is_canonical_envelope_inputs_name(name, &package_prefix);
            let temporary = is_envelope_inputs_temp_name(name, &package_prefix);
            if !canonical && !temporary {
                return Err(self.invalid_at_name(name, "malformed package-owned capsule name"));
            }
            if canonical && retained.as_deref() == Some(name) {
                continue;
            }
            if residue.len() == MAX_ENVELOPE_INPUT_RESIDUE_ENTRIES {
                return Err(self.invalid("too many package-owned envelope input residues"));
            }
            let descriptor = openat(
                &self.directory,
                name,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
                Mode::empty(),
            )
            .map_err(std::io::Error::from)?;
            validate_private_file(&self.directory, &descriptor, name, &self.display_path, None)?;
            residue.push(name.to_owned());
        }
        if !residue.is_empty() {
            for name in residue {
                unlinkat(&self.directory, &name, AtFlags::empty()).map_err(std::io::Error::from)?;
            }
            fsync(&self.directory).map_err(std::io::Error::from)?;
            self.verify_output_path()?;
        }
        Ok(())
    }

    fn cleanup_envelope_temp_residue(&self) -> Result<(), ResumeMarkerErrorV1> {
        let package_prefix = envelope_temp_package_prefix(self.package);
        let scan =
            rustix::io::fcntl_dupfd_cloexec(&self.directory, 0).map_err(std::io::Error::from)?;
        let mut directory = rustix::fs::Dir::read_from(&scan).map_err(std::io::Error::from)?;
        let mut residue = Vec::new();
        for entry in &mut directory {
            let entry = entry.map_err(std::io::Error::from)?;
            let bytes = entry.file_name().to_bytes();
            if !bytes.starts_with(package_prefix.as_bytes()) {
                continue;
            }
            let name = std::str::from_utf8(bytes)
                .map_err(|_| self.invalid("load-envelope temp residue name is not UTF-8"))?;
            if !is_envelope_temp_name(name, &package_prefix) {
                return Err(
                    self.invalid_at_name(name, "malformed package-owned load-envelope temp name")
                );
            }
            if residue.len() == MAX_ENVELOPE_TEMP_RESIDUE_ENTRIES {
                return Err(self.invalid("too many package-owned load-envelope temp residues"));
            }
            let descriptor = openat(
                &self.directory,
                name,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
                Mode::empty(),
            )
            .map_err(std::io::Error::from)?;
            validate_private_file(&self.directory, &descriptor, name, &self.display_path, None)?;
            residue.push(name.to_owned());
        }
        if !residue.is_empty() {
            for name in residue {
                unlinkat(&self.directory, &name, AtFlags::empty()).map_err(std::io::Error::from)?;
            }
            fsync(&self.directory).map_err(std::io::Error::from)?;
            self.verify_output_path()?;
        }
        Ok(())
    }

    pub(crate) fn verify_output_path(&self) -> Result<(), ResumeMarkerErrorV1> {
        let reopened = open_output_directory(&self.display_path, false)?;
        let stat = fstat(&reopened).map_err(std::io::Error::from)?;
        if stat.st_dev != self.device || stat.st_ino != self.inode {
            return Err(ResumeMarkerErrorV1::OutputDirectoryChanged(
                self.display_path.clone(),
            ));
        }
        Ok(())
    }

    pub(crate) fn persist_envelope_inputs(
        &self,
        attempt: BuildAttempt,
        inputs: &WorkerV2EnvelopeInputsV1,
    ) -> Result<(), ResumeMarkerErrorV1> {
        self.verify_output_path()?;
        let bytes = inputs.to_bytes();
        if bytes.is_empty() || bytes.len() > MAX_WORKER_V2_ENVELOPE_INPUTS_BYTES {
            return Err(self.invalid("envelope input capsule exceeds its canonical bound"));
        }
        let decoded = WorkerV2EnvelopeInputsV1::from_bytes(&bytes)
            .map_err(|error| self.invalid(format!("envelope input capsule is invalid: {error}")))?;
        if decoded != *inputs {
            return Err(self.invalid("envelope input capsule changed during canonical encoding"));
        }
        let name = envelope_inputs_name(self.package, attempt);
        if let Some(existing) = self.read_envelope_inputs(&name)? {
            return if existing == *inputs {
                Ok(())
            } else {
                Err(self.invalid_at_name(&name, "conflicting envelope input capsule"))
            };
        }

        let temp_name = format!(
            "{name}{TEMP_SUFFIX}{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        );
        let result = (|| {
            let descriptor = openat(
                &self.directory,
                &temp_name,
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::RUSR | Mode::WUSR,
            )
            .map_err(std::io::Error::from)?;
            let mut file = File::from(descriptor);
            file.set_len(bytes.len() as u64)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            validate_private_file(
                &self.directory,
                &file,
                &temp_name,
                &self.display_path,
                Some(bytes.len()),
            )?;
            #[cfg(feature = "worker-v2-fault-injection-test-only")]
            injected_fault_point_v1("envelope-inputs-temp-synced");
            self.verify_output_path()?;
            match renameat_with(
                &self.directory,
                &temp_name,
                &self.directory,
                &name,
                RenameFlags::NOREPLACE,
            ) {
                Ok(()) => {}
                Err(rustix::io::Errno::EXIST) => {
                    unlinkat(&self.directory, &temp_name, AtFlags::empty())
                        .map_err(std::io::Error::from)?;
                    let existing = self.read_envelope_inputs(&name)?.ok_or_else(|| {
                        self.invalid_at_name(&name, "capsule disappeared after create-new conflict")
                    })?;
                    return if existing == *inputs {
                        Ok(())
                    } else {
                        Err(self.invalid_at_name(&name, "conflicting envelope input capsule"))
                    };
                }
                Err(error) => return Err(std::io::Error::from(error).into()),
            }
            fsync(&self.directory).map_err(std::io::Error::from)?;
            self.verify_output_path()?;
            let published = self.read_envelope_inputs(&name)?.ok_or_else(|| {
                self.invalid_at_name(&name, "capsule is absent after durable publication")
            })?;
            if published != *inputs {
                return Err(self.invalid_at_name(&name, "capsule changed after publication"));
            }
            Ok(())
        })();
        if result.is_err() {
            let _ = unlinkat(&self.directory, &temp_name, AtFlags::empty());
        }
        result
    }

    pub(crate) fn recover_envelope_inputs(
        &self,
        attempt: BuildAttempt,
    ) -> Result<WorkerV2EnvelopeInputsV1, ResumeMarkerErrorV1> {
        let name = envelope_inputs_name(self.package, attempt);
        self.read_envelope_inputs(&name)?.ok_or_else(|| {
            self.invalid_at_name(&name, "canonical envelope input capsule is missing")
        })
    }

    fn read_envelope_inputs(
        &self,
        name: &str,
    ) -> Result<Option<WorkerV2EnvelopeInputsV1>, ResumeMarkerErrorV1> {
        self.verify_output_path()?;
        let descriptor = match openat(
            &self.directory,
            name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::empty(),
        ) {
            Ok(descriptor) => descriptor,
            Err(rustix::io::Errno::NOENT) => return Ok(None),
            Err(error) => return Err(std::io::Error::from(error).into()),
        };
        validate_private_file(&self.directory, &descriptor, name, &self.display_path, None)?;
        let initial = fstat(&descriptor).map_err(std::io::Error::from)?;
        let initial_size = usize::try_from(initial.st_size).ok();
        if initial_size.is_none_or(|size| size == 0 || size > MAX_WORKER_V2_ENVELOPE_INPUTS_BYTES) {
            return Err(self.invalid_at_name(name, "capsule size exceeds its canonical bound"));
        }
        let mut file = File::from(descriptor);
        let mut bytes = Vec::with_capacity(initial_size.unwrap_or(0).saturating_add(1));
        Read::by_ref(&mut file)
            .take((MAX_WORKER_V2_ENVELOPE_INPUTS_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
        let final_stat = fstat(&file).map_err(std::io::Error::from)?;
        if final_stat.st_dev != initial.st_dev
            || final_stat.st_ino != initial.st_ino
            || final_stat.st_mode != initial.st_mode
            || final_stat.st_nlink != 1
            || final_stat.st_mtime != initial.st_mtime
            || final_stat.st_mtime_nsec != initial.st_mtime_nsec
            || final_stat.st_ctime != initial.st_ctime
            || final_stat.st_ctime_nsec != initial.st_ctime_nsec
            || usize::try_from(final_stat.st_size).ok() != Some(bytes.len())
            || bytes.len() > MAX_WORKER_V2_ENVELOPE_INPUTS_BYTES
        {
            return Err(self.invalid_at_name(name, "capsule changed while it was read"));
        }
        let inputs = WorkerV2EnvelopeInputsV1::from_bytes(&bytes)
            .map_err(|error| self.invalid_at_name(name, format!("invalid capsule: {error}")))?;
        if inputs.to_bytes() != bytes {
            return Err(self.invalid_at_name(name, "capsule encoding is not canonical"));
        }
        Ok(Some(inputs))
    }

    pub(crate) fn publish_load_envelope(
        &self,
        envelope: &WorkerV2LoadEnvelopeV1,
    ) -> Result<WorkerV2EnvelopePublicationOutcomeV1, ResumeMarkerErrorV1> {
        self.verify_output_path()?;
        if envelope
            .published_claim()
            .plan()
            .scope()
            .package()
            .as_bytes()
            != &self.package
        {
            return Err(self.invalid("load envelope belongs to another producer package"));
        }
        let bytes = envelope.to_bytes();
        let decoded = WorkerV2LoadEnvelopeV1::from_bytes(&bytes)
            .map_err(|error| self.invalid(format!("envelope is not canonical: {error}")))?;
        if &decoded != envelope {
            return Err(self.invalid("envelope changed during canonical serialization"));
        }
        let receipt = envelope.published_claim().receipt();
        let name = envelope_name(receipt.publication_identity());
        if let Some(existing) = self.read_load_envelope(&name, receipt)? {
            return if existing == *envelope {
                Ok(WorkerV2EnvelopePublicationOutcomeV1::AlreadyPublished)
            } else {
                Err(self.invalid("conflicting canonical envelope publication"))
            };
        }

        let temp_name = envelope_temp_name(
            self.package,
            receipt.publication_identity(),
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed),
        );
        let result = (|| {
            let descriptor = openat(
                &self.directory,
                &temp_name,
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::RUSR | Mode::WUSR,
            )
            .map_err(std::io::Error::from)?;
            let mut file = File::from(descriptor);
            file.set_len(bytes.len() as u64)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            validate_private_file(
                &self.directory,
                &file,
                &temp_name,
                &self.display_path,
                Some(bytes.len()),
            )?;
            #[cfg(feature = "worker-v2-fault-injection-test-only")]
            injected_fault_point_v1("envelope-temp-synced");
            self.verify_output_path()?;
            match renameat_with(
                &self.directory,
                &temp_name,
                &self.directory,
                &name,
                RenameFlags::NOREPLACE,
            ) {
                Ok(()) => {}
                Err(rustix::io::Errno::EXIST) => {
                    unlinkat(&self.directory, &temp_name, AtFlags::empty())
                        .map_err(std::io::Error::from)?;
                    let existing = self.read_load_envelope(&name, receipt)?.ok_or_else(|| {
                        self.invalid("envelope disappeared after create-new conflict")
                    })?;
                    return if existing == *envelope {
                        Ok(WorkerV2EnvelopePublicationOutcomeV1::AlreadyPublished)
                    } else {
                        Err(self.invalid("conflicting canonical envelope publication"))
                    };
                }
                Err(error) => return Err(std::io::Error::from(error).into()),
            }
            fsync(&self.directory).map_err(std::io::Error::from)?;
            self.verify_output_path()?;
            #[cfg(feature = "worker-v2-fault-injection-test-only")]
            injected_fault_point_v1("envelope-published");
            let published = self.read_load_envelope(&name, receipt)?.ok_or_else(|| {
                self.invalid("envelope is absent after durable create-new publication")
            })?;
            if published != *envelope {
                return Err(self.invalid("envelope changed after durable publication"));
            }
            Ok(WorkerV2EnvelopePublicationOutcomeV1::Published)
        })();
        if result.is_err() {
            let _ = unlinkat(&self.directory, &temp_name, AtFlags::empty());
        }
        result
    }

    pub(crate) fn recover_load_envelope(
        &self,
        receipt: BackendPublicationReceiptV1,
    ) -> Result<WorkerV2LoadEnvelopeV1, ResumeMarkerErrorV1> {
        let name = envelope_name(receipt.publication_identity());
        self.read_load_envelope(&name, receipt)?
            .ok_or_else(|| self.invalid("canonical Worker V2 load envelope is missing"))
    }

    fn read_load_envelope(
        &self,
        name: &str,
        receipt: BackendPublicationReceiptV1,
    ) -> Result<Option<WorkerV2LoadEnvelopeV1>, ResumeMarkerErrorV1> {
        self.verify_output_path()?;
        let descriptor = match openat(
            &self.directory,
            name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::empty(),
        ) {
            Ok(descriptor) => descriptor,
            Err(rustix::io::Errno::NOENT) => return Ok(None),
            Err(error) => return Err(std::io::Error::from(error).into()),
        };
        validate_private_file(&self.directory, &descriptor, name, &self.display_path, None)?;
        let initial = fstat(&descriptor).map_err(std::io::Error::from)?;
        let mut file = File::from(descriptor);
        let mut bytes = Vec::new();
        Read::by_ref(&mut file)
            .take((MAX_WORKER_V2_LOAD_ENVELOPE_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
        let final_stat = fstat(&file).map_err(std::io::Error::from)?;
        if final_stat.st_dev != initial.st_dev
            || final_stat.st_ino != initial.st_ino
            || final_stat.st_mode != initial.st_mode
            || final_stat.st_nlink != 1
            || final_stat.st_mtime != initial.st_mtime
            || final_stat.st_mtime_nsec != initial.st_mtime_nsec
            || final_stat.st_ctime != initial.st_ctime
            || final_stat.st_ctime_nsec != initial.st_ctime_nsec
            || usize::try_from(final_stat.st_size).ok() != Some(bytes.len())
            || bytes.len() > MAX_WORKER_V2_LOAD_ENVELOPE_BYTES
        {
            return Err(self.invalid_at_name(name, "envelope changed while it was read"));
        }
        let envelope = WorkerV2LoadEnvelopeV1::from_bytes(&bytes)
            .map_err(|error| self.invalid_at_name(name, format!("invalid envelope: {error}")))?;
        if envelope.to_bytes() != bytes {
            return Err(self.invalid_at_name(name, "envelope encoding is not canonical"));
        }
        if envelope
            .published_claim()
            .plan()
            .scope()
            .package()
            .as_bytes()
            != &self.package
        {
            return Err(self.invalid_at_name(name, "envelope producer package was substituted"));
        }
        if envelope.published_claim().receipt() != receipt {
            return Err(self.invalid_at_name(name, "envelope publication receipt was substituted"));
        }
        Ok(Some(envelope))
    }

    pub(crate) fn load(&self) -> Result<Option<ResumeMarkerStateV1>, ResumeMarkerErrorV1> {
        self.verify_output_path()?;
        let descriptor = match openat(
            &self.directory,
            &self.marker_name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::empty(),
        ) {
            Ok(descriptor) => descriptor,
            Err(rustix::io::Errno::NOENT) => return Ok(None),
            Err(error) => return Err(std::io::Error::from(error).into()),
        };
        validate_private_file(
            &self.directory,
            &descriptor,
            &self.marker_name,
            &self.display_path,
            None,
        )?;
        let initial = fstat(&descriptor).map_err(std::io::Error::from)?;
        let mut file = File::from(descriptor);
        let mut bytes = Vec::with_capacity(MARKER_BYTES + 1);
        Read::by_ref(&mut file)
            .take((MARKER_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
        let final_stat = fstat(&file).map_err(std::io::Error::from)?;
        let canonical_size = usize::try_from(final_stat.st_size).is_ok_and(|size| {
            size == MARKER_BYTES || size == PREVIOUS_MARKER_BYTES || size == LEGACY_MARKER_BYTES
        });
        if final_stat.st_dev != initial.st_dev
            || final_stat.st_ino != initial.st_ino
            || final_stat.st_mode != initial.st_mode
            || final_stat.st_nlink != 1
            || final_stat.st_mtime != initial.st_mtime
            || final_stat.st_mtime_nsec != initial.st_mtime_nsec
            || final_stat.st_ctime != initial.st_ctime
            || final_stat.st_ctime_nsec != initial.st_ctime_nsec
            || usize::try_from(final_stat.st_size).ok() != Some(bytes.len())
            || !canonical_size
        {
            return Err(self.invalid("marker changed while it was read"));
        }
        decode_marker(&bytes, self.package)
            .map(Some)
            .map_err(|reason| self.invalid(reason))
    }

    #[allow(dead_code)] // Ordinary fixture staging uses the no-capsule transition directly.
    pub(crate) fn persist_pending(
        &self,
        publication: WorkerV2PublicationKindV1,
        attempt: BuildAttempt,
        admission: [u8; 32],
    ) -> Result<(), ResumeMarkerErrorV1> {
        self.persist_pending_with_envelope_inputs(publication, attempt, admission, None)
    }

    pub(crate) fn persist_pending_with_envelope_inputs(
        &self,
        publication: WorkerV2PublicationKindV1,
        attempt: BuildAttempt,
        admission: [u8; 32],
        envelope_inputs: Option<WorkerV2EnvelopeInputsIdentityV1>,
    ) -> Result<(), ResumeMarkerErrorV1> {
        let envelope_inputs = envelope_inputs.map_or([0; 32], |identity| identity.as_bytes());
        if admission == [0; 32] || publication.requires_envelope() != (envelope_inputs != [0; 32]) {
            return Err(ResumeMarkerErrorV1::InvalidTransition);
        }
        let pending = ResumeMarkerStateV1::Pending {
            legacy: false,
            publication,
            attempt,
            admission,
            envelope_inputs,
        };
        match self.load()? {
            None => self.write(pending, false),
            Some(existing) if existing == pending => Ok(()),
            Some(_) => Err(ResumeMarkerErrorV1::ConflictingMarker),
        }
    }

    pub(crate) fn persist_ready(
        &self,
        publication: WorkerV2PublicationKindV1,
        attempt: BuildAttempt,
        intent: WorkerV2PublicationIntentIdentityV1,
    ) -> Result<(), ResumeMarkerErrorV1> {
        match self.load()? {
            Some(ResumeMarkerStateV1::Pending {
                legacy: false,
                publication: current_publication,
                attempt: current_attempt,
                admission,
                envelope_inputs,
            }) if current_publication == publication && current_attempt == attempt => self.write(
                ResumeMarkerStateV1::Ready {
                    legacy: false,
                    publication,
                    attempt,
                    admission,
                    envelope_inputs,
                    intent,
                },
                true,
            ),
            Some(ResumeMarkerStateV1::Ready {
                legacy: false,
                publication: current_publication,
                attempt: current_attempt,
                intent: current_intent,
                ..
            }) if current_publication == publication
                && current_attempt == attempt
                && current_intent == intent =>
            {
                Ok(())
            }
            _ => Err(ResumeMarkerErrorV1::InvalidTransition),
        }
    }

    pub(crate) fn persist_completed(
        &self,
        publication: WorkerV2PublicationKindV1,
        attempt: BuildAttempt,
        intent: WorkerV2PublicationIntentIdentityV1,
        receipt: BackendPublicationReceiptV1,
    ) -> Result<ResumeMarkerStateV1, ResumeMarkerErrorV1> {
        if publication.requires_envelope() {
            return Err(ResumeMarkerErrorV1::InvalidTransition);
        }
        self.persist_completed_inner(publication, attempt, intent, receipt, None)
    }

    /// Durably publishes the required inert envelope before advancing the marker to `Completed`.
    pub(crate) fn persist_envelope_and_completed(
        &self,
        publication: WorkerV2PublicationKindV1,
        attempt: BuildAttempt,
        intent: WorkerV2PublicationIntentIdentityV1,
        receipt: BackendPublicationReceiptV1,
        envelope: &WorkerV2LoadEnvelopeV1,
    ) -> Result<ResumeMarkerStateV1, ResumeMarkerErrorV1> {
        if !publication.requires_envelope() {
            return Err(ResumeMarkerErrorV1::InvalidTransition);
        }
        let (admission, marker_inputs) = match self.load()? {
            Some(ResumeMarkerStateV1::Ready {
                legacy: false,
                publication: current_publication,
                attempt: current_attempt,
                admission,
                envelope_inputs,
                intent: current_intent,
            })
            | Some(ResumeMarkerStateV1::Completed {
                legacy: false,
                publication: current_publication,
                attempt: current_attempt,
                admission,
                envelope_inputs,
                intent: current_intent,
                ..
            }) if current_publication == publication
                && current_attempt == attempt
                && current_intent == intent =>
            {
                (admission, envelope_inputs)
            }
            _ => return Err(ResumeMarkerErrorV1::InvalidTransition),
        };
        let claim = envelope.published_claim();
        let envelope_inputs = self.recover_envelope_inputs(attempt)?;
        let carried_inputs = WorkerV2EnvelopeInputsV1::new(
            envelope.direct_link_evidence().clone(),
            envelope.proof_records().to_vec(),
            envelope.raw_hsaco().clone(),
        )
        .map_err(|error| self.invalid(format!("required envelope inputs are invalid: {error}")))?;
        if claim.receipt() != receipt
            || claim.plan().attempt() != attempt
            || marker_inputs != envelope_inputs.identity().as_bytes()
            || carried_inputs != envelope_inputs
            || restart_admission_commitment_with_inputs_v1(
                publication,
                claim.plan(),
                claim.upstream_evidence(),
                envelope.finalized_payload(),
                Some(envelope_inputs.identity()),
            ) != admission
            || envelope.grants_currentness_authority()
            || envelope.grants_load_authority()
            || envelope.grants_launch_authority()
        {
            return Err(self.invalid("required envelope disagrees with the ready publication"));
        }
        self.publish_load_envelope(envelope)?;
        self.persist_completed_inner(
            publication,
            attempt,
            intent,
            receipt,
            Some(envelope.identity()),
        )
    }

    /// Recovers only the canonical envelope named by `receipt` before advancing to `Completed`.
    #[allow(dead_code)] // Exercises exact durable-envelope recovery in owner tests.
    pub(crate) fn recover_envelope_and_completed(
        &self,
        publication: WorkerV2PublicationKindV1,
        attempt: BuildAttempt,
        intent: WorkerV2PublicationIntentIdentityV1,
        receipt: BackendPublicationReceiptV1,
    ) -> Result<ResumeMarkerStateV1, ResumeMarkerErrorV1> {
        if !publication.requires_envelope() {
            return Err(ResumeMarkerErrorV1::InvalidTransition);
        }
        let envelope = self.recover_load_envelope(receipt)?;
        self.persist_envelope_and_completed(publication, attempt, intent, receipt, &envelope)
    }

    fn persist_completed_inner(
        &self,
        publication: WorkerV2PublicationKindV1,
        attempt: BuildAttempt,
        intent: WorkerV2PublicationIntentIdentityV1,
        receipt: BackendPublicationReceiptV1,
        envelope: Option<WorkerV2LoadEnvelopeIdentityV1>,
    ) -> Result<ResumeMarkerStateV1, ResumeMarkerErrorV1> {
        let envelope = envelope.map_or([0; 32], |identity| identity.as_bytes());
        if publication.requires_envelope() != (envelope != [0; 32]) {
            return Err(ResumeMarkerErrorV1::InvalidTransition);
        }
        match self.load()? {
            Some(ResumeMarkerStateV1::Ready {
                legacy: false,
                publication: current_publication,
                attempt: current_attempt,
                admission,
                envelope_inputs,
                intent: current_intent,
            }) if current_publication == publication
                && current_attempt == attempt
                && current_intent == intent =>
            {
                let completed = ResumeMarkerStateV1::Completed {
                    legacy: false,
                    publication,
                    attempt,
                    admission,
                    envelope_inputs,
                    envelope,
                    intent,
                    receipt: ReceiptRecordV1::from_receipt(receipt),
                };
                self.write(completed, true)?;
                Ok(completed)
            }
            Some(
                existing @ ResumeMarkerStateV1::Completed {
                    publication: current_publication,
                    attempt: current_attempt,
                    intent: current_intent,
                    receipt: current_receipt,
                    envelope: current_envelope,
                    ..
                },
            ) if current_publication == publication
                && current_attempt == attempt
                && current_intent == intent
                && current_receipt == ReceiptRecordV1::from_receipt(receipt)
                && current_envelope == envelope =>
            {
                Ok(existing)
            }
            _ => Err(ResumeMarkerErrorV1::InvalidTransition),
        }
    }

    #[allow(dead_code)] // Ordinary state-machine owner tests exercise this narrow transition.
    pub(crate) fn clear_completed(
        &self,
        expected: ResumeMarkerStateV1,
    ) -> Result<(), ResumeMarkerErrorV1> {
        if !matches!(expected, ResumeMarkerStateV1::Completed { .. }) {
            return Err(ResumeMarkerErrorV1::InvalidTransition);
        }
        self.clear_exact(expected)
    }

    pub(crate) fn clear_completed_and_envelope_inputs(
        &self,
        expected: ResumeMarkerStateV1,
    ) -> Result<(), ResumeMarkerErrorV1> {
        if !matches!(expected, ResumeMarkerStateV1::Completed { .. }) {
            return Err(ResumeMarkerErrorV1::InvalidTransition);
        }
        self.clear_exact_and_envelope_inputs(expected)
    }

    pub(crate) fn clear_abandoned_pending(
        &self,
        expected: ResumeMarkerStateV1,
    ) -> Result<(), ResumeMarkerErrorV1> {
        if !matches!(expected, ResumeMarkerStateV1::Pending { .. }) {
            return Err(ResumeMarkerErrorV1::InvalidTransition);
        }
        self.clear_exact_and_envelope_inputs(expected)
    }

    fn clear_exact_and_envelope_inputs(
        &self,
        expected: ResumeMarkerStateV1,
    ) -> Result<(), ResumeMarkerErrorV1> {
        let expected_inputs = if expected.publication().requires_envelope() {
            let inputs = self.recover_envelope_inputs(expected.attempt())?;
            if inputs.identity().as_bytes() != expected.envelope_inputs() {
                return Err(self.invalid("marker disagrees with its envelope input capsule"));
            }
            Some(inputs.identity())
        } else {
            None
        };
        self.clear_exact(expected)?;
        if let Some(identity) = expected_inputs {
            self.remove_envelope_inputs(expected.attempt(), identity)?;
        }
        Ok(())
    }

    fn remove_envelope_inputs(
        &self,
        attempt: BuildAttempt,
        expected: WorkerV2EnvelopeInputsIdentityV1,
    ) -> Result<(), ResumeMarkerErrorV1> {
        let name = envelope_inputs_name(self.package, attempt);
        let inputs = self
            .read_envelope_inputs(&name)?
            .ok_or_else(|| self.invalid_at_name(&name, "capsule disappeared before cleanup"))?;
        if inputs.identity() != expected {
            return Err(self.invalid_at_name(&name, "capsule identity changed before cleanup"));
        }
        unlinkat(&self.directory, &name, AtFlags::empty()).map_err(std::io::Error::from)?;
        fsync(&self.directory).map_err(std::io::Error::from)?;
        self.verify_output_path()?;
        Ok(())
    }

    pub(crate) fn clear_exact(
        &self,
        expected: ResumeMarkerStateV1,
    ) -> Result<(), ResumeMarkerErrorV1> {
        if self.load()? != Some(expected) {
            return Err(ResumeMarkerErrorV1::InvalidTransition);
        }
        unlinkat(&self.directory, &self.marker_name, AtFlags::empty())
            .map_err(std::io::Error::from)?;
        fsync(&self.directory).map_err(std::io::Error::from)?;
        self.verify_output_path()?;
        Ok(())
    }

    fn write(&self, state: ResumeMarkerStateV1, replace: bool) -> Result<(), ResumeMarkerErrorV1> {
        if state.is_legacy() {
            return Err(ResumeMarkerErrorV1::InvalidTransition);
        }
        self.verify_output_path()?;
        let bytes = encode_marker(self.package, state);
        let temp_name = format!(
            "{}{TEMP_SUFFIX}{}-{}",
            self.marker_name,
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        );
        let result = (|| {
            let descriptor = openat(
                &self.directory,
                &temp_name,
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::RUSR | Mode::WUSR,
            )
            .map_err(std::io::Error::from)?;
            let mut file = File::from(descriptor);
            file.set_len(bytes.len() as u64)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            validate_private_file(
                &self.directory,
                &file,
                &temp_name,
                &self.display_path,
                Some(MARKER_BYTES),
            )?;
            self.verify_output_path()?;
            if replace {
                renameat(
                    &self.directory,
                    &temp_name,
                    &self.directory,
                    &self.marker_name,
                )
                .map_err(std::io::Error::from)?;
            } else {
                renameat_with(
                    &self.directory,
                    &temp_name,
                    &self.directory,
                    &self.marker_name,
                    RenameFlags::NOREPLACE,
                )
                .map_err(std::io::Error::from)?;
            }
            fsync(&self.directory).map_err(std::io::Error::from)?;
            self.verify_output_path()?;
            if self.load()? != Some(state) {
                return Err(self.invalid("marker changed after atomic publication"));
            }
            Ok(())
        })();
        if result.is_err() {
            let _ = unlinkat(&self.directory, &temp_name, AtFlags::empty());
        }
        result
    }

    pub(crate) fn migrate_legacy_to_ready(
        &self,
        expected: ResumeMarkerStateV1,
        admission: [u8; 32],
        intent: WorkerV2PublicationIntentIdentityV1,
    ) -> Result<(), ResumeMarkerErrorV1> {
        if !expected.is_legacy()
            || expected.publication() != WorkerV2PublicationKindV1::Raw
            || admission == [0; 32]
            || self.load()? != Some(expected)
        {
            return Err(ResumeMarkerErrorV1::InvalidTransition);
        }
        self.write(
            ResumeMarkerStateV1::Ready {
                legacy: false,
                publication: WorkerV2PublicationKindV1::Raw,
                attempt: expected.attempt(),
                admission,
                envelope_inputs: [0; 32],
                intent,
            },
            true,
        )
    }

    fn invalid(&self, reason: impl Into<String>) -> ResumeMarkerErrorV1 {
        ResumeMarkerErrorV1::InvalidMarker {
            path: self.display_path.join(&self.marker_name),
            reason: reason.into(),
        }
    }

    fn invalid_at_name(&self, name: &str, reason: impl Into<String>) -> ResumeMarkerErrorV1 {
        ResumeMarkerErrorV1::InvalidMarker {
            path: self.display_path.join(name),
            reason: reason.into(),
        }
    }

    fn invalid_at(path: &Path, reason: impl Into<String>) -> ResumeMarkerErrorV1 {
        ResumeMarkerErrorV1::InvalidMarker {
            path: path.to_path_buf(),
            reason: reason.into(),
        }
    }
}

fn open_output_directory(path: &Path, create: bool) -> Result<OwnedFd, ResumeMarkerErrorV1> {
    #[cfg(target_os = "linux")]
    if let Some(directory) = duplicate_proc_self_fd_directory(path) {
        return directory.map_err(Into::into);
    }

    if create {
        std::fs::create_dir_all(path)?;
    }
    open(
        path,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(std::io::Error::from)
    .map_err(Into::into)
}

#[cfg(target_os = "linux")]
fn duplicate_proc_self_fd_directory(path: &Path) -> Option<std::io::Result<OwnedFd>> {
    use std::os::unix::ffi::OsStrExt;

    const PREFIX: &[u8] = b"/proc/self/fd/";
    let descriptor = path.as_os_str().as_bytes().strip_prefix(PREFIX)?;
    let canonical = descriptor == b"0"
        || descriptor
            .first()
            .is_some_and(|byte| matches!(byte, b'1'..=b'9'))
            && descriptor.iter().all(u8::is_ascii_digit);
    if !canonical {
        return Some(Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "procfs descriptor path is not canonical",
        )));
    }
    let Some(raw_fd) = descriptor.iter().try_fold(0_i32, |value, digit| {
        value.checked_mul(10)?.checked_add(i32::from(*digit - b'0'))
    }) else {
        return Some(Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "procfs descriptor number is out of range",
        )));
    };

    // Raw fcntl reports EBADF for stale descriptor numbers without manufacturing an invalid
    // BorrowedFd. A successful return is a new descriptor owned by this process.
    let duplicated = unsafe { libc::fcntl(raw_fd, libc::F_DUPFD_CLOEXEC, 0) };
    if duplicated < 0 {
        return Some(Err(std::io::Error::last_os_error()));
    }
    let directory = unsafe { OwnedFd::from_raw_fd(duplicated) };
    let stat = match fstat(&directory) {
        Ok(stat) => stat,
        Err(error) => return Some(Err(std::io::Error::from(error))),
    };
    if FileType::from_raw_mode(stat.st_mode) != FileType::Directory {
        return Some(Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "procfs descriptor does not reference a directory",
        )));
    }
    Some(Ok(directory))
}

fn validate_private_file(
    directory: &OwnedFd,
    descriptor: &impl rustix::fd::AsFd,
    name: &str,
    display_path: &Path,
    expected_size: Option<usize>,
) -> Result<(), ResumeMarkerErrorV1> {
    let descriptor_stat = fstat(descriptor).map_err(std::io::Error::from)?;
    let path_stat =
        statat(directory, name, AtFlags::SYMLINK_NOFOLLOW).map_err(std::io::Error::from)?;
    let valid_kind = FileType::from_raw_mode(descriptor_stat.st_mode) == FileType::RegularFile
        && FileType::from_raw_mode(path_stat.st_mode) == FileType::RegularFile;
    let valid_identity =
        descriptor_stat.st_dev == path_stat.st_dev && descriptor_stat.st_ino == path_stat.st_ino;
    let valid_links = descriptor_stat.st_nlink == 1 && path_stat.st_nlink == 1;
    let valid_mode = descriptor_stat.st_mode & 0o077 == 0 && path_stat.st_mode & 0o077 == 0;
    let valid_size = expected_size.is_none_or(|size| {
        descriptor_stat.st_size == size as i64 && path_stat.st_size == size as i64
    });
    if !valid_kind || !valid_identity || !valid_links || !valid_mode || !valid_size {
        return Err(ResumeMarkerErrorV1::InvalidMarker {
            path: display_path.join(name),
            reason: "entry must be one private, single-link regular file with exact size".into(),
        });
    }
    Ok(())
}

fn encode_marker(package: [u8; 32], state: ResumeMarkerStateV1) -> Vec<u8> {
    debug_assert!(!state.is_legacy());
    let attempt = state.attempt();
    let publication = state.publication();
    let admission = state.admission();
    let envelope_inputs = state.envelope_inputs();
    let envelope = state.envelope();
    let (stage, intent, receipt) = match state {
        ResumeMarkerStateV1::Pending { .. } => (1, [0; 32], ReceiptRecordV1([[0; 32]; 7])),
        ResumeMarkerStateV1::Ready { intent, .. } => {
            (2, intent.as_bytes(), ReceiptRecordV1([[0; 32]; 7]))
        }
        ResumeMarkerStateV1::Completed {
            intent, receipt, ..
        } => (3, intent.as_bytes(), receipt),
    };
    let mut bytes = Vec::with_capacity(MARKER_BYTES);
    bytes.extend_from_slice(MARKER_MAGIC);
    bytes.extend_from_slice(&MARKER_VERSION.to_le_bytes());
    bytes.push(stage);
    bytes.push(publication.tag());
    bytes.extend_from_slice(&package);
    bytes.extend_from_slice(&attempt.generation().to_le_bytes());
    bytes.extend_from_slice(attempt.session().as_bytes());
    bytes.extend_from_slice(attempt.invocation().as_bytes());
    bytes.extend_from_slice(&admission);
    bytes.extend_from_slice(&envelope_inputs);
    bytes.extend_from_slice(&envelope);
    bytes.extend_from_slice(&intent);
    receipt.encode(&mut bytes);
    let checksum = checksum(&bytes);
    bytes.extend_from_slice(&checksum);
    debug_assert_eq!(bytes.len(), MARKER_BYTES);
    bytes
}

fn decode_marker(
    bytes: &[u8],
    expected_package: [u8; 32],
) -> Result<ResumeMarkerStateV1, &'static str> {
    if bytes.len() != MARKER_BYTES
        && bytes.len() != PREVIOUS_MARKER_BYTES
        && bytes.len() != LEGACY_MARKER_BYTES
    {
        return Err("marker has a noncanonical length");
    }
    let (body, encoded_checksum) = bytes.split_at(bytes.len() - 32);
    if checksum(body).as_slice() != encoded_checksum {
        return Err("marker checksum mismatch");
    }
    let mut decoder = Decoder::new(body);
    if decoder.take(MARKER_MAGIC.len())? != MARKER_MAGIC {
        return Err("marker magic mismatch");
    }
    let version = decoder.u16()?;
    match (version, bytes.len()) {
        (MARKER_VERSION, MARKER_BYTES) => decode_marker_v3(&mut decoder, expected_package),
        (PREVIOUS_MARKER_VERSION, PREVIOUS_MARKER_BYTES) => {
            decode_previous_marker_v2(&mut decoder, expected_package)
        }
        (LEGACY_MARKER_VERSION, LEGACY_MARKER_BYTES) => {
            decode_legacy_raw_marker_v1(&mut decoder, expected_package)
        }
        _ => Err("unsupported marker version or noncanonical version length"),
    }
}

fn decode_marker_v3(
    decoder: &mut Decoder<'_>,
    expected_package: [u8; 32],
) -> Result<ResumeMarkerStateV1, &'static str> {
    let stage = decoder.byte()?;
    let publication = WorkerV2PublicationKindV1::from_tag(decoder.byte()?)
        .ok_or("marker publication kind is noncanonical")?;
    if decoder.array()? != expected_package {
        return Err("marker producer package mismatch");
    }
    let generation = decoder.u64()?;
    let session = BuildSession::from_bytes(decoder.array()?);
    let invocation = fe2o3_artifact_transaction::BuildInvocation::from_bytes(decoder.array()?);
    let attempt = BuildAttempt::from_env_value(&format!(
        "{generation}:{}:{}",
        session.to_hex(),
        invocation.to_hex()
    ))
    .map_err(|_| "marker contains an invalid attempt")?;
    let admission = decoder.array()?;
    let envelope_inputs = decoder.array()?;
    let envelope = decoder.array()?;
    let intent = WorkerV2PublicationIntentIdentityV1::from_bytes(decoder.array()?);
    let receipt = ReceiptRecordV1::decode(decoder)?;
    if !decoder.finished() {
        return Err("marker has trailing body bytes");
    }
    let required = publication.requires_envelope();
    let input_fields_valid = required == (envelope_inputs != [0; 32]);
    match stage {
        1 if admission != [0; 32]
            && input_fields_valid
            && envelope == [0; 32]
            && intent.as_bytes() == [0; 32]
            && receipt.is_zero() =>
        {
            Ok(ResumeMarkerStateV1::Pending {
                legacy: false,
                publication,
                attempt,
                admission,
                envelope_inputs,
            })
        }
        2 if admission != [0; 32]
            && input_fields_valid
            && envelope == [0; 32]
            && intent.as_bytes() != [0; 32]
            && receipt.is_zero() =>
        {
            Ok(ResumeMarkerStateV1::Ready {
                legacy: false,
                publication,
                attempt,
                admission,
                envelope_inputs,
                intent,
            })
        }
        3 if admission != [0; 32]
            && input_fields_valid
            && required == (envelope != [0; 32])
            && intent.as_bytes() != [0; 32]
            && !receipt.is_zero() =>
        {
            Ok(ResumeMarkerStateV1::Completed {
                legacy: false,
                publication,
                attempt,
                admission,
                envelope_inputs,
                envelope,
                intent,
                receipt,
            })
        }
        _ => Err("marker stage fields are noncanonical"),
    }
}

fn decode_previous_marker_v2(
    decoder: &mut Decoder<'_>,
    expected_package: [u8; 32],
) -> Result<ResumeMarkerStateV1, &'static str> {
    let stage = decoder.byte()?;
    let publication = WorkerV2PublicationKindV1::from_tag(decoder.byte()?)
        .ok_or("marker publication kind is noncanonical")?;
    if publication.requires_envelope() {
        return Err("required V2 marker lacks an exact envelope input identity");
    }
    if decoder.array()? != expected_package {
        return Err("marker producer package mismatch");
    }
    let generation = decoder.u64()?;
    let session = BuildSession::from_bytes(decoder.array()?);
    let invocation = fe2o3_artifact_transaction::BuildInvocation::from_bytes(decoder.array()?);
    let attempt = BuildAttempt::from_env_value(&format!(
        "{generation}:{}:{}",
        session.to_hex(),
        invocation.to_hex()
    ))
    .map_err(|_| "marker contains an invalid attempt")?;
    let admission = decoder.array()?;
    let intent = WorkerV2PublicationIntentIdentityV1::from_bytes(decoder.array()?);
    let receipt = ReceiptRecordV1::decode(decoder)?;
    if !decoder.finished() {
        return Err("marker has trailing body bytes");
    }
    match stage {
        1 if admission != [0; 32] && intent.as_bytes() == [0; 32] && receipt.is_zero() => {
            Ok(ResumeMarkerStateV1::Pending {
                legacy: false,
                publication,
                attempt,
                admission,
                envelope_inputs: [0; 32],
            })
        }
        2 if admission != [0; 32] && intent.as_bytes() != [0; 32] && receipt.is_zero() => {
            Ok(ResumeMarkerStateV1::Ready {
                legacy: false,
                publication,
                attempt,
                admission,
                envelope_inputs: [0; 32],
                intent,
            })
        }
        3 if admission != [0; 32] && intent.as_bytes() != [0; 32] && !receipt.is_zero() => {
            Ok(ResumeMarkerStateV1::Completed {
                legacy: false,
                publication,
                attempt,
                admission,
                envelope_inputs: [0; 32],
                envelope: [0; 32],
                intent,
                receipt,
            })
        }
        _ => Err("marker stage fields are noncanonical"),
    }
}

fn decode_legacy_raw_marker_v1(
    decoder: &mut Decoder<'_>,
    expected_package: [u8; 32],
) -> Result<ResumeMarkerStateV1, &'static str> {
    let stage = decoder.byte()?;
    if decoder.array()? != expected_package {
        return Err("marker producer package mismatch");
    }
    let generation = decoder.u64()?;
    let session = BuildSession::from_bytes(decoder.array()?);
    let invocation = fe2o3_artifact_transaction::BuildInvocation::from_bytes(decoder.array()?);
    let attempt = BuildAttempt::from_env_value(&format!(
        "{generation}:{}:{}",
        session.to_hex(),
        invocation.to_hex()
    ))
    .map_err(|_| "marker contains an invalid attempt")?;
    let legacy_value = decoder.array()?;
    let receipt = ReceiptRecordV1::decode(decoder)?;
    if !decoder.finished() {
        return Err("marker has trailing body bytes");
    }
    let publication = WorkerV2PublicationKindV1::Raw;
    match stage {
        1 if legacy_value != [0; 32] && receipt.is_zero() => Ok(ResumeMarkerStateV1::Pending {
            legacy: true,
            publication,
            attempt,
            admission: legacy_value,
            envelope_inputs: [0; 32],
        }),
        2 if legacy_value != [0; 32] && receipt.is_zero() => Ok(ResumeMarkerStateV1::Ready {
            legacy: true,
            publication,
            attempt,
            admission: [0; 32],
            envelope_inputs: [0; 32],
            intent: WorkerV2PublicationIntentIdentityV1::from_bytes(legacy_value),
        }),
        3 if legacy_value != [0; 32] && !receipt.is_zero() => Ok(ResumeMarkerStateV1::Completed {
            legacy: true,
            publication,
            attempt,
            admission: [0; 32],
            envelope_inputs: [0; 32],
            envelope: [0; 32],
            intent: WorkerV2PublicationIntentIdentityV1::from_bytes(legacy_value),
            receipt,
        }),
        _ => Err("legacy raw marker stage fields are noncanonical"),
    }
}

fn checksum(bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(MARKER_CHECKSUM_DOMAIN);
    digest.update(bytes);
    digest.finalize().into()
}

fn hex(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use fmt::Write as _;
        write!(text, "{byte:02x}").expect("writing to String cannot fail");
    }
    text
}

struct Decoder<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Decoder<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], &'static str> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or("marker length overflow")?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or("marker is truncated")?;
        self.offset = end;
        Ok(value)
    }

    fn byte(&mut self) -> Result<u8, &'static str> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, &'static str> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }

    fn u64(&mut self) -> Result<u64, &'static str> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], &'static str> {
        Ok(self.take(N)?.try_into().unwrap())
    }

    fn finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::sync::atomic::{AtomicU64, Ordering};

    use fe2o3_artifact_transaction::{
        AtomicPublicationIdentityV1, BuildInvocation, BuildSession, CanonicalLinkRequestIdentityV1,
        DurableLinkPublicationPlanV1, FinalizationIdentityV1, FinalizedOutputIdentityV1,
        KernelSetIdentityV1, LinkPublicationScopeV1, LinkedOutputIdentityV1, PackageIdentityV1,
        PinnedWorkerIdentityV1, TargetIdentityV1, UpstreamCodeObjectEvidenceIdentityV1,
        ValidatedResponseIdentityV1, begin_build_attempt, clear_worker_v2_publication_intent_v1,
        publish_exact_hsaco_evidence_for_attempt_v1,
    };

    use super::*;

    static NEXT: AtomicU64 = AtomicU64::new(1);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "cargo-fe2o3-worker-v2-resume-marker-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn producer(seed: u8) -> ProducerIdentity {
        ProducerIdentity::from_codegen(
            &format!("resume_{seed}"),
            Some(Path::new(&format!("/src/resume-{seed}.rs"))),
        )
        .unwrap()
    }

    fn attempt(path: &Path, producer: &ProducerIdentity, seed: u8) -> BuildAttempt {
        begin_build_attempt(
            path,
            producer,
            BuildInvocation::from_bytes([seed; 32]),
            BuildSession::from_bytes([seed.wrapping_add(1); 16]),
        )
        .unwrap()
    }

    fn publication_inputs(
        attempt: BuildAttempt,
        seed: u8,
    ) -> (
        Vec<u8>,
        DurableLinkPublicationPlanV1,
        UpstreamCodeObjectEvidenceIdentityV1,
    ) {
        let output = vec![seed; 19];
        let plan = DurableLinkPublicationPlanV1::new(
            attempt,
            LinkPublicationScopeV1::new(
                PackageIdentityV1::from_bytes([seed; 32]),
                KernelSetIdentityV1::from_bytes([seed.wrapping_add(1); 32]),
                TargetIdentityV1::from_bytes([seed.wrapping_add(2); 32]),
            ),
            CanonicalLinkRequestIdentityV1::from_bytes([seed.wrapping_add(3); 32]),
            PinnedWorkerIdentityV1::from_bytes([seed.wrapping_add(4); 32]),
            ValidatedResponseIdentityV1::from_bytes([seed.wrapping_add(5); 32]),
            LinkedOutputIdentityV1::from_bytes(Sha256::digest(&output).into()),
            FinalizationIdentityV1::from_bytes([seed.wrapping_add(6); 32]),
            FinalizedOutputIdentityV1::from_bytes(Sha256::digest(&output).into()),
            AtomicPublicationIdentityV1::from_bytes([seed.wrapping_add(7); 32]),
        );
        (
            output,
            plan,
            UpstreamCodeObjectEvidenceIdentityV1::from_bytes([seed.wrapping_add(8); 32]),
        )
    }

    fn receipt(
        path: &Path,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        seed: u8,
    ) -> BackendPublicationReceiptV1 {
        let (output, plan, upstream) = publication_inputs(attempt, seed);
        publish_exact_hsaco_evidence_for_attempt_v1(
            path, producer, attempt, plan, upstream, &output,
        )
        .unwrap()
        .receipt()
    }

    fn legacy_marker_bytes(
        package: [u8; 32],
        attempt: BuildAttempt,
        stage: u8,
        value: [u8; 32],
        receipt: ReceiptRecordV1,
    ) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(LEGACY_MARKER_BYTES);
        bytes.extend_from_slice(MARKER_MAGIC);
        bytes.extend_from_slice(&LEGACY_MARKER_VERSION.to_le_bytes());
        bytes.push(stage);
        bytes.extend_from_slice(&package);
        bytes.extend_from_slice(&attempt.generation().to_le_bytes());
        bytes.extend_from_slice(attempt.session().as_bytes());
        bytes.extend_from_slice(attempt.invocation().as_bytes());
        bytes.extend_from_slice(&value);
        receipt.encode(&mut bytes);
        let checksum = checksum(&bytes);
        bytes.extend_from_slice(&checksum);
        assert_eq!(bytes.len(), LEGACY_MARKER_BYTES);
        bytes
    }

    fn previous_marker_bytes(
        package: [u8; 32],
        publication: WorkerV2PublicationKindV1,
        attempt: BuildAttempt,
        stage: u8,
        admission: [u8; 32],
        intent: [u8; 32],
        receipt: ReceiptRecordV1,
    ) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(PREVIOUS_MARKER_BYTES);
        bytes.extend_from_slice(MARKER_MAGIC);
        bytes.extend_from_slice(&PREVIOUS_MARKER_VERSION.to_le_bytes());
        bytes.push(stage);
        bytes.push(publication.tag());
        bytes.extend_from_slice(&package);
        bytes.extend_from_slice(&attempt.generation().to_le_bytes());
        bytes.extend_from_slice(attempt.session().as_bytes());
        bytes.extend_from_slice(attempt.invocation().as_bytes());
        bytes.extend_from_slice(&admission);
        bytes.extend_from_slice(&intent);
        receipt.encode(&mut bytes);
        let checksum = checksum(&bytes);
        bytes.extend_from_slice(&checksum);
        assert_eq!(bytes.len(), PREVIOUS_MARKER_BYTES);
        bytes
    }

    fn install_marker(store: &WorkerV2ResumeStoreV1, bytes: &[u8]) {
        let path = store.display_path.join(&store.marker_name);
        fs::write(&path, bytes).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }

    #[test]
    fn canonical_state_machine_round_trips_exactly() {
        let directory = TestDirectory::new();
        let producer = producer(1);
        let attempt = attempt(&directory.0, &producer, 1);
        let store = WorkerV2ResumeStoreV1::open(&directory.0, &producer).unwrap();
        assert_eq!(store.load().unwrap(), None);

        let publication = WorkerV2PublicationKindV1::Raw;
        let admission = [0x31; 32];
        store
            .persist_pending(publication, attempt, admission)
            .unwrap();
        assert_eq!(
            store.load().unwrap(),
            Some(ResumeMarkerStateV1::Pending {
                legacy: false,
                publication,
                attempt,
                admission,
                envelope_inputs: [0; 32],
            })
        );
        store
            .persist_pending(publication, attempt, admission)
            .unwrap();

        let intent = WorkerV2PublicationIntentIdentityV1::from_bytes([0x41; 32]);
        store.persist_ready(publication, attempt, intent).unwrap();
        assert_eq!(
            store.load().unwrap(),
            Some(ResumeMarkerStateV1::Ready {
                legacy: false,
                publication,
                attempt,
                admission,
                envelope_inputs: [0; 32],
                intent,
            })
        );
        store.persist_ready(publication, attempt, intent).unwrap();

        let receipt = receipt(&directory.0, &producer, attempt, 7);
        store
            .persist_completed(publication, attempt, intent, receipt)
            .unwrap();
        let completed = ResumeMarkerStateV1::Completed {
            legacy: false,
            publication,
            attempt,
            admission,
            envelope_inputs: [0; 32],
            envelope: [0; 32],
            intent,
            receipt: ReceiptRecordV1::from_receipt(receipt),
        };
        assert_eq!(store.load().unwrap(), Some(completed));
        store
            .persist_completed(publication, attempt, intent, receipt)
            .unwrap();
        store.clear_completed(completed).unwrap();
        assert_eq!(store.load().unwrap(), None);
    }

    #[test]
    fn startup_scavenges_only_package_owned_capsules_and_temps_without_growth() {
        let directory = TestDirectory::new();
        let publisher = producer(81);
        let other = producer(82);
        let attempt = attempt(&directory.0, &publisher, 81);
        let store = WorkerV2ResumeStoreV1::open(&directory.0, &publisher).unwrap();
        let package = store.package;
        drop(store);

        let canonical = envelope_inputs_name(package, attempt);
        let temporary = format!("{canonical}{TEMP_SUFFIX}1-1");
        let unrelated =
            envelope_inputs_name(*producer_package_identity_v1(&other).as_bytes(), attempt);
        fs::write(directory.0.join(&unrelated), b"unrelated").unwrap();
        fs::set_permissions(
            directory.0.join(&unrelated),
            fs::Permissions::from_mode(0o600),
        )
        .unwrap();

        for _ in 0..3 {
            for name in [&canonical, &temporary] {
                fs::write(directory.0.join(name), b"abandoned").unwrap();
                fs::set_permissions(directory.0.join(name), fs::Permissions::from_mode(0o600))
                    .unwrap();
            }
            let store = WorkerV2ResumeStoreV1::open(&directory.0, &publisher).unwrap();
            assert_eq!(store.load().unwrap(), None);
            assert!(!directory.0.join(&canonical).exists());
            assert!(!directory.0.join(&temporary).exists());
            assert!(directory.0.join(&unrelated).exists());
        }
    }

    #[test]
    fn startup_scavenges_only_package_owned_envelope_temps_without_growth() {
        let directory = TestDirectory::new();
        let publisher = producer(83);
        let other = producer(84);
        let store = WorkerV2ResumeStoreV1::open(&directory.0, &publisher).unwrap();
        let package = store.package;
        drop(store);
        let other_package = *producer_package_identity_v1(&other).as_bytes();
        let publication = [0x83; 32];
        let unrelated = envelope_temp_name(other_package, publication, 2, 1);
        let legacy = format!("{}{TEMP_SUFFIX}3-1", envelope_name(publication));
        for name in [&unrelated, &legacy] {
            fs::write(directory.0.join(name), b"not owned by this package").unwrap();
            fs::set_permissions(directory.0.join(name), fs::Permissions::from_mode(0o600)).unwrap();
        }

        for counter in 1..=3 {
            let temporary = envelope_temp_name(package, publication, 1, counter);
            fs::write(directory.0.join(&temporary), b"abandoned").unwrap();
            fs::set_permissions(
                directory.0.join(&temporary),
                fs::Permissions::from_mode(0o600),
            )
            .unwrap();

            let store = WorkerV2ResumeStoreV1::open(&directory.0, &publisher).unwrap();
            assert_eq!(store.load().unwrap(), None);
            assert!(!directory.0.join(&temporary).exists());
            assert!(directory.0.join(&unrelated).exists());
            assert!(directory.0.join(&legacy).exists());
        }
    }

    #[test]
    fn malformed_package_owned_envelope_temp_fails_closed() {
        let directory = TestDirectory::new();
        let publisher = producer(85);
        let store = WorkerV2ResumeStoreV1::open(&directory.0, &publisher).unwrap();
        let package_prefix = envelope_temp_package_prefix(store.package);
        drop(store);
        let malformed = format!("{package_prefix}not-a-canonical-temp");
        fs::write(directory.0.join(&malformed), b"malformed").unwrap();
        fs::set_permissions(
            directory.0.join(&malformed),
            fs::Permissions::from_mode(0o600),
        )
        .unwrap();

        let error = match WorkerV2ResumeStoreV1::open(&directory.0, &publisher) {
            Ok(_) => panic!("malformed package-owned temp unexpectedly opened"),
            Err(error) => error,
        };
        assert!(
            error
                .to_string()
                .contains("malformed package-owned load-envelope temp name")
        );
        assert!(directory.0.join(malformed).exists());
    }

    #[test]
    fn package_owned_envelope_temp_symlink_fails_closed() {
        let directory = TestDirectory::new();
        let publisher = producer(86);
        let store = WorkerV2ResumeStoreV1::open(&directory.0, &publisher).unwrap();
        let temporary = envelope_temp_name(store.package, [0x86; 32], 1, 1);
        drop(store);
        let target = directory.0.join("untrusted-envelope-temp-target");
        fs::write(&target, b"must survive").unwrap();
        symlink(&target, directory.0.join(&temporary)).unwrap();

        assert!(WorkerV2ResumeStoreV1::open(&directory.0, &publisher).is_err());
        assert_eq!(fs::read(target).unwrap(), b"must survive");
        assert!(directory.0.join(temporary).exists());
    }

    #[test]
    fn legacy_v1_raw_ready_marker_recovers_and_migrates_to_v3() {
        let directory = TestDirectory::new();
        let producer = producer(91);
        let attempt = attempt(&directory.0, &producer, 91);
        let (output, plan, upstream) = publication_inputs(attempt, 91);
        let persisted = persist_worker_v2_publication_intent_v1(
            &directory.0,
            &producer,
            attempt,
            plan,
            upstream,
            &output,
        )
        .unwrap();
        let intent = persisted.record().identity();
        drop(persisted);

        let store = WorkerV2ResumeStoreV1::open(&directory.0, &producer).unwrap();
        install_marker(
            &store,
            &legacy_marker_bytes(
                store.package,
                attempt,
                2,
                intent.as_bytes(),
                ReceiptRecordV1([[0; 32]; RECEIPT_FIELDS]),
            ),
        );
        let legacy = store.load().unwrap().unwrap();
        assert!(legacy.is_legacy());
        assert_eq!(legacy.publication(), WorkerV2PublicationKindV1::Raw);
        let recovered = recover_worker_v2_intent_v1(&store, &producer, legacy).unwrap();
        assert_eq!(recovered.exact_output(), output);

        let migrated = store.load().unwrap().unwrap();
        assert!(!migrated.is_legacy());
        assert_eq!(migrated.publication(), WorkerV2PublicationKindV1::Raw);
        assert_eq!(migrated.intent(), Some(intent));
        assert_eq!(
            migrated.admission(),
            restart_admission_commitment_v1(
                WorkerV2PublicationKindV1::Raw,
                plan,
                upstream,
                &output,
            )
        );
    }

    #[test]
    fn required_v2_markers_do_not_silently_upgrade_without_capsule_identity() {
        let directory = TestDirectory::new();
        let producer = producer(92);
        let attempt = attempt(&directory.0, &producer, 92);
        let store = WorkerV2ResumeStoreV1::open(&directory.0, &producer).unwrap();
        install_marker(
            &store,
            &previous_marker_bytes(
                store.package,
                WorkerV2PublicationKindV1::FinalizedEnvelopeRequired,
                attempt,
                2,
                [0x31; 32],
                [0x41; 32],
                ReceiptRecordV1([[0; 32]; RECEIPT_FIELDS]),
            ),
        );
        assert!(store.load().is_err());
    }

    #[test]
    fn ordinary_v2_ready_marker_remains_recoverable() {
        let directory = TestDirectory::new();
        let producer = producer(94);
        let attempt = attempt(&directory.0, &producer, 94);
        let store = WorkerV2ResumeStoreV1::open(&directory.0, &producer).unwrap();
        install_marker(
            &store,
            &previous_marker_bytes(
                store.package,
                WorkerV2PublicationKindV1::Finalized,
                attempt,
                2,
                [0x31; 32],
                [0x41; 32],
                ReceiptRecordV1([[0; 32]; RECEIPT_FIELDS]),
            ),
        );
        let state = store.load().unwrap().unwrap();
        assert_eq!(state.publication(), WorkerV2PublicationKindV1::Finalized);
        assert_eq!(state.envelope_inputs(), [0; 32]);
        assert_eq!(state.envelope(), [0; 32]);
    }

    #[test]
    fn impossible_required_marker_identity_states_are_rejected() {
        let directory = TestDirectory::new();
        let producer = producer(93);
        let attempt = attempt(&directory.0, &producer, 93);
        let store = WorkerV2ResumeStoreV1::open(&directory.0, &producer).unwrap();
        let publication = WorkerV2PublicationKindV1::FinalizedEnvelopeRequired;
        let missing_inputs = ResumeMarkerStateV1::Pending {
            legacy: false,
            publication,
            attempt,
            admission: [0x31; 32],
            envelope_inputs: [0; 32],
        };
        install_marker(&store, &encode_marker(store.package, missing_inputs));
        assert!(store.load().is_err());

        let missing_envelope = ResumeMarkerStateV1::Completed {
            legacy: false,
            publication,
            attempt,
            admission: [0x31; 32],
            envelope_inputs: [0x32; 32],
            envelope: [0; 32],
            intent: WorkerV2PublicationIntentIdentityV1::from_bytes([0x41; 32]),
            receipt: ReceiptRecordV1([[0x51; 32]; RECEIPT_FIELDS]),
        };
        install_marker(&store, &encode_marker(store.package, missing_envelope));
        assert!(store.load().is_err());
        assert!(matches!(
            store.persist_pending(publication, attempt, [0x31; 32]),
            Err(ResumeMarkerErrorV1::InvalidTransition)
        ));
    }

    #[test]
    fn legacy_v1_pending_requires_the_exact_legacy_raw_commitment() {
        for substituted in [false, true] {
            let directory = TestDirectory::new();
            let producer = producer(92 + u8::from(substituted));
            let attempt = attempt(&directory.0, &producer, 92 + u8::from(substituted));
            let (output, plan, upstream) = publication_inputs(attempt, 92 + u8::from(substituted));
            persist_worker_v2_publication_intent_v1(
                &directory.0,
                &producer,
                attempt,
                plan,
                upstream,
                &output,
            )
            .unwrap();
            let store = WorkerV2ResumeStoreV1::open(&directory.0, &producer).unwrap();
            let mut admission = legacy_restart_admission_commitment_v1(plan, upstream, &output);
            if substituted {
                admission[0] ^= 1;
            }
            install_marker(
                &store,
                &legacy_marker_bytes(
                    store.package,
                    attempt,
                    1,
                    admission,
                    ReceiptRecordV1([[0; 32]; RECEIPT_FIELDS]),
                ),
            );
            let state = store.load().unwrap().unwrap();
            let recovered = recover_worker_v2_intent_v1(&store, &producer, state);
            if substituted {
                assert!(matches!(
                    recovered,
                    Err(RestartIntentErrorV1::IntentIdentityMismatch)
                ));
                assert_eq!(store.load().unwrap(), Some(state));
            } else {
                assert_eq!(recovered.unwrap().exact_output(), output);
                assert!(!store.load().unwrap().unwrap().is_legacy());
            }
        }
    }

    #[test]
    fn legacy_v1_completed_reconciles_as_raw_and_cleans_exact_state() {
        let directory = TestDirectory::new();
        let producer = producer(94);
        let attempt = attempt(&directory.0, &producer, 94);
        let (output, plan, upstream) = publication_inputs(attempt, 94);
        let persisted = persist_worker_v2_publication_intent_v1(
            &directory.0,
            &producer,
            attempt,
            plan,
            upstream,
            &output,
        )
        .unwrap();
        let intent = persisted.record().identity();
        drop(persisted);
        let published = publish_exact_hsaco_evidence_for_attempt_v1(
            &directory.0,
            &producer,
            attempt,
            plan,
            upstream,
            &output,
        )
        .unwrap();

        let store = WorkerV2ResumeStoreV1::open(&directory.0, &producer).unwrap();
        install_marker(
            &store,
            &legacy_marker_bytes(
                store.package,
                attempt,
                3,
                intent.as_bytes(),
                ReceiptRecordV1::from_receipt(published.receipt()),
            ),
        );
        let completed = store.load().unwrap().unwrap();
        assert!(completed.is_legacy());
        assert_eq!(completed.publication(), WorkerV2PublicationKindV1::Raw);
        let recovered = recover_worker_v2_intent_v1(&store, &producer, completed).unwrap();
        assert_eq!(recovered.exact_output(), output);
        drop(recovered);

        clear_worker_v2_publication_intent_v1(&directory.0, &producer, attempt, intent).unwrap();
        fe2o3_artifact_transaction::finish_build_attempt(&directory.0, &producer, attempt).unwrap();
        store.clear_completed(completed).unwrap();
        assert_eq!(store.load().unwrap(), None);
    }

    #[test]
    fn rejects_out_of_order_and_substituted_transitions() {
        let directory = TestDirectory::new();
        let producer = producer(2);
        let attempt = attempt(&directory.0, &producer, 2);
        let store = WorkerV2ResumeStoreV1::open(&directory.0, &producer).unwrap();
        let intent = WorkerV2PublicationIntentIdentityV1::from_bytes([0x42; 32]);

        assert!(matches!(
            store.persist_ready(WorkerV2PublicationKindV1::Raw, attempt, intent),
            Err(ResumeMarkerErrorV1::InvalidTransition)
        ));
        store
            .persist_pending(WorkerV2PublicationKindV1::Raw, attempt, [0x32; 32])
            .unwrap();
        assert!(matches!(
            store.persist_pending(
                WorkerV2PublicationKindV1::Raw,
                BuildAttempt::from_env_value(&format!(
                    "{}:{}:{}",
                    attempt.generation() + 1,
                    attempt.session().to_hex(),
                    attempt.invocation().to_hex()
                ))
                .unwrap(),
                [0x32; 32]
            ),
            Err(ResumeMarkerErrorV1::ConflictingMarker)
        ));
        assert!(matches!(
            store.persist_ready(
                WorkerV2PublicationKindV1::Finalized,
                attempt,
                WorkerV2PublicationIntentIdentityV1::from_bytes([0x43; 32])
            ),
            Err(ResumeMarkerErrorV1::InvalidTransition)
        ));
        assert!(matches!(
            store.persist_ready(
                WorkerV2PublicationKindV1::Raw,
                attempt,
                WorkerV2PublicationIntentIdentityV1::from_bytes([0x43; 32])
            ),
            Ok(())
        ));
        assert!(matches!(
            store.persist_completed(
                WorkerV2PublicationKindV1::Raw,
                attempt,
                intent,
                receipt(&directory.0, &producer, attempt, 8)
            ),
            Err(ResumeMarkerErrorV1::InvalidTransition)
        ));
    }

    #[test]
    fn rejects_checksum_mode_symlink_and_hardlink_tamper() {
        for case in 0..4 {
            let directory = TestDirectory::new();
            let producer = producer(20 + case);
            let attempt = attempt(&directory.0, &producer, 20 + case);
            let store = WorkerV2ResumeStoreV1::open(&directory.0, &producer).unwrap();
            store
                .persist_pending(WorkerV2PublicationKindV1::Raw, attempt, [0x33; 32])
                .unwrap();
            let marker = directory.0.join(&store.marker_name);
            drop(store);

            match case {
                0 => {
                    let mut bytes = fs::read(&marker).unwrap();
                    bytes[MARKER_MAGIC.len() + 3] ^= 1;
                    fs::write(&marker, bytes).unwrap();
                }
                1 => {
                    fs::set_permissions(&marker, fs::Permissions::from_mode(0o644)).unwrap();
                }
                2 => {
                    let replacement = directory.0.join("replacement");
                    fs::write(&replacement, vec![0; MARKER_BYTES]).unwrap();
                    fs::set_permissions(&replacement, fs::Permissions::from_mode(0o600)).unwrap();
                    fs::remove_file(&marker).unwrap();
                    symlink(&replacement, &marker).unwrap();
                }
                3 => {
                    fs::hard_link(&marker, directory.0.join("alias")).unwrap();
                }
                _ => unreachable!(),
            }

            let rejected = match WorkerV2ResumeStoreV1::open(&directory.0, &producer) {
                Err(_) => true,
                Ok(store) => store.load().is_err(),
            };
            assert!(rejected, "tamper case {case} was accepted");
        }
    }

    #[test]
    fn rejects_output_directory_substitution() {
        let parent = TestDirectory::new();
        let output = parent.0.join("output");
        fs::create_dir(&output).unwrap();
        let producer = producer(40);
        let store = WorkerV2ResumeStoreV1::open(&output, &producer).unwrap();
        fs::rename(&output, parent.0.join("moved")).unwrap();
        fs::create_dir(&output).unwrap();
        assert!(matches!(
            store.verify_output_path(),
            Err(ResumeMarkerErrorV1::OutputDirectoryChanged(_))
        ));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn proc_self_fd_resume_store_stays_bound_to_retained_directory() {
        let parent = TestDirectory::new();
        let output = parent.0.join("output");
        let moved = parent.0.join("moved");
        fs::create_dir(&output).unwrap();
        let producer = producer(41);
        let attempt = attempt(&output, &producer, 41);
        let retained = fs::File::open(&output).unwrap();
        let descriptor_path = PathBuf::from(format!("/proc/self/fd/{}", retained.as_raw_fd()));
        let store = WorkerV2ResumeStoreV1::open(&descriptor_path, &producer).unwrap();
        let marker_name = store.marker_name.clone();
        let publication = WorkerV2PublicationKindV1::Finalized;
        let admission = [0x35; 32];
        store
            .persist_pending(publication, attempt, admission)
            .unwrap();

        fs::rename(&output, &moved).unwrap();
        fs::create_dir(&output).unwrap();
        let intent = WorkerV2PublicationIntentIdentityV1::from_bytes([0x45; 32]);
        store.persist_ready(publication, attempt, intent).unwrap();
        drop(store);

        let reopened = WorkerV2ResumeStoreV1::open(&descriptor_path, &producer).unwrap();
        assert_eq!(
            reopened.load().unwrap(),
            Some(ResumeMarkerStateV1::Ready {
                legacy: false,
                publication,
                attempt,
                admission,
                envelope_inputs: [0; 32],
                intent,
            })
        );
        assert!(moved.join(marker_name).is_file());
        assert!(fs::read_dir(&output).unwrap().next().is_none());
        assert!(open_output_directory(Path::new("/proc/self/fd/01"), false).is_err());
    }

    #[test]
    fn pending_marker_promotes_and_ready_marker_replays_after_restart() {
        let directory = TestDirectory::new();
        let producer = producer(50);
        let attempt = attempt(&directory.0, &producer, 50);
        let (output, plan, upstream) = publication_inputs(attempt, 50);
        let store = WorkerV2ResumeStoreV1::open(&directory.0, &producer).unwrap();
        let publication = WorkerV2PublicationKindV1::Raw;
        let admission = restart_admission_commitment_v1(publication, plan, upstream, &output);
        store
            .persist_pending(publication, attempt, admission)
            .unwrap();
        let persisted = persist_worker_v2_publication_intent_v1(
            &directory.0,
            &producer,
            attempt,
            plan,
            upstream,
            &output,
        )
        .unwrap();
        let intent_identity = persisted.record().identity();
        drop(persisted);
        drop(store);

        let store = WorkerV2ResumeStoreV1::open(&directory.0, &producer).unwrap();
        let pending = store.load().unwrap().unwrap();
        assert!(matches!(pending, ResumeMarkerStateV1::Pending { .. }));
        let recovered = recover_worker_v2_intent_v1(&store, &producer, pending).unwrap();
        assert_eq!(recovered.record().identity(), intent_identity);
        assert_eq!(recovered.exact_output(), output);
        assert_eq!(
            store.load().unwrap(),
            Some(ResumeMarkerStateV1::Ready {
                legacy: false,
                publication,
                attempt,
                admission,
                envelope_inputs: [0; 32],
                intent: intent_identity,
            })
        );
        drop(recovered);
        drop(store);

        let store = WorkerV2ResumeStoreV1::open(&directory.0, &producer).unwrap();
        let ready = store.load().unwrap().unwrap();
        let recovered = recover_worker_v2_intent_v1(&store, &producer, ready).unwrap();
        let published = publish_exact_hsaco_evidence_for_attempt_v1(
            &directory.0,
            &producer,
            attempt,
            recovered.record().plan(),
            recovered.record().upstream_evidence(),
            recovered.exact_output(),
        )
        .unwrap();
        store
            .persist_completed(publication, attempt, intent_identity, published.receipt())
            .unwrap();
        let completed = store.load().unwrap().unwrap();
        clear_worker_v2_publication_intent_v1(&directory.0, &producer, attempt, intent_identity)
            .unwrap();
        fe2o3_artifact_transaction::finish_build_attempt(&directory.0, &producer, attempt).unwrap();
        store.clear_completed(completed).unwrap();
        assert_eq!(store.load().unwrap(), None);
    }

    #[test]
    fn exact_intent_clear_rejects_receipt_with_substituted_request_plan_field() {
        let directory = TestDirectory::new();
        let producer = producer(59);
        let attempt = attempt(&directory.0, &producer, 59);
        let (output, intent_plan, upstream) = publication_inputs(attempt, 59);
        let persisted = persist_worker_v2_publication_intent_v1(
            &directory.0,
            &producer,
            attempt,
            intent_plan,
            upstream,
            &output,
        )
        .unwrap();
        let intent_identity = persisted.record().identity();
        drop(persisted);

        let receipt_plan = DurableLinkPublicationPlanV1::new(
            attempt,
            intent_plan.scope(),
            CanonicalLinkRequestIdentityV1::from_bytes([0xee; 32]),
            intent_plan.worker(),
            intent_plan.response(),
            intent_plan.linked_output(),
            intent_plan.finalization(),
            intent_plan.finalized_output(),
            intent_plan.publication(),
        );
        publish_exact_hsaco_evidence_for_attempt_v1(
            &directory.0,
            &producer,
            attempt,
            receipt_plan,
            upstream,
            &output,
        )
        .unwrap();

        assert!(
            clear_worker_v2_publication_intent_v1(
                &directory.0,
                &producer,
                attempt,
                intent_identity,
            )
            .is_err()
        );
        assert!(
            recover_worker_v2_publication_intent_v1(&directory.0, &producer, attempt).is_ok(),
            "a receipt with a substituted request must not authorize intent removal"
        );
    }

    #[test]
    fn ready_markers_reject_raw_and_finalized_snapshot_substitution() {
        for (seed, publication) in [
            (60, WorkerV2PublicationKindV1::Raw),
            (61, WorkerV2PublicationKindV1::Finalized),
        ] {
            let directory = TestDirectory::new();
            let producer = producer(seed);
            let attempt = attempt(&directory.0, &producer, seed);
            let (output, plan, upstream) = publication_inputs(attempt, seed);
            let store = WorkerV2ResumeStoreV1::open(&directory.0, &producer).unwrap();
            store
                .persist_pending(
                    publication,
                    attempt,
                    restart_admission_commitment_v1(publication, plan, upstream, &output),
                )
                .unwrap();
            let persisted = persist_worker_v2_publication_intent_v1(
                &directory.0,
                &producer,
                attempt,
                plan,
                upstream,
                &output,
            )
            .unwrap();
            store
                .persist_ready(publication, attempt, persisted.record().identity())
                .unwrap();
            drop(persisted);
            drop(store);

            let retained_output = fs::read_dir(&directory.0)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .find(|path| {
                    let name = path.file_name().unwrap().to_string_lossy();
                    name.starts_with(".fe2o3-worker-v2-publication-intent-v1-")
                        && name.ends_with(".output")
                })
                .unwrap();
            let mut substituted = fs::read(&retained_output).unwrap();
            substituted[0] ^= 1;
            fs::write(&retained_output, substituted).unwrap();

            let store = WorkerV2ResumeStoreV1::open(&directory.0, &producer).unwrap();
            let state = store.load().unwrap().unwrap();
            assert_eq!(state.publication(), publication);
            assert!(matches!(
                recover_worker_v2_intent_v1(&store, &producer, state),
                Err(RestartIntentErrorV1::Intent(_))
            ));
            assert_eq!(store.load().unwrap(), Some(state));
        }
    }

    #[test]
    fn stale_attempt_marker_cannot_recover_a_current_intent() {
        let directory = TestDirectory::new();
        let producer = producer(62);
        let current = attempt(&directory.0, &producer, 62);
        let stale = BuildAttempt::from_env_value(&format!(
            "{}:{}:{}",
            current.generation() + 1,
            current.session().to_hex(),
            current.invocation().to_hex()
        ))
        .unwrap();
        let (output, plan, upstream) = publication_inputs(current, 62);
        persist_worker_v2_publication_intent_v1(
            &directory.0,
            &producer,
            current,
            plan,
            upstream,
            &output,
        )
        .unwrap();

        let publication = WorkerV2PublicationKindV1::Finalized;
        let store = WorkerV2ResumeStoreV1::open(&directory.0, &producer).unwrap();
        store
            .persist_pending(
                publication,
                stale,
                restart_admission_commitment_v1(publication, plan, upstream, &output),
            )
            .unwrap();
        let state = store.load().unwrap().unwrap();
        assert_eq!(state.attempt(), stale);
        assert!(matches!(
            recover_worker_v2_intent_v1(&store, &producer, state),
            Err(RestartIntentErrorV1::Intent(_))
        ));
        assert_eq!(store.load().unwrap(), Some(state));
        assert!(recover_worker_v2_publication_intent_v1(&directory.0, &producer, current).is_ok());
    }

    #[test]
    fn pending_marker_rejects_substituted_journal_commitment() {
        let directory = TestDirectory::new();
        let producer = producer(70);
        let attempt = attempt(&directory.0, &producer, 70);
        let (output, plan, upstream) = publication_inputs(attempt, 70);
        let store = WorkerV2ResumeStoreV1::open(&directory.0, &producer).unwrap();
        store
            .persist_pending(WorkerV2PublicationKindV1::Raw, attempt, [0xa5; 32])
            .unwrap();
        persist_worker_v2_publication_intent_v1(
            &directory.0,
            &producer,
            attempt,
            plan,
            upstream,
            &output,
        )
        .unwrap();

        let state = store.load().unwrap().unwrap();
        assert!(matches!(
            recover_worker_v2_intent_v1(&store, &producer, state),
            Err(RestartIntentErrorV1::IntentIdentityMismatch)
        ));
        assert_eq!(store.load().unwrap(), Some(state));
    }

    #[test]
    fn publication_route_is_exact_and_never_downgrades_descriptor_bearing_cov6() {
        use CanonicalDescriptorSectionObservationV1::{
            Missing, PresentButNotFinalizedByThisInspection as Present,
        };

        assert_eq!(
            select_publication_kind_v1(
                CodeObjectVersion::V5,
                Missing,
                WorkerV2EnvelopeModeV1::NonAuthoritative,
            )
            .unwrap(),
            WorkerV2PublicationKindV1::Raw
        );
        assert_eq!(
            select_publication_kind_v1(
                CodeObjectVersion::V6,
                Present,
                WorkerV2EnvelopeModeV1::NonAuthoritative,
            )
            .unwrap(),
            WorkerV2PublicationKindV1::Finalized
        );
        assert_eq!(
            select_publication_kind_v1(
                CodeObjectVersion::V6,
                Present,
                WorkerV2EnvelopeModeV1::Required,
            )
            .unwrap(),
            WorkerV2PublicationKindV1::FinalizedEnvelopeRequired
        );
        assert!(matches!(
            select_publication_kind_v1(
                CodeObjectVersion::V5,
                Missing,
                WorkerV2EnvelopeModeV1::Required,
            ),
            Err(RestartIntentErrorV1::UnsupportedPublicationRoute { .. })
        ));
        for (version, descriptor) in [
            (CodeObjectVersion::V4, Missing),
            (CodeObjectVersion::V4, Present),
            (CodeObjectVersion::V5, Present),
            (CodeObjectVersion::V6, Missing),
        ] {
            assert!(matches!(
                select_publication_kind_v1(
                    version,
                    descriptor,
                    WorkerV2EnvelopeModeV1::NonAuthoritative,
                ),
                Err(RestartIntentErrorV1::UnsupportedPublicationRoute { .. })
            ));
        }
    }

    #[test]
    fn raw_and_finalized_journals_recover_their_exact_distinct_bytes() {
        for (index, publication) in [
            WorkerV2PublicationKindV1::Raw,
            WorkerV2PublicationKindV1::Finalized,
        ]
        .into_iter()
        .enumerate()
        {
            let seed = 80 + u8::try_from(index).unwrap();
            let directory = TestDirectory::new();
            let producer = producer(seed);
            let attempt = attempt(&directory.0, &producer, seed);
            let (mut output, _, _) = publication_inputs(attempt, seed);
            output.extend_from_slice(match publication {
                WorkerV2PublicationKindV1::Raw => b"-raw",
                WorkerV2PublicationKindV1::Finalized => b"-canonical-finalized",
                WorkerV2PublicationKindV1::FinalizedEnvelopeRequired => {
                    b"-required-canonical-finalized"
                }
            });
            let (_, template, upstream) = publication_inputs(attempt, seed);
            let plan = DurableLinkPublicationPlanV1::new(
                attempt,
                template.scope(),
                template.request(),
                template.worker(),
                template.response(),
                LinkedOutputIdentityV1::from_bytes(Sha256::digest(&output).into()),
                template.finalization(),
                FinalizedOutputIdentityV1::from_bytes(Sha256::digest(&output).into()),
                template.publication(),
            );
            let admission = restart_admission_commitment_v1(publication, plan, upstream, &output);
            let store = WorkerV2ResumeStoreV1::open(&directory.0, &producer).unwrap();
            store
                .persist_pending(publication, attempt, admission)
                .unwrap();
            let persisted = persist_worker_v2_publication_intent_v1(
                &directory.0,
                &producer,
                attempt,
                plan,
                upstream,
                &output,
            )
            .unwrap();
            store
                .persist_ready(publication, attempt, persisted.record().identity())
                .unwrap();
            drop(persisted);

            let state = store.load().unwrap().unwrap();
            assert_eq!(state.publication(), publication);
            let recovered = recover_worker_v2_intent_v1(&store, &producer, state).unwrap();
            assert_eq!(recovered.exact_output(), output);
        }
    }

    #[test]
    fn restart_admission_rejects_publication_kind_substitution() {
        let directory = TestDirectory::new();
        let producer = producer(90);
        let attempt = attempt(&directory.0, &producer, 90);
        let (output, plan, upstream) = publication_inputs(attempt, 90);
        let finalized_admission = restart_admission_commitment_v1(
            WorkerV2PublicationKindV1::Finalized,
            plan,
            upstream,
            &output,
        );
        assert_ne!(
            finalized_admission,
            restart_admission_commitment_v1(
                WorkerV2PublicationKindV1::Raw,
                plan,
                upstream,
                &output,
            )
        );
        assert_ne!(
            finalized_admission,
            restart_admission_commitment_v1(
                WorkerV2PublicationKindV1::FinalizedEnvelopeRequired,
                plan,
                upstream,
                &output,
            )
        );

        let store = WorkerV2ResumeStoreV1::open(&directory.0, &producer).unwrap();
        store
            .persist_pending(WorkerV2PublicationKindV1::Raw, attempt, finalized_admission)
            .unwrap();
        persist_worker_v2_publication_intent_v1(
            &directory.0,
            &producer,
            attempt,
            plan,
            upstream,
            &output,
        )
        .unwrap();
        let state = store.load().unwrap().unwrap();
        assert!(matches!(
            recover_worker_v2_intent_v1(&store, &producer, state),
            Err(RestartIntentErrorV1::IntentIdentityMismatch)
        ));
    }

    #[test]
    fn completed_marker_rejects_publication_kind_and_admission_substitution() {
        let directory = TestDirectory::new();
        let producer = producer(95);
        let attempt = attempt(&directory.0, &producer, 95);
        let (output, plan, upstream) = publication_inputs(attempt, 95);
        let persisted = persist_worker_v2_publication_intent_v1(
            &directory.0,
            &producer,
            attempt,
            plan,
            upstream,
            &output,
        )
        .unwrap();
        let intent = persisted.record().identity();
        drop(persisted);
        let receipt = publish_exact_hsaco_evidence_for_attempt_v1(
            &directory.0,
            &producer,
            attempt,
            plan,
            upstream,
            &output,
        )
        .unwrap()
        .receipt();
        let raw_admission = restart_admission_commitment_v1(
            WorkerV2PublicationKindV1::Raw,
            plan,
            upstream,
            &output,
        );
        let substituted = ResumeMarkerStateV1::Completed {
            legacy: false,
            publication: WorkerV2PublicationKindV1::Finalized,
            attempt,
            admission: raw_admission,
            envelope_inputs: [0; 32],
            envelope: [0; 32],
            intent,
            receipt: ReceiptRecordV1::from_receipt(receipt),
        };
        let store = WorkerV2ResumeStoreV1::open(&directory.0, &producer).unwrap();
        install_marker(&store, &encode_marker(store.package, substituted));

        assert!(matches!(
            recover_worker_v2_intent_v1(&store, &producer, substituted),
            Err(RestartIntentErrorV1::IntentIdentityMismatch)
        ));
        assert_eq!(store.load().unwrap(), Some(substituted));
    }
}
