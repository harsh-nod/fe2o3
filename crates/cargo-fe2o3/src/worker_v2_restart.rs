use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(feature = "worker-v2-fault-injection-test-only")]
use std::ffi::OsStr;

use fe2o3_artifact_transaction::{
    BackendPublicationReceiptV1, BackendPublicationReceiptV2, BuildAttempt, BuildSession,
    DurableLinkPublicationPlanV1, DurablePublishedHsacoClaimV2,
    MAX_WORKER_V2_PUBLICATION_INTENT_CLEANUP_ESCROW_CAPSULE_BYTES_V1, PersistedBackendReceiptV2,
    ProducerIdentity, RecoveredWorkerV2PublicationIntentV1, RecoveredWorkerV2PublicationIntentV2,
    UpstreamCodeObjectEvidenceIdentityV1, WorkerV2PublicationIntentCleanupEscrowOptionsV1,
    WorkerV2PublicationIntentCleanupEscrowStateV1, WorkerV2PublicationIntentCleanupEscrowV1,
    WorkerV2PublicationIntentErrorV1, WorkerV2PublicationIntentErrorV2,
    WorkerV2PublicationIntentIdentityV1, WorkerV2PublicationIntentIdentityV2,
    acquire_worker_v2_publication_intent_lease_v2, clear_worker_v2_publication_intent_v2,
    commit_worker_v2_publication_intent_cleanup_escrow_after_exact_successor_v2,
    commit_worker_v2_publication_intent_cleanup_escrow_v1, persist_worker_v2_publication_intent_v1,
    persist_worker_v2_publication_intent_v2,
    prepare_worker_v2_publication_intent_cleanup_escrow_v1, producer_package_identity_v1,
    read_backend_publication_receipt_v2, recover_published_hsaco_claim_for_attempt_v2,
    recover_worker_v2_publication_intent_cleanup_escrow_v1,
    recover_worker_v2_publication_intent_v1, recover_worker_v2_publication_intent_v2,
    rollback_worker_v2_publication_intent_cleanup_escrow_v1,
};
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_compiler_ffi::CodeObjectVersion;
use fe2o3_hsaco_finalize::{
    CanonicalDescriptorSectionObservationV1, InspectedRawWorkerV2HsacoV1,
    PreparedFinalizedWorkerV2HsacoPublicationV1, PreparedWorkerV2HsacoPublicationV1,
    SealedWorkerV2HsacoPublicationIntentV1, WorkerV2HsacoFinalizationError,
    WorkerV2HsacoPublicationError, finalize_inspected_worker_v2_hsaco_v1,
    prepare_finalized_worker_v2_hsaco_publication_v1, prepare_worker_v2_hsaco_publication_v1,
};
use fe2o3_worker_v2_bundle::{
    MAX_WORKER_V2_ARTIFACT_DIRECTORY_ENTRIES_V1, MAX_WORKER_V2_ENVELOPE_INPUTS_BYTES,
    MAX_WORKER_V2_LOAD_ENVELOPE_BYTES, MAX_WORKER_V2_LOAD_ENVELOPE_BYTES_V2,
    WORKER_V2_LOAD_ENVELOPE_NAME_PREFIX_V1 as ENVELOPE_PREFIX,
    WORKER_V2_LOAD_ENVELOPE_NAME_SUFFIX_V1 as ENVELOPE_SUFFIX, WorkerV2EnvelopeInputsIdentityV1,
    WorkerV2EnvelopeInputsV1, WorkerV2LoadEnvelopeIdentityV1, WorkerV2LoadEnvelopeIdentityV2,
    WorkerV2LoadEnvelopeV1, WorkerV2LoadEnvelopeV2, worker_v2_load_envelope_name_v1,
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
#[cfg(test)]
const OBSOLETE_PROTECTED_MARKER_VERSION_V4: u16 = 4;
const PROTECTED_MARKER_VERSION: u16 = 5;
const PROTECTED_INTENT_SCHEMA_V2: u8 = 2;
const MARKER_CHECKSUM_DOMAIN: &[u8] = b"FE2O3/CARGO-WORKER-V2-RESUME-CHECKSUM/V1\0";
const PROTECTED_MARKER_CHECKSUM_DOMAIN_V5: &[u8] =
    b"FE2O3/CARGO-WORKER-V2-PROTECTED-RESUME-CHECKSUM/V5\0";
const ADMISSION_COMMITMENT_DOMAIN: &[u8] = b"FE2O3/CARGO-WORKER-V2-ADMISSION-COMMITMENT/V1\0";
const PROTECTED_ADMISSION_COMMITMENT_DOMAIN_V2: &[u8] =
    b"FE2O3/CARGO-WORKER-V2-PROTECTED-ADMISSION-COMMITMENT/V2\0";
const MARKER_PREFIX: &str = ".fe2o3-cargo-worker-v2-resume-v1-";
const LOCK_SUFFIX: &str = ".lock";
const RECORD_SUFFIX: &str = ".record";
const TEMP_SUFFIX: &str = ".tmp-";
const ENVELOPE_INPUTS_PREFIX: &str = ".fe2o3-worker-v2-envelope-inputs-v1-";
const ENVELOPE_INPUTS_SUFFIX: &str = ".capsule";
const ENVELOPE_INPUTS_NAME_DOMAIN: &[u8] = b"FE2O3/WORKER-V2-ENVELOPE-INPUTS-NAME/V1\0";
const PROTECTED_ENVELOPE_PREFIX_V2: &str = ".fe2o3-worker-v2-protected-load-envelope-v2-";
const PROTECTED_ENVELOPE_SUFFIX_V2: &str = ".envelope";
const PROTECTED_ENVELOPE_TEMP_PREFIX_V2: &str = ".fe2o3-worker-v2-protected-load-envelope-temp-v2-";
const PROTECTED_CLEANUP_EVIDENCE_MAGIC_V1: &[u8] =
    b"FE2O3-CARGO-WORKER-V2-PROTECTED-CLEANUP-EVIDENCE-V1\0";
const PROTECTED_CLEANUP_EVIDENCE_VERSION_V1: u16 = 1;
const PROTECTED_CLEANUP_EVIDENCE_CHECKSUM_DOMAIN_V1: &[u8] =
    b"FE2O3/CARGO-WORKER-V2-PROTECTED-CLEANUP-EVIDENCE-CHECKSUM/V1\0";
const PROTECTED_CLEANUP_EVIDENCE_MARKER_DOMAIN_V1: &[u8] =
    b"FE2O3/CARGO-WORKER-V2-PROTECTED-CLEANUP-EVIDENCE-MARKER/V1\0";
const PROTECTED_CLEANUP_EVIDENCE_PREFIX_V1: &str =
    ".fe2o3-cargo-worker-v2-protected-cleanup-evidence-v1-";
const PROTECTED_CLEANUP_EVIDENCE_SUFFIX_V1: &str = ".evidence";
const PROTECTED_CLEANUP_LOCAL_NAME_DOMAIN_V1: &[u8] =
    b"FE2O3/CARGO-WORKER-V2-PROTECTED-CLEANUP-LOCAL-NAME/V1\0";
const PROTECTED_CLEANUP_LOCAL_PREFIX_V1: &str =
    ".fe2o3-cargo-worker-v2-protected-cleanup-local-v1-";
const PROTECTED_CLEANUP_MARKER_QUARANTINE_SUFFIX_V1: &str = ".marker.quarantine";
const PROTECTED_CLEANUP_INPUTS_QUARANTINE_SUFFIX_V1: &str = ".inputs.quarantine";
const MAX_ENVELOPE_INPUT_RESIDUE_ENTRIES: usize = 256;
const MAX_ENVELOPE_TEMP_RESIDUE_ENTRIES: usize = 256;
const MAX_MARKER_TEMP_RESIDUE_ENTRIES: usize = 256;
const MAX_PROTECTED_CLEANUP_TEMP_RESIDUE_ENTRIES_V1: usize = 256;
const RECEIPT_FIELDS: usize = 7;
const COMPILER_CLOSURE_DIGEST_FIELDS_V2: usize = 7;
const COMPILER_CLOSURE_BYTES_V2: usize = COMPILER_CLOSURE_DIGEST_FIELDS_V2 * 32 + 2;
const PREVIOUS_MARKER_BYTES: usize =
    MARKER_MAGIC.len() + 2 + 1 + 1 + 32 + 8 + 16 + 32 + 32 + 32 + RECEIPT_FIELDS * 32 + 32;
const MARKER_BYTES: usize = PREVIOUS_MARKER_BYTES + 32 + 32;
const OBSOLETE_PROTECTED_MARKER_BYTES_V4: usize = MARKER_BYTES + 1;
const PROTECTED_MARKER_BYTES: usize =
    OBSOLETE_PROTECTED_MARKER_BYTES_V4 + COMPILER_CLOSURE_BYTES_V2;
const MAX_MARKER_BYTES: usize = PROTECTED_MARKER_BYTES;
const LEGACY_MARKER_BYTES: usize =
    MARKER_MAGIC.len() + 2 + 1 + 32 + 8 + 16 + 32 + 32 + RECEIPT_FIELDS * 32 + 32;
const PROTECTED_CLEANUP_LOCAL_FILE_SNAPSHOT_BYTES_V1: usize = 5 * 8;
const PROTECTED_CLEANUP_EVIDENCE_FIXED_BODY_BYTES_V1: usize = PROTECTED_CLEANUP_EVIDENCE_MAGIC_V1
    .len()
    + 2
    + 32
    + 1
    + 8
    + 16
    + 32
    + 32
    + 32
    + 32
    + 32
    + RECEIPT_FIELDS * 32
    + COMPILER_CLOSURE_BYTES_V2
    + 32
    + 2 * PROTECTED_CLEANUP_LOCAL_FILE_SNAPSHOT_BYTES_V1
    + MAX_WORKER_V2_PUBLICATION_INTENT_CLEANUP_ESCROW_CAPSULE_BYTES_V1;
const MAX_PROTECTED_CLEANUP_EVIDENCE_BYTES_V1: usize =
    PROTECTED_CLEANUP_EVIDENCE_FIXED_BODY_BYTES_V1 + 32;

static NEXT_TEMP: AtomicU64 = AtomicU64::new(1);

fn count_restart_artifact_entry(entries: &mut usize, name: &[u8]) -> Result<bool, ()> {
    if matches!(name, b"." | b"..") {
        return Ok(false);
    }
    *entries = entries
        .checked_add(1)
        .filter(|entries| *entries <= MAX_WORKER_V2_ARTIFACT_DIRECTORY_ENTRIES_V1)
        .ok_or(())?;
    Ok(true)
}

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
pub(crate) struct ReceiptRecordV2 {
    fields: [[u8; 32]; RECEIPT_FIELDS],
    compiler_closure: Option<CompilerClosureV2>,
}

impl ReceiptRecordV2 {
    pub(crate) fn from_receipt(receipt: BackendPublicationReceiptV2) -> Self {
        Self {
            fields: [
                receipt.attempt_identity(),
                receipt.producer_identity(),
                receipt.scope_identity(),
                receipt.plan_commitment(),
                receipt.upstream_evidence_identity(),
                receipt.finalized_output_identity(),
                receipt.publication_identity(),
            ],
            compiler_closure: Some(receipt.compiler_closure()),
        }
    }

    pub(crate) fn matches(self, receipt: BackendPublicationReceiptV2) -> bool {
        self == Self::from_receipt(receipt)
    }

    const fn compiler_closure(self) -> Option<CompilerClosureV2> {
        self.compiler_closure
    }

    fn encode(self, bytes: &mut Vec<u8>) {
        for field in self.fields {
            bytes.extend_from_slice(&field);
        }
        encode_compiler_closure_v2(self.compiler_closure, bytes);
    }

    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, &'static str> {
        let mut fields = [[0_u8; 32]; RECEIPT_FIELDS];
        for field in &mut fields {
            *field = decoder.array()?;
        }
        Ok(Self {
            fields,
            compiler_closure: decode_optional_compiler_closure_v2(decoder)?,
        })
    }

    fn is_zero(self) -> bool {
        self.fields.iter().all(|field| *field == [0; 32]) && self.compiler_closure.is_none()
    }
}

fn encode_compiler_closure_v2(closure: Option<CompilerClosureV2>, bytes: &mut Vec<u8>) {
    let Some(closure) = closure else {
        bytes.resize(bytes.len() + COMPILER_CLOSURE_BYTES_V2, 0);
        return;
    };
    for digest in [
        closure.cargo_executable_sha256(),
        closure.cargo_binding_trampoline_sha256(),
        closure.cargo_fe2o3_binding_wrapper_sha256(),
        closure.rustc_executable_sha256(),
        closure.rustc_runtime_tree_sha256(),
        closure.codegen_backend_sha256(),
    ] {
        bytes.extend_from_slice(&digest);
    }
    bytes.extend_from_slice(
        &closure
            .cargo_binding_transition_protocol_version()
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&closure.identity_sha256());
}

fn decode_optional_compiler_closure_v2(
    decoder: &mut Decoder<'_>,
) -> Result<Option<CompilerClosureV2>, &'static str> {
    let cargo = decoder.array()?;
    let trampoline = decoder.array()?;
    let wrapper = decoder.array()?;
    let rustc = decoder.array()?;
    let runtime = decoder.array()?;
    let backend = decoder.array()?;
    let protocol = decoder.u16()?;
    let identity = decoder.array()?;
    if [
        cargo, trampoline, wrapper, rustc, runtime, backend, identity,
    ]
    .iter()
    .all(|digest| *digest == [0; 32])
        && protocol == 0
    {
        return Ok(None);
    }
    CompilerClosureV2::from_pins_and_identity(
        cargo, trampoline, wrapper, rustc, runtime, backend, protocol, identity,
    )
    .map(Some)
    .map_err(|_| "marker receipt contains a noncanonical compiler closure")
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

/// Canonical protected restart state bound to a V2 intent and exact V2 completion receipt.
#[allow(dead_code, clippy::large_enum_variant)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResumeMarkerStateV2 {
    Pending {
        publication: WorkerV2PublicationKindV1,
        attempt: BuildAttempt,
        admission: [u8; 32],
        envelope_inputs: [u8; 32],
    },
    Ready {
        publication: WorkerV2PublicationKindV1,
        attempt: BuildAttempt,
        admission: [u8; 32],
        envelope_inputs: [u8; 32],
        intent: WorkerV2PublicationIntentIdentityV2,
    },
    Completed {
        publication: WorkerV2PublicationKindV1,
        attempt: BuildAttempt,
        admission: [u8; 32],
        envelope_inputs: [u8; 32],
        envelope: [u8; 32],
        intent: WorkerV2PublicationIntentIdentityV2,
        receipt: ReceiptRecordV2,
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

/// Closure-bound protected counterpart retained until the V2 binding owner is integrated.
#[allow(dead_code)] // Its fields are consumed by that pending binding integration.
pub(crate) struct PersistedAdmittedWorkerV2IntentV2 {
    pub(crate) intent: RecoveredWorkerV2PublicationIntentV2,
    pub(crate) publication: WorkerV2PublicationKindV1,
}

#[derive(Debug)]
pub(crate) enum RestartIntentErrorV2 {
    Marker(ResumeMarkerErrorV1),
    Intent(WorkerV2PublicationIntentErrorV2),
    IntentIdentityMismatch,
    MissingEnvelopeInputs,
}

impl fmt::Display for RestartIntentErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Marker(error) => {
                write!(
                    formatter,
                    "protected Worker V2 resume marker failed: {error}"
                )
            }
            Self::Intent(error) => {
                write!(
                    formatter,
                    "protected Worker V2 publication intent failed: {error}"
                )
            }
            Self::IntentIdentityMismatch => formatter.write_str(
                "recovered protected Worker V2 publication intent does not match its resume marker",
            ),
            Self::MissingEnvelopeInputs => formatter.write_str(
                "canonical protected Worker V2 envelope inputs are missing or unexpected",
            ),
        }
    }
}

impl Error for RestartIntentErrorV2 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Marker(error) => Some(error),
            Self::Intent(error) => Some(error),
            Self::IntentIdentityMismatch | Self::MissingEnvelopeInputs => None,
        }
    }
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
    let prepared =
        prepare_admitted_worker_v2_intent_v1(producer, inspected, envelope_mode, envelope_inputs)?;
    let (intent, publication) = persist_admitted_worker_v2_intent::<OrdinaryIntentSchemaV1>(
        store,
        producer,
        prepared.inputs(envelope_inputs),
        (),
    )?;
    Ok(PersistedAdmittedWorkerV2IntentV1 {
        intent,
        publication,
    })
}

struct PreparedAdmittedWorkerV2IntentV1 {
    publication: WorkerV2PublicationKindV1,
    plan: DurableLinkPublicationPlanV1,
    upstream: UpstreamCodeObjectEvidenceIdentityV1,
    exact_output: Vec<u8>,
}

impl PreparedAdmittedWorkerV2IntentV1 {
    fn inputs<'a>(
        &'a self,
        envelope_inputs: Option<&'a WorkerV2EnvelopeInputsV1>,
    ) -> AdmittedWorkerV2IntentInputs<'a> {
        AdmittedWorkerV2IntentInputs {
            publication: self.publication,
            plan: self.plan,
            upstream: self.upstream,
            exact_output: &self.exact_output,
            envelope_inputs,
        }
    }
}

fn prepare_admitted_worker_v2_intent_v1(
    producer: &ProducerIdentity,
    inspected: InspectedRawWorkerV2HsacoV1,
    envelope_mode: WorkerV2EnvelopeModeV1,
    envelope_inputs: Option<&WorkerV2EnvelopeInputsV1>,
) -> Result<PreparedAdmittedWorkerV2IntentV1, RestartIntentErrorV1> {
    let publication = select_publication_kind_v1(
        inspected.code_object_version(),
        inspected.canonical_descriptor_section(),
        envelope_mode,
    )?;
    let (_attempt, plan, upstream, exact_bytes) = match publication {
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
    Ok(PreparedAdmittedWorkerV2IntentV1 {
        publication,
        plan,
        upstream,
        exact_output: exact_bytes,
    })
}

/// Persists an already-admitted protected publication without depending on its future wrapper.
#[allow(dead_code)] // The protected binding caller lands with its inspected wrapper.
#[allow(clippy::too_many_arguments)]
pub(crate) fn persist_admitted_worker_v2_intent_v2(
    store: &WorkerV2ResumeStoreV2,
    producer: &ProducerIdentity,
    publication: WorkerV2PublicationKindV1,
    plan: DurableLinkPublicationPlanV1,
    upstream: UpstreamCodeObjectEvidenceIdentityV1,
    exact_output: &[u8],
    envelope_inputs: Option<&WorkerV2EnvelopeInputsV1>,
    expected_compiler_closure: CompilerClosureV2,
) -> Result<PersistedAdmittedWorkerV2IntentV2, RestartIntentErrorV2> {
    if producer != &store.producer {
        return Err(RestartIntentErrorV2::IntentIdentityMismatch);
    }
    let inputs = AdmittedWorkerV2IntentInputs {
        publication,
        plan,
        upstream,
        exact_output,
        envelope_inputs,
    };
    let (intent, publication) = persist_admitted_worker_v2_intent::<ProtectedIntentSchemaV2>(
        store,
        producer,
        inputs,
        expected_compiler_closure,
    )?;
    Ok(PersistedAdmittedWorkerV2IntentV2 {
        intent,
        publication,
    })
}

#[derive(Clone, Copy)]
struct AdmittedWorkerV2IntentInputs<'a> {
    publication: WorkerV2PublicationKindV1,
    plan: DurableLinkPublicationPlanV1,
    upstream: UpstreamCodeObjectEvidenceIdentityV1,
    exact_output: &'a [u8],
    envelope_inputs: Option<&'a WorkerV2EnvelopeInputsV1>,
}

trait WorkerV2IntentSchema {
    type Store;
    type State: Copy;
    type Binding: Copy;
    type Recovered;
    type Identity: Copy + Eq;
    type IntentError;
    type Error;

    fn marker_error(error: ResumeMarkerErrorV1) -> Self::Error;
    fn intent_error(error: Self::IntentError) -> Self::Error;
    fn identity_mismatch() -> Self::Error;
    fn missing_envelope_inputs() -> Self::Error;

    fn verify_output_path(store: &Self::Store) -> Result<(), ResumeMarkerErrorV1>;
    fn persist_envelope_inputs(
        store: &Self::Store,
        attempt: BuildAttempt,
        inputs: &WorkerV2EnvelopeInputsV1,
    ) -> Result<(), ResumeMarkerErrorV1>;
    fn recover_envelope_inputs_identity(
        store: &Self::Store,
        attempt: BuildAttempt,
    ) -> Result<WorkerV2EnvelopeInputsIdentityV1, ResumeMarkerErrorV1>;
    fn persist_pending(
        store: &Self::Store,
        inputs: AdmittedWorkerV2IntentInputs<'_>,
        admission: [u8; 32],
        envelope_inputs: Option<WorkerV2EnvelopeInputsIdentityV1>,
    ) -> Result<(), ResumeMarkerErrorV1>;
    fn persist_ready(
        store: &Self::Store,
        publication: WorkerV2PublicationKindV1,
        attempt: BuildAttempt,
        recovered: &Self::Recovered,
        binding: Self::Binding,
    ) -> Result<(), ResumeMarkerErrorV1>;
    fn persist_intent(
        store: &Self::Store,
        producer: &ProducerIdentity,
        inputs: AdmittedWorkerV2IntentInputs<'_>,
        binding: Self::Binding,
    ) -> Result<Self::Recovered, Self::IntentError>;
    fn recover_intent(
        store: &Self::Store,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        binding: Self::Binding,
    ) -> Result<Self::Recovered, Self::IntentError>;

    fn recovered_identity(recovered: &Self::Recovered) -> Self::Identity;
    fn recovered_plan(recovered: &Self::Recovered) -> DurableLinkPublicationPlanV1;
    fn recovered_upstream(recovered: &Self::Recovered) -> UpstreamCodeObjectEvidenceIdentityV1;
    fn recovered_output(recovered: &Self::Recovered) -> &[u8];
    fn recovered_binding_matches(recovered: &Self::Recovered, expected: Self::Binding) -> bool;
    fn admission(
        publication: WorkerV2PublicationKindV1,
        plan: DurableLinkPublicationPlanV1,
        upstream: UpstreamCodeObjectEvidenceIdentityV1,
        output: &[u8],
        envelope_inputs: Option<WorkerV2EnvelopeInputsIdentityV1>,
        binding: Self::Binding,
    ) -> [u8; 32];

    fn state_attempt(state: Self::State) -> BuildAttempt;
    fn state_publication(state: Self::State) -> WorkerV2PublicationKindV1;
    fn state_admission(state: Self::State) -> [u8; 32];
    fn state_envelope_inputs(state: Self::State) -> [u8; 32];
    fn state_intent(state: Self::State) -> Option<Self::Identity>;
    fn finish_recovery(
        store: &Self::Store,
        state: Self::State,
        current_admission: [u8; 32],
        recovered: &Self::Recovered,
        binding: Self::Binding,
    ) -> Result<(), Self::Error>;
}

struct OrdinaryIntentSchemaV1;
struct ProtectedIntentSchemaV2;

fn persist_admitted_worker_v2_intent<S: WorkerV2IntentSchema>(
    store: &S::Store,
    producer: &ProducerIdentity,
    inputs: AdmittedWorkerV2IntentInputs<'_>,
    binding: S::Binding,
) -> Result<(S::Recovered, WorkerV2PublicationKindV1), S::Error> {
    let envelope_inputs_identity = match (
        inputs.publication.requires_envelope(),
        inputs.envelope_inputs,
    ) {
        (true, Some(inputs)) => Some(inputs.identity()),
        (false, None) => None,
        (true, None) | (false, Some(_)) => return Err(S::missing_envelope_inputs()),
    };
    let admission = S::admission(
        inputs.publication,
        inputs.plan,
        inputs.upstream,
        inputs.exact_output,
        envelope_inputs_identity,
        binding,
    );
    // A required marker is recoverable only after its exact capsule name and bytes are durable.
    if let Some(envelope_inputs) = inputs.envelope_inputs {
        S::persist_envelope_inputs(store, inputs.plan.attempt(), envelope_inputs)
            .map_err(S::marker_error)?;
        #[cfg(feature = "worker-v2-fault-injection-test-only")]
        injected_fault_point_v1("envelope-inputs-persisted");
    }
    S::persist_pending(store, inputs, admission, envelope_inputs_identity)
        .map_err(S::marker_error)?;
    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    injected_fault_point_v1("pending-marker");
    S::verify_output_path(store).map_err(S::marker_error)?;
    let persisted = S::persist_intent(store, producer, inputs, binding).map_err(S::intent_error)?;
    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    injected_fault_point_v1("pending-intent");
    S::verify_output_path(store).map_err(S::marker_error)?;
    if !recovered_matches_admitted_inputs::<S>(
        &persisted,
        inputs,
        envelope_inputs_identity,
        admission,
        binding,
    ) {
        return Err(S::identity_mismatch());
    }
    S::persist_ready(
        store,
        inputs.publication,
        inputs.plan.attempt(),
        &persisted,
        binding,
    )
    .map_err(S::marker_error)?;
    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    injected_fault_point_v1("ready");
    Ok((persisted, inputs.publication))
}

fn recovered_matches_admitted_inputs<S: WorkerV2IntentSchema>(
    recovered: &S::Recovered,
    inputs: AdmittedWorkerV2IntentInputs<'_>,
    envelope_inputs: Option<WorkerV2EnvelopeInputsIdentityV1>,
    admission: [u8; 32],
    binding: S::Binding,
) -> bool {
    let plan = S::recovered_plan(recovered);
    let upstream = S::recovered_upstream(recovered);
    let output = S::recovered_output(recovered);
    S::recovered_binding_matches(recovered, binding)
        && plan == inputs.plan
        && upstream == inputs.upstream
        && output == inputs.exact_output
        && S::admission(
            inputs.publication,
            plan,
            upstream,
            output,
            envelope_inputs,
            binding,
        ) == admission
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
    recover_worker_v2_intent::<OrdinaryIntentSchemaV1>(store, producer, state, ())
}

/// Recovers only a closure-equal V2 record, promotes Pending, and resumes interrupted Ready cleanup.
#[allow(dead_code)] // The protected binding caller lands with its inspected wrapper.
pub(crate) fn recover_worker_v2_intent_v2(
    store: &WorkerV2ResumeStoreV2,
    producer: &ProducerIdentity,
    state: ResumeMarkerStateV2,
    expected_compiler_closure: CompilerClosureV2,
) -> Result<RecoveredWorkerV2PublicationIntentV2, RestartIntentErrorV2> {
    if producer != &store.producer {
        return Err(RestartIntentErrorV2::IntentIdentityMismatch);
    }
    store
        .validate_completed_state(state, expected_compiler_closure)
        .map_err(RestartIntentErrorV2::Marker)?;
    recover_worker_v2_intent::<ProtectedIntentSchemaV2>(
        store,
        producer,
        state,
        expected_compiler_closure,
    )
}

/// Clears only the exact closure-bound V2 intent named by a durable protected completion marker.
#[allow(dead_code)] // The protected binding caller lands with its inspected wrapper.
pub(crate) fn clear_worker_v2_intent_v2(
    store: &WorkerV2ResumeStoreV2,
    producer: &ProducerIdentity,
    completed: ResumeMarkerStateV2,
    receipt: BackendPublicationReceiptV2,
    expected_compiler_closure: CompilerClosureV2,
) -> Result<(), RestartIntentErrorV2> {
    if producer != &store.producer {
        return Err(RestartIntentErrorV2::IntentIdentityMismatch);
    }
    let ResumeMarkerStateV2::Completed {
        attempt, intent, ..
    } = completed
    else {
        return Err(RestartIntentErrorV2::Marker(
            ResumeMarkerErrorV1::InvalidTransition,
        ));
    };
    store
        .validate_completed_receipt(completed, receipt, expected_compiler_closure)
        .map_err(RestartIntentErrorV2::Marker)?;
    store
        .verify_output_path()
        .map_err(RestartIntentErrorV2::Marker)?;
    let recovered = match recover_worker_v2_publication_intent_v2(
        &store.inner.display_path,
        producer,
        attempt,
        expected_compiler_closure,
    ) {
        Ok(recovered) => Some(recovered),
        Err(WorkerV2PublicationIntentErrorV2::NotFound) => None,
        Err(error) => return Err(RestartIntentErrorV2::Intent(error)),
    };
    store
        .verify_output_path()
        .map_err(RestartIntentErrorV2::Marker)?;
    if completed.publication().requires_envelope() {
        if store.load().map_err(RestartIntentErrorV2::Marker)? != Some(completed) {
            return Err(RestartIntentErrorV2::Marker(
                ResumeMarkerErrorV1::InvalidTransition,
            ));
        }
        store
            .ensure_protected_cleanup_evidence(completed, expected_compiler_closure)
            .map_err(RestartIntentErrorV2::Marker)?;
        return Ok(());
    }
    if let Err(error) = store.validate_completed_restart_inputs(
        completed,
        receipt,
        expected_compiler_closure,
        recovered.as_ref(),
    ) {
        return Err(RestartIntentErrorV2::Marker(error));
    }
    if store.load().map_err(RestartIntentErrorV2::Marker)? != Some(completed) {
        return Err(RestartIntentErrorV2::Marker(
            ResumeMarkerErrorV1::InvalidTransition,
        ));
    }
    let Some(recovered) = recovered else {
        return Ok(());
    };
    if recovered.record().identity() != intent {
        return Err(RestartIntentErrorV2::IntentIdentityMismatch);
    }
    if store.load().map_err(RestartIntentErrorV2::Marker)? != Some(completed) {
        return Err(RestartIntentErrorV2::Marker(
            ResumeMarkerErrorV1::InvalidTransition,
        ));
    }
    store
        .verify_output_path()
        .map_err(RestartIntentErrorV2::Marker)?;
    clear_worker_v2_publication_intent_v2(
        &store.inner.display_path,
        producer,
        attempt,
        expected_compiler_closure,
        recovered.record().identity(),
    )
    .map_err(RestartIntentErrorV2::Intent)?;
    store
        .verify_output_path()
        .map_err(RestartIntentErrorV2::Marker)?;
    Ok(())
}

fn recover_worker_v2_intent<S: WorkerV2IntentSchema>(
    store: &S::Store,
    producer: &ProducerIdentity,
    state: S::State,
    binding: S::Binding,
) -> Result<S::Recovered, S::Error> {
    let attempt = S::state_attempt(state);
    let publication = S::state_publication(state);
    let envelope_inputs_identity = if publication.requires_envelope() {
        let identity =
            S::recover_envelope_inputs_identity(store, attempt).map_err(S::marker_error)?;
        if S::state_envelope_inputs(state) != identity.as_bytes() {
            return Err(S::identity_mismatch());
        }
        Some(identity)
    } else {
        None
    };
    S::verify_output_path(store).map_err(S::marker_error)?;
    let recovered =
        S::recover_intent(store, producer, attempt, binding).map_err(S::intent_error)?;
    S::verify_output_path(store).map_err(S::marker_error)?;
    let identity = S::recovered_identity(&recovered);
    if S::state_intent(state).is_some_and(|expected| expected != identity)
        || !S::recovered_binding_matches(&recovered, binding)
    {
        return Err(S::identity_mismatch());
    }
    let current_admission = S::admission(
        publication,
        S::recovered_plan(&recovered),
        S::recovered_upstream(&recovered),
        S::recovered_output(&recovered),
        envelope_inputs_identity,
        binding,
    );
    S::finish_recovery(store, state, current_admission, &recovered, binding)?;
    Ok(recovered)
}

impl WorkerV2IntentSchema for OrdinaryIntentSchemaV1 {
    type Store = WorkerV2ResumeStoreV1;
    type State = ResumeMarkerStateV1;
    type Binding = ();
    type Recovered = RecoveredWorkerV2PublicationIntentV1;
    type Identity = WorkerV2PublicationIntentIdentityV1;
    type IntentError = WorkerV2PublicationIntentErrorV1;
    type Error = RestartIntentErrorV1;

    fn marker_error(error: ResumeMarkerErrorV1) -> Self::Error {
        RestartIntentErrorV1::Marker(error)
    }

    fn intent_error(error: Self::IntentError) -> Self::Error {
        RestartIntentErrorV1::Intent(error)
    }

    fn identity_mismatch() -> Self::Error {
        RestartIntentErrorV1::IntentIdentityMismatch
    }

    fn missing_envelope_inputs() -> Self::Error {
        RestartIntentErrorV1::MissingEnvelopeInputs
    }

    fn verify_output_path(store: &Self::Store) -> Result<(), ResumeMarkerErrorV1> {
        store.verify_output_path()
    }

    fn persist_envelope_inputs(
        store: &Self::Store,
        attempt: BuildAttempt,
        inputs: &WorkerV2EnvelopeInputsV1,
    ) -> Result<(), ResumeMarkerErrorV1> {
        store.persist_envelope_inputs(attempt, inputs)
    }

    fn recover_envelope_inputs_identity(
        store: &Self::Store,
        attempt: BuildAttempt,
    ) -> Result<WorkerV2EnvelopeInputsIdentityV1, ResumeMarkerErrorV1> {
        store
            .recover_envelope_inputs(attempt)
            .map(|inputs| inputs.identity())
    }

    fn persist_pending(
        store: &Self::Store,
        inputs: AdmittedWorkerV2IntentInputs<'_>,
        admission: [u8; 32],
        envelope_inputs: Option<WorkerV2EnvelopeInputsIdentityV1>,
    ) -> Result<(), ResumeMarkerErrorV1> {
        store.persist_pending_with_envelope_inputs(
            inputs.publication,
            inputs.plan.attempt(),
            admission,
            envelope_inputs,
        )
    }

    fn persist_ready(
        store: &Self::Store,
        publication: WorkerV2PublicationKindV1,
        attempt: BuildAttempt,
        recovered: &Self::Recovered,
        (): Self::Binding,
    ) -> Result<(), ResumeMarkerErrorV1> {
        store.persist_ready(publication, attempt, recovered.record().identity())
    }

    fn persist_intent(
        store: &Self::Store,
        producer: &ProducerIdentity,
        inputs: AdmittedWorkerV2IntentInputs<'_>,
        (): Self::Binding,
    ) -> Result<Self::Recovered, Self::IntentError> {
        persist_worker_v2_publication_intent_v1(
            &store.display_path,
            producer,
            inputs.plan.attempt(),
            inputs.plan,
            inputs.upstream,
            inputs.exact_output,
        )
    }

    fn recover_intent(
        store: &Self::Store,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        (): Self::Binding,
    ) -> Result<Self::Recovered, Self::IntentError> {
        recover_worker_v2_publication_intent_v1(&store.display_path, producer, attempt)
    }

    fn recovered_identity(recovered: &Self::Recovered) -> Self::Identity {
        recovered.record().identity()
    }

    fn recovered_plan(recovered: &Self::Recovered) -> DurableLinkPublicationPlanV1 {
        recovered.record().plan()
    }

    fn recovered_upstream(recovered: &Self::Recovered) -> UpstreamCodeObjectEvidenceIdentityV1 {
        recovered.record().upstream_evidence()
    }

    fn recovered_output(recovered: &Self::Recovered) -> &[u8] {
        recovered.exact_output()
    }

    fn recovered_binding_matches(_recovered: &Self::Recovered, (): Self::Binding) -> bool {
        true
    }

    fn admission(
        publication: WorkerV2PublicationKindV1,
        plan: DurableLinkPublicationPlanV1,
        upstream: UpstreamCodeObjectEvidenceIdentityV1,
        output: &[u8],
        envelope_inputs: Option<WorkerV2EnvelopeInputsIdentityV1>,
        (): Self::Binding,
    ) -> [u8; 32] {
        restart_admission_commitment_with_inputs_v1(
            publication,
            plan,
            upstream,
            output,
            envelope_inputs,
        )
    }

    fn state_attempt(state: Self::State) -> BuildAttempt {
        state.attempt()
    }

    fn state_publication(state: Self::State) -> WorkerV2PublicationKindV1 {
        state.publication()
    }

    fn state_admission(state: Self::State) -> [u8; 32] {
        state.admission()
    }

    fn state_envelope_inputs(state: Self::State) -> [u8; 32] {
        state.envelope_inputs()
    }

    fn state_intent(state: Self::State) -> Option<Self::Identity> {
        state.intent()
    }

    fn finish_recovery(
        store: &Self::Store,
        state: Self::State,
        current_admission: [u8; 32],
        recovered: &Self::Recovered,
        (): Self::Binding,
    ) -> Result<(), Self::Error> {
        let record = recovered.record();
        let intent = record.identity();
        if state.is_legacy() {
            if state.publication() != WorkerV2PublicationKindV1::Raw {
                return Err(Self::identity_mismatch());
            }
            if matches!(state, ResumeMarkerStateV1::Pending { .. })
                && legacy_restart_admission_commitment_v1(
                    record.plan(),
                    record.upstream_evidence(),
                    recovered.exact_output(),
                ) != state.admission()
            {
                return Err(Self::identity_mismatch());
            }
            if !matches!(state, ResumeMarkerStateV1::Completed { .. }) {
                store.migrate_legacy_to_ready(state, current_admission, intent)?;
            }
        } else if current_admission != Self::state_admission(state) {
            return Err(Self::identity_mismatch());
        } else if matches!(state, ResumeMarkerStateV1::Pending { .. }) {
            store.persist_ready(state.publication(), state.attempt(), intent)?;
        }
        Ok(())
    }
}

impl WorkerV2IntentSchema for ProtectedIntentSchemaV2 {
    type Store = WorkerV2ResumeStoreV2;
    type State = ResumeMarkerStateV2;
    type Binding = CompilerClosureV2;
    type Recovered = RecoveredWorkerV2PublicationIntentV2;
    type Identity = WorkerV2PublicationIntentIdentityV2;
    type IntentError = WorkerV2PublicationIntentErrorV2;
    type Error = RestartIntentErrorV2;

    fn marker_error(error: ResumeMarkerErrorV1) -> Self::Error {
        RestartIntentErrorV2::Marker(error)
    }

    fn intent_error(error: Self::IntentError) -> Self::Error {
        RestartIntentErrorV2::Intent(error)
    }

    fn identity_mismatch() -> Self::Error {
        RestartIntentErrorV2::IntentIdentityMismatch
    }

    fn missing_envelope_inputs() -> Self::Error {
        RestartIntentErrorV2::MissingEnvelopeInputs
    }

    fn verify_output_path(store: &Self::Store) -> Result<(), ResumeMarkerErrorV1> {
        store.verify_output_path()
    }

    fn persist_envelope_inputs(
        store: &Self::Store,
        attempt: BuildAttempt,
        inputs: &WorkerV2EnvelopeInputsV1,
    ) -> Result<(), ResumeMarkerErrorV1> {
        store.persist_envelope_inputs(attempt, inputs)
    }

    fn recover_envelope_inputs_identity(
        store: &Self::Store,
        attempt: BuildAttempt,
    ) -> Result<WorkerV2EnvelopeInputsIdentityV1, ResumeMarkerErrorV1> {
        store
            .recover_envelope_inputs(attempt)
            .map(|inputs| inputs.identity())
    }

    fn persist_pending(
        store: &Self::Store,
        inputs: AdmittedWorkerV2IntentInputs<'_>,
        admission: [u8; 32],
        envelope_inputs: Option<WorkerV2EnvelopeInputsIdentityV1>,
    ) -> Result<(), ResumeMarkerErrorV1> {
        store.persist_pending_with_envelope_inputs(
            inputs.publication,
            inputs.plan.attempt(),
            admission,
            envelope_inputs,
        )
    }

    fn persist_ready(
        store: &Self::Store,
        publication: WorkerV2PublicationKindV1,
        attempt: BuildAttempt,
        recovered: &Self::Recovered,
        binding: Self::Binding,
    ) -> Result<(), ResumeMarkerErrorV1> {
        store.persist_ready(publication, attempt, recovered, binding)
    }

    fn persist_intent(
        store: &Self::Store,
        producer: &ProducerIdentity,
        inputs: AdmittedWorkerV2IntentInputs<'_>,
        binding: Self::Binding,
    ) -> Result<Self::Recovered, Self::IntentError> {
        persist_worker_v2_publication_intent_v2(
            &store.inner.display_path,
            producer,
            inputs.plan.attempt(),
            inputs.plan,
            inputs.upstream,
            binding,
            inputs.exact_output,
        )
    }

    fn recover_intent(
        store: &Self::Store,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        binding: Self::Binding,
    ) -> Result<Self::Recovered, Self::IntentError> {
        recover_worker_v2_publication_intent_v2(
            &store.inner.display_path,
            producer,
            attempt,
            binding,
        )
    }

    fn recovered_identity(recovered: &Self::Recovered) -> Self::Identity {
        recovered.record().identity()
    }

    fn recovered_plan(recovered: &Self::Recovered) -> DurableLinkPublicationPlanV1 {
        recovered.record().plan()
    }

    fn recovered_upstream(recovered: &Self::Recovered) -> UpstreamCodeObjectEvidenceIdentityV1 {
        recovered.record().upstream_evidence()
    }

    fn recovered_output(recovered: &Self::Recovered) -> &[u8] {
        recovered.exact_output()
    }

    fn recovered_binding_matches(recovered: &Self::Recovered, expected: Self::Binding) -> bool {
        recovered.compiler_closure() == expected
    }

    fn admission(
        publication: WorkerV2PublicationKindV1,
        plan: DurableLinkPublicationPlanV1,
        upstream: UpstreamCodeObjectEvidenceIdentityV1,
        output: &[u8],
        envelope_inputs: Option<WorkerV2EnvelopeInputsIdentityV1>,
        binding: Self::Binding,
    ) -> [u8; 32] {
        restart_admission_commitment_with_inputs_v2(
            publication,
            plan,
            upstream,
            output,
            envelope_inputs,
            binding,
        )
    }

    fn state_attempt(state: Self::State) -> BuildAttempt {
        state.attempt()
    }

    fn state_publication(state: Self::State) -> WorkerV2PublicationKindV1 {
        state.publication()
    }

    fn state_admission(state: Self::State) -> [u8; 32] {
        state.admission()
    }

    fn state_envelope_inputs(state: Self::State) -> [u8; 32] {
        state.envelope_inputs()
    }

    fn state_intent(state: Self::State) -> Option<Self::Identity> {
        state.intent()
    }

    fn finish_recovery(
        store: &Self::Store,
        state: Self::State,
        current_admission: [u8; 32],
        recovered: &Self::Recovered,
        binding: Self::Binding,
    ) -> Result<(), Self::Error> {
        if current_admission != Self::state_admission(state) {
            return Err(Self::identity_mismatch());
        }
        if matches!(
            state,
            ResumeMarkerStateV2::Pending { .. } | ResumeMarkerStateV2::Ready { .. }
        ) {
            store
                .persist_ready(state.publication(), state.attempt(), recovered, binding)
                .map_err(Self::marker_error)?;
        }
        Ok(())
    }
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
        update_restart_admission_commitment(
            digest,
            publication,
            plan,
            upstream,
            output,
            envelope_inputs,
        );
    })
}

fn restart_admission_commitment_with_inputs_v2(
    publication: WorkerV2PublicationKindV1,
    plan: DurableLinkPublicationPlanV1,
    upstream: UpstreamCodeObjectEvidenceIdentityV1,
    output: &[u8],
    envelope_inputs: Option<WorkerV2EnvelopeInputsIdentityV1>,
    compiler_closure: CompilerClosureV2,
) -> [u8; 32] {
    hash_identity(PROTECTED_ADMISSION_COMMITMENT_DOMAIN_V2, |digest| {
        update_restart_admission_commitment(
            digest,
            publication,
            plan,
            upstream,
            output,
            envelope_inputs,
        );
        digest.update(compiler_closure.cargo_executable_sha256());
        digest.update(compiler_closure.cargo_binding_trampoline_sha256());
        digest.update(compiler_closure.cargo_fe2o3_binding_wrapper_sha256());
        digest.update(compiler_closure.rustc_executable_sha256());
        digest.update(compiler_closure.rustc_runtime_tree_sha256());
        digest.update(compiler_closure.codegen_backend_sha256());
        digest.update(
            compiler_closure
                .cargo_binding_transition_protocol_version()
                .to_le_bytes(),
        );
        digest.update(compiler_closure.identity_sha256());
    })
}

fn update_restart_admission_commitment(
    digest: &mut Sha256,
    publication: WorkerV2PublicationKindV1,
    plan: DurableLinkPublicationPlanV1,
    upstream: UpstreamCodeObjectEvidenceIdentityV1,
    output: &[u8],
    envelope_inputs: Option<WorkerV2EnvelopeInputsIdentityV1>,
) {
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

fn protected_envelope_name_v2(publication_identity: [u8; 32]) -> String {
    format!(
        "{PROTECTED_ENVELOPE_PREFIX_V2}{}{PROTECTED_ENVELOPE_SUFFIX_V2}",
        hex(&publication_identity)
    )
}

fn protected_envelope_temp_package_prefix_v2(package: [u8; 32]) -> String {
    format!("{PROTECTED_ENVELOPE_TEMP_PREFIX_V2}{}-", hex(&package))
}

fn protected_envelope_temp_name_v2(
    package: [u8; 32],
    publication_identity: [u8; 32],
    process: u32,
    counter: u64,
) -> String {
    format!(
        "{}{}{PROTECTED_ENVELOPE_SUFFIX_V2}{TEMP_SUFFIX}{process}-{counter}",
        protected_envelope_temp_package_prefix_v2(package),
        hex(&publication_identity)
    )
}

fn is_protected_envelope_temp_name_v2(name: &str, package_prefix: &str) -> bool {
    let bytes = name.as_bytes();
    let identity_start = package_prefix.len();
    let identity_end = identity_start + 64;
    let suffix_end = identity_end + PROTECTED_ENVELOPE_SUFFIX_V2.len();
    let temp_end = suffix_end + TEMP_SUFFIX.len();
    bytes.len() > temp_end
        && bytes.starts_with(package_prefix.as_bytes())
        && bytes[identity_start..identity_end]
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        && &bytes[identity_end..suffix_end] == PROTECTED_ENVELOPE_SUFFIX_V2.as_bytes()
        && &bytes[suffix_end..temp_end] == TEMP_SUFFIX.as_bytes()
        && has_decimal_temp_counters(&bytes[temp_end..])
}

fn protected_cleanup_evidence_name_v1(package: [u8; 32]) -> String {
    format!(
        "{PROTECTED_CLEANUP_EVIDENCE_PREFIX_V1}{}{PROTECTED_CLEANUP_EVIDENCE_SUFFIX_V1}",
        hex(&package)
    )
}

fn protected_cleanup_evidence_temp_prefix_v1(package: [u8; 32]) -> String {
    format!(
        "{}{TEMP_SUFFIX}",
        protected_cleanup_evidence_name_v1(package)
    )
}

fn protected_cleanup_evidence_temp_name_v1(
    package: [u8; 32],
    process: u32,
    counter: u64,
) -> String {
    format!(
        "{}{process}-{counter}",
        protected_cleanup_evidence_temp_prefix_v1(package)
    )
}

fn is_protected_cleanup_evidence_temp_name_v1(name: &str, prefix: &str) -> bool {
    name.as_bytes().starts_with(prefix.as_bytes())
        && has_decimal_temp_counters(&name.as_bytes()[prefix.len()..])
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProtectedCleanupLocalNamesV1 {
    marker: String,
    inputs: String,
}

enum ProtectedCleanupLocalEntryV1 {
    Canonical(PinnedProtectedCleanupFileV1),
    Quarantined(PinnedProtectedCleanupFileV1),
    Absent,
}

impl ProtectedCleanupLocalNamesV1 {
    fn new(evidence: &ProtectedIntentCleanupEvidenceV1) -> Self {
        let identity = hash_identity(PROTECTED_CLEANUP_LOCAL_NAME_DOMAIN_V1, |digest| {
            digest.update(evidence.package);
            digest.update(evidence.attempt.generation().to_le_bytes());
            digest.update(evidence.attempt.session().as_bytes());
            digest.update(evidence.attempt.invocation().as_bytes());
            digest.update(evidence.intent.as_bytes());
            digest.update(evidence.escrow.identity().as_bytes());
        });
        let stem = format!("{PROTECTED_CLEANUP_LOCAL_PREFIX_V1}{}", hex(&identity));
        Self {
            marker: format!("{stem}{PROTECTED_CLEANUP_MARKER_QUARANTINE_SUFFIX_V1}"),
            inputs: format!("{stem}{PROTECTED_CLEANUP_INPUTS_QUARANTINE_SUFFIX_V1}"),
        }
    }
}

fn marker_temp_prefix(marker_name: &str) -> String {
    format!("{marker_name}{TEMP_SUFFIX}")
}

fn is_marker_temp_name(name: &str, prefix: &str) -> bool {
    name.as_bytes().starts_with(prefix.as_bytes())
        && has_decimal_temp_counters(&name.as_bytes()[prefix.len()..])
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

#[allow(dead_code)] // The protected caller is integrated separately from this marker boundary.
impl ResumeMarkerStateV2 {
    pub(crate) const fn attempt(self) -> BuildAttempt {
        match self {
            Self::Pending { attempt, .. }
            | Self::Ready { attempt, .. }
            | Self::Completed { attempt, .. } => attempt,
        }
    }

    pub(crate) const fn intent(self) -> Option<WorkerV2PublicationIntentIdentityV2> {
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

    const fn completed_receipt(self) -> Option<ReceiptRecordV2> {
        match self {
            Self::Pending { .. } | Self::Ready { .. } => None,
            Self::Completed { receipt, .. } => Some(receipt),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProtectedCleanupLocalFileSnapshotV1 {
    device: u64,
    inode: u64,
    byte_len: u64,
    mode: u64,
    links: u64,
}

impl ProtectedCleanupLocalFileSnapshotV1 {
    fn from_stat(stat: &rustix::fs::Stat) -> Result<Self, &'static str> {
        if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile
            || stat.st_nlink != 1
            || stat.st_mode & 0o077 != 0
        {
            return Err("cleanup local snapshot requires one private single-link regular file");
        }
        Ok(Self {
            device: stat.st_dev,
            inode: stat.st_ino,
            byte_len: u64::try_from(stat.st_size)
                .map_err(|_| "cleanup local snapshot size is invalid")?,
            mode: u64::from(stat.st_mode),
            links: stat.st_nlink,
        })
    }

    fn encode(self, bytes: &mut Vec<u8>) {
        for field in [
            self.device,
            self.inode,
            self.byte_len,
            self.mode,
            self.links,
        ] {
            bytes.extend_from_slice(&field.to_le_bytes());
        }
    }

    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, &'static str> {
        let snapshot = Self {
            device: decoder.u64()?,
            inode: decoder.u64()?,
            byte_len: decoder.u64()?,
            mode: decoder.u64()?,
            links: decoder.u64()?,
        };
        snapshot.validate()?;
        Ok(snapshot)
    }

    fn validate(self) -> Result<(), &'static str> {
        if self.device == 0
            || self.inode == 0
            || self.byte_len == 0
            || self.links != 1
            || self.mode & 0o077 != 0
            || FileType::from_raw_mode(self.mode as _) != FileType::RegularFile
        {
            return Err("cleanup local snapshot metadata is invalid");
        }
        Ok(())
    }

    fn matches(self, stat: &rustix::fs::Stat) -> bool {
        self.device == stat.st_dev
            && self.inode == stat.st_ino
            && u64::try_from(stat.st_size) == Ok(self.byte_len)
            && u64::from(stat.st_mode) == self.mode
            && stat.st_nlink == self.links
            && FileType::from_raw_mode(stat.st_mode) == FileType::RegularFile
            && stat.st_mode & 0o077 == 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProtectedIntentCleanupEvidenceV1 {
    package: [u8; 32],
    publication: WorkerV2PublicationKindV1,
    attempt: BuildAttempt,
    intent: WorkerV2PublicationIntentIdentityV2,
    envelope: [u8; 32],
    admission: [u8; 32],
    marker: [u8; 32],
    receipt: ReceiptRecordV2,
    envelope_inputs: [u8; 32],
    marker_file: ProtectedCleanupLocalFileSnapshotV1,
    inputs_file: ProtectedCleanupLocalFileSnapshotV1,
    escrow: WorkerV2PublicationIntentCleanupEscrowV1,
}

impl ProtectedIntentCleanupEvidenceV1 {
    fn new(
        package: [u8; 32],
        completed: ResumeMarkerStateV2,
        marker_file: ProtectedCleanupLocalFileSnapshotV1,
        inputs_file: ProtectedCleanupLocalFileSnapshotV1,
        escrow: WorkerV2PublicationIntentCleanupEscrowV1,
    ) -> Result<Self, &'static str> {
        let ResumeMarkerStateV2::Completed {
            publication,
            attempt,
            admission,
            envelope_inputs,
            envelope,
            intent,
            receipt,
        } = completed
        else {
            return Err("cleanup evidence requires a completed protected marker");
        };
        let evidence = Self {
            package,
            publication,
            attempt,
            intent,
            envelope,
            admission,
            marker: protected_cleanup_marker_commitment_v1(package, completed),
            receipt,
            envelope_inputs,
            marker_file,
            inputs_file,
            escrow,
        };
        evidence.validate()?;
        Ok(evidence)
    }

    fn compiler_closure(&self) -> CompilerClosureV2 {
        self.receipt
            .compiler_closure()
            .expect("validated cleanup evidence has a compiler closure")
    }

    fn validate(&self) -> Result<(), &'static str> {
        if !self.publication.requires_envelope() {
            return Err("cleanup evidence requires a protected envelope publication");
        }
        if self.intent.as_bytes() == [0; 32]
            || self.envelope == [0; 32]
            || self.admission == [0; 32]
            || self.marker == [0; 32]
            || self.envelope_inputs == [0; 32]
            || !self.receipt.is_complete()
        {
            return Err("cleanup evidence contains an empty required identity");
        }
        if self.escrow.package_identity() != self.package
            || self.escrow.attempt() != self.attempt
            || self.escrow.intent() != self.intent
            || !self.receipt.matches(self.escrow.receipt())
            || self.escrow.receipt().compiler_closure() != self.compiler_closure()
        {
            return Err("cleanup evidence differs from its artifact-owned escrow capsule");
        }
        self.marker_file.validate()?;
        self.inputs_file.validate()?;
        if protected_cleanup_marker_commitment_v1(self.package, self.completed_state())
            != self.marker
        {
            return Err("cleanup evidence completed marker commitment is invalid");
        }
        Ok(())
    }

    fn completed_state(&self) -> ResumeMarkerStateV2 {
        ResumeMarkerStateV2::Completed {
            publication: self.publication,
            attempt: self.attempt,
            admission: self.admission,
            envelope_inputs: self.envelope_inputs,
            envelope: self.envelope,
            intent: self.intent,
            receipt: self.receipt,
        }
    }

    fn matches_completed(&self, package: [u8; 32], completed: ResumeMarkerStateV2) -> bool {
        let ResumeMarkerStateV2::Completed {
            publication,
            attempt,
            admission,
            envelope_inputs,
            envelope,
            intent,
            receipt,
        } = completed
        else {
            return false;
        };
        self.package == package
            && self.publication == publication
            && self.attempt == attempt
            && self.admission == admission
            && self.envelope_inputs == envelope_inputs
            && self.envelope == envelope
            && self.intent == intent
            && self.receipt == receipt
            && self.marker == protected_cleanup_marker_commitment_v1(package, completed)
    }

    fn matches_recovered(&self, recovered: &RecoveredWorkerV2PublicationIntentV2) -> bool {
        let record = recovered.record();
        record.attempt() == self.attempt
            && record.producer_identity() == self.escrow.producer_identity()
            && record.identity() == self.intent
            && record.compiler_closure() == self.compiler_closure()
            && recovered.compiler_closure() == self.compiler_closure()
            && Sha256::digest(recovered.exact_output()).as_slice()
                == self.escrow.receipt().finalized_output_identity()
    }

    fn to_bytes(self) -> Vec<u8> {
        debug_assert!(self.validate().is_ok());
        let mut bytes = Vec::with_capacity(MAX_PROTECTED_CLEANUP_EVIDENCE_BYTES_V1);
        bytes.extend_from_slice(PROTECTED_CLEANUP_EVIDENCE_MAGIC_V1);
        bytes.extend_from_slice(&PROTECTED_CLEANUP_EVIDENCE_VERSION_V1.to_le_bytes());
        bytes.extend_from_slice(&self.package);
        bytes.push(self.publication.tag());
        bytes.extend_from_slice(&self.attempt.generation().to_le_bytes());
        bytes.extend_from_slice(self.attempt.session().as_bytes());
        bytes.extend_from_slice(self.attempt.invocation().as_bytes());
        bytes.extend_from_slice(&self.intent.as_bytes());
        bytes.extend_from_slice(&self.envelope);
        bytes.extend_from_slice(&self.admission);
        bytes.extend_from_slice(&self.marker);
        self.receipt.encode(&mut bytes);
        bytes.extend_from_slice(&self.envelope_inputs);
        self.marker_file.encode(&mut bytes);
        self.inputs_file.encode(&mut bytes);
        bytes.extend_from_slice(&self.escrow.to_bytes());
        debug_assert_eq!(bytes.len(), PROTECTED_CLEANUP_EVIDENCE_FIXED_BODY_BYTES_V1);
        let checksum = protected_cleanup_evidence_checksum_v1(&bytes);
        bytes.extend_from_slice(&checksum);
        bytes
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() != MAX_PROTECTED_CLEANUP_EVIDENCE_BYTES_V1 {
            return Err("cleanup evidence has a noncanonical fixed length");
        }
        let (body, checksum) = bytes.split_at(bytes.len() - 32);
        if protected_cleanup_evidence_checksum_v1(body) != checksum {
            return Err("cleanup evidence checksum mismatch");
        }
        let mut decoder = Decoder::new(body);
        if decoder.take(PROTECTED_CLEANUP_EVIDENCE_MAGIC_V1.len())?
            != PROTECTED_CLEANUP_EVIDENCE_MAGIC_V1
        {
            return Err("cleanup evidence magic mismatch");
        }
        if decoder.u16()? != PROTECTED_CLEANUP_EVIDENCE_VERSION_V1 {
            return Err("unsupported cleanup evidence version");
        }
        let package = decoder.array()?;
        let publication = WorkerV2PublicationKindV1::from_tag(decoder.byte()?)
            .ok_or("cleanup evidence publication tag is invalid")?;
        let generation = decoder.u64()?;
        let session = BuildSession::from_bytes(decoder.array()?);
        let invocation = fe2o3_artifact_transaction::BuildInvocation::from_bytes(decoder.array()?);
        let attempt = BuildAttempt::from_env_value(&format!(
            "{generation}:{}:{}",
            session.to_hex(),
            invocation.to_hex()
        ))
        .map_err(|_| "cleanup evidence contains an invalid attempt")?;
        let intent = WorkerV2PublicationIntentIdentityV2::from_bytes(decoder.array()?);
        let envelope = decoder.array()?;
        let admission = decoder.array()?;
        let marker = decoder.array()?;
        let receipt = ReceiptRecordV2::decode(&mut decoder)?;
        let envelope_inputs = decoder.array()?;
        let marker_file = ProtectedCleanupLocalFileSnapshotV1::decode(&mut decoder)?;
        let inputs_file = ProtectedCleanupLocalFileSnapshotV1::decode(&mut decoder)?;
        let escrow = WorkerV2PublicationIntentCleanupEscrowV1::from_bytes(
            decoder.take(MAX_WORKER_V2_PUBLICATION_INTENT_CLEANUP_ESCROW_CAPSULE_BYTES_V1)?,
        )
        .map_err(|_| "cleanup evidence contains an invalid artifact escrow capsule")?;
        if !decoder.finished() {
            return Err("cleanup evidence has trailing body bytes");
        }
        let evidence = Self {
            package,
            publication,
            attempt,
            intent,
            envelope,
            admission,
            marker,
            receipt,
            envelope_inputs,
            marker_file,
            inputs_file,
            escrow,
        };
        evidence.validate()?;
        if evidence.to_bytes() != bytes {
            return Err("cleanup evidence encoding is not canonical");
        }
        Ok(evidence)
    }
}

fn protected_cleanup_marker_commitment_v1(
    package: [u8; 32],
    completed: ResumeMarkerStateV2,
) -> [u8; 32] {
    checksum_for(
        PROTECTED_CLEANUP_EVIDENCE_MARKER_DOMAIN_V1,
        &encode_protected_marker(package, completed),
    )
}

fn protected_cleanup_evidence_checksum_v1(bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(PROTECTED_CLEANUP_EVIDENCE_CHECKSUM_DOMAIN_V1);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    digest.finalize().into()
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
        let store = Self::open_locked(output_dir, producer)?;
        let retained = store
            .load()?
            .map(|state| (state.publication(), state.attempt()));
        store.cleanup_envelope_input_residue(retained)?;
        store.cleanup_envelope_temp_residue()?;
        Ok(store)
    }

    fn open_locked(
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
        store.cleanup_marker_temp_residue()?;
        Ok(store)
    }

    fn cleanup_marker_temp_residue(&self) -> Result<(), ResumeMarkerErrorV1> {
        let prefix = marker_temp_prefix(&self.marker_name);
        let scan =
            rustix::io::fcntl_dupfd_cloexec(&self.directory, 0).map_err(std::io::Error::from)?;
        let mut directory = rustix::fs::Dir::read_from(&scan).map_err(std::io::Error::from)?;
        let mut entries = 0_usize;
        let mut residue = Vec::new();
        for entry in &mut directory {
            let entry = entry.map_err(std::io::Error::from)?;
            let bytes = entry.file_name().to_bytes();
            if !count_restart_artifact_entry(&mut entries, bytes)
                .map_err(|()| self.invalid("artifact directory exceeds its scan bound"))?
            {
                continue;
            }
            if !bytes.starts_with(prefix.as_bytes()) {
                continue;
            }
            let name = std::str::from_utf8(bytes)
                .map_err(|_| self.invalid("resume-marker temp residue name is not UTF-8"))?;
            if !is_marker_temp_name(name, &prefix) {
                return Err(
                    self.invalid_at_name(name, "malformed package-owned resume-marker temp name")
                );
            }
            if residue.len() == MAX_MARKER_TEMP_RESIDUE_ENTRIES {
                return Err(self.invalid("too many package-owned resume-marker temp residues"));
            }
            let descriptor = openat(
                &self.directory,
                name,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
                Mode::empty(),
            )
            .map_err(std::io::Error::from)?;
            validate_private_file(&self.directory, &descriptor, name, &self.display_path, None)?;
            let initial = fstat(&descriptor).map_err(std::io::Error::from)?;
            residue.push((
                name.to_owned(),
                PinnedProtectedCleanupFileV1 {
                    file: File::from(descriptor),
                    initial,
                },
            ));
        }
        if !residue.is_empty() {
            for (name, pinned) in residue {
                pinned.unlink_and_sync(
                    &self.directory,
                    &name,
                    &self.display_path,
                    "resume-marker-temp-before-unlink",
                    "resume-marker-temp-after-unlink",
                    "resume-marker-temp-directory-synced",
                )?;
            }
            self.verify_output_path()?;
        }
        Ok(())
    }

    fn cleanup_envelope_input_residue(
        &self,
        retained_state: Option<(WorkerV2PublicationKindV1, BuildAttempt)>,
    ) -> Result<(), ResumeMarkerErrorV1> {
        let retained = retained_state.and_then(|(publication, attempt)| {
            publication
                .requires_envelope()
                .then(|| envelope_inputs_name(self.package, attempt))
        });
        let package_prefix = envelope_inputs_package_prefix(self.package);
        let scan =
            rustix::io::fcntl_dupfd_cloexec(&self.directory, 0).map_err(std::io::Error::from)?;
        let mut directory = rustix::fs::Dir::read_from(&scan).map_err(std::io::Error::from)?;
        let mut entries = 0_usize;
        let mut residue = Vec::new();
        for entry in &mut directory {
            let entry = entry.map_err(std::io::Error::from)?;
            let bytes = entry.file_name().to_bytes();
            if !count_restart_artifact_entry(&mut entries, bytes)
                .map_err(|()| self.invalid("artifact directory exceeds its scan bound"))?
            {
                continue;
            }
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
            let initial = fstat(&descriptor).map_err(std::io::Error::from)?;
            residue.push((
                name.to_owned(),
                PinnedProtectedCleanupFileV1 {
                    file: File::from(descriptor),
                    initial,
                },
            ));
        }
        if !residue.is_empty() {
            for (name, pinned) in residue {
                pinned.unlink_and_sync(
                    &self.directory,
                    &name,
                    &self.display_path,
                    "envelope-input-residue-before-unlink",
                    "envelope-input-residue-after-unlink",
                    "envelope-input-residue-directory-synced",
                )?;
            }
            self.verify_output_path()?;
        }
        Ok(())
    }

    fn cleanup_envelope_temp_residue(&self) -> Result<(), ResumeMarkerErrorV1> {
        let package_prefix = envelope_temp_package_prefix(self.package);
        let scan =
            rustix::io::fcntl_dupfd_cloexec(&self.directory, 0).map_err(std::io::Error::from)?;
        let mut directory = rustix::fs::Dir::read_from(&scan).map_err(std::io::Error::from)?;
        let mut entries = 0_usize;
        let mut residue = Vec::new();
        for entry in &mut directory {
            let entry = entry.map_err(std::io::Error::from)?;
            let bytes = entry.file_name().to_bytes();
            if !count_restart_artifact_entry(&mut entries, bytes)
                .map_err(|()| self.invalid("artifact directory exceeds its scan bound"))?
            {
                continue;
            }
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
            let initial = fstat(&descriptor).map_err(std::io::Error::from)?;
            residue.push((
                name.to_owned(),
                PinnedProtectedCleanupFileV1 {
                    file: File::from(descriptor),
                    initial,
                },
            ));
        }
        if !residue.is_empty() {
            for (name, pinned) in residue {
                pinned.unlink_and_sync(
                    &self.directory,
                    &name,
                    &self.display_path,
                    "load-envelope-temp-before-unlink",
                    "load-envelope-temp-after-unlink",
                    "load-envelope-temp-directory-synced",
                )?;
            }
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
        self.read_marker_bytes()?
            .map(|bytes| decode_marker(&bytes, self.package))
            .transpose()
            .map_err(|reason| self.invalid(reason))
    }

    fn load_protected(&self) -> Result<Option<ResumeMarkerStateV2>, ResumeMarkerErrorV1> {
        self.read_marker_bytes()?
            .map(|bytes| decode_protected_marker(&bytes, self.package))
            .transpose()
            .map_err(|reason| self.invalid(reason))
    }

    fn read_marker_bytes(&self) -> Result<Option<Vec<u8>>, ResumeMarkerErrorV1> {
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
        let mut bytes = Vec::with_capacity(MAX_MARKER_BYTES + 1);
        Read::by_ref(&mut file)
            .take((MAX_MARKER_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
        let final_stat = fstat(&file).map_err(std::io::Error::from)?;
        let canonical_size = usize::try_from(final_stat.st_size).is_ok_and(|size| {
            size == PROTECTED_MARKER_BYTES
                || size == OBSOLETE_PROTECTED_MARKER_BYTES_V4
                || size == MARKER_BYTES
                || size == PREVIOUS_MARKER_BYTES
                || size == LEGACY_MARKER_BYTES
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
        Ok(Some(bytes))
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

    /// Refuses the V1 envelope placeholder even when the exact V2 receipt is durable.
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
        let envelope_identity = self.validate_and_publish_required_envelope(
            attempt,
            receipt,
            envelope,
            admission,
            marker_inputs,
            |plan, upstream, output, inputs| {
                restart_admission_commitment_with_inputs_v1(
                    publication,
                    plan,
                    upstream,
                    output,
                    Some(inputs),
                )
            },
        )?;
        self.persist_completed_inner(
            publication,
            attempt,
            intent,
            receipt,
            Some(envelope_identity),
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

    fn validate_and_publish_required_envelope(
        &self,
        attempt: BuildAttempt,
        receipt: BackendPublicationReceiptV1,
        envelope: &WorkerV2LoadEnvelopeV1,
        admission: [u8; 32],
        marker_inputs: [u8; 32],
        admission_commitment: impl FnOnce(
            DurableLinkPublicationPlanV1,
            UpstreamCodeObjectEvidenceIdentityV1,
            &[u8],
            WorkerV2EnvelopeInputsIdentityV1,
        ) -> [u8; 32],
    ) -> Result<WorkerV2LoadEnvelopeIdentityV1, ResumeMarkerErrorV1> {
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
            || admission_commitment(
                claim.plan(),
                claim.upstream_evidence(),
                envelope.finalized_payload(),
                envelope_inputs.identity(),
            ) != admission
            || envelope.grants_currentness_authority()
            || envelope.grants_load_authority()
            || envelope.grants_launch_authority()
        {
            return Err(self.invalid("required envelope disagrees with the ready publication"));
        }
        self.publish_load_envelope(envelope)?;
        Ok(envelope.identity())
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
        let canonical = state
            .into_canonical()
            .map_err(|()| ResumeMarkerErrorV1::InvalidTransition)?;
        let bytes = encode_marker(self.package, state);
        self.write_encoded::<OrdinaryMarkerSchemaV1>(canonical, bytes, replace)
    }

    fn write_protected(
        &self,
        state: ResumeMarkerStateV2,
        replace: bool,
    ) -> Result<(), ResumeMarkerErrorV1> {
        let bytes = encode_protected_marker(self.package, state);
        self.write_encoded::<ProtectedMarkerSchemaV2>(state.into(), bytes, replace)
    }

    fn write_encoded<S: CanonicalMarkerSchema>(
        &self,
        state: CanonicalMarkerState<S::Intent, S::Receipt>,
        bytes: Vec<u8>,
        replace: bool,
    ) -> Result<(), ResumeMarkerErrorV1> {
        self.verify_output_path()?;
        debug_assert_eq!(bytes.len(), S::ENCODED_BYTES);
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
                Some(S::ENCODED_BYTES),
            )?;
            #[cfg(feature = "worker-v2-fault-injection-test-only")]
            injected_fault_point_v1("resume-marker-temp-synced");
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
            #[cfg(feature = "worker-v2-fault-injection-test-only")]
            injected_fault_point_v1("resume-marker-renamed");
            fsync(&self.directory).map_err(std::io::Error::from)?;
            self.verify_output_path()?;
            let published = self
                .read_marker_bytes()?
                .ok_or_else(|| self.invalid("marker disappeared after atomic publication"))?;
            let decoded = decode_canonical_marker::<S>(&published, self.package)
                .map_err(|reason| self.invalid(reason))?;
            if decoded != state {
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

/// Protected marker owner. Its decoder never accepts ordinary V1/V2/V3 marker records.
#[allow(dead_code)] // The protected caller is integrated separately from this marker boundary.
pub(crate) struct WorkerV2ResumeStoreV2 {
    inner: WorkerV2ResumeStoreV1,
    producer: ProducerIdentity,
}

#[allow(dead_code)] // The protected caller is integrated separately from this marker boundary.
impl WorkerV2ResumeStoreV2 {
    pub(crate) fn open(
        output_dir: &Path,
        producer: &ProducerIdentity,
    ) -> Result<Self, ResumeMarkerErrorV1> {
        let inner = WorkerV2ResumeStoreV1::open_locked(output_dir, producer)?;
        let store = Self {
            inner,
            producer: producer.clone(),
        };
        store.cleanup_protected_envelope_temp_residue()?;
        store.cleanup_protected_cleanup_evidence_temp_residue()?;
        store.reconcile_protected_cleanup_evidence_on_open()?;
        let retained = store
            .load()?
            .map(|state| (state.publication(), state.attempt()));
        store.inner.cleanup_envelope_input_residue(retained)?;
        Ok(store)
    }

    pub(crate) fn verify_output_path(&self) -> Result<(), ResumeMarkerErrorV1> {
        self.inner.verify_output_path()
    }

    pub(crate) fn load(&self) -> Result<Option<ResumeMarkerStateV2>, ResumeMarkerErrorV1> {
        self.inner.load_protected()
    }

    pub(crate) fn persist_envelope_inputs(
        &self,
        attempt: BuildAttempt,
        inputs: &WorkerV2EnvelopeInputsV1,
    ) -> Result<(), ResumeMarkerErrorV1> {
        self.inner.persist_envelope_inputs(attempt, inputs)
    }

    pub(crate) fn recover_envelope_inputs(
        &self,
        attempt: BuildAttempt,
    ) -> Result<WorkerV2EnvelopeInputsV1, ResumeMarkerErrorV1> {
        self.inner.recover_envelope_inputs(attempt)
    }

    fn recover_complete_intent(
        &self,
        attempt: BuildAttempt,
        expected_compiler_closure: CompilerClosureV2,
    ) -> Result<RecoveredWorkerV2PublicationIntentV2, ResumeMarkerErrorV1> {
        self.verify_output_path()?;
        let recovered = recover_worker_v2_publication_intent_v2(
            &self.inner.display_path,
            &self.producer,
            attempt,
            expected_compiler_closure,
        )
        .map_err(|error| {
            self.inner.invalid(format!(
                "the complete protected publication intent cannot be recovered: {error}"
            ))
        })?;
        self.verify_output_path()?;
        self.validate_complete_intent_snapshot(&recovered, attempt, expected_compiler_closure)?;
        Ok(recovered)
    }

    fn validate_complete_intent_snapshot(
        &self,
        recovered: &RecoveredWorkerV2PublicationIntentV2,
        attempt: BuildAttempt,
        expected_compiler_closure: CompilerClosureV2,
    ) -> Result<(), ResumeMarkerErrorV1> {
        let record = recovered.record();
        let output_identity: [u8; 32] = Sha256::digest(recovered.exact_output()).into();
        if record.attempt() != attempt
            || record.plan().attempt() != attempt
            || record.compiler_closure() != expected_compiler_closure
            || recovered.compiler_closure() != expected_compiler_closure
            || record.output_length() != recovered.exact_output().len()
            || record.output_identity().as_bytes() != &output_identity
            || record.plan().finalized_output() != record.output_identity()
            || record.identity().as_bytes() == [0; 32]
        {
            return Err(self
                .inner
                .invalid("the complete protected publication intent changed after recovery"));
        }
        Ok(())
    }

    fn recover_complete_claim(
        &self,
        claim: &DurablePublishedHsacoClaimV2,
        receipt: BackendPublicationReceiptV2,
        expected_compiler_closure: CompilerClosureV2,
    ) -> Result<DurablePublishedHsacoClaimV2, ResumeMarkerErrorV1> {
        self.verify_output_path()?;
        let recovered = recover_published_hsaco_claim_for_attempt_v2(
            &self.inner.display_path,
            &self.producer,
            claim.plan().attempt(),
            claim.plan(),
            claim.upstream_evidence(),
            expected_compiler_closure,
            receipt,
        )
        .map_err(|error| {
            self.inner.invalid(format!(
                "the complete protected publication claim cannot be recovered: {error}"
            ))
        })?;
        self.verify_output_path()?;
        if recovered != *claim {
            return Err(self
                .inner
                .invalid("protected envelope claim differs from durable publication state"));
        }
        Ok(recovered)
    }

    fn recover_claim_for_intent(
        &self,
        recovered: &RecoveredWorkerV2PublicationIntentV2,
        receipt: BackendPublicationReceiptV2,
        expected_compiler_closure: CompilerClosureV2,
    ) -> Result<DurablePublishedHsacoClaimV2, ResumeMarkerErrorV1> {
        let record = recovered.record();
        self.validate_complete_intent_snapshot(
            recovered,
            record.attempt(),
            expected_compiler_closure,
        )?;
        self.verify_output_path()?;
        let claim = recover_published_hsaco_claim_for_attempt_v2(
            &self.inner.display_path,
            &self.producer,
            record.attempt(),
            record.plan(),
            record.upstream_evidence(),
            expected_compiler_closure,
            receipt,
        )
        .map_err(|error| {
            self.inner.invalid(format!(
                "the protected intent's complete publication claim cannot be recovered: {error}"
            ))
        })?;
        self.verify_output_path()?;
        Ok(claim)
    }

    fn cleanup_protected_envelope_temp_residue(&self) -> Result<(), ResumeMarkerErrorV1> {
        let package_prefix = protected_envelope_temp_package_prefix_v2(self.inner.package);
        let scan = rustix::io::fcntl_dupfd_cloexec(&self.inner.directory, 0)
            .map_err(std::io::Error::from)?;
        let mut directory = rustix::fs::Dir::read_from(&scan).map_err(std::io::Error::from)?;
        let mut entries = 0_usize;
        let mut residue = Vec::new();
        for entry in &mut directory {
            let entry = entry.map_err(std::io::Error::from)?;
            let bytes = entry.file_name().to_bytes();
            if !count_restart_artifact_entry(&mut entries, bytes).map_err(|()| {
                self.inner
                    .invalid("artifact directory exceeds its scan bound")
            })? {
                continue;
            }
            if !bytes.starts_with(package_prefix.as_bytes()) {
                continue;
            }
            let name = std::str::from_utf8(bytes).map_err(|_| {
                self.inner
                    .invalid("protected load-envelope temp residue name is not UTF-8")
            })?;
            if !is_protected_envelope_temp_name_v2(name, &package_prefix) {
                return Err(self.inner.invalid_at_name(
                    name,
                    "malformed package-owned protected load-envelope temp name",
                ));
            }
            if residue.len() == MAX_ENVELOPE_TEMP_RESIDUE_ENTRIES {
                return Err(self
                    .inner
                    .invalid("too many package-owned protected load-envelope temp residues"));
            }
            let descriptor = openat(
                &self.inner.directory,
                name,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
                Mode::empty(),
            )
            .map_err(std::io::Error::from)?;
            validate_private_file(
                &self.inner.directory,
                &descriptor,
                name,
                &self.inner.display_path,
                None,
            )?;
            let initial = fstat(&descriptor).map_err(std::io::Error::from)?;
            residue.push((
                name.to_owned(),
                PinnedProtectedCleanupFileV1 {
                    file: File::from(descriptor),
                    initial,
                },
            ));
        }
        if !residue.is_empty() {
            for (name, pinned) in residue {
                pinned.unlink_and_sync(
                    &self.inner.directory,
                    &name,
                    &self.inner.display_path,
                    "protected-envelope-temp-before-unlink",
                    "protected-envelope-temp-after-unlink",
                    "protected-envelope-temp-directory-synced",
                )?;
            }
            self.verify_output_path()?;
        }
        Ok(())
    }

    fn cleanup_protected_cleanup_evidence_temp_residue(&self) -> Result<(), ResumeMarkerErrorV1> {
        let prefix = protected_cleanup_evidence_temp_prefix_v1(self.inner.package);
        let scan = rustix::io::fcntl_dupfd_cloexec(&self.inner.directory, 0)
            .map_err(std::io::Error::from)?;
        let mut directory = rustix::fs::Dir::read_from(&scan).map_err(std::io::Error::from)?;
        let mut entries = 0_usize;
        let mut residue = Vec::new();
        for entry in &mut directory {
            let entry = entry.map_err(std::io::Error::from)?;
            let bytes = entry.file_name().to_bytes();
            if !count_restart_artifact_entry(&mut entries, bytes).map_err(|()| {
                self.inner
                    .invalid("artifact directory exceeds its scan bound")
            })? {
                continue;
            }
            if !bytes.starts_with(prefix.as_bytes()) {
                continue;
            }
            let name = std::str::from_utf8(bytes).map_err(|_| {
                self.inner
                    .invalid("protected cleanup-evidence temp name is not UTF-8")
            })?;
            if !is_protected_cleanup_evidence_temp_name_v1(name, &prefix) {
                return Err(self.inner.invalid_at_name(
                    name,
                    "malformed package-owned protected cleanup-evidence temp name",
                ));
            }
            if residue.len() == MAX_PROTECTED_CLEANUP_TEMP_RESIDUE_ENTRIES_V1 {
                return Err(self
                    .inner
                    .invalid("too many package-owned protected cleanup-evidence temp residues"));
            }
            let descriptor = openat(
                &self.inner.directory,
                name,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
                Mode::empty(),
            )
            .map_err(std::io::Error::from)?;
            validate_private_file(
                &self.inner.directory,
                &descriptor,
                name,
                &self.inner.display_path,
                None,
            )?;
            let initial = fstat(&descriptor).map_err(std::io::Error::from)?;
            residue.push((
                name.to_owned(),
                PinnedProtectedCleanupFileV1 {
                    file: File::from(descriptor),
                    initial,
                },
            ));
        }
        if !residue.is_empty() {
            for (name, pinned) in residue {
                pinned.unlink_and_sync(
                    &self.inner.directory,
                    &name,
                    &self.inner.display_path,
                    "protected-cleanup-evidence-temp-before-unlink",
                    "protected-cleanup-evidence-temp-after-unlink",
                    "protected-cleanup-evidence-temp-directory-synced",
                )?;
            }
            fsync(&self.inner.directory).map_err(std::io::Error::from)?;
            self.verify_output_path()?;
        }
        Ok(())
    }

    fn persist_protected_cleanup_evidence(
        &self,
        evidence: &ProtectedIntentCleanupEvidenceV1,
    ) -> Result<(), ResumeMarkerErrorV1> {
        evidence
            .validate()
            .map_err(|reason| self.inner.invalid(reason))?;
        if evidence.package != self.inner.package {
            return Err(self
                .inner
                .invalid("protected cleanup evidence names a different package"));
        }
        self.verify_output_path()?;
        let bytes = evidence.to_bytes();
        if bytes.len() != MAX_PROTECTED_CLEANUP_EVIDENCE_BYTES_V1 {
            return Err(self
                .inner
                .invalid("protected cleanup evidence exceeds its canonical bound"));
        }
        let name = protected_cleanup_evidence_name_v1(self.inner.package);
        if let Some(existing) = self.read_protected_cleanup_evidence()? {
            if existing == *evidence {
                fsync(&self.inner.directory).map_err(std::io::Error::from)?;
                self.verify_output_path()?;
                return Ok(());
            }
            return Err(self
                .inner
                .invalid_at_name(&name, "conflicting protected cleanup evidence"));
        }
        let temp_name = protected_cleanup_evidence_temp_name_v1(
            self.inner.package,
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed),
        );
        let result = (|| {
            #[cfg(feature = "worker-v2-fault-injection-test-only")]
            injected_fault_point_v1("protected-cleanup-evidence-before-temp-create");
            let descriptor = openat(
                &self.inner.directory,
                &temp_name,
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::RUSR | Mode::WUSR,
            )
            .map_err(std::io::Error::from)?;
            #[cfg(feature = "worker-v2-fault-injection-test-only")]
            injected_fault_point_v1("protected-cleanup-evidence-after-temp-create");
            let mut file = File::from(descriptor);
            file.set_len(bytes.len() as u64)?;
            #[cfg(feature = "worker-v2-fault-injection-test-only")]
            injected_fault_point_v1("protected-cleanup-evidence-before-temp-write");
            file.write_all(&bytes)?;
            #[cfg(feature = "worker-v2-fault-injection-test-only")]
            injected_fault_point_v1("protected-cleanup-evidence-after-temp-write");
            #[cfg(feature = "worker-v2-fault-injection-test-only")]
            injected_fault_point_v1("protected-cleanup-evidence-before-temp-sync");
            file.sync_all()?;
            #[cfg(feature = "worker-v2-fault-injection-test-only")]
            injected_fault_point_v1("protected-cleanup-evidence-after-temp-sync");
            validate_private_file(
                &self.inner.directory,
                &file,
                &temp_name,
                &self.inner.display_path,
                Some(bytes.len()),
            )?;
            self.verify_output_path()?;
            #[cfg(feature = "worker-v2-fault-injection-test-only")]
            injected_fault_point_v1("protected-cleanup-evidence-before-rename");
            match renameat_with(
                &self.inner.directory,
                &temp_name,
                &self.inner.directory,
                &name,
                RenameFlags::NOREPLACE,
            ) {
                Ok(()) => {}
                Err(rustix::io::Errno::EXIST) => {
                    unlinkat(&self.inner.directory, &temp_name, AtFlags::empty())
                        .map_err(std::io::Error::from)?;
                    let existing = self.read_protected_cleanup_evidence()?.ok_or_else(|| {
                        self.inner.invalid_at_name(
                            &name,
                            "protected cleanup evidence disappeared after create-new conflict",
                        )
                    })?;
                    return if existing == *evidence {
                        fsync(&self.inner.directory).map_err(std::io::Error::from)?;
                        Ok(())
                    } else {
                        Err(self
                            .inner
                            .invalid_at_name(&name, "conflicting protected cleanup evidence"))
                    };
                }
                Err(error) => return Err(std::io::Error::from(error).into()),
            }
            #[cfg(feature = "worker-v2-fault-injection-test-only")]
            injected_fault_point_v1("protected-cleanup-evidence-after-rename");
            #[cfg(feature = "worker-v2-fault-injection-test-only")]
            injected_fault_point_v1("protected-cleanup-evidence-before-directory-sync");
            fsync(&self.inner.directory).map_err(std::io::Error::from)?;
            #[cfg(feature = "worker-v2-fault-injection-test-only")]
            injected_fault_point_v1("protected-cleanup-evidence-after-directory-sync");
            self.verify_output_path()?;
            #[cfg(feature = "worker-v2-fault-injection-test-only")]
            injected_fault_point_v1("protected-cleanup-evidence-published");
            let published = self.read_protected_cleanup_evidence()?.ok_or_else(|| {
                self.inner.invalid_at_name(
                    &name,
                    "protected cleanup evidence is absent after durable publication",
                )
            })?;
            if published != *evidence {
                return Err(self.inner.invalid_at_name(
                    &name,
                    "protected cleanup evidence changed after publication",
                ));
            }
            Ok(())
        })();
        if result.is_err() {
            let _ = unlinkat(&self.inner.directory, &temp_name, AtFlags::empty());
        }
        result
    }

    fn read_protected_cleanup_evidence(
        &self,
    ) -> Result<Option<ProtectedIntentCleanupEvidenceV1>, ResumeMarkerErrorV1> {
        self.verify_output_path()?;
        let name = protected_cleanup_evidence_name_v1(self.inner.package);
        let descriptor = match openat(
            &self.inner.directory,
            &name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::empty(),
        ) {
            Ok(descriptor) => descriptor,
            Err(rustix::io::Errno::NOENT) => return Ok(None),
            Err(error) => return Err(std::io::Error::from(error).into()),
        };
        validate_private_file(
            &self.inner.directory,
            &descriptor,
            &name,
            &self.inner.display_path,
            None,
        )?;
        let initial = fstat(&descriptor).map_err(std::io::Error::from)?;
        let initial_size = usize::try_from(initial.st_size).ok();
        if initial_size != Some(MAX_PROTECTED_CLEANUP_EVIDENCE_BYTES_V1) {
            return Err(self
                .inner
                .invalid_at_name(&name, "protected cleanup evidence size is invalid"));
        }
        let mut file = File::from(descriptor);
        let mut bytes = Vec::with_capacity(initial_size.unwrap_or(0).saturating_add(1));
        Read::by_ref(&mut file)
            .take((MAX_PROTECTED_CLEANUP_EVIDENCE_BYTES_V1 + 1) as u64)
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
            || bytes.len() != MAX_PROTECTED_CLEANUP_EVIDENCE_BYTES_V1
        {
            return Err(self.inner.invalid_at_name(
                &name,
                "protected cleanup evidence changed while it was read",
            ));
        }
        let evidence = ProtectedIntentCleanupEvidenceV1::from_bytes(&bytes)
            .map_err(|reason| self.inner.invalid_at_name(&name, reason))?;
        if evidence.package != self.inner.package {
            return Err(self
                .inner
                .invalid_at_name(&name, "protected cleanup evidence package was substituted"));
        }
        Ok(Some(evidence))
    }

    fn validate_protected_cleanup_evidence(
        &self,
        evidence: &ProtectedIntentCleanupEvidenceV1,
        completed: ResumeMarkerStateV2,
    ) -> Result<BackendPublicationReceiptV2, ResumeMarkerErrorV1> {
        if !evidence.matches_completed(self.inner.package, completed) {
            return Err(self
                .inner
                .invalid("protected cleanup evidence differs from its completed marker"));
        }
        evidence
            .validate()
            .map_err(|reason| self.inner.invalid(reason))?;
        let receipt = self.durable_receipt(completed.attempt())?;
        if !evidence.receipt.matches(receipt)
            || receipt.compiler_closure() != evidence.compiler_closure()
        {
            return Err(self.inner.invalid(
                "protected cleanup evidence differs from the durable V2 publication receipt",
            ));
        }
        Ok(receipt)
    }

    fn ensure_protected_cleanup_escrow_prepared(
        &self,
        evidence: &ProtectedIntentCleanupEvidenceV1,
    ) -> Result<(), ResumeMarkerErrorV1> {
        let escrow = prepare_worker_v2_publication_intent_cleanup_escrow_v1(
            &self.inner.display_path,
            &self.producer,
            evidence.attempt,
            evidence.compiler_closure(),
            evidence.intent,
            evidence.escrow.receipt(),
        )
        .map_err(|error| {
            self.inner.invalid(format!(
                "artifact-owned protected cleanup escrow retry failed: {error}"
            ))
        })?;
        if escrow != evidence.escrow {
            return Err(self
                .inner
                .invalid("protected cleanup evidence differs from its retried artifact escrow"));
        }
        Ok(())
    }

    fn validate_protected_cleanup_envelope(
        &self,
        evidence: &ProtectedIntentCleanupEvidenceV1,
    ) -> Result<BackendPublicationReceiptV2, ResumeMarkerErrorV1> {
        recover_worker_v2_publication_intent_cleanup_escrow_v1(
            &self.inner.display_path,
            &self.producer,
            evidence.escrow,
        )
        .map_err(|error| {
            self.inner.invalid(format!(
                "protected cleanup envelope escrow recovery failed: {error}"
            ))
        })?;
        self.validate_protected_cleanup_envelope_facts(evidence)
    }

    fn validate_protected_cleanup_envelope_facts(
        &self,
        evidence: &ProtectedIntentCleanupEvidenceV1,
    ) -> Result<BackendPublicationReceiptV2, ResumeMarkerErrorV1> {
        evidence
            .validate()
            .map_err(|reason| self.inner.invalid(reason))?;
        if evidence.package != self.inner.package {
            return Err(self
                .inner
                .invalid("protected cleanup evidence names a different package"));
        }
        let receipt = evidence.escrow.receipt();
        if !evidence.receipt.matches(receipt)
            || receipt.compiler_closure() != evidence.compiler_closure()
        {
            return Err(self.inner.invalid(
                "protected cleanup evidence differs from the durable V2 publication receipt",
            ));
        }
        let name = protected_envelope_name_v2(receipt.publication_identity());
        let envelope = self.read_cleanup_load_envelope(&name)?.ok_or_else(|| {
            self.inner.invalid_at_name(
                &name,
                "canonical protected Worker V2 cleanup envelope is missing",
            )
        })?;
        let artifact = envelope.final_artifact_evidence();
        let claim = artifact.published_claim();
        let intent = artifact.publication_intent_transcript();
        let carried_inputs = WorkerV2EnvelopeInputsV1::new(
            envelope.direct_link_evidence().clone(),
            envelope.proof_records().to_vec(),
            envelope.raw_hsaco().clone(),
        )
        .map_err(|error| {
            self.inner.invalid(format!(
                "protected cleanup envelope inputs are invalid: {error}"
            ))
        })?;
        let finalized_payload_identity: [u8; 32] =
            Sha256::digest(envelope.finalized_payload()).into();
        if envelope.identity().as_bytes() != evidence.envelope
            || claim.plan().scope().package().as_bytes() != &evidence.package
            || claim.plan() != intent.plan()
            || claim.upstream_evidence() != intent.upstream_evidence()
            || claim.receipt() != receipt
            || claim.compiler_closure() != evidence.compiler_closure()
            || intent.source_record_identity() != evidence.intent
            || intent.attempt() != evidence.attempt
            || intent.producer_identity() != evidence.escrow.producer_identity()
            || intent.output_identity().as_bytes() != &receipt.finalized_output_identity()
            || usize::try_from(intent.output_length()) != Ok(envelope.finalized_payload().len())
            || intent.compiler_closure() != evidence.compiler_closure()
            || finalized_payload_identity != receipt.finalized_output_identity()
            || carried_inputs.identity().as_bytes() != evidence.envelope_inputs
            || restart_admission_commitment_with_inputs_v2(
                evidence.publication,
                intent.plan(),
                intent.upstream_evidence(),
                envelope.finalized_payload(),
                Some(carried_inputs.identity()),
                evidence.compiler_closure(),
            ) != evidence.admission
        {
            return Err(self
                .inner
                .invalid("protected cleanup envelope differs from its durable recovery evidence"));
        }
        Ok(receipt)
    }

    fn read_cleanup_load_envelope(
        &self,
        name: &str,
    ) -> Result<Option<WorkerV2LoadEnvelopeV2>, ResumeMarkerErrorV1> {
        self.verify_output_path()?;
        let descriptor = match openat(
            &self.inner.directory,
            name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::empty(),
        ) {
            Ok(descriptor) => descriptor,
            Err(rustix::io::Errno::NOENT) => return Ok(None),
            Err(error) => return Err(std::io::Error::from(error).into()),
        };
        validate_private_file(
            &self.inner.directory,
            &descriptor,
            name,
            &self.inner.display_path,
            None,
        )?;
        let initial = fstat(&descriptor).map_err(std::io::Error::from)?;
        let initial_size = usize::try_from(initial.st_size).ok();
        if initial_size.is_none_or(|size| size == 0 || size > MAX_WORKER_V2_LOAD_ENVELOPE_BYTES_V2)
        {
            return Err(self.inner.invalid_at_name(
                name,
                "protected cleanup envelope exceeds its canonical V2 bound",
            ));
        }
        let mut file = File::from(descriptor);
        let mut bytes = Vec::with_capacity(initial_size.unwrap_or(0).saturating_add(1));
        Read::by_ref(&mut file)
            .take((MAX_WORKER_V2_LOAD_ENVELOPE_BYTES_V2 + 1) as u64)
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
            || bytes.len() > MAX_WORKER_V2_LOAD_ENVELOPE_BYTES_V2
        {
            return Err(self
                .inner
                .invalid_at_name(name, "protected cleanup envelope changed while it was read"));
        }
        let envelope = WorkerV2LoadEnvelopeV2::from_bytes(&bytes).map_err(|error| {
            self.inner.invalid_at_name(
                name,
                format!("invalid protected cleanup V2 envelope: {error}"),
            )
        })?;
        if envelope.to_bytes() != bytes {
            return Err(self.inner.invalid_at_name(
                name,
                "protected cleanup V2 envelope encoding is not canonical",
            ));
        }
        Ok(Some(envelope))
    }

    fn protected_cleanup_marker_entry(
        &self,
        evidence: &ProtectedIntentCleanupEvidenceV1,
        names: &ProtectedCleanupLocalNamesV1,
    ) -> Result<ProtectedCleanupLocalEntryV1, ResumeMarkerErrorV1> {
        let canonical = PinnedProtectedCleanupFileV1::open(
            &self.inner.directory,
            &self.inner.marker_name,
            &self.inner.display_path,
        )?;
        let quarantined = PinnedProtectedCleanupFileV1::open(
            &self.inner.directory,
            &names.marker,
            &self.inner.display_path,
        )?;
        match (canonical, quarantined) {
            (Some(_), Some(_)) => Err(self
                .inner
                .invalid("protected cleanup marker exists in canonical and quarantine namespaces")),
            (Some(pinned), None) => {
                pinned.require_snapshot(
                    evidence.marker_file,
                    &self.inner.marker_name,
                    &self.inner.display_path,
                )?;
                self.validate_protected_cleanup_marker_bytes(
                    &pinned,
                    &self.inner.marker_name,
                    evidence,
                )?;
                Ok(ProtectedCleanupLocalEntryV1::Canonical(pinned))
            }
            (None, Some(pinned)) => {
                pinned.require_snapshot(
                    evidence.marker_file,
                    &names.marker,
                    &self.inner.display_path,
                )?;
                self.validate_protected_cleanup_marker_bytes(&pinned, &names.marker, evidence)?;
                Ok(ProtectedCleanupLocalEntryV1::Quarantined(pinned))
            }
            (None, None) => Ok(ProtectedCleanupLocalEntryV1::Absent),
        }
    }

    fn validate_protected_cleanup_marker_bytes(
        &self,
        pinned: &PinnedProtectedCleanupFileV1,
        name: &str,
        evidence: &ProtectedIntentCleanupEvidenceV1,
    ) -> Result<(), ResumeMarkerErrorV1> {
        let bytes = pinned.read_bounded(
            &self.inner.directory,
            name,
            &self.inner.display_path,
            PROTECTED_MARKER_BYTES,
        )?;
        if bytes != encode_protected_marker(self.inner.package, evidence.completed_state()) {
            return Err(self
                .inner
                .invalid_at_name(name, "protected cleanup marker bytes were replaced"));
        }
        Ok(())
    }

    fn protected_cleanup_inputs_entry(
        &self,
        evidence: &ProtectedIntentCleanupEvidenceV1,
        names: &ProtectedCleanupLocalNamesV1,
    ) -> Result<ProtectedCleanupLocalEntryV1, ResumeMarkerErrorV1> {
        let canonical_name = envelope_inputs_name(self.inner.package, evidence.attempt);
        let canonical = PinnedProtectedCleanupFileV1::open(
            &self.inner.directory,
            &canonical_name,
            &self.inner.display_path,
        )?;
        let quarantined = PinnedProtectedCleanupFileV1::open(
            &self.inner.directory,
            &names.inputs,
            &self.inner.display_path,
        )?;
        match (canonical, quarantined) {
            (Some(_), Some(_)) => Err(self
                .inner
                .invalid("protected cleanup inputs exist in canonical and quarantine namespaces")),
            (Some(pinned), None) => {
                pinned.require_snapshot(
                    evidence.inputs_file,
                    &canonical_name,
                    &self.inner.display_path,
                )?;
                self.validate_protected_cleanup_inputs_bytes(&pinned, &canonical_name, evidence)?;
                Ok(ProtectedCleanupLocalEntryV1::Canonical(pinned))
            }
            (None, Some(pinned)) => {
                pinned.require_snapshot(
                    evidence.inputs_file,
                    &names.inputs,
                    &self.inner.display_path,
                )?;
                self.validate_protected_cleanup_inputs_bytes(&pinned, &names.inputs, evidence)?;
                Ok(ProtectedCleanupLocalEntryV1::Quarantined(pinned))
            }
            (None, None) => Ok(ProtectedCleanupLocalEntryV1::Absent),
        }
    }

    fn validate_protected_cleanup_inputs_bytes(
        &self,
        pinned: &PinnedProtectedCleanupFileV1,
        name: &str,
        evidence: &ProtectedIntentCleanupEvidenceV1,
    ) -> Result<(), ResumeMarkerErrorV1> {
        let bytes = pinned.read_bounded(
            &self.inner.directory,
            name,
            &self.inner.display_path,
            MAX_WORKER_V2_ENVELOPE_INPUTS_BYTES,
        )?;
        let decoded = WorkerV2EnvelopeInputsV1::from_bytes(&bytes).map_err(|error| {
            self.inner.invalid_at_name(
                name,
                format!("protected cleanup input capsule is invalid: {error}"),
            )
        })?;
        if decoded.identity().as_bytes() != evidence.envelope_inputs || decoded.to_bytes() != bytes
        {
            return Err(self
                .inner
                .invalid_at_name(name, "protected cleanup input capsule was replaced"));
        }
        Ok(())
    }

    fn prepare_protected_cleanup_local_quarantine(
        &self,
        evidence: &ProtectedIntentCleanupEvidenceV1,
    ) -> Result<(), ResumeMarkerErrorV1> {
        let names = ProtectedCleanupLocalNamesV1::new(evidence);
        let marker = self.protected_cleanup_marker_entry(evidence, &names)?;
        let inputs = self.protected_cleanup_inputs_entry(evidence, &names)?;
        match (marker, inputs) {
            (
                ProtectedCleanupLocalEntryV1::Canonical(marker),
                ProtectedCleanupLocalEntryV1::Canonical(inputs),
            ) => {
                marker.rename_and_sync(
                    &self.inner.directory,
                    &self.inner.marker_name,
                    &names.marker,
                    &self.inner.display_path,
                    ProtectedCleanupRenameFaultsV1 {
                        _before_rename: "protected-cleanup-marker-before-quarantine-rename",
                        _after_rename: "protected-cleanup-marker-after-quarantine-rename",
                        _after_sync: "protected-cleanup-marker-quarantine-directory-synced",
                    },
                )?;
                self.validate_protected_cleanup_marker_bytes(&marker, &names.marker, evidence)?;
                inputs.rename_and_sync(
                    &self.inner.directory,
                    &envelope_inputs_name(self.inner.package, evidence.attempt),
                    &names.inputs,
                    &self.inner.display_path,
                    ProtectedCleanupRenameFaultsV1 {
                        _before_rename: "protected-cleanup-input-before-quarantine-rename",
                        _after_rename: "protected-cleanup-input-after-quarantine-rename",
                        _after_sync: "protected-cleanup-input-quarantine-directory-synced",
                    },
                )?;
                self.validate_protected_cleanup_inputs_bytes(&inputs, &names.inputs, evidence)?;
            }
            (
                ProtectedCleanupLocalEntryV1::Quarantined(marker),
                ProtectedCleanupLocalEntryV1::Canonical(inputs),
            ) => {
                fsync(&self.inner.directory).map_err(std::io::Error::from)?;
                self.validate_protected_cleanup_marker_bytes(&marker, &names.marker, evidence)?;
                inputs.rename_and_sync(
                    &self.inner.directory,
                    &envelope_inputs_name(self.inner.package, evidence.attempt),
                    &names.inputs,
                    &self.inner.display_path,
                    ProtectedCleanupRenameFaultsV1 {
                        _before_rename: "protected-cleanup-input-before-quarantine-rename",
                        _after_rename: "protected-cleanup-input-after-quarantine-rename",
                        _after_sync: "protected-cleanup-input-quarantine-directory-synced",
                    },
                )?;
                self.validate_protected_cleanup_inputs_bytes(&inputs, &names.inputs, evidence)?;
            }
            (
                ProtectedCleanupLocalEntryV1::Quarantined(marker),
                ProtectedCleanupLocalEntryV1::Quarantined(inputs),
            ) => {
                fsync(&self.inner.directory).map_err(std::io::Error::from)?;
                self.validate_protected_cleanup_marker_bytes(&marker, &names.marker, evidence)?;
                self.validate_protected_cleanup_inputs_bytes(&inputs, &names.inputs, evidence)?;
            }
            _ => {
                return Err(self.inner.invalid(
                    "protected cleanup marker/input quarantine is in an impossible mixed state",
                ));
            }
        }
        fsync(&self.inner.directory).map_err(std::io::Error::from)?;
        let marker = self.protected_cleanup_marker_entry(evidence, &names)?;
        let inputs = self.protected_cleanup_inputs_entry(evidence, &names)?;
        if !matches!(marker, ProtectedCleanupLocalEntryV1::Quarantined(_))
            || !matches!(inputs, ProtectedCleanupLocalEntryV1::Quarantined(_))
        {
            return Err(self
                .inner
                .invalid("protected cleanup local quarantine did not retain both exact files"));
        }
        self.verify_output_path()
    }

    fn finish_committed_protected_cleanup_local_unlink(
        &self,
        evidence: &ProtectedIntentCleanupEvidenceV1,
    ) -> Result<(), ResumeMarkerErrorV1> {
        let names = ProtectedCleanupLocalNamesV1::new(evidence);
        let marker = self.protected_cleanup_marker_entry(evidence, &names)?;
        let inputs = self.protected_cleanup_inputs_entry(evidence, &names)?;
        let (marker, inputs) = match (marker, inputs) {
            (
                ProtectedCleanupLocalEntryV1::Quarantined(marker),
                ProtectedCleanupLocalEntryV1::Quarantined(inputs),
            ) => {
                fsync(&self.inner.directory).map_err(std::io::Error::from)?;
                (Some(marker), Some(inputs))
            }
            (
                ProtectedCleanupLocalEntryV1::Absent,
                ProtectedCleanupLocalEntryV1::Quarantined(inputs),
            ) => {
                fsync(&self.inner.directory).map_err(std::io::Error::from)?;
                (None, Some(inputs))
            }
            (ProtectedCleanupLocalEntryV1::Absent, ProtectedCleanupLocalEntryV1::Absent) => {
                fsync(&self.inner.directory).map_err(std::io::Error::from)?;
                (None, None)
            }
            _ => {
                return Err(self.inner.invalid(
                    "committed protected cleanup has impossible canonical/quarantine state",
                ));
            }
        };
        if let Some(marker) = marker {
            marker.unlink_and_sync(
                &self.inner.directory,
                &names.marker,
                &self.inner.display_path,
                "protected-cleanup-marker-before-unlink",
                "protected-cleanup-marker-after-unlink",
                "protected-cleanup-marker-directory-synced",
            )?;
        }
        if let Some(inputs) = inputs {
            inputs.unlink_and_sync(
                &self.inner.directory,
                &names.inputs,
                &self.inner.display_path,
                "protected-cleanup-input-before-unlink",
                "protected-cleanup-input-after-unlink",
                "protected-cleanup-input-directory-synced",
            )?;
        }
        fsync(&self.inner.directory).map_err(std::io::Error::from)?;
        if !entry_is_absent_v1(&self.inner.directory, &self.inner.marker_name)?
            || !entry_is_absent_v1(
                &self.inner.directory,
                &envelope_inputs_name(self.inner.package, evidence.attempt),
            )?
            || !entry_is_absent_v1(&self.inner.directory, &names.marker)?
            || !entry_is_absent_v1(&self.inner.directory, &names.inputs)?
        {
            return Err(self
                .inner
                .invalid("committed protected cleanup left Cargo namespace state behind"));
        }
        self.verify_output_path()
    }

    fn finish_committed_superseded_cleanup_local_unlink(
        &self,
        evidence: &ProtectedIntentCleanupEvidenceV1,
    ) -> Result<(), ResumeMarkerErrorV1> {
        let names = ProtectedCleanupLocalNamesV1::new(evidence);
        let marker = PinnedProtectedCleanupFileV1::open(
            &self.inner.directory,
            &names.marker,
            &self.inner.display_path,
        )?;
        if let Some(marker) = &marker {
            marker.require_snapshot(
                evidence.marker_file,
                &names.marker,
                &self.inner.display_path,
            )?;
            self.validate_protected_cleanup_marker_bytes(marker, &names.marker, evidence)?;
        }
        let canonical_inputs = envelope_inputs_name(self.inner.package, evidence.attempt);
        if !entry_is_absent_v1(&self.inner.directory, &canonical_inputs)? {
            return Err(self.inner.invalid_at_name(
                &canonical_inputs,
                "committed superseded cleanup retained canonical predecessor inputs",
            ));
        }
        let inputs = PinnedProtectedCleanupFileV1::open(
            &self.inner.directory,
            &names.inputs,
            &self.inner.display_path,
        )?;
        if let Some(inputs) = &inputs {
            inputs.require_snapshot(
                evidence.inputs_file,
                &names.inputs,
                &self.inner.display_path,
            )?;
            self.validate_protected_cleanup_inputs_bytes(inputs, &names.inputs, evidence)?;
        }
        fsync(&self.inner.directory).map_err(std::io::Error::from)?;
        if let Some(marker) = marker {
            marker.unlink_and_sync(
                &self.inner.directory,
                &names.marker,
                &self.inner.display_path,
                "protected-cleanup-marker-before-unlink",
                "protected-cleanup-marker-after-unlink",
                "protected-cleanup-marker-directory-synced",
            )?;
        }
        if let Some(inputs) = inputs {
            inputs.unlink_and_sync(
                &self.inner.directory,
                &names.inputs,
                &self.inner.display_path,
                "protected-cleanup-input-before-unlink",
                "protected-cleanup-input-after-unlink",
                "protected-cleanup-input-directory-synced",
            )?;
        }
        fsync(&self.inner.directory).map_err(std::io::Error::from)?;
        if !entry_is_absent_v1(&self.inner.directory, &names.marker)?
            || !entry_is_absent_v1(&self.inner.directory, &names.inputs)?
            || !entry_is_absent_v1(&self.inner.directory, &canonical_inputs)?
        {
            return Err(self
                .inner
                .invalid("committed superseded cleanup left predecessor Cargo state behind"));
        }
        self.verify_output_path()
    }

    fn validate_prepared_superseded_cleanup_local_quarantine(
        &self,
        evidence: &ProtectedIntentCleanupEvidenceV1,
    ) -> Result<(), ResumeMarkerErrorV1> {
        let names = ProtectedCleanupLocalNamesV1::new(evidence);
        let marker = PinnedProtectedCleanupFileV1::open(
            &self.inner.directory,
            &names.marker,
            &self.inner.display_path,
        )?
        .ok_or_else(|| {
            self.inner.invalid_at_name(
                &names.marker,
                "prepared superseded cleanup lost its quarantined marker",
            )
        })?;
        marker.require_snapshot(
            evidence.marker_file,
            &names.marker,
            &self.inner.display_path,
        )?;
        self.validate_protected_cleanup_marker_bytes(&marker, &names.marker, evidence)?;
        let canonical_inputs = envelope_inputs_name(self.inner.package, evidence.attempt);
        if !entry_is_absent_v1(&self.inner.directory, &canonical_inputs)? {
            return Err(self.inner.invalid_at_name(
                &canonical_inputs,
                "prepared superseded cleanup retained canonical predecessor inputs",
            ));
        }
        let inputs = PinnedProtectedCleanupFileV1::open(
            &self.inner.directory,
            &names.inputs,
            &self.inner.display_path,
        )?
        .ok_or_else(|| {
            self.inner.invalid_at_name(
                &names.inputs,
                "prepared superseded cleanup lost its quarantined inputs",
            )
        })?;
        inputs.require_snapshot(
            evidence.inputs_file,
            &names.inputs,
            &self.inner.display_path,
        )?;
        self.validate_protected_cleanup_inputs_bytes(&inputs, &names.inputs, evidence)?;
        fsync(&self.inner.directory).map_err(std::io::Error::from)?;
        self.verify_output_path()
    }

    fn rollback_protected_cleanup_local_quarantine(
        &self,
        evidence: &ProtectedIntentCleanupEvidenceV1,
    ) -> Result<(), ResumeMarkerErrorV1> {
        let names = ProtectedCleanupLocalNamesV1::new(evidence);
        let marker = self.protected_cleanup_marker_entry(evidence, &names)?;
        let inputs = self.protected_cleanup_inputs_entry(evidence, &names)?;
        match (marker, inputs) {
            (
                ProtectedCleanupLocalEntryV1::Canonical(_),
                ProtectedCleanupLocalEntryV1::Canonical(_),
            ) => {
                fsync(&self.inner.directory).map_err(std::io::Error::from)?;
            }
            (
                ProtectedCleanupLocalEntryV1::Quarantined(marker),
                ProtectedCleanupLocalEntryV1::Canonical(_),
            ) => {
                fsync(&self.inner.directory).map_err(std::io::Error::from)?;
                marker.rename_and_sync(
                    &self.inner.directory,
                    &names.marker,
                    &self.inner.marker_name,
                    &self.inner.display_path,
                    ProtectedCleanupRenameFaultsV1 {
                        _before_rename: "protected-cleanup-marker-before-rollback-rename",
                        _after_rename: "protected-cleanup-marker-after-rollback-rename",
                        _after_sync: "protected-cleanup-marker-rollback-directory-synced",
                    },
                )?;
            }
            (
                ProtectedCleanupLocalEntryV1::Quarantined(marker),
                ProtectedCleanupLocalEntryV1::Quarantined(inputs),
            ) => {
                fsync(&self.inner.directory).map_err(std::io::Error::from)?;
                inputs.rename_and_sync(
                    &self.inner.directory,
                    &names.inputs,
                    &envelope_inputs_name(self.inner.package, evidence.attempt),
                    &self.inner.display_path,
                    ProtectedCleanupRenameFaultsV1 {
                        _before_rename: "protected-cleanup-input-before-rollback-rename",
                        _after_rename: "protected-cleanup-input-after-rollback-rename",
                        _after_sync: "protected-cleanup-input-rollback-directory-synced",
                    },
                )?;
                marker.rename_and_sync(
                    &self.inner.directory,
                    &names.marker,
                    &self.inner.marker_name,
                    &self.inner.display_path,
                    ProtectedCleanupRenameFaultsV1 {
                        _before_rename: "protected-cleanup-marker-before-rollback-rename",
                        _after_rename: "protected-cleanup-marker-after-rollback-rename",
                        _after_sync: "protected-cleanup-marker-rollback-directory-synced",
                    },
                )?;
            }
            _ => {
                return Err(self.inner.invalid(
                    "protected cleanup rollback cannot reconstruct deleted Cargo-owned state",
                ));
            }
        }
        if self.load()? != Some(evidence.completed_state())
            || self
                .recover_envelope_inputs(evidence.attempt)?
                .identity()
                .as_bytes()
                != evidence.envelope_inputs
        {
            return Err(self
                .inner
                .invalid("protected cleanup local rollback did not restore exact state"));
        }
        fsync(&self.inner.directory).map_err(std::io::Error::from)?;
        self.verify_output_path()
    }

    fn recover_or_restore_protected_intent(
        &self,
        evidence: &ProtectedIntentCleanupEvidenceV1,
    ) -> Result<RecoveredWorkerV2PublicationIntentV2, ResumeMarkerErrorV1> {
        self.verify_output_path()?;
        rollback_worker_v2_publication_intent_cleanup_escrow_v1(
            &self.inner.display_path,
            &self.producer,
            evidence.escrow,
        )
        .map_err(|error| {
            self.inner.invalid(format!(
                "artifact-owned protected cleanup escrow rollback failed: {error}"
            ))
        })?;
        self.verify_output_path()?;
        let recovered = match recover_worker_v2_publication_intent_v2(
            &self.inner.display_path,
            &self.producer,
            evidence.attempt,
            evidence.compiler_closure(),
        ) {
            Ok(recovered) => recovered,
            Err(WorkerV2PublicationIntentErrorV2::NotFound) => {
                return Err(self.inner.invalid(
                    "artifact-owned cleanup rollback did not restore the protected intent",
                ));
            }
            Err(error) => {
                return Err(self
                    .inner
                    .invalid(format!("protected cleanup intent recovery failed: {error}")));
            }
        };
        self.verify_output_path()?;
        if !evidence.matches_recovered(&recovered) {
            return Err(self
                .inner
                .invalid("restored protected intent differs from cleanup evidence"));
        }
        Ok(recovered)
    }

    fn restore_protected_completed_from_cleanup_evidence(
        &self,
        evidence: &ProtectedIntentCleanupEvidenceV1,
    ) -> Result<(), ResumeMarkerErrorV1> {
        evidence
            .validate()
            .map_err(|reason| self.inner.invalid(reason))?;
        let receipt = self.durable_receipt(evidence.attempt)?;
        if !evidence.receipt.matches(receipt)
            || receipt.compiler_closure() != evidence.compiler_closure()
        {
            return Err(self
                .inner
                .invalid("protected cleanup evidence cannot restore a different durable receipt"));
        }
        self.recover_or_restore_protected_intent(evidence)?;
        self.rollback_protected_cleanup_local_quarantine(evidence)?;
        self.recover_or_restore_protected_intent(evidence)?;
        Ok(())
    }

    fn reconcile_protected_cleanup_evidence_on_open(&self) -> Result<(), ResumeMarkerErrorV1> {
        let state = self.load()?;
        let evidence = self.read_protected_cleanup_evidence()?;
        match (state, evidence) {
            (None, None) => Ok(()),
            (Some(completed @ ResumeMarkerStateV2::Completed { .. }), None)
                if completed.publication().requires_envelope() =>
            {
                let closure = completed
                    .completed_receipt()
                    .and_then(ReceiptRecordV2::compiler_closure)
                    .ok_or_else(|| {
                        self.inner
                            .invalid("protected completed marker lacks its exact compiler closure")
                    })?;
                match recover_worker_v2_publication_intent_v2(
                    &self.inner.display_path,
                    &self.producer,
                    completed.attempt(),
                    closure,
                ) {
                    Ok(_) => Ok(()),
                    Err(WorkerV2PublicationIntentErrorV2::NotFound) => {
                        self.ensure_protected_cleanup_evidence(completed, closure)?;
                        let evidence = self.read_protected_cleanup_evidence()?.ok_or_else(|| {
                            self.inner.invalid(
                                "protected cleanup escrow recovery did not publish evidence",
                            )
                        })?;
                        self.reconcile_current_protected_cleanup(&evidence)
                    }
                    Err(error) => Err(self.inner.invalid(format!(
                        "protected completed intent recovery failed without cleanup evidence: {error}"
                    ))),
                }
            }
            (Some(_), None) => Ok(()),
            (None, Some(evidence)) => self.reconcile_current_protected_cleanup(&evidence),
            (Some(completed @ ResumeMarkerStateV2::Completed { .. }), Some(evidence)) => {
                if completed.attempt() == evidence.attempt {
                    self.validate_protected_cleanup_evidence(&evidence, completed)?;
                    return self.reconcile_current_protected_cleanup(&evidence);
                }
                Err(self.inner.invalid(
                    "protected cleanup evidence conflicts with a different completed attempt",
                ))
            }
            (Some(state @ ResumeMarkerStateV2::Ready { .. }), Some(evidence))
                if strictly_newer_attempt_generation_v1(state.attempt(), evidence.attempt) =>
            {
                self.reconcile_superseded_protected_cleanup(&evidence)
            }
            (Some(state @ ResumeMarkerStateV2::Pending { .. }), Some(evidence))
                if strictly_newer_attempt_generation_v1(state.attempt(), evidence.attempt) =>
            {
                self.reconcile_superseded_protected_cleanup(&evidence)
            }
            (Some(_), Some(_)) => Err(self.inner.invalid(
                "protected cleanup evidence conflicts with its attempt's non-completed marker",
            )),
        }
    }

    fn reconcile_current_protected_cleanup(
        &self,
        evidence: &ProtectedIntentCleanupEvidenceV1,
    ) -> Result<(), ResumeMarkerErrorV1> {
        match recover_worker_v2_publication_intent_cleanup_escrow_v1(
            &self.inner.display_path,
            &self.producer,
            evidence.escrow,
        ) {
            Ok(WorkerV2PublicationIntentCleanupEscrowStateV1::Prepared) => {
                let envelope = self.validate_protected_cleanup_envelope(evidence);
                self.restore_protected_completed_from_cleanup_evidence(evidence)?;
                envelope.map(|_| ())
            }
            Ok(WorkerV2PublicationIntentCleanupEscrowStateV1::Committed) => {
                self.finish_committed_protected_cleanup_local_unlink(evidence)?;
                self.remove_protected_cleanup_evidence(evidence)
            }
            Err(_) => {
                self.restore_protected_completed_from_cleanup_evidence(evidence)?;
                self.validate_protected_cleanup_envelope_facts(evidence)
                    .map(|_| ())
            }
        }
    }

    fn reconcile_superseded_protected_cleanup(
        &self,
        evidence: &ProtectedIntentCleanupEvidenceV1,
    ) -> Result<(), ResumeMarkerErrorV1> {
        let state = recover_worker_v2_publication_intent_cleanup_escrow_v1(
            &self.inner.display_path,
            &self.producer,
            evidence.escrow,
        )
        .map_err(|error| {
            self.inner.invalid(format!(
                "superseded protected cleanup escrow recovery failed: {error}"
            ))
        })?;
        match state {
            WorkerV2PublicationIntentCleanupEscrowStateV1::Prepared => {
                self.validate_prepared_superseded_cleanup_local_quarantine(evidence)
            }
            WorkerV2PublicationIntentCleanupEscrowStateV1::Committed => {
                self.finish_committed_superseded_cleanup_local_unlink(evidence)?;
                self.remove_protected_cleanup_evidence(evidence)
            }
        }
    }

    fn commit_protected_cleanup_escrow(
        &self,
        evidence: &ProtectedIntentCleanupEvidenceV1,
    ) -> Result<(), ResumeMarkerErrorV1> {
        self.verify_output_path()?;
        #[cfg(feature = "worker-v2-fault-injection-test-only")]
        injected_fault_point_v1("protected-cleanup-before-artifact-escrow-commit");
        commit_worker_v2_publication_intent_cleanup_escrow_v1(
            &self.inner.display_path,
            &self.producer,
            evidence.escrow,
        )
        .map_err(|error| {
            self.inner.invalid(format!(
                "artifact-owned protected cleanup escrow commit failed: {error}"
            ))
        })?;
        #[cfg(feature = "worker-v2-fault-injection-test-only")]
        injected_fault_point_v1("protected-cleanup-after-artifact-escrow-commit");
        self.verify_output_path()
    }

    fn retire_protected_cleanup_evidence_after_ready(
        &self,
        ready: ResumeMarkerStateV2,
        recovered: &RecoveredWorkerV2PublicationIntentV2,
    ) -> Result<(), ResumeMarkerErrorV1> {
        let Some(evidence) = self.read_protected_cleanup_evidence()? else {
            return Ok(());
        };
        let ResumeMarkerStateV2::Ready { intent, .. } = ready else {
            return Err(ResumeMarkerErrorV1::InvalidTransition);
        };
        if !strictly_newer_attempt_generation_v1(ready.attempt(), evidence.attempt)
            || self.load()? != Some(ready)
            || recovered.record().attempt() != ready.attempt()
            || recovered.record().identity() != intent
        {
            return Err(self.inner.invalid(
                "protected cleanup evidence cannot be retired without a newer exact Ready state",
            ));
        }
        let escrow_state = recover_worker_v2_publication_intent_cleanup_escrow_v1(
            &self.inner.display_path,
            &self.producer,
            evidence.escrow,
        )
        .map_err(|error| {
            self.inner.invalid(format!(
                "superseded protected cleanup escrow recovery failed: {error}"
            ))
        })?;
        if escrow_state == WorkerV2PublicationIntentCleanupEscrowStateV1::Prepared {
            self.validate_prepared_superseded_cleanup_local_quarantine(&evidence)?;
            self.validate_protected_cleanup_envelope(&evidence)?;
        }
        let lease = acquire_worker_v2_publication_intent_lease_v2(
            &self.inner.display_path,
            &self.producer,
            ready.attempt(),
            recovered.compiler_closure(),
            intent,
        )
        .map_err(|error| {
            self.inner.invalid(format!(
                "superseding Ready intent lease acquisition failed: {error}"
            ))
        })?;
        let current = lease.recovered();
        if self.load()? != Some(ready)
            || current.record() != recovered.record()
            || current.exact_output() != recovered.exact_output()
        {
            return Err(self
                .inner
                .invalid("protected Ready state changed during superseded cleanup retirement"));
        }
        lease.revalidate().map_err(|error| {
            self.inner.invalid(format!(
                "superseding Ready intent lease revalidation failed: {error}"
            ))
        })?;
        let commit = commit_worker_v2_publication_intent_cleanup_escrow_after_exact_successor_v2(
            lease,
            &self.producer,
            evidence.escrow,
            ready.attempt(),
            recovered.compiler_closure(),
            intent,
            recovered,
            WorkerV2PublicationIntentCleanupEscrowOptionsV1::default(),
        )
        .map_err(|error| {
            self.inner.invalid(format!(
                "atomic superseding Ready/predecessor escrow commit failed: {error}"
            ))
        })?;
        if commit.predecessor() != evidence.escrow.identity()
            || commit.successor() != recovered.record()
            || commit.grants_publication_authority()
            || commit.grants_load_authority()
            || commit.grants_launch_authority()
        {
            return Err(self
                .inner
                .invalid("atomic superseding Ready cleanup returned inconsistent evidence"));
        }
        #[cfg(feature = "worker-v2-fault-injection-test-only")]
        injected_fault_point_v1("protected-cleanup-newer-ready-after-predecessor-commit");
        #[cfg(feature = "worker-v2-fault-injection-test-only")]
        injected_fault_point_v1("protected-cleanup-newer-ready-after-successor-lease-release");
        if self.load()? != Some(ready) {
            return Err(self
                .inner
                .invalid("superseding Ready marker changed after atomic predecessor commit"));
        }
        #[cfg(feature = "worker-v2-fault-injection-test-only")]
        injected_fault_point_v1("protected-cleanup-newer-ready-before-local-cleanup");
        self.finish_committed_superseded_cleanup_local_unlink(&evidence)?;
        #[cfg(feature = "worker-v2-fault-injection-test-only")]
        injected_fault_point_v1("protected-cleanup-newer-ready-after-local-cleanup");
        self.remove_protected_cleanup_evidence(&evidence)
    }

    fn remove_protected_cleanup_evidence(
        &self,
        expected: &ProtectedIntentCleanupEvidenceV1,
    ) -> Result<(), ResumeMarkerErrorV1> {
        let name = protected_cleanup_evidence_name_v1(self.inner.package);
        let Some(pinned) = PinnedProtectedCleanupFileV1::open(
            &self.inner.directory,
            &name,
            &self.inner.display_path,
        )?
        else {
            let state = recover_worker_v2_publication_intent_cleanup_escrow_v1(
                &self.inner.display_path,
                &self.producer,
                expected.escrow,
            )
            .map_err(|error| {
                self.inner.invalid(format!(
                    "missing cleanup evidence has invalid artifact escrow state: {error}"
                ))
            })?;
            if state != WorkerV2PublicationIntentCleanupEscrowStateV1::Committed {
                return Err(self.inner.invalid_at_name(
                    &name,
                    "protected cleanup evidence disappeared before escrow commit",
                ));
            }
            fsync(&self.inner.directory).map_err(std::io::Error::from)?;
            return self.verify_output_path();
        };
        let bytes = pinned.read_bounded(
            &self.inner.directory,
            &name,
            &self.inner.display_path,
            MAX_PROTECTED_CLEANUP_EVIDENCE_BYTES_V1,
        )?;
        let decoded = ProtectedIntentCleanupEvidenceV1::from_bytes(&bytes)
            .map_err(|reason| self.inner.invalid_at_name(&name, reason))?;
        if decoded != *expected {
            return Err(self.inner.invalid_at_name(
                &name,
                "protected cleanup evidence changed before retirement",
            ));
        }
        let state = recover_worker_v2_publication_intent_cleanup_escrow_v1(
            &self.inner.display_path,
            &self.producer,
            expected.escrow,
        )
        .map_err(|error| {
            self.inner.invalid(format!(
                "protected cleanup evidence escrow recovery failed before retirement: {error}"
            ))
        })?;
        if state != WorkerV2PublicationIntentCleanupEscrowStateV1::Committed {
            return Err(self.inner.invalid_at_name(
                &name,
                "protected cleanup evidence cannot be removed before artifact escrow commit",
            ));
        }
        pinned.unlink_and_sync(
            &self.inner.directory,
            &name,
            &self.inner.display_path,
            "protected-cleanup-evidence-before-unlink",
            "protected-cleanup-evidence-after-unlink",
            "protected-cleanup-evidence-directory-synced",
        )?;
        self.verify_output_path()
    }

    pub(crate) fn publish_load_envelope(
        &self,
        envelope: &WorkerV2LoadEnvelopeV2,
        receipt: BackendPublicationReceiptV2,
        expected_compiler_closure: CompilerClosureV2,
    ) -> Result<WorkerV2EnvelopePublicationOutcomeV1, ResumeMarkerErrorV1> {
        let state = self.load()?.ok_or(ResumeMarkerErrorV1::InvalidTransition)?;
        if !matches!(state, ResumeMarkerStateV2::Ready { .. }) {
            return Err(ResumeMarkerErrorV1::InvalidTransition);
        }
        let recovered = self.recover_complete_intent(state.attempt(), expected_compiler_closure)?;
        self.validate_required_envelope_join(
            state,
            envelope,
            receipt,
            expected_compiler_closure,
            Some(&recovered),
        )?;
        let outcome =
            self.publish_load_envelope_bytes(envelope, receipt, expected_compiler_closure)?;
        if self.load()? != Some(state) {
            return Err(self
                .inner
                .invalid("protected Ready marker changed during envelope publication"));
        }
        Ok(outcome)
    }

    fn publish_load_envelope_bytes(
        &self,
        envelope: &WorkerV2LoadEnvelopeV2,
        receipt: BackendPublicationReceiptV2,
        expected_compiler_closure: CompilerClosureV2,
    ) -> Result<WorkerV2EnvelopePublicationOutcomeV1, ResumeMarkerErrorV1> {
        self.verify_output_path()?;
        self.validate_protected_envelope_receipt(envelope, receipt, expected_compiler_closure)?;
        let bytes = envelope.to_bytes();
        if bytes.is_empty() || bytes.len() > MAX_WORKER_V2_LOAD_ENVELOPE_BYTES_V2 {
            return Err(self
                .inner
                .invalid("protected load envelope exceeds its canonical V2 bound"));
        }
        let decoded = WorkerV2LoadEnvelopeV2::from_bytes(&bytes).map_err(|error| {
            self.inner.invalid(format!(
                "protected load envelope is not canonical V2: {error}"
            ))
        })?;
        if decoded != *envelope {
            return Err(self
                .inner
                .invalid("protected load envelope changed during canonical V2 serialization"));
        }
        let name = protected_envelope_name_v2(receipt.publication_identity());
        if let Some(existing) =
            self.read_load_envelope(&name, receipt, expected_compiler_closure)?
        {
            return if existing == *envelope {
                Ok(WorkerV2EnvelopePublicationOutcomeV1::AlreadyPublished)
            } else {
                Err(self
                    .inner
                    .invalid("conflicting canonical protected load-envelope publication"))
            };
        }

        let temp_name = protected_envelope_temp_name_v2(
            self.inner.package,
            receipt.publication_identity(),
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed),
        );
        let result = (|| {
            let descriptor = openat(
                &self.inner.directory,
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
                &self.inner.directory,
                &file,
                &temp_name,
                &self.inner.display_path,
                Some(bytes.len()),
            )?;
            #[cfg(feature = "worker-v2-fault-injection-test-only")]
            injected_fault_point_v1("protected-envelope-v2-temp-synced");
            self.verify_output_path()?;
            match renameat_with(
                &self.inner.directory,
                &temp_name,
                &self.inner.directory,
                &name,
                RenameFlags::NOREPLACE,
            ) {
                Ok(()) => {}
                Err(rustix::io::Errno::EXIST) => {
                    unlinkat(&self.inner.directory, &temp_name, AtFlags::empty())
                        .map_err(std::io::Error::from)?;
                    let existing = self
                        .read_load_envelope(&name, receipt, expected_compiler_closure)?
                        .ok_or_else(|| {
                            self.inner.invalid_at_name(
                                &name,
                                "protected envelope disappeared after create-new conflict",
                            )
                        })?;
                    return if existing == *envelope {
                        Ok(WorkerV2EnvelopePublicationOutcomeV1::AlreadyPublished)
                    } else {
                        Err(self.inner.invalid_at_name(
                            &name,
                            "conflicting canonical protected load envelope",
                        ))
                    };
                }
                Err(error) => return Err(std::io::Error::from(error).into()),
            }
            fsync(&self.inner.directory).map_err(std::io::Error::from)?;
            self.verify_output_path()?;
            #[cfg(feature = "worker-v2-fault-injection-test-only")]
            injected_fault_point_v1("protected-envelope-v2-published");
            let published = self
                .read_load_envelope(&name, receipt, expected_compiler_closure)?
                .ok_or_else(|| {
                    self.inner.invalid_at_name(
                        &name,
                        "protected envelope is absent after durable create-new publication",
                    )
                })?;
            if published != *envelope {
                return Err(self.inner.invalid_at_name(
                    &name,
                    "protected envelope changed after durable publication",
                ));
            }
            Ok(WorkerV2EnvelopePublicationOutcomeV1::Published)
        })();
        if result.is_err() {
            let _ = unlinkat(&self.inner.directory, &temp_name, AtFlags::empty());
        }
        result
    }

    pub(crate) fn recover_load_envelope(
        &self,
        receipt: BackendPublicationReceiptV2,
        expected_compiler_closure: CompilerClosureV2,
    ) -> Result<WorkerV2LoadEnvelopeV2, ResumeMarkerErrorV1> {
        let state = self.load()?.ok_or(ResumeMarkerErrorV1::InvalidTransition)?;
        if !state.publication().requires_envelope()
            || !matches!(
                state,
                ResumeMarkerStateV2::Ready { .. } | ResumeMarkerStateV2::Completed { .. }
            )
        {
            return Err(ResumeMarkerErrorV1::InvalidTransition);
        }
        let name = protected_envelope_name_v2(receipt.publication_identity());
        let envelope = self
            .read_load_envelope(&name, receipt, expected_compiler_closure)?
            .ok_or_else(|| {
                self.inner.invalid_at_name(
                    &name,
                    "canonical protected Worker V2 load envelope is missing",
                )
            })?;
        self.verify_output_path()?;
        let recovered = match recover_worker_v2_publication_intent_v2(
            &self.inner.display_path,
            &self.producer,
            state.attempt(),
            expected_compiler_closure,
        ) {
            Ok(recovered) => {
                self.validate_complete_intent_snapshot(
                    &recovered,
                    state.attempt(),
                    expected_compiler_closure,
                )?;
                Some(recovered)
            }
            Err(WorkerV2PublicationIntentErrorV2::NotFound)
                if matches!(state, ResumeMarkerStateV2::Completed { .. }) =>
            {
                None
            }
            Err(error) => {
                return Err(self.inner.invalid(format!(
                    "the protected envelope's complete publication intent cannot be recovered: {error}"
                )));
            }
        };
        self.verify_output_path()?;
        self.validate_required_envelope_join(
            state,
            &envelope,
            receipt,
            expected_compiler_closure,
            recovered.as_ref(),
        )?;
        Ok(envelope)
    }

    fn read_load_envelope(
        &self,
        name: &str,
        receipt: BackendPublicationReceiptV2,
        expected_compiler_closure: CompilerClosureV2,
    ) -> Result<Option<WorkerV2LoadEnvelopeV2>, ResumeMarkerErrorV1> {
        self.verify_output_path()?;
        let descriptor = match openat(
            &self.inner.directory,
            name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::empty(),
        ) {
            Ok(descriptor) => descriptor,
            Err(rustix::io::Errno::NOENT) => return Ok(None),
            Err(error) => return Err(std::io::Error::from(error).into()),
        };
        validate_private_file(
            &self.inner.directory,
            &descriptor,
            name,
            &self.inner.display_path,
            None,
        )?;
        let initial = fstat(&descriptor).map_err(std::io::Error::from)?;
        let initial_size = usize::try_from(initial.st_size).ok();
        if initial_size.is_none_or(|size| size == 0 || size > MAX_WORKER_V2_LOAD_ENVELOPE_BYTES_V2)
        {
            return Err(self.inner.invalid_at_name(
                name,
                "protected envelope size exceeds its canonical V2 bound",
            ));
        }
        let mut file = File::from(descriptor);
        let mut bytes = Vec::with_capacity(initial_size.unwrap_or(0).saturating_add(1));
        Read::by_ref(&mut file)
            .take((MAX_WORKER_V2_LOAD_ENVELOPE_BYTES_V2 + 1) as u64)
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
            || bytes.len() > MAX_WORKER_V2_LOAD_ENVELOPE_BYTES_V2
        {
            return Err(self
                .inner
                .invalid_at_name(name, "protected envelope changed while it was read"));
        }
        let envelope = WorkerV2LoadEnvelopeV2::from_bytes(&bytes).map_err(|error| {
            self.inner
                .invalid_at_name(name, format!("invalid protected V2 envelope: {error}"))
        })?;
        if envelope.to_bytes() != bytes {
            return Err(self
                .inner
                .invalid_at_name(name, "protected V2 envelope encoding is not canonical"));
        }
        self.validate_protected_envelope_receipt(&envelope, receipt, expected_compiler_closure)?;
        Ok(Some(envelope))
    }

    fn validate_protected_envelope_receipt(
        &self,
        envelope: &WorkerV2LoadEnvelopeV2,
        receipt: BackendPublicationReceiptV2,
        expected_compiler_closure: CompilerClosureV2,
    ) -> Result<(), ResumeMarkerErrorV1> {
        let evidence = envelope.final_artifact_evidence();
        let claim = evidence.published_claim();
        let intent = evidence.publication_intent_transcript();
        let final_output_identity: [u8; 32] = Sha256::digest(envelope.finalized_payload()).into();
        if claim.plan().scope().package().as_bytes() != &self.inner.package
            || evidence.backend_receipt() != receipt
            || claim.receipt() != receipt
            || evidence.compiler_closure() != expected_compiler_closure
            || intent.compiler_closure() != expected_compiler_closure
            || !intent.matches_backend_receipt(receipt)
            || intent.plan() != claim.plan()
            || intent.upstream_evidence() != claim.upstream_evidence()
            || usize::try_from(intent.output_length()) != Ok(envelope.finalized_payload().len())
            || intent.output_identity().as_bytes() != &final_output_identity
            || receipt.compiler_closure() != expected_compiler_closure
            || claim.compiler_closure() != expected_compiler_closure
            || envelope.grants_compiler_authority()
            || envelope.grants_proof_authority()
            || envelope.grants_currentness_authority()
            || envelope.grants_load_authority()
            || envelope.grants_launch_authority()
        {
            return Err(self
                .inner
                .invalid("protected load envelope disagrees with its exact V2 receipt"));
        }
        self.validate_durable_receipt(claim.plan().attempt(), receipt, expected_compiler_closure)?;
        self.recover_complete_claim(claim, receipt, expected_compiler_closure)?;
        Ok(())
    }

    fn validate_required_envelope_join(
        &self,
        state: ResumeMarkerStateV2,
        envelope: &WorkerV2LoadEnvelopeV2,
        receipt: BackendPublicationReceiptV2,
        expected_compiler_closure: CompilerClosureV2,
        recovered: Option<&RecoveredWorkerV2PublicationIntentV2>,
    ) -> Result<(), ResumeMarkerErrorV1> {
        if self.load()? != Some(state)
            || !state.publication().requires_envelope()
            || !matches!(
                state,
                ResumeMarkerStateV2::Ready { .. } | ResumeMarkerStateV2::Completed { .. }
            )
        {
            return Err(ResumeMarkerErrorV1::InvalidTransition);
        }
        self.validate_protected_envelope_receipt(envelope, receipt, expected_compiler_closure)?;
        if let Some(recorded_receipt) = state.completed_receipt()
            && (!recorded_receipt.matches(receipt)
                || recorded_receipt.compiler_closure() != Some(expected_compiler_closure)
                || state.envelope() != envelope.identity().as_bytes())
        {
            return Err(ResumeMarkerErrorV1::InvalidTransition);
        }

        let evidence = envelope.final_artifact_evidence();
        let intent = evidence.publication_intent_transcript();
        let envelope_inputs = self.recover_envelope_inputs(state.attempt())?;
        let carried_inputs = WorkerV2EnvelopeInputsV1::new(
            envelope.direct_link_evidence().clone(),
            envelope.proof_records().to_vec(),
            envelope.raw_hsaco().clone(),
        )
        .map_err(|error| {
            self.inner.invalid(format!(
                "required protected envelope inputs are invalid: {error}"
            ))
        })?;
        if state.intent() != Some(intent.source_record_identity())
            || intent.attempt() != state.attempt()
            || intent.package_identity().as_bytes() != &self.inner.package
            || intent.compiler_closure() != expected_compiler_closure
            || state.envelope_inputs() != envelope_inputs.identity().as_bytes()
            || carried_inputs != envelope_inputs
            || restart_admission_commitment_with_inputs_v2(
                state.publication(),
                intent.plan(),
                intent.upstream_evidence(),
                envelope.finalized_payload(),
                Some(envelope_inputs.identity()),
                expected_compiler_closure,
            ) != state.admission()
        {
            return Err(self
                .inner
                .invalid("required protected envelope disagrees with its resume marker"));
        }

        if let Some(recovered) = recovered {
            self.validate_complete_intent_snapshot(
                recovered,
                state.attempt(),
                expected_compiler_closure,
            )?;
            let record = recovered.record();
            if !intent.matches_source_record(record)
                || recovered.exact_output() != envelope.finalized_payload()
                || record.identity() != intent.source_record_identity()
                || record.plan() != evidence.published_claim().plan()
                || record.upstream_evidence() != evidence.published_claim().upstream_evidence()
            {
                return Err(self.inner.invalid(
                    "required protected envelope differs from its complete durable intent",
                ));
            }
        } else if !matches!(state, ResumeMarkerStateV2::Completed { .. }) {
            return Err(ResumeMarkerErrorV1::InvalidTransition);
        }
        Ok(())
    }

    fn validate_non_envelope_intent_join(
        &self,
        state: ResumeMarkerStateV2,
        recovered: &RecoveredWorkerV2PublicationIntentV2,
        receipt: BackendPublicationReceiptV2,
        expected_compiler_closure: CompilerClosureV2,
    ) -> Result<(), ResumeMarkerErrorV1> {
        if self.load()? != Some(state)
            || state.publication().requires_envelope()
            || !matches!(
                state,
                ResumeMarkerStateV2::Ready { .. } | ResumeMarkerStateV2::Completed { .. }
            )
        {
            return Err(ResumeMarkerErrorV1::InvalidTransition);
        }
        self.validate_complete_intent_snapshot(
            recovered,
            state.attempt(),
            expected_compiler_closure,
        )?;
        let record = recovered.record();
        if state.intent() != Some(record.identity())
            || restart_admission_commitment_with_inputs_v2(
                state.publication(),
                record.plan(),
                record.upstream_evidence(),
                recovered.exact_output(),
                None,
                expected_compiler_closure,
            ) != state.admission()
        {
            return Err(self
                .inner
                .invalid("protected completion differs from its complete durable intent"));
        }
        if let Some(recorded_receipt) = state.completed_receipt()
            && (!recorded_receipt.matches(receipt)
                || recorded_receipt.compiler_closure() != Some(expected_compiler_closure))
        {
            return Err(ResumeMarkerErrorV1::InvalidTransition);
        }
        let claim = self.recover_claim_for_intent(recovered, receipt, expected_compiler_closure)?;
        if claim.plan() != record.plan()
            || claim.upstream_evidence() != record.upstream_evidence()
            || claim.receipt() != receipt
            || claim.compiler_closure() != expected_compiler_closure
        {
            return Err(self
                .inner
                .invalid("protected completion claim differs from its durable intent"));
        }
        Ok(())
    }

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
        let pending = ResumeMarkerStateV2::Pending {
            publication,
            attempt,
            admission,
            envelope_inputs,
        };
        match self.load()? {
            None => self.inner.write_protected(pending, false),
            Some(existing) if existing == pending => Ok(()),
            Some(_) => Err(ResumeMarkerErrorV1::ConflictingMarker),
        }
    }

    pub(crate) fn persist_ready(
        &self,
        publication: WorkerV2PublicationKindV1,
        attempt: BuildAttempt,
        recovered: &RecoveredWorkerV2PublicationIntentV2,
        expected_compiler_closure: CompilerClosureV2,
    ) -> Result<(), ResumeMarkerErrorV1> {
        self.validate_complete_intent_snapshot(recovered, attempt, expected_compiler_closure)?;
        let lease = acquire_worker_v2_publication_intent_lease_v2(
            &self.inner.display_path,
            &self.producer,
            attempt,
            expected_compiler_closure,
            recovered.record().identity(),
        )
        .map_err(|error| {
            self.inner.invalid(format!(
                "protected Ready promotion could not acquire its exact local intent lease: {error}"
            ))
        })?;
        let local = lease.recovered();
        if local.record() != recovered.record()
            || local.exact_output() != recovered.exact_output()
            || local.compiler_closure() != recovered.compiler_closure()
        {
            return Err(self.inner.invalid(
                "protected Ready promotion requires the exact durable local publication intent",
            ));
        }
        let record = local.record();
        let intent = record.identity();
        let ready = match self.load()? {
            Some(ResumeMarkerStateV2::Pending {
                publication: current_publication,
                attempt: current_attempt,
                admission,
                envelope_inputs,
            }) if current_publication == publication
                && current_attempt == attempt
                && restart_admission_commitment_with_inputs_v2(
                    publication,
                    record.plan(),
                    record.upstream_evidence(),
                    local.exact_output(),
                    publication
                        .requires_envelope()
                        .then(|| WorkerV2EnvelopeInputsIdentityV1::from_bytes(envelope_inputs)),
                    expected_compiler_closure,
                ) == admission =>
            {
                let ready = ResumeMarkerStateV2::Ready {
                    publication,
                    attempt,
                    admission,
                    envelope_inputs,
                    intent,
                };
                self.inner.write_protected(ready, true)?;
                #[cfg(feature = "worker-v2-fault-injection-test-only")]
                injected_fault_point_v1("protected-ready-after-marker-publication");
                ready
            }
            Some(
                ready @ ResumeMarkerStateV2::Ready {
                    publication: current_publication,
                    attempt: current_attempt,
                    admission,
                    envelope_inputs,
                    intent: current_intent,
                },
            ) if current_publication == publication
                && current_attempt == attempt
                && current_intent == intent
                && restart_admission_commitment_with_inputs_v2(
                    publication,
                    record.plan(),
                    record.upstream_evidence(),
                    local.exact_output(),
                    publication
                        .requires_envelope()
                        .then(|| WorkerV2EnvelopeInputsIdentityV1::from_bytes(envelope_inputs)),
                    expected_compiler_closure,
                ) == admission =>
            {
                ready
            }
            _ => return Err(ResumeMarkerErrorV1::InvalidTransition),
        };
        lease.revalidate().map_err(|error| {
            self.inner.invalid(format!(
                "protected Ready promotion lost its exact local intent lease: {error}"
            ))
        })?;
        if self.load()? != Some(ready) {
            return Err(self
                .inner
                .invalid("protected Ready marker changed after durable publication"));
        }
        drop(lease);
        self.retire_protected_cleanup_evidence_after_ready(ready, recovered)
    }

    pub(crate) fn persist_completed(
        &self,
        publication: WorkerV2PublicationKindV1,
        attempt: BuildAttempt,
        intent: WorkerV2PublicationIntentIdentityV2,
        receipt: BackendPublicationReceiptV2,
        expected_compiler_closure: CompilerClosureV2,
    ) -> Result<ResumeMarkerStateV2, ResumeMarkerErrorV1> {
        if publication.requires_envelope() {
            return Err(ResumeMarkerErrorV1::InvalidTransition);
        }
        let state = self.load()?.ok_or(ResumeMarkerErrorV1::InvalidTransition)?;
        let recovered = self.recover_complete_intent(attempt, expected_compiler_closure)?;
        if recovered.record().identity() != intent {
            return Err(ResumeMarkerErrorV1::InvalidTransition);
        }
        self.validate_non_envelope_intent_join(
            state,
            &recovered,
            receipt,
            expected_compiler_closure,
        )?;
        self.persist_completed_inner(
            publication,
            attempt,
            intent,
            receipt,
            expected_compiler_closure,
            None,
        )
    }

    pub(crate) fn persist_envelope_and_completed(
        &self,
        publication: WorkerV2PublicationKindV1,
        attempt: BuildAttempt,
        intent: WorkerV2PublicationIntentIdentityV2,
        receipt: BackendPublicationReceiptV2,
        expected_compiler_closure: CompilerClosureV2,
        envelope: &WorkerV2LoadEnvelopeV2,
    ) -> Result<ResumeMarkerStateV2, ResumeMarkerErrorV1> {
        if !publication.requires_envelope() {
            return Err(ResumeMarkerErrorV1::InvalidTransition);
        }
        let state = self.load()?.ok_or(ResumeMarkerErrorV1::InvalidTransition)?;
        if state.publication() != publication
            || state.attempt() != attempt
            || state.intent() != Some(intent)
        {
            return Err(ResumeMarkerErrorV1::InvalidTransition);
        }
        let recovered = self.recover_complete_intent(attempt, expected_compiler_closure)?;
        if recovered.record().identity() != intent {
            return Err(ResumeMarkerErrorV1::InvalidTransition);
        }
        self.validate_required_envelope_join(
            state,
            envelope,
            receipt,
            expected_compiler_closure,
            Some(&recovered),
        )?;
        match state {
            ResumeMarkerStateV2::Ready { .. } => {
                self.publish_load_envelope(envelope, receipt, expected_compiler_closure)?;
            }
            ResumeMarkerStateV2::Completed { .. } => {
                let persisted = self.recover_load_envelope(receipt, expected_compiler_closure)?;
                if persisted != *envelope {
                    return Err(self
                        .inner
                        .invalid("completed protected envelope changed during exact retry"));
                }
            }
            ResumeMarkerStateV2::Pending { .. } => {
                return Err(ResumeMarkerErrorV1::InvalidTransition);
            }
        }
        let envelope_identity = envelope.identity();
        self.persist_completed_inner(
            publication,
            attempt,
            intent,
            receipt,
            expected_compiler_closure,
            Some(envelope_identity),
        )
    }

    /// Recovers only the canonical protected V2 envelope named by `receipt` before completion.
    pub(crate) fn recover_envelope_and_completed(
        &self,
        publication: WorkerV2PublicationKindV1,
        attempt: BuildAttempt,
        intent: WorkerV2PublicationIntentIdentityV2,
        receipt: BackendPublicationReceiptV2,
        expected_compiler_closure: CompilerClosureV2,
    ) -> Result<ResumeMarkerStateV2, ResumeMarkerErrorV1> {
        if !publication.requires_envelope() {
            return Err(ResumeMarkerErrorV1::InvalidTransition);
        }
        self.validate_durable_receipt(attempt, receipt, expected_compiler_closure)?;
        let envelope = self.recover_load_envelope(receipt, expected_compiler_closure)?;
        self.persist_envelope_and_completed(
            publication,
            attempt,
            intent,
            receipt,
            expected_compiler_closure,
            &envelope,
        )
    }

    fn persist_completed_inner(
        &self,
        publication: WorkerV2PublicationKindV1,
        attempt: BuildAttempt,
        intent: WorkerV2PublicationIntentIdentityV2,
        receipt: BackendPublicationReceiptV2,
        expected_compiler_closure: CompilerClosureV2,
        envelope: Option<WorkerV2LoadEnvelopeIdentityV2>,
    ) -> Result<ResumeMarkerStateV2, ResumeMarkerErrorV1> {
        let envelope = envelope.map_or([0; 32], |identity| identity.as_bytes());
        self.validate_durable_receipt(attempt, receipt, expected_compiler_closure)?;
        let receipt = ReceiptRecordV2::from_receipt(receipt);
        if intent.as_bytes() == [0; 32]
            || !receipt.is_complete()
            || publication.requires_envelope() != (envelope != [0; 32])
        {
            return Err(ResumeMarkerErrorV1::InvalidTransition);
        }
        match self.load()? {
            Some(ResumeMarkerStateV2::Ready {
                publication: current_publication,
                attempt: current_attempt,
                admission,
                envelope_inputs,
                intent: current_intent,
            }) if current_publication == publication
                && current_attempt == attempt
                && current_intent == intent =>
            {
                let completed = ResumeMarkerStateV2::Completed {
                    publication,
                    attempt,
                    admission,
                    envelope_inputs,
                    envelope,
                    intent,
                    receipt,
                };
                self.inner.write_protected(completed, true)?;
                Ok(completed)
            }
            Some(
                existing @ ResumeMarkerStateV2::Completed {
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
                && current_receipt == receipt
                && current_envelope == envelope =>
            {
                Ok(existing)
            }
            _ => Err(ResumeMarkerErrorV1::InvalidTransition),
        }
    }

    fn durable_receipt(
        &self,
        attempt: BuildAttempt,
    ) -> Result<BackendPublicationReceiptV2, ResumeMarkerErrorV1> {
        match read_backend_publication_receipt_v2(&self.inner.display_path, &self.producer, attempt)
        {
            Ok(PersistedBackendReceiptV2::Provenance(receipt)) => Ok(receipt),
            Ok(
                PersistedBackendReceiptV2::None | PersistedBackendReceiptV2::PendingProvenance(_),
            ) => Err(self
                .inner
                .invalid("the exact protected publication receipt is not durable")),
            Err(error) => Err(self.inner.invalid(format!(
                "the protected publication receipt cannot be recovered: {error}"
            ))),
        }
    }

    fn validate_durable_receipt(
        &self,
        attempt: BuildAttempt,
        receipt: BackendPublicationReceiptV2,
        expected_compiler_closure: CompilerClosureV2,
    ) -> Result<(), ResumeMarkerErrorV1> {
        if receipt.compiler_closure() != expected_compiler_closure
            || self.durable_receipt(attempt)? != receipt
        {
            return Err(ResumeMarkerErrorV1::InvalidTransition);
        }
        Ok(())
    }

    fn validate_completed_state(
        &self,
        state: ResumeMarkerStateV2,
        expected_compiler_closure: CompilerClosureV2,
    ) -> Result<(), ResumeMarkerErrorV1> {
        let Some(record) = state.completed_receipt() else {
            return Ok(());
        };
        if record.compiler_closure() != Some(expected_compiler_closure) {
            return Err(ResumeMarkerErrorV1::InvalidTransition);
        }
        let receipt = self.durable_receipt(state.attempt())?;
        if receipt.compiler_closure() != expected_compiler_closure || !record.matches(receipt) {
            return Err(ResumeMarkerErrorV1::InvalidTransition);
        }
        if state.publication().requires_envelope() {
            self.recover_load_envelope(receipt, expected_compiler_closure)?;
        }
        Ok(())
    }

    fn validate_completed_receipt(
        &self,
        state: ResumeMarkerStateV2,
        receipt: BackendPublicationReceiptV2,
        expected_compiler_closure: CompilerClosureV2,
    ) -> Result<(), ResumeMarkerErrorV1> {
        let Some(record) = state.completed_receipt() else {
            return Err(ResumeMarkerErrorV1::InvalidTransition);
        };
        if record.compiler_closure() != Some(expected_compiler_closure) || !record.matches(receipt)
        {
            return Err(ResumeMarkerErrorV1::InvalidTransition);
        }
        self.validate_durable_receipt(state.attempt(), receipt, expected_compiler_closure)
    }

    fn validate_completed_restart_inputs(
        &self,
        state: ResumeMarkerStateV2,
        receipt: BackendPublicationReceiptV2,
        expected_compiler_closure: CompilerClosureV2,
        recovered: Option<&RecoveredWorkerV2PublicationIntentV2>,
    ) -> Result<(), ResumeMarkerErrorV1> {
        self.validate_completed_receipt(state, receipt, expected_compiler_closure)?;
        if state.publication().requires_envelope() {
            let envelope = self.recover_load_envelope(receipt, expected_compiler_closure)?;
            self.validate_required_envelope_join(
                state,
                &envelope,
                receipt,
                expected_compiler_closure,
                recovered,
            )
        } else if let Some(recovered) = recovered {
            self.validate_non_envelope_intent_join(
                state,
                recovered,
                receipt,
                expected_compiler_closure,
            )
        } else {
            self.verify_output_path()?;
            let recovered = match recover_worker_v2_publication_intent_v2(
                &self.inner.display_path,
                &self.producer,
                state.attempt(),
                expected_compiler_closure,
            ) {
                Ok(recovered) => Some(recovered),
                Err(WorkerV2PublicationIntentErrorV2::NotFound) => None,
                Err(error) => {
                    return Err(self.inner.invalid(format!(
                        "the completed protected intent cannot be recovered before cleanup: {error}"
                    )));
                }
            };
            self.verify_output_path()?;
            if let Some(recovered) = recovered {
                self.validate_non_envelope_intent_join(
                    state,
                    &recovered,
                    receipt,
                    expected_compiler_closure,
                )
            } else {
                Ok(())
            }
        }
    }

    pub(crate) fn clear_completed(
        &self,
        expected: ResumeMarkerStateV2,
        receipt: BackendPublicationReceiptV2,
        expected_compiler_closure: CompilerClosureV2,
    ) -> Result<(), ResumeMarkerErrorV1> {
        self.validate_completed_restart_inputs(expected, receipt, expected_compiler_closure, None)?;
        self.clear_exact(expected)
    }

    pub(crate) fn clear_completed_and_envelope_inputs(
        &self,
        expected: ResumeMarkerStateV2,
        receipt: BackendPublicationReceiptV2,
        expected_compiler_closure: CompilerClosureV2,
    ) -> Result<(), ResumeMarkerErrorV1> {
        self.validate_completed_restart_inputs(expected, receipt, expected_compiler_closure, None)?;
        if expected.publication().requires_envelope() {
            self.ensure_protected_cleanup_evidence(expected, expected_compiler_closure)?;
            self.clear_protected_completed_and_envelope_inputs(expected)
        } else {
            self.clear_exact_and_envelope_inputs(expected)
        }
    }

    pub(crate) fn clear_abandoned_pending(
        &self,
        expected: ResumeMarkerStateV2,
    ) -> Result<(), ResumeMarkerErrorV1> {
        if !matches!(expected, ResumeMarkerStateV2::Pending { .. }) {
            return Err(ResumeMarkerErrorV1::InvalidTransition);
        }
        self.clear_exact_and_envelope_inputs(expected)
    }

    fn clear_exact_and_envelope_inputs(
        &self,
        expected: ResumeMarkerStateV2,
    ) -> Result<(), ResumeMarkerErrorV1> {
        let expected_inputs = if expected.publication().requires_envelope() {
            let inputs = self.recover_envelope_inputs(expected.attempt())?;
            if inputs.identity().as_bytes() != expected.envelope_inputs() {
                return Err(self
                    .inner
                    .invalid("marker disagrees with its envelope input capsule"));
            }
            Some(inputs.identity())
        } else {
            None
        };
        self.clear_exact(expected)?;
        if let Some(identity) = expected_inputs {
            self.inner
                .remove_envelope_inputs(expected.attempt(), identity)?;
        }
        Ok(())
    }

    fn capture_protected_cleanup_local_snapshots(
        &self,
        completed: ResumeMarkerStateV2,
    ) -> Result<
        (
            ProtectedCleanupLocalFileSnapshotV1,
            ProtectedCleanupLocalFileSnapshotV1,
        ),
        ResumeMarkerErrorV1,
    > {
        let marker = PinnedProtectedCleanupFileV1::open(
            &self.inner.directory,
            &self.inner.marker_name,
            &self.inner.display_path,
        )?
        .ok_or(ResumeMarkerErrorV1::InvalidTransition)?;
        let marker_bytes = marker.read_bounded(
            &self.inner.directory,
            &self.inner.marker_name,
            &self.inner.display_path,
            PROTECTED_MARKER_BYTES,
        )?;
        if marker_bytes != encode_protected_marker(self.inner.package, completed) {
            return Err(self
                .inner
                .invalid("cleanup snapshot marker differs from the exact completed state"));
        }
        let inputs_name = envelope_inputs_name(self.inner.package, completed.attempt());
        let inputs = PinnedProtectedCleanupFileV1::open(
            &self.inner.directory,
            &inputs_name,
            &self.inner.display_path,
        )?
        .ok_or(ResumeMarkerErrorV1::InvalidTransition)?;
        let inputs_bytes = inputs.read_bounded(
            &self.inner.directory,
            &inputs_name,
            &self.inner.display_path,
            MAX_WORKER_V2_ENVELOPE_INPUTS_BYTES,
        )?;
        let decoded = WorkerV2EnvelopeInputsV1::from_bytes(&inputs_bytes).map_err(|error| {
            self.inner.invalid_at_name(
                &inputs_name,
                format!("cleanup snapshot input capsule is invalid: {error}"),
            )
        })?;
        if decoded.identity().as_bytes() != completed.envelope_inputs()
            || decoded.to_bytes() != inputs_bytes
        {
            return Err(self
                .inner
                .invalid("cleanup snapshot inputs differ from the exact completed state"));
        }
        Ok((marker.snapshot()?, inputs.snapshot()?))
    }

    fn ensure_protected_cleanup_evidence(
        &self,
        completed: ResumeMarkerStateV2,
        expected_compiler_closure: CompilerClosureV2,
    ) -> Result<(), ResumeMarkerErrorV1> {
        if let Some(evidence) = self.read_protected_cleanup_evidence()? {
            self.validate_protected_cleanup_evidence(&evidence, completed)?;
            self.ensure_protected_cleanup_escrow_prepared(&evidence)?;
            if let Err(error) = self.validate_protected_cleanup_envelope(&evidence) {
                self.restore_protected_completed_from_cleanup_evidence(&evidence)?;
                return Err(error);
            }
            return Ok(());
        }
        let ResumeMarkerStateV2::Completed { intent, .. } = completed else {
            return Err(ResumeMarkerErrorV1::InvalidTransition);
        };
        let inputs = self.recover_envelope_inputs(completed.attempt())?;
        if inputs.identity().as_bytes() != completed.envelope_inputs() {
            return Err(self
                .inner
                .invalid("completed marker disagrees with its envelope input capsule"));
        }
        let receipt = self.durable_receipt(completed.attempt())?;
        self.validate_completed_receipt(completed, receipt, expected_compiler_closure)?;
        let (marker_file, inputs_file) =
            self.capture_protected_cleanup_local_snapshots(completed)?;
        let escrow = prepare_worker_v2_publication_intent_cleanup_escrow_v1(
            &self.inner.display_path,
            &self.producer,
            completed.attempt(),
            expected_compiler_closure,
            intent,
            receipt,
        )
        .map_err(|error| {
            self.inner.invalid(format!(
                "artifact-owned protected cleanup escrow preparation failed: {error}"
            ))
        })?;
        let evidence = ProtectedIntentCleanupEvidenceV1::new(
            self.inner.package,
            completed,
            marker_file,
            inputs_file,
            escrow,
        )
        .map_err(|reason| self.inner.invalid(reason))?;
        self.persist_protected_cleanup_evidence(&evidence)?;
        self.validate_protected_cleanup_evidence(&evidence, completed)?;
        if let Err(error) = self.validate_protected_cleanup_envelope(&evidence) {
            self.restore_protected_completed_from_cleanup_evidence(&evidence)?;
            return Err(error);
        }
        Ok(())
    }

    fn clear_protected_completed_and_envelope_inputs(
        &self,
        expected: ResumeMarkerStateV2,
    ) -> Result<(), ResumeMarkerErrorV1> {
        let evidence = self.read_protected_cleanup_evidence()?.ok_or_else(|| {
            self.inner
                .invalid("protected completed-state cleanup requires durable recovery evidence")
        })?;
        self.validate_protected_cleanup_evidence(&evidence, expected)?;
        self.validate_protected_cleanup_envelope(&evidence)?;
        let inputs = self.recover_envelope_inputs(expected.attempt())?;
        if inputs.identity().as_bytes() != evidence.envelope_inputs
            || inputs.identity().as_bytes() != expected.envelope_inputs()
        {
            return Err(self
                .inner
                .invalid("completed marker disagrees with its cleanup envelope inputs"));
        }

        self.prepare_protected_cleanup_local_quarantine(&evidence)?;
        if let Err(error) = self.validate_protected_cleanup_envelope(&evidence) {
            self.restore_protected_completed_from_cleanup_evidence(&evidence)?;
            return Err(error);
        }
        self.commit_protected_cleanup_escrow(&evidence)?;
        self.finish_committed_protected_cleanup_local_unlink(&evidence)?;
        self.remove_protected_cleanup_evidence(&evidence)
    }

    fn clear_exact(&self, expected: ResumeMarkerStateV2) -> Result<(), ResumeMarkerErrorV1> {
        if self.load()? != Some(expected) {
            return Err(ResumeMarkerErrorV1::InvalidTransition);
        }
        unlinkat(
            &self.inner.directory,
            &self.inner.marker_name,
            AtFlags::empty(),
        )
        .map_err(std::io::Error::from)?;
        fsync(&self.inner.directory).map_err(std::io::Error::from)?;
        self.verify_output_path()?;
        Ok(())
    }
}

fn strictly_newer_attempt_generation_v1(
    candidate: BuildAttempt,
    predecessor: BuildAttempt,
) -> bool {
    candidate.generation() > predecessor.generation()
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

struct PinnedProtectedCleanupFileV1 {
    file: File,
    initial: rustix::fs::Stat,
}

struct ProtectedCleanupRenameFaultsV1 {
    _before_rename: &'static str,
    _after_rename: &'static str,
    _after_sync: &'static str,
}

#[cfg(test)]
struct ProtectedCleanupBeforeUnlinkTestHookV1 {
    expected_name: String,
    hook: Box<dyn FnOnce()>,
}

#[cfg(test)]
thread_local! {
    static PROTECTED_CLEANUP_BEFORE_UNLINK_TEST_HOOK_V1:
        std::cell::RefCell<Option<ProtectedCleanupBeforeUnlinkTestHookV1>> = const {
            std::cell::RefCell::new(None)
        };
}

#[cfg(test)]
fn install_protected_cleanup_before_unlink_test_hook_v1(
    expected_name: String,
    hook: impl FnOnce() + 'static,
) {
    PROTECTED_CLEANUP_BEFORE_UNLINK_TEST_HOOK_V1.with(|installed| {
        assert!(
            installed
                .borrow_mut()
                .replace(ProtectedCleanupBeforeUnlinkTestHookV1 {
                    expected_name,
                    hook: Box::new(hook),
                })
                .is_none(),
            "a protected-cleanup unlink test hook is already installed"
        );
    });
}

#[cfg(test)]
fn run_protected_cleanup_before_unlink_test_hook_v1(name: &str) {
    PROTECTED_CLEANUP_BEFORE_UNLINK_TEST_HOOK_V1.with(|installed| {
        let matches = installed
            .borrow()
            .as_ref()
            .is_some_and(|hook| hook.expected_name == name);
        if matches {
            let installed = installed
                .borrow_mut()
                .take()
                .expect("matching protected-cleanup unlink test hook disappeared");
            (installed.hook)();
        }
    });
}

impl PinnedProtectedCleanupFileV1 {
    fn open(
        directory: &OwnedFd,
        name: &str,
        display_path: &Path,
    ) -> Result<Option<Self>, ResumeMarkerErrorV1> {
        let descriptor = match openat(
            directory,
            name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::empty(),
        ) {
            Ok(descriptor) => descriptor,
            Err(rustix::io::Errno::NOENT) => return Ok(None),
            Err(error) => return Err(std::io::Error::from(error).into()),
        };
        validate_private_file(directory, &descriptor, name, display_path, None)?;
        let initial = fstat(&descriptor).map_err(std::io::Error::from)?;
        Ok(Some(Self {
            file: File::from(descriptor),
            initial,
        }))
    }

    fn require_named_as(
        &self,
        directory: &OwnedFd,
        name: &str,
        display_path: &Path,
    ) -> Result<(), ResumeMarkerErrorV1> {
        validate_private_file(directory, &self.file, name, display_path, None)?;
        let current = fstat(&self.file).map_err(std::io::Error::from)?;
        if current.st_dev != self.initial.st_dev
            || current.st_ino != self.initial.st_ino
            || current.st_mode != self.initial.st_mode
            || current.st_size != self.initial.st_size
            || current.st_nlink != 1
            || current.st_mtime != self.initial.st_mtime
            || current.st_mtime_nsec != self.initial.st_mtime_nsec
        {
            return Err(ResumeMarkerErrorV1::InvalidMarker {
                path: display_path.join(name),
                reason: "protected cleanup entry changed while pinned".into(),
            });
        }
        Ok(())
    }

    fn snapshot(&self) -> Result<ProtectedCleanupLocalFileSnapshotV1, ResumeMarkerErrorV1> {
        ProtectedCleanupLocalFileSnapshotV1::from_stat(&self.initial)
            .map_err(|_| ResumeMarkerErrorV1::InvalidTransition)
    }

    fn require_snapshot(
        &self,
        expected: ProtectedCleanupLocalFileSnapshotV1,
        name: &str,
        display_path: &Path,
    ) -> Result<(), ResumeMarkerErrorV1> {
        let current = fstat(&self.file).map_err(std::io::Error::from)?;
        if !expected.matches(&current) {
            return Err(ResumeMarkerErrorV1::InvalidMarker {
                path: display_path.join(name),
                reason: "protected cleanup entry inode differs from durable evidence".into(),
            });
        }
        Ok(())
    }

    fn read_bounded(
        &self,
        directory: &OwnedFd,
        name: &str,
        display_path: &Path,
        maximum: usize,
    ) -> Result<Vec<u8>, ResumeMarkerErrorV1> {
        self.require_named_as(directory, name, display_path)?;
        let length = usize::try_from(self.initial.st_size).ok();
        if length.is_none_or(|length| length == 0 || length > maximum) {
            return Err(ResumeMarkerErrorV1::InvalidMarker {
                path: display_path.join(name),
                reason: "protected cleanup entry exceeds its canonical bound".into(),
            });
        }
        let mut bytes = vec![0; length.unwrap_or(0)];
        self.file.read_exact_at(&mut bytes, 0)?;
        self.require_named_as(directory, name, display_path)?;
        Ok(bytes)
    }

    fn rename_and_sync(
        &self,
        directory: &OwnedFd,
        source: &str,
        destination: &str,
        display_path: &Path,
        faults: ProtectedCleanupRenameFaultsV1,
    ) -> Result<(), ResumeMarkerErrorV1> {
        self.require_named_as(directory, source, display_path)?;
        match statat(directory, destination, AtFlags::SYMLINK_NOFOLLOW) {
            Err(rustix::io::Errno::NOENT) => {}
            Ok(_) => {
                return Err(ResumeMarkerErrorV1::InvalidMarker {
                    path: display_path.join(destination),
                    reason: "protected cleanup quarantine destination already exists".into(),
                });
            }
            Err(error) => return Err(std::io::Error::from(error).into()),
        }
        #[cfg(feature = "worker-v2-fault-injection-test-only")]
        injected_fault_point_v1(faults._before_rename);
        renameat_with(
            directory,
            source,
            directory,
            destination,
            RenameFlags::NOREPLACE,
        )
        .map_err(std::io::Error::from)?;
        #[cfg(feature = "worker-v2-fault-injection-test-only")]
        injected_fault_point_v1(faults._after_rename);
        self.require_named_as(directory, destination, display_path)?;
        fsync(directory).map_err(std::io::Error::from)?;
        #[cfg(feature = "worker-v2-fault-injection-test-only")]
        injected_fault_point_v1(faults._after_sync);
        #[cfg(not(feature = "worker-v2-fault-injection-test-only"))]
        let _ = faults;
        self.require_named_as(directory, destination, display_path)
    }

    fn unlink_and_sync(
        &self,
        directory: &OwnedFd,
        name: &str,
        display_path: &Path,
        _before_unlink: &'static str,
        _after_unlink: &'static str,
        _after_sync: &'static str,
    ) -> Result<(), ResumeMarkerErrorV1> {
        self.require_named_as(directory, name, display_path)?;
        #[cfg(feature = "worker-v2-fault-injection-test-only")]
        injected_fault_point_v1(_before_unlink);
        #[cfg(test)]
        run_protected_cleanup_before_unlink_test_hook_v1(name);
        // The fault/test boundary deliberately runs before the final inode check. Cooperating
        // writers hold the package lock, and an out-of-contract replacement is rejected before
        // the pathname is unlinked.
        self.require_named_as(directory, name, display_path)?;
        unlinkat(directory, name, AtFlags::empty()).map_err(std::io::Error::from)?;
        #[cfg(feature = "worker-v2-fault-injection-test-only")]
        injected_fault_point_v1(_after_unlink);
        let unlinked = fstat(&self.file).map_err(std::io::Error::from)?;
        if unlinked.st_dev != self.initial.st_dev
            || unlinked.st_ino != self.initial.st_ino
            || unlinked.st_mode != self.initial.st_mode
            || unlinked.st_size != self.initial.st_size
            || unlinked.st_nlink != 0
            || !entry_is_absent_v1(directory, name)?
        {
            return Err(ResumeMarkerErrorV1::InvalidMarker {
                path: display_path.join(name),
                reason: "protected cleanup unlink did not remove the pinned exact inode".into(),
            });
        }
        fsync(directory).map_err(std::io::Error::from)?;
        #[cfg(feature = "worker-v2-fault-injection-test-only")]
        injected_fault_point_v1(_after_sync);
        if fstat(&self.file).map_err(std::io::Error::from)?.st_nlink != 0
            || !entry_is_absent_v1(directory, name)?
        {
            return Err(ResumeMarkerErrorV1::InvalidMarker {
                path: display_path.join(name),
                reason: "protected cleanup unlink was not durably stable".into(),
            });
        }
        Ok(())
    }
}

fn entry_is_absent_v1(directory: &OwnedFd, name: &str) -> Result<bool, ResumeMarkerErrorV1> {
    match statat(directory, name, AtFlags::SYMLINK_NOFOLLOW) {
        Err(rustix::io::Errno::NOENT) => Ok(true),
        Ok(_) => Ok(false),
        Err(error) => Err(std::io::Error::from(error).into()),
    }
}

trait MarkerIntentIdentity: Copy + Eq {
    fn from_marker_bytes(bytes: [u8; 32]) -> Self;
    fn marker_bytes(self) -> [u8; 32];
}

trait MarkerReceiptRecord: Copy + Eq {
    fn empty() -> Self;
    fn encode(self, bytes: &mut Vec<u8>);
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, &'static str>;
    fn is_zero(self) -> bool;
    fn is_complete(self) -> bool;
}

impl MarkerReceiptRecord for ReceiptRecordV1 {
    fn empty() -> Self {
        Self([[0; 32]; RECEIPT_FIELDS])
    }

    fn encode(self, bytes: &mut Vec<u8>) {
        Self::encode(self, bytes);
    }

    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, &'static str> {
        Self::decode(decoder)
    }

    fn is_zero(self) -> bool {
        Self::is_zero(self)
    }

    fn is_complete(self) -> bool {
        !Self::is_zero(self)
    }
}

impl MarkerReceiptRecord for ReceiptRecordV2 {
    fn empty() -> Self {
        Self {
            fields: [[0; 32]; RECEIPT_FIELDS],
            compiler_closure: None,
        }
    }

    fn encode(self, bytes: &mut Vec<u8>) {
        Self::encode(self, bytes);
    }

    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, &'static str> {
        Self::decode(decoder)
    }

    fn is_zero(self) -> bool {
        Self::is_zero(self)
    }

    fn is_complete(self) -> bool {
        self.compiler_closure.is_some() && self.fields.iter().all(|field| *field != [0; 32])
    }
}

impl MarkerIntentIdentity for WorkerV2PublicationIntentIdentityV1 {
    fn from_marker_bytes(bytes: [u8; 32]) -> Self {
        Self::from_bytes(bytes)
    }

    fn marker_bytes(self) -> [u8; 32] {
        self.as_bytes()
    }
}

impl MarkerIntentIdentity for WorkerV2PublicationIntentIdentityV2 {
    fn from_marker_bytes(bytes: [u8; 32]) -> Self {
        Self::from_bytes(bytes)
    }

    fn marker_bytes(self) -> [u8; 32] {
        self.as_bytes()
    }
}

trait CanonicalMarkerSchema {
    type Intent: MarkerIntentIdentity;
    type Receipt: MarkerReceiptRecord;

    const VERSION: u16;
    const DISCRIMINATOR: &'static [u8];
    const ENCODED_BYTES: usize;
    const CHECKSUM_DOMAIN: &'static [u8];
    const SUPPORTS_ENVELOPE: bool;
}

struct OrdinaryMarkerSchemaV1;

impl CanonicalMarkerSchema for OrdinaryMarkerSchemaV1 {
    type Intent = WorkerV2PublicationIntentIdentityV1;
    type Receipt = ReceiptRecordV1;

    const VERSION: u16 = MARKER_VERSION;
    const DISCRIMINATOR: &'static [u8] = &[];
    const ENCODED_BYTES: usize = MARKER_BYTES;
    const CHECKSUM_DOMAIN: &'static [u8] = MARKER_CHECKSUM_DOMAIN;
    const SUPPORTS_ENVELOPE: bool = true;
}

struct ProtectedMarkerSchemaV2;

impl CanonicalMarkerSchema for ProtectedMarkerSchemaV2 {
    type Intent = WorkerV2PublicationIntentIdentityV2;
    type Receipt = ReceiptRecordV2;

    const VERSION: u16 = PROTECTED_MARKER_VERSION;
    const DISCRIMINATOR: &'static [u8] = &[PROTECTED_INTENT_SCHEMA_V2];
    const ENCODED_BYTES: usize = PROTECTED_MARKER_BYTES;
    const CHECKSUM_DOMAIN: &'static [u8] = PROTECTED_MARKER_CHECKSUM_DOMAIN_V5;
    const SUPPORTS_ENVELOPE: bool = true;
}

#[cfg(test)]
struct ObsoleteProtectedMarkerSchemaV4;

#[cfg(test)]
impl CanonicalMarkerSchema for ObsoleteProtectedMarkerSchemaV4 {
    type Intent = WorkerV2PublicationIntentIdentityV2;
    type Receipt = ReceiptRecordV1;

    const VERSION: u16 = OBSOLETE_PROTECTED_MARKER_VERSION_V4;
    const DISCRIMINATOR: &'static [u8] = &[PROTECTED_INTENT_SCHEMA_V2];
    const ENCODED_BYTES: usize = OBSOLETE_PROTECTED_MARKER_BYTES_V4;
    const CHECKSUM_DOMAIN: &'static [u8] = MARKER_CHECKSUM_DOMAIN;
    const SUPPORTS_ENVELOPE: bool = true;
}

#[allow(clippy::large_enum_variant)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CanonicalMarkerState<I, R> {
    Pending {
        publication: WorkerV2PublicationKindV1,
        attempt: BuildAttempt,
        admission: [u8; 32],
        envelope_inputs: [u8; 32],
    },
    Ready {
        publication: WorkerV2PublicationKindV1,
        attempt: BuildAttempt,
        admission: [u8; 32],
        envelope_inputs: [u8; 32],
        intent: I,
    },
    Completed {
        publication: WorkerV2PublicationKindV1,
        attempt: BuildAttempt,
        admission: [u8; 32],
        envelope_inputs: [u8; 32],
        envelope: [u8; 32],
        intent: I,
        receipt: R,
    },
}

impl<I: Copy, R: Copy> CanonicalMarkerState<I, R> {
    const fn attempt(self) -> BuildAttempt {
        match self {
            Self::Pending { attempt, .. }
            | Self::Ready { attempt, .. }
            | Self::Completed { attempt, .. } => attempt,
        }
    }

    const fn publication(self) -> WorkerV2PublicationKindV1 {
        match self {
            Self::Pending { publication, .. }
            | Self::Ready { publication, .. }
            | Self::Completed { publication, .. } => publication,
        }
    }

    const fn admission(self) -> [u8; 32] {
        match self {
            Self::Pending { admission, .. }
            | Self::Ready { admission, .. }
            | Self::Completed { admission, .. } => admission,
        }
    }

    const fn envelope_inputs(self) -> [u8; 32] {
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

    const fn envelope(self) -> [u8; 32] {
        match self {
            Self::Pending { .. } | Self::Ready { .. } => [0; 32],
            Self::Completed { envelope, .. } => envelope,
        }
    }
}

impl ResumeMarkerStateV1 {
    fn into_canonical(
        self,
    ) -> Result<CanonicalMarkerState<WorkerV2PublicationIntentIdentityV1, ReceiptRecordV1>, ()>
    {
        match self {
            Self::Pending {
                legacy: false,
                publication,
                attempt,
                admission,
                envelope_inputs,
            } => Ok(CanonicalMarkerState::Pending {
                publication,
                attempt,
                admission,
                envelope_inputs,
            }),
            Self::Ready {
                legacy: false,
                publication,
                attempt,
                admission,
                envelope_inputs,
                intent,
            } => Ok(CanonicalMarkerState::Ready {
                publication,
                attempt,
                admission,
                envelope_inputs,
                intent,
            }),
            Self::Completed {
                legacy: false,
                publication,
                attempt,
                admission,
                envelope_inputs,
                envelope,
                intent,
                receipt,
            } => Ok(CanonicalMarkerState::Completed {
                publication,
                attempt,
                admission,
                envelope_inputs,
                envelope,
                intent,
                receipt,
            }),
            _ => Err(()),
        }
    }

    fn from_canonical(
        state: CanonicalMarkerState<WorkerV2PublicationIntentIdentityV1, ReceiptRecordV1>,
    ) -> Self {
        match state {
            CanonicalMarkerState::Pending {
                publication,
                attempt,
                admission,
                envelope_inputs,
            } => Self::Pending {
                legacy: false,
                publication,
                attempt,
                admission,
                envelope_inputs,
            },
            CanonicalMarkerState::Ready {
                publication,
                attempt,
                admission,
                envelope_inputs,
                intent,
            } => Self::Ready {
                legacy: false,
                publication,
                attempt,
                admission,
                envelope_inputs,
                intent,
            },
            CanonicalMarkerState::Completed {
                publication,
                attempt,
                admission,
                envelope_inputs,
                envelope,
                intent,
                receipt,
            } => Self::Completed {
                legacy: false,
                publication,
                attempt,
                admission,
                envelope_inputs,
                envelope,
                intent,
                receipt,
            },
        }
    }
}

impl From<ResumeMarkerStateV2>
    for CanonicalMarkerState<WorkerV2PublicationIntentIdentityV2, ReceiptRecordV2>
{
    fn from(state: ResumeMarkerStateV2) -> Self {
        match state {
            ResumeMarkerStateV2::Pending {
                publication,
                attempt,
                admission,
                envelope_inputs,
            } => Self::Pending {
                publication,
                attempt,
                admission,
                envelope_inputs,
            },
            ResumeMarkerStateV2::Ready {
                publication,
                attempt,
                admission,
                envelope_inputs,
                intent,
            } => Self::Ready {
                publication,
                attempt,
                admission,
                envelope_inputs,
                intent,
            },
            ResumeMarkerStateV2::Completed {
                publication,
                attempt,
                admission,
                envelope_inputs,
                envelope,
                intent,
                receipt,
            } => Self::Completed {
                publication,
                attempt,
                admission,
                envelope_inputs,
                envelope,
                intent,
                receipt,
            },
        }
    }
}

impl From<CanonicalMarkerState<WorkerV2PublicationIntentIdentityV2, ReceiptRecordV2>>
    for ResumeMarkerStateV2
{
    fn from(
        state: CanonicalMarkerState<WorkerV2PublicationIntentIdentityV2, ReceiptRecordV2>,
    ) -> Self {
        match state {
            CanonicalMarkerState::Pending {
                publication,
                attempt,
                admission,
                envelope_inputs,
            } => Self::Pending {
                publication,
                attempt,
                admission,
                envelope_inputs,
            },
            CanonicalMarkerState::Ready {
                publication,
                attempt,
                admission,
                envelope_inputs,
                intent,
            } => Self::Ready {
                publication,
                attempt,
                admission,
                envelope_inputs,
                intent,
            },
            CanonicalMarkerState::Completed {
                publication,
                attempt,
                admission,
                envelope_inputs,
                envelope,
                intent,
                receipt,
            } => Self::Completed {
                publication,
                attempt,
                admission,
                envelope_inputs,
                envelope,
                intent,
                receipt,
            },
        }
    }
}

fn encode_marker(package: [u8; 32], state: ResumeMarkerStateV1) -> Vec<u8> {
    let canonical = state
        .into_canonical()
        .expect("legacy marker states cannot be encoded canonically");
    encode_canonical_marker::<OrdinaryMarkerSchemaV1>(package, canonical)
}

fn encode_protected_marker(package: [u8; 32], state: ResumeMarkerStateV2) -> Vec<u8> {
    encode_canonical_marker::<ProtectedMarkerSchemaV2>(package, state.into())
}

fn encode_canonical_marker<S: CanonicalMarkerSchema>(
    package: [u8; 32],
    state: CanonicalMarkerState<S::Intent, S::Receipt>,
) -> Vec<u8> {
    let attempt = state.attempt();
    let publication = state.publication();
    let admission = state.admission();
    let envelope_inputs = state.envelope_inputs();
    let envelope = state.envelope();
    let (stage, intent, receipt) = match state {
        CanonicalMarkerState::Pending { .. } => (1, [0; 32], S::Receipt::empty()),
        CanonicalMarkerState::Ready { intent, .. } => {
            (2, intent.marker_bytes(), S::Receipt::empty())
        }
        CanonicalMarkerState::Completed {
            intent, receipt, ..
        } => (3, intent.marker_bytes(), receipt),
    };
    let mut bytes = Vec::with_capacity(S::ENCODED_BYTES);
    bytes.extend_from_slice(MARKER_MAGIC);
    bytes.extend_from_slice(&S::VERSION.to_le_bytes());
    bytes.extend_from_slice(S::DISCRIMINATOR);
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
    let checksum = checksum_for(S::CHECKSUM_DOMAIN, &bytes);
    bytes.extend_from_slice(&checksum);
    debug_assert_eq!(bytes.len(), S::ENCODED_BYTES);
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
        (MARKER_VERSION, MARKER_BYTES) => {
            decode_canonical_marker_body::<OrdinaryMarkerSchemaV1>(&mut decoder, expected_package)
                .map(ResumeMarkerStateV1::from_canonical)
        }
        (PREVIOUS_MARKER_VERSION, PREVIOUS_MARKER_BYTES) => {
            decode_previous_marker_v2(&mut decoder, expected_package)
        }
        (LEGACY_MARKER_VERSION, LEGACY_MARKER_BYTES) => {
            decode_legacy_raw_marker_v1(&mut decoder, expected_package)
        }
        _ => Err("unsupported marker version or noncanonical version length"),
    }
}

fn decode_protected_marker(
    bytes: &[u8],
    expected_package: [u8; 32],
) -> Result<ResumeMarkerStateV2, &'static str> {
    decode_canonical_marker::<ProtectedMarkerSchemaV2>(bytes, expected_package)
        .map(ResumeMarkerStateV2::from)
}

fn decode_canonical_marker<S: CanonicalMarkerSchema>(
    bytes: &[u8],
    expected_package: [u8; 32],
) -> Result<CanonicalMarkerState<S::Intent, S::Receipt>, &'static str> {
    if bytes.len() != S::ENCODED_BYTES {
        return Err("marker has a noncanonical schema length");
    }
    let (body, encoded_checksum) = bytes.split_at(bytes.len() - 32);
    if checksum_for(S::CHECKSUM_DOMAIN, body).as_slice() != encoded_checksum {
        return Err("marker checksum mismatch");
    }
    let mut decoder = Decoder::new(body);
    if decoder.take(MARKER_MAGIC.len())? != MARKER_MAGIC {
        return Err("marker magic mismatch");
    }
    if decoder.u16()? != S::VERSION {
        return Err("unsupported marker schema version");
    }
    decode_canonical_marker_body::<S>(&mut decoder, expected_package)
}

fn decode_canonical_marker_body<S: CanonicalMarkerSchema>(
    decoder: &mut Decoder<'_>,
    expected_package: [u8; 32],
) -> Result<CanonicalMarkerState<S::Intent, S::Receipt>, &'static str> {
    if decoder.take(S::DISCRIMINATOR.len())? != S::DISCRIMINATOR {
        return Err("marker intent schema mismatch");
    }
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
    let intent = S::Intent::from_marker_bytes(decoder.array()?);
    let receipt = S::Receipt::decode(decoder)?;
    if !decoder.finished() {
        return Err("marker has trailing body bytes");
    }
    let required = publication.requires_envelope();
    if required && !S::SUPPORTS_ENVELOPE {
        return Err("marker schema has no protected envelope receipt");
    }
    let input_fields_valid = required == (envelope_inputs != [0; 32]);
    match stage {
        1 if admission != [0; 32]
            && input_fields_valid
            && envelope == [0; 32]
            && intent.marker_bytes() == [0; 32]
            && receipt.is_zero() =>
        {
            Ok(CanonicalMarkerState::Pending {
                publication,
                attempt,
                admission,
                envelope_inputs,
            })
        }
        2 if admission != [0; 32]
            && input_fields_valid
            && envelope == [0; 32]
            && intent.marker_bytes() != [0; 32]
            && receipt.is_zero() =>
        {
            Ok(CanonicalMarkerState::Ready {
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
            && intent.marker_bytes() != [0; 32]
            && receipt.is_complete() =>
        {
            Ok(CanonicalMarkerState::Completed {
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
    checksum_for(MARKER_CHECKSUM_DOMAIN, bytes)
}

fn checksum_for(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
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
        publish_exact_hsaco_evidence_for_attempt_v1, publish_exact_hsaco_evidence_for_attempt_v2,
    };
    use fe2o3_artifacts::{
        AbiField, AbiKind, AbiLayout, Access, AddressSpace, AliasClass, ArgumentOwnership,
        ArtifactContainerV1, BlockSize, BundleIndexV1, CallerClaimedPackageIdentityV1,
        CodeObjectFormat, CodeObjectIdentity, CodeObjectPayload, CompilerIdentity,
        DeclaredRustLayoutIdentity, DeclaredRustTypeIdentity, DigestAlgorithm, DigestBytes,
        Dimensions, DirectLinkBindingExpectationV1, DirectLinkBindingSourceV1,
        DirectLinkBundleEvidenceV1, DirectLinkFfiClosureIdentityV1,
        DirectLinkFinalizationIdentityV1, DirectLinkFinalizedPayloadIdentityV1,
        DirectLinkLinkedOutputIdentityV1, DirectLinkRequestIdentityV1,
        DirectLinkResponseIdentityV1, DirectLinkToolchainConfigurationIdentityV1,
        DirectLinkToolchainExecutableIdentityV1, DirectLinkToolchainIdentityV1,
        DirectLinkTransformationIdentityV1, DirectLinkWorkerConfigurationIdentityV1,
        DirectLinkWorkerExecutableIdentityV1, DirectLinkWorkerIdentityV1, Endianness, IdentityText,
        KernelEntry, LaunchContract, ManifestClaimDerivedLinkPublicationScopeV1,
        ManifestClaimDirectLinkPublicationBridgeV1, ManifestV1, MeasuredToolIdentity, Mutability,
        Name, PayloadDigest, PointerWidth, ProofArtifactIdentity, ProofExecutionIdentity,
        ProofOutcome, ProofProperty, ProofRecordV1, ProofTargetIdentity, ScalarType,
        SourceContractIdentity, TargetIdentity as ArtifactTargetIdentity, ToolIdentity,
        TypeIdentity, VerificationModelIdentity,
    };
    use fe2o3_kernel_descriptor::{
        BlockSizeV1, BuildEvidenceV1, CanonicalCodeObjectDigest, CompilerIdentityV1,
        DeviceDescriptorTableV1, DeviceLayoutDescriptorV1, DeviceLayoutRecordV1, DeviceTargetV1,
        DimensionsV1, EvidenceDigest, EvidenceIdentity, KernelAbiLayoutV1, KernelDescriptorV1,
        KernelId, LaunchConstraintsV1, LogicalArgumentV1, ProducerIdentityV1, ScalarTypeV1,
        SourceTypeDescriptorV1, SourceTypeRecordV1, Text, ValidName,
    };
    use fe2o3_worker_v2_bundle::{
        DescriptorLineageV1, ExactRawHsacoV1, WorkerV2FinalArtifactEvidenceV2,
        WorkerV2ProducerBindingV2,
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

    #[test]
    fn cleanup_evidence_retirement_requires_strictly_greater_generation() {
        let build_attempt = |generation: u64, seed: u8| {
            BuildAttempt::from_env_value(&format!(
                "{generation}:{}:{}",
                BuildSession::from_bytes([seed; 16]).to_hex(),
                BuildInvocation::from_bytes([seed.wrapping_add(1); 32]).to_hex(),
            ))
            .unwrap()
        };
        let predecessor = build_attempt(7, 1);
        let lower = build_attempt(6, 2);
        let equal_but_different = build_attempt(7, 3);
        let newer = build_attempt(8, 4);

        assert!(!strictly_newer_attempt_generation_v1(lower, predecessor));
        assert!(!strictly_newer_attempt_generation_v1(
            equal_but_different,
            predecessor
        ));
        assert!(strictly_newer_attempt_generation_v1(newer, predecessor));
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

    fn compiler_closure(seed: u8) -> CompilerClosureV2 {
        CompilerClosureV2::new(
            [seed; 32],
            [seed.wrapping_add(1); 32],
            [seed.wrapping_add(2); 32],
            [seed.wrapping_add(3); 32],
            [seed.wrapping_add(4); 32],
            [seed.wrapping_add(5); 32],
        )
        .unwrap()
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

    fn mutated_compiler_closure(seed: u8, role: usize) -> CompilerClosureV2 {
        let mut pins = [
            [seed; 32],
            [seed.wrapping_add(1); 32],
            [seed.wrapping_add(2); 32],
            [seed.wrapping_add(3); 32],
            [seed.wrapping_add(4); 32],
            [seed.wrapping_add(5); 32],
        ];
        pins[role][0] ^= 0x80;
        CompilerClosureV2::new(pins[0], pins[1], pins[2], pins[3], pins[4], pins[5]).unwrap()
    }

    struct ProtectedEnvelopeFixture {
        envelope: WorkerV2LoadEnvelopeV2,
        envelope_inputs: WorkerV2EnvelopeInputsV1,
        intent: WorkerV2PublicationIntentIdentityV2,
        receipt: BackendPublicationReceiptV2,
        plan: DurableLinkPublicationPlanV1,
        upstream: UpstreamCodeObjectEvidenceIdentityV1,
        output: Vec<u8>,
    }

    fn fixture_digest(seed: u8) -> DigestBytes {
        DigestBytes::from_bytes([seed; 32])
    }

    fn fixture_tagged(seed: u8) -> PayloadDigest {
        PayloadDigest::new(DigestAlgorithm::Sha256, fixture_digest(seed))
    }

    fn fixture_text(value: &str) -> IdentityText {
        IdentityText::new(value).unwrap()
    }

    fn fixture_name(value: &str) -> Name {
        Name::new(value).unwrap()
    }

    fn fixture_manifest_abi() -> AbiLayout {
        let source_type =
            SourceTypeRecordV1::new(SourceTypeDescriptorV1::scalar(ScalarTypeV1::F32));
        let layout = DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::scalar(ScalarTypeV1::F32));
        AbiLayout::new(
            4,
            4,
            PointerWidth::Bits64,
            vec![
                AbiField::new(
                    fixture_name("value"),
                    0,
                    4,
                    4,
                    AbiKind::Scalar(ScalarType::F32),
                    Mutability::Immutable,
                    Access::ByValue,
                    AddressSpace::Value,
                    TypeIdentity::new(
                        DeclaredRustTypeIdentity::from_untrusted_bytes(DigestBytes::from_bytes(
                            *source_type.identity().as_bytes(),
                        )),
                        DeclaredRustLayoutIdentity::from_untrusted_bytes(DigestBytes::from_bytes(
                            *layout.identity().as_bytes(),
                        )),
                    ),
                    ArgumentOwnership::ByValue,
                    AliasClass::Value,
                )
                .unwrap(),
            ],
        )
        .unwrap()
    }

    fn fixture_manifest_launch() -> LaunchContract {
        LaunchContract::new(
            1,
            BlockSize::Exact(Dimensions::new(256, 1, 1).unwrap()),
            Dimensions::new(65_535, 1, 1).unwrap(),
            0,
            0,
        )
        .unwrap()
    }

    fn fixture_descriptor_lineage(finalized_hsaco: &[u8]) -> DescriptorLineageV1 {
        let source_type =
            SourceTypeRecordV1::new(SourceTypeDescriptorV1::scalar(ScalarTypeV1::F32));
        let layout = DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::scalar(ScalarTypeV1::F32));
        let kernel = KernelDescriptorV1::new(
            KernelId::from_bytes([0x21; 32]),
            ValidName::new("alpha").unwrap(),
            ValidName::new("alpha").unwrap(),
            ValidName::new("alpha.kd").unwrap(),
            BuildEvidenceV1::new(
                EvidenceIdentity::from_opaque_bytes([0x71; 32]),
                EvidenceDigest::from_sha256_bytes([0x31; 32]),
            ),
            BuildEvidenceV1::new(
                EvidenceIdentity::from_opaque_bytes([0x72; 32]),
                EvidenceDigest::from_sha256_bytes([0x41; 32]),
            ),
            vec![],
            KernelAbiLayoutV1::new(4, 4, 4).unwrap(),
            LaunchConstraintsV1::new(
                1,
                BlockSizeV1::Exact(DimensionsV1::new(256, 1, 1).unwrap()),
                DimensionsV1::new(65_535, 1, 1).unwrap(),
                256,
                0,
                0,
            )
            .unwrap(),
            vec![
                LogicalArgumentV1::scalar(
                    0,
                    ValidName::new("value").unwrap(),
                    &source_type,
                    &layout,
                    0,
                )
                .unwrap(),
            ],
        )
        .unwrap();
        DescriptorLineageV1::new(
            DeviceDescriptorTableV1::new(
                CanonicalCodeObjectDigest::calculate_from_canonicalized_hsaco(finalized_hsaco),
                fe2o3_kernel_descriptor::CodeObjectVersion::V6,
                CompilerIdentityV1::new(
                    Text::new("rustc").unwrap(),
                    Text::new("1.94.0-nightly").unwrap(),
                    [0x11; 20],
                ),
                ProducerIdentityV1::new(
                    Text::new("cargo-fe2o3").unwrap(),
                    Text::new("0.1.0").unwrap(),
                ),
                DeviceTargetV1::parse("gfx942:xnack-").unwrap(),
                vec![source_type],
                vec![layout],
                vec![kernel],
            )
            .unwrap(),
        )
    }

    fn fixture_proof() -> ProofRecordV1 {
        let artifact = ProofArtifactIdentity::new(
            fixture_tagged(0x21),
            fixture_tagged(0x51),
            fixture_tagged(0x31),
            fixture_tagged(0x53),
            fixture_tagged(0x41),
            fixture_tagged(0x55),
            fixture_tagged(0x56),
            fixture_tagged(0x57),
        );
        let contracts = SourceContractIdentity::new(
            fixture_tagged(0x61),
            fixture_tagged(0x62),
            fixture_tagged(0x63),
            fixture_tagged(0x64),
            fixture_tagged(0x65),
        );
        let tool = |name: &str, seed: u8| {
            MeasuredToolIdentity::new(
                fixture_text(name),
                fixture_text("1.0"),
                fixture_tagged(seed),
                fixture_tagged(seed.wrapping_add(1)),
            )
        };
        ProofRecordV1::new(
            ProofTargetIdentity::new(artifact, contracts),
            vec![],
            ProofExecutionIdentity::new(
                VerificationModelIdentity::new(
                    fixture_text("verus-model-v1"),
                    fixture_tagged(0x70),
                ),
                tool("verus", 0x71),
                tool("z3", 0x73),
                tool("recorder", 0x75),
                fixture_tagged(0x77),
            ),
            ProofOutcome::Proved,
            vec![ProofProperty::Bounds],
            vec![],
        )
        .unwrap()
    }

    fn protected_envelope_fixture(
        path: &Path,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        closure: CompilerClosureV2,
    ) -> ProtectedEnvelopeFixture {
        protected_envelope_fixture_with_binding_seed(
            path,
            producer,
            attempt,
            closure,
            attempt.invocation().as_bytes()[0],
        )
    }

    fn protected_envelope_fixture_with_binding_seed(
        path: &Path,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        closure: CompilerClosureV2,
        binding_seed: u8,
    ) -> ProtectedEnvelopeFixture {
        let output = vec![0xf1; 64];
        let raw_bytes = vec![0x71; 64];
        let payload =
            CodeObjectPayload::from_bytes(DigestAlgorithm::Sha256, output.clone()).unwrap();
        let finalized_identity = payload.digest();
        let manifest = ManifestV1::new(
            CompilerIdentity::new(fixture_text("rustc"), fixture_text("1.94.0-nightly")),
            ToolIdentity::new(fixture_text("cargo-fe2o3"), fixture_text("0.1.0")),
            ArtifactTargetIdentity::new(
                fixture_text("amdgcn-amd-amdhsa"),
                fixture_text("gfx942:xnack-"),
                PointerWidth::Bits64,
                Endianness::Little,
                vec![],
            )
            .unwrap(),
            vec![
                CodeObjectIdentity::new(
                    finalized_identity.bytes(),
                    CodeObjectFormat::NativeExecutable,
                    payload.bytes().len() as u64,
                )
                .unwrap(),
            ],
            vec![
                KernelEntry::new(
                    fixture_digest(0x21),
                    fixture_name("alpha"),
                    fixture_name("alpha"),
                    fixture_digest(0x31),
                    fixture_digest(0x41),
                    finalized_identity.bytes(),
                    vec![],
                    fixture_manifest_launch(),
                    fixture_manifest_abi(),
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let container =
            ArtifactContainerV1::new(manifest, DigestAlgorithm::Sha256, vec![payload]).unwrap();
        let bundle = BundleIndexV1::from_containers(std::slice::from_ref(&container)).unwrap();
        let raw = ExactRawHsacoV1::from_bytes(raw_bytes).unwrap();
        let expectation = DirectLinkBindingExpectationV1::new(
            DirectLinkRequestIdentityV1::new(fixture_tagged(0x81)),
            DirectLinkWorkerIdentityV1::new(
                fixture_text("fe2o3-worker"),
                fixture_text("v2"),
                DirectLinkWorkerExecutableIdentityV1::new(fixture_tagged(0x82)),
                DirectLinkWorkerConfigurationIdentityV1::new(fixture_tagged(0x83)),
            ),
            DirectLinkToolchainIdentityV1::new(
                fixture_text("llvm"),
                fixture_text("22"),
                DirectLinkToolchainExecutableIdentityV1::new(fixture_tagged(0x84)),
                DirectLinkToolchainConfigurationIdentityV1::new(fixture_tagged(0x85)),
            ),
            DirectLinkResponseIdentityV1::new(fixture_tagged(0x86)),
            DirectLinkTransformationIdentityV1::new(
                DirectLinkLinkedOutputIdentityV1::new(raw.identity()),
                DirectLinkFinalizationIdentityV1::new(fixture_tagged(0x87)),
                DirectLinkFinalizedPayloadIdentityV1::new(finalized_identity),
            ),
            DirectLinkFfiClosureIdentityV1::new(fixture_tagged(0x88)),
        );
        let evidence = DirectLinkBundleEvidenceV1::bind(
            &bundle,
            &[&container],
            &[DirectLinkBindingSourceV1::new(
                &container,
                expectation.clone(),
            )],
        )
        .unwrap();
        let validated = evidence
            .validate_against(
                &bundle,
                &[&container],
                &[DirectLinkBindingSourceV1::new(&container, expectation)],
            )
            .unwrap();
        let package =
            PackageIdentityV1::from_bytes(*producer_package_identity_v1(producer).as_bytes());
        let scope = ManifestClaimDerivedLinkPublicationScopeV1::derive(
            CallerClaimedPackageIdentityV1::new(package),
            &validated,
            0,
            &container,
        )
        .unwrap();
        let bridge = ManifestClaimDirectLinkPublicationBridgeV1::prepare_with_manifest_claim_scope(
            attempt, scope, &validated, 0,
        )
        .unwrap();
        let plan = DurableLinkPublicationPlanV1::new(
            attempt,
            bridge
                .non_authoritative_diagnostics()
                .descriptive_scope_claim(),
            bridge.request_identity(),
            bridge.worker_identity(),
            bridge.response_identity(),
            bridge.linked_output_identity(),
            bridge.finalization_identity(),
            bridge.finalized_output_identity(),
            bridge.publication_identity(),
        );
        let upstream = UpstreamCodeObjectEvidenceIdentityV1::from_bytes(
            Sha256::digest(evidence.to_bytes()).into(),
        );
        let durable_intent = persist_worker_v2_publication_intent_v2(
            path, producer, attempt, plan, upstream, closure, &output,
        )
        .unwrap();
        let intent_record = durable_intent.record();
        let intent = intent_record.identity();
        drop(durable_intent);
        let publication = publish_exact_hsaco_evidence_for_attempt_v2(
            path, producer, attempt, plan, upstream, closure, &output,
        )
        .unwrap();
        let receipt = publication.receipt();
        let claim = publication.published_claim().clone();
        drop(publication);
        let descriptor = fixture_descriptor_lineage(&output);
        let proofs = vec![fixture_proof()];
        let seed = binding_seed;
        let source = format!("/src/resume-{seed}.rs");
        let producer_binding = WorkerV2ProducerBindingV2::from_codegen(
            &format!("resume_{seed}"),
            Some(Path::new(&source)),
        )
        .unwrap();
        let final_artifact = WorkerV2FinalArtifactEvidenceV2::from_proof_records(
            &container,
            &descriptor,
            &proofs,
            closure,
            intent_record,
            producer_binding,
            receipt,
            claim,
        )
        .unwrap();
        let envelope_inputs =
            WorkerV2EnvelopeInputsV1::new(evidence.clone(), proofs.clone(), raw.clone()).unwrap();
        let envelope = WorkerV2LoadEnvelopeV2::new(
            container,
            bundle,
            evidence,
            descriptor,
            proofs,
            raw,
            final_artifact,
        )
        .unwrap();
        ProtectedEnvelopeFixture {
            envelope,
            envelope_inputs,
            intent,
            receipt,
            plan,
            upstream,
            output,
        }
    }

    fn stage_protected_required_ready(
        store: &WorkerV2ResumeStoreV2,
        fixture: &ProtectedEnvelopeFixture,
        closure: CompilerClosureV2,
    ) {
        let publication = WorkerV2PublicationKindV1::FinalizedEnvelopeRequired;
        let attempt = fixture.plan.attempt();
        store
            .persist_envelope_inputs(attempt, &fixture.envelope_inputs)
            .unwrap();
        store
            .persist_pending_with_envelope_inputs(
                publication,
                attempt,
                restart_admission_commitment_with_inputs_v2(
                    publication,
                    fixture.plan,
                    fixture.upstream,
                    &fixture.output,
                    Some(fixture.envelope_inputs.identity()),
                    closure,
                ),
                Some(fixture.envelope_inputs.identity()),
            )
            .unwrap();
        store
            .persist_ready(
                publication,
                attempt,
                &recover_worker_v2_publication_intent_v2(
                    &store.inner.display_path,
                    &store.producer,
                    attempt,
                    closure,
                )
                .unwrap(),
                closure,
            )
            .unwrap();
    }

    fn protected_v4_marker_bytes(
        package: [u8; 32],
        state: CanonicalMarkerState<WorkerV2PublicationIntentIdentityV2, ReceiptRecordV1>,
    ) -> Vec<u8> {
        encode_canonical_marker::<ObsoleteProtectedMarkerSchemaV4>(package, state)
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

    fn marker_v3_bytes(package: [u8; 32], state: ResumeMarkerStateV1) -> Vec<u8> {
        assert!(!state.is_legacy());
        let attempt = state.attempt();
        let publication = state.publication();
        let admission = state.admission();
        let envelope_inputs = state.envelope_inputs();
        let envelope = state.envelope();
        let (stage, intent, receipt) = match state {
            ResumeMarkerStateV1::Pending { .. } => {
                (1, [0; 32], ReceiptRecordV1([[0; 32]; RECEIPT_FIELDS]))
            }
            ResumeMarkerStateV1::Ready { intent, .. } => (
                2,
                intent.as_bytes(),
                ReceiptRecordV1([[0; 32]; RECEIPT_FIELDS]),
            ),
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
        assert_eq!(bytes.len(), MARKER_BYTES);
        bytes
    }

    fn fixed_attempt() -> BuildAttempt {
        BuildAttempt::from_env_value(&format!(
            "7:{}:{}",
            BuildSession::from_bytes([0x11; 16]).to_hex(),
            BuildInvocation::from_bytes([0x22; 32]).to_hex()
        ))
        .unwrap()
    }

    fn marker_sha256(bytes: &[u8]) -> String {
        hex(&Sha256::digest(bytes))
    }

    fn reseal_marker(bytes: &mut [u8]) {
        let body_len = bytes.len() - 32;
        let digest = checksum(&bytes[..body_len]);
        bytes[body_len..].copy_from_slice(&digest);
    }

    fn reseal_protected_marker_v5(bytes: &mut [u8]) {
        let body_len = bytes.len() - 32;
        let digest = checksum_for(PROTECTED_MARKER_CHECKSUM_DOMAIN_V5, &bytes[..body_len]);
        bytes[body_len..].copy_from_slice(&digest);
    }

    fn install_marker(store: &WorkerV2ResumeStoreV1, bytes: &[u8]) {
        let path = store.display_path.join(&store.marker_name);
        fs::write(&path, bytes).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }

    fn replace_unique_bytes(bytes: &mut [u8], original: &[u8], replacement: &[u8]) {
        assert_eq!(original.len(), replacement.len());
        let offsets = bytes
            .windows(original.len())
            .enumerate()
            .filter_map(|(offset, candidate)| (candidate == original).then_some(offset))
            .collect::<Vec<_>>();
        assert_eq!(offsets.len(), 1, "nested record must occur exactly once");
        bytes[offsets[0]..offsets[0] + replacement.len()].copy_from_slice(replacement);
    }

    fn reseal_claim_v2(bytes: &mut [u8]) {
        const DOMAIN: &[u8] = b"fe2o3.published-hsaco-claim.checksum.v2\0";
        let body_len = bytes.len() - 32;
        let mut digest = Sha256::new();
        digest.update(DOMAIN);
        digest.update(&bytes[..body_len]);
        bytes[body_len..].copy_from_slice(&digest.finalize());
    }

    fn reseal_length_bound_record_v2(bytes: &mut [u8], domain: &[u8]) {
        let body_len = bytes.len() - 32;
        let mut digest = Sha256::new();
        digest.update(domain);
        digest.update((body_len as u64).to_le_bytes());
        digest.update(&bytes[..body_len]);
        bytes[body_len..].copy_from_slice(&digest.finalize());
    }

    fn envelope_with_resealed_claim_substitution(
        envelope: &WorkerV2LoadEnvelopeV2,
        binding_offset: usize,
    ) -> WorkerV2LoadEnvelopeV2 {
        const FINAL_DOMAIN: &[u8] =
            b"FE2O3/PROTECTED-WORKER-V2/FINAL-ARTIFACT-EVIDENCE-CHECKSUM/V2\0";
        const ENVELOPE_DOMAIN: &[u8] = b"FE2O3/PROTECTED-WORKER-V2/LOAD-ENVELOPE-CHECKSUM/V2\0";
        const FILE_BINDING_BYTES: usize = 3 * 16 + 8;

        let original_claim = envelope
            .final_artifact_evidence()
            .published_claim()
            .encode_canonical()
            .unwrap();
        let mut substituted_claim = original_claim.clone();
        let claim_body_len = substituted_claim.len() - 32;
        assert!(binding_offset < FILE_BINDING_BYTES);
        substituted_claim[claim_body_len - FILE_BINDING_BYTES + binding_offset] ^= 1;
        reseal_claim_v2(&mut substituted_claim);
        assert_ne!(substituted_claim, original_claim);
        DurablePublishedHsacoClaimV2::decode_canonical(&substituted_claim).unwrap();

        let original_final = envelope.final_artifact_evidence().to_bytes();
        let mut substituted_final = original_final.clone();
        replace_unique_bytes(&mut substituted_final, &original_claim, &substituted_claim);
        reseal_length_bound_record_v2(&mut substituted_final, FINAL_DOMAIN);

        let mut substituted_envelope = envelope.to_bytes();
        replace_unique_bytes(
            &mut substituted_envelope,
            &original_final,
            &substituted_final,
        );
        reseal_length_bound_record_v2(&mut substituted_envelope, ENVELOPE_DOMAIN);
        WorkerV2LoadEnvelopeV2::from_bytes(&substituted_envelope).unwrap()
    }

    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    const MARKER_CRASH_HELPER_DIRECTORY_ENV: &str =
        "FE2O3_TEST_RESTART_MARKER_CRASH_HELPER_DIRECTORY";
    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    const MARKER_CRASH_HELPER_ATTEMPT_ENV: &str = "FE2O3_TEST_RESTART_MARKER_CRASH_HELPER_ATTEMPT";
    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    const MARKER_CRASH_HELPER_SEED_ENV: &str = "FE2O3_TEST_RESTART_MARKER_CRASH_HELPER_SEED";
    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    const CLEANUP_CRASH_HELPER_DIRECTORY_ENV: &str =
        "FE2O3_TEST_RESTART_CLEANUP_CRASH_HELPER_DIRECTORY";
    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    const CLEANUP_CRASH_HELPER_PRODUCER_SEED_ENV: &str =
        "FE2O3_TEST_RESTART_CLEANUP_CRASH_HELPER_PRODUCER_SEED";
    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    const CLEANUP_CRASH_HELPER_CLOSURE_SEED_ENV: &str =
        "FE2O3_TEST_RESTART_CLEANUP_CRASH_HELPER_CLOSURE_SEED";
    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    const CLEANUP_CRASH_HELPER_ACTION_ENV: &str = "FE2O3_TEST_RESTART_CLEANUP_CRASH_HELPER_ACTION";

    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    fn run_marker_crash_helper(
        directory: &Path,
        attempt: BuildAttempt,
        seed: u8,
        fault: &str,
    ) -> std::process::Output {
        std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg("worker_v2_restart::tests::restart_marker_crash_subprocess_helper")
            .arg("--nocapture")
            .env(MARKER_CRASH_HELPER_DIRECTORY_ENV, directory)
            .env(MARKER_CRASH_HELPER_ATTEMPT_ENV, attempt.to_env_value())
            .env(MARKER_CRASH_HELPER_SEED_ENV, seed.to_string())
            .env("FE2O3_TEST_WORKER_V2_FAULT_POINT_V1", fault)
            .output()
            .unwrap()
    }

    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    fn run_protected_cleanup_crash_helper(
        directory: &Path,
        producer_seed: u8,
        closure_seed: u8,
        action: &str,
        fault: &str,
    ) -> std::process::Output {
        std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg("worker_v2_restart::tests::protected_cleanup_crash_subprocess_helper")
            .arg("--nocapture")
            .env(CLEANUP_CRASH_HELPER_DIRECTORY_ENV, directory)
            .env(
                CLEANUP_CRASH_HELPER_PRODUCER_SEED_ENV,
                producer_seed.to_string(),
            )
            .env(
                CLEANUP_CRASH_HELPER_CLOSURE_SEED_ENV,
                closure_seed.to_string(),
            )
            .env(CLEANUP_CRASH_HELPER_ACTION_ENV, action)
            .env("FE2O3_TEST_WORKER_V2_FAULT_POINT_V1", fault)
            .output()
            .unwrap()
    }

    #[test]
    fn restart_scans_accept_exact_real_entry_bound_and_reject_limit_plus_one() {
        let mut entries = 0_usize;
        assert!(!count_restart_artifact_entry(&mut entries, b".").unwrap());
        assert!(!count_restart_artifact_entry(&mut entries, b"..").unwrap());
        for _ in 0..MAX_WORKER_V2_ARTIFACT_DIRECTORY_ENTRIES_V1 {
            assert!(count_restart_artifact_entry(&mut entries, b"real").unwrap());
        }
        assert_eq!(entries, MAX_WORKER_V2_ARTIFACT_DIRECTORY_ENTRIES_V1);
        assert!(count_restart_artifact_entry(&mut entries, b"over-limit").is_err());
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
    fn ordinary_v1_v2_v3_and_obsolete_protected_v4_bytes_match_fixed_goldens() {
        let package = [0x2a; 32];
        let attempt = fixed_attempt();
        let empty_receipt = ReceiptRecordV1([[0; 32]; RECEIPT_FIELDS]);
        let v1 = legacy_marker_bytes(package, attempt, 2, [0x44; 32], empty_receipt);
        let v2 = previous_marker_bytes(
            package,
            WorkerV2PublicationKindV1::Finalized,
            attempt,
            2,
            [0x33; 32],
            [0x44; 32],
            empty_receipt,
        );
        let state_v3 = ResumeMarkerStateV1::Ready {
            legacy: false,
            publication: WorkerV2PublicationKindV1::Finalized,
            attempt,
            admission: [0x33; 32],
            envelope_inputs: [0; 32],
            intent: WorkerV2PublicationIntentIdentityV1::from_bytes([0x44; 32]),
        };
        let v3 = encode_marker(package, state_v3);
        let independently_encoded_v3 = marker_v3_bytes(package, state_v3);
        let v4 = protected_v4_marker_bytes(
            package,
            CanonicalMarkerState::Ready {
                publication: WorkerV2PublicationKindV1::Finalized,
                attempt,
                admission: [0x33; 32],
                envelope_inputs: [0; 32],
                intent: WorkerV2PublicationIntentIdentityV2::from_bytes([0x44; 32]),
            },
        );

        assert_eq!(v3, independently_encoded_v3);
        assert_eq!(
            marker_sha256(&v1),
            "a379fdd88a0928a362a0922e51fce83a603ec33c4306bb0648f6b053fe10712f"
        );
        assert_eq!(
            marker_sha256(&v2),
            "897e7f443aa155e352f66073ebee8bdd8b359c22ece67ab4cd34686fb2f86816"
        );
        assert_eq!(
            marker_sha256(&v3),
            "7764c36932904b5069adb7e0ba5527d1e6275fe7b5583afdee086f1012af8d51"
        );
        assert_eq!(
            marker_sha256(&v4),
            "ec692675ecd13b68e865f9c6ade566d76e40d7fa9fb7af176ad8295bfc889e0c"
        );
        assert!(matches!(
            decode_marker(&v1, package),
            Ok(ResumeMarkerStateV1::Ready {
                legacy: true,
                intent,
                ..
            }) if intent.as_bytes() == [0x44; 32]
        ));
        assert!(matches!(
            decode_marker(&v2, package),
            Ok(ResumeMarkerStateV1::Ready {
                legacy: false,
                publication: WorkerV2PublicationKindV1::Finalized,
                intent,
                ..
            }) if intent.as_bytes() == [0x44; 32]
        ));
        assert_eq!(decode_marker(&v3, package), Ok(state_v3));
    }

    #[test]
    fn protected_v5_state_machine_round_trips_restarts_replays_and_clears() {
        let directory = TestDirectory::new();
        let producer = producer(101);
        let attempt = attempt(&directory.0, &producer, 101);
        let closure = compiler_closure(101);
        let store = WorkerV2ResumeStoreV2::open(&directory.0, &producer).unwrap();
        let publication = WorkerV2PublicationKindV1::Finalized;
        let (output, plan, upstream) = publication_inputs(attempt, 101);
        let admission = restart_admission_commitment_with_inputs_v2(
            publication,
            plan,
            upstream,
            &output,
            None,
            closure,
        );

        assert_eq!(store.load().unwrap(), None);
        store
            .persist_pending(publication, attempt, admission)
            .unwrap();
        let pending = ResumeMarkerStateV2::Pending {
            publication,
            attempt,
            admission,
            envelope_inputs: [0; 32],
        };
        assert_eq!(store.load().unwrap(), Some(pending));
        store
            .persist_pending(publication, attempt, admission)
            .unwrap();

        let recovered = persist_worker_v2_publication_intent_v2(
            &directory.0,
            &producer,
            attempt,
            plan,
            upstream,
            closure,
            &output,
        )
        .unwrap();
        let intent = recovered.record().identity();
        store
            .persist_ready(publication, attempt, &recovered, closure)
            .unwrap();
        let ready = ResumeMarkerStateV2::Ready {
            publication,
            attempt,
            admission,
            envelope_inputs: [0; 32],
            intent,
        };
        assert_eq!(store.load().unwrap(), Some(ready));
        store
            .persist_ready(publication, attempt, &recovered, closure)
            .unwrap();

        let published = publish_exact_hsaco_evidence_for_attempt_v2(
            &directory.0,
            &producer,
            attempt,
            plan,
            upstream,
            closure,
            &output,
        )
        .unwrap();
        let receipt = published.receipt();
        drop(published);
        let completed = store
            .persist_completed(publication, attempt, intent, receipt, closure)
            .unwrap();
        assert!(matches!(completed, ResumeMarkerStateV2::Completed { .. }));
        assert_eq!(store.load().unwrap(), Some(completed));
        assert_eq!(
            store
                .persist_completed(publication, attempt, intent, receipt, closure)
                .unwrap(),
            completed
        );

        let marker = fs::read(
            store
                .inner
                .display_path
                .join(store.inner.marker_name.as_str()),
        )
        .unwrap();
        assert_eq!(marker.len(), PROTECTED_MARKER_BYTES);
        assert_eq!(
            &marker[MARKER_MAGIC.len()..MARKER_MAGIC.len() + 2],
            &PROTECTED_MARKER_VERSION.to_le_bytes()
        );
        assert_eq!(marker[MARKER_MAGIC.len() + 2], PROTECTED_INTENT_SCHEMA_V2);

        drop(store);
        let store = WorkerV2ResumeStoreV2::open(&directory.0, &producer).unwrap();
        assert_eq!(store.load().unwrap(), Some(completed));
        store.clear_completed(completed, receipt, closure).unwrap();
        assert_eq!(store.load().unwrap(), None);
    }

    #[test]
    fn protected_v5_store_rejects_invalid_transitions_without_changing_state() {
        let directory = TestDirectory::new();
        let producer = producer(105);
        let attempt = attempt(&directory.0, &producer, 105);
        let closure = compiler_closure(105);
        let store = WorkerV2ResumeStoreV2::open(&directory.0, &producer).unwrap();
        let publication = WorkerV2PublicationKindV1::Finalized;
        let (output, plan, upstream) = publication_inputs(attempt, 105);
        let recovered = persist_worker_v2_publication_intent_v2(
            &directory.0,
            &producer,
            attempt,
            plan,
            upstream,
            closure,
            &output,
        )
        .unwrap();
        let admission = restart_admission_commitment_with_inputs_v2(
            publication,
            plan,
            upstream,
            &output,
            None,
            closure,
        );

        assert!(matches!(
            store.persist_ready(publication, attempt, &recovered, closure),
            Err(ResumeMarkerErrorV1::InvalidTransition)
        ));
        store
            .persist_pending(publication, attempt, admission)
            .unwrap();
        let pending = store.load().unwrap().unwrap();
        assert!(
            store
                .persist_ready(
                    publication,
                    attempt,
                    &recovered,
                    mutated_compiler_closure(105, 0),
                )
                .is_err()
        );
        assert!(matches!(
            store.persist_ready(WorkerV2PublicationKindV1::Raw, attempt, &recovered, closure,),
            Err(ResumeMarkerErrorV1::InvalidTransition)
        ));
        assert_eq!(store.load().unwrap(), Some(pending));
        store.clear_abandoned_pending(pending).unwrap();
        assert_eq!(store.load().unwrap(), None);
    }

    #[test]
    fn protected_ready_rejects_cross_directory_intent_replay_without_marker_mutation() {
        let local_directory = TestDirectory::new();
        let foreign_directory = TestDirectory::new();
        let producer = producer(140);
        let local_attempt = attempt(&local_directory.0, &producer, 140);
        let foreign_attempt = attempt(&foreign_directory.0, &producer, 140);
        assert_eq!(local_attempt, foreign_attempt);
        let closure = compiler_closure(140);
        let publication = WorkerV2PublicationKindV1::Finalized;
        let (output, plan, upstream) = publication_inputs(foreign_attempt, 140);
        let foreign = persist_worker_v2_publication_intent_v2(
            &foreign_directory.0,
            &producer,
            foreign_attempt,
            plan,
            upstream,
            closure,
            &output,
        )
        .unwrap();
        let store = WorkerV2ResumeStoreV2::open(&local_directory.0, &producer).unwrap();
        let admission = restart_admission_commitment_with_inputs_v2(
            publication,
            plan,
            upstream,
            &output,
            None,
            closure,
        );
        store
            .persist_pending(publication, local_attempt, admission)
            .unwrap();
        let pending = store.load().unwrap().unwrap();
        let marker_path = store
            .inner
            .display_path
            .join(store.inner.marker_name.as_str());
        let marker_before = fs::read(&marker_path).unwrap();

        assert!(
            store
                .persist_ready(publication, local_attempt, &foreign, closure)
                .is_err()
        );
        assert_eq!(store.load().unwrap(), Some(pending));
        assert_eq!(fs::read(marker_path).unwrap(), marker_before);
        assert!(matches!(
            recover_worker_v2_publication_intent_v2(
                &local_directory.0,
                &producer,
                local_attempt,
                closure,
            ),
            Err(WorkerV2PublicationIntentErrorV2::NotFound)
        ));
    }

    #[test]
    fn protected_ready_rejects_cross_producer_intent_replay_without_marker_mutation() {
        let local_directory = TestDirectory::new();
        let foreign_directory = TestDirectory::new();
        let local_producer = producer(141);
        let foreign_producer = producer(142);
        let local_attempt = attempt(&local_directory.0, &local_producer, 141);
        let foreign_attempt = begin_build_attempt(
            &foreign_directory.0,
            &foreign_producer,
            local_attempt.invocation(),
            local_attempt.session(),
        )
        .unwrap();
        assert_eq!(local_attempt, foreign_attempt);
        let closure = compiler_closure(141);
        let publication = WorkerV2PublicationKindV1::Finalized;
        let (output, plan, upstream) = publication_inputs(foreign_attempt, 141);
        let foreign = persist_worker_v2_publication_intent_v2(
            &foreign_directory.0,
            &foreign_producer,
            foreign_attempt,
            plan,
            upstream,
            closure,
            &output,
        )
        .unwrap();
        let store = WorkerV2ResumeStoreV2::open(&local_directory.0, &local_producer).unwrap();
        let admission = restart_admission_commitment_with_inputs_v2(
            publication,
            plan,
            upstream,
            &output,
            None,
            closure,
        );
        store
            .persist_pending(publication, local_attempt, admission)
            .unwrap();
        let pending = store.load().unwrap().unwrap();
        let marker_path = store
            .inner
            .display_path
            .join(store.inner.marker_name.as_str());
        let marker_before = fs::read(&marker_path).unwrap();

        assert!(
            store
                .persist_ready(publication, local_attempt, &foreign, closure)
                .is_err()
        );
        assert_eq!(store.load().unwrap(), Some(pending));
        assert_eq!(fs::read(marker_path).unwrap(), marker_before);
        assert!(matches!(
            recover_worker_v2_publication_intent_v2(
                &local_directory.0,
                &local_producer,
                local_attempt,
                closure,
            ),
            Err(WorkerV2PublicationIntentErrorV2::NotFound)
        ));
    }

    #[test]
    fn protected_v5_store_rejects_files_beyond_the_canonical_bound() {
        let directory = TestDirectory::new();
        let producer = producer(106);
        let store = WorkerV2ResumeStoreV2::open(&directory.0, &producer).unwrap();
        let marker_path = store
            .inner
            .display_path
            .join(store.inner.marker_name.as_str());
        drop(store);
        fs::write(&marker_path, vec![0; PROTECTED_MARKER_BYTES + 1]).unwrap();
        fs::set_permissions(&marker_path, fs::Permissions::from_mode(0o600)).unwrap();

        assert!(WorkerV2ResumeStoreV2::open(&directory.0, &producer).is_err());
    }

    #[test]
    fn protected_v5_codec_accepts_every_canonical_stage() {
        let package = [0x2a; 32];
        let attempt = fixed_attempt();
        let publication = WorkerV2PublicationKindV1::Finalized;
        let intent = WorkerV2PublicationIntentIdentityV2::from_bytes([0x44; 32]);
        let closure = compiler_closure(0x55);
        let states = [
            ResumeMarkerStateV2::Pending {
                publication,
                attempt,
                admission: [0x33; 32],
                envelope_inputs: [0; 32],
            },
            ResumeMarkerStateV2::Ready {
                publication,
                attempt,
                admission: [0x33; 32],
                envelope_inputs: [0; 32],
                intent,
            },
            ResumeMarkerStateV2::Completed {
                publication,
                attempt,
                admission: [0x33; 32],
                envelope_inputs: [0; 32],
                envelope: [0; 32],
                intent,
                receipt: ReceiptRecordV2 {
                    fields: [[0x55; 32]; RECEIPT_FIELDS],
                    compiler_closure: Some(closure),
                },
            },
        ];

        for state in states {
            let encoded = encode_protected_marker(package, state);
            assert_eq!(encoded.len(), PROTECTED_MARKER_BYTES);
            assert_eq!(decode_protected_marker(&encoded, package), Ok(state));
        }
    }

    #[test]
    fn protected_v5_rejects_truncation_trailing_oversize_and_noncanonical_fields() {
        let package = [0x2a; 32];
        let attempt = fixed_attempt();
        let publication = WorkerV2PublicationKindV1::Finalized;
        let canonical = ResumeMarkerStateV2::Ready {
            publication,
            attempt,
            admission: [0x33; 32],
            envelope_inputs: [0; 32],
            intent: WorkerV2PublicationIntentIdentityV2::from_bytes([0x44; 32]),
        };
        let encoded = encode_protected_marker(package, canonical);

        for length in 0..encoded.len() {
            assert!(
                decode_protected_marker(&encoded[..length], package).is_err(),
                "truncation at {length} bytes was accepted"
            );
        }
        let mut trailing = encoded.clone();
        trailing.push(0);
        assert!(decode_protected_marker(&trailing, package).is_err());
        assert!(decode_protected_marker(&encoded, [0x2b; 32]).is_err());

        let invalid = [
            ResumeMarkerStateV2::Pending {
                publication,
                attempt,
                admission: [0; 32],
                envelope_inputs: [0; 32],
            },
            ResumeMarkerStateV2::Pending {
                publication,
                attempt,
                admission: [0x33; 32],
                envelope_inputs: [0x34; 32],
            },
            ResumeMarkerStateV2::Ready {
                publication,
                attempt,
                admission: [0x33; 32],
                envelope_inputs: [0; 32],
                intent: WorkerV2PublicationIntentIdentityV2::from_bytes([0; 32]),
            },
            ResumeMarkerStateV2::Completed {
                publication,
                attempt,
                admission: [0x33; 32],
                envelope_inputs: [0; 32],
                envelope: [0; 32],
                intent: WorkerV2PublicationIntentIdentityV2::from_bytes([0x44; 32]),
                receipt: ReceiptRecordV2::empty(),
            },
        ];
        for state in invalid {
            assert!(
                decode_protected_marker(&encode_protected_marker(package, state), package).is_err()
            );
        }

        let required_envelope_placeholder = ResumeMarkerStateV2::Completed {
            publication: WorkerV2PublicationKindV1::FinalizedEnvelopeRequired,
            attempt,
            admission: [0x33; 32],
            envelope_inputs: [0x34; 32],
            envelope: [0; 32],
            intent: WorkerV2PublicationIntentIdentityV2::from_bytes([0x44; 32]),
            receipt: ReceiptRecordV2 {
                fields: [[0x55; 32]; RECEIPT_FIELDS],
                compiler_closure: Some(compiler_closure(0x55)),
            },
        };
        assert!(
            decode_protected_marker(
                &encode_protected_marker(package, required_envelope_placeholder),
                package,
            )
            .is_err()
        );

        let completed = ResumeMarkerStateV2::Completed {
            publication,
            attempt,
            admission: [0x33; 32],
            envelope_inputs: [0; 32],
            envelope: [0; 32],
            intent: WorkerV2PublicationIntentIdentityV2::from_bytes([0x44; 32]),
            receipt: ReceiptRecordV2 {
                fields: [[0x55; 32]; RECEIPT_FIELDS],
                compiler_closure: Some(compiler_closure(0x55)),
            },
        };
        let mut noncanonical_closure = encode_protected_marker(package, completed);
        let closure_offset = PROTECTED_MARKER_BYTES - 32 - COMPILER_CLOSURE_BYTES_V2;
        noncanonical_closure[closure_offset] ^= 1;
        reseal_protected_marker_v5(&mut noncanonical_closure);
        assert!(decode_protected_marker(&noncanonical_closure, package).is_err());
    }

    #[test]
    fn ordinary_and_protected_marker_schemas_reject_downgrade_and_cross_use() {
        let package = [0x2a; 32];
        let attempt = fixed_attempt();
        let empty_receipt = ReceiptRecordV1([[0; 32]; RECEIPT_FIELDS]);
        let ordinary_v1 = legacy_marker_bytes(package, attempt, 2, [0x44; 32], empty_receipt);
        let ordinary_v2 = previous_marker_bytes(
            package,
            WorkerV2PublicationKindV1::Finalized,
            attempt,
            2,
            [0x33; 32],
            [0x44; 32],
            empty_receipt,
        );
        let ordinary_v3 = encode_marker(
            package,
            ResumeMarkerStateV1::Ready {
                legacy: false,
                publication: WorkerV2PublicationKindV1::Finalized,
                attempt,
                admission: [0x33; 32],
                envelope_inputs: [0; 32],
                intent: WorkerV2PublicationIntentIdentityV1::from_bytes([0x44; 32]),
            },
        );
        let protected_v4 = protected_v4_marker_bytes(
            package,
            CanonicalMarkerState::Ready {
                publication: WorkerV2PublicationKindV1::Finalized,
                attempt,
                admission: [0x33; 32],
                envelope_inputs: [0; 32],
                intent: WorkerV2PublicationIntentIdentityV2::from_bytes([0x44; 32]),
            },
        );
        let protected_v5 = encode_protected_marker(
            package,
            ResumeMarkerStateV2::Ready {
                publication: WorkerV2PublicationKindV1::Finalized,
                attempt,
                admission: [0x33; 32],
                envelope_inputs: [0; 32],
                intent: WorkerV2PublicationIntentIdentityV2::from_bytes([0x44; 32]),
            },
        );

        for ordinary in [&ordinary_v1, &ordinary_v2, &ordinary_v3] {
            assert!(decode_protected_marker(ordinary, package).is_err());
        }
        assert!(decode_marker(&protected_v4, package).is_err());
        assert!(decode_marker(&protected_v5, package).is_err());
        assert!(decode_protected_marker(&protected_v4, package).is_err());
        assert!(
            decode_canonical_marker::<ObsoleteProtectedMarkerSchemaV4>(&protected_v5, package)
                .is_err()
        );

        for version in [
            0,
            LEGACY_MARKER_VERSION,
            PREVIOUS_MARKER_VERSION,
            MARKER_VERSION,
            OBSOLETE_PROTECTED_MARKER_VERSION_V4,
            PROTECTED_MARKER_VERSION + 1,
            u16::MAX,
        ] {
            let mut wrong_version = protected_v5.clone();
            wrong_version[MARKER_MAGIC.len()..MARKER_MAGIC.len() + 2]
                .copy_from_slice(&version.to_le_bytes());
            reseal_protected_marker_v5(&mut wrong_version);
            assert!(decode_protected_marker(&wrong_version, package).is_err());
            assert!(decode_marker(&wrong_version, package).is_err());
        }

        let mut upgraded = ordinary_v3.clone();
        upgraded[MARKER_MAGIC.len()..MARKER_MAGIC.len() + 2]
            .copy_from_slice(&PROTECTED_MARKER_VERSION.to_le_bytes());
        reseal_marker(&mut upgraded);
        assert!(decode_marker(&upgraded, package).is_err());
        assert!(decode_protected_marker(&upgraded, package).is_err());

        let mut cross_schema = protected_v5;
        cross_schema[MARKER_MAGIC.len() + 2] = 1;
        reseal_protected_marker_v5(&mut cross_schema);
        assert!(decode_protected_marker(&cross_schema, package).is_err());
    }

    #[test]
    fn protected_store_rejects_obsolete_v4_v1_receipt_state_without_modifying_it() {
        let directory = TestDirectory::new();
        let producer = producer(116);
        let attempt = attempt(&directory.0, &producer, 116);
        let store = WorkerV2ResumeStoreV2::open(&directory.0, &producer).unwrap();
        let publication = WorkerV2PublicationKindV1::Finalized;
        let admission = [0x33; 32];
        let intent = WorkerV2PublicationIntentIdentityV2::from_bytes([0x44; 32]);
        let receipt = receipt(&directory.0, &producer, attempt, 116);
        let obsolete = protected_v4_marker_bytes(
            store.inner.package,
            CanonicalMarkerState::Completed {
                publication,
                attempt,
                admission,
                envelope_inputs: [0; 32],
                envelope: [0; 32],
                intent,
                receipt: ReceiptRecordV1::from_receipt(receipt),
            },
        );
        let marker_path = store
            .inner
            .display_path
            .join(store.inner.marker_name.as_str());
        install_marker(&store.inner, &obsolete);
        drop(store);

        assert!(read_backend_publication_receipt_v2(&directory.0, &producer, attempt).is_err());
        assert!(WorkerV2ResumeStoreV2::open(&directory.0, &producer).is_err());
        assert_eq!(fs::read(&marker_path).unwrap(), obsolete);
        assert!(WorkerV2ResumeStoreV1::open(&directory.0, &producer).is_err());
        assert_eq!(fs::read(marker_path).unwrap(), obsolete);
    }

    #[test]
    fn protected_completion_binds_all_six_compiler_closure_roles() {
        let directory = TestDirectory::new();
        let producer = producer(117);
        let attempt = attempt(&directory.0, &producer, 117);
        let (output, plan, upstream) = publication_inputs(attempt, 117);
        let closure = compiler_closure(117);
        let publication = WorkerV2PublicationKindV1::Finalized;
        let store = WorkerV2ResumeStoreV2::open(&directory.0, &producer).unwrap();
        let persisted = persist_admitted_worker_v2_intent_v2(
            &store,
            &producer,
            publication,
            plan,
            upstream,
            &output,
            None,
            closure,
        )
        .unwrap();
        let intent = persisted.intent.record().identity();
        drop(persisted);
        let receipt = publish_exact_hsaco_evidence_for_attempt_v2(
            &directory.0,
            &producer,
            attempt,
            plan,
            upstream,
            closure,
            &output,
        )
        .unwrap()
        .receipt();
        let ready = store.load().unwrap().unwrap();

        for role in 0..6 {
            assert!(
                store
                    .persist_completed(
                        publication,
                        attempt,
                        intent,
                        receipt,
                        mutated_compiler_closure(117, role),
                    )
                    .is_err()
            );
            assert_eq!(store.load().unwrap(), Some(ready));
        }

        let completed = store
            .persist_completed(publication, attempt, intent, receipt, closure)
            .unwrap();
        assert_eq!(
            completed.completed_receipt().unwrap().compiler_closure(),
            Some(closure)
        );
        store.clear_completed(completed, receipt, closure).unwrap();
    }

    #[test]
    fn protected_recovery_and_clear_reject_each_substituted_receipt_field() {
        let directory = TestDirectory::new();
        let producer = producer(118);
        let attempt = attempt(&directory.0, &producer, 118);
        let (output, plan, upstream) = publication_inputs(attempt, 118);
        let closure = compiler_closure(118);
        let publication = WorkerV2PublicationKindV1::Finalized;
        let store = WorkerV2ResumeStoreV2::open(&directory.0, &producer).unwrap();
        let persisted = persist_admitted_worker_v2_intent_v2(
            &store,
            &producer,
            publication,
            plan,
            upstream,
            &output,
            None,
            closure,
        )
        .unwrap();
        let intent = persisted.intent.record().identity();
        drop(persisted);
        let receipt = publish_exact_hsaco_evidence_for_attempt_v2(
            &directory.0,
            &producer,
            attempt,
            plan,
            upstream,
            closure,
            &output,
        )
        .unwrap()
        .receipt();
        let completed = store
            .persist_completed(publication, attempt, intent, receipt, closure)
            .unwrap();
        let canonical = encode_protected_marker(store.inner.package, completed);
        let receipt_offset =
            PROTECTED_MARKER_BYTES - 32 - COMPILER_CLOSURE_BYTES_V2 - RECEIPT_FIELDS * 32;

        for field in 0..RECEIPT_FIELDS {
            let mut substituted = canonical.clone();
            substituted[receipt_offset + field * 32] ^= 1;
            reseal_protected_marker_v5(&mut substituted);
            install_marker(&store.inner, &substituted);
            let substituted_state = store.load().unwrap().unwrap();
            assert!(matches!(
                recover_worker_v2_intent_v2(&store, &producer, substituted_state, closure,),
                Err(RestartIntentErrorV2::Marker(
                    ResumeMarkerErrorV1::InvalidTransition
                ))
            ));
            assert!(matches!(
                store.clear_completed(substituted_state, receipt, closure),
                Err(ResumeMarkerErrorV1::InvalidTransition)
            ));
            assert_eq!(
                fs::read(store.inner.display_path.join(&store.inner.marker_name)).unwrap(),
                substituted
            );
        }

        install_marker(&store.inner, &canonical);
        assert_eq!(store.load().unwrap(), Some(completed));
        assert!(recover_worker_v2_intent_v2(&store, &producer, completed, closure).is_ok());
        store.clear_completed(completed, receipt, closure).unwrap();
    }

    #[test]
    fn protected_required_marker_requires_the_exact_envelope_input_identity() {
        let directory = TestDirectory::new();
        let producer = producer(119);
        let attempt = attempt(&directory.0, &producer, 119);
        let store = WorkerV2ResumeStoreV2::open(&directory.0, &producer).unwrap();
        assert!(matches!(
            store.persist_pending_with_envelope_inputs(
                WorkerV2PublicationKindV1::FinalizedEnvelopeRequired,
                attempt,
                [0x33; 32],
                None,
            ),
            Err(ResumeMarkerErrorV1::InvalidTransition)
        ));
        assert_eq!(store.load().unwrap(), None);
        let inputs = WorkerV2EnvelopeInputsIdentityV1::from_bytes([0x34; 32]);
        store
            .persist_pending_with_envelope_inputs(
                WorkerV2PublicationKindV1::FinalizedEnvelopeRequired,
                attempt,
                [0x33; 32],
                Some(inputs),
            )
            .unwrap();
        let state = store.load().unwrap().unwrap();
        assert_eq!(state.envelope_inputs(), inputs.as_bytes());
        assert_eq!(state.envelope(), [0; 32]);
    }

    #[test]
    fn protected_v2_envelope_persists_recovers_reconciles_and_clears_exactly() {
        let directory = TestDirectory::new();
        let producer = producer(120);
        let attempt = attempt(&directory.0, &producer, 120);
        let closure = compiler_closure(120);
        let fixture = protected_envelope_fixture(&directory.0, &producer, attempt, closure);
        let store = WorkerV2ResumeStoreV2::open(&directory.0, &producer).unwrap();
        stage_protected_required_ready(&store, &fixture, closure);
        let publication = WorkerV2PublicationKindV1::FinalizedEnvelopeRequired;

        let completed = store
            .persist_envelope_and_completed(
                publication,
                attempt,
                fixture.intent,
                fixture.receipt,
                closure,
                &fixture.envelope,
            )
            .unwrap();
        assert_eq!(completed.envelope(), fixture.envelope.identity().as_bytes());
        assert_eq!(
            store
                .persist_envelope_and_completed(
                    publication,
                    attempt,
                    fixture.intent,
                    fixture.receipt,
                    closure,
                    &fixture.envelope,
                )
                .unwrap(),
            completed
        );
        let protected_name = protected_envelope_name_v2(fixture.receipt.publication_identity());
        assert!(directory.0.join(&protected_name).is_file());
        assert!(
            !directory
                .0
                .join(envelope_name(fixture.receipt.publication_identity()))
                .exists()
        );
        drop(store);

        let restarted = WorkerV2ResumeStoreV2::open(&directory.0, &producer).unwrap();
        assert_eq!(
            restarted
                .recover_load_envelope(fixture.receipt, closure)
                .unwrap()
                .to_bytes(),
            fixture.envelope.to_bytes()
        );
        assert_eq!(
            restarted
                .recover_envelope_and_completed(
                    publication,
                    attempt,
                    fixture.intent,
                    fixture.receipt,
                    closure,
                )
                .unwrap(),
            completed
        );
        restarted
            .clear_completed_and_envelope_inputs(completed, fixture.receipt, closure)
            .unwrap();
        assert_eq!(restarted.load().unwrap(), None);
        assert!(restarted.recover_envelope_inputs(attempt).is_err());
        assert!(matches!(
            restarted.recover_load_envelope(fixture.receipt, closure),
            Err(ResumeMarkerErrorV1::InvalidTransition)
        ));
    }

    #[test]
    fn protected_v2_envelope_crash_window_reconciles_after_envelope_publication() {
        let directory = TestDirectory::new();
        let producer = producer(121);
        let attempt = attempt(&directory.0, &producer, 121);
        let closure = compiler_closure(121);
        let fixture = protected_envelope_fixture(&directory.0, &producer, attempt, closure);
        let store = WorkerV2ResumeStoreV2::open(&directory.0, &producer).unwrap();
        stage_protected_required_ready(&store, &fixture, closure);
        let ready = store.load().unwrap().unwrap();
        assert!(matches!(ready, ResumeMarkerStateV2::Ready { .. }));
        assert_eq!(
            store
                .publish_load_envelope(&fixture.envelope, fixture.receipt, closure)
                .unwrap(),
            WorkerV2EnvelopePublicationOutcomeV1::Published
        );
        drop(store);

        let restarted = WorkerV2ResumeStoreV2::open(&directory.0, &producer).unwrap();
        assert_eq!(restarted.load().unwrap(), Some(ready));
        let completed = restarted
            .recover_envelope_and_completed(
                WorkerV2PublicationKindV1::FinalizedEnvelopeRequired,
                attempt,
                fixture.intent,
                fixture.receipt,
                closure,
            )
            .unwrap();
        assert!(matches!(completed, ResumeMarkerStateV2::Completed { .. }));
        assert!(matches!(
            restarted.publish_load_envelope(&fixture.envelope, fixture.receipt, closure),
            Err(ResumeMarkerErrorV1::InvalidTransition)
        ));
    }

    #[test]
    fn protected_v2_envelope_rejects_tamper_v1_magic_and_oversize_without_marker_mutation() {
        let directory = TestDirectory::new();
        let producer = producer(122);
        let attempt = attempt(&directory.0, &producer, 122);
        let closure = compiler_closure(122);
        let fixture = protected_envelope_fixture(&directory.0, &producer, attempt, closure);
        let store = WorkerV2ResumeStoreV2::open(&directory.0, &producer).unwrap();
        stage_protected_required_ready(&store, &fixture, closure);
        let ready = store.load().unwrap().unwrap();
        store
            .publish_load_envelope(&fixture.envelope, fixture.receipt, closure)
            .unwrap();
        let name = protected_envelope_name_v2(fixture.receipt.publication_identity());
        let path = directory.0.join(&name);
        let canonical = fs::read(&path).unwrap();

        let mut tampered = canonical.clone();
        *tampered.last_mut().unwrap() ^= 1;
        fs::write(&path, &tampered).unwrap();
        assert!(
            store
                .recover_load_envelope(fixture.receipt, closure)
                .is_err()
        );
        assert_eq!(store.load().unwrap(), Some(ready));

        let mut v1_tagged = canonical.clone();
        v1_tagged[..fe2o3_worker_v2_bundle::WORKER_V2_LOAD_ENVELOPE_MAGIC.len()]
            .copy_from_slice(&fe2o3_worker_v2_bundle::WORKER_V2_LOAD_ENVELOPE_MAGIC);
        fs::write(&path, &v1_tagged).unwrap();
        assert!(
            store
                .recover_load_envelope(fixture.receipt, closure)
                .is_err()
        );
        assert_eq!(store.load().unwrap(), Some(ready));

        let oversized = File::create(&path).unwrap();
        oversized
            .set_len((MAX_WORKER_V2_LOAD_ENVELOPE_BYTES_V2 + 1) as u64)
            .unwrap();
        drop(oversized);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(
            store
                .recover_load_envelope(fixture.receipt, closure)
                .is_err()
        );
        assert_eq!(store.load().unwrap(), Some(ready));
    }

    #[test]
    fn protected_v2_envelope_recovery_binds_every_compiler_closure_role() {
        let directory = TestDirectory::new();
        let producer = producer(123);
        let attempt = attempt(&directory.0, &producer, 123);
        let closure = compiler_closure(123);
        let fixture = protected_envelope_fixture(&directory.0, &producer, attempt, closure);
        let store = WorkerV2ResumeStoreV2::open(&directory.0, &producer).unwrap();
        stage_protected_required_ready(&store, &fixture, closure);
        store
            .publish_load_envelope(&fixture.envelope, fixture.receipt, closure)
            .unwrap();
        let ready = store.load().unwrap().unwrap();

        for role in 0..6 {
            let substituted = mutated_compiler_closure(123, role);
            assert!(
                store
                    .recover_load_envelope(fixture.receipt, substituted)
                    .is_err()
            );
            assert!(
                store
                    .recover_envelope_and_completed(
                        WorkerV2PublicationKindV1::FinalizedEnvelopeRequired,
                        attempt,
                        fixture.intent,
                        fixture.receipt,
                        substituted,
                    )
                    .is_err()
            );
            assert_eq!(store.load().unwrap(), Some(ready));
        }
    }

    #[test]
    fn protected_v2_envelope_rejects_every_resealed_claim_file_binding_substitution() {
        for (case, binding_offset) in [0_usize, 16, 32, 48].into_iter().enumerate() {
            let seed = 127 + case as u8;
            let directory = TestDirectory::new();
            let producer = producer(seed);
            let attempt = attempt(&directory.0, &producer, seed);
            let closure = compiler_closure(seed);
            let fixture = protected_envelope_fixture(&directory.0, &producer, attempt, closure);
            let substituted =
                envelope_with_resealed_claim_substitution(&fixture.envelope, binding_offset);
            let store = WorkerV2ResumeStoreV2::open(&directory.0, &producer).unwrap();
            stage_protected_required_ready(&store, &fixture, closure);
            let ready = store.load().unwrap().unwrap();

            assert!(
                store
                    .publish_load_envelope(&substituted, fixture.receipt, closure)
                    .is_err(),
                "resealed claim substitution {case} was accepted"
            );
            assert_eq!(store.load().unwrap(), Some(ready));
            assert!(
                recover_worker_v2_publication_intent_v2(&directory.0, &producer, attempt, closure,)
                    .is_ok()
            );
            assert!(
                !directory
                    .0
                    .join(protected_envelope_name_v2(
                        fixture.receipt.publication_identity()
                    ))
                    .exists()
            );
        }
    }

    #[test]
    fn protected_v2_envelope_publication_requires_the_exact_durable_ready_intent() {
        for pending in [false, true] {
            let seed = 132 + u8::from(pending);
            let directory = TestDirectory::new();
            let producer = producer(seed);
            let attempt = attempt(&directory.0, &producer, seed);
            let closure = compiler_closure(seed);
            let fixture = protected_envelope_fixture(&directory.0, &producer, attempt, closure);
            let store = WorkerV2ResumeStoreV2::open(&directory.0, &producer).unwrap();
            if pending {
                store
                    .persist_envelope_inputs(attempt, &fixture.envelope_inputs)
                    .unwrap();
                store
                    .persist_pending_with_envelope_inputs(
                        WorkerV2PublicationKindV1::FinalizedEnvelopeRequired,
                        attempt,
                        restart_admission_commitment_with_inputs_v2(
                            WorkerV2PublicationKindV1::FinalizedEnvelopeRequired,
                            fixture.plan,
                            fixture.upstream,
                            &fixture.output,
                            Some(fixture.envelope_inputs.identity()),
                            closure,
                        ),
                        Some(fixture.envelope_inputs.identity()),
                    )
                    .unwrap();
            }
            let marker = store.load().unwrap();

            assert!(matches!(
                store.publish_load_envelope(&fixture.envelope, fixture.receipt, closure),
                Err(ResumeMarkerErrorV1::InvalidTransition)
            ));
            assert_eq!(store.load().unwrap(), marker);
            assert!(
                recover_worker_v2_publication_intent_v2(&directory.0, &producer, attempt, closure,)
                    .is_ok()
            );
        }
    }

    #[test]
    fn protected_v2_envelope_rejects_a_resealed_ready_intent_substitution() {
        let seed = 134;
        let directory = TestDirectory::new();
        let producer = producer(seed);
        let attempt = attempt(&directory.0, &producer, seed);
        let closure = compiler_closure(seed);
        let fixture = protected_envelope_fixture(&directory.0, &producer, attempt, closure);
        let store = WorkerV2ResumeStoreV2::open(&directory.0, &producer).unwrap();
        stage_protected_required_ready(&store, &fixture, closure);
        let ResumeMarkerStateV2::Ready {
            publication,
            attempt,
            admission,
            envelope_inputs,
            intent,
        } = store.load().unwrap().unwrap()
        else {
            unreachable!()
        };
        let mut substituted_intent = intent.as_bytes();
        substituted_intent[0] ^= 1;
        let substituted = ResumeMarkerStateV2::Ready {
            publication,
            attempt,
            admission,
            envelope_inputs,
            intent: WorkerV2PublicationIntentIdentityV2::from_bytes(substituted_intent),
        };
        install_marker(
            &store.inner,
            &encode_protected_marker(store.inner.package, substituted),
        );

        assert!(
            store
                .publish_load_envelope(&fixture.envelope, fixture.receipt, closure)
                .is_err()
        );
        assert_eq!(store.load().unwrap(), Some(substituted));
        assert!(
            recover_worker_v2_publication_intent_v2(&directory.0, &producer, attempt, closure,)
                .is_ok()
        );
    }

    #[test]
    fn protected_v2_intent_cleanup_preserves_both_inputs_when_envelope_is_invalid() {
        for case in 0..3 {
            let seed = 135 + case;
            let directory = TestDirectory::new();
            let producer = producer(seed);
            let attempt = attempt(&directory.0, &producer, seed);
            let closure = compiler_closure(seed);
            let fixture = protected_envelope_fixture(&directory.0, &producer, attempt, closure);
            let store = WorkerV2ResumeStoreV2::open(&directory.0, &producer).unwrap();
            stage_protected_required_ready(&store, &fixture, closure);
            let completed = store
                .persist_envelope_and_completed(
                    WorkerV2PublicationKindV1::FinalizedEnvelopeRequired,
                    attempt,
                    fixture.intent,
                    fixture.receipt,
                    closure,
                    &fixture.envelope,
                )
                .unwrap();
            let envelope_path = directory.0.join(protected_envelope_name_v2(
                fixture.receipt.publication_identity(),
            ));
            match case {
                0 => fs::remove_file(&envelope_path).unwrap(),
                1 => {
                    let mut bytes = fs::read(&envelope_path).unwrap();
                    *bytes.last_mut().unwrap() ^= 1;
                    fs::write(&envelope_path, bytes).unwrap();
                }
                2 => {
                    let substituted =
                        envelope_with_resealed_claim_substitution(&fixture.envelope, 0);
                    fs::write(&envelope_path, substituted.to_bytes()).unwrap();
                }
                _ => unreachable!(),
            }

            assert!(
                clear_worker_v2_intent_v2(&store, &producer, completed, fixture.receipt, closure,)
                    .is_err()
            );
            assert_eq!(store.load().unwrap(), Some(completed));
            assert!(
                recover_worker_v2_publication_intent_v2(&directory.0, &producer, attempt, closure,)
                    .is_ok()
            );
        }
    }

    #[test]
    fn protected_v2_intent_cleanup_retry_retains_a_recoverable_anchor() {
        let seed = 138;
        let directory = TestDirectory::new();
        let producer = producer(seed);
        let attempt = attempt(&directory.0, &producer, seed);
        let closure = compiler_closure(seed);
        let fixture = protected_envelope_fixture(&directory.0, &producer, attempt, closure);
        let store = WorkerV2ResumeStoreV2::open(&directory.0, &producer).unwrap();
        stage_protected_required_ready(&store, &fixture, closure);
        let completed = store
            .persist_envelope_and_completed(
                WorkerV2PublicationKindV1::FinalizedEnvelopeRequired,
                attempt,
                fixture.intent,
                fixture.receipt,
                closure,
                &fixture.envelope,
            )
            .unwrap();

        clear_worker_v2_intent_v2(&store, &producer, completed, fixture.receipt, closure).unwrap();
        assert_eq!(store.load().unwrap(), Some(completed));
        let evidence = store.read_protected_cleanup_evidence().unwrap().unwrap();
        assert_eq!(
            recover_worker_v2_publication_intent_cleanup_escrow_v1(
                &directory.0,
                &producer,
                evidence.escrow,
            )
            .unwrap(),
            WorkerV2PublicationIntentCleanupEscrowStateV1::Prepared
        );
        assert_eq!(evidence.completed_state(), completed);
        assert_eq!(
            evidence.envelope_inputs,
            fixture.envelope_inputs.identity().as_bytes()
        );
        assert_eq!(
            evidence.escrow.receipt().finalized_output_identity(),
            Sha256::digest(&fixture.output).as_slice()
        );
        let canonical_evidence = evidence.to_bytes();
        assert_eq!(
            ProtectedIntentCleanupEvidenceV1::from_bytes(&canonical_evidence).unwrap(),
            evidence
        );
        let mut malformed_evidence = canonical_evidence;
        malformed_evidence[PROTECTED_CLEANUP_EVIDENCE_MAGIC_V1.len() + 3] ^= 1;
        assert!(ProtectedIntentCleanupEvidenceV1::from_bytes(&malformed_evidence).is_err());
        clear_worker_v2_intent_v2(&store, &producer, completed, fixture.receipt, closure).unwrap();
        assert_eq!(store.load().unwrap(), Some(completed));
        store
            .clear_completed_and_envelope_inputs(completed, fixture.receipt, closure)
            .unwrap();
        assert_eq!(store.load().unwrap(), None);
        assert!(matches!(
            recover_worker_v2_publication_intent_v2(&directory.0, &producer, attempt, closure),
            Err(WorkerV2PublicationIntentErrorV2::NotFound)
        ));
        assert_eq!(
            recover_worker_v2_publication_intent_cleanup_escrow_v1(
                &directory.0,
                &producer,
                evidence.escrow,
            )
            .unwrap(),
            WorkerV2PublicationIntentCleanupEscrowStateV1::Committed
        );
        assert!(store.read_protected_cleanup_evidence().unwrap().is_none());
    }

    #[test]
    fn protected_cleanup_evidence_owner_rejects_removal_while_escrow_is_prepared() {
        let seed = 219;
        let directory = TestDirectory::new();
        let producer = producer(seed);
        let attempt = attempt(&directory.0, &producer, seed);
        let closure = compiler_closure(seed);
        let fixture = protected_envelope_fixture(&directory.0, &producer, attempt, closure);
        let store = WorkerV2ResumeStoreV2::open(&directory.0, &producer).unwrap();
        stage_protected_required_ready(&store, &fixture, closure);
        let completed = store
            .persist_envelope_and_completed(
                WorkerV2PublicationKindV1::FinalizedEnvelopeRequired,
                attempt,
                fixture.intent,
                fixture.receipt,
                closure,
                &fixture.envelope,
            )
            .unwrap();
        clear_worker_v2_intent_v2(&store, &producer, completed, fixture.receipt, closure).unwrap();
        let evidence = store.read_protected_cleanup_evidence().unwrap().unwrap();

        assert!(store.remove_protected_cleanup_evidence(&evidence).is_err());
        assert_eq!(
            store.read_protected_cleanup_evidence().unwrap(),
            Some(evidence)
        );
        assert_eq!(
            recover_worker_v2_publication_intent_cleanup_escrow_v1(
                &directory.0,
                &producer,
                evidence.escrow,
            )
            .unwrap(),
            WorkerV2PublicationIntentCleanupEscrowStateV1::Prepared
        );
    }

    #[test]
    fn protected_cleanup_local_unlink_rejects_preexisting_and_racing_path_attacks() {
        let mut case = 0_u8;
        for target in ["marker", "inputs"] {
            for attack in ["replacement", "hardlink", "symlink"] {
                for racing in [false, true] {
                    let seed = 220 + case;
                    case += 1;
                    let directory = TestDirectory::new();
                    let producer = producer(seed);
                    let attempt = attempt(&directory.0, &producer, seed);
                    let closure = compiler_closure(seed);
                    let fixture =
                        protected_envelope_fixture(&directory.0, &producer, attempt, closure);
                    let store = WorkerV2ResumeStoreV2::open(&directory.0, &producer).unwrap();
                    stage_protected_required_ready(&store, &fixture, closure);
                    let completed = store
                        .persist_envelope_and_completed(
                            WorkerV2PublicationKindV1::FinalizedEnvelopeRequired,
                            attempt,
                            fixture.intent,
                            fixture.receipt,
                            closure,
                            &fixture.envelope,
                        )
                        .unwrap();
                    clear_worker_v2_intent_v2(
                        &store,
                        &producer,
                        completed,
                        fixture.receipt,
                        closure,
                    )
                    .unwrap();
                    let evidence = store.read_protected_cleanup_evidence().unwrap().unwrap();
                    store
                        .prepare_protected_cleanup_local_quarantine(&evidence)
                        .unwrap();
                    store.commit_protected_cleanup_escrow(&evidence).unwrap();

                    let names = ProtectedCleanupLocalNamesV1::new(&evidence);
                    let target_name = match target {
                        "marker" => names.marker.clone(),
                        "inputs" => names.inputs.clone(),
                        _ => unreachable!(),
                    };
                    let target_path = directory.0.join(&target_name);
                    let backup_path = directory
                        .0
                        .join(format!("hostile-protected-cleanup-backup-{case}"));
                    let extra_path = directory
                        .0
                        .join(format!("hostile-protected-cleanup-link-{case}"));
                    let exact_bytes = fs::read(&target_path).unwrap();
                    let expected_target_bytes = exact_bytes.clone();
                    let mutate_target_path = target_path.clone();
                    let mutate_backup_path = backup_path.clone();
                    let mutate_extra_path = extra_path.clone();
                    let mutate = move || match attack {
                        "replacement" => {
                            fs::rename(&mutate_target_path, &mutate_backup_path).unwrap();
                            fs::write(&mutate_target_path, &exact_bytes).unwrap();
                            fs::set_permissions(
                                &mutate_target_path,
                                fs::Permissions::from_mode(0o600),
                            )
                            .unwrap();
                        }
                        "hardlink" => {
                            fs::hard_link(&mutate_target_path, &mutate_extra_path).unwrap()
                        }
                        "symlink" => {
                            fs::rename(&mutate_target_path, &mutate_backup_path).unwrap();
                            symlink(&mutate_backup_path, &mutate_target_path).unwrap();
                        }
                        _ => unreachable!(),
                    };
                    if racing {
                        install_protected_cleanup_before_unlink_test_hook_v1(target_name, mutate);
                    } else {
                        mutate();
                    }

                    assert!(
                        store
                            .finish_committed_protected_cleanup_local_unlink(&evidence)
                            .is_err(),
                        "accepted {attack} attack on {target} (racing={racing})"
                    );
                    PROTECTED_CLEANUP_BEFORE_UNLINK_TEST_HOOK_V1.with(|installed| {
                        assert!(
                            installed.borrow().is_none(),
                            "unlink test hook was not consumed"
                        );
                    });
                    match attack {
                        "replacement" => {
                            assert_eq!(fs::read(&target_path).unwrap(), expected_target_bytes);
                            assert!(backup_path.is_file());
                        }
                        "hardlink" => {
                            assert!(target_path.is_file());
                            assert!(extra_path.is_file());
                        }
                        "symlink" => {
                            assert!(
                                fs::symlink_metadata(&target_path)
                                    .unwrap()
                                    .file_type()
                                    .is_symlink()
                            );
                            assert!(backup_path.is_file());
                        }
                        _ => unreachable!(),
                    }
                    assert_eq!(
                        store.read_protected_cleanup_evidence().unwrap(),
                        Some(evidence),
                        "retired evidence after {attack} attack on {target} (racing={racing})"
                    );
                    assert_eq!(
                        recover_worker_v2_publication_intent_cleanup_escrow_v1(
                            &directory.0,
                            &producer,
                            evidence.escrow,
                        )
                        .unwrap(),
                        WorkerV2PublicationIntentCleanupEscrowStateV1::Committed
                    );
                }
            }
        }
    }

    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    #[test]
    fn protected_cleanup_reopens_every_intent_marker_and_input_boundary_after_hostile_envelope_removal_and_substitution()
     {
        let boundaries = [
            (
                "intent",
                "protected-cleanup-evidence-before-temp-create",
                false,
            ),
            (
                "intent",
                "protected-cleanup-evidence-after-temp-create",
                false,
            ),
            (
                "intent",
                "protected-cleanup-evidence-before-temp-write",
                false,
            ),
            (
                "intent",
                "protected-cleanup-evidence-after-temp-write",
                false,
            ),
            (
                "intent",
                "protected-cleanup-evidence-before-temp-sync",
                false,
            ),
            (
                "intent",
                "protected-cleanup-evidence-after-temp-sync",
                false,
            ),
            ("intent", "protected-cleanup-evidence-before-rename", false),
            ("intent", "protected-cleanup-evidence-after-rename", false),
            (
                "intent",
                "protected-cleanup-evidence-before-directory-sync",
                false,
            ),
            (
                "intent",
                "protected-cleanup-evidence-after-directory-sync",
                false,
            ),
            ("intent", "protected-cleanup-evidence-published", false),
            (
                "completed",
                "protected-cleanup-marker-before-quarantine-rename",
                false,
            ),
            (
                "completed",
                "protected-cleanup-marker-after-quarantine-rename",
                false,
            ),
            (
                "completed",
                "protected-cleanup-marker-quarantine-directory-synced",
                false,
            ),
            (
                "completed",
                "protected-cleanup-input-before-quarantine-rename",
                false,
            ),
            (
                "completed",
                "protected-cleanup-input-after-quarantine-rename",
                false,
            ),
            (
                "completed",
                "protected-cleanup-input-quarantine-directory-synced",
                false,
            ),
            (
                "completed",
                "protected-cleanup-before-artifact-escrow-commit",
                false,
            ),
            (
                "completed",
                "protected-cleanup-after-artifact-escrow-commit",
                true,
            ),
            ("completed", "protected-cleanup-marker-before-unlink", true),
            ("completed", "protected-cleanup-marker-after-unlink", true),
            (
                "completed",
                "protected-cleanup-marker-directory-synced",
                true,
            ),
            ("completed", "protected-cleanup-input-before-unlink", true),
            ("completed", "protected-cleanup-input-after-unlink", true),
            (
                "completed",
                "protected-cleanup-input-directory-synced",
                true,
            ),
        ];
        for (case, (action, fault, committed, remove_envelope)) in boundaries
            .into_iter()
            .flat_map(|(action, fault, committed)| {
                [
                    (action, fault, committed, true),
                    (action, fault, committed, false),
                ]
            })
            .enumerate()
        {
            let boundary_case = case / 2;
            let tamper_case = case % 2;
            let seed = 140 + case as u8;
            let directory = TestDirectory::new();
            let producer = producer(seed);
            let attempt = attempt(&directory.0, &producer, seed);
            let closure = compiler_closure(seed);
            let fixture = protected_envelope_fixture(&directory.0, &producer, attempt, closure);
            let store = WorkerV2ResumeStoreV2::open(&directory.0, &producer).unwrap();
            stage_protected_required_ready(&store, &fixture, closure);
            let completed = store
                .persist_envelope_and_completed(
                    WorkerV2PublicationKindV1::FinalizedEnvelopeRequired,
                    attempt,
                    fixture.intent,
                    fixture.receipt,
                    closure,
                    &fixture.envelope,
                )
                .unwrap();
            if action == "completed" {
                clear_worker_v2_intent_v2(&store, &producer, completed, fixture.receipt, closure)
                    .unwrap();
            }
            let marker_path = store
                .inner
                .display_path
                .join(store.inner.marker_name.as_str());
            drop(store);

            let output =
                run_protected_cleanup_crash_helper(&directory.0, seed, seed, action, fault);
            assert_eq!(output.status.code(), Some(86), "{fault}: {output:?}");

            let envelope_path = directory.0.join(protected_envelope_name_v2(
                fixture.receipt.publication_identity(),
            ));
            if remove_envelope {
                fs::remove_file(&envelope_path).unwrap();
            } else {
                let hostile = envelope_with_resealed_claim_substitution(
                    &fixture.envelope,
                    (boundary_case % 4) * 16,
                );
                fs::write(&envelope_path, hostile.to_bytes()).unwrap();
            }
            if committed {
                let restarted = WorkerV2ResumeStoreV2::open(&directory.0, &producer).unwrap();
                assert_eq!(restarted.load().unwrap(), None, "{fault}");
                assert!(
                    restarted
                        .read_protected_cleanup_evidence()
                        .unwrap()
                        .is_none(),
                    "{fault}"
                );
                assert!(!marker_path.exists(), "{fault}");
                assert!(matches!(
                    recover_worker_v2_publication_intent_v2(
                        &directory.0,
                        &producer,
                        attempt,
                        closure,
                    ),
                    Err(WorkerV2PublicationIntentErrorV2::NotFound)
                ));
            } else {
                let reopen = WorkerV2ResumeStoreV2::open(&directory.0, &producer);
                let reopen_error = reopen.err();
                assert!(
                    reopen_error.is_some(),
                    "{fault} accepted pre-commit envelope tamper case {tamper_case}"
                );
                assert!(
                    marker_path.is_file(),
                    "{fault} did not restore Completed for tamper case {tamper_case}: {reopen_error:?}"
                );
                fs::write(&envelope_path, fixture.envelope.to_bytes()).unwrap();
                fs::set_permissions(&envelope_path, fs::Permissions::from_mode(0o600)).unwrap();
                let restarted = WorkerV2ResumeStoreV2::open(&directory.0, &producer).unwrap();
                assert_eq!(restarted.load().unwrap(), Some(completed), "{fault}");
                assert_eq!(
                    restarted.recover_envelope_inputs(attempt).unwrap(),
                    fixture.envelope_inputs,
                    "{fault}"
                );
                let recovered = recover_worker_v2_publication_intent_v2(
                    &directory.0,
                    &producer,
                    attempt,
                    closure,
                )
                .unwrap();
                assert_eq!(recovered.record().identity(), fixture.intent, "{fault}");
                assert_eq!(recovered.exact_output(), fixture.output, "{fault}");
            }
        }
    }

    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    #[test]
    fn protected_cleanup_reopens_every_local_rollback_boundary() {
        for (case, fault) in [
            "protected-cleanup-input-before-rollback-rename",
            "protected-cleanup-input-after-rollback-rename",
            "protected-cleanup-input-rollback-directory-synced",
            "protected-cleanup-marker-before-rollback-rename",
            "protected-cleanup-marker-after-rollback-rename",
            "protected-cleanup-marker-rollback-directory-synced",
        ]
        .into_iter()
        .enumerate()
        {
            let seed = 232 + case as u8;
            let directory = TestDirectory::new();
            let producer = producer(seed);
            let attempt = attempt(&directory.0, &producer, seed);
            let closure = compiler_closure(seed);
            let fixture = protected_envelope_fixture(&directory.0, &producer, attempt, closure);
            let store = WorkerV2ResumeStoreV2::open(&directory.0, &producer).unwrap();
            stage_protected_required_ready(&store, &fixture, closure);
            let completed = store
                .persist_envelope_and_completed(
                    WorkerV2PublicationKindV1::FinalizedEnvelopeRequired,
                    attempt,
                    fixture.intent,
                    fixture.receipt,
                    closure,
                    &fixture.envelope,
                )
                .unwrap();
            clear_worker_v2_intent_v2(&store, &producer, completed, fixture.receipt, closure)
                .unwrap();
            let evidence = store.read_protected_cleanup_evidence().unwrap().unwrap();
            store
                .prepare_protected_cleanup_local_quarantine(&evidence)
                .unwrap();
            let names = ProtectedCleanupLocalNamesV1::new(&evidence);
            let marker_path = directory.0.join(&store.inner.marker_name);
            let inputs_path = directory
                .0
                .join(envelope_inputs_name(store.inner.package, attempt));
            assert!(!marker_path.exists());
            assert!(!inputs_path.exists());
            assert!(directory.0.join(&names.marker).is_file());
            assert!(directory.0.join(&names.inputs).is_file());
            drop(store);

            let envelope_path = directory.0.join(protected_envelope_name_v2(
                fixture.receipt.publication_identity(),
            ));
            fs::remove_file(&envelope_path).unwrap();
            let output =
                run_protected_cleanup_crash_helper(&directory.0, seed, seed, "completed", fault);
            assert_eq!(output.status.code(), Some(86), "{fault}: {output:?}");

            assert!(
                WorkerV2ResumeStoreV2::open(&directory.0, &producer).is_err(),
                "{fault} accepted a missing envelope while restoring Prepared cleanup"
            );
            assert!(marker_path.is_file(), "{fault}");
            assert!(inputs_path.is_file(), "{fault}");
            assert!(!directory.0.join(&names.marker).exists(), "{fault}");
            assert!(!directory.0.join(&names.inputs).exists(), "{fault}");

            fs::write(&envelope_path, fixture.envelope.to_bytes()).unwrap();
            fs::set_permissions(&envelope_path, fs::Permissions::from_mode(0o600)).unwrap();
            let restarted = WorkerV2ResumeStoreV2::open(&directory.0, &producer).unwrap();
            assert_eq!(restarted.load().unwrap(), Some(completed), "{fault}");
            assert_eq!(
                restarted.recover_envelope_inputs(attempt).unwrap(),
                fixture.envelope_inputs,
                "{fault}"
            );
            let recovered =
                recover_worker_v2_publication_intent_v2(&directory.0, &producer, attempt, closure)
                    .unwrap();
            assert_eq!(recovered.record().identity(), fixture.intent, "{fault}");
            assert_eq!(recovered.exact_output(), fixture.output, "{fault}");
            assert_eq!(
                restarted.read_protected_cleanup_evidence().unwrap(),
                Some(evidence),
                "{fault}"
            );
        }
    }

    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    #[test]
    fn durable_ready_recovery_completes_interrupted_predecessor_retirement() {
        let producer_seed = 168;
        let next_seed = 169;
        let directory = TestDirectory::new();
        let producer = producer(producer_seed);
        let old_attempt = attempt(&directory.0, &producer, producer_seed);
        let old_closure = compiler_closure(producer_seed);
        let old = protected_envelope_fixture(&directory.0, &producer, old_attempt, old_closure);
        let store = WorkerV2ResumeStoreV2::open(&directory.0, &producer).unwrap();
        stage_protected_required_ready(&store, &old, old_closure);
        let old_completed = store
            .persist_envelope_and_completed(
                WorkerV2PublicationKindV1::FinalizedEnvelopeRequired,
                old_attempt,
                old.intent,
                old.receipt,
                old_closure,
                &old.envelope,
            )
            .unwrap();
        clear_worker_v2_intent_v2(&store, &producer, old_completed, old.receipt, old_closure)
            .unwrap();
        let old_evidence = store.read_protected_cleanup_evidence().unwrap().unwrap();
        let old_local_names = ProtectedCleanupLocalNamesV1::new(&old_evidence);
        store
            .prepare_protected_cleanup_local_quarantine(&old_evidence)
            .unwrap();
        assert_eq!(store.load().unwrap(), None);
        assert_eq!(
            recover_worker_v2_publication_intent_cleanup_escrow_v1(
                &directory.0,
                &producer,
                old_evidence.escrow,
            )
            .unwrap(),
            WorkerV2PublicationIntentCleanupEscrowStateV1::Prepared
        );

        let next_attempt = attempt(&directory.0, &producer, next_seed);
        let next_closure = compiler_closure(next_seed);
        let next = protected_envelope_fixture_with_binding_seed(
            &directory.0,
            &producer,
            next_attempt,
            next_closure,
            producer_seed,
        );
        store
            .persist_envelope_inputs(next_attempt, &next.envelope_inputs)
            .unwrap();
        store
            .persist_pending_with_envelope_inputs(
                WorkerV2PublicationKindV1::FinalizedEnvelopeRequired,
                next_attempt,
                restart_admission_commitment_with_inputs_v2(
                    WorkerV2PublicationKindV1::FinalizedEnvelopeRequired,
                    next.plan,
                    next.upstream,
                    &next.output,
                    Some(next.envelope_inputs.identity()),
                    next_closure,
                ),
                Some(next.envelope_inputs.identity()),
            )
            .unwrap();
        drop(store);

        let output = run_protected_cleanup_crash_helper(
            &directory.0,
            producer_seed,
            next_seed,
            "ready",
            "protected-ready-after-marker-publication",
        );
        assert_eq!(output.status.code(), Some(86), "{output:?}");

        let restarted = WorkerV2ResumeStoreV2::open(&directory.0, &producer).unwrap();
        let ready = restarted.load().unwrap().unwrap();
        assert!(matches!(ready, ResumeMarkerStateV2::Ready { .. }));
        assert_eq!(ready.attempt(), next_attempt);
        assert_eq!(
            restarted.read_protected_cleanup_evidence().unwrap(),
            Some(old_evidence)
        );
        assert_eq!(
            recover_worker_v2_publication_intent_cleanup_escrow_v1(
                &directory.0,
                &producer,
                old_evidence.escrow,
            )
            .unwrap(),
            WorkerV2PublicationIntentCleanupEscrowStateV1::Prepared
        );

        let recovered =
            recover_worker_v2_intent_v2(&restarted, &producer, ready, next_closure).unwrap();
        assert_eq!(recovered.record().identity(), next.intent);
        assert_eq!(recovered.exact_output(), next.output);
        assert_eq!(restarted.load().unwrap(), Some(ready));
        assert!(
            restarted
                .read_protected_cleanup_evidence()
                .unwrap()
                .is_none()
        );
        assert_eq!(
            recover_worker_v2_publication_intent_cleanup_escrow_v1(
                &directory.0,
                &producer,
                old_evidence.escrow,
            )
            .unwrap(),
            WorkerV2PublicationIntentCleanupEscrowStateV1::Committed
        );
        assert!(!directory.0.join(old_local_names.marker).exists());
        assert!(!directory.0.join(old_local_names.inputs).exists());

        let completed = restarted
            .persist_envelope_and_completed(
                WorkerV2PublicationKindV1::FinalizedEnvelopeRequired,
                next_attempt,
                next.intent,
                next.receipt,
                next_closure,
                &next.envelope,
            )
            .unwrap();
        clear_worker_v2_intent_v2(&restarted, &producer, completed, next.receipt, next_closure)
            .unwrap();
        assert_eq!(restarted.load().unwrap(), Some(completed));
        assert!(
            restarted
                .read_protected_cleanup_evidence()
                .unwrap()
                .is_some()
        );
        restarted
            .clear_completed_and_envelope_inputs(completed, next.receipt, next_closure)
            .unwrap();
        assert_eq!(restarted.load().unwrap(), None);
    }

    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    #[test]
    fn protected_cleanup_evidence_retirement_reopens_every_boundary_with_a_new_ready_state() {
        for (case, (fault, remove_envelope)) in [
            "protected-cleanup-newer-ready-after-predecessor-commit",
            "protected-cleanup-newer-ready-after-successor-lease-release",
            "protected-cleanup-newer-ready-before-local-cleanup",
            "protected-cleanup-newer-ready-after-local-cleanup",
            "protected-cleanup-evidence-before-unlink",
            "protected-cleanup-evidence-after-unlink",
            "protected-cleanup-evidence-directory-synced",
        ]
        .into_iter()
        .flat_map(|fault| [(fault, true), (fault, false)])
        .enumerate()
        {
            let boundary_case = case / 2;
            let producer_seed = 170 + case as u8 * 2;
            let next_seed = producer_seed + 1;
            let directory = TestDirectory::new();
            let producer = producer(producer_seed);
            let old_attempt = attempt(&directory.0, &producer, producer_seed);
            let old_closure = compiler_closure(producer_seed);
            let old = protected_envelope_fixture(&directory.0, &producer, old_attempt, old_closure);
            let store = WorkerV2ResumeStoreV2::open(&directory.0, &producer).unwrap();
            stage_protected_required_ready(&store, &old, old_closure);
            let old_completed = store
                .persist_envelope_and_completed(
                    WorkerV2PublicationKindV1::FinalizedEnvelopeRequired,
                    old_attempt,
                    old.intent,
                    old.receipt,
                    old_closure,
                    &old.envelope,
                )
                .unwrap();
            clear_worker_v2_intent_v2(&store, &producer, old_completed, old.receipt, old_closure)
                .unwrap();
            let old_evidence = store.read_protected_cleanup_evidence().unwrap().unwrap();
            store
                .prepare_protected_cleanup_local_quarantine(&old_evidence)
                .unwrap();
            assert_eq!(store.load().unwrap(), None);
            assert_eq!(
                recover_worker_v2_publication_intent_cleanup_escrow_v1(
                    &directory.0,
                    &producer,
                    old_evidence.escrow,
                )
                .unwrap(),
                WorkerV2PublicationIntentCleanupEscrowStateV1::Prepared
            );

            let next_attempt = attempt(&directory.0, &producer, next_seed);
            let next_closure = compiler_closure(next_seed);
            let next = protected_envelope_fixture_with_binding_seed(
                &directory.0,
                &producer,
                next_attempt,
                next_closure,
                producer_seed,
            );
            store
                .persist_envelope_inputs(next_attempt, &next.envelope_inputs)
                .unwrap();
            store
                .persist_pending_with_envelope_inputs(
                    WorkerV2PublicationKindV1::FinalizedEnvelopeRequired,
                    next_attempt,
                    restart_admission_commitment_with_inputs_v2(
                        WorkerV2PublicationKindV1::FinalizedEnvelopeRequired,
                        next.plan,
                        next.upstream,
                        &next.output,
                        Some(next.envelope_inputs.identity()),
                        next_closure,
                    ),
                    Some(next.envelope_inputs.identity()),
                )
                .unwrap();
            drop(store);

            let output = run_protected_cleanup_crash_helper(
                &directory.0,
                producer_seed,
                next_seed,
                "ready",
                fault,
            );
            assert_eq!(output.status.code(), Some(86), "{fault}: {output:?}");
            let old_envelope_path = directory.0.join(protected_envelope_name_v2(
                old.receipt.publication_identity(),
            ));
            if remove_envelope {
                fs::remove_file(old_envelope_path).unwrap();
            } else {
                let hostile = envelope_with_resealed_claim_substitution(
                    &old.envelope,
                    (boundary_case % 4) * 16,
                );
                fs::write(old_envelope_path, hostile.to_bytes()).unwrap();
            }

            let restarted = WorkerV2ResumeStoreV2::open(&directory.0, &producer).unwrap();
            let ready = restarted.load().unwrap().unwrap();
            assert!(
                matches!(ready, ResumeMarkerStateV2::Ready { .. }),
                "{fault}"
            );
            assert_eq!(ready.attempt(), next_attempt, "{fault}");
            let recovered =
                recover_worker_v2_intent_v2(&restarted, &producer, ready, next_closure).unwrap();
            assert_eq!(recovered.record().identity(), next.intent, "{fault}");
            assert_eq!(recovered.exact_output(), next.output, "{fault}");
            assert!(
                restarted
                    .read_protected_cleanup_evidence()
                    .unwrap()
                    .is_none(),
                "{fault}"
            );
            assert_eq!(restarted.load().unwrap(), Some(ready), "{fault}");
            assert_eq!(
                recover_worker_v2_publication_intent_cleanup_escrow_v1(
                    &directory.0,
                    &producer,
                    old_evidence.escrow,
                )
                .unwrap(),
                WorkerV2PublicationIntentCleanupEscrowStateV1::Committed,
                "{fault}"
            );
        }
    }

    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    #[test]
    fn newer_ready_retirement_fails_closed_when_current_intent_changes_after_old_commit() {
        let producer_seed = 210;
        let next_seed = 211;
        let directory = TestDirectory::new();
        let producer = producer(producer_seed);
        let old_attempt = attempt(&directory.0, &producer, producer_seed);
        let old_closure = compiler_closure(producer_seed);
        let old = protected_envelope_fixture(&directory.0, &producer, old_attempt, old_closure);
        let store = WorkerV2ResumeStoreV2::open(&directory.0, &producer).unwrap();
        stage_protected_required_ready(&store, &old, old_closure);
        let old_completed = store
            .persist_envelope_and_completed(
                WorkerV2PublicationKindV1::FinalizedEnvelopeRequired,
                old_attempt,
                old.intent,
                old.receipt,
                old_closure,
                &old.envelope,
            )
            .unwrap();
        clear_worker_v2_intent_v2(&store, &producer, old_completed, old.receipt, old_closure)
            .unwrap();
        let old_evidence = store.read_protected_cleanup_evidence().unwrap().unwrap();
        store
            .prepare_protected_cleanup_local_quarantine(&old_evidence)
            .unwrap();

        let next_attempt = attempt(&directory.0, &producer, next_seed);
        let next_closure = compiler_closure(next_seed);
        let next = protected_envelope_fixture_with_binding_seed(
            &directory.0,
            &producer,
            next_attempt,
            next_closure,
            producer_seed,
        );
        store
            .persist_envelope_inputs(next_attempt, &next.envelope_inputs)
            .unwrap();
        store
            .persist_pending_with_envelope_inputs(
                WorkerV2PublicationKindV1::FinalizedEnvelopeRequired,
                next_attempt,
                restart_admission_commitment_with_inputs_v2(
                    WorkerV2PublicationKindV1::FinalizedEnvelopeRequired,
                    next.plan,
                    next.upstream,
                    &next.output,
                    Some(next.envelope_inputs.identity()),
                    next_closure,
                ),
                Some(next.envelope_inputs.identity()),
            )
            .unwrap();
        let next_recovered = recover_worker_v2_publication_intent_v2(
            &directory.0,
            &producer,
            next_attempt,
            next_closure,
        )
        .unwrap();
        drop(store);

        let output = run_protected_cleanup_crash_helper(
            &directory.0,
            producer_seed,
            next_seed,
            "ready",
            "protected-cleanup-newer-ready-after-predecessor-commit",
        );
        assert_eq!(output.status.code(), Some(86), "{output:?}");
        assert_eq!(
            recover_worker_v2_publication_intent_cleanup_escrow_v1(
                &directory.0,
                &producer,
                old_evidence.escrow,
            )
            .unwrap(),
            WorkerV2PublicationIntentCleanupEscrowStateV1::Committed
        );
        clear_worker_v2_publication_intent_v2(
            &directory.0,
            &producer,
            next_attempt,
            next_closure,
            next.intent,
        )
        .unwrap();

        let restarted = WorkerV2ResumeStoreV2::open(&directory.0, &producer).unwrap();
        let ready = restarted.load().unwrap().unwrap();
        assert!(matches!(ready, ResumeMarkerStateV2::Ready { .. }));
        assert!(
            restarted
                .persist_ready(
                    WorkerV2PublicationKindV1::FinalizedEnvelopeRequired,
                    next_attempt,
                    &next_recovered,
                    next_closure,
                )
                .is_err()
        );
        assert!(
            restarted
                .read_protected_cleanup_evidence()
                .unwrap()
                .is_none()
        );
        assert_eq!(restarted.load().unwrap(), Some(ready));
    }

    #[test]
    fn protected_v2_restart_apis_reject_a_deputy_for_another_producer() {
        let seed = 139;
        let directory = TestDirectory::new();
        let deputy = producer(seed + 1);
        let producer = producer(seed);
        let attempt = attempt(&directory.0, &producer, seed);
        let closure = compiler_closure(seed);
        let fixture = protected_envelope_fixture(&directory.0, &producer, attempt, closure);
        let store = WorkerV2ResumeStoreV2::open(&directory.0, &producer).unwrap();
        stage_protected_required_ready(&store, &fixture, closure);
        let ready = store.load().unwrap().unwrap();

        assert!(matches!(
            recover_worker_v2_intent_v2(&store, &deputy, ready, closure),
            Err(RestartIntentErrorV2::IntentIdentityMismatch)
        ));
        assert_eq!(store.load().unwrap(), Some(ready));
        let completed = store
            .persist_envelope_and_completed(
                WorkerV2PublicationKindV1::FinalizedEnvelopeRequired,
                attempt,
                fixture.intent,
                fixture.receipt,
                closure,
                &fixture.envelope,
            )
            .unwrap();
        assert!(matches!(
            clear_worker_v2_intent_v2(&store, &deputy, completed, fixture.receipt, closure,),
            Err(RestartIntentErrorV2::IntentIdentityMismatch)
        ));
        assert_eq!(store.load().unwrap(), Some(completed));
        assert!(
            recover_worker_v2_publication_intent_v2(&directory.0, &producer, attempt, closure,)
                .is_ok()
        );
    }

    #[test]
    fn protected_v2_temp_cleanup_is_schema_separated_from_ordinary_v1() {
        let directory = TestDirectory::new();
        let producer = producer(124);
        let ordinary = WorkerV2ResumeStoreV1::open(&directory.0, &producer).unwrap();
        let package = ordinary.package;
        drop(ordinary);
        let protected_temp = protected_envelope_temp_name_v2(package, [0x91; 32], 1, 1);
        let ordinary_temp = envelope_temp_name(package, [0x92; 32], 1, 1);
        for name in [&protected_temp, &ordinary_temp] {
            fs::write(directory.0.join(name), b"abandoned").unwrap();
            fs::set_permissions(directory.0.join(name), fs::Permissions::from_mode(0o600)).unwrap();
        }

        let protected = WorkerV2ResumeStoreV2::open(&directory.0, &producer).unwrap();
        assert_eq!(protected.load().unwrap(), None);
        assert!(!directory.0.join(protected_temp).exists());
        assert!(directory.0.join(ordinary_temp).exists());
    }

    #[test]
    fn protected_cleanup_evidence_rejects_malformed_files_and_symlinks_without_removal() {
        let malformed_directory = TestDirectory::new();
        let malformed_producer = producer(160);
        let store =
            WorkerV2ResumeStoreV2::open(&malformed_directory.0, &malformed_producer).unwrap();
        let name = protected_cleanup_evidence_name_v1(store.inner.package);
        drop(store);
        let malformed = b"not protected cleanup evidence";
        fs::write(malformed_directory.0.join(&name), malformed).unwrap();
        fs::set_permissions(
            malformed_directory.0.join(&name),
            fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        assert!(WorkerV2ResumeStoreV2::open(&malformed_directory.0, &malformed_producer).is_err());
        assert_eq!(
            fs::read(malformed_directory.0.join(name)).unwrap(),
            malformed
        );

        let symlink_directory = TestDirectory::new();
        let symlink_producer = producer(161);
        let store = WorkerV2ResumeStoreV2::open(&symlink_directory.0, &symlink_producer).unwrap();
        let name = protected_cleanup_evidence_name_v1(store.inner.package);
        drop(store);
        fs::write(symlink_directory.0.join("outside"), b"outside").unwrap();
        symlink("outside", symlink_directory.0.join(&name)).unwrap();
        assert!(WorkerV2ResumeStoreV2::open(&symlink_directory.0, &symlink_producer).is_err());
        assert!(
            fs::symlink_metadata(symlink_directory.0.join(name))
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(
            fs::read(symlink_directory.0.join("outside")).unwrap(),
            b"outside"
        );
    }

    #[test]
    fn protected_cleanup_temp_residue_cleanup_is_bounded_and_rejects_symlinks() {
        let exact_directory = TestDirectory::new();
        let exact_producer = producer(162);
        let store = WorkerV2ResumeStoreV2::open(&exact_directory.0, &exact_producer).unwrap();
        let package = store.inner.package;
        drop(store);
        for index in 0..MAX_PROTECTED_CLEANUP_TEMP_RESIDUE_ENTRIES_V1 {
            let name = protected_cleanup_evidence_temp_name_v1(package, 1, index as u64 + 1);
            fs::write(exact_directory.0.join(&name), b"residue").unwrap();
            fs::set_permissions(
                exact_directory.0.join(name),
                fs::Permissions::from_mode(0o600),
            )
            .unwrap();
        }
        let reopened = WorkerV2ResumeStoreV2::open(&exact_directory.0, &exact_producer).unwrap();
        assert_eq!(reopened.load().unwrap(), None);
        let prefix = protected_cleanup_evidence_temp_prefix_v1(package);
        assert_eq!(
            fs::read_dir(&exact_directory.0)
                .unwrap()
                .filter(|entry| entry
                    .as_ref()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(&prefix))
                .count(),
            0
        );

        let overflow_directory = TestDirectory::new();
        let overflow_producer = producer(163);
        let store = WorkerV2ResumeStoreV2::open(&overflow_directory.0, &overflow_producer).unwrap();
        let package = store.inner.package;
        drop(store);
        for index in 0..=MAX_PROTECTED_CLEANUP_TEMP_RESIDUE_ENTRIES_V1 {
            let name = protected_cleanup_evidence_temp_name_v1(package, 2, index as u64 + 1);
            fs::write(overflow_directory.0.join(&name), b"residue").unwrap();
            fs::set_permissions(
                overflow_directory.0.join(name),
                fs::Permissions::from_mode(0o600),
            )
            .unwrap();
        }
        assert!(WorkerV2ResumeStoreV2::open(&overflow_directory.0, &overflow_producer).is_err());
        let prefix = protected_cleanup_evidence_temp_prefix_v1(package);
        assert_eq!(
            fs::read_dir(&overflow_directory.0)
                .unwrap()
                .filter(|entry| entry
                    .as_ref()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(&prefix))
                .count(),
            MAX_PROTECTED_CLEANUP_TEMP_RESIDUE_ENTRIES_V1 + 1
        );

        let symlink_directory = TestDirectory::new();
        let symlink_producer = producer(164);
        let store = WorkerV2ResumeStoreV2::open(&symlink_directory.0, &symlink_producer).unwrap();
        let package = store.inner.package;
        drop(store);
        fs::write(symlink_directory.0.join("outside"), b"outside").unwrap();
        let name = protected_cleanup_evidence_temp_name_v1(package, 3, 1);
        symlink("outside", symlink_directory.0.join(&name)).unwrap();
        assert!(WorkerV2ResumeStoreV2::open(&symlink_directory.0, &symlink_producer).is_err());
        assert!(
            fs::symlink_metadata(symlink_directory.0.join(name))
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }

    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    #[test]
    fn protected_cleanup_evidence_temp_unlink_reopens_every_boundary() {
        for (case, fault) in [
            "protected-cleanup-evidence-temp-before-unlink",
            "protected-cleanup-evidence-temp-after-unlink",
            "protected-cleanup-evidence-temp-directory-synced",
        ]
        .into_iter()
        .enumerate()
        {
            let seed = 180 + case as u8;
            let directory = TestDirectory::new();
            let producer = producer(seed);
            let attempt = attempt(&directory.0, &producer, seed);
            let closure = compiler_closure(seed);
            let fixture = protected_envelope_fixture(&directory.0, &producer, attempt, closure);
            let store = WorkerV2ResumeStoreV2::open(&directory.0, &producer).unwrap();
            stage_protected_required_ready(&store, &fixture, closure);
            let package = store.inner.package;
            let ready = store.load().unwrap().unwrap();
            drop(store);

            let residue_name = protected_cleanup_evidence_temp_name_v1(
                package,
                std::process::id(),
                case as u64 + 1,
            );
            let residue_path = directory.0.join(&residue_name);
            fs::write(&residue_path, b"bounded residue").unwrap();
            fs::set_permissions(&residue_path, fs::Permissions::from_mode(0o600)).unwrap();

            let output =
                run_protected_cleanup_crash_helper(&directory.0, seed, seed, "ready", fault);
            assert_eq!(output.status.code(), Some(86), "{fault}: {output:?}");
            assert_eq!(
                residue_path.exists(),
                fault == "protected-cleanup-evidence-temp-before-unlink",
                "unexpected residue state at {fault}"
            );

            let restarted = WorkerV2ResumeStoreV2::open(&directory.0, &producer).unwrap();
            assert_eq!(restarted.load().unwrap(), Some(ready), "{fault}");
            assert!(!residue_path.exists(), "{fault}");
        }
    }

    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    #[test]
    fn protected_cleanup_crash_subprocess_helper() {
        let Some(directory) = std::env::var_os(CLEANUP_CRASH_HELPER_DIRECTORY_ENV) else {
            return;
        };
        let producer_seed = std::env::var(CLEANUP_CRASH_HELPER_PRODUCER_SEED_ENV)
            .unwrap()
            .parse::<u8>()
            .unwrap();
        let closure_seed = std::env::var(CLEANUP_CRASH_HELPER_CLOSURE_SEED_ENV)
            .unwrap()
            .parse::<u8>()
            .unwrap();
        let producer = producer(producer_seed);
        let closure = compiler_closure(closure_seed);
        let store = WorkerV2ResumeStoreV2::open(Path::new(&directory), &producer).unwrap();
        let state = store.load().unwrap().unwrap();
        match std::env::var(CLEANUP_CRASH_HELPER_ACTION_ENV)
            .unwrap()
            .as_str()
        {
            "intent" => {
                let receipt = store.durable_receipt(state.attempt()).unwrap();
                clear_worker_v2_intent_v2(&store, &producer, state, receipt, closure).unwrap();
            }
            "completed" => {
                let receipt = store.durable_receipt(state.attempt()).unwrap();
                store
                    .clear_completed_and_envelope_inputs(state, receipt, closure)
                    .unwrap();
            }
            "ready" => {
                let recovered = recover_worker_v2_publication_intent_v2(
                    Path::new(&directory),
                    &producer,
                    state.attempt(),
                    closure,
                )
                .unwrap();
                store
                    .persist_ready(state.publication(), state.attempt(), &recovered, closure)
                    .unwrap();
            }
            action => panic!("unknown protected cleanup crash action {action}"),
        }
    }

    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    #[test]
    fn restart_marker_crash_subprocess_helper() {
        let Some(directory) = std::env::var_os(MARKER_CRASH_HELPER_DIRECTORY_ENV) else {
            return;
        };
        let attempt =
            BuildAttempt::from_env_value(&std::env::var(MARKER_CRASH_HELPER_ATTEMPT_ENV).unwrap())
                .unwrap();
        let seed = std::env::var(MARKER_CRASH_HELPER_SEED_ENV)
            .unwrap()
            .parse::<u8>()
            .unwrap();
        let producer = producer(seed);
        let store = WorkerV2ResumeStoreV2::open(Path::new(&directory), &producer).unwrap();
        store
            .persist_pending(WorkerV2PublicationKindV1::Finalized, attempt, [0x33; 32])
            .unwrap();
    }

    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    #[test]
    fn marker_temp_sync_crash_is_cleaned_before_restart_state_is_read() {
        let directory = TestDirectory::new();
        let seed = 125;
        let producer = producer(seed);
        let attempt = attempt(&directory.0, &producer, seed);
        let store = WorkerV2ResumeStoreV2::open(&directory.0, &producer).unwrap();
        let marker_name = store.inner.marker_name.clone();
        let temp_prefix = marker_temp_prefix(&marker_name);
        drop(store);

        let output =
            run_marker_crash_helper(&directory.0, attempt, seed, "resume-marker-temp-synced");
        assert_eq!(output.status.code(), Some(86), "{output:?}");
        assert!(!directory.0.join(&marker_name).exists());
        let residue = fs::read_dir(&directory.0)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .filter(|name| name.to_string_lossy().starts_with(&temp_prefix))
            .collect::<Vec<_>>();
        assert_eq!(residue.len(), 1);

        let restarted = WorkerV2ResumeStoreV2::open(&directory.0, &producer).unwrap();
        assert_eq!(restarted.load().unwrap(), None);
        assert!(
            fs::read_dir(&directory.0)
                .unwrap()
                .map(|entry| entry.unwrap().file_name())
                .all(|name| !name.to_string_lossy().starts_with(&temp_prefix))
        );
    }

    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    #[test]
    fn marker_rename_crash_recovers_the_exact_published_state_idempotently() {
        let directory = TestDirectory::new();
        let seed = 126;
        let producer = producer(seed);
        let attempt = attempt(&directory.0, &producer, seed);
        let store = WorkerV2ResumeStoreV2::open(&directory.0, &producer).unwrap();
        let marker_name = store.inner.marker_name.clone();
        let temp_prefix = marker_temp_prefix(&marker_name);
        drop(store);

        let output = run_marker_crash_helper(&directory.0, attempt, seed, "resume-marker-renamed");
        assert_eq!(output.status.code(), Some(86), "{output:?}");
        assert!(directory.0.join(&marker_name).is_file());
        assert!(
            fs::read_dir(&directory.0)
                .unwrap()
                .map(|entry| entry.unwrap().file_name())
                .all(|name| !name.to_string_lossy().starts_with(&temp_prefix))
        );

        let restarted = WorkerV2ResumeStoreV2::open(&directory.0, &producer).unwrap();
        let pending = ResumeMarkerStateV2::Pending {
            publication: WorkerV2PublicationKindV1::Finalized,
            attempt,
            admission: [0x33; 32],
            envelope_inputs: [0; 32],
        };
        assert_eq!(restarted.load().unwrap(), Some(pending));
        restarted
            .persist_pending(WorkerV2PublicationKindV1::Finalized, attempt, [0x33; 32])
            .unwrap();
        assert_eq!(restarted.load().unwrap(), Some(pending));
    }

    #[test]
    fn ordinary_and_protected_stores_reject_each_others_markers() {
        let ordinary_directory = TestDirectory::new();
        let ordinary_producer = producer(103);
        let ordinary_attempt = attempt(&ordinary_directory.0, &ordinary_producer, 103);
        let ordinary_store =
            WorkerV2ResumeStoreV1::open(&ordinary_directory.0, &ordinary_producer).unwrap();
        ordinary_store
            .persist_pending(
                WorkerV2PublicationKindV1::Finalized,
                ordinary_attempt,
                [0x33; 32],
            )
            .unwrap();
        drop(ordinary_store);
        assert!(WorkerV2ResumeStoreV2::open(&ordinary_directory.0, &ordinary_producer).is_err());

        let protected_directory = TestDirectory::new();
        let protected_producer = producer(104);
        let protected_attempt = attempt(&protected_directory.0, &protected_producer, 104);
        let protected_store =
            WorkerV2ResumeStoreV2::open(&protected_directory.0, &protected_producer).unwrap();
        protected_store
            .persist_pending(
                WorkerV2PublicationKindV1::Finalized,
                protected_attempt,
                [0x33; 32],
            )
            .unwrap();
        drop(protected_store);
        assert!(WorkerV2ResumeStoreV1::open(&protected_directory.0, &protected_producer).is_err());
    }

    #[test]
    fn protected_intent_crash_retry_promotes_only_after_exact_v2_intent() {
        let before_intent = TestDirectory::new();
        let before_producer = producer(107);
        let before_attempt = attempt(&before_intent.0, &before_producer, 107);
        let (before_output, before_plan, before_upstream) = publication_inputs(before_attempt, 107);
        let closure = compiler_closure(107);
        let publication = WorkerV2PublicationKindV1::Finalized;
        let before_store = WorkerV2ResumeStoreV2::open(&before_intent.0, &before_producer).unwrap();
        let before_admission = restart_admission_commitment_with_inputs_v2(
            publication,
            before_plan,
            before_upstream,
            &before_output,
            None,
            closure,
        );
        before_store
            .persist_pending(publication, before_attempt, before_admission)
            .unwrap();
        let before_state = before_store.load().unwrap().unwrap();
        assert!(matches!(before_state, ResumeMarkerStateV2::Pending { .. }));
        assert!(matches!(
            recover_worker_v2_intent_v2(&before_store, &before_producer, before_state, closure,),
            Err(RestartIntentErrorV2::Intent(
                WorkerV2PublicationIntentErrorV2::NotFound
            ))
        ));
        assert_eq!(before_store.load().unwrap(), Some(before_state));
        before_store.clear_abandoned_pending(before_state).unwrap();

        let after_intent = TestDirectory::new();
        let after_producer = producer(108);
        let after_attempt = attempt(&after_intent.0, &after_producer, 108);
        let (after_output, after_plan, after_upstream) = publication_inputs(after_attempt, 108);
        let after_store = WorkerV2ResumeStoreV2::open(&after_intent.0, &after_producer).unwrap();
        let after_admission = restart_admission_commitment_with_inputs_v2(
            publication,
            after_plan,
            after_upstream,
            &after_output,
            None,
            closure,
        );
        after_store
            .persist_pending(publication, after_attempt, after_admission)
            .unwrap();
        let durable = persist_worker_v2_publication_intent_v2(
            &after_intent.0,
            &after_producer,
            after_attempt,
            after_plan,
            after_upstream,
            closure,
            &after_output,
        )
        .unwrap();
        let intent = durable.record().identity();
        drop(durable);

        let retried = persist_admitted_worker_v2_intent_v2(
            &after_store,
            &after_producer,
            publication,
            after_plan,
            after_upstream,
            &after_output,
            None,
            closure,
        )
        .unwrap();
        assert_eq!(retried.publication, publication);
        assert_eq!(retried.intent.record().identity(), intent);
        assert_eq!(retried.intent.compiler_closure(), closure);
        assert_eq!(retried.intent.exact_output(), after_output);
        assert_eq!(
            retried.intent.outcome(),
            fe2o3_artifact_transaction::WorkerV2PublicationIntentOutcomeV2::Recovered
        );
        assert_eq!(
            after_store.load().unwrap(),
            Some(ResumeMarkerStateV2::Ready {
                publication,
                attempt: after_attempt,
                admission: after_admission,
                envelope_inputs: [0; 32],
                intent,
            })
        );
    }

    #[test]
    fn protected_ready_replay_completion_and_marker_clear_require_the_exact_closure() {
        let directory = TestDirectory::new();
        let producer = producer(109);
        let attempt = attempt(&directory.0, &producer, 109);
        let (output, plan, upstream) = publication_inputs(attempt, 109);
        let closure = compiler_closure(109);
        let wrong_closure = compiler_closure(110);
        let publication = WorkerV2PublicationKindV1::Finalized;
        let store = WorkerV2ResumeStoreV2::open(&directory.0, &producer).unwrap();
        let persisted = persist_admitted_worker_v2_intent_v2(
            &store,
            &producer,
            publication,
            plan,
            upstream,
            &output,
            None,
            closure,
        )
        .unwrap();
        let intent = persisted.intent.record().identity();
        let ready = store.load().unwrap().unwrap();
        assert!(matches!(
            recover_worker_v2_intent_v2(&store, &producer, ready, wrong_closure),
            Err(RestartIntentErrorV2::Intent(
                WorkerV2PublicationIntentErrorV2::CompilerClosureMismatch
            ))
        ));
        assert_eq!(store.load().unwrap(), Some(ready));

        let recovered = recover_worker_v2_intent_v2(&store, &producer, ready, closure).unwrap();
        assert_eq!(recovered.record().identity(), intent);
        assert_eq!(recovered.compiler_closure(), closure);
        assert_eq!(recovered.exact_output(), output);
        let receipt = publish_exact_hsaco_evidence_for_attempt_v2(
            &directory.0,
            &producer,
            attempt,
            plan,
            upstream,
            closure,
            &output,
        )
        .unwrap()
        .receipt();
        let completed = store
            .persist_completed(publication, attempt, intent, receipt, closure)
            .unwrap();
        assert!(matches!(
            clear_worker_v2_intent_v2(&store, &producer, completed, receipt, wrong_closure,),
            Err(RestartIntentErrorV2::Marker(
                ResumeMarkerErrorV1::InvalidTransition
            ))
        ));
        assert!(
            recover_worker_v2_publication_intent_v2(&directory.0, &producer, attempt, closure,)
                .is_ok()
        );
        assert!(matches!(
            store.clear_completed(completed, receipt, wrong_closure),
            Err(ResumeMarkerErrorV1::InvalidTransition)
        ));
        assert_eq!(store.load().unwrap(), Some(completed));
        store.clear_completed(completed, receipt, closure).unwrap();
        assert_eq!(store.load().unwrap(), None);
    }

    #[test]
    fn protected_recovery_rejects_stale_attempt_without_changing_marker() {
        let directory = TestDirectory::new();
        let producer = producer(111);
        let current = attempt(&directory.0, &producer, 111);
        let stale = BuildAttempt::from_env_value(&format!(
            "{}:{}:{}",
            current.generation() + 1,
            current.session().to_hex(),
            current.invocation().to_hex()
        ))
        .unwrap();
        let (output, plan, upstream) = publication_inputs(current, 111);
        let closure = compiler_closure(111);
        persist_worker_v2_publication_intent_v2(
            &directory.0,
            &producer,
            current,
            plan,
            upstream,
            closure,
            &output,
        )
        .unwrap();
        let publication = WorkerV2PublicationKindV1::Finalized;
        let store = WorkerV2ResumeStoreV2::open(&directory.0, &producer).unwrap();
        store
            .persist_pending(
                publication,
                stale,
                restart_admission_commitment_with_inputs_v2(
                    publication,
                    plan,
                    upstream,
                    &output,
                    None,
                    closure,
                ),
            )
            .unwrap();
        let state = store.load().unwrap().unwrap();
        assert!(matches!(
            recover_worker_v2_intent_v2(&store, &producer, state, closure),
            Err(RestartIntentErrorV2::Intent(_))
        ));
        assert_eq!(store.load().unwrap(), Some(state));
        assert!(
            recover_worker_v2_publication_intent_v2(&directory.0, &producer, current, closure,)
                .is_ok()
        );
    }

    #[test]
    fn ordinary_and_protected_intent_recovery_never_crosses_or_downgrades_schema() {
        let v1_directory = TestDirectory::new();
        let v1_producer = producer(112);
        let v1_attempt = attempt(&v1_directory.0, &v1_producer, 112);
        let (v1_output, v1_plan, v1_upstream) = publication_inputs(v1_attempt, 112);
        persist_worker_v2_publication_intent_v1(
            &v1_directory.0,
            &v1_producer,
            v1_attempt,
            v1_plan,
            v1_upstream,
            &v1_output,
        )
        .unwrap();
        let closure = compiler_closure(112);
        let publication = WorkerV2PublicationKindV1::Finalized;
        let protected_store = WorkerV2ResumeStoreV2::open(&v1_directory.0, &v1_producer).unwrap();
        protected_store
            .persist_pending(
                publication,
                v1_attempt,
                restart_admission_commitment_with_inputs_v2(
                    publication,
                    v1_plan,
                    v1_upstream,
                    &v1_output,
                    None,
                    closure,
                ),
            )
            .unwrap();
        let protected_state = protected_store.load().unwrap().unwrap();
        assert!(matches!(
            recover_worker_v2_intent_v2(&protected_store, &v1_producer, protected_state, closure,),
            Err(RestartIntentErrorV2::Intent(
                WorkerV2PublicationIntentErrorV2::NotFound
            ))
        ));
        assert_eq!(protected_store.load().unwrap(), Some(protected_state));

        let v2_directory = TestDirectory::new();
        let v2_producer = producer(113);
        let v2_attempt = attempt(&v2_directory.0, &v2_producer, 113);
        let (v2_output, v2_plan, v2_upstream) = publication_inputs(v2_attempt, 113);
        persist_worker_v2_publication_intent_v2(
            &v2_directory.0,
            &v2_producer,
            v2_attempt,
            v2_plan,
            v2_upstream,
            closure,
            &v2_output,
        )
        .unwrap();
        let ordinary_store = WorkerV2ResumeStoreV1::open(&v2_directory.0, &v2_producer).unwrap();
        ordinary_store
            .persist_pending(
                publication,
                v2_attempt,
                restart_admission_commitment_v1(publication, v2_plan, v2_upstream, &v2_output),
            )
            .unwrap();
        let ordinary_state = ordinary_store.load().unwrap().unwrap();
        assert!(matches!(
            recover_worker_v2_intent_v1(&ordinary_store, &v2_producer, ordinary_state),
            Err(RestartIntentErrorV1::Intent(
                WorkerV2PublicationIntentErrorV1::NotFound
            ))
        ));
        assert_eq!(ordinary_store.load().unwrap(), Some(ordinary_state));
        assert!(
            recover_worker_v2_publication_intent_v2(
                &v2_directory.0,
                &v2_producer,
                v2_attempt,
                closure,
            )
            .is_ok()
        );
    }

    #[test]
    fn protected_admission_binds_the_complete_compiler_closure_under_a_new_domain() {
        let attempt = fixed_attempt();
        let (output, plan, upstream) = publication_inputs(attempt, 114);
        let publication = WorkerV2PublicationKindV1::Finalized;
        let first = restart_admission_commitment_with_inputs_v2(
            publication,
            plan,
            upstream,
            &output,
            None,
            compiler_closure(114),
        );
        let second = restart_admission_commitment_with_inputs_v2(
            publication,
            plan,
            upstream,
            &output,
            None,
            compiler_closure(115),
        );
        assert_ne!(first, second);
        assert_ne!(
            first,
            restart_admission_commitment_with_inputs_v1(publication, plan, upstream, &output, None,)
        );
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
